package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToKey
import com.github.takahirom.roborazzi.captureRoboImage
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.TvCode
import fr.leslignesclaires.kybernow.model.Announced
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.account.CodeStep
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
import fr.leslignesclaires.kybernow.ui.welcome.AwayHintContent
import fr.leslignesclaires.kybernow.ui.welcome.WelcomeActions
import fr.leslignesclaires.kybernow.ui.welcome.WelcomeContent
import fr.leslignesclaires.kybernow.ui.welcome.WelcomeUi
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * L'accueil et l'incitation au compte à l'écran (docs/link-ui.md, « Accueil et compte KyberNow »), en français : les
 * trois écrans (téléphone, tablette, téléviseur avec le code), le bouton et la carte du compte, le bandeau après un
 * appairage et « Hors de chez vous ? ». Chaque test laisse une capture dans app/screenshots/ (`bienvenue-…`), pour
 * vérifier à l'œil que rien ne se chevauche ni ne passe sur deux lignes.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w411dp-h914dp-port-xxhdpi")
class WelcomeUiTest {
    @get:Rule val rule = createComposeRule()

    private val n = " "
    private val screen = Screen(ScreenMode(1, 1920, 1080, 60), listOf(ScreenMode(1, 1920, 1080, 60)), false, false)
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, true)
    private val announced = listOf(Announced("kn", "PC-DU-SALON", "192.168.1.30", 47990, "sha256:aa"))

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }
    private fun capture(name: String) = rule.onRoot().captureRoboImage("screenshots/bienvenue-$name.png")
    private fun androidx.compose.ui.test.junit4.ComposeContentTestRule.onRoot() = this.onNode(androidx.compose.ui.test.isRoot())

    @Test
    fun ecran1_puisContinuer() {
        var next = 0
        var skip = 0
        show(FormFactor.PHONE) { WelcomeContent(WelcomeUi(0), WelcomeActions(next = { next++ }, skip = { skip++ })) }
        rule.onNodeWithText("BIENVENUE · 1 / 3").assertExists()
        rule.onNodeWithText("VOTRE ORDINATEUR, SUR TOUS VOS APPAREILS").assertExists()
        rule.onNodeWithText("Jouez, travaillez et retrouvez votre bureau depuis un autre écran, comme si vous étiez devant.").assertExists()
        capture("1-telephone")
        rule.onNodeWithText("Continuer").performClick()
        rule.onNodeWithText("Passer").performClick()
        assertEquals(1, next)
        assertEquals(1, skip)
    }

    @Test
    fun ecran2_telephone() {
        var signIn = 0
        var create = 0
        var next = 0
        show(FormFactor.PHONE) { WelcomeContent(WelcomeUi(1), WelcomeActions(next = { next++ }, signIn = { signIn++ }, createAccount = { create++ })) }
        rule.onNodeWithText("AVEC UN COMPTE, TOUT VOUS SUIT").assertExists()
        rule.onNodeWithText("HORS DE CHEZ VOUS").assertExists()
        rule.onNodeWithText("PARTAGER ET PRÊTER").assertExists()
        capture("2-telephone")
        rule.onNodeWithText("Connecter un compte").performScrollTo().performClick()
        rule.onNodeWithText("Créer un compte").performScrollTo().performClick()
        rule.onNodeWithText("Continuer sans compte").performScrollTo().performClick()
        rule.onNodeWithText("La page de share.kybernow.app s'ouvre${n}: votre mot de passe ne passe jamais par l'app.").performScrollTo().assertExists()
        assertEquals(listOf(1, 1, 1), listOf(signIn, create, next))
    }

    @Test
    @Config(qualifiers = "fr-w1280dp-h800dp-land-xhdpi")
    fun ecran2_tablette() {
        show(FormFactor.TABLET) { WelcomeContent(WelcomeUi(1), WelcomeActions()) }
        rule.onNodeWithText("Connecter un compte").assertExists()
        capture("2-tablette")
    }

    @Test
    @Config(qualifiers = "fr-w960dp-h540dp-land-television-xhdpi")
    fun ecran2_televiseur_leCode() {
        val code = TvCode("share.kybernow.app", "KQ7M-4XPA", "share.kybernow.app/tv", "https://share.kybernow.app/tv?code=KQ7M-4XPA", 5, "{}")
        var next = 0
        show(FormFactor.TV) { WelcomeContent(WelcomeUi(1, code = CodeStep.Showing(code, 9)), WelcomeActions(next = { next++ })) }
        rule.onNodeWithText("CONNECTER UN COMPTE").assertExists()
        rule.onNodeWithText("KQ7M-4XPA").assertExists()
        rule.onNodeWithContentDescription("QR code de connexion").assertExists()
        rule.onNodeWithText("Pas encore de compte${n}? Créez-le sur cette page.").assertExists()
        rule.onNodeWithText("Créer un compte").assertDoesNotExist()
        capture("2-tv")
        rule.onNodeWithText("Continuer sans compte").performClick()
        assertEquals(1, next)
    }

    @Test
    fun ecran3_aPortee() {
        val added = mutableListOf<String>()
        var join = 0
        var download = 0
        var later = 0
        show(FormFactor.PHONE) {
            WelcomeContent(WelcomeUi(2, announced), WelcomeActions(skip = { later++ }, addAnnounced = { added += it.name }, join = { join++ }, download = { download++ }))
        }
        rule.onNodeWithText("AJOUTEZ VOTRE ORDINATEUR").assertExists()
        rule.onNodeWithText("ORDINATEURS À PORTÉE").assertExists()
        rule.onNodeWithText("SUR LE RÉSEAU").assertExists()
        capture("3-telephone")
        rule.onNodeWithText("Ajouter").performScrollTo().performClick()
        rule.onNodeWithText("Rejoindre avec un code").performScrollTo().performClick()
        rule.onNodeWithText("www.kybernow.app").performScrollTo().performClick()
        rule.onNodeWithText("Plus tard").performClick()
        assertEquals(listOf("PC-DU-SALON"), added)
        assertEquals(listOf(1, 1, 1), listOf(join, download, later))
    }

    @Test
    @Config(qualifiers = "fr-w960dp-h540dp-land-television-xhdpi")
    fun ecran3_televiseur() {
        show(FormFactor.TV) { WelcomeContent(WelcomeUi(2, announced), WelcomeActions()) }
        capture("3-tv")
    }

    // MARK: - Sans compte, ensuite

    private val atelier = Host(id = "a", name = "Atelier", address = "192.168.1.20", port = 47990, fingerprint = "sha256:aa", user = "m", os = "Windows 11 Pro")
    private val bureau = Host(id = "b", name = "Bureau", address = "192.168.1.21", port = 47990, fingerprint = "sha256:bb", user = "m")

    private fun home(card: Boolean = true, final: Boolean = false, banner: String? = null) = HomeState(
        listOf(atelier, bureau), "a", mapOf("a" to HostStatus(true), "b" to HostStatus(true)), emptyMap(), emptyList(), screen, decoders,
        hasAccount = false, accountCard = card, accountCardFinal = final, pairedBanner = banner,
    )

    @Test
    @Config(qualifiers = "fr-w1280dp-h800dp-land-xhdpi")
    fun sansCompte_boutonEtCarte() {
        var signIn = 0
        var create = 0
        var close = 0
        show(FormFactor.TABLET) {
            HomeContent(home(), HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {}, signIn = { signIn++ }, createAccount = { create++ }, closeAccountCard = { close++ }))
        }
        rule.onNodeWithText("COMPTE KYBERNOW").performScrollTo().assertExists()
        rule.onNodeWithText("Retrouvez vos ordinateurs partout").assertExists()
        capture("carte-tablette")
        rule.onNodeWithContentDescription("Fermer").performClick()
        rule.onNodeWithText("Créer un compte").performClick()
        // Le bouton du compte en haut, et celui de la carte.
        rule.onAllNodes(androidx.compose.ui.test.hasText("Connecter un compte")).apply { fetchSemanticsNodes().indices.forEach { get(it).performClick() } }
        assertEquals(listOf(2, 1, 1), listOf(signIn, create, close))
    }

    @Test
    fun sansCompte_telephone_iconeEtCarteRevenue() {
        show(FormFactor.PHONE) { HomeContent(home(final = true), HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {})) }
        rule.onNodeWithContentDescription("Connecter un compte").assertExists()
        rule.onNode(androidx.compose.ui.test.hasScrollToKeyAction()).performScrollToKey("compte")
        rule.onNodeWithContentDescription("Ne plus afficher").assertExists()
        capture("carte-telephone")
    }

    @Test
    fun avecUnCompte_rien() {
        show(FormFactor.PHONE) { HomeContent(home().copy(hasAccount = true, pairedBanner = "a"), HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {})) }
        rule.onNodeWithContentDescription("Connecter un compte").assertDoesNotExist()
        rule.onNodeWithText("COMPTE KYBERNOW").assertDoesNotExist()
        rule.onNodeWithText("Retrouvez-le aussi", substring = true).assertDoesNotExist()
    }

    @Test
    @Config(qualifiers = "fr-w1280dp-h800dp-land-xhdpi")
    fun apresAppairage_leBandeau() {
        var later = 0
        show(FormFactor.TABLET) { HomeContent(home(card = false, banner = "a"), HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {}, dismissPairedBanner = { later++ })) }
        rule.onNodeWithText("Retrouvez-le aussi hors de chez vous et sur vos autres appareils${n}: connectez un compte KyberNow.").assertExists()
        capture("bandeau-tablette")
        rule.onNodeWithText("Plus tard").performClick()
        assertEquals(1, later)
    }

    @Test
    @Config(qualifiers = "fr-w1280dp-h800dp-land-xhdpi")
    fun apresAppairage_attendQueLaMiseAJourDeLaStationSoitPartie() {
        val shown = mutableListOf<String>()
        val update = mapOf("a" to fr.leslignesclaires.kybernow.core.StationOffer(fr.leslignesclaires.kybernow.core.StationBanner.Update("0.12.0")))
        var state by androidx.compose.runtime.mutableStateOf(home(card = true).copy(pairedPending = "a", offers = update))
        show(FormFactor.TABLET) { HomeContent(state, HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {}, showPairedBanner = { shown += it.id })) }
        // Un seul bandeau : celui de la Station ; le bandeau du compte n'est pas compté comme vu. Le bouton et la carte restent.
        rule.onNodeWithText("Retrouvez-le aussi", substring = true).assertDoesNotExist()
        rule.onNodeWithText("COMPTE KYBERNOW").assertExists()
        rule.waitForIdle()
        assertEquals(emptyList<String>(), shown)
        capture("bandeau-maj-station-tablette")
        // « Plus tard » sur la mise à jour : le bandeau du compte a sa place.
        state = state.copy(offers = emptyMap())
        rule.waitForIdle()
        assertEquals(listOf("a"), shown)
    }

    @Test
    fun apresAppairage_telephone() {
        show(FormFactor.PHONE) { HomeContent(home(card = false, banner = "a"), HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {})) }
        capture("bandeau-telephone")
    }

    @Test
    fun horsDeChezVous() {
        var notNow = 0
        show(FormFactor.PHONE) { AwayHintContent("PC-DE-MATTHIEU", { notNow++ }, {}, {}) }
        rule.onNodeWithText("Hors de chez vous${n}?").assertExists()
        rule.onNodeWithText("PC-DE-MATTHIEU n'est pas sur ce réseau.", substring = true).assertExists()
        capture("hors-de-chez-vous-telephone")
        rule.onNodeWithText("Pas maintenant").performClick()
        assertEquals(1, notNow)
    }

    @Test
    @Config(qualifiers = "fr-w960dp-h540dp-land-television-xhdpi")
    fun horsDeChezVous_tv() {
        show(FormFactor.TV) { AwayHintContent("PC-DE-MATTHIEU", {}, {}, {}) }
        capture("hors-de-chez-vous-tv")
    }
}
