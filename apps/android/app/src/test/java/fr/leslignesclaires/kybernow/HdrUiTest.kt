package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
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

/**
 * Le HDR à l'écran (docs/link-ui.md, section HDR), en français : la puce de l'accueil à côté du profil et la ligne
 * « Écran … en HDR », rien sur un écran SDR, la puce inactive ; Réglages › Image (interrupteur « HDR », texte net
 * inactif) ; « HDR » dans la barre et le menu du téléviseur, l'avis du refus.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class HdrUiTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()
    private val nb = " "

    private val host = Host(id = "a", name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab", user = "m", settings = HostSettings(profile = Profile.DESKTOP))
    private val screen = Screen(ScreenMode(1, 3840, 2160, 60), listOf(ScreenMode(1, 3840, 2160, 60)), true, false)
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, true, hevcHdr = true)
    private val on = HdrState(true, true)
    private val inactive = HdrState(true, false, "Le HDR demande Windows 11 24H2 sur l'ordinateur.")

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    private fun home(hdr: HdrState, set: HostSettings = host.settings, toggled: MutableList<Boolean> = mutableListOf()) {
        val h = host.copy(settings = set)
        val actions = HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {}, setHdr = { _, v -> toggled += v })
        val state = HomeState(listOf(h), "a", mapOf("a" to HostStatus(true)), emptyMap(), emptyList(), screen, decoders, hdr = mapOf("a" to hdr))
        show(FormFactor.TABLET) { HomeContent(state, actions) }
    }

    @Test
    fun accueil_puce_a_cote_du_profil_et_ligne_ecran_en_hdr() {
        val toggled = mutableListOf<Boolean>()
        home(on, toggled = toggled)
        rule.onNodeWithText("Écran de 3840${nb}×${nb}2160 à 60${nb}Hz, à la taille de cet écran, en HDR").assertExists()
        rule.onNodeWithText("HDR").assertIsEnabled().performClick()
        assertEquals(listOf(false), toggled)
    }

    @Test
    fun accueil_hdr_eteint_pour_cet_ordinateur() {
        home(on, set = host.settings.copy(hdr = false))
        rule.onNodeWithText("HDR").assertExists()
        rule.onNodeWithText("Écran de 3840${nb}×${nb}2160 à 60${nb}Hz, à la taille de cet écran").assertExists()
    }

    @Test
    fun accueil_rien_sur_un_ecran_sdr() {
        home(HdrState.HIDDEN)
        rule.onNodeWithText("HDR").assertDoesNotExist()
        rule.onNodeWithText("Écran de 3840${nb}×${nb}2160 à 60${nb}Hz, à la taille de cet écran").assertExists()
    }

    @Test
    fun accueil_puce_inactive() {
        home(inactive)
        rule.onNodeWithText("HDR").assertIsNotEnabled()
        rule.onNodeWithText("Écran de 3840${nb}×${nb}2160 à 60${nb}Hz, à la taille de cet écran").assertExists()
    }

    private fun settings(hdr: HdrState, updates: MutableList<HostSettings> = mutableListOf()) {
        var current = host.settings
        val actions = HostSettingsActions({ f -> current = f(current); updates += current }, { _, _ -> }, {}, {}, { _, _ -> }, {}, {}, {}, {})
        show(FormFactor.PHONE) {
            HostSettingsContent(HostSettingsState(host, HostStatus(true), screen, decoders, "https://192.168.1.20:41234/", hdr = hdr), actions, Rubric.PICTURE, only = Rubric.PICTURE)
        }
    }

    @Test
    fun reglages_image_interrupteur_hdr_et_texte_net_inactif() {
        val updates = mutableListOf<HostSettings>()
        settings(on, updates)
        rule.onNodeWithText("Le HDR n'existe pas en 4:4:4.").assertExists()
        rule.onNodeWithText("Couleurs plus riches et lumières plus vives, sur cet écran HDR. Le texte net (4:4:4) n'existe pas en HDR.").assertExists()
        rule.onNodeWithText("HDR").performScrollTo().performClick()
        assertEquals(false, updates.last().hdr)
    }

    @Test
    fun reglages_image_hdr_inactif_avec_la_raison() {
        settings(inactive)
        rule.onNodeWithText("Le HDR demande Windows 11 24H2 sur l'ordinateur.").assertExists()
        // Le HDR impossible : le texte net garde son aide.
        rule.onNodeWithText("Couleurs complètes : le texte reste net, au prix d'un débit plus élevé.".replace(" :", " :")).assertExists()
    }

    @Test
    fun reglages_image_rien_sur_un_ecran_sdr() {
        settings(HdrState.HIDDEN)
        rule.onNodeWithText("HDR").assertDoesNotExist()
    }

    private class Actions : SessionActions {
        val hdr = mutableListOf<Boolean>()
        override fun resume() {}
        override fun quit() {}
        override fun setProfile(p: Profile) {}
        override fun setHdr(on: Boolean) { hdr += on }
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

    private fun session(tv: Boolean, actions: Actions, setup: SessionUi.() -> Unit): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        return SessionUi("Atelier", tv, state, actions).apply { passageDone = true; setup() }
    }

    @Test
    fun barre_hdr_cochable_pres_du_profil() {
        val actions = Actions()
        val ui = session(false, actions) { hdrState = on; hdrChoice = true; barOpen = true }
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        // Les réglages de la barre défilent : « HDR » suit les profils.
        rule.onNodeWithText("HDR").performScrollTo().performClick()
        assertEquals(listOf(false), actions.hdr)
    }

    @Test
    fun barre_sans_hdr_sur_un_ecran_sdr() {
        val ui = session(false, Actions()) { barOpen = true }
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText("Bureau").assertExists()
        rule.onNodeWithText("HDR").assertDoesNotExist()
    }

    @Test
    fun menu_du_televiseur_tuile_hdr() {
        val actions = Actions()
        val ui = session(true, actions) { hdrState = on; hdrChoice = false; menuOpen = true }
        show(FormFactor.TV) { SessionOverlay(ui) }
        rule.onNodeWithText("éteint").assertExists()
        rule.onNodeWithText("HDR").performClick()
        assertEquals(listOf(true), actions.hdr)
    }

    @Test
    fun avis_du_refus() {
        val ui = session(false, Actions()) {}
        ui.state.capsule.value = Capsule.HdrRefused("")
        show(FormFactor.TABLET) { SessionOverlay(ui) }
        rule.onNodeWithText("L'ordinateur n'a pas pu passer en HDR : image en SDR.").assertExists()
    }
}
