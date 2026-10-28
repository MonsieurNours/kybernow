package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.HostInfo
import fr.leslignesclaires.kybernow.core.RouteInfo
import fr.leslignesclaires.kybernow.core.RouteTransport
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareRelay
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.core.ShareVia
import fr.leslignesclaires.kybernow.core.StationInfo
import fr.leslignesclaires.kybernow.core.StationUpdate
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.LastSession
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.SharePanelContent
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.RelayRefusal
import fr.leslignesclaires.kybernow.ui.home.relayRefusalText
import fr.leslignesclaires.kybernow.ui.home.statusLabel
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsActions
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsContent
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsState
import fr.leslignesclaires.kybernow.ui.settings.LastSessionCard
import fr.leslignesclaires.kybernow.ui.settings.Rubric
import fr.leslignesclaires.kybernow.ui.share.JoinContent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Le relais, l'interface (docs/relais.md ; textes de docs/link-ui.md, « Partage » et « Hors de chez vous »), en français :
 * les demandes de l'hôte avec « Accepter » et « Refuser », l'aide selon le relais, « hors de chez vous » sur la ligne
 * d'un invité, les pastilles, la puce « Par le relais » et la ligne « Chemin », l'attente de l'accord de l'hôte, l'état
 * « Hors de chez vous » de l'accueil et ses refus.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class RelayUiTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()
    private val nbsp = " "

    private class Actions : SessionActions {
        val answers = mutableListOf<String>()
        var quits = 0
        var shares = 0
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
        override fun shareSetPads(guest: ShareGuest, on: Boolean) {}
        override fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean) {}
        override fun shareAccept(guest: ShareGuest) { answers += "${guest.name} accepté" }
        override fun shareRefuse(guest: ShareGuest) { answers += "${guest.name} refusé" }
        override fun shareRemove(guest: ShareGuest) {}
        override fun shareStop() {}
        override fun shareCopyLink() {}
        override fun setShareQuality(quality: ShareQuality) {}
    }

    private val actions = Actions()

    private fun ui(tv: Boolean = false, guest: Boolean = false, route: RouteInfo? = null, setup: SessionUi.() -> Unit = {}): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        state.route.value = route
        return SessionUi(if (guest) "Invité sur Atelier" else "Atelier", tv, state, actions, guest = guest, computerName = "Atelier").apply { passageDone = true; setup() }
    }

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    private fun share(
        guests: List<ShareGuest> = emptyList(), requests: List<ShareGuest> = emptyList(), relay: ShareRelay? = ShareRelay.JOINED, web: String = "https://share.kybernow.app/j/428913",
    ) = ShareState("428913", "428 913", "kybernow://join?host=192.168.1.20&port=41234&fp=sha256:ab&code=428913&relay=share.kybernow.app&sid=abc", web, guests, relay, requests)

    private fun request(id: String, name: String, app: String = "ios") = ShareGuest(id, name, app, ShareRole.SPECTATOR, since = 100, pending = true, via = ShareVia.RELAY)

    private val relayRoute = RouteInfo(RouteTransport.RELAY, "share.kybernow.app", 12.0, 3.5)

    // MARK: - Le panneau de l'hôte

    @Test
    fun theRequests_comeAboveTheGuests_withAcceptAndDecline() {
        val state = share(guests = listOf(ShareGuest("g2", "Tom", "mac", ShareRole.SPECTATOR)), requests = listOf(request("g1", "Léa")))
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        rule.onNodeWithText("Demandes", ignoreCase = true).assertExists()
        rule.onNodeWithText("Léa demande à rejoindre").assertExists()
        // « {app} · hors de chez vous » sous la demande.
        rule.onNodeWithText("iPhone ou iPad · hors de chez vous").assertExists()
        rule.onNodeWithText("Accepter").assertHasClickAction().assertIsEnabled().performClick()
        rule.onNodeWithText("Refuser").assertHasClickAction().performClick()
        assertEquals(listOf("Léa accepté", "Léa refusé"), actions.answers)
        // Une demande n'a ni manettes ni « Retirer » : ce sont ceux de Tom seul, un seul jeu de boutons.
        rule.onNodeWithText("Retirer").assertExists()
        rule.onNodeWithText("Léa · Spectateur").assertDoesNotExist()
    }

    @Test
    fun withoutRequests_thereIsNoRequestsSection() {
        show(FormFactor.PHONE) { SharePanelContent("Atelier", share(guests = listOf(ShareGuest("g2", "Tom", "mac", ShareRole.SPECTATOR))), false, null, null, false, actions, {}) }
        rule.onNodeWithText("Demandes", ignoreCase = true).assertDoesNotExist()
        rule.onNodeWithText("Accepter").assertDoesNotExist()
        rule.onNodeWithText("Refuser").assertDoesNotExist()
    }

    @Test
    fun whenTheFourPlacesAreTaken_acceptIsOff_declineStays() {
        val guests = (1..4).map { ShareGuest("g$it", "Invité$it", "mac", ShareRole.SPECTATOR) }
        show(FormFactor.PHONE) { SharePanelContent("Atelier", share(guests = guests, requests = listOf(request("p1", "Léa"))), false, null, null, false, actions, {}) }
        rule.onNodeWithText("Quatre invités au plus.").assertExists()
        rule.onNodeWithText("Accepter").assertIsNotEnabled()
        rule.onNodeWithText("Refuser").assertIsEnabled().performClick()
        assertEquals(listOf("Léa refusé"), actions.answers)
    }

    @Test
    fun whileACallRuns_theRequestButtonsWait() {
        show(FormFactor.PHONE) { SharePanelContent("Atelier", share(requests = listOf(request("p1", "Léa"))), true, null, null, false, actions, {}) }
        rule.onNodeWithText("Accepter").assertIsNotEnabled()
        rule.onNodeWithText("Refuser").assertIsNotEnabled()
    }

    @Test
    fun onATelevision_theRequestCanBeAnsweredToo() {
        show(FormFactor.TV) { SharePanelContent("Atelier", share(requests = listOf(request("p1", "Léa"))), false, null, null, true, actions, {}) }
        rule.onNodeWithText("Accepter").assertHasClickAction().performClick()
        assertEquals(listOf("Léa accepté"), actions.answers)
    }

    @Test
    fun theHelp_followsTheRelay() {
        // Relais joint : le lien vaut aussi ailleurs, et chaque invité venu d'ailleurs s'accepte.
        show(FormFactor.PHONE) { SharePanelContent("Atelier", share(relay = ShareRelay.JOINED), false, null, null, false, actions, {}) }
        rule.onNodeWithText("Sur le même réseau ou ailleurs", substring = true).assertExists()
        rule.onNodeWithText("Hors de chez vous, vous acceptez chaque invité.", substring = true).assertExists()
        rule.onNodeWithText("Pas encore joignable hors de chez vous.").assertDoesNotExist()
    }

    @Test
    fun theHelp_whenTheRelayIsUnreachable_saysSoUnderTheNetworkHelp() {
        show(FormFactor.PHONE) { SharePanelContent("Atelier", share(relay = ShareRelay.UNREACHABLE, web = "https://192.168.1.20:41234/link/?join=428913"), false, null, null, false, actions, {}) }
        rule.onNodeWithText("Sur le même réseau$nbsp:", substring = true).assertExists()
        rule.onNodeWithText("Sur le même réseau ou ailleurs", substring = true).assertDoesNotExist()
        rule.onNodeWithText("Pas encore joignable hors de chez vous.").assertExists()
    }

    @Test
    fun theHelp_withoutARelay_isTheNetworkOne() {
        show(FormFactor.PHONE) { SharePanelContent("Atelier", share(relay = null), false, null, null, false, actions, {}) }
        rule.onNodeWithText("Sur le même réseau$nbsp:", substring = true).assertExists()
        rule.onNodeWithText("Pas encore joignable hors de chez vous.").assertDoesNotExist()
        rule.onNodeWithText("Hors de chez vous, vous acceptez", substring = true).assertDoesNotExist()
    }

    @Test
    fun aGuestWhoCameByTheRelay_saysAwayFromHomeAfterItsApp() {
        val now = System.currentTimeMillis() / 1000
        val state = share(
            guests = listOf(
                ShareGuest("g1", "Léa", "ios", ShareRole.SPECTATOR, since = now - 200, via = ShareVia.RELAY),
                ShareGuest("g2", "Tom", "mac", ShareRole.PLAYER, since = now - 200, via = ShareVia.LAN),
            ),
        )
        show(FormFactor.PHONE) { SharePanelContent("Atelier", state, false, null, null, false, actions, {}) }
        // « iPhone ou iPad · hors de chez vous · depuis 3 min » ; un invité du réseau n'a pas la mention.
        rule.onNodeWithText("iPhone ou iPad · hors de chez vous · depuis 3 min").assertExists()
        rule.onNodeWithText("Mac · depuis 3 min").assertExists()
    }

    // MARK: - La barre et le menu

    @Test
    fun theBar_hasAPastilleForTheRequests_nextToTheGuests() {
        val ui = ui { barOpen = true }
        ui.state.share.value = share(guests = listOf(ShareGuest("g2", "Tom", "mac", ShareRole.SPECTATOR)), requests = listOf(request("p1", "Léa"), request("p2", "Max")))
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        // Les demandes ne comptent pas dans les invités.
        rule.onNodeWithText("1 invité").assertExists()
        rule.onNodeWithText("2 demandes").assertHasClickAction().performClick()
        assertEquals(1, actions.shares)
    }

    @Test
    fun theRequestsPastille_isSingularForOne_andAbsentWithoutRequests() {
        val one = ui { barOpen = true }
        one.state.share.value = share(requests = listOf(request("p1", "Léa")))
        show(FormFactor.PHONE) { SessionOverlay(one) }
        rule.onNodeWithText("1 demande").assertExists()
        // Pas d'invité : jamais « 0 invité ».
        rule.onNodeWithText("0 invité").assertDoesNotExist()
    }

    @Test
    fun noRequests_noPastilleForThem() {
        val none = ui { barOpen = true }
        none.state.share.value = share(guests = listOf(ShareGuest("g2", "Tom", "mac", ShareRole.SPECTATOR)))
        show(FormFactor.PHONE) { SessionOverlay(none) }
        rule.onNodeWithText("1 invité").assertExists()
        rule.onNodeWithText("demande", substring = true).assertDoesNotExist()
    }

    @Test
    fun theTelevisionTile_showsTheRequestsFirst_thenTheGuests() {
        val ui = ui(tv = true) { menuOpen = true }
        ui.state.share.value = share(guests = listOf(ShareGuest("g2", "Tom", "mac", ShareRole.SPECTATOR), ShareGuest("g3", "Eva", "ios", ShareRole.SPECTATOR)), requests = listOf(request("p1", "Léa")))
        show(FormFactor.TV) { SessionOverlay(ui) }
        // Sans pastille dans un menu à tuiles, les demandes se lisent sur la tuile « Partager », avant les invités.
        rule.onNodeWithText("1 demande · 2 invités").assertExists()
    }

    @Test
    fun theTelevisionTile_saysOnlyWhatThereIs() {
        val onlyRequests = ui(tv = true) { menuOpen = true }
        onlyRequests.state.share.value = share(requests = listOf(request("p1", "Léa"), request("p2", "Max")))
        show(FormFactor.TV) { SessionOverlay(onlyRequests) }
        rule.onNodeWithText("2 demandes").assertExists()
        rule.onNodeWithText("invité", substring = true).assertDoesNotExist()
    }

    @Test
    fun theTelevisionTile_withoutRequests_isTheGuestsThenTheCode() {
        val guests = ui(tv = true) { menuOpen = true }
        guests.state.share.value = share(guests = listOf(ShareGuest("g2", "Tom", "mac", ShareRole.SPECTATOR)))
        show(FormFactor.TV) { SessionOverlay(guests) }
        rule.onNodeWithText("1 invité").assertExists()
        rule.onNodeWithText("demande", substring = true).assertDoesNotExist()
    }

    @Test
    fun aRequest_isAnnouncedInACapsule_andChoosingItOpensThePanel() {
        val ui = ui()
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        ui.state.capsule.value = fr.leslignesclaires.kybernow.session.Capsule.ShareRequest("Léa")
        // L'avis bref de link-ui : « {prénom} demande à rejoindre. » (avec son point, à la différence de la ligne du panneau).
        rule.onNodeWithText("Léa demande à rejoindre.").assertHasClickAction().performClick()
        assertEquals(1, actions.shares)
    }

    // MARK: - « Par le relais »

    @Test
    fun theBar_showsTheRelayChip_onlyByTheRelay() {
        val relayed = ui(route = relayRoute) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(relayed) }
        // Dans la partie fixe de la barre ; un toucher ouvre les statistiques.
        rule.onNodeWithText("Par le relais").assertHasClickAction().performClick()
        assertTrue(relayed.statsOpen)
    }

    @Test
    fun theBar_hasNoRelayChip_onTheNetwork() {
        val direct = ui(route = RouteInfo(RouteTransport.DIRECT)) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(direct) }
        rule.onNodeWithText("Par le relais").assertDoesNotExist()
    }

    @Test
    fun theBar_hasNoRelayChip_beforeTheCoreSaysWhichPath() {
        show(FormFactor.PHONE) { SessionOverlay(ui(route = null) { barOpen = true }) }
        rule.onNodeWithText("Par le relais").assertDoesNotExist()
    }

    @Test
    fun aGuestBar_hasTheRelayChipToo() {
        show(FormFactor.PHONE) { SessionOverlay(ui(guest = true, route = relayRoute) { barOpen = true }) }
        rule.onNodeWithText("Par le relais").assertExists()
        rule.onNodeWithText("Quitter le partage").assertExists()
    }

    @Test
    fun theTelevisionMenu_hasARelayTile() {
        val ui = ui(tv = true, route = relayRoute) { menuOpen = true }
        show(FormFactor.TV) { SessionOverlay(ui) }
        rule.onNodeWithText("Par le relais").assertHasClickAction().performClick()
        assertTrue(ui.statsOpen)
    }

    @Test
    fun theGuestTelevisionMenu_hasARelayTile() {
        show(FormFactor.TV) { SessionOverlay(ui(tv = true, guest = true, route = relayRoute) { menuOpen = true }) }
        rule.onNodeWithText("Par le relais").assertHasClickAction()
        rule.onNodeWithText("Quitter le partage").assertExists()
    }

    @Test
    fun theTelevisionMenu_hasNoRelayTile_onTheNetwork() {
        show(FormFactor.TV) { SessionOverlay(ui(tv = true, route = RouteInfo(RouteTransport.DIRECT)) { menuOpen = true }) }
        rule.onNodeWithText("Par le relais").assertDoesNotExist()
    }

    @Test
    fun theStatistics_haveAPathLine_direct() {
        show(FormFactor.PHONE) { SessionOverlay(ui(route = RouteInfo(RouteTransport.DIRECT)) { statsOpen = true }) }
        rule.onNodeWithText("Chemin$nbsp: Direct").assertExists()
    }

    @Test
    fun theStatistics_haveAPathLine_byTheRelay() {
        show(FormFactor.PHONE) { SessionOverlay(ui(route = relayRoute) { statsOpen = true }) }
        rule.onNodeWithText("Chemin$nbsp: Par le relais").assertExists()
    }

    @Test
    fun theStatisticsPathLine_saysViaTheRelayTcp() {
        val ui = ui(route = RouteInfo(RouteTransport.RELAY_TCP)) { statsOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Chemin$nbsp: Par le relais (TCP)").assertExists()
    }

    @Test
    fun theStatistics_haveNoPathLine_beforeTheCoreSaysWhichPath() {
        show(FormFactor.PHONE) { SessionOverlay(ui(route = null) { statsOpen = true }) }
        rule.onNodeWithText("Chemin", substring = true).assertDoesNotExist()
    }

    @Test
    fun theLastSession_hasAPathLine() {
        val last = LastSession(8.9, 14.2, 2520, 1_761_000_000_000, 2560, 1440, 120, "hevc", 80, route = "relay")
        show(FormFactor.PHONE) { LastSessionCard(last, 1_761_000_000_000) }
        rule.onNodeWithText("Chemin$nbsp: Par le relais").assertExists()
    }

    @Test
    fun theLastSession_beforeTheRelay_hasNoPathLine() {
        val last = LastSession(8.9, 14.2, 2520, 1_761_000_000_000, 2560, 1440, 120, "hevc", 80)
        show(FormFactor.PHONE) { LastSessionCard(last, 1_761_000_000_000) }
        rule.onNodeWithText("Chemin", substring = true).assertDoesNotExist()
    }

    // MARK: - Rejoindre, hors de chez soi

    @Test
    fun whileTheHostDecides_theScreenWaits_withCancel() {
        var cancelled = 0
        var joined = 0
        show(FormFactor.PHONE) { JoinContent(null, false, "Léa", ShareQuality.AUTO, true, null, {}, { _, _, _, _ -> joined++ }, waiting = true, onCancel = { cancelled++ }) }
        // Le titre ne change pas : le prénom de l'hôte et l'ordinateur ne sont connus qu'après l'accord.
        rule.onNodeWithText("Rejoindre un partage").assertExists()
        rule.onNodeWithText("En attente de l'accord de l'hôte…").assertExists()
        rule.onNodeWithText("Annuler").assertHasClickAction().performClick()
        assertEquals(1, cancelled)
        assertEquals(0, joined)
        // Plus de code, de prénom ni de qualité : on attend.
        rule.onNodeWithText("Votre prénom", substring = true).assertDoesNotExist()
        rule.onNodeWithText("Rejoindre", substring = false).assertDoesNotExist()
    }

    @Test
    fun theTelevision_waitsToo() {
        show(FormFactor.TV) { JoinContent(null, false, "SHIELD", ShareQuality.AUTO, true, null, {}, { _, _, _, _ -> }, waiting = true) }
        rule.onNodeWithText("En attente de l'accord de l'hôte…").assertExists()
        rule.onNodeWithText("Annuler").assertHasClickAction()
    }

    @Test
    fun aRefusal_isShownAsTheCoreWritesIt() {
        val text = "L'hôte a refusé."
        show(FormFactor.PHONE) { JoinContent(null, false, "Léa", ShareQuality.AUTO, false, text, {}, { _, _, _, _ -> }) }
        rule.onNodeWithText(text).assertExists()
        // Elle se lit sur l'écran du code : on peut corriger le code et redemander.
        rule.onNodeWithText("Rejoindre").assertExists()
    }

    // MARK: - L'accueil

    @Test
    fun theHomeStatus_saysAwayFromHome() {
        show(FormFactor.PHONE) { Text(statusLabel(HostStatus(online = true, away = true))) }
        rule.onNodeWithText("Hors de chez vous").assertExists()
    }

    @Test
    fun theHomeStatus_keepsItsOtherWords() {
        show(FormFactor.PHONE) {
            androidx.compose.foundation.layout.Column {
                Text(statusLabel(HostStatus(online = true)))
                Text(statusLabel(HostStatus(online = false)))
                Text(statusLabel(HostStatus()))
                Text(statusLabel(null))
            }
        }
        rule.onNodeWithText("En ligne").assertExists()
        rule.onNodeWithText("Hors ligne").assertExists()
        rule.onAllNodesWithText("Recherche…").assertCountEquals(2)
        rule.onNodeWithText("Hors de chez vous").assertDoesNotExist()
    }

    @Test
    fun theRefusals_useTheLinkUiTexts_withTheComputerName() {
        show(FormFactor.PHONE) {
            androidx.compose.foundation.layout.Column {
                Text(relayRefusalText(RelayRefusal.StationOffline, "Atelier"))
                Text(relayRefusalText(RelayRefusal.AccessOff, "Atelier"))
                Text(relayRefusalText(RelayRefusal.Message("Le relais ne répond pas."), "Atelier"))
                Text(relayRefusalText(RelayRefusal.Message(""), "Atelier"))
            }
        }
        rule.onNodeWithText("Atelier n'est pas joignable$nbsp: l'ordinateur est éteint ou en veille.").assertExists()
        rule.onNodeWithText("Atelier n'est pas joignable hors de chez vous (accès coupé dans Admin).").assertExists()
        rule.onNodeWithText("Le relais ne répond pas.").assertExists()
        rule.onNodeWithText("Admin ne répond pas à cette adresse.").assertExists()
    }

    // MARK: - Par le relais, pas d'action lourde

    private fun station(status: HostStatus) = show(FormFactor.PHONE) {
        val host = Host(id = "a", name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab", user = "matthieu")
        val screen = Screen(ScreenMode(1, 1080, 2400, 120), listOf(ScreenMode(1, 1080, 2400, 120)), false, true)
        val decoders = Decoders(DecoderInfo("c2.qti.avc.decoder", 8192, 4320, true), null, null, false)
        val actions = HostSettingsActions({}, { _, _ -> }, {}, {}, { _, _ -> }, {}, {}, {}, {})
        HostSettingsContent(HostSettingsState(host, status, screen, decoders, "https://192.168.1.20:41234/"), actions, Rubric.STATION, only = Rubric.STATION)
    }

    @Test
    fun byTheRelay_theStationCannotBeRestartedNorUpdated_andTheReasonIsSaid() {
        station(HostStatus(online = true, away = true))
        rule.onNodeWithText("Hors de chez vous, l'app ne peut ni redémarrer ni mettre à jour la Station.").assertExists()
        rule.onNodeWithText("Redémarrer la Station").assertDoesNotExist()
        rule.onNodeWithText("Mettre à jour la Station").assertDoesNotExist()
    }

    @Test
    fun onTheNetwork_theStationCanBeRestarted_withoutTheRelayReason() {
        station(HostStatus(online = true, info = HostInfo("ATELIER", "Windows 11 Pro", "RTX", false, "0.8.0", true, true, StationInfo("0.8.0", StationUpdate(), false))))
        rule.onNodeWithText("Redémarrer la Station").assertExists()
        rule.onNodeWithText("Hors de chez vous, l'app ne peut ni", substring = true).assertDoesNotExist()
    }
}
