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
