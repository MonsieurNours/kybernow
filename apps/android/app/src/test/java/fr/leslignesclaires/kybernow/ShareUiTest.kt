package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.ShareAvailability
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareImage
import fr.leslignesclaires.kybernow.core.ShareLink
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.core.ShareTarget
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
import fr.leslignesclaires.kybernow.ui.share.JoinContent
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
        val rights = mutableListOf<String>()
        override fun shareSetPads(guest: ShareGuest, on: Boolean) { rights += "${guest.name} manettes $on" }
        override fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean) { rights += "${guest.name} clavier et souris $on" }
        override fun shareRemove(guest: ShareGuest) {}
        override fun shareStop() {}
        override fun shareCopyLink() {}
        val qualities = mutableListOf<ShareQuality>()
        override fun setShareQuality(quality: ShareQuality) { qualities += quality }
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
        "Donner les manettes", "Reprendre les manettes", "Donner le clavier et la souris", "Reprendre le clavier et la souris", "Retirer",
    )

    private fun shownActions() = rule.onAllNodes(hasClickAction()).fetchSemanticsNodes().mapNotNull { it.config.getOrNull(SemanticsProperties.Text)?.firstOrNull()?.text }.filter { it in guestLabels }

    @Test
    fun guestActions_followTheCommonOrder_twoIndependentRights() {
        val state = share(
            ShareGuest("g1", "Léa", "ios", ShareRole.SPECTATOR),
            ShareGuest("g2", "Max", "web", ShareRole.PLAYER),
            ShareGuest("g3", "Zoé", "android", ShareRole.KBM),
            ShareGuest("g4", "Eva", "mac", ShareRole.PLAYER_KBM),
        )
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        // Les manettes, puis le clavier et la souris, chacun « Donner » ou « Reprendre » selon l'invité, puis « Retirer ».
        assertEquals(
            listOf(
                "Donner les manettes", "Donner le clavier et la souris", "Retirer",
                "Reprendre les manettes", "Donner le clavier et la souris", "Retirer",
                "Donner les manettes", "Reprendre le clavier et la souris", "Retirer",
                "Reprendre les manettes", "Reprendre le clavier et la souris", "Retirer",
            ),
            shownActions(),
        )
        // « Faire jouer » et « Faire regarder » n'existent plus.
        rule.onNodeWithText("Faire jouer").assertDoesNotExist()
        rule.onNodeWithText("Faire regarder").assertDoesNotExist()
    }

    @Test
    fun guestLabels_nameTheirRights() {
        val state = share(
            ShareGuest("g1", "Léa", "ios", ShareRole.SPECTATOR),
            ShareGuest("g2", "Max", "web", ShareRole.PLAYER),
            ShareGuest("g3", "Zoé", "android", ShareRole.KBM),
            ShareGuest("g4", "Eva", "mac", ShareRole.PLAYER_KBM),
        )
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        rule.onNodeWithText("Léa · Spectateur").assertExists()
        rule.onNodeWithText("Max · Joueur").assertExists()
        rule.onNodeWithText("Zoé · Clavier et souris").assertExists()
        rule.onNodeWithText("Eva · Joueur, clavier et souris").assertExists()
    }

    @Test
    fun eachButton_changesItsOwnRightOnly() {
        val state = share(
            ShareGuest("g2", "Max", "web", ShareRole.PLAYER),
            ShareGuest("g4", "Eva", "mac", ShareRole.PLAYER_KBM),
        )
        actions.rights.clear()
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        // Max (manettes) : « Donner le clavier et la souris » ne parle que du clavier et de la souris.
        rule.onAllNodesWithText("Donner le clavier et la souris")[0].performClick()
        // Eva (les deux) : chaque reprise à part.
        rule.onAllNodesWithText("Reprendre les manettes")[1].performClick()
        rule.onAllNodesWithText("Reprendre le clavier et la souris")[0].performClick()
        // Max : « Reprendre les manettes » (le dernier droit).
        rule.onAllNodesWithText("Reprendre les manettes")[0].performClick()
        assertEquals(
            listOf("Max clavier et souris true", "Eva manettes false", "Eva clavier et souris false", "Max manettes false"),
            actions.rights,
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
        rule.onNodeWithText("Navigateur · depuis 3\u00a0min").assertExists()
        rule.onNodeWithText("Android · depuis 1\u00a0h\u00a012").assertExists()
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
    fun guestBar_keyboardAndMouseOnly_hasNoControllerButIsAPlayer() {
        val kbm = ui(guest = true, role = ShareRole.KBM) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(kbm) }
        // « Joueur » dès qu'il a un droit ; le toucher, la souris, le clavier et les touches avec le clavier et la souris...
        rule.onNodeWithText("Joueur").assertExists()
        rule.onNodeWithText("Souris", substring = true).assertExists()
        rule.onNodeWithText("Pavé tactile").assertExists()
        rule.onNodeWithText("Touches").assertExists()
        // ... mais pas la manette à l'écran, sans les manettes.
        rule.onNodeWithText("Manette à l'écran").assertDoesNotExist()
        rule.onNodeWithText("Spectateur").assertDoesNotExist()
    }

    @Test
    fun guestBar_playerWithKeyboardAndMouse_hasEverything() {
        val both = ui(guest = true, role = ShareRole.PLAYER_KBM) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(both) }
        rule.onNodeWithText("Joueur").assertExists()
        rule.onNodeWithText("Manette à l'écran").assertExists()
        rule.onNodeWithText("Pavé tactile").assertExists()
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
        assertTrue(ui(guest = true, role = ShareRole.KBM).keyboardMouseAllowed())
        // Le rôle qui change en cours de partage : l'interface le suit, chaque droit à part.
        val g = ui(guest = true, role = ShareRole.PLAYER_KBM)
        g.state.onShareRole(ShareRole.PLAYER.code, ShareRole.PLAYER_KBM.code)
        assertFalse(g.keyboardMouseAllowed())
        g.state.onShareRole(ShareRole.PLAYER_KBM.code, ShareRole.PLAYER.code)
        assertTrue(g.keyboardMouseAllowed())
        g.state.onShareRole(ShareRole.KBM.code, ShareRole.PLAYER_KBM.code)
        assertTrue(g.keyboardMouseAllowed())
    }

    @Test
    fun rightsNoticeCapsule_saysWhichRightChanged() {
        val ui = ui(guest = true, role = ShareRole.PLAYER_KBM)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        ui.state.capsule.value = Capsule.ShareRights(R.string.share_notice_pads_back)
        rule.onNodeWithText("Les manettes sont reprises.").assertExists()
        ui.state.capsule.value = Capsule.ShareRights(R.string.share_notice_play)
        rule.onNodeWithText("Vous pouvez jouer à la manette.").assertExists()
    }

    // MARK: - Qualité de l'image

    /** « 1080p · 60 Hz » tel que le cœur l'écrit, avec l'espace insécable de l'app avant « Hz ». */
    private fun hz(label: String) = label.replace(" Hz", "\u00a0Hz")

    /** L'image reçue d'un invité : [label] n'existe que si la Station dit la taille. */
    private fun received(label: String? = "1080p · 60 Hz", followsHost: Boolean? = false, quality: ShareQuality = ShareQuality.AUTO) =
        ShareImage(quality, followsHost = followsHost, reason = followsHost?.let { "asked" }, label = label?.let(::hz))

    @Test
    fun guestBar_hasQualityOfTheImage_whateverTheRole_andShowsTheReceivedQuality() {
        val ui = ui(guest = true, role = ShareRole.SPECTATOR) { barOpen = true; state.shareImage.value = received() }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        // Ce qui ne touche que l'appareil : un spectateur l'a aussi ; le bouton montre la qualité reçue.
        rule.onNodeWithText("Qualité de l'image").assertHasClickAction()
        rule.onNodeWithText(hz("1080p · 60 Hz")).assertExists()
        rule.onNodeWithText("Spectateur").assertExists()
    }

    @Test
    fun hostBar_hasNoQualityOfTheImage() {
        val ui = ui { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Qualité de l'image").assertDoesNotExist()
    }

    @Test
    fun theQualitySheet_opensFromTheBar_andChoosingTellsTheSessionOnce() {
        val ui = ui(guest = true, role = ShareRole.PLAYER) { barOpen = true; state.shareImage.value = received() }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Comme l'hôte").assertDoesNotExist()
        rule.onNodeWithText("Qualité de l'image").performClick()
        assertTrue(ui.qualityOpen)
        // Les cinq choix, dans l'ordre de link-ui ; « Auto » allumé (la qualité choisie au départ).
        rule.onNodeWithText("Auto").assertIsSelected()
        rule.onNodeWithText("Comme l'hôte").assertIsNotSelected()
        rule.onNodeWithText(hz("4K · 60 Hz")).assertIsNotSelected()
        rule.onNodeWithText(hz("720p · 30 Hz")).assertExists()
        // La qualité reçue est dite en tête de la feuille, en plus du choix de même nom : trois fois à l'écran avec la barre.
        rule.onAllNodesWithText(hz("1080p · 60 Hz")).assertCountEquals(3)
        rule.onNodeWithText("Comme l'hôte").performClick()
        assertEquals(listOf(ShareQuality.HOST), actions.qualities)
        assertFalse(ui.qualityOpen)
    }

    @Test
    fun theQualitySheet_offers4K_andChoosingItAsksFor2160p60() {
        val ui = ui(guest = true, role = ShareRole.PLAYER) { qualityOpen = true; state.shareImage.value = received(label = "4K · 60 Hz", quality = ShareQuality.P2160_60) }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        // La qualité reçue en tête (« 4K · 60 Hz ») et le choix de même nom : deux fois.
        rule.onAllNodesWithText(hz("4K · 60 Hz")).assertCountEquals(2)
        rule.onAllNodesWithText(hz("4K · 60 Hz")).onLast().performClick()
        assertEquals(listOf(ShareQuality.P2160_60), actions.qualities)
        assertFalse(ui.qualityOpen)
    }

    @Test
    fun theQualitySheet_marksTheChosenQuality() {
        val ui = ui(guest = true, role = ShareRole.PLAYER) { qualityOpen = true; state.shareQuality.value = ShareQuality.P720_30 }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText(hz("720p · 30 Hz")).assertIsSelected()
        rule.onNodeWithText("Auto").assertIsNotSelected()
        rule.onNodeWithText(hz("1080p · 60 Hz")).assertIsNotSelected()
        rule.onNodeWithText("Fermer").performClick()
        assertFalse(ui.qualityOpen)
        assertTrue(actions.qualities.isEmpty())
    }

    @Test
    fun anUnknownImage_showsNoReceivedQuality() {
        // Kyber ne dit rien de la taille : le cœur n'écrit aucun libellé, l'interface ne montre aucune qualité reçue.
        val ui = ui(guest = true, role = ShareRole.PLAYER) { barOpen = true; state.shareImage.value = received(label = null, followsHost = null) }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Qualité de l'image").assertHasClickAction()
        rule.onNodeWithText("1080p", substring = true).assertDoesNotExist()
        rule.onNodeWithText("720p", substring = true).assertDoesNotExist()
        rule.onNodeWithText("Qualité de l'image").performClick()
        // Seul le choix « 1080p · 60 Hz » : pas de ligne de qualité reçue.
        rule.onAllNodesWithText(hz("1080p · 60 Hz")).assertCountEquals(1)
    }

    @Test
    fun tvMenu_hasTheQualityTile_showingTheReceivedQuality_andOpensTheSheet() {
        val ui = ui(tv = true, guest = true, role = ShareRole.PLAYER) { menuOpen = true; state.shareImage.value = received("720p · 30 Hz") }
        show(FormFactor.TV) { SessionOverlay(ui) }
        rule.onNodeWithText("Qualité de l'image").assertHasClickAction()
        rule.onNodeWithText(hz("720p · 30 Hz")).assertExists()
        rule.onNodeWithText("Qualité de l'image").performClick()
        assertTrue(ui.qualityOpen)
        rule.onNodeWithText("Comme l'hôte").performClick()
        assertEquals(listOf(ShareQuality.HOST), actions.qualities)
    }

    @Test
    fun tvMenu_withoutAKnownImage_showsTheChosenQuality() {
        val ui = ui(tv = true, guest = true, role = ShareRole.SPECTATOR) { menuOpen = true; state.shareQuality.value = ShareQuality.HOST }
        show(FormFactor.TV) { SessionOverlay(ui) }
        // Pas de qualité reçue : la tuile dit celle qu'on a choisie.
        rule.onNodeWithText("Qualité de l'image").assertExists()
        rule.onNodeWithText("Comme l'hôte").assertExists()
    }

    @Test
    fun theStationsNotice_isShownLikeARightsNotice() {
        val ui = ui(guest = true, role = ShareRole.PLAYER)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        ui.state.capsule.value = Capsule.ShareNotice("Image allégée pour suivre.")
        rule.onNodeWithText("Image allégée pour suivre.").assertExists()
        ui.state.capsule.value = Capsule.ShareNotice("L'ordinateur ne peut pas faire une image de plus : vous recevez 720p · 30 Hz.")
        rule.onNodeWithText("L'ordinateur ne peut pas faire une image de plus : vous recevez 720p · 30 Hz.").assertExists()
    }

    @Test
    fun theHostPanel_showsTheQualityUnderAGuest_onlyWhenItIsNotTheHosts() {
        val now = System.currentTimeMillis() / 1000
        val state = share(
            // Suit l'image de l'hôte : rien.
            ShareGuest("g1", "Léa", "ios", ShareRole.SPECTATOR, image = received("1080p · 60 Hz", followsHost = true, quality = ShareQuality.HOST)),
            // Image à part : « app · qualité · depuis ».
            ShareGuest("g2", "Max", "web", ShareRole.PLAYER, since = now - 200, image = received("720p · 30 Hz", followsHost = false, quality = ShareQuality.P720_30)),
            // Image encore inconnue : comme l'hôte, rien.
            ShareGuest("g3", "Zoé", "android", ShareRole.SPECTATOR, image = received(null, followsHost = null)),
            // Image à part dont la taille n'est pas encore sue : pas de libellé, rien.
            ShareGuest("g4", "Eva", "mac", ShareRole.SPECTATOR, image = received(null, followsHost = false)),
        )
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        rule.onNodeWithText("iPhone ou iPad").assertExists()
        rule.onNodeWithText(hz("Navigateur · 720p · 30 Hz · depuis 3\u00a0min")).assertExists()
        rule.onNodeWithText("Android").assertExists()
        rule.onNodeWithText("Mac").assertExists()
        // Lecture seule : aucune action ne parle de la qualité.
        rule.onNodeWithText("Qualité de l'image").assertDoesNotExist()
    }

    // MARK: - « Rejoindre »

    private val link = ShareLink(ShareTarget("192.168.1.20", 41234, "sha256:ab"), "428913")

    @Test
    fun join_offersTheFiveQualities_autoProposed_andSendsTheChoice() {
        val joins = mutableListOf<Triple<String, String, ShareQuality>>()
        show(FormFactor.PHONE) { JoinContent(link, false, "Pixel de Léa", ShareQuality.AUTO, false, null, {}, { code, name, quality, _ -> joins += Triple(code, name, quality) }) }
        // Sous le prénom : « Qualité de l'image » (en capitales dans la charte) et les cinq choix, « Auto » proposé.
        rule.onNodeWithText("Qualité de l'image", ignoreCase = true).performScrollTo().assertExists()
        rule.onNodeWithText("Auto").assertIsSelected()
        rule.onNodeWithText("Comme l'hôte").assertIsNotSelected()
        rule.onNodeWithText(hz("4K · 60 Hz")).assertIsNotSelected()
        rule.onNodeWithText(hz("1080p · 60 Hz")).assertExists()
        rule.onNodeWithText(hz("720p · 30 Hz")).performScrollTo().performClick()
        rule.onNodeWithText(hz("720p · 30 Hz")).assertIsSelected()
        rule.onNodeWithText("Auto").assertIsNotSelected()
        rule.onNodeWithText("Rejoindre").performScrollTo().performClick()
        // Rien ne part sans « Rejoindre » ; puis le code, le prénom et la qualité choisie.
        assertEquals(listOf(Triple("428913", "Pixel de Léa", ShareQuality.P720_30)), joins)
    }

    @Test
    fun join_sendsTheChoiceOf4K() {
        val joins = mutableListOf<Triple<String, String, ShareQuality>>()
        show(FormFactor.PHONE) { JoinContent(link, false, "Pixel de Léa", ShareQuality.AUTO, false, null, {}, { code, name, quality, _ -> joins += Triple(code, name, quality) }) }
        rule.onNodeWithText(hz("4K · 60 Hz")).performScrollTo().performClick()
        rule.onNodeWithText(hz("4K · 60 Hz")).assertIsSelected()
        rule.onNodeWithText("Auto").assertIsNotSelected()
        rule.onNodeWithText("Rejoindre").performScrollTo().performClick()
        assertEquals(listOf(Triple("428913", "Pixel de Léa", ShareQuality.P2160_60)), joins)
    }

    @Test
    fun join_startsFromTheRememberedQuality() {
        show(FormFactor.PHONE) { JoinContent(link, false, "Léa", ShareQuality.P1080_60, false, null, {}, { _, _, _, _ -> }) }
        rule.onNodeWithText(hz("1080p · 60 Hz")).performScrollTo().assertIsSelected()
        rule.onNodeWithText("Auto").assertIsNotSelected()
    }

    @Test
    fun join_onATelevision_hasTheQualityToo() {
        show(FormFactor.TV) { JoinContent(null, false, "SHIELD", ShareQuality.HOST, false, null, {}, { _, _, _, _ -> }) }
        rule.onNodeWithText("Qualité de l'image", ignoreCase = true).assertExists()
        rule.onNodeWithText("Comme l'hôte").assertIsSelected()
        rule.onNodeWithText("Auto").assertExists()
        rule.onNodeWithText(hz("4K · 60 Hz")).assertExists()
        rule.onNodeWithText(hz("720p · 30 Hz")).assertExists()
    }
}
