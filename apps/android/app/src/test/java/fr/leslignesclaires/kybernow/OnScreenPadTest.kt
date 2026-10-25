package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.core.Pad
import fr.leslignesclaires.kybernow.model.PlayerSlots
import fr.leslignesclaires.kybernow.session.Feel
import fr.leslignesclaires.kybernow.session.OnScreenPad
import fr.leslignesclaires.kybernow.session.PadControl
import fr.leslignesclaires.kybernow.session.PadGeometry
import fr.leslignesclaires.kybernow.session.PadOut
import fr.leslignesclaires.kybernow.session.PadShape
import fr.leslignesclaires.kybernow.session.StickZone
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.cos
import kotlin.math.sin

/** Ce que la manette a envoyé, dans l'ordre. */
private class Record : PadOut {
    val events = mutableListOf<String>()
    val feels = mutableListOf<Feel>()
    val axes = IntArray(6)
    val buttons = mutableSetOf<Int>()
    override fun button(button: Int, pressed: Boolean) {
        events += "b$button=${if (pressed) 1 else 0}"
        if (pressed) buttons += button else buttons -= button
    }
    override fun axis(axis: Int, value: Int) {
        events += "a$axis=$value"
        axes[axis] = value
    }
    override fun feel(f: Feel) { feels += f }
}

class OnScreenPadTest {
    /** Un téléphone en paysage (914 × 411), un pixel par dp, taille moyenne : u = 1. */
    private val g = PadGeometry.compute(914f, 411f, 0f, 0f, 0f, 0f, 1f, 1)
    private val out = Record()
    private val pad = OnScreenPad(out).apply { geometry = g }
    private val stick = g[PadControl.LEFT_STICK]

    @Test
    fun stickStartsWhereTheFingerLands() {
        pad.down(1, stick.cx + 10, stick.cy - 8, 0)
        assertEquals(0, out.axes[Pad.LEFT_X])
        pad.move(1, stick.cx + 10 + 44, stick.cy - 8)
        assertEquals(32767, out.axes[Pad.LEFT_X])
        assertEquals(0, out.axes[Pad.LEFT_Y])
        // Y vers le bas, comme XInput côté kynput.
        pad.move(1, stick.cx + 10, stick.cy - 8 + 22)
        assertEquals(0, out.axes[Pad.LEFT_X])
        assertEquals(16384, out.axes[Pad.LEFT_Y])
        pad.up(1, 100)
        assertEquals(0, out.axes[Pad.LEFT_Y])
        assertFalse(pad.left.touched)
    }

    @Test
    fun stickTouchedOnItsRingPushesAtOnce() {
        pad.down(1, stick.cx + 40, stick.cy, 0)
        assertEquals(40.0 / 44 * 32767, out.axes[Pad.LEFT_X].toDouble(), 1.0)
    }

    @Test
    fun stickStopsAtItsCircle() {
        pad.down(1, stick.cx, stick.cy, 0)
        pad.move(1, stick.cx + 100, stick.cy + 100)
        assertEquals(23170.0, out.axes[Pad.LEFT_X].toDouble(), 1.0)
        assertEquals(23170.0, out.axes[Pad.LEFT_Y].toDouble(), 1.0)
    }

    @Test
    fun stickTicks() {
        pad.down(1, stick.cx, stick.cy, 0)
        pad.move(1, stick.cx + 15, stick.cy)
        assertEquals(listOf(Feel.STICK_LEAVE), out.feels)
        pad.move(1, stick.cx + 60, stick.cy)
        assertEquals(Feel.STICK_EDGE, out.feels.last())
    }

    @Test
    fun tappingAStickTwiceClicksIt() {
        pad.down(1, stick.cx, stick.cy, 0)
        pad.up(1, 120)
        pad.down(2, stick.cx, stick.cy, 260)
        assertTrue(Pad.LEFT_STICK in out.buttons)
        assertTrue(pad.left.clicked)
        assertEquals(Feel.CLICK, out.feels.last())
        pad.up(2, 400)
        assertFalse(Pad.LEFT_STICK in out.buttons)
        // Trop tard : pas de clic.
        pad.down(3, stick.cx, stick.cy, 1000)
        pad.up(3, 1100)
        pad.down(4, stick.cx, stick.cy, 1600)
        assertFalse(Pad.LEFT_STICK in out.buttons)
    }

