package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.model.JournalRelay
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.settings.JournalQrContent
import fr.leslignesclaires.kybernow.ui.settings.JournalQrStep
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** L'écran « Envoyer le journal » d'Android TV, en français : les textes de link-ui à chaque étape, et ses boutons. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w960dp-h540dp-land-xhdpi")
class JournalQrUiTest {
    @get:Rule val rule = createComposeRule()

    private var done = 0
    private var retried = 0

    private fun show(step: JournalQrStep) = rule.setContent {
        KnTheme(FormFactor.TV, motion = false) { JournalQrContent(step, onDone = { done++ }, onRetry = { retried++ }) }
    }

    @Test
    fun qrCodeWithItsSentences() {
        val relay = JournalRelay.parse("share.kybernow.app")!!
        show(JournalQrStep.Shown("https://share.kybernow.app/journal/AAAAAAAAAAAAAAAAAAAAAA#k=" + "A".repeat(43), relay, "AAAAAAAAAAAAAAAAAAAAAA", "BBBBBBBBBBBBBBBBBBBBBB", 0))
        rule.onNodeWithText("Envoyer le journal").assertExists()
        rule.onNodeWithText("Scannez ce QR code avec votre téléphone, puis enregistrez ou partagez le journal.").assertExists()
        rule.onNodeWithText("Chiffré sur cet appareil : seul ce QR code l'ouvre. Il sert une fois, pendant une heure.").assertExists()
        rule.onNodeWithText("Terminé").performClick()
        assertEquals(1, done)
    }

    @Test
    fun expiredOffersANewCode() {
        show(JournalQrStep.Expired)
        rule.onNodeWithText("Ce QR code a expiré.").assertExists()
        rule.onNodeWithText("Nouveau QR code").performClick()
        assertEquals(1, retried)
    }

    @Test
    fun unreachableRelayOffersToTryAgain() {
        show(JournalQrStep.Unreachable)
        rule.onNodeWithText("Le relais ne répond pas : vérifiez la connexion à Internet, puis réessayez.").assertExists()
        rule.onNodeWithText("Réessayer").performClick()
        assertEquals(1, retried)
    }

    @Test
    fun otherSteps() {
        show(JournalQrStep.Uploading)
        rule.onNodeWithText("Envoi du journal…").assertExists()
    }

    @Test
    fun openedOnThePhone() {
        show(JournalQrStep.Opened)
        rule.onNodeWithText("Journal ouvert sur le téléphone.").assertExists()
        rule.onNodeWithText("Nouveau QR code").assertDoesNotExist()
    }
}
