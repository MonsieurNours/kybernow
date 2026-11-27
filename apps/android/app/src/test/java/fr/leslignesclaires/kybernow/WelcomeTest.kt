package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.Welcome
import fr.leslignesclaires.kybernow.core.WelcomeStep
import fr.leslignesclaires.kybernow.model.WelcomeStore
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * L'accueil et l'incitation au compte (docs/link-ui.md, « Accueil et compte KyberNow ») : l'état gardé par l'app
 * (préférences `welcome`) et ce qu'elle fait des réponses du cœur. Les règles elles-mêmes (une fois, trente jours, une
 * fois par jour et par ordinateur) sont celles de `welcome.rs`, essayées par le cœur : ici, un faux cœur qui note ce
 * qu'on lui demande.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class WelcomeTest {
    private val context get() = ApplicationProvider.getApplicationContext<Application>()

    /** Compte les appels dans l'état (`n`) et répond [answer] ; note les opérations demandées. */
    private class Counting(var answer: Boolean = true) : FakeCore() {
        val ops = mutableListOf<String>()

        override fun welcome(stateJson: String, op: String, hasAccount: Boolean, computer: String, now: Long): String {
            ops += op
            val state = JSONObject(stateJson.ifBlank { "{}" })
            state.put("n", state.optInt("n") + 1)
            return JSONObject().put("state", state).put("value", answer && !hasAccount).toString()
        }
    }

    @After fun realCore() = FakeCore.reset()

    @Test
    fun theStateGivenByTheCore_isKept_andGivenBack() {
        val core = Counting()
        FakeCore.install(core)
        val store = WelcomeStore(context)
        assertTrue(store.shouldShow(false))
        store.done()
        assertEquals(listOf("should_show", "done"), core.ops)
        // Relu des préférences : l'état écrit par le cœur revient tel quel au cœur.
        assertEquals(2, JSONObject(WelcomeStore(context).state.value).getInt("n"))
        WelcomeStore(context).cardVisible(false, 1)
        assertEquals(3, JSONObject(WelcomeStore(context).state.value).getInt("n"))
    }

    @Test
    fun thePairedBanner_waitsForItsPlace_thenAsksTheCore() {
        val core = Counting()
        FakeCore.install(core)
        val store = WelcomeStore(context)
        store.paired(false, "pc1")
        // En attente de sa place : rien n'est demandé au cœur tant qu'il ne s'affiche pas.
        assertEquals("pc1", store.pairedPending.value)
        assertNull(store.pairedBanner.value)
        assertTrue(core.ops.isEmpty())
        store.showPaired(false, "pc1")
        assertEquals(listOf("take_paired_hint"), core.ops)
        assertEquals("pc1", store.pairedBanner.value)
        assertNull(store.pairedPending.value)
        store.dismissPairedBanner()
        assertNull(store.pairedBanner.value)
        // Le cœur dit non (déjà montré) : pas de bandeau, l'attente s'arrête.
        core.answer = false
        store.paired(false, "pc1")
        store.showPaired(false, "pc1")
        assertNull(store.pairedBanner.value)
        assertNull(store.pairedPending.value)
        // Avec un compte : jamais en attente.
        store.paired(true, "pc2")
        assertNull(store.pairedPending.value)
    }

    @Test
    fun withoutTheCore_noWelcome_andTheStateStays() {
        // Sans bibliothèque native : ni accueil, ni carte, ni bandeau, et rien n'est écrit (pas de copie des règles).
        val step = Welcome.apply("""{"welcome_seen":0}""", Welcome.Op.SHOULD_SHOW, false)
        assertFalse(step.value)
        assertEquals("""{"welcome_seen":0}""", step.state)
        assertEquals("", Welcome.signUpUrl(""))
    }

    @Test
    fun theCoresAnswer_isRead() {
        val state = """{"welcome_seen":1,"card_closed":1}"""
        assertEquals(WelcomeStep(JSONObject(state).toString(), false), Welcome.read("""{"state":$state,"value":false}"""))
        assertNull(Welcome.read("pas du json"))
        assertEquals("www.kybernow.app", Welcome.stationLabel("https://www.kybernow.app/#telecharger"))
    }
}
