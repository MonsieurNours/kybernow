package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.PyroWave
import fr.leslignesclaires.kybernow.model.PyroWaveBlock
import fr.leslignesclaires.kybernow.model.PyroWaveChoice
import fr.leslignesclaires.kybernow.model.PyroWaveLink
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** PyroWave dans « Format vidéo » (docs/link-ui.md, « PyroWave ») : quand la valeur se montre, s'active, et par où passe l'image. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = android.app.Application::class)
class PyroWaveTest {
    private fun host(pyrowave: Boolean?) = Host(name = "PC du salon", address = "192.168.1.20", port = 41234, fingerprint = null, user = "u", pyrowave = pyrowave)

    @Test
    fun annonceDeLaStation() {
        assertNull("Station d'avant PyroWave", PyroWave.offered(JSONObject("""{"hostname":"pc"}""")))
        assertEquals(true, PyroWave.offered(JSONObject("""{"pyrowave":{"available":true}}""")))
        assertEquals(false, PyroWave.offered(JSONObject("""{"pyrowave":{"available":false,"reason":"vulkan"}}""")))
    }

    @Test
    fun valeurMontreeInactiveOuChoisie() {
        assertFalse("pas annoncé : absent", PyroWaveChoice.of(host(null), true, true, false).shown)
        val off = PyroWaveChoice.of(host(false), true, true, false)
        assertTrue(off.shown)
        assertEquals(PyroWaveBlock.NOT_OFFERED, off.block)
        assertEquals(PyroWaveBlock.NO_DECODER, PyroWaveChoice.of(host(true), false, true, false).block)
        val ok = PyroWaveChoice.of(host(true), true, true, false)
        assertTrue(ok.usable)
        assertEquals(PyroWaveLink.CABLE, ok.link)
        assertEquals(PyroWaveLink.WIFI, PyroWaveChoice.of(host(true), true, false, false).link)
        assertEquals("le relais l'emporte sur le câble", PyroWaveLink.RELAY, PyroWaveChoice.of(host(true), true, true, true).link)
    }

    @Test
    fun ordinateurGardeAvecSonAnnonce() {
        assertEquals(true, Host.parse(host(true).json()).pyrowave)
        assertEquals(false, Host.parse(host(false).json()).pyrowave)
        assertNull(Host.parse(host(null).json()).pyrowave)
    }
}
