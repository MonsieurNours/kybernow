package fr.leslignesclaires.kybernow.update

import android.app.PendingIntent
import android.content.ActivityNotFoundException
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.util.Log
import fr.leslignesclaires.kybernow.BuildConfig
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.Native
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.File

/** Où en est la mise à jour de l'app. */
sealed interface UpdateState {
    data object Idle : UpdateState
    data object Checking : UpdateState
    data class UpToDate(val at: Long) : UpdateState
    data class Available(val version: String, val notes: String, val page: String, val size: Long) : UpdateState
    data class Downloading(val version: String, val received: Long, val size: Long) : UpdateState
    data class Ready(val version: String, val file: File) : UpdateState
    /** Android doit d'abord autoriser l'app à installer des applications. */
    data class NeedsPermission(val version: String, val file: File) : UpdateState
    data class Installing(val version: String) : UpdateState
    data class Failed(val message: String, val retry: UpdateState?) : UpdateState

    /** Version proposée (pastille de l'accueil), s'il y en a une. */
    val offered: String?
        get() = when (this) {
            is Available -> version
            is Downloading -> version
            is Ready -> version
            is NeedsPermission -> version
            else -> null
        }
}

/**
 * Mises à jour de KyberNow Link : la dernière version publiée sur GitHub qui
 * contient `KyberNow-Link-Android.apk`, sur le canal choisi (stables, ou
 * aussi les bêtas), vérifiée par le manifeste signé (cœur Rust, même clé que
 * la Station et l'app Windows). L'APK téléchargé doit avoir la taille et
 * l'empreinte SHA-256 du manifeste ; Android vérifie ensuite qu'il est signé
 * par la même clé que l'app installée.
 *
 * Recherche au lancement puis toutes les 6 heures ; téléchargement et
 * installation à la demande (Android demande toujours de confirmer).
 */
class Updates(private val context: Context) {
    private val prefs = context.getSharedPreferences("updates", Context.MODE_PRIVATE)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var job: Job? = null

    private val _state = MutableStateFlow<UpdateState>(UpdateState.Idle)
    val state: StateFlow<UpdateState> = _state.asStateFlow()

    private val _beta = MutableStateFlow(prefs.getBoolean("beta", false))
    val beta: StateFlow<Boolean> = _beta.asStateFlow()

    private val dir get() = File(context.cacheDir, "updates")

    /** Recherches régulières, tant que l'app tourne. */
    fun start() {
        scope.launch {
            while (true) {
                checkIfDue()
                delay(PERIOD)
            }
        }
    }

    fun setBeta(on: Boolean) {
        if (on == _beta.value) return
        _beta.value = on
        prefs.edit().putBoolean("beta", on).apply()
        check()
    }

    /** Au retour dans l'app : recherche si la dernière date, installation qui attendait l'autorisation. */
    fun onResume() {
        val s = _state.value
        if (s is UpdateState.NeedsPermission && context.packageManager.canRequestPackageInstalls()) install(s.version, s.file)
        checkIfDue()
    }

    private fun checkIfDue() {
        val last = prefs.getLong("last_check", 0)
        if (System.currentTimeMillis() - last >= PERIOD) check()
    }

    /** Recherche maintenant (sans effet pendant un téléchargement ou une installation). */
    fun check() {
        when (_state.value) {
            is UpdateState.Checking, is UpdateState.Downloading, is UpdateState.Installing -> return
            else -> {}
        }
        job?.cancel()
        _state.value = UpdateState.Checking
        job = scope.launch {
            val json = runCatching { JSONObject(Native.updateCheck(BuildConfig.VERSION_NAME, _beta.value)) }.getOrNull()
            prefs.edit().putLong("last_check", System.currentTimeMillis()).apply()
            _state.value = when {
                json == null -> UpdateState.Failed("?", null)
                json.has("error") -> UpdateState.Failed(json.getString("error"), null)
                json.has("available") -> {
                    val a = json.getJSONObject("available")
                    val version = a.getString("version")
                    // Déjà téléchargée et vérifiée : prête.
                    val file = apk(version)
                    if (file.isFile && file.length() == a.getLong("size")) UpdateState.Ready(version, file)
                    else UpdateState.Available(version, a.optString("notes"), a.optString("page"), a.getLong("size"))
                }
                else -> UpdateState.UpToDate(System.currentTimeMillis())
            }
            Log.i(TAG, "mise à jour : ${_state.value}")
        }
    }

    private fun apk(version: String) = File(dir, "KyberNow-Link-Android-$version.apk")