    @Test
    fun aSlideIsNotATapForTheDoubleTap() {
        pad.down(1, stick.cx, stick.cy, 0)
        pad.move(1, stick.cx + 30, stick.cy)
        pad.up(1, 120)
        pad.down(2, stick.cx, stick.cy, 200)
        assertFalse(Pad.LEFT_STICK in out.buttons)
    }

    @Test
    fun triggersAreProgressive() {
        val lt = g[PadControl.LT]
        pad.down(1, lt.cx, lt.top + 1, 0)
        assertEquals(listOf(Feel.TRIGGER_START), out.feels)
        val light = out.axes[Pad.L2]
        assertTrue(light in 4900..5800)
        pad.move(1, lt.cx, lt.cy)
        assertTrue(out.axes[Pad.L2] in (light + 1) until 32767)
        // Tirée vers le bas : à fond, un clic franc, une seule fois.
        pad.move(1, lt.cx, lt.bottom - 2)
        pad.move(1, lt.cx, lt.bottom + 30)
        assertEquals(32767, out.axes[Pad.L2])
        assertEquals(listOf(Feel.TRIGGER_START, Feel.TRIGGER_END), out.feels)
        pad.up(1, 500)
        assertEquals(0, out.axes[Pad.L2])
    }

    @Test
    fun severalFingersAtOnce() {
        val (ax, ay) = g.faceCenter(Pad.A)
        val rt = g[PadControl.RT]
        val right = g[PadControl.RIGHT_STICK]
        pad.down(1, stick.cx, stick.cy, 0)
        pad.down(2, ax, ay, 10)
        pad.down(3, rt.cx, rt.cy, 20)
        pad.down(4, right.cx, right.cy, 30)
        pad.move(1, stick.cx - 44, stick.cy)
        pad.move(4, right.cx, right.cy - 44)
        assertEquals(setOf(Pad.A), out.buttons)
        assertEquals(-32767, out.axes[Pad.LEFT_X])
        assertEquals(-32767, out.axes[Pad.RIGHT_Y])
        assertTrue(out.axes[Pad.R2] > 0)
        pad.up(2, 100)
        assertEquals(emptySet<Int>(), out.buttons)
        assertEquals(-32767, out.axes[Pad.LEFT_X])
        assertTrue(out.axes[Pad.R2] > 0)
    }

    @Test
    fun dpadDiagonalsAndSlides() {
        val d = g[PadControl.DPAD]
        pad.down(1, d.cx + 30, d.cy + 30, 0)
        assertEquals(setOf(Pad.RIGHT, Pad.DOWN), out.buttons)
        pad.move(1, d.cx + 40, d.cy)
        assertEquals(setOf(Pad.RIGHT), out.buttons)
        pad.move(1, d.cx, d.cy - 40)
        assertEquals(setOf(Pad.UP), out.buttons)
        // Un clic à chaque flèche qui s'enfonce (pas quand une diagonale en lâche une).
        assertEquals(2, out.feels.count { it == Feel.CLICK })
        pad.up(1, 300)
        assertEquals(emptySet<Int>(), out.buttons)
    }

    @Test
    fun faceButtonsFollowTheThumb() {
        val (ax, ay) = g.faceCenter(Pad.A)
        val (bx, by) = g.faceCenter(Pad.B)
        pad.down(1, ax, ay, 0)
        pad.move(1, bx, by)
        assertEquals(listOf("b${Pad.A}=1", "b${Pad.A}=0", "b${Pad.B}=1"), out.events)
    }

