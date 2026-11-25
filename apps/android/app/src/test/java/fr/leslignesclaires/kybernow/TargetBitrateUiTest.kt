package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.AccountSummary
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.session.HISTORY_SECONDS
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.session.TargetBitrate
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.account.AccountsRows
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * « Débit visé » dans les Statistiques (docs/link-ui.md, « Statistiques »), en français et sans bibliothèque native (le
 * texte du cœur remplacé par les Mbit/s) : les valeurs 22 à 24 de la seconde, la courbe des 30 dernières secondes, la
 * ligne sous « Débit » ; et l'interrupteur « Synchroniser les réglages des ordinateurs » des réglages de l'app.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class TargetBitrateUiTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    /** Une seconde du cœur : débit reçu (octets), puis 22 à 24 (−1 : sans débit visé). */
    private fun second(bytes: Double, target: Long, profile: Long, reason: Int) = DoubleArray(25).also {
        it[9] = bytes
        it[22] = target.toDouble()
        it[23] = profile.toDouble()
        it[24] = reason.toDouble()
    }

    @Test
    fun laSeconde_porteLeDebitVise_etLaCourbeGarde30Secondes() {
        val state = SessionState(context, Gamepads(context))
        state.onSecond(second(5_000_000.0, 42_000_000, 60_000_000, 1))
        assertEquals(TargetBitrate(42_000_000, 60_000_000, 1), state.second.value!!.target)
        // −1 : pas de débit visé (invité sur l'image de l'hôte) ; un cœur plus ancien n'a pas ces valeurs.
        state.onSecond(second(5_000_000.0, -1, -1, -1))
        assertNull(state.second.value!!.target)
        state.onSecond(DoubleArray(22))
        assertNull(state.second.value!!.target)
        repeat(40) { state.onSecond(second(7_500_000.0, 60_000_000, 60_000_000, 0)) }
        val h = state.bitrateHistory.value
        assertEquals(HISTORY_SECONDS, h.size)
        assertEquals(60.0, h.last().target!!, 0.0)
        assertEquals(60.0, h.last().mbps, 0.0)
        state.close()
    }

    @Test
    fun lesStatistiques_montrentLeDebitVise_sousDebit() {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        repeat(5) { state.onSecond(second(5_000_000.0, 42_000_000, 60_000_000, 1)) }
        val ui = SessionUi("Atelier", false, state, Actions()).apply { passageDone = true; statsOpen = true }
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText("Débit visé 42 / 60 Mbit/s").assertExists()
        rule.onNodeWithContentDescription("Débit").assertExists()
        // Sans débit visé : ni ligne, ni trait.
        state.onSecond(second(5_000_000.0, -1, -1, -1))
        rule.waitForIdle()
        rule.onNodeWithText("Débit visé", substring = true).assertDoesNotExist()
        state.close()
    }

    @Test
    fun lInterrupteur_desReglagesSynchronises_avecUnCompte() {
        val lea = AccountSummary("share.kybernow.app|acc_lea", "Léa Martin", "lea@exemple.fr", "share.kybernow.app", "https://share.kybernow.app/compte")
        val changes = mutableListOf<Boolean>()
        show(FormFactor.PHONE) { Column { AccountsRows(listOf(lea), false, {}, {}, {}, {}, syncSettings = true, onSyncSettings = { changes += it }) } }
        rule.onNodeWithText("Le profil, le format, le débit, le HDR et le mode immersif de chaque ordinateur vous suivent sur vos appareils.").assertExists()
        rule.onNodeWithText("Synchroniser les réglages des ordinateurs").performClick()
        assertEquals(listOf(false), changes)
    }

    @Test
    fun lInterrupteur_absentSansCompte() {
        show(FormFactor.PHONE) { Column { AccountsRows(emptyList(), false, {}, {}, {}, {}) } }
        rule.onNodeWithText("Synchroniser les réglages des ordinateurs").assertDoesNotExist()
    }

    private class Actions : SessionActions {
        override fun resume() {}
        override fun quit() {}
        override fun setProfile(p: Profile) {}
        override fun switchToHevc() {}
        override fun setMouseMode(m: Int) {}
        override fun setDirectTouch(on: Boolean) {}
        override fun setRemoteMouse(on: Boolean) {}
        override fun setPadMouse(on: Boolean) {}
        override fun shortcut(s: Shortcut) {}
        override fun key(scancode: Int) {}
        override fun text(s: String) {}
        override fun restoreMode() {}
        override fun pickDisplay(id: Int) {}
        override fun openKeyboard(open: Boolean) {}
        override fun setOnScreenPad(on: Boolean) {}
        override fun setPadLayout(simple: Boolean) {}
        override fun share() {}
        override fun shareNewCode() {}
        override fun shareSetPads(guest: ShareGuest, on: Boolean) {}
        override fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean) {}
        override fun shareAccept(guest: ShareGuest) {}
        override fun shareRefuse(guest: ShareGuest) {}
        override fun shareRemove(guest: ShareGuest) {}
        override fun shareStop() {}
        override fun shareCopyLink() {}
        override fun setShareQuality(quality: ShareQuality) {}
    }
}
