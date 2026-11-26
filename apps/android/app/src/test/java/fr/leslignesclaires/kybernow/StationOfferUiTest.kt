package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.StationBanner
import fr.leslignesclaires.kybernow.core.StationOffer
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.NoticeEntry
import fr.leslignesclaires.kybernow.ui.StationAction
import fr.leslignesclaires.kybernow.ui.StationNotice
import fr.leslignesclaires.kybernow.ui.StationNoticeActions
import fr.leslignesclaires.kybernow.ui.StationNoticePanel
import fr.leslignesclaires.kybernow.ui.StationTask
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
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
 * Mise à jour des Stations proposée par l'app (docs/link-ui.md, « Mise à jour des Stations »), en français : la sortie du
 * cœur lue ([StationOffer.parse]), le bandeau de l'accueil (proposition, installateur, pendant, confirmation lancée d'ici),
 * la puce « · Mise à jour » de la rangée, l'avis après une mise à jour de l'app (une Station, plusieurs, « Fermer »).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class StationOfferUiTest {
    @get:Rule val rule = createComposeRule()

    private val host = Host(id = "a", name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab", user = "m")
    private val other = Host(id = "b", name = "Salon", address = "192.168.1.21", port = 41234, fingerprint = "sha256:cd", user = "m")
    private val screen = Screen(ScreenMode(1, 1920, 1080, 60), listOf(ScreenMode(1, 1920, 1080, 60)), true, false)
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, true)

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    private class Calls {
        val updates = mutableListOf<Boolean>()
        val later = mutableListOf<String>()
        var cancelled = 0
    }

    private fun home(offers: Map<String, StationOffer>, tasks: Map<String, StationTask> = emptyMap(), fromOffer: Set<String> = emptySet(), calls: Calls = Calls()) {
        val actions = HomeActions(
            {}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {},
            stationUpdate = { _, force -> calls.updates += force },
            stationLater = { _, v -> calls.later += v },
            stationCancel = { calls.cancelled++ },
        )
        val state = HomeState(
            listOf(host, other), "a", mapOf("a" to HostStatus(true), "b" to HostStatus(true)), emptyMap(), emptyList(), screen, decoders,
            offers = offers, stationTasks = tasks, fromOffer = fromOffer,
        )
        show(FormFactor.TABLET) { HomeContent(state, actions) }
    }

    @Test
    fun sortie_du_coeur_lue() {
        val o = StationOffer.parse("""{"banner":{"kind":"update","version":"0.11.3"},"chip":true,"notice_version":"0.11.3","should_check":false}""")
        assertEquals(StationBanner.Update("0.11.3"), o.banner)
        assertTrue(o.chip)
        assertEquals("0.11.3", o.noticeVersion)
        assertFalse(o.shouldCheck)
        val none = StationOffer.parse("""{"banner":{"kind":"none"},"chip":false,"notice_version":null,"should_check":true}""")
        assertEquals(StationBanner.None, none.banner)
        assertNull(none.noticeVersion)
        assertTrue(none.shouldCheck)
        assertEquals(StationBanner.Installing, StationOffer.parse("""{"banner":{"kind":"installing"}}""").banner)
        assertEquals(StationOffer(), StationOffer.parse("illisible"))
    }

    @Test
    fun memoire_plus_tard_et_verification() {
        val m = StationOffer.checked(StationOffer.snooze("{}", "0.11.3", 100), "0.11.3", 200)
        val o = org.json.JSONObject(m)
        assertEquals("0.11.3", o.getString("snoozed_version"))
        assertEquals(100, o.getLong("snoozed_at"))
        assertEquals("0.11.3", o.getString("checked_app_version"))
        assertEquals(200, o.getLong("checked_at"))
    }

    @Test
    fun bandeau_propose_la_mise_a_jour() {
        val calls = Calls()
        home(mapOf("a" to StationOffer(StationBanner.Update("0.11.3"), chip = true)), calls = calls)
        rule.onNodeWithText("KyberNow Station 0.11.3 est disponible pour cet ordinateur. Environ une minute.").performScrollTo().assertExists()
        rule.onNodeWithText("Mettre à jour").performScrollTo().performClick()
        rule.onNodeWithText("Plus tard").performScrollTo().performClick()
        assertEquals(listOf(false), calls.updates)
        assertEquals(listOf("0.11.3"), calls.later)
    }

    @Test
    fun bandeau_installateur_plus_tard_seul() {
        home(mapOf("a" to StationOffer(StationBanner.Setup("0.12.0"))))
        rule.onNodeWithText("KyberNow Station 0.12.0 est disponible. Elle s'installe sur l'ordinateur, avec l'installateur.").performScrollTo().assertExists()
        rule.onNodeWithText("Plus tard").assertExists()
        rule.onNodeWithText("Mettre à jour").assertDoesNotExist()
    }

    @Test
    fun bandeau_pendant() {
        home(mapOf("a" to StationOffer(StationBanner.Update("0.11.3"))), tasks = mapOf("a" to StationTask.Working(StationAction.UPDATE, "0.11.3")))
        rule.onNodeWithText("Mise à jour de la Station…").performScrollTo().assertExists()
        rule.onNodeWithText("Plus tard").assertDoesNotExist()
    }

    @Test
    fun bandeau_confirmation_lancee_d_ici() {
        val calls = Calls()
        home(
            mapOf("a" to StationOffer(StationBanner.Update("0.11.3"))), tasks = mapOf("a" to StationTask.Confirm(StationAction.UPDATE, "Pixel")),
            fromOffer = setOf("a"), calls = calls,
        )
        rule.onNodeWithText("Pixel est connecté").performScrollTo().assertExists()
        rule.onNodeWithText("Mettre à jour quand même").performScrollTo().performClick()
        rule.onNodeWithText("Annuler").performScrollTo().performClick()
        assertEquals(listOf(true), calls.updates)
        assertEquals(1, calls.cancelled)
    }

    @Test
    fun puce_de_la_rangee() {
        home(mapOf("b" to StationOffer(chip = true)))
        rule.onNodeWithText("En ligne · Mise à jour").assertExists()
    }

    @Test
    fun avis_une_station() {
        val updated = mutableListOf<String>()
        var closed = 0
        show(FormFactor.TABLET) {
            StationNoticePanel(
                StationNotice("0.11.3", listOf(NoticeEntry("a", "Atelier", "0.11.3"))), emptyMap(),
                StationNoticeActions(update = { id, _ -> updated += id }, updateAll = {}, close = { closed++ }),
            )
        }
        rule.onNodeWithText("Atelier a aussi une mise à jour").assertExists()
        rule.onNodeWithText("Mettre à jour la Station").performClick()
        rule.onNodeWithText("Plus tard").performClick()
        assertEquals(listOf("a"), updated)
        assertEquals(1, closed)
    }

    @Test
    fun avis_plusieurs_stations_puis_fermer() {
        val entries = listOf(NoticeEntry("a", "Atelier", "0.11.3"), NoticeEntry("b", "Salon", "0.11.3"))
        show(FormFactor.TABLET) {
            StationNoticePanel(
                StationNotice("0.11.3", entries, updated = setOf("a", "b")), emptyMap(),
                StationNoticeActions(update = { _, _ -> }, updateAll = {}, close = {}),
            )
        }
        rule.onNodeWithText("Vos Stations aussi ont une mise à jour").assertExists()
        rule.onNodeWithText("Fermer").assertExists()
        rule.onNodeWithText("Tout mettre à jour").assertDoesNotExist()
    }
}
