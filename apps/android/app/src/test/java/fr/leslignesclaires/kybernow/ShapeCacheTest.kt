package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.session.ShapeCache
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

/** Formes du pointeur (AND-26) : une image par forme, la plus ancienne oubliée au-delà de la taille. */
class ShapeCacheTest {
    @Test
    fun sameShapeSameImage() {
        val c = ShapeCache<Any>(2)
        var made = 0
        val a = c.get(2, 2, 0, 0, intArrayOf(1, 2, 3, 4)) { made++; Any() }
        val b = c.get(2, 2, 0, 0, intArrayOf(1, 2, 3, 4)) { made++; Any() }
        assertSame(a, b)
        assertEquals(1, made)
        // Point chaud différent : une autre forme.
        c.get(2, 2, 1, 0, intArrayOf(1, 2, 3, 4)) { made++; Any() }
        c.get(2, 2, 0, 0, intArrayOf(9, 2, 3, 4)) { made++; Any() }
        assertEquals(3, made)
        assertEquals(2, c.count())
    }
}
