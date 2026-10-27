package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.ShareAvailability
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.Ended
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.SharePanelContent
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.ui.KnTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * KyberNow Share, l'interface : « Partager » impossible (estompé, sélectionnable, panneau réduit à la raison),
 * les actions sur un invité dans l'ordre de docs/link-ui.md, les durées, la barre et les fins d'un invité. En français
 * (les textes sont ceux de link-ui), sans animations sans fin (`motion = false`).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class ShareUiTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()
    private val reason = "Le partage demande KyberNow Station 0.8.0 ou plus récente."

    private class Actions : SessionActions {
        var shares = 0
        var quits = 0
        override fun resume() {}
        override fun quit() { quits++ }
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
        override fun share() { shares++ }
        override fun shareNewCode() {}
        override fun shareSetRole(guest: ShareGuest, role: ShareRole) {}
        override fun shareRemove(guest: ShareGuest) {}
        override fun shareStop() {}
        override fun shareCopyLink() {}
    }

    private val actions = Actions()

    private fun ui(tv: Boolean = false, guest: Boolean = false, role: ShareRole = ShareRole.SPECTATOR, setup: SessionUi.() -> Unit = {}): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        state.shareRole.value = role
        return SessionUi(if (guest) "Invité sur Atelier" else "Atelier", tv, state, actions, guest = guest, computerName = "Atelier").apply { passageDone = true; setup() }
    }

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    // MARK: - « Partager » impossible

    @Test
    fun unavailableShare_staysSelectableOnTheBar_andTheReasonIsNotPermanentlyShown() {
        val ui = ui { barOpen = true; shareAvailability = ShareAvailability.UNSUPPORTED; shareUnavailable = reason }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        // Estompé, mais sélectionnable : au toucher il ouvre le panneau réduit à la raison (SessionActivity.share).
        rule.onNodeWithText("Partager").assertHasClickAction().assertIsEnabled().performClick()
        assertEquals(1, actions.shares)
        // Plus de ligne permanente sous la barre : la raison est dans le panneau (toucher) ou l'infobulle (souris).
        rule.onNodeWithText(reason).assertDoesNotExist()
    }

    @Test
    fun unavailableShare_staysSelectableInTheTelevisionMenu() {
        val ui = ui(tv = true) { menuOpen = true; shareAvailability = ShareAvailability.DISABLED; shareUnavailable = "Le partage est coupé sur cet ordinateur (Admin › Appareils Link)." }
        show(FormFactor.TV) { SessionOverlay(ui) }
        rule.onNodeWithText("Partager").performClick()
        assertEquals(1, actions.shares)
    }

    @Test
    fun availableShare_opensAsBefore() {
        val ui = ui { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Partager").performClick()
        assertEquals(1, actions.shares)
        assertTrue(ui.shareUsable)
    }

    @Test
    fun theReasonPanel_isReducedToTheReason() {
        show(FormFactor.PHONE) { SharePanelContent("Atelier", null, false, null, null, false, actions, {}, unavailable = reason) }
        rule.onNodeWithText("Partager Atelier").assertExists()
        rule.onNodeWithText(reason).assertExists()
        rule.onNodeWithText("Fermer").assertHasClickAction()
        rule.onNodeWithText("Copier le lien").assertDoesNotExist()
        rule.onNodeWithText("Invités").assertDoesNotExist()
        rule.onNodeWithText("Arrêter le partage").assertDoesNotExist()
    }

    // MARK: - Les invités de l'hôte

    private fun share(vararg guests: ShareGuest) =
        ShareState("428913", "428 913", "kybernow://join?host=192.168.1.20&port=41234&fp=sha256:ab&code=428913", "https://192.168.1.20:41234/link/?join=428913", guests.toList())

    private val guestLabels = listOf(
        "Faire jouer", "Faire regarder", "Donner le clavier et la souris", "Reprendre le clavier et la souris", "Retirer",
    )

    @Test
    fun guestActions_followTheCommonOrder() {
        val state = share(
            ShareGuest("g1", "Léa", "ios", ShareRole.SPECTATOR),
            ShareGuest("g2", "Max", "web", ShareRole.PLAYER),
            ShareGuest("g3", "Zoé", "android", ShareRole.PLAYER_KBM),
        )
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        val shown = rule.onAllNodes(hasClickAction()).fetchSemanticsNodes().mapNotNull { it.config.getOrNull(SemanticsProperties.Text)?.firstOrNull()?.text }.filter { it in guestLabels }
        // Spectateur : « Faire jouer ». Joueur : « Faire regarder » à la place, puis le clavier et la souris. Et « Retirer ».
        assertEquals(
            listOf(
                "Faire jouer", "Retirer",
                "Faire regarder", "Donner le clavier et la souris", "Retirer",
                "Faire regarder", "Reprendre le clavier et la souris", "Retirer",
            ),
            shown,
        )
    }

    @Test
    fun durations_areShortMinutesOrHours_andNeverNegative() {
        val now = System.currentTimeMillis() / 1000
        val state = share(
            ShareGuest("g1", "Léa", "ios", ShareRole.SPECTATOR, since = now - 42),
            ShareGuest("g2", "Max", "web", ShareRole.SPECTATOR, since = now - 200),
            ShareGuest("g3", "Zoé", "android", ShareRole.SPECTATOR, since = now - 4350),
            // Horloges décalées : l'arrivée est dans l'avenir de la Station.
            ShareGuest("g4", "Hugo", "mac", ShareRole.SPECTATOR, since = now + 3600),
        )
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        rule.onNodeWithText("iPhone ou iPad · depuis moins d'une minute").assertExists()
        rule.onNodeWithText("Navigateur · depuis 3 min").assertExists()
        rule.onNodeWithText("Android · depuis 1 h 12").assertExists()
        rule.onNodeWithText("Mac · depuis moins d'une minute").assertExists()
    }

    @Test
    fun aGuestWithoutArrivalTime_showsOnlyItsApp() {
        val state = share(ShareGuest("g5", "Eva", "windows", ShareRole.SPECTATOR))
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        rule.onNodeWithText("Windows").assertExists()
    }

    // MARK: - L'invité

    @Test
    fun guestBar_mouseModeOnlyWithKeyboardAndMouse() {
        val withKm = ui(guest = true, role = ShareRole.PLAYER_KBM) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(withKm) }
        rule.onNodeWithText("Souris", substring = true).assertExists()
        rule.onNodeWithText("Pavé tactile").assertExists()
        rule.onNodeWithText("Joueur").assertExists()
        rule.onNodeWithText("Quitter le partage").assertExists()
        // Ni profil, ni « Partager ».
        rule.onNodeWithText("Partager").assertDoesNotExist()
    }

    @Test
    fun guestBar_player_hasNoMouseNorKeyboard() {
        val player = ui(guest = true, role = ShareRole.PLAYER) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(player) }
        rule.onNodeWithText("Manette à l'écran").assertExists()
        rule.onNodeWithText("Souris", substring = true).assertDoesNotExist()
        rule.onNodeWithText("Pavé tactile").assertDoesNotExist()
    }

    @Test
    fun guestBar_spectator_hasOnlyTheDeviceItems() {
        val spectator = ui(guest = true, role = ShareRole.SPECTATOR) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(spectator) }
        rule.onNodeWithText("Spectateur").assertExists()
        rule.onNodeWithText("Manette à l'écran").assertDoesNotExist()
        rule.onNodeWithText("Souris", substring = true).assertDoesNotExist()
        rule.onNodeWithText("Quitter le partage").assertHasClickAction()
    }

    @Test
    fun reconnectingCapsule_aGuestLeavesTheShare_aHostDisconnects() {
        val guest = ui(guest = true) { state.capsule.value = Capsule.Reconnecting(1, 5) }
        show(FormFactor.PHONE) { SessionOverlay(guest) }
        rule.onNodeWithText("Quitter le partage").performClick()
        assertEquals(1, actions.quits)
    }

    @Test
    fun hostReconnectingCapsule_keepsDisconnect() {
        val host = ui { state.capsule.value = Capsule.Reconnecting(1, 5) }
        show(FormFactor.PHONE) { SessionOverlay(host) }
        rule.onNodeWithText("Se déconnecter").assertHasClickAction()
    }

    @Test
    fun theEndOfAShare_keepsTheHostNameAsWritten() {
        val text = "iPhone de Léa a arrêté le partage."
        val ui = ui(guest = true, role = ShareRole.PLAYER)
        ui.state.onShareEnd(0, text)
        ui.state.phase.value = Phase.ENDED
        ui.state.ended.value = Ended(text, null, null, null)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText(text).assertExists()
        rule.onNodeWithText("IPhone de Léa a arrêté le partage.").assertDoesNotExist()
        rule.onNodeWithText("Retour à l'accueil").assertHasClickAction()
    }

    @Test
    fun otherErrors_stillBecomeASentence() {
        val ui = ui()
        ui.state.phase.value = Phase.ENDED
        ui.state.ended.value = Ended("l'ordinateur ne répond plus", null, null, null)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("L'ordinateur ne répond plus.").assertExists()
    }

    @Test
    fun keyboardAndMouse_onlyForAHostOrAGuestWhoWasGivenThem() {
        assertTrue(ui().keyboardMouseAllowed())
        assertFalse(ui(guest = true, role = ShareRole.SPECTATOR).keyboardMouseAllowed())
        assertFalse(ui(guest = true, role = ShareRole.PLAYER).keyboardMouseAllowed())
        assertTrue(ui(guest = true, role = ShareRole.PLAYER_KBM).keyboardMouseAllowed())
        // Le rôle qui change en cours de partage : l'interface le suit.
        val g = ui(guest = true, role = ShareRole.PLAYER_KBM)
        g.state.onShareRole(ShareRole.PLAYER.code, ShareRole.PLAYER_KBM.code)
        assertFalse(g.keyboardMouseAllowed())
    }
}
