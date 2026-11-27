package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.model.parseEach
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Les listes enregistrées (AND-22) : une entrée abîmée n'efface plus les autres. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class JsonListsTest {
    @Test
    fun oneBadEntryKeepsTheOthers() {
        val json = """[{"id":"a"},{"autre":1},"pas un objet",{"id":"c"}]"""
        assertEquals(listOf("a", "c"), parseEach(json) { it.getString("id") })
    }

    @Test
    fun unreadableOrEmpty() {
        assertEquals(emptyList<String>(), parseEach("illisible") { it.getString("id") })
        assertEquals(emptyList<String>(), parseEach(null) { it.getString("id") })
    }
}

/** Une seule règle pour les IPv4 (réveil, ajout d'une console). */
class Ipv4Test {
    @Test
    fun ipv4() {
        org.junit.Assert.assertTrue(fr.leslignesclaires.kybernow.model.isIpv4("192.168.1.20"))
        listOf("300.1.1.1", "1.2.3", "a.b.c.d", "fe80::1", "salon.local", "1.2.3.4.5", "", "+1.2.3.4").forEach {
            org.junit.Assert.assertFalse(it, fr.leslignesclaires.kybernow.model.isIpv4(it))
        }
    }
}
