package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.session.RelativeMotion
import org.junit.Assert.assertEquals
import org.junit.Test

/** Souris capturée (AND-13) : historique additionné, fractions gardées. */
class RelativeMotionTest {
    private fun RelativeMotion.sent(): List<Pair<Int, Int>> = mutableListOf<Pair<Int, Int>>().also { out -> take { x, y -> out += x to y } }

    @Test
    fun slowMovesAddUp() {
        val m = RelativeMotion()
        // Dix échantillons de 0,3 px : 3 px au total (l'arrondi de chacun aurait donné 0).
        val out = (1..10).flatMap { m.add(0.3f, -0.3f); m.sent() }
        assertEquals(3, out.sumOf { it.first })
        assertEquals(-3, out.sumOf { it.second })
    }

    @Test
    fun historyIsCounted() {
        val m = RelativeMotion()
        listOf(4f, 5f, 6.5f).forEach { m.add(it, 1f) }
        assertEquals(listOf(15 to 3), m.sent())
        // Le demi-pixel attend le suivant.
        m.add(0.5f, 0f)
        assertEquals(listOf(1 to 0), m.sent())
        assertEquals(emptyList<Pair<Int, Int>>(), m.sent())
    }

    @Test
    fun resetForgets() {
        val m = RelativeMotion()
        m.add(0.9f, 0.9f)
        m.reset()
        m.add(0.2f, 0.2f)
        assertEquals(emptyList<Pair<Int, Int>>(), m.sent())
    }
}