    /** Le bouton de la page : télécharger, installer ou réessayer selon l'état. */
    fun act() {
        when (val s = _state.value) {
            is UpdateState.Available -> download(s)
            is UpdateState.Ready -> install(s.version, s.file)
            is UpdateState.NeedsPermission -> install(s.version, s.file)
            is UpdateState.Failed -> s.retry?.let { _state.value = it; act() } ?: check()
            else -> check()
        }
    }

    private fun download(a: UpdateState.Available) {
        _state.value = UpdateState.Downloading(a.version, 0, a.size)
        job = scope.launch {
            dir.deleteRecursively()
            dir.mkdirs()
            val dest = apk(a.version)
            val part = File(dir, dest.name + ".part")
            val error = runCatching {
                Native.updateDownload(part.path, object : Native.UpdateProgress {
                    override fun onProgress(received: Long) {
                        _state.value = UpdateState.Downloading(a.version, received, a.size)
                    }
                })
            }.getOrElse { it.message ?: it.javaClass.simpleName }
            if (error == null && part.renameTo(dest)) {
                _state.value = UpdateState.Ready(a.version, dest)
                install(a.version, dest)
            } else {
                part.delete()
                _state.value = UpdateState.Failed(error ?: "?", a)
            }
        }
    }

    private fun install(version: String, file: File) {
        val pm = context.packageManager
        if (!pm.canRequestPackageInstalls()) {
            _state.value = UpdateState.NeedsPermission(version, file)
            openPermission()
            return
        }
        _state.value = UpdateState.Installing(version)
        scope.launch {
            runCatching {
                val installer = pm.packageInstaller
                val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
                    setAppPackageName(context.packageName)
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
                }
                val id = installer.createSession(params)
                installer.openSession(id).use { session ->
                    session.openWrite("base.apk", 0, file.length()).use { out ->
                        file.inputStream().use { it.copyTo(out) }
                        session.fsync(out)
                    }
                    val intent = Intent(context, InstallReceiver::class.java).putExtra(EXTRA_VERSION, version)
                    val flags = PendingIntent.FLAG_UPDATE_CURRENT or (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
                    session.commit(PendingIntent.getBroadcast(context, id, intent, flags).intentSender)
                }
            }.onFailure { e ->
                Log.w(TAG, "installation impossible", e)
                _state.value = UpdateState.Failed(e.message ?: e.javaClass.simpleName, UpdateState.Ready(version, file))
            }
        }
    }

    /** Réglage « Installer des applis inconnues » de l'app ; au téléviseur, il n'existe pas toujours. */
    private fun openPermission() {
        val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        try {
            context.startActivity(intent)
        } catch (_: ActivityNotFoundException) {
            // Shield et certains téléviseurs : pas d'écran pour une seule app ; la page dit le chemin.
        } catch (_: SecurityException) {
        }
    }

    /** Réponse d'Android à la session d'installation. */
    internal fun installResult(version: String, status: Int, message: String?, confirm: Intent?) {
        val ready = apk(version).takeIf { it.isFile }?.let { UpdateState.Ready(version, it) }
        when (status) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                confirm?.let { runCatching { context.startActivity(it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) } }
            }
            // Android relance l'app à jour ; ce processus s'arrête.
            PackageInstaller.STATUS_SUCCESS -> dir.deleteRecursively()
            PackageInstaller.STATUS_FAILURE_ABORTED -> _state.value = ready ?: UpdateState.Idle
            PackageInstaller.STATUS_FAILURE_CONFLICT, PackageInstaller.STATUS_FAILURE_INCOMPATIBLE ->
                _state.value = UpdateState.Failed(context.getString(R.string.updates_conflict), null)
            else -> _state.value = UpdateState.Failed(context.getString(R.string.updates_install_failed, message ?: status.toString()), ready)
        }
        Log.i(TAG, "installation de la $version : état $status ${message ?: ""}")
    }

    companion object {
        private const val TAG = "KyberNow"
        const val EXTRA_VERSION = "version"
        private const val PERIOD = 6 * 3600 * 1000L
    }
}

/** Reçoit l'issue de l'installation (PackageInstaller). */
class InstallReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val version = intent.getStringExtra(Updates.EXTRA_VERSION) ?: return
        val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
        val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
        @Suppress("DEPRECATION")
        val confirm = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java) else intent.getParcelableExtra(Intent.EXTRA_INTENT)
        KyberNowApp.instance.updates.installResult(version, status, message, confirm)
    }
}
