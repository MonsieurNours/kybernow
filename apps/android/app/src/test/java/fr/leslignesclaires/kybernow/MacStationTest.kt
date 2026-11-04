package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.PadMouse
import fr.leslignesclaires.kybernow.session.PadsNotice
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.Sc
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.session.stickyKeys
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsActions
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsContent
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsState
import fr.leslignesclaires.kybernow.ui.settings.Rubric
import org.json.JSONObject
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Face à une Station Mac (docs/link-ui.md, « Face à une Station Mac » et « Manettes indisponibles »), en français : le
 * menu Touches et la rangée du clavier de l'écran du Mac, « Ctrl agit comme ⌘ » gardé pour chaque ordinateur, l'avis
 * et la ligne de l'accueil des manettes indisponibles.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class MacStationTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()

    private class Actions : SessionActions {
        val sent = mutableListOf<Shortcut>()
        override fun resume() {}
        override fun quit() {}
        override fun setProfile(p: Profile) {}
        override fun setMouseMode(m: Int) {}
        override fun setDirectTouch(on: Boolean) {}
        override fun setRemoteMouse(on: Boolean) {}
        override fun setPadMouse(on: Boolean) {}
        override fun shortcut(s: Shortcut) { sent += s }
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

    private val actions = Actions()

    private fun ui(mac: Boolean, tv: Boolean = false, setup: SessionUi.() -> Unit = {}): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        return SessionUi("Atelier", tv, state, actions, mac = mac).apply { passageDone = true; setup() }
    }

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    private val macHost = Host(id = "m", name = "Studio", address = "192.168.1.30", port = 41234, fingerprint = "sha256:ab", user = "m", os = "macOS 26.1")

    // MARK: - Les touches

    @Test
    fun theMacKeys_leaveAsTheyAre_withTheMacsCommandKey() {
        assertArrayEquals(intArrayOf(Sc.WIN, Sc.TAB), Shortcut.MAC_SWITCH_APP.keys)
        assertArrayEquals(intArrayOf(Sc.WIN, Sc.SPACE), Shortcut.MAC_SPOTLIGHT.keys)
        assertArrayEquals(intArrayOf(Sc.CTRL, Sc.WIN, Sc.Q), Shortcut.MAC_LOCK.keys)
        assertArrayEquals(intArrayOf(Sc.SHIFT, Sc.WIN, Sc.FIVE), Shortcut.MAC_SCREENSHOT.keys)
        assertArrayEquals(intArrayOf(Sc.ALT, Sc.WIN, Sc.ESC), Shortcut.MAC_FORCE_QUIT.keys)
        assertEquals(Shortcut.MAC, Shortcut.menu(true))
        assertEquals(Shortcut.PC, Shortcut.menu(false))
        assertEquals(Shortcut.MAC_FORCE_QUIT, Shortcut.rowKey(true))
        assertEquals(Shortcut.CTRL_ALT_DEL, Shortcut.rowKey(false))
        // Mêmes touches collantes, d'autres noms.
        assertEquals(listOf(Sc.CTRL to "⌃", Sc.ALT to "⌥", Sc.WIN to "⌘"), stickyKeys(true))
        assertEquals(listOf(Sc.CTRL to "Ctrl", Sc.ALT to "Alt", Sc.WIN to "⊞"), stickyKeys(false))
    }

    @Test
    fun theControllerAsMouse_holdsCommand_andStartOpensSpotlight_withAMac() {
        // Face à un Mac : L1 / R1 tiennent ⌘ (la touche ⊞ pour Kyber), Start envoie ⌘ Espace.
        assertEquals(Sc.WIN, PadMouse.switcherKey(true))
        assertArrayEquals(intArrayOf(Sc.WIN, Sc.SPACE), PadMouse.startKeys(true))
        // Face à un PC (et pour un invité, un ami qui emprunte) : Alt + Tab, ⊞ seule.
        assertEquals(Sc.ALT, PadMouse.switcherKey(false))
        assertArrayEquals(intArrayOf(Sc.WIN), PadMouse.startKeys(false))
    }

    @Test
    fun withAMac_theKeysMenu_offersTheMacsKeysInstead() {
        show(FormFactor.PHONE) { SessionOverlay(ui(mac = true) { keysOpen = true }) }
        for (label in listOf("⌘ ⇥", "⌘ Espace", "⌃ ⌘ Q", "⇧ ⌘ 5", "⌥ ⌘ Échap")) rule.onNodeWithText(label).assertExists()
        rule.onNodeWithText("Ctrl + Alt + Suppr").assertDoesNotExist()
        rule.onNodeWithText("Alt + Tab").assertDoesNotExist()
        rule.onNodeWithText("⌃ ⌘ Q").performClick()
        assertEquals(listOf(Shortcut.MAC_LOCK), actions.sent)
    }

    @Test
    fun withAPc_theKeysMenu_staysThePcs() {
        show(FormFactor.PHONE) { SessionOverlay(ui(mac = false) { keysOpen = true }) }
        rule.onNodeWithText("Ctrl + Alt + Suppr").assertExists()
        rule.onNodeWithText("Alt + Tab").assertExists()
        rule.onNodeWithText("⌘ ⇥").assertDoesNotExist()
    }

    @Test
    fun withAMac_theKeyRow_namesTheModifiersTheMacWay() {
        show(FormFactor.PHONE) { SessionOverlay(ui(mac = true) { keyboardOpen = true }) }
        for (label in listOf("⌃", "⌥", "⌘", "⌥ ⌘ Échap")) rule.onNodeWithText(label).assertExists()
        rule.onNodeWithText("Ctrl").assertDoesNotExist()
        rule.onNodeWithText("Ctrl+Alt+Suppr").assertDoesNotExist()
        rule.onNodeWithText("⌥ ⌘ Échap").performScrollTo().performClick()
        assertEquals(listOf(Shortcut.MAC_FORCE_QUIT), actions.sent)
    }

    @Test
    fun withAMac_theTvKeysTile_saysTheForceQuit() {
        show(FormFactor.TV) { SessionOverlay(ui(mac = true, tv = true) { menuOpen = true }) }
        rule.onNodeWithText("⌥ ⌘ Échap").assertExists()
        rule.onNodeWithText("Ctrl+Alt+Suppr").assertDoesNotExist()
    }

    // MARK: - « Ctrl agit comme ⌘ »

    @Test
    fun ctrlAsCommand_isKeptForEachComputer_nullUntilChanged() {
        assertNull(HostSettings().ctrlCommand)
        assertEquals(-1, HostSettings().ctrlCommandCode)
        assertNull(HostSettings.parse(JSONObject("""{"profile":"jeu"}""")).ctrlCommand)
        assertNull(HostSettings.parse(HostSettings().json()).ctrlCommand)
        assertEquals(false, HostSettings.parse(HostSettings(ctrlCommand = false).json()).ctrlCommand)
        assertEquals(true, HostSettings.parse(HostSettings(ctrlCommand = true).json()).ctrlCommand)
        assertEquals(0, HostSettings(ctrlCommand = false).ctrlCommandCode)
        assertEquals(1, HostSettings(ctrlCommand = true).ctrlCommandCode)
    }

    private fun controls(host: Host, mac: Boolean, update: ((HostSettings) -> HostSettings) -> Unit = {}) = show(FormFactor.PHONE) {
        val screen = Screen(ScreenMode(1, 1080, 2400, 120), listOf(ScreenMode(1, 1080, 2400, 120)), false, true)
        val decoders = Decoders(DecoderInfo("c2.qti.avc.decoder", 8192, 4320, true), null, null, false)
        val actions = HostSettingsActions(update, { _, _ -> }, {}, {}, { _, _ -> }, {}, {}, {}, {})
        HostSettingsContent(HostSettingsState(host, HostStatus(true), screen, decoders, "https://192.168.1.30:41234/", mac = mac), actions, Rubric.CONTROLS, only = Rubric.CONTROLS)
    }

    @Test
    fun withAMac_theControls_haveCtrlAsCommand_onByDefault() {
        var settings = macHost.settings
        controls(macHost, mac = true) { f -> settings = f(settings) }
        rule.onNodeWithText("Ctrl agit comme ⌘").assertExists()
        rule.onNodeWithText("Copier, coller et les autres raccourcis du Mac avec Ctrl, comme sur un PC. La touche ⊞ devient ⌃.").assertExists()
        // Allumé par défaut : le toucher l'éteint, et c'est gardé.
        rule.onNodeWithText("Ctrl agit comme ⌘").performScrollTo().performClick()
        assertEquals(false, settings.ctrlCommand)
    }

    @Test
    fun withAPc_theControls_haveNoCtrlAsCommand() {
        controls(macHost.copy(os = "Windows 11 Pro"), mac = false)
        rule.onNodeWithText("Ctrl agit comme ⌘").assertDoesNotExist()
    }

    // MARK: - Manettes indisponibles

    @Test
    fun theUnavailablePadsNotice_onceASession_atTheImageOrTheFirstPad() {
        // Une manette déjà branchée : à l'arrivée de l'image, une fois.
        PadsNotice(true).let { n ->
            assertFalse("pas avant l'image", n.due(false, 1))
            assertTrue(n.due(true, 1))
            assertFalse("une fois par session", n.due(true, 2))
        }
        // Sans manette à l'arrivée de l'image : quand la première se branche.
        PadsNotice(true).let { n ->
            assertFalse(n.due(true, 0))
            assertTrue(n.due(true, 1))
        }
        // Un ordinateur qui crée les manettes (ou un invité, un ami qui emprunte) : jamais.
        assertFalse(PadsNotice(false).due(true, 1))
    }

    @Test
    fun theUnavailablePadsNotice_saysItWithItsText() {
        val ui = ui(mac = true)
        ui.state.capsule.value = Capsule.PadsUnavailable
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Manettes indisponibles sur cet ordinateur.").assertExists()
    }

    @Test
    fun theComputersPads_areKeptWithIt_availableByDefault() {
        assertTrue(Host.parse(JSONObject("""{"id":"a","name":"Atelier"}""")).gamepads)
        assertFalse(Host.parse(macHost.copy(gamepads = false).json()).gamepads)
        assertTrue(Host.parse(macHost.json()).gamepads)
    }

    @Test
    @Config(qualifiers = "fr-w960dp-h540dp-land-television-xhdpi")
    fun onTheTvHome_padsUnavailable_replaceTheCount() {
        val noActions = HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {})
        val screen = Screen(ScreenMode(1, 3840, 2160, 60), listOf(ScreenMode(1, 3840, 2160, 60)), true, false)
        val decoders = Decoders(DecoderInfo("OMX.Nvidia.h264.decode", 4096, 2160, true), null, null, false)
        fun home(host: Host) = HomeState(listOf(host), host.id, mapOf(host.id to HostStatus(true)), emptyMap(), emptyList(), screen, decoders, pads = 2)
        show(FormFactor.TV) { HomeContent(home(macHost.copy(gamepads = false)), noActions) }
        rule.onNodeWithText("Manettes indisponibles sur cet ordinateur").assertExists()
        rule.onNodeWithText("2 manettes").assertDoesNotExist()
    }
}
