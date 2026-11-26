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
import fr.leslignesclaires.kybernow.model.AutoChoice
import fr.leslignesclaires.kybernow.model.AutoStep
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.PyroWaveChoice
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.ui.AutoTestActions
import fr.leslignesclaires.kybernow.ui.AutoTestPanel
import fr.leslignesclaires.kybernow.ui.AutoTestState
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
 * Le profil Auto à l'écran (docs/link-ui.md, « Profil Auto et débit adaptatif »), en français : Auto parmi les profils
 * de l'accueil, le panneau du test (étapes, résumé, échec), la carte « Débit » (le test qui a choisi, « Refaire le
 * test », « Débit adaptatif » qui ne fait pas passer en « Personnalisé »).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class AutoProfileUiTest {
    @get:Rule val rule = createComposeRule()

    private val n = " "
    private fun fr(s: String) = s.replace(" :", "$n:")

    private val host = Host(id = "a", name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab", user = "m", pyrowave = true)
    private val screen = Screen(ScreenMode(1, 3840, 2160, 60), listOf(ScreenMode(1, 3840, 2160, 60)), true, false)
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, false)

    private val result = JSONObject()
        .put("at", 1_795_000_000L).put("network", "Ethernet")
        .put("measures", JSONObject().put("link", "cable").put("decode", org.json.JSONArray()))
        .put("codec", "hevc").put("yuv444", false).put("width", 3840).put("height", 2160).put("refresh", 60)
        .put("mbps", 150).put("mode_mbps", 150).put("reliable", false)
        .toString()

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    @Test
    fun accueil_auto_parmi_les_profils() {
        val picked = mutableListOf<Profile>()
        val actions = HomeActions({}, { _, p -> picked += p }, {}, {}, {}, {}, {}, {}, {}, {})
        val state = HomeState(listOf(host), "a", mapOf("a" to HostStatus(true)), emptyMap(), emptyList(), screen, decoders)
        show(FormFactor.TABLET) { HomeContent(state, actions) }
        rule.onNodeWithText("Auto").performScrollTo().performClick()
        assertEquals(listOf(Profile.AUTO), picked)
    }

    private val steps = listOf(
        AutoStep("link", "Liaison", AutoStep.State.DONE, "Câble"),
        AutoStep("latency", "Latence au repos", AutoStep.State.DONE, "2,1 ms · gigue 0,4 ms"),
        AutoStep("bandwidth", "Débit vers cet appareil", AutoStep.State.RUNNING, null),
        AutoStep("decode", "Décodage de cet appareil", AutoStep.State.WAITING, null),
    )

    @Test
    fun panneau_pendant_le_test() {
        var cancelled = false
        show(FormFactor.TABLET) { AutoTestPanel(AutoTestState("a", "Atelier", steps), AutoTestActions({}, { cancelled = true }, {})) }
        rule.onNodeWithText("Profil Auto").assertExists()
        rule.onNodeWithText("KyberNow mesure votre réseau et cet appareil, puis propose les meilleurs réglages.").assertExists()
        rule.onNodeWithText("Latence au repos").assertExists()
        rule.onNodeWithText("2,1 ms · gigue 0,4 ms").assertExists()
        rule.onNodeWithText("Décodage de cet appareil").assertExists()
        rule.onNodeWithText("Annuler").performClick()
        assertTrue(cancelled)
    }

    @Test
    fun panneau_reglages_proposes() {
        var applied = false
        var again = false
        val choices = listOf(
            AutoChoice("codec", "Format vidéo", "PyroWave", "relié par câble, et le réseau le porte"),
            AutoChoice("adaptive", "Débit adaptatif", "Oui", "suit le réseau pendant la session"),
        )
        show(FormFactor.TABLET) {
            AutoTestPanel(AutoTestState("a", "Atelier", steps, result = result, choices = choices), AutoTestActions({ applied = true }, {}, { again = true }))
        }
        rule.onNodeWithText("Réglages proposés".uppercase()).assertExists()
        rule.onNodeWithText("relié par câble, et le réseau le porte").assertExists()
        rule.onNodeWithText("Refaire le test").performScrollTo().performClick()
        assertTrue(again)
        rule.onNodeWithText("Appliquer").performScrollTo().performClick()
        assertTrue(applied)
    }

    @Test
    fun panneau_echec() {
        var again = false
        show(FormFactor.TABLET) {
            AutoTestPanel(AutoTestState("a", "Atelier", steps, error = "Le test n'a pas pu joindre Atelier."), AutoTestActions({}, {}, { again = true }))
        }
        rule.onNodeWithText("Le test n'a pas pu joindre Atelier.").assertExists()
        rule.onNodeWithText("Réessayer").performClick()
        assertTrue(again)
    }

    private fun settings(set: HostSettings, updates: MutableList<HostSettings> = mutableListOf(), retest: () -> Unit = {}) {
        var current = set
        val h = host.copy(settings = current)
        val actions = HostSettingsActions({ f -> current = f(current); updates += current }, { _, _ -> }, {}, {}, { _, _ -> }, {}, {}, {}, {}, retest = retest)
        show(FormFactor.PHONE) {
            HostSettingsContent(
                HostSettingsState(h, HostStatus(true), screen, decoders, "https://192.168.1.20:41234/", pyrowave = PyroWaveChoice.of(h, decodes = true, wired = true, relayed = false)),
                actions, Rubric.PICTURE, only = Rubric.PICTURE,
            )
        }
    }

    @Test
    fun carte_debit_profil_auto() {
        var retested = false
        val updates = mutableListOf<HostSettings>()
        settings(HostSettings(profile = Profile.AUTO, autoResult = result), updates) { retested = true }
        rule.onNodeWithText("Choisi par le test du", substring = true).performScrollTo().assertExists()
        rule.onNodeWithText("(Ethernet).", substring = true).assertExists()
        rule.onNodeWithText("Refaire le test").performScrollTo().performClick()
        assertTrue(retested)
        rule.onNodeWithText("Débit adaptatif").performScrollTo().performClick()
        assertFalse(updates.last().adaptive)
        assertFalse("le débit adaptatif ne fait pas passer en « Personnalisé »", updates.last().custom)
        assertEquals(Profile.AUTO, updates.last().profile)
    }

    @Test
    fun carte_debit_aide_pyrowave() {
        settings(HostSettings(profile = Profile.GAME, codec = "pyrowave"))
        rule.onNodeWithText(
            fr("Baisse le débit quand le réseau sature, puis le remonte. Coupé, le débit ne bouge jamais. En PyroWave, il est en pause : la définition baisse à la place."),
        ).performScrollTo().assertExists()
        rule.onNodeWithText("Choisi par le test du", substring = true).assertDoesNotExist()
    }

    private class Actions : SessionActions {
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
    }

    /** « Refaire le test », ou Auto choisi sans test, en session : le panneau attend la fin de la session, et la capsule le dit. */
    @Test
    fun session_test_a_la_fin_de_la_session() {
        val context = ApplicationProvider.getApplicationContext<Application>()
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        state.capsule.value = Capsule.AutoTestLater
        val ui = SessionUi("Atelier", false, state, Actions()).apply { passageDone = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Le test se fera à la fin de la session.").assertExists()
    }
}
