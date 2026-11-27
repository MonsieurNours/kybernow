package fr.leslignesclaires.kybernow.model

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import androidx.core.content.FileProvider
import java.io.File
import java.io.InputStream
import java.io.PrintWriter
import java.io.StringWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/**
 * Le journal de l'app (AND-01), pour comprendre un plantage sans `adb` :
 * - le pont natif copie ce que le cœur et Kyber écrivent dans `files/journal/natif.log` (tournant, 2 × 1 Mo), paniques
 *   Rust comprises (fil, emplacement, pile) : voir `jni/src/journal.rs` ;
 * - une exception Kotlin non rattrapée est écrite dans `java.log`, puis Android arrête l'app comme d'habitude ;
 * - au lancement suivant, la raison de la fin précédente (`ApplicationExitInfo` : plantage, plantage natif, ANR,
 *   mémoire…) est écrite dans `fins.log` et dans logcat, avec la trace qu'Android en garde (ANR ; plantage natif à
 *   partir d'Android 12) ;
 * - « Envoyer le journal » (Réglages › À propos) réunit le tout, plus ce que logcat a encore de l'app, dans un fichier
 *   texte partagé par la feuille de partage d'Android ; au téléviseur, chiffré et déposé sur le relais, ouvert par le
 *   téléphone qui scanne un QR code ([JournalSeal], `ui/settings/JournalQr.kt`).
 *
 * Ni mot de passe, ni jeton, ni code n'y sont écrits par l'app : le cœur et Kyber ne les mettent pas au journal.
 */
object Journal {
    const val TAG = "KyberNow"
    /** Taille au-delà de laquelle `java.log` et `fins.log` repartent de leur dernière moitié. */
    const val MAX_SMALL = 256 * 1024
    /** Trace d'une fin (ANR, tombstone) : au plus ceci. */
    const val MAX_TRACE = 96 * 1024
    /** Lignes de logcat jointes au journal envoyé. */
    const val LOGCAT_LINES = 8000

    fun dir(context: Context): File = File(context.filesDir, "journal").apply { mkdirs() }