    @Test
    fun simpleLayoutReleasesWhatDisappears() {
        val rt = g[PadControl.RT]
        pad.down(1, stick.cx, stick.cy, 0)
        pad.move(1, stick.cx + 44, stick.cy)
        pad.down(2, rt.cx, rt.cy, 10)
        pad.setLayout(true)
        assertEquals(0, out.axes[Pad.R2])
        assertEquals(32767, out.axes[Pad.LEFT_X])
        // Les commandes de la disposition complète ne répondent plus.
        pad.down(3, rt.cx, rt.cy, 20)
        assertEquals(0, out.axes[Pad.R2])
        assertNull(g.hit(g[PadControl.LB].cx, g[PadControl.LB].cy, simple = true))
        pad.up(1, 30)
        assertEquals(0, out.axes[Pad.LEFT_X])
    }

    @Test
    fun aTapOutsideTheControlsOnlyCallsBack() {
        var taps = 0
        pad.onTapOutside = { taps++ }
        pad.down(1, 457f, 150f, 0)
        pad.up(1, 100)
        assertEquals(1, taps)
        pad.down(2, 457f, 150f, 200)
        pad.move(2, 520f, 150f)
        pad.up(2, 300)
        assertEquals(1, taps)
        assertTrue(out.events.isEmpty())
    }

    @Test
    fun theLayoutButton() {
        var switches = 0
        pad.onSwitch = { switches++ }
        val sw = g[PadControl.SWITCH]
        pad.down(1, sw.cx, sw.cy, 0)
        assertTrue(pad.switchDown)
        pad.up(1, 80)
        assertEquals(1, switches)
        // Doigt parti en glissant : rien.
        pad.down(2, sw.cx, sw.cy, 200)
        pad.move(2, sw.cx + 80, sw.cy)
        pad.up(2, 280)
        assertEquals(1, switches)
    }

    @Test
    fun releaseAll() {
        val (ax, ay) = g.faceCenter(Pad.A)
        pad.down(1, ax, ay, 0)
        pad.down(2, stick.cx, stick.cy, 0)
        pad.move(2, stick.cx, stick.cy + 44)
        pad.releaseAll()
        assertEquals(emptySet<Int>(), out.buttons)
        assertTrue(out.axes.all { it == 0 })
        assertEquals(emptySet<Int>(), pad.pressed)
    }

    /** Dessins dans les zones sûres, sans se chevaucher, à chaque taille, au téléphone, à la tablette, en portrait. */
    @Test
    fun layoutsFitWithoutOverlapping() {
        val screens = listOf(
            // largeur, hauteur (px), zones sûres gauche, haut, droite, bas (px), densité
            floatArrayOf(2400f, 1080f, 110f, 63f, 63f, 63f, 2.625f),
            floatArrayOf(1600f, 720f, 0f, 0f, 0f, 0f, 2f),
            floatArrayOf(2560f, 1600f, 0f, 48f, 0f, 48f, 2f),
            floatArrayOf(1600f, 2560f, 0f, 48f, 0f, 48f, 2f),
        )
        for (sc in screens) for (size in 0..2) {
            for (simple in listOf(false, true)) {
            val geo = PadGeometry.compute(sc[0], sc[1], sc[2], sc[3], sc[4], sc[5], sc[6], size, simple)
            val shapes = geo.shapes(simple)
            for (s in shapes) {
                assertTrue("${s.control} déborde", s.left >= sc[2] && s.top >= sc[3] && s.right <= sc[0] - sc[4] && s.bottom <= sc[1] - sc[5])
            }
            for (a in shapes) for (b in shapes) if (a !== b) assertFalse("${a.control} chevauche ${b.control}", overlap(a, b))
            }
        }
    }

    private fun overlap(a: PadShape, b: PadShape): Boolean {
        if (a.round && b.round) return kotlin.math.hypot(a.cx - b.cx, a.cy - b.cy) < a.hw + b.hw
        return a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
    }
}

/** Les tics des sticks : sortie du centre, changement de direction, butée (fonction pure). */
class StickZoneTest {
    private fun at(m: Float, deg: Float) = Math.toRadians(deg.toDouble()).let { (m * cos(it)).toFloat() to (m * sin(it)).toFloat() }

    private fun step(before: StickZone, m: Float, deg: Float): Pair<StickZone, Feel?> {
        val (x, y) = at(m, deg)
        val after = StickZone.of(x, y, before)
        return after to StickZone.tick(before, after)
    }

