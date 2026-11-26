package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.text.TextLayoutResult
import androidx.test.core.app.ApplicationProvider
import com.github.takahirom.roborazzi.captureRoboImage
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.StationBanner
import fr.leslignesclaires.kybernow.core.StationOffer
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.HdrState
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
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
 * Les rangées de l'accueil à la place la plus étroite de chaque forme (docs/link-ui.md, « Principes » : un bouton tient
 * sur une ligne ; quand la place manque, la rangée passe à la ligne, jamais le libellé) : « Se connecter », les quatre
 * profils (Auto compris) et « Personnalisé », la puce « HDR », « Réglages » ; en haut, la mise à jour de l'app,
 * « Connecter un compte » et « Réglages de l'app » ; le bandeau de la Station. En anglais (libellés plus longs) et en
 * français. Chaque libellé : une seule ligne, rien de coupé. Captures dans `build/rangees/`.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class)
class HomeRowsUiTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()

    private val host = Host(
        id = "a", name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab", user = "m",
        // Auto choisi, puis un réglage changé : « Personnalisé » en plus des quatre profils (la rangée la plus longue).
        settings = HostSettings(profile = Profile.AUTO, codec = "h264"),
    )
    private val screen = Screen(ScreenMode(1, 3840, 2160, 60), listOf(ScreenMode(1, 3840, 2160, 60)), true, false)
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, true, hevcHdr = true)

    private fun home(form: FormFactor, name: String) {
        val state = HomeState(
            listOf(host), "a", mapOf("a" to HostStatus(true)), emptyMap(), emptyList(), screen, decoders,
            update = "0.11.3",
            hdr = mapOf("a" to HdrState(true, true)),
            offers = mapOf("a" to StationOffer(StationBanner.Update("0.11.3"), chip = true)),
            hasAccount = false,
        )
        rule.setContent { KnTheme(form, motion = false) { HomeContent(state, HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {})) } }
        rule.onRoot().captureRoboImage("build/rangees/$name.png")
    }

    /** Chaque libellé présent : une ligne, sans rien de coupé (un texte serré passerait à la ligne ou serait rogné). */
    private fun assertOneLine(vararg ids: Int, extra: List<String> = emptyList()) {
        val labels = ids.map { context.getString(it) } + extra
        for (label in labels) {
            val nodes = rule.onAllNodesWithText(label, useUnmergedTree = true).fetchSemanticsNodes()
            for (node in nodes) {
                val layouts = mutableListOf<TextLayoutResult>()
                node.config[SemanticsActions.GetTextLayoutResult].action?.invoke(layouts)
                val layout = layouts.single()
                assertEquals("« $label » sur plus d'une ligne", 1, layout.lineCount)
                // Le texte d'une ligne, mesuré, et la place que la mise en page lui donne : rien de rogné.
                val text = layout.getLineRight(0) - layout.getLineLeft(0)
                assertTrue("« $label » rogné : ${text} px de texte dans ${node.size.width} px", text <= node.size.width + 1f)
            }
        }
    }

    private fun check(form: FormFactor, name: String, tall: Boolean) {
        home(form, name)
        val ids = mutableListOf(
            R.string.home_connect, R.string.profile_game, R.string.profile_desktop, R.string.profile_max, R.string.profile_auto,
            R.string.profile_custom, R.string.hdr, R.string.station_offer_update, R.string.station_offer_later,
        )
        // Le téléphone montre l'icône seule pour le compte et les réglages de l'app ; en pile, l'engrenage seul.
        if (form != FormFactor.PHONE) ids += listOf(R.string.account_sign_in, R.string.home_app_settings)
        if (!tall) ids += R.string.home_host_settings
        assertOneLine(*ids.toIntArray(), extra = listOf(context.getString(R.string.updates_available, "0.11.3")))
        assertTrue(rule.onAllNodesWithText(context.getString(R.string.profile_auto), useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty())
    }

    // Tablette en paysage la plus étroite (sw600) : rangée en ligne qui passe à la ligne.
    @Test @Config(qualifiers = "en-w960dp-h600dp-land-xhdpi")
    fun tablette_paysage_en() = check(FormFactor.TABLET, "tablette-paysage-en", tall = false)

    @Test @Config(qualifiers = "fr-w960dp-h600dp-land-xhdpi")
    fun tablette_paysage_fr() = check(FormFactor.TABLET, "tablette-paysage-fr", tall = false)

    // Tablette en portrait la plus étroite : profils en commande segmentée.
    @Test @Config(qualifiers = "en-w600dp-h960dp-port-xhdpi")
    fun tablette_portrait_en() = check(FormFactor.TABLET, "tablette-portrait-en", tall = true)

    @Test @Config(qualifiers = "fr-w600dp-h960dp-port-xhdpi")
    fun tablette_portrait_fr() = check(FormFactor.TABLET, "tablette-portrait-fr", tall = true)

    // Téléphone : 360 dp, le plus étroit courant ; en paysage, la mise en page large dès 720 dp.
    @Test @Config(qualifiers = "en-w360dp-h780dp-port-xxhdpi")
    fun telephone_en() = check(FormFactor.PHONE, "telephone-en", tall = true)

    @Test @Config(qualifiers = "fr-w360dp-h780dp-port-xxhdpi")
    fun telephone_fr() = check(FormFactor.PHONE, "telephone-fr", tall = true)

    @Test @Config(qualifiers = "en-w720dp-h360dp-land-xxhdpi")
    fun telephone_paysage_en() = check(FormFactor.PHONE, "telephone-paysage-en", tall = false)

    @Test @Config(qualifiers = "fr-w720dp-h360dp-land-xxhdpi")
    fun telephone_paysage_fr() = check(FormFactor.PHONE, "telephone-paysage-fr", tall = false)

    // Téléviseur, 1080p (960 × 540 dp).
    @Test @Config(qualifiers = "en-w960dp-h540dp-land-television-xhdpi")
    fun tv_en() = check(FormFactor.TV, "tv-en", tall = false)

    @Test @Config(qualifiers = "fr-w960dp-h540dp-land-television-xhdpi")
    fun tv_fr() = check(FormFactor.TV, "tv-fr", tall = false)
}
