package fr.leslignesclaires.kybernow

import android.app.Application
import android.os.Looper
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.RecordingState
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.ui.KnTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Enregistrement de la session (docs/enregistrement.md ; textes de docs/link-ui.md, « Enregistrement de la session ») :
 * l'état du cœur rangé pour l'interface (le temps compté par l'appareil), l'avis bref, le bouton de la barre, la
 * pastille en haut de l'image, la tuile du téléviseur, l'invité, et rien face à une Station qui n'enregistre pas.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class RecordingTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()

    private class Actions : SessionActions {
        val calls = mutableListOf<String>()
        override fun resume() {}
        override fun quit() {}
        override fun setProfile(p: Profile) {}
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
        override fun record() { calls += "enregistrer" }
    }

    private val actions = Actions()

    /** Le JSON de `RecordingState::ui_json` du cœur, en français. */
    private fun json(active: Boolean, allowed: Boolean = true, elapsed: Long = 134) = """
        {"available":true,"allowed":$allowed,"active":$active,"elapsed":$elapsed,"texts":{
        "record":"Enregistrer","pill":"Enregistrement · {time}","record_tooltip":"Enregistrer la session sur l'ordinateur ({shortcut})",
        "stop_tooltip":"Arrêter l'enregistrement ({shortcut})","stop":"Arrêter l'enregistrement","on_computer":"Sur l'ordinateur",
        "record_menu":"Enregistrer la session","stop_menu":"Arrêter l'enregistrement ({time})","guest_pill":"Enregistrement",
        "guest_tooltip":"Atelier enregistre la session.","disabled":"L'enregistrement est coupé sur cet ordinateur (Admin › Enregistrements)."}}
    """.trimIndent()

    private var now = 10_000L

    private fun state() = SessionState(context, Gamepads(context)).apply {
        clockMs = { now }
        phase.value = Phase.STREAMING
        firstImageAt.set(1)
    }

    private fun ui(tv: Boolean = false, guest: Boolean = false, setup: SessionUi.() -> Unit = {}) =
        SessionUi("Atelier", tv, state(), actions, guest = guest, computerName = "Atelier").apply { passageDone = true; setup() }

    private fun show(form: FormFactor, ui: SessionUi) = rule.setContent { KnTheme(form, motion = false) { SessionOverlay(ui) } }

    // MARK: - L'état

    @Test
    fun theClock_isTheCoresFormat() {
        assertEquals("00:00", RecordingState.clock(0))
        assertEquals("02:14", RecordingState.clock(134))
        assertEquals("59:59", RecordingState.clock(3599))
        assertEquals("1:02:14", RecordingState.clock(3734))
    }

    @Test
    fun theTime_isCountedByTheDevice_fromTheStationsElapsed() {
        val r = RecordingState.parse(json(active = true), 1_000)!!
        assertEquals("Enregistrement · 02:14", r.pill(1_000))
        assertEquals("Enregistrement · 02:16", r.pill(3_500))
        assertEquals("Arrêter l'enregistrement", r.action)
        val idle = RecordingState.parse(json(active = false), 1_000)!!
        assertEquals(0, idle.seconds(9_000))
        assertEquals("Enregistrer", idle.action)
        assertNull(RecordingState.parse("pas du json", 0))
    }

    @Test
    fun theCoresNotice_isShownAsIs() {
        val s = state()
        s.onRecording(json(active = true), "Enregistrement lancé sur l'ordinateur.")
        shadowOf(Looper.getMainLooper()).idle()
        assertTrue(s.recording.value!!.active)
        assertEquals(Capsule.RecordingNotice("Enregistrement lancé sur l'ordinateur."), s.capsule.value)
        s.capsule.value = null
        s.onRecording(json(active = true), null)
        shadowOf(Looper.getMainLooper()).idle()
        assertNull(s.capsule.value)
    }

    // MARK: - L'interface

    @Test
    fun withoutRecording_nothingIsShown() {
        show(FormFactor.PHONE, ui { barOpen = true })
        rule.onNodeWithText("Enregistrer").assertDoesNotExist()
        rule.onNodeWithText("Enregistrement", substring = true).assertDoesNotExist()
    }

    @Test
    fun theHostsBar_hasRecord_thenStop_andThePillStaysOnTheImage() {
        val ui = ui { barOpen = true }
        ui.state.recording.value = RecordingState.parse(json(active = false), now)
        show(FormFactor.PHONE, ui)
        rule.onNodeWithText("Enregistrer").assertHasClickAction().performClick()
        assertEquals(listOf("enregistrer"), actions.calls)
        ui.state.recording.value = RecordingState.parse(json(active = true), now)
        rule.onNodeWithText("Arrêter l'enregistrement").assertHasClickAction()
        rule.onNodeWithText("Enregistrement · 02:14").assertExists()
        // La barre fermée : la pastille reste.
        ui.barOpen = false
        rule.onNodeWithText("Enregistrement · 02:14").assertExists()
    }

    @Test
    fun turnedOffInAdmin_theButtonStaysSelectable() {
        val ui = ui { barOpen = true }
        ui.state.recording.value = RecordingState.parse(json(active = false, allowed = false), now)
        show(FormFactor.PHONE, ui)
        rule.onNodeWithText("Enregistrer").assertHasClickAction().performClick()
        assertEquals(listOf("enregistrer"), actions.calls)
    }

    @Test
    fun theTvTile_saysOnTheComputer_thenTheTime() {
        val ui = ui(tv = true) { menuOpen = true }
        ui.state.recording.value = RecordingState.parse(json(active = false), now)
        show(FormFactor.TV, ui)
        rule.onNodeWithText("Sur l'ordinateur").assertExists()
        rule.onNodeWithText("Enregistrer").performClick()
        assertEquals(listOf("enregistrer"), actions.calls)
        ui.state.recording.value = RecordingState.parse(json(active = true), now)
        rule.onNodeWithText("Arrêter l'enregistrement").assertExists()
        rule.onNodeWithText("02:14").assertExists()
    }

    @Test
    fun aGuest_seesThePillOnly() {
        val ui = ui(guest = true) { barOpen = true }
        ui.state.recording.value = RecordingState.parse(json(active = true), now)
        show(FormFactor.PHONE, ui)
        rule.onNodeWithText("Enregistrement").assertExists()
        rule.onNodeWithText("Arrêter l'enregistrement").assertDoesNotExist()
        rule.onNodeWithText("Enregistrer").assertDoesNotExist()
    }
}