    @Test
    fun leaveTurnAndEdge() {
        var z = StickZone.CENTER
        var (n, t) = step(z, 0.1f, 0f); z = n
        assertEquals(StickZone.CENTER, z); assertNull(t)
        step(z, 0.3f, 0f).let { (a, b) -> z = a; t = b }
        assertEquals(Feel.STICK_LEAVE, t); assertEquals(0, z.sector)
        // Entre les deux seuils du centre : toujours sorti, pas de tic.
        step(z, 0.19f, 0f).let { (a, b) -> z = a; t = b }
        assertTrue(z.out); assertNull(t)
        // Juste après la frontière (22,5°), dans la marge : même direction.
        step(z, 0.5f, 26f).let { (a, b) -> z = a; t = b }
        assertEquals(0, z.sector); assertNull(t)
        step(z, 0.5f, 32f).let { (a, b) -> z = a; t = b }
        assertEquals(1, z.sector); assertEquals(Feel.STICK_SECTOR, t)
        // Retour vers la frontière : on garde le secteur atteint.
        step(z, 0.5f, 19f).let { (a, b) -> z = a; t = b }
        assertEquals(1, z.sector); assertNull(t)
        step(z, 0.98f, 45f).let { (a, b) -> z = a; t = b }
        assertEquals(Feel.STICK_EDGE, t)
        step(z, 0.94f, 45f).let { (a, b) -> z = a; t = b }
        assertTrue(z.edge); assertNull(t)
        step(z, 0.85f, 45f).let { (a, b) -> z = a; t = b }
        assertFalse(z.edge); assertNull(t)
        step(z, 0.99f, 45f).let { (a, b) -> z = a; t = b }
        assertEquals(Feel.STICK_EDGE, t)
        step(z, 0.05f, 45f).let { (a, b) -> z = a; t = b }
        assertEquals(StickZone.CENTER, z); assertNull(t)
    }

    @Test
    fun aFlickToTheEdgeTicksOnce() {
        val (z, t) = step(StickZone.CENTER, 1f, 270f)
        assertEquals(Feel.STICK_EDGE, t)
        assertEquals(6, z.sector)
    }

    @Test
    fun aroundTheZeroAngle() {
        var z = step(StickZone.CENTER, 0.5f, 350f).first
        assertEquals(0, z.sector)
        z = step(z, 0.5f, 340f).first
        assertEquals(0, z.sector)
        val (n, t) = step(z, 0.5f, 330f)
        assertEquals(7, n.sector); assertEquals(Feel.STICK_SECTOR, t)
    }
}

/** Les places de joueur : la manette à l'écran prend la première libre, comme une manette de plus. */
class PlayerSlotsTest {
    private val pad = "écran"

    @Test
    fun theOnScreenControllerTakesTheFirstFreeSlot() {
        val slots = PlayerSlots()
        assertEquals(mapOf("a" to 0), slots.assign(listOf("a")))
        assertEquals(mapOf("a" to 0, pad to 1), slots.assign(listOf("a", pad)))
        assertEquals(mapOf("a" to 0, "b" to 2, pad to 1), slots.assign(listOf("a", "b", pad)))
        // Retirée : sa place se libère, les autres gardent la leur ; la suivante la prend.
        assertEquals(mapOf("a" to 0, "b" to 2), slots.assign(listOf("a", "b")))
        assertEquals(mapOf("a" to 0, "b" to 2, "c" to 1), slots.assign(listOf("a", "b", "c")))
    }

    @Test
    fun aloneItIsPlayerOne() {
        val slots = PlayerSlots()
        assertEquals(mapOf(pad to 0), slots.assign(listOf(pad)))
        assertEquals(mapOf("a" to 1, pad to 0), slots.assign(listOf("a", pad)))
    }

    @Test
    fun aFifthControllerWaits() {
        val slots = PlayerSlots()
        assertEquals(-1, slots.assign(listOf("a", "b", "c", "d", pad))[pad])
        assertEquals(1, slots.assign(listOf("a", "c", "d", pad))[pad])
    }
}