    /** Les exceptions Kotlin non rattrapées, notées avant que le gestionnaire d'origine n'arrête l'app. */
    fun install(context: Context) {
        val dir = dir(context)
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, e ->
            runCatching {
                val text = crashText(thread.name, e)
                Log.e(TAG, text)
                append(File(dir, "java.log"), "${stamp(System.currentTimeMillis())} $text")
            }
            previous?.uncaughtException(thread, e)
        }
    }

    /** Une exception et sa pile, causes comprises. */
    fun crashText(thread: String, e: Throwable): String {
        val sw = StringWriter()
        e.printStackTrace(PrintWriter(sw))
        return "exception non rattrapée dans le fil « $thread » : $sw"
    }

    /**
     * Les fins du processus survenues depuis le dernier lancement, notées une fois chacune (la dernière vue est gardée
     * dans les préférences `journal`). Hors du fil principal : la trace d'un ANR se lit sur le disque.
     */
    fun recordExits(context: Context) {
        val am = context.getSystemService(ActivityManager::class.java) ?: return
        val prefs = context.getSharedPreferences("journal", Context.MODE_PRIVATE)
        val seen = prefs.getLong("last_exit", 0L)
        val exits = runCatching { am.getHistoricalProcessExitReasons(context.packageName, 0, 16) }.getOrNull() ?: return
        val fresh = exits.filter { it.timestamp > seen }.sortedBy { it.timestamp }
        if (fresh.isEmpty()) return
        val file = File(dir(context), "fins.log")
        for (x in fresh) {
            val line = exitLine(x.timestamp, x.reason, x.status, x.importance, x.description, x.pss, x.rss)
            Log.w(TAG, "fin précédente : $line")
            val trace = if (hasTrace(x.reason)) runCatching { x.traceInputStream?.use { readTrace(it, x.reason) } }.getOrNull() else null
            append(file, line + (trace?.let { "\n$it" } ?: ""))
        }
        prefs.edit().putLong("last_exit", fresh.last().timestamp).apply()
    }

    /** Les raisons qui ont une trace utile : ANR (texte), plantage natif (tombstone, Android 12 et après). */
    fun hasTrace(reason: Int) = reason == ApplicationExitInfo.REASON_ANR || reason == ApplicationExitInfo.REASON_CRASH_NATIVE

    /** Trace d'un ANR en texte ; celle d'un plantage natif est un tombstone protobuf : on en garde les textes lisibles. */
    fun readTrace(input: InputStream, reason: Int): String {
        val bytes = input.readNBytesCompat(if (reason == ApplicationExitInfo.REASON_CRASH_NATIVE) MAX_TRACE * 4 else MAX_TRACE)
        val text = if (reason == ApplicationExitInfo.REASON_CRASH_NATIVE) printable(bytes) else String(bytes, Charsets.UTF_8)
        return text.take(MAX_TRACE)
    }

    private fun InputStream.readNBytesCompat(max: Int): ByteArray {
        val out = java.io.ByteArrayOutputStream()
        val buf = ByteArray(8192)
        while (out.size() < max) {
            val n = read(buf, 0, minOf(buf.size, max - out.size()))
            if (n <= 0) break
            out.write(buf, 0, n)
        }
        return out.toByteArray()
    }

    /** Les suites lisibles (4 caractères ASCII ou plus) d'un fichier binaire, une par ligne, comme `strings`. */
    fun printable(bytes: ByteArray, min: Int = 4): String {
        val out = StringBuilder()
        val run = StringBuilder()
        fun flush() {
            if (run.length >= min) out.append(run).append('\n')
            run.setLength(0)
        }
        for (b in bytes) {
            val c = b.toInt() and 0xff
            if (c in 0x20..0x7e || c == 0x09) run.append(c.toChar()) else flush()
        }
        flush()
        return out.toString()
    }

    /** Une fin, sur une ligne : « 2026-11-27 14:03:07.042 UTC · plantage natif · état 11 · premier plan · … ». */
    fun exitLine(timestamp: Long, reason: Int, status: Int, importance: Int, description: String?, pssKb: Long, rssKb: Long): String =
        listOfNotNull(
            stamp(timestamp),
            reasonName(reason),
            "état $status",
            if (importance <= ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND) "premier plan" else "arrière-plan ($importance)",
            description?.takeIf { it.isNotBlank() },
            "mémoire ${pssKb / 1024} Mo (PSS), ${rssKb / 1024} Mo (RSS)",
        ).joinToString(" · ")

    fun reasonName(reason: Int): String = when (reason) {
        ApplicationExitInfo.REASON_EXIT_SELF -> "fin demandée par l'app"
        ApplicationExitInfo.REASON_SIGNALED -> "signal"
        ApplicationExitInfo.REASON_LOW_MEMORY -> "manque de mémoire"
        ApplicationExitInfo.REASON_CRASH -> "plantage (Java/Kotlin)"
        ApplicationExitInfo.REASON_CRASH_NATIVE -> "plantage natif"
        ApplicationExitInfo.REASON_ANR -> "ANR (l'app ne répondait plus)"
        ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "échec du démarrage"
        ApplicationExitInfo.REASON_PERMISSION_CHANGE -> "autorisation changée"
        ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE -> "ressources excessives"
        ApplicationExitInfo.REASON_USER_REQUESTED -> "arrêtée par l'utilisateur"
        ApplicationExitInfo.REASON_USER_STOPPED -> "utilisateur arrêté"
        ApplicationExitInfo.REASON_DEPENDENCY_DIED -> "dépendance arrêtée"
        ApplicationExitInfo.REASON_OTHER -> "autre (système)"
        ApplicationExitInfo.REASON_FREEZER -> "gelée par le système"
        ApplicationExitInfo.REASON_PACKAGE_STATE_CHANGE -> "app désactivée"
        ApplicationExitInfo.REASON_PACKAGE_UPDATED -> "app mise à jour"
        else -> "raison $reason"
    }

    /** Ajoute un bloc ; au-delà de [MAX_SMALL], seule la seconde moitié du fichier est gardée. */
    fun append(file: File, text: String) {
        file.parentFile?.mkdirs()
        if (file.length() > MAX_SMALL) {
            val keep = file.readText().takeLast(MAX_SMALL / 2)
            file.writeText(keep)
        }
        file.appendText(if (text.endsWith("\n")) text else text + "\n")
    }

    fun stamp(millis: Long): String =
        SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS 'UTC'", Locale.ROOT).apply { timeZone = TimeZone.getTimeZone("UTC") }.format(Date(millis))

    /** L'en-tête : version, appareil, Android, ce qui ne se devine pas du reste. */
    fun header(version: String, versionCode: Long, now: Long): String = buildString {
        append("KyberNow Link $version ($versionCode)\n")
        append("Appareil : ${Build.MANUFACTURER} ${Build.MODEL} (${Build.DEVICE})\n")
        append("Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT}), ${Build.SUPPORTED_ABIS.joinToString()}\n")
        append("Journal écrit le ${stamp(now)}\n")
    }

    /**
     * Le journal à envoyer (`files/journal/kybernow-journal.txt`) : en-tête, fins précédentes, exceptions Kotlin,
     * journal natif (l'ancien fichier puis le courant), puis logcat de l'app (le processus courant et les précédents,
     * tant qu'Android les garde). Sur un fil d'arrière-plan : logcat peut prendre une seconde.
     */
    fun build(context: Context): File {
        val dir = dir(context)
        val info = runCatching { context.packageManager.getPackageInfo(context.packageName, 0) }.getOrNull()
        val out = File(dir, "kybernow-journal.txt")
        out.bufferedWriter().use { w ->
            w.write(header(info?.versionName ?: "?", info?.longVersionCode ?: 0L, System.currentTimeMillis()))
            for (name in listOf("fins.log", "java.log", "natif.log.1", "natif.log")) {
                val f = File(dir, name)
                w.write("\n===== $name =====\n")
                if (f.isFile) f.reader().use { it.copyTo(w) } else w.write("(vide)\n")
            }
            w.write("\n===== logcat =====\n")
            val logcat = runCatching {
                val p = ProcessBuilder("logcat", "-d", "-v", "threadtime", "-b", "main,system,crash", "-t", LOGCAT_LINES.toString())
                    .redirectErrorStream(true).start()
                p.inputStream.bufferedReader().use { it.copyTo(w) }
                p.waitFor()
            }
            if (logcat.isFailure) w.write("(logcat illisible : ${logcat.exceptionOrNull()?.message})\n")
        }
        return out
    }

    /** La section qui suit l'en-tête et les fins précédentes dans [build]. */
    const val AFTER_HEAD = "\n===== java.log =====\n"

    /**
     * La fin de l'en-tête à garder quand le journal doit être coupé (dépôt par QR code, 2 Mio compressés au plus) :
     * l'appareil, la version et les fins précédentes, tout ce qui précède [AFTER_HEAD] (rien sans elle).
     */
    fun headEnd(text: ByteArray): Int {
        val mark = AFTER_HEAD.toByteArray(Charsets.UTF_8)
        outer@ for (i in 0..text.size - mark.size) {
            for (j in mark.indices) if (text[i + j] != mark[j]) continue@outer
            return i
        }
        return 0
    }

    /** La feuille de partage d'Android, avec le fichier du journal (FileProvider). */
    fun shareIntent(context: Context, file: File, title: String): Intent {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.images", file)
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_STREAM, uri)
            putExtra(Intent.EXTRA_SUBJECT, file.name)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        return Intent.createChooser(send, title).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
}
