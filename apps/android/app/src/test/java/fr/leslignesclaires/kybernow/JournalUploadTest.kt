package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.model.Journal
import fr.leslignesclaires.kybernow.model.JournalDeposit
import fr.leslignesclaires.kybernow.model.JournalPost
import fr.leslignesclaires.kybernow.model.JournalState
import fr.leslignesclaires.kybernow.model.JournalUpload
import fr.leslignesclaires.kybernow.ui.settings.JournalQrStep
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * « Envoyer le journal » au téléviseur (docs/relais.md, « Dépôt d'un journal ») : le dépôt `KNJ1` est fait par le cœur
 * (vecteurs d'essai dans `journal::upload`) ; ici, l'en-tête séparé du journal, les réponses du cœur et l'étape qui suit.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class JournalUploadTest {
    @Test
    fun headerAndBody() {
        val head = "KyberNow Link 0.11.8 (118)\nAppareil : Google Chromecast\n\n===== fins.log =====\nplantage natif\n"
        val (header, body) = JournalUpload.split((head + Journal.AFTER_HEAD + "ligne\n").toByteArray())
        assertEquals(head, header)
        assertEquals(Journal.AFTER_HEAD + "ligne\n", body)
        // Sans la marque : pas d'en-tête à garder.
        assertEquals("" to "tout\n", JournalUpload.split("tout\n".toByteArray()))
    }

    @Test
    fun coreAnswers() {
        val ok = JSONObject("""{"relay":"","id":"AAAAAAAAAAAAAAAAAAAAAA","owner":"BBBBBBBBBBBBBBBBBBBBBB","url":"https://share.kybernow.app/journal/AAAAAAAAAAAAAAAAAAAAAA#k=k","expires_in":3600}""")
        assertEquals(
            JournalPost.Posted(JournalDeposit("", "AAAAAAAAAAAAAAAAAAAAAA", "BBBBBBBBBBBBBBBBBBBBBB", "https://share.kybernow.app/journal/AAAAAAAAAAAAAAAAAAAAAA#k=k", 3600)),
            JournalUpload.readPost(ok),
        )
        assertEquals(JournalPost.OldRelay, JournalUpload.readPost(JSONObject("""{"error":91,"detail":"HTTP 404"}""")))
        assertEquals(JournalPost.TooMany, JournalUpload.readPost(JSONObject("""{"error":92,"detail":"HTTP 429/503"}""")))
        assertEquals(JournalPost.Unreachable, JournalUpload.readPost(JSONObject("""{"error":90,"detail":"dns"}""")))
        assertEquals(JournalPost.Unreachable, JournalUpload.readPost(JSONObject("""{"error":1,"detail":"adresse"}""")))
        assertEquals(JournalPost.Unreachable, JournalUpload.readPost(JSONObject("""{"id":"x"}""")))
        assertEquals(JournalPost.Unreachable, JournalUpload.readPost(null))

        assertEquals(JournalState.WAITING, JournalUpload.readState(0))
        assertEquals(JournalState.OPENED, JournalUpload.readState(1))
        assertEquals(JournalState.GONE, JournalUpload.readState(93))
        assertEquals(JournalState.UNKNOWN, JournalUpload.readState(90))
    }

    @Test
    fun stepAfterTheDeposit() {
        val deposit = JournalDeposit("relais.exemple.org:9443#sha256:ab", "AAAAAAAAAAAAAAAAAAAAAA", "BBBBBBBBBBBBBBBBBBBBBB", "https://relais.exemple.org:9443/journal/AAAAAAAAAAAAAAAAAAAAAA#k=k", 3600)
        val shown = JournalQrStep.after(JournalPost.Posted(deposit), 1000L)
        assertTrue(shown is JournalQrStep.Shown)
        shown as JournalQrStep.Shown
        assertEquals(1000L + 3_600_000L, shown.until)
        // Le suivi et le retrait passent par le même relais, empreinte épinglée comprise.
        assertEquals("relais.exemple.org:9443#sha256:ab", shown.relay)
        assertEquals(JournalQrStep.TooMany, JournalQrStep.after(JournalPost.TooMany, 0))
        assertEquals(JournalQrStep.OldRelay, JournalQrStep.after(JournalPost.OldRelay, 0))
        assertEquals(JournalQrStep.Unreachable, JournalQrStep.after(JournalPost.Unreachable, 0))
    }
}
