package fr.leslignesclaires.kybernow

import android.app.ActivityManager
import android.app.Application
import android.app.ApplicationExitInfo
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.model.Journal
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

/** Le journal de l'app (AND-01) : fins précédentes, exceptions, fichier envoyé. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class JournalTest {
    private val context: Application get() = ApplicationProvider.getApplicationContext()

    @Test
    fun exitLineSaysWhatHappened() {
        val line = Journal.exitLine(1_795_788_187_042L, ApplicationExitInfo.REASON_CRASH_NATIVE, 11, ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND, null, 512_000, 700_000)
        assertEquals("2026-11-27 14:03:07.042 UTC · plantage natif · état 11 · premier plan · mémoire 500 Mo (PSS), 683 Mo (RSS)", line)
        assertTrue(Journal.exitLine(0, ApplicationExitInfo.REASON_ANR, 0, 400, "Input dispatching timed out", 0, 0).contains("ANR (l'app ne répondait plus) · état 0 · arrière-plan (400) · Input dispatching timed out"))
        assertEquals("raison 999", Journal.reasonName(999))
    }

    @Test
    fun tombstoneKeepsReadableText() {
        val bytes = byteArrayOf(0x0a, 0x05) + "libvulkan.so".toByteArray() + byteArrayOf(0, 1, 2) + "ab".toByteArray() + byteArrayOf(0x12) + "SIGSEGV".toByteArray()
        assertEquals("libvulkan.so\nSIGSEGV\n", Journal.printable(bytes))
        assertTrue(Journal.hasTrace(ApplicationExitInfo.REASON_ANR) && Journal.hasTrace(ApplicationExitInfo.REASON_CRASH_NATIVE))
        assertTrue(!Journal.hasTrace(ApplicationExitInfo.REASON_LOW_MEMORY))
        assertEquals("trace", Journal.readTrace("trace".byteInputStream(), ApplicationExitInfo.REASON_ANR))
    }

    @Test
    fun smallFilesStayBounded() {
        val f = File(Journal.dir(context), "essai.log").apply { delete() }
        val block = "x".repeat(10_000)
        repeat(60) { Journal.append(f, "$it $block") }
        assertTrue(f.length() <= Journal.MAX_SMALL + 20_000)
        assertTrue(f.readText().contains("59 x"))
    }

    @Test
    fun uncaughtExceptionIsWritten() {
        val text = Journal.crashText("main", IllegalStateException("essai", RuntimeException("cause")))
        assertTrue(text.startsWith("exception non rattrapée dans le fil « main » : java.lang.IllegalStateException: essai"))
        assertTrue(text.contains("Caused by: java.lang.RuntimeException: cause"))
    }

    @Test
    fun builtJournalGathersEverything() {
        val dir = Journal.dir(context)
        File(dir, "natif.log").writeText("2026-11-27 14:03:07.042 panique dans le fil « kybernow-image » : essai\n")
        File(dir, "fins.log").writeText("2026-11-27 · plantage natif\n")
        val out = Journal.build(context)
        val text = out.readText()
        assertTrue(text.startsWith("KyberNow Link "))
        assertTrue(text.contains("===== fins.log =====\n2026-11-27 · plantage natif"))
        assertTrue(text.contains("===== java.log =====\n(vide)"))
        assertTrue(text.contains("panique dans le fil « kybernow-image »"))
        assertTrue(text.contains("===== logcat ====="))
        val share = Journal.shareIntent(context, out, "Envoyer le journal")
        assertEquals(Intent.ACTION_CHOOSER, share.action)
        @Suppress("DEPRECATION")
        val send = share.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)!!
        assertEquals("text/plain", send.type)
        assertTrue(send.getParcelableExtra<android.net.Uri>(Intent.EXTRA_STREAM).toString().endsWith("/journal/kybernow-journal.txt"))
    }
}
