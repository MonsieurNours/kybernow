package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
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
import fr.leslignesclaires.kybernow.model.HdrState
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsActions
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsContent
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsState
import fr.leslignesclaires.kybernow.ui.settings.Rubric
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

import fr.leslignesclaires.kybernow.model.PyroWaveChoice
import fr.leslignesclaires.kybernow.model.PyroWaveLink
import fr.leslignesclaires.kybernow.core.RouteInfo
import fr.leslignesclaires.kybernow.session.Second
import fr.leslignesclaires.kybernow.session.Lowered

/**
 * PyroWave à l'écran (docs/link-ui.md, « PyroWave »), en français : la valeur de « Format vidéo », son aide, sa
 * raison quand elle est inactive, l'avertissement Wi-Fi ou relais, l'aide du texte net ; en session, la puce ambre,
 * le bandeau avec « Passer en HEVC » et « Images perdues ».
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class PyroWaveUiTest {
    @org.junit.Before fun fakeCore() = FakeCore.install()

    @org.junit.After fun realCore() = FakeCore.reset()

    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()
    private val n = "\u202f"
    private val host = Host(id = "a", name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab", user = "m", settings = HostSettings(profile = Profile.GAME), pyrowave = true)
    private val screen = Screen(ScreenMode(1, 1920, 1080, 60), listOf(ScreenMode(1, 1920, 1080, 60)), true, false)
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, true)

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    private fun fr(s: String) = s.replace(" :", "$n:").replace("« ", "«$n").replace(" »", "$n»")

    private fun settings(codec: String, pw: PyroWaveChoice, updates: MutableList<HostSettings> = mutableListOf(), profile: Profile = Profile.GAME) {
        var current = host.settings.copy(codec = codec, profile = profile)
        val h = host.copy(settings = current)
        val actions = HostSettingsActions({ f -> current = f(current); updates += current }, { _, _ -> }, {}, {}, { _, _ -> }, {}, {}, {}, {})
        show(FormFactor.PHONE) {
            HostSettingsContent(HostSettingsState(h, HostStatus(true), screen, decoders, "https://192.168.1.20:41234/", pyrowave = pw), actions, Rubric.PICTURE, only = Rubric.PICTURE)
        }
    }

    @Test
    fun format_video_pyrowave_choisi_en_wifi() {
        settings("pyrowave", PyroWaveChoice.of(host, decodes = true, wired = false, relayed = false))
        rule.onNodeWithText("PyroWave").assertIsEnabled()
        rule.onNodeWithText("Presque aucun délai de compression, mais 5 à 10 fois plus de débit que HEVC. Fait pour un câble Ethernet.").assertExists()
        rule.onNodeWithText(fr("En Wi-Fi, PyroWave peut saccader : il lui faut environ 300\u00a0Mbit/s en 1080p. Gardez « Auto » si vous n'êtes pas sûr.")).assertExists()
        rule.onNodeWithText(fr("Avec PyroWave, presque sans coût : seul le débit monte.")).performScrollTo().assertExists()
    }

    @Test
    fun format_video_pyrowave_par_le_relais() {
        settings("pyrowave", PyroWaveChoice.of(host, decodes = true, wired = true, relayed = true))
        rule.onNodeWithText(fr("Hors de chez vous, PyroWave passe par le relais et par votre connexion Internet : l'image risque de saccader. Gardez « Auto » si vous n'êtes pas sûr.")).assertExists()
    }

    @Test
    fun format_video_auto_par_cable_et_en_wifi() {
        settings("auto", PyroWaveChoice.of(host, decodes = true, wired = true, relayed = false), profile = Profile.MAX)
        rule.onNodeWithText(fr("Auto : PyroWave, car cet appareil est relié par câble. Sinon HEVC.")).assertExists()
        rule.onNodeWithText(fr("En Wi-Fi, PyroWave peut saccader : il lui faut environ 300\u00a0Mbit/s en 1080p. Gardez « Auto » si vous n'êtes pas sûr.")).assertDoesNotExist()
    }

    @Test
    fun format_video_auto_par_cable_en_jeu_et_bureau() {
        // Jeu et Bureau restent en HEVC par câble : seuls Max et Auto prennent PyroWave (docs/profil-auto.md).
        settings("auto", PyroWaveChoice.of(host, decodes = true, wired = true, relayed = false))
        rule.onNodeWithText(fr("Auto : HEVC. PyroWave est pris par Max et Auto, quand cet appareil est relié par câble.")).assertExists()
        rule.onNodeWithText(fr("Auto : PyroWave, car cet appareil est relié par câble. Sinon HEVC.")).assertDoesNotExist()
    }

    @Test
    fun format_video_auto_sans_cable() {
        settings("auto", PyroWaveChoice.of(host, decodes = true, wired = false, relayed = false))
        rule.onNodeWithText(fr("Auto : HEVC. PyroWave est pris seulement quand cet appareil est relié par câble.")).assertExists()
    }

    @Test
    fun format_video_pyrowave_inactif_avec_sa_raison() {
        settings("auto", PyroWaveChoice.of(host, decodes = false, wired = true, relayed = false))
        rule.onNodeWithText("PyroWave").assertIsNotEnabled()
        rule.onNodeWithText("Cet appareil ne sait pas décoder PyroWave.", substring = true).assertExists()
    }

    @Test
    fun format_video_station_qui_ne_le_propose_pas() {
        settings("auto", PyroWaveChoice.of(host.copy(pyrowave = false), decodes = true, wired = true, relayed = false))
        rule.onNodeWithText("Atelier ne propose pas PyroWave.", substring = true).assertExists()
    }

    @Test
    fun format_video_station_d_avant_pyrowave() {
        settings("auto", PyroWaveChoice.of(host.copy(pyrowave = null), decodes = true, wired = true, relayed = false))
        rule.onNodeWithText("PyroWave").assertDoesNotExist()
    }

    private class Actions : SessionActions {
        var hevc = 0
        override fun resume() {}
        override fun quit() {}
        override fun setProfile(p: Profile) {}
        override fun switchToHevc() { hevc++ }
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

    private fun session(setup: SessionUi.() -> Unit, actions: Actions): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        state.decoder.value = "PyroWave (Vulkan, Mali-G57, fragment)"
        return SessionUi("Atelier", false, state, actions).apply { passageDone = true; setup() }
    }

    @Test
    fun session_bandeau_puce_et_passer_en_hevc() {
        val actions = Actions()
        val ui = session({ pyroChosen = true; pyroLink = PyroWaveLink.WIFI; barOpen = true }, actions)
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText("L'image peut saccader.").assertExists()
        rule.onNodeWithText("Passer en HEVC").performClick()
        assertEquals(1, actions.hevc)
        // La puce ambre de la barre (et le titre du bandeau) : « PyroWave en Wi-Fi ».
        rule.onAllNodesWithText("PyroWave en Wi-Fi").fetchSemanticsNodes().isNotEmpty().let { assertEquals(true, it) }
    }

    @Test
    fun session_par_cable_ni_bandeau_ni_puce() {
        val ui = session({ pyroChosen = true; pyroLink = PyroWaveLink.CABLE; barOpen = true }, Actions())
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText("L'image peut saccader.").assertDoesNotExist()
        rule.onNodeWithText("PyroWave en Wi-Fi").assertDoesNotExist()
    }

    @Test
    fun statistiques_images_perdues() {
        val ui = session({ statsOpen = true }, Actions())
        ui.state.second.value = Second(4.1, 1.0, 119, 1, 0, 0, 290.0, null)
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText("Images perdues ", substring = true).assertExists()
    }

    @Test
    fun statistiques_definition_ramenee_du_coeur() {
        val ui = session({ statsOpen = true }, Actions())
        ui.state.second.value = Second(4.1, 1.0, 120, 0, 0, 0, 290.0, null, lowered = Lowered(1920, 1080, 120, 300))
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        // Le texte du cœur (`pyrowave::Lowered::text`), ici celui du faux cœur.
        rule.onNodeWithText("Définition ramenée à 1920 × 1080 pour tenir dans 300 Mbit/s.").assertExists()
    }

    @Test
    fun avis_definition_ramenee_et_retour() {
        val ui = session({}, Actions())
        val text = "Définition ramenée à 1920 × 1080 pour tenir dans 300 Mbit/s."
        ui.state.onNotice(9, 1920, 1080, false, 0, 300, true, text)
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText(text).assertExists()
        assertEquals(text, ui.state.lowered.value)
        ui.state.onNotice(9, 0, 0, false, 0, 0, false, null)
        rule.waitForIdle()
        assertEquals(null, ui.state.lowered.value)
    }

    @Test
    fun bandeau_du_debut_reprend_la_definition_ramenee() {
        val ui = session({ pyroChosen = true; pyroLink = PyroWaveLink.WIFI }, Actions())
        ui.state.onNotice(9, 1920, 1080, false, 0, 300, true, "Définition ramenée à 1920 × 1080 pour tenir dans 300 Mbit/s.")
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText("L'image peut saccader. Définition ramenée à 1920 × 1080 pour tenir dans 300 Mbit/s.").assertExists()
    }

    @Test
    fun bandeau_quand_le_reseau_sature() {
        // Avis 10 du cœur (docs/debit-automatique.md) : même bandeau, titré « Le réseau sature », même par câble.
        val actions = Actions()
        val ui = session({ pyroLink = PyroWaveLink.CABLE }, actions)
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText("Le réseau sature").assertDoesNotExist()
        ui.state.onNotice(10, 0, 0, false, 0, 0, false, null)
        rule.waitForIdle()
        rule.onNodeWithText("Le réseau sature").assertExists()
        rule.onNodeWithText("L'image peut saccader.").assertExists()
        rule.onNodeWithText("Passer en HEVC").performClick()
        assertEquals(1, actions.hevc)
    }

    @Test
    fun bandeau_quand_le_reseau_sature_au_televiseur() {
        val ui = session({ pyroChosen = true; pyroLink = PyroWaveLink.WIFI }, Actions())
        ui.state.onNotice(10, 0, 0, false, 0, 0, false, null)
        show(FormFactor.TV) { SessionOverlay(ui) }
        // Il prend la place du bandeau du début : un seul titre.
        rule.onNodeWithText("Le réseau sature").assertExists()
        rule.onNodeWithText("PyroWave en Wi-Fi").assertDoesNotExist()
        rule.onNodeWithText("L'image peut saccader.").assertExists()
    }
}
