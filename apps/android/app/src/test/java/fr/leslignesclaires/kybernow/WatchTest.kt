package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.longClick
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.AccountComputer
import fr.leslignesclaires.kybernow.core.Accounts
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.WatchReply
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostAccount
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.session.WatchUi
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.RelayRefusal
import fr.leslignesclaires.kybernow.ui.WatchRefusal
import fr.leslignesclaires.kybernow.ui.watchRefusal
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
import org.json.JSONObject
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
 * « Regarder » (droit `view` d'un compte KyberNow ; docs/link-ui.md, « Comptes KyberNow », lignes « Regarder… ») : les
 * réponses du cœur rangées (les écrans, les refus 34 à 36 avec leur texte), les droits de la carte, « Regarder » à la
 * place de « Se connecter » ou dans le menu de l'ordinateur, la barre et le menu du téléviseur de qui regarde
 * (« Arrêter de regarder », « Écran » et sa liste). Sans bibliothèque native : les mots du cœur sont donnés tels qu'il
 * les écrit en français.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class WatchTest {
    @get:Rule val rule = createComposeRule()

    private val context get() = ApplicationProvider.getApplicationContext<Application>()

    // MARK: - Les réponses du cœur

    private val watched = """{"target":{"host":"192.168.1.20","port":47990,"fingerprint":"sha256:ad"},"joined":{"guest":"g1","secret":"S","role":"spectator","host":"Matthieu","computer":"PC du salon","display":{"id":2,"width":3840,"height":2160,"refresh":60},
        "watch":{"screens":[{"id":2,"index":1,"width":3840,"height":2160,"refresh":60,"primary":true},{"id":5,"index":2,"width":1920,"height":1080,"refresh":144,"primary":false}],"switch":true,"display":2}}}"""

    @Test
    fun watching_entersAsAGuest_withTheScreensOfThePc() {
        val reply = Share.readWatch("""{"code":0,"value":$watched}""")
        reply as WatchReply.Joined
        assertEquals("PC du salon", reply.found.computer)
        assertEquals(ShareRole.SPECTATOR, reply.found.role)
        val w = reply.found.watch!!
        assertEquals(listOf(2, 5), w.screens.map { it.id })
        assertTrue(w.screens.first().primary)
        assertEquals(2, w.display)
        assertTrue("plusieurs écrans qu'on peut changer : « Écran »", w.menu)
        // Le JSON `joined` part tel quel à la session d'invité, `watch` compris.
        assertTrue(reply.found.joined.has("watch"))
    }

    @Test
    fun theScreenMenu_onlyWhenTheStationAllowsIt_andThereAreSeveral() {
        val fixed = JSONObject(watched).apply { getJSONObject("joined").getJSONObject("watch").put("switch", false) }
        assertFalse((Share.readWatch("""{"code":0,"value":$fixed}""") as WatchReply.Joined).found.watch!!.menu)
        val one = JSONObject(watched).apply { getJSONObject("joined").getJSONObject("watch").getJSONArray("screens").remove(1) }
        assertFalse((Share.readWatch("""{"code":0,"value":$one}""") as WatchReply.Joined).found.watch!!.menu)
        // Un invité venu par un code n'a pas de `watch`.
        val code = JSONObject(watched).apply { getJSONObject("joined").remove("watch") }
        assertNull((Share.readWatch("""{"code":0,"value":$code}""") as WatchReply.Joined).found.watch)
    }

    private fun refused(code: Int, kind: String?, text: String) =
        Share.readWatch(JSONObject().put("code", code).put("error", text).apply { if (kind != null) put("kind", kind) }.toString()) as WatchReply.Refused

    @Test
    fun refusals_keepTheCodeTheKindAndTheTextOfTheCore() {
        assertEquals(WatchReply.Refused(34, "watch_unsupported", "Cette Station ne permet pas encore de regarder : mettez-la à jour."), refused(34, "watch_unsupported", "Cette Station ne permet pas encore de regarder : mettez-la à jour."))
        assertEquals(WatchReply.Refused(35, "lent", "PC du salon est prêté en ce moment."), refused(35, "lent", "PC du salon est prêté en ce moment."))
        assertEquals(WatchReply.Refused(36, "disabled", "Le partage est coupé sur cet ordinateur (Admin › Appareils Link)."), refused(36, "disabled", "Le partage est coupé sur cet ordinateur (Admin › Appareils Link)."))
        // Le reste : le texte d'Admin, tel quel.
        assertEquals(WatchReply.Refused(1, "other", "Les comptes sont coupés sur cet ordinateur."), refused(1, "other", "Les comptes sont coupés sur cet ordinateur."))
        // Les refus des comptes et du relais passent par l'enveloppe d'Admin : leur code, sans `kind`.
        assertEquals(70, refused(70, null, "Ce compte a été déconnecté de cet appareil : reconnectez-vous.").code)
        assertEquals("relay", refused(31, null, "").kind)
        assertEquals(WatchReply.Refused(1, "other", ""), Share.readWatch("pas du JSON"))
        assertEquals(WatchReply.Refused(1, "other", ""), Share.readWatch(null))
    }

    @Test
    fun refusals_ofTheAccountAndTheRelay_asForConnect() {
        // 70 : le compte est oublié ; 71 : l'ordinateur quitte l'accueil (jamais d'après le texte).
        assertEquals(WatchRefusal.SignedOut, watchRefusal(refused(70, null, "Ce compte a été déconnecté de cet appareil : reconnectez-vous.")))
        assertEquals(WatchRefusal.ComputerGone, watchRefusal(refused(71, null, "Cet ordinateur n'est plus sur votre compte.")))
        // Le relais : les phrases de l'accueil avec le nom de l'ordinateur (31, 32), son texte pour 30.
        assertEquals(WatchRefusal.Show(RelayRefusal.StationOffline), watchRefusal(refused(31, null, "x")))
        assertEquals(WatchRefusal.Show(RelayRefusal.AccessOff), watchRefusal(refused(32, null, "x")))
        assertEquals(WatchRefusal.Show(RelayRefusal.Message("Le relais ne répond pas.")), watchRefusal(refused(30, null, "Le relais ne répond pas.")))
        // Ceux de « Regarder » et d'Admin : le texte du cœur, tel quel.
        assertEquals(WatchRefusal.Show(RelayRefusal.Message("PC du salon est prêté en ce moment.")), watchRefusal(refused(35, "lent", "PC du salon est prêté en ce moment.")))
        assertEquals(WatchRefusal.Show(RelayRefusal.Message("Les comptes sont coupés sur cet ordinateur.")), watchRefusal(refused(1, "other", "Les comptes sont coupés sur cet ordinateur.")))
    }

    @Test
    fun theRequest_isThatOfTheAndroidApp_withTheQuality() {
        val r = Share.watchRequest(ShareQuality.P1080_60, JSONObject().put("width", 1920))
        assertEquals("android", r.getString("app"))
        assertEquals(ShareQuality.P1080_60.id, r.getString("quality"))
        assertEquals(1920, r.getJSONObject("auto").getInt("width"))
        // Sans `display` : l'écran principal au départ.
        assertFalse(r.has("display"))
        assertFalse(Share.watchRequest(ShareQuality.AUTO, null).has("auto"))
    }

    @Test
    fun theRights_ofTheCard() {
        fun c(vararg rights: String) = AccountComputer("relais|a", "share.kybernow.app", "st-1", "PC du salon", rights.toList(), json = "{}")
        assertTrue(Accounts.canWatch(c("view")))
        assertTrue("« Regarder » seul : à la place de « Se connecter »", Accounts.watchOnly(c("view")))
        assertTrue(Accounts.canWatch(c("session", "view")))
        assertFalse("avec « Ouvrir le bureau » : dans le menu", Accounts.watchOnly(c("session", "view")))
        assertFalse(Accounts.canWatch(c("session")))
    }

    // MARK: - L'accueil

    private val screen = Screen(ScreenMode(1, 1920, 1080, 60), listOf(ScreenMode(1, 1920, 1080, 60)), false, false)
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, true)
    private val pc = Host(id = "a", name = "PC du salon", address = "", port = 0, fingerprint = "sha256:aa", user = "", account = HostAccount("relais|a", "st-1"))

    private fun home(watchOnly: Boolean?, form: FormFactor = FormFactor.TABLET, watches: MutableList<String>, connects: MutableList<String>) {
        val state = HomeState(
            listOf(pc), "a", mapOf("a" to HostStatus(true, away = true)), emptyMap(), emptyList(), screen, decoders,
            watch = if (watchOnly == null) emptyMap() else mapOf("a" to watchOnly), watchWord = "Regarder",
        )
        val actions = HomeActions({}, { _, _ -> }, { connects += it.id }, {}, {}, {}, {}, {}, {}, {}, watch = { watches += it.id })
        rule.setContent { KnTheme(form, motion = false) { HomeContent(state, actions) } }
    }

    @Test
    fun watchOnly_theCardOffersWatch_insteadOfConnect() {
        val watches = mutableListOf<String>()
        val connects = mutableListOf<String>()
        home(true, watches = watches, connects = connects)
        rule.onAllNodesWithText("Se connecter").assertCountEquals(0)
        rule.onNodeWithText("Regarder").performClick()
        assertEquals(listOf("a"), watches)
        assertTrue(connects.isEmpty())
    }

    @Test
    fun watchOnly_onATelevision_too() {
        val watches = mutableListOf<String>()
        home(true, FormFactor.TV, watches, mutableListOf())
        rule.onAllNodesWithText("Se connecter").assertCountEquals(0)
        rule.onNodeWithText("Regarder").performClick()
        assertEquals(listOf("a"), watches)
    }

    @Test
    fun bothRights_theCardKeepsConnect_andWatchIsInTheComputersMenu() {
        val watches = mutableListOf<String>()
        val connects = mutableListOf<String>()
        home(false, watches = watches, connects = connects)
        rule.onNodeWithText("Se connecter").assertExists()
        rule.onAllNodesWithText("Regarder").assertCountEquals(0)
        // Le menu de l'ordinateur : un appui long sur sa carte.
        openMenu()
        rule.onNodeWithText("Regarder").performClick()
        assertEquals(listOf("a"), watches)
        assertTrue(connects.isEmpty())
    }

    @Test
    fun withoutTheRight_noWatch() {
        home(null, watches = mutableListOf(), connects = mutableListOf())
        rule.onNodeWithText("Se connecter").assertExists()
        openMenu()
        rule.onAllNodesWithText("Regarder").assertCountEquals(0)
    }

    /** Un appui long sur la carte de l'ordinateur, dans la rangée : l'image cliquable juste au-dessus de son nom. */
    private fun openMenu() {
        val name = rule.onAllNodesWithText("PC du salon").fetchSemanticsNodes().maxBy { it.boundsInRoot.top }.boundsInRoot
        val card = rule.onAllNodes(hasClickAction()).fetchSemanticsNodes()
            .filter { it.boundsInRoot.bottom <= name.top + 1f && it.boundsInRoot.left <= name.left + 1f && it.boundsInRoot.right > name.left }
            .maxBy { it.boundsInRoot.bottom }
        rule.onNode(SemanticsMatcher("la carte") { it.id == card.id }).performTouchInput { longClick() }
        rule.waitForIdle()
    }

    // MARK: - La barre et le menu de qui regarde

    private class Actions : SessionActions {
        var quits = 0
        val screens = mutableListOf<Int>()
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
        override fun setShareQuality(quality: ShareQuality) {}
        override fun watchScreen(id: Int) { screens += id }
        override fun share() {}
        override fun shareNewCode() {}
        override fun shareSetPads(guest: ShareGuest, on: Boolean) {}
        override fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean) {}
        override fun shareAccept(guest: ShareGuest) {}
        override fun shareRefuse(guest: ShareGuest) {}
        override fun shareRemove(guest: ShareGuest) {}
        override fun shareStop() {}
        override fun shareCopyLink() {}
    }

    private val actions = Actions()
    private val twoScreens = WatchUi("Arrêter de regarder", "Écran", listOf(2 to "Écran 1 — 3840 × 2160", 5 to "Écran 2 — 1920 × 1080"))

    private fun ui(tv: Boolean, watch: WatchUi?, setup: SessionUi.() -> Unit): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.firstImageAt.set(1)
        state.watchDisplay.value = 2
        return SessionUi(if (watch != null) "Regarder PC du salon" else "Invité sur PC du salon", tv, state, actions, guest = true, computerName = "PC du salon", watch = watch)
            .apply { passageDone = true; setup() }
    }

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    @Test
    fun theBar_stopWatching_insteadOfLeavingTheShare_asASpectator() {
        val ui = ui(false, twoScreens) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onAllNodesWithText("Quitter le partage").assertCountEquals(0)
        rule.onNodeWithText("Spectateur").assertExists()
        rule.onNodeWithText("Arrêter de regarder").performClick()
        assertEquals(1, actions.quits)
    }

    @Test
    fun theBar_screen_listsTheScreens_ticksTheWatchedOne_andChoosingCallsTheCore() {
        val ui = ui(false, twoScreens) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        // « Écran » montre celui qu'on regarde.
        rule.onNodeWithText("Écran 1 — 3840 × 2160", substring = true).assertExists()
        rule.onNodeWithText("Écran").performClick()
        assertTrue(ui.watchScreenOpen)
        rule.onAllNodesWithText("Écran 1 — 3840 × 2160")[1].assertIsSelected()
        rule.onNodeWithText("Écran 2 — 1920 × 1080").performClick()
        assertEquals(listOf(5), actions.screens)
        assertFalse(ui.watchScreenOpen)
    }

    @Test
    fun theBar_withASingleScreen_hasNoScreenMenu() {
        val ui = ui(false, WatchUi("Arrêter de regarder", "Écran")) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onAllNodesWithText("Écran").assertCountEquals(0)
        rule.onNodeWithText("Arrêter de regarder").assertExists()
    }

    @Test
    fun aGuestWhoCameWithACode_keepsLeaveTheShare() {
        val ui = ui(false, null) { barOpen = true }
        show(FormFactor.PHONE) { SessionOverlay(ui) }
        rule.onNodeWithText("Quitter le partage").assertExists()
        rule.onAllNodesWithText("Arrêter de regarder").assertCountEquals(0)
    }

    @Test
    fun theTelevisionMenu_stopWatching_andTheScreenTile() {
        val ui = ui(true, twoScreens) { menuOpen = true }
        show(FormFactor.TV) { SessionOverlay(ui) }
        rule.onNodeWithText("Regarder PC du salon · Spectateur", substring = true, ignoreCase = true).assertExists()
        rule.onAllNodesWithText("Quitter le partage").assertCountEquals(0)
        rule.onNodeWithText("Écran").performClick()
        rule.onNodeWithText("Écran 2 — 1920 × 1080").performClick()
        assertEquals(listOf(5), actions.screens)
        rule.onNodeWithText("Arrêter de regarder").performClick()
        assertEquals(1, actions.quits)
    }
}
