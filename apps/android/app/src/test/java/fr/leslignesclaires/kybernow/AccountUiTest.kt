package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import fr.leslignesclaires.kybernow.core.AccountSummary
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.TvCode
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.account.AccountCodeContent
import fr.leslignesclaires.kybernow.ui.account.AccountRelayContent
import fr.leslignesclaires.kybernow.ui.account.AccountsRows
import fr.leslignesclaires.kybernow.ui.account.CodeStep
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Les comptes KyberNow à l'écran (docs/link-ui.md, « Comptes KyberNow »), en français : l'accueil sans ordinateur
 * (« Retrouvez vos ordinateurs », le bouton, son aide selon la fenêtre du système ou le code, « ou sans compte » et les
 * boutons de toujours, « Connexion au compte… »), la pastille d'origine seulement quand un compte est connecté, l'écran
 * du code d'Android TV (étapes, code, QR code, attente, relais, refus et « Nouveau code »), le choix du relais (« Un
 * autre relais… ») et les comptes des réglages de l'app (« Gérer le compte », « Se déconnecter » confirmé).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class AccountUiTest {
    @org.junit.Before fun fakeCore() = FakeCore.install()

    @org.junit.After fun realCore() = FakeCore.reset()

    @get:Rule val rule = createComposeRule()

    /** Espace fine insécable (avant : ; ! ?, dans « »), espace insécable (avant min). */
    private val n = " "
    private val nb = " "

    private val screen = Screen(ScreenMode(1, 1920, 1080, 60), listOf(ScreenMode(1, 1920, 1080, 60)), false, false)
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, true)

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    private fun actions(signIn: () -> Unit = {}) = HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {}, signIn = signIn)

    // MARK: - Accueil

    @Test
    fun noComputer_offersToSignIn_thenTheUsualButtons() {
        var signIn = 0
        show(FormFactor.PHONE) { HomeContent(HomeState(emptyList(), null, emptyMap(), emptyMap(), emptyList(), screen, decoders), actions { signIn++ }) }
        rule.onNodeWithText("Retrouvez vos ordinateurs").assertExists()
        rule.onNodeWithText("Avec un compte KyberNow, vos ordinateurs et ceux qu'on vous a ouverts arrivent ici, où que vous soyez.").assertExists()
        rule.onNodeWithText("La page du relais s'ouvre", substring = true).assertExists()
        rule.onNodeWithText("Un code s'affiche", substring = true).assertDoesNotExist()
        rule.onNodeWithText("OU SANS COMPTE").assertExists()
        rule.onNodeWithText("Ajouter un ordinateur ou une console").performScrollTo().assertExists()
        rule.onNodeWithText("Rejoindre avec un code").assertExists()
        rule.onNodeWithText("Connecter un compte").performScrollTo().performClick()
        assertEquals(1, signIn)
    }

    @Test
    fun atTheTv_aCodeAppears() {
        show(FormFactor.TV) { HomeContent(HomeState(emptyList(), null, emptyMap(), emptyMap(), emptyList(), screen, decoders), actions()) }
        rule.onNodeWithText("Un code s'affiche${n}: vous l'approuvez depuis votre téléphone.").assertExists()
        rule.onNodeWithText("La page du relais s'ouvre", substring = true).assertDoesNotExist()
    }

    @Test
    fun whileTheReturnIsExchanged_signingIn() {
        var signIn = 0
        show(FormFactor.PHONE) { HomeContent(HomeState(emptyList(), null, emptyMap(), emptyMap(), emptyList(), screen, decoders, signingIn = true), actions { signIn++ }) }
        rule.onNodeWithText("Connexion au compte…").performScrollTo().performClick()
        assertEquals(0, signIn)
    }

    @Test
    fun theOriginBadge_onlyWhenAnAccountIsSignedIn() {
        val h = Host(id = "a", name = "PC du salon", address = "", port = 0, fingerprint = "sha256:aa", user = "")
        val base = HomeState(listOf(h), "a", mapOf("a" to HostStatus(true, away = true)), emptyMap(), emptyList(), screen, decoders)
        show(FormFactor.TABLET) { HomeContent(base.copy(origins = mapOf("a" to "Compte")), actions()) }
        assertTrue(rule.onAllNodesWithText("Compte").fetchSemanticsNodes().isNotEmpty())
        rule.onAllNodesWithText("Appairé").assertCountEquals(0)
    }

    @Test
    fun withoutAccount_noBadge() {
        val h = Host(id = "a", name = "PC du salon", address = "192.168.1.20", port = 47990, fingerprint = "sha256:aa", user = "m")
        show(FormFactor.TABLET) { HomeContent(HomeState(listOf(h), "a", mapOf("a" to HostStatus(true)), emptyMap(), emptyList(), screen, decoders), actions()) }
        rule.onAllNodesWithText("Compte").assertCountEquals(0)
        rule.onAllNodesWithText("Appairé").assertCountEquals(0)
    }

    // MARK: - Le code (Android TV)

    private val code = TvCode("share.kybernow.app", "KQ7M-4XPA", "share.kybernow.app/tv", "https://share.kybernow.app/compte/appareil?code=KQ7M-4XPA", 5, "{}")

    @Test
    fun theCode_itsStepsQrCodeAndWait() {
        var other = 0
        var back = 0
        show(FormFactor.TV) { AccountCodeContent(CodeStep.Showing(code, 9), "share.kybernow.app", onOtherRelay = { other++ }, onBack = { back++ }, onNewCode = {}) }
        rule.onNodeWithText("Connecter un compte KyberNow").assertExists()
        rule.onNodeWithText("Scannez le QR code avec votre téléphone,").assertExists()
        rule.onNodeWithText("ou ouvrez share.kybernow.app/tv et entrez le code,").assertExists()
        rule.onNodeWithText("puis touchez «${n}Connecter${n}».").assertExists()
        rule.onNodeWithText("KQ7M-4XPA").assertExists()
        rule.onNodeWithContentDescription("QR code de connexion").assertExists()
        rule.onNodeWithText("En attente de votre téléphone… Le code change dans 9${nb}min.").assertExists()
        rule.onNodeWithText("Relais${n}: share.kybernow.app").assertExists()
        rule.onNodeWithText("Nouveau code").assertDoesNotExist()
        rule.onNodeWithText("Un autre relais…").performClick()
        rule.onNodeWithText("Retour").performClick()
        assertEquals(1, other)
        assertEquals(1, back)
    }

    @Test
    fun declinedOnThePhone_aNewCode() {
        var again = 0
        show(FormFactor.TV) { AccountCodeContent(CodeStep.Denied, "share.kybernow.app", onOtherRelay = {}, onBack = {}, onNewCode = { again++ }) }
        rule.onNodeWithText("Connexion refusée sur le téléphone.").assertExists()
        rule.onNodeWithText("KQ7M-4XPA").assertDoesNotExist()
        rule.onNodeWithText("Nouveau code").performClick()
        assertEquals(1, again)
        rule.onNodeWithText("Un autre relais…").assertExists()
        rule.onNodeWithText("Retour").assertExists()
    }

    @Test
    fun theRelayRefuses_itsTextAndANewCode() {
        show(FormFactor.TV) { AccountCodeContent(CodeStep.Failed("Ce relais n'a pas de comptes KyberNow."), "relais.exemple.org:9443", onOtherRelay = {}, onBack = {}, onNewCode = {}) }
        rule.onNodeWithText("Ce relais n'a pas de comptes KyberNow.").assertExists()
        rule.onNodeWithText("Relais${n}: relais.exemple.org:9443").assertExists()
        rule.onNodeWithText("Nouveau code").assertExists()
    }

    // MARK: - Le relais

    @Test
    fun addAnAccount_theRelayFirst() {
        var default = 0
        var other = 0
        show(FormFactor.PHONE) { AccountRelayContent(typing = false, error = null, onDefault = { default++ }, onOther = { other++ }, onContinue = {}, onCancel = {}, onBack = {}) }
        rule.onNodeWithText("Ajouter un compte").assertExists()
        rule.onNodeWithText("share.kybernow.app").performClick()
        rule.onNodeWithText("Un autre relais…").performClick()
        assertEquals(1, default)
        assertEquals(1, other)
    }

    @Test
    fun anotherRelay_typedThenContinue() {
        val sent = mutableListOf<String>()
        var cancelled = 0
        show(FormFactor.PHONE) { AccountRelayContent(typing = true, error = "Adresse du relais illisible.", onDefault = {}, onOther = {}, onContinue = { sent += it }, onCancel = { cancelled++ }, onBack = {}) }
        rule.onNodeWithText("Un autre relais").assertExists()
        rule.onNodeWithText("L'adresse du relais de votre compte, si ce n'est pas share.kybernow.app.").assertExists()
        rule.onNodeWithText("Adresse du relais illisible.").assertExists()
        rule.onNodeWithText("Continuer").assertIsNotEnabled()
        rule.onNode(hasSetTextAction()).performTextInput("relais.exemple.org:9443")
        rule.onNodeWithText("Continuer").assertIsEnabled().performClick()
        assertEquals(listOf("relais.exemple.org:9443"), sent)
        rule.onNodeWithText("Annuler").performClick()
        assertEquals(1, cancelled)
    }

    // MARK: - Réglages de l'app › Comptes

    @Test
    fun noAccount_signIn() {
        var signIn = 0
        show(FormFactor.PHONE) { Column { AccountsRows(emptyList(), false, {}, {}, { signIn++ }, {}) } }
        rule.onNodeWithText("Aucun compte.").assertExists()
        rule.onNodeWithText("Connecter un compte").performClick()
        assertEquals(1, signIn)
        rule.onNodeWithText("Ajouter un compte").assertDoesNotExist()
        rule.onNodeWithText("Créer un compte, changer son mot de passe ou le supprimer${n}: sur la page du compte.").assertExists()
    }

    @Test
    fun anAccount_manageAndSignOutConfirmed() {
        val lea = AccountSummary("share.kybernow.app|acc_lea", "Léa Martin", "lea@exemple.fr", "share.kybernow.app", "https://share.kybernow.app/compte")
        val managed = mutableListOf<String>()
        val signedOut = mutableListOf<String>()
        var added = 0
        show(FormFactor.PHONE) { Column { AccountsRows(listOf(lea), false, { managed += it.console }, { signedOut += it.key }, {}, { added++ }) } }
        rule.onNodeWithText("Léa Martin").assertExists()
        rule.onNodeWithText("lea@exemple.fr").assertExists()
        rule.onNodeWithText("share.kybernow.app").assertExists()
        rule.onNodeWithText("Gérer le compte").performClick()
        assertEquals(listOf("https://share.kybernow.app/compte"), managed)
        rule.onNodeWithText("Se déconnecter").performClick()
        // Confirmé d'abord.
        assertTrue(signedOut.isEmpty())
        rule.onNodeWithText("Se déconnecter de Léa Martin${n}? Ses ordinateurs quittent cet appareil${n}; ceux que vous avez appairés restent.").assertExists()
        rule.onNodeWithText("Se déconnecter").performClick()
        assertEquals(listOf("share.kybernow.app|acc_lea"), signedOut)
        rule.onNodeWithText("Ajouter un compte").performClick()
        assertEquals(1, added)
    }
}
