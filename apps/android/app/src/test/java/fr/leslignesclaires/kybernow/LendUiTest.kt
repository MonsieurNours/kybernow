package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.LendAvailability
import fr.leslignesclaires.kybernow.core.LendAvailable
import fr.leslignesclaires.kybernow.core.LendBorrower
import fr.leslignesclaires.kybernow.core.LendCodeView
import fr.leslignesclaires.kybernow.core.LendDuration
import fr.leslignesclaires.kybernow.core.LendEnd
import fr.leslignesclaires.kybernow.core.LendHostState
import fr.leslignesclaires.kybernow.core.LendKind
import fr.leslignesclaires.kybernow.core.LendNotice
import fr.leslignesclaires.kybernow.core.LendNoticeReason
import fr.leslignesclaires.kybernow.core.LendPeriod
import fr.leslignesclaires.kybernow.core.LendRequest
import fr.leslignesclaires.kybernow.core.LendSlot
import fr.leslignesclaires.kybernow.core.LendStatus
import fr.leslignesclaires.kybernow.core.LendSteam
import fr.leslignesclaires.kybernow.core.LendView
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareRelay
import fr.leslignesclaires.kybernow.core.ShareVia
import fr.leslignesclaires.kybernow.model.BorrowedComputer
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.ComeBack
import fr.leslignesclaires.kybernow.session.Ended
import fr.leslignesclaires.kybernow.session.LendPanelContent
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.session.lendUnavailableText
import fr.leslignesclaires.kybernow.ui.BorrowStep
import fr.leslignesclaires.kybernow.ui.FriendStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.home.FriendHero
import fr.leslignesclaires.kybernow.ui.home.LentLine
import fr.leslignesclaires.kybernow.ui.home.TakeBackButton
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
 * Le prêt de l'ordinateur, l'interface (docs/pret.md ; textes de docs/link-ui.md, « Prêt de l'ordinateur (KyberNow
 * Share) »), en français : le panneau « Prêter {nom} » de l'hôte (durée, compte Steam, code, demandes, raisons), « Prêter
 * l'ordinateur… » dans la barre et au téléviseur, la barre et le menu d'un emprunt (« Prêté par {hôte} », le temps,
 * « Rendre l'ordinateur » et sa confirmation), la carte de fin (« Revenir »), la carte « Ordinateur de {hôte} » de
 * l'accueil, « Prêté à {prénom} » et « Reprendre l'ordinateur », l'attente de l'hôte après un code de prêt.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class LendUiTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()
    private val nbsp = " "

    private class Actions : SessionActions {
        val calls = mutableListOf<String>()
        override fun resume() {}
        override fun quit() { calls += "quitter" }
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
        override fun share() { calls += "partager" }
        override fun shareNewCode() {}
        override fun shareSetPads(guest: ShareGuest, on: Boolean) {}
        override fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean) {}
        override fun shareAccept(guest: ShareGuest) {}
        override fun shareRefuse(guest: ShareGuest) {}
        override fun shareRemove(guest: ShareGuest) {}
        override fun shareStop() {}
        override fun shareCopyLink() {}
        override fun setShareQuality(quality: ShareQuality) {}
        override fun giveBack() { calls += "rendre" }
        override fun comeBack() { calls += "revenir" }
        override fun lend() { calls += "prêter" }
        override fun lendCreateCode() { calls += "créer" }
        override fun lendCancelCode() { calls += "annuler le code" }
        override fun lendAnswer(request: LendRequest, accept: Boolean) { calls += "${request.name} ${if (accept) "accepté" else "refusé"}" }
        override fun lendCopyLink() { calls += "copier" }
    }

    private val actions = Actions()

    /** L'espace fine insécable de la typographie française, avant : ; ! ? */
    private val thin = " "

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    private fun host(tv: Boolean = false, setup: SessionUi.() -> Unit = {}): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        return SessionUi("Atelier", tv, state, actions, computerName = "Atelier").apply { passageDone = true; setup() }
    }

    private fun borrower(tv: Boolean = false, until: Long? = System.currentTimeMillis() / 1000 + 4800, setup: SessionUi.() -> Unit = {}): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        return SessionUi("Ordinateur de Matthieu", tv, state, actions, computerName = "PC-SALON", borrower = true, lender = "Matthieu", lendUntil = until).apply { passageDone = true; setup() }
    }

    private fun code() = LendCodeView("123456", "123 456", "kybernow://join?code=123456", "https://192.168.1.20:41234/link/?join=123456", LendDuration.TWO_HOURS, LendSteam.FRIEND, null)

    private fun request(id: String, name: String, via: ShareVia = ShareVia.LAN, slot: Boolean = false, steam: LendSteam = LendSteam.FRIEND) =
        LendRequest(id, name, LendKind.FRIEND, "ios", via, 100, steam, slot)

    private fun panel(view: LendView?, unavailable: String? = null, busy: Boolean = false, warning: Boolean = false, tv: Boolean = false, onDuration: (LendDuration) -> Unit = {}, onSteam: (LendSteam) -> Unit = {}) =
        show(if (tv) FormFactor.TV else FormFactor.PHONE) {
            LendPanelContent("Atelier", view, unavailable, busy, null, null, tv, LendDuration.TWO_HOURS, LendSteam.FRIEND, warning, actions, onDuration, onSteam, {})
        }

    // MARK: - Le panneau de l'hôte

    @Test
    fun withoutCode_thePanelOffersDurationSteamAndCreate() {
        val durations = mutableListOf<LendDuration>()
        val steams = mutableListOf<LendSteam>()
        panel(LendView(true, LendHostState.FREE), warning = true, onDuration = { durations += it }, onSteam = { steams += it })
        rule.onNodeWithText("Prêter Atelier").assertExists()
        rule.onNodeWithText("Durée", ignoreCase = true).assertExists()
        listOf("1 heure", "2 heures", "4 heures").forEach { rule.onNodeWithText(it).assertHasClickAction() }
        rule.onNodeWithText("Jusqu'à ce que je reprenne").performClick()
        assertEquals(listOf(LendDuration.OPEN), durations)
        rule.onNodeWithText("Compte Steam", ignoreCase = true).assertExists()
        rule.onNodeWithText("Son compte Steam").assertExists()
        rule.onNodeWithText("Votre compte Steam").performClick()
        assertEquals(listOf(LendSteam.HOST), steams)
        // L'avertissement, montré jusqu'au premier code.
        rule.onNodeWithText("Votre ami joue dans votre session Windows", substring = true).assertExists()
        rule.onNodeWithText("Créer un code").assertIsEnabled().performClick()
        assertEquals(listOf("créer"), actions.calls)
    }

    @Test
    fun withACode_itIsShownLikeASharedCode_withItsHelpAndCancel() {
        panel(LendView(true, LendHostState.FREE, code = code()))
        rule.onNodeWithText("123 456").assertExists()
        rule.onNodeWithContentDescription("QR code du prêt").assertExists()
        rule.onNodeWithText("Pour une séance, valable 24${nbsp}h.").assertExists()
        rule.onNodeWithText("Votre ami entre ce code avec", substring = true).assertExists()
        rule.onNodeWithText("Votre session se fermera quand votre ami arrivera.").assertExists()
        rule.onNodeWithText("Créer un code").assertDoesNotExist()
        rule.onNodeWithText("Copier le lien").performClick()
        rule.onNodeWithText("Annuler le code").performClick()
        assertEquals(listOf("copier", "annuler le code"), actions.calls)
        // Plus d'avertissement une fois un code créé.
        rule.onNodeWithText("Votre ami joue dans votre session Windows", substring = true).assertDoesNotExist()
    }

    @Test
    fun whileTheRelayLinkOfTheCodeIsPrepared_copyAndTheQrCodeWait_likeTheShare() {
        panel(LendView(true, LendHostState.FREE, code = code().copy(code = "555001", relay = ShareRelay.CONNECTING)))
        rule.onNodeWithText("Préparation du lien hors de chez vous…").assertExists()
        rule.onNodeWithText("Pas encore joignable hors de chez vous.").assertDoesNotExist()
        rule.onNodeWithText("Copier le lien").assertIsNotEnabled()
        // Le QR code laisse la place à l'indicateur.
        rule.onNodeWithContentDescription("QR code du prêt").assertDoesNotExist()
        rule.onNodeWithContentDescription("Préparation du lien hors de chez vous…").assertExists()
    }

    @Test
    fun afterTenSeconds_theRelayLinkOfTheCode_isUnreachable_andTheNetworkLinkIsGiven() {
        rule.mainClock.autoAdvance = false
        panel(LendView(true, LendHostState.FREE, code = code().copy(code = "555002", relay = ShareRelay.CONNECTING)))
        rule.mainClock.advanceTimeBy(1_000)
        rule.onNodeWithText("Préparation du lien hors de chez vous…").assertExists()
        // Sans nouvel état de la Station, le panneau change de lui-même au bout des 10 s.
        rule.mainClock.advanceTimeBy(10_000)
        rule.onNodeWithText("Préparation du lien hors de chez vous…").assertDoesNotExist()
        rule.onNodeWithText("Pas encore joignable hors de chez vous.").assertExists()
        rule.onNodeWithText("Copier le lien").assertIsEnabled()
        rule.onNodeWithContentDescription("QR code du prêt").assertExists()
    }

    @Test
    fun withTheRelayJoined_theCodeSaysNothingMore() {
        panel(LendView(true, LendHostState.FREE, code = code().copy(relay = ShareRelay.JOINED)))
        rule.onNodeWithText("Préparation du lien hors de chez vous…").assertDoesNotExist()
        rule.onNodeWithText("Pas encore joignable hors de chez vous.").assertDoesNotExist()
        rule.onNodeWithText("Copier le lien").assertIsEnabled()
    }

    @Test
    fun whenLendingIsNotPossibleOnThisComputer_thePanelSaysSo() {
        show(FormFactor.PHONE) {
            LendPanelContent("Atelier", null, lendUnavailableText(LendAvailable.UNAVAILABLE), false, null, null, false, LendDuration.TWO_HOURS, LendSteam.FRIEND, false, actions, {}, {}, {})
        }
        rule.onNodeWithText("Le prêt n'est pas possible sur cet ordinateur.").assertExists()
        rule.onNodeWithText("Créer un code").assertDoesNotExist()
    }

    @Test
    fun onATelevision_thereIsNoCopy() {
        panel(LendView(true, LendHostState.FREE, code = code()), tv = true)
        rule.onNodeWithText("Copier le lien").assertDoesNotExist()
        rule.onNodeWithText("Annuler le code").assertHasClickAction()
    }

    @Test
    fun theRequests_comeFirst_withAcceptAndDecline() {
        panel(LendView(true, LendHostState.FREE, requests = listOf(request("r1", "Léa", via = ShareVia.RELAY))))
        rule.onNodeWithText("Léa veut emprunter l'ordinateur.").assertExists()
        rule.onNodeWithText("iPhone ou iPad · hors de chez vous").assertExists()
        rule.onNodeWithText("Votre session va se fermer.").assertExists()
        rule.onNodeWithText("Accepter").performClick()
        rule.onNodeWithText("Refuser").performClick()
        assertEquals(listOf("Léa accepté", "Léa refusé"), actions.calls)
    }

    @Test
    fun aSlotRequest_saysTheSlot_andCannotBeDeclined() {
        panel(LendView(true, LendHostState.FREE, requests = listOf(request("r1", "Léa", slot = true))))
        rule.onNodeWithText("Créneau de Léa$thin: l'ordinateur lui est prêté dans 2${nbsp}min.").assertExists()
        rule.onNodeWithText("Accepter").assertHasClickAction()
        rule.onNodeWithText("Refuser").assertDoesNotExist()
    }

    @Test
    fun whileACallRuns_theButtonsWait() {
        panel(LendView(true, LendHostState.FREE, requests = listOf(request("r1", "Léa"))), busy = true)
        rule.onNodeWithText("Accepter").assertIsNotEnabled()
        rule.onNodeWithText("Créer un code").assertIsNotEnabled()
    }

    @Test
    fun whenLendingIsImpossible_thePanelIsTheReasonOnly() {
        panel(null, unavailable = "Le prêt est coupé dans Admin (page «${" "}Prêt${" "}»).")
        rule.onNodeWithText("Le prêt est coupé dans Admin", substring = true).assertExists()
        rule.onNodeWithText("Créer un code").assertDoesNotExist()
        rule.onNodeWithText("Fermer").assertHasClickAction()
    }

    @Test
    fun theBar_hasLendTheComputer_andARequestsPastille() {
        val ui = host { barOpen = true }
        ui.state.lend.value = LendView(true, LendHostState.FREE, requests = listOf(request("r1", "Léa"), request("r2", "Tom")))
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Prêter l'ordinateur…").performClick()
        rule.onNodeWithText("2 demandes").performClick()
        assertEquals(listOf("prêter", "prêter"), actions.calls)
    }

    @Test
    fun theTelevisionMenu_hasALendTile_withTheCode() {
        val ui = host(tv = true) { menuOpen = true }
        ui.state.lend.value = LendView(true, LendHostState.FREE, code = code())
        show(FormFactor.TV) { SessionOverlay(ui) }
        rule.onNodeWithText("Prêter l'ordinateur…").assertHasClickAction()
        rule.onNodeWithText("123 456").assertExists()
    }

    @Test
    fun aLendRequest_isAnnouncedInACapsule_thatOpensThePanel() {
        val ui = host()
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        ui.state.capsule.value = Capsule.LendRequest("Léa")
        rule.onNodeWithText("Léa veut emprunter l'ordinateur.").performClick()
        assertEquals(listOf("prêter"), actions.calls)
    }

    // MARK: - La session d'emprunt

    @Test
    fun theBorrowerBar_saysWhoLendsAndTheTimeLeft_withoutProfilesNorSharing() {
        show(FormFactor.PHONE) { SessionOverlay(borrower { barOpen = true }) }
        rule.onNodeWithText("Prêté par Matthieu · Encore 1${nbsp}h${nbsp}20").assertExists()
        rule.onNodeWithText("Rendre l'ordinateur").assertHasClickAction()
        rule.onNodeWithText("Se déconnecter").assertHasClickAction()
        rule.onNodeWithText("Jeu").assertDoesNotExist()
        rule.onNodeWithText("Partager").assertDoesNotExist()
        rule.onNodeWithText("Prêter l'ordinateur…").assertDoesNotExist()
    }

    @Test
    fun withoutEnd_theBarSaysUntilTheHostTakesItBack() {
        show(FormFactor.PHONE) { SessionOverlay(borrower(until = null) { barOpen = true }) }
        rule.onNodeWithText("Prêté par Matthieu · Jusqu'à ce que Matthieu reprenne").assertExists()
    }

    @Test
    fun duringTheNotice_theTimeLeftIsTheNotice() {
        val ui = borrower(until = null) { barOpen = true }
        ui.state.lendNotice.value = LendNotice(LendNoticeReason.TAKEN_BACK, System.currentTimeMillis() / 1000 + 120)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Prêté par Matthieu · Encore 2${nbsp}min").assertExists()
    }

    @Test
    fun givingBack_asksForConfirmation() {
        val ui = borrower { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Rendre l'ordinateur").performClick()
        rule.onNodeWithText("Rendre l'ordinateur à Matthieu$thin?").assertExists()
        rule.onNodeWithText("Annuler").performClick()
        assertTrue(actions.calls.isEmpty())
        rule.onNodeWithText("Rendre l'ordinateur").performClick()
        rule.onNodeWithText("Rendre l'ordinateur à Matthieu$thin?").assertExists()
        // Deux « Rendre l'ordinateur » : la barre et la confirmation (dessinée par-dessus) ; celui de la confirmation rend.
        rule.onAllNodesWithText("Rendre l'ordinateur").onLast().performClick()
        assertEquals(listOf("rendre"), actions.calls)
    }

    @Test
    fun theBorrowerTelevisionMenu_hasTheLoanAndGiveBack() {
        show(FormFactor.TV) { SessionOverlay(borrower(tv = true) { menuOpen = true }) }
        rule.onNodeWithText("Ordinateur de Matthieu · Prêté par Matthieu", ignoreCase = true).assertExists()
        rule.onNodeWithText("Encore 1${nbsp}h${nbsp}20").assertExists()
        rule.onNodeWithText("Rendre l'ordinateur").assertHasClickAction()
        rule.onNodeWithText("Se déconnecter").assertHasClickAction()
        rule.onNodeWithText("Partager").assertDoesNotExist()
    }

    @Test
    fun thePassage_preparesTheComputerFirst() {
        val ui = borrower { passageDone = false }
        ui.state.phase.value = Phase.PREPARING
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Préparation de l'ordinateur").assertExists()
    }

    @Test
    fun theEndOfTheLoan_isItsReason_withClose() {
        val ui = borrower()
        ui.state.lendEnd = LendEnd.TAKEN_BACK
        ui.state.ended.value = Ended("Matthieu a repris l'ordinateur.", null, null, null)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Matthieu a repris l'ordinateur.").assertExists()
        rule.onNodeWithText("Revenir").assertDoesNotExist()
        rule.onNodeWithText("Fermer").performClick()
        assertEquals(listOf("quitter"), actions.calls)
    }

    @Test
    fun theEndCard_isSignedKyberNowShare() {
        val ui = borrower()
        ui.state.lendEnd = LendEnd.TIME_UP
        ui.state.ended.value = Ended("Le prêt est terminé.", null, null, null)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        // Le surtitre s'écrit en capitales.
        rule.onNodeWithText("KYBERNOW SHARE").assertExists()
    }

    @Test
    fun givingBack_isSaidOnTheEndCard_withClose() {
        val ui = borrower()
        ui.state.lendEnd = LendEnd.LEFT
        ui.state.ended.value = Ended("Vous avez rendu l'ordinateur.", null, null, null)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Vous avez rendu l'ordinateur.").assertExists()
        rule.onNodeWithText("Vous pouvez revenir pendant 15${nbsp}min.").assertDoesNotExist()
        rule.onNodeWithText("Fermer").performClick()
        assertEquals(listOf("quitter"), actions.calls)
    }

    @Test
    fun aLostSession_canComeBackFor15Minutes() {
        val ui = borrower()
        ui.state.ended.value = Ended("l'ordinateur prêté ne répond plus", null, null, null)
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("L'ordinateur prêté ne répond plus.").assertExists()
        rule.onNodeWithText("Vous pouvez revenir pendant 15${nbsp}min.").assertExists()
        rule.onNodeWithText("Revenir").performClick()
        assertEquals(listOf("revenir"), actions.calls)
        ui.comeBack = ComeBack(ComeBack.Step.WAITING)
        rule.onNodeWithText("En attente de Matthieu…").assertExists()
        ui.comeBack = ComeBack(ComeBack.Step.FAILED, "Matthieu a refusé.")
        rule.onNodeWithText("Matthieu a refusé.").assertExists()
        rule.onNodeWithText("Revenir").assertHasClickAction()
    }

    // MARK: - L'accueil

    private val friend = BorrowedComputer(id = "pret-1", host = "Matthieu", computer = "PC-SALON", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab", login = "lea")

    private fun hero(status: FriendStatus?, known: Boolean = true, step: BorrowStep? = null, error: String? = null, borrow: (String?) -> Unit = {}, cancel: () -> Unit = {}) =
        show(FormFactor.PHONE) { FriendHero(friend, status, known, step, error, 40f, null, borrow, cancel, {}) }

    @Test
    fun theFriendCard_offersBorrow_whenFree() {
        val asked = mutableListOf<String?>()
        hero(FriendStatus(LendStatus(LendAvailability.FREE, "Matthieu", "PC-SALON")), borrow = { asked += it })
        rule.onNodeWithText("Ordinateur de Matthieu").assertExists()
        rule.onNodeWithText("Disponible", ignoreCase = true).assertExists()
        rule.onNodeWithText("PC-SALON").assertExists()
        rule.onNodeWithText("Emprunter").assertIsEnabled().performClick()
        // Le mot de passe est connu : rien n'est tapé.
        assertEquals(listOf<String?>(null), asked)
    }

    @Test
    fun theFriendCard_asksTheHost_whenInUse() {
        hero(FriendStatus(LendStatus(LendAvailability.BUSY, "Matthieu", "PC-SALON")))
        rule.onNodeWithText("Occupé", ignoreCase = true).assertExists()
        rule.onNodeWithText("Demander à Matthieu").assertIsEnabled()
    }

    @Test
    fun theFriendCard_isOff_whenLentLockedOrOffline() {
        hero(FriendStatus(LendStatus(LendAvailability.LOCKED, "Matthieu", "PC-SALON")))
        rule.onNodeWithText("Verrouillé", ignoreCase = true).assertExists()
        rule.onNodeWithText("Emprunter").assertIsNotEnabled()
    }

    @Test
    fun theFriendCard_offline_saysSo() {
        hero(FriendStatus(offline = true))
        rule.onNodeWithText("Hors ligne", ignoreCase = true).assertExists()
        rule.onNodeWithText("Emprunter").assertIsNotEnabled()
    }

    @Test
    fun theFriendCard_ownLoan_canBeBorrowedAgain() {
        hero(FriendStatus(LendStatus(LendAvailability.LENT, "Matthieu", "PC-SALON", mine = true)))
        rule.onNodeWithText("Prêté à quelqu'un d'autre", ignoreCase = true).assertDoesNotExist()
        rule.onNodeWithText("Emprunter").assertIsEnabled()
    }

    @Test
    fun theFriendCard_saysItsSlot() {
        // Mercredi 28 octobre 2026, de 14 h à 18 h (heure de Paris), avec le fuseau de l'appareil des essais.
        val zone = java.util.TimeZone.getDefault()
        java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone("Europe/Paris"))
        try {
            hero(FriendStatus(LendStatus(LendAvailability.FREE, "Matthieu", "PC-SALON", LendSlot(false, LendPeriod(1_793_206_800L - 4 * 3600, 1_793_206_800L)))))
            rule.onNodeWithText("Votre créneau$thin: mercredi, 14${nbsp}h – 18${nbsp}h").assertExists()
        } finally {
            java.util.TimeZone.setDefault(zone)
        }
    }

    @Test
    fun withoutAKnownPassword_itIsTypedFirst() {
        val asked = mutableListOf<String?>()
        hero(null, known = false, borrow = { asked += it })
        rule.onNodeWithText("Emprunter").assertIsNotEnabled()
        rule.onNodeWithText("Mot de passe").performTextInput("secret")
        rule.onNodeWithText("Emprunter").assertIsEnabled().performClick()
        assertEquals(listOf<String?>("secret"), asked)
    }

    @Test
    fun withoutAKnownPassword_theCardAsksForIt() {
        hero(null, known = false)
        rule.onNodeWithText("Entrez votre mot de passe pour emprunter.").assertExists()
    }

    @Test
    fun withAKnownPassword_theCardDoesNotAskForIt() {
        hero(FriendStatus(LendStatus(LendAvailability.FREE, "Matthieu", "PC-SALON")))
        rule.onNodeWithText("Entrez votre mot de passe pour emprunter.").assertDoesNotExist()
    }

    @Test
    fun removingAFriendsComputer_asksWithTheHostsName() {
        hero(FriendStatus(LendStatus(LendAvailability.FREE, "Matthieu", "PC-SALON")))
        rule.onNodeWithText("Retirer cet ordinateur").performClick()
        rule.onNodeWithText("Retirer l'ordinateur de Matthieu de cet appareil$thin?").assertExists()
        rule.onNodeWithText("Retirer").assertHasClickAction()
    }

    @Test
    fun removingAFriendsComputer_elidesBeforeAVowel() {
        hero(FriendStatus(LendStatus(LendAvailability.FREE, "Hugo", "PC-SALON")))
        rule.onNodeWithText("Retirer cet ordinateur").performClick()
        rule.onNodeWithText("Retirer l'ordinateur d'Hugo de cet appareil$thin?").assertExists()
    }

    @Test
    fun whileBorrowing_theCardPreparesThenWaits() {
        var cancelled = 0
        hero(FriendStatus(LendStatus(LendAvailability.BUSY, "Matthieu", "PC-SALON")), step = BorrowStep.Waiting("Matthieu"), cancel = { cancelled++ })
        rule.onNodeWithText("En attente de Matthieu…").assertExists()
        rule.onNodeWithText("Annuler").performClick()
        assertEquals(1, cancelled)
    }

    @Test
    fun whilePreparing_theButtonSaysSo() {
        hero(FriendStatus(LendStatus(LendAvailability.FREE, "Matthieu", "PC-SALON")), step = BorrowStep.Preparing)
        rule.onNodeWithText("Préparation de l'ordinateur").assertExists()
        rule.onNodeWithText("Emprunter").assertDoesNotExist()
    }

    @Test
    fun aRefusal_isShownUnderTheCard() {
        hero(FriendStatus(LendStatus(LendAvailability.BUSY, "Matthieu", "PC-SALON")), error = "Matthieu a refusé.")
        rule.onNodeWithText("Matthieu a refusé.").assertExists()
    }

    @Test
    fun theHostHome_saysLentTo_andTakesBack() {
        val taken = mutableListOf<Boolean>()
        val view = LendView(true, LendHostState.LENT, LendBorrower("Léa", LendKind.CODE, "ios", ShareVia.LAN, 1, null, LendSteam.FRIEND, true))
        show(FormFactor.PHONE) {
            androidx.compose.foundation.layout.Column {
                LentLine(view)
                TakeBackButton(view, { taken += it }, androidx.compose.ui.Modifier)
            }
        }
        rule.onNodeWithText("Prêté à Léa").assertExists()
        rule.onNodeWithText("Reprendre l'ordinateur").performClick()
        assertEquals(listOf(false), taken)
    }

    @Test
    fun duringTheNotice_theHostCanTakeBackRightNow() {
        val taken = mutableListOf<Boolean>()
        val at = System.currentTimeMillis() / 1000 + 102
        val view = LendView(true, LendHostState.ENDING, LendBorrower("Léa", LendKind.CODE, "ios", ShareVia.LAN, 1, null, LendSteam.FRIEND, true), LendNotice(LendNoticeReason.TAKEN_BACK, at))
        show(FormFactor.PHONE) {
            androidx.compose.foundation.layout.Column {
                LentLine(view)
                TakeBackButton(view, { taken += it }, androidx.compose.ui.Modifier)
            }
        }
        rule.onNodeWithText("Reprise dans", substring = true).assertExists()
        rule.onNodeWithText("Reprendre tout de suite").performClick()
        assertEquals(listOf(true), taken)
    }

    // MARK: - Rejoindre avec un code de prêt

    @Test
    fun aLendCode_waitsForTheHostByName() {
        var cancelled = 0
        show(FormFactor.PHONE) { JoinContent(null, false, "Léa", ShareQuality.AUTO, true, null, {}, { _, _, _, _ -> }, waiting = true, onCancel = { cancelled++ }, waitingFor = "Matthieu") }
        rule.onNodeWithText("En attente de Matthieu…").assertExists()
        rule.onNodeWithText("Annuler").performClick()
        assertEquals(1, cancelled)
    }

    @Test
    fun aLendCode_preparesTheComputer() {
        show(FormFactor.PHONE) { JoinContent(null, false, "Léa", ShareQuality.AUTO, true, null, {}, { _, _, _, _ -> }, preparing = true) }
        rule.onNodeWithText("Préparation de l'ordinateur").assertExists()
    }
}
