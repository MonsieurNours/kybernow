package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.Welcome
import fr.leslignesclaires.kybernow.model.WelcomeStore
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * L'accueil et l'incitation au compte (docs/link-ui.md, « Accueil et compte KyberNow ») : les règles du cœur
 * (`welcome.rs`), ici appliquées sans bibliothèque native, et l'état gardé par l'app (préférences `welcome`).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class WelcomeTest {
    private val context get() = ApplicationProvider.getApplicationContext<Application>()
    private val t0 = 1_800_000_000L

    @Test
    fun accueil_uneFoisSansCompte_etGardé() {
        val store = WelcomeStore(context)
        assertTrue("premier lancement", store.shouldShow(false))
        assertFalse("avec un compte : jamais", store.shouldShow(true))
        store.done()
        assertFalse(store.shouldShow(false))
        // Relu des préférences : toujours vu.
        assertFalse(WelcomeStore(context).shouldShow(false))
    }

    @Test
    fun carte_fermée_revientAprèsTrenteJours_puisNePlusAfficher() {
        val store = WelcomeStore(context)
        assertTrue(store.cardVisible(false, t0))
        assertFalse(store.cardVisible(true, t0))
        assertFalse(store.cardCloseIsFinal())
        store.closeCard(t0)
        assertFalse(store.cardVisible(false, t0 + 29 * 24 * 3600))
        assertTrue(store.cardVisible(false, t0 + 30 * 24 * 3600))
        assertTrue("revenue : « Ne plus afficher »", store.cardCloseIsFinal())
        store.closeCard(t0 + 30 * 24 * 3600)
        assertFalse("pour de bon", store.cardVisible(false, t0 + 365L * 24 * 3600))
    }

    @Test
    fun bandeauAprèsAppairage_uneFoisParOrdinateur() {
        val store = WelcomeStore(context)
        store.paired(false, "pc1")
        // En attente de sa place : rien n'est compté tant qu'il ne s'affiche pas.
        assertEquals("pc1", store.pairedPending.value)
        assertNull(store.pairedBanner.value)
        store.showPaired(false, "pc1")
        assertEquals("pc1", store.pairedBanner.value)
        assertNull(store.pairedPending.value)
        store.dismissPairedBanner()
        store.paired(false, "pc1")
        store.showPaired(false, "pc1")
        assertNull("déjà montré pour pc1", store.pairedBanner.value)
        assertNull(store.pairedPending.value)
        store.paired(true, "pc2")
        assertNull("avec un compte : jamais", store.pairedPending.value)
        store.paired(false, "pc2")
        store.showPaired(false, "pc2")
        assertEquals("pc2", store.pairedBanner.value)
    }

    @Test
    fun horsDeChezVous_uneFoisParJourEtParOrdinateur() {
        val store = WelcomeStore(context)
        assertTrue(store.takeAwayHint(false, "pc1", t0))
        assertFalse(store.takeAwayHint(false, "pc1", t0 + 3600))
        assertTrue("un autre ordinateur", store.takeAwayHint(false, "pc2", t0 + 3600))
        assertTrue("le lendemain", store.takeAwayHint(false, "pc1", t0 + 24 * 3600))
        assertFalse("avec un compte : jamais", store.takeAwayHint(true, "pc3", t0))
    }

    @Test
    fun étatDuCœur_lu() {
        // Un état écrit par le cœur (champs de `WelcomeState`) se lit par les règles recopiées, et inversement.
        val core = """{"welcome_seen":1,"card_closed":1,"card_closed_at":$t0,"paired_hint":["pc1"],"away_hint":[["pc1",$t0]]}"""
        assertFalse(Welcome.apply(core, Welcome.Op.SHOULD_SHOW, false).value)
        assertTrue(Welcome.apply(core, Welcome.Op.CARD_CLOSE_IS_FINAL, false).value)
        assertFalse(Welcome.apply(core, Welcome.Op.TAKE_PAIRED_HINT, false, "pc1").value)
        assertFalse(Welcome.apply(core, Welcome.Op.TAKE_AWAY_HINT, false, "pc1", t0 + 60).value)
        assertTrue("illisible : comme au premier lancement", Welcome.apply("pas du json", Welcome.Op.SHOULD_SHOW, false).value)
        assertEquals(WelcomeStepOf(core), Welcome.read("""{"state":$core,"value":false}"""))
    }

    private fun WelcomeStepOf(state: String) = fr.leslignesclaires.kybernow.core.WelcomeStep(org.json.JSONObject(state).toString(), false)

    @Test
    fun adresses() {
        assertEquals("https://share.kybernow.app/compte/inscription?lang=fr", Welcome.signUpUrl("", "fr"))
        assertEquals("https://relais.exemple.org:9443/compte/inscription?lang=en", Welcome.signUpUrl("https://relais.exemple.org:9443/", "en"))
        assertEquals("www.kybernow.app", Welcome.stationLabel(Welcome.stationUrl))
    }
}
