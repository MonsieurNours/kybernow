package fr.leslignesclaires.kybernow

import android.app.Application
import android.os.Looper
import android.view.MotionEvent
import fr.leslignesclaires.kybernow.session.PenOut
import fr.leslignesclaires.kybernow.session.PenSample
import fr.leslignesclaires.kybernow.session.PointerOut
import fr.leslignesclaires.kybernow.session.StylusInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.time.Duration

/** Le stylet (docs/link-ui.md, « Stylet ») : en stylet quand l'ordinateur l'a annoncé, sinon souris à cet endroit et clic gauche. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class StylusInputTest {
    private var accepted = true
    private val pens = mutableListOf<Pair<Int, PenSample>>()
    private val mouse = mutableListOf<String>()
    private val penOut = object : PenOut {
        override fun pen(x: Double, y: Double, s: PenSample): Boolean {
            pens += x.toInt() to s
            return accepted
        }
    }
    private val mouseOut = object : PointerOut {
        override fun moveTo(x: Double, y: Double) { mouse += "to ${x.toInt()}" }
        override fun moveBy(dx: Double, dy: Double) { mouse += "by" }
        override fun button(button: Int, pressed: Boolean) { mouse += "b$button ${if (pressed) "down" else "up"}" }
        override fun wheel(dx: Float, dy: Float) { mouse += "wheel" }
    }
    private val stylus = StylusInput(penOut, mouseOut) { x, y -> x.toDouble() to y.toDouble() }

    private fun ev(
        action: Int,
        t: Long,
        x: Float = 100f,
        pressure: Float = 0.5f,
        tool: Int = MotionEvent.TOOL_TYPE_STYLUS,
        buttons: Int = 0,
    ): MotionEvent {
        val props = arrayOf(MotionEvent.PointerProperties().apply { id = 0; toolType = tool })
        val coords = arrayOf(MotionEvent.PointerCoords().apply { this.x = x; y = 50f; this.pressure = pressure })
        return MotionEvent.obtain(0, t, action, 1, props, coords, 0, buttons, 1f, 1f, 0, 0, 0, 0)
    }

    private fun idle(ms: Long) = shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(ms))

    @Test
    fun aStrokeGoesAsAPenThenLeaves() {
        val down = ev(MotionEvent.ACTION_DOWN, 0)
        assertTrue(stylus.claims(down))
        stylus.onTouch(down)
        val move = ev(MotionEvent.ACTION_MOVE, 10, 110f, 0.6f).apply {
            addBatch(20, arrayOf(MotionEvent.PointerCoords().apply { x = 120f; y = 50f; pressure = 0.8f }), 0)
        }
        stylus.onTouch(move)
        stylus.onTouch(ev(MotionEvent.ACTION_UP, 30, 120f, 0f))
        // Le point groupé (110) part avant le dernier (120).
        assertEquals(listOf(100, 110, 120, 120), pens.map { it.first })
        assertTrue(pens.take(3).all { it.second.contact && it.second.inRange })
        assertEquals(0.8f, pens[2].second.pressure)
        assertTrue(!pens[3].second.contact && pens[3].second.inRange)
        idle(200)
        assertEquals(5, pens.size)
        assertFalse(pens[4].second.inRange)
        assertEquals(emptyList<String>(), mouse)
    }

    @Test
    fun beforeTheStationKnowsThePen_aMouseWithTheLeftButton() {
        accepted = false
        stylus.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        stylus.onTouch(ev(MotionEvent.ACTION_MOVE, 10, 140f))
        stylus.onTouch(ev(MotionEvent.ACTION_UP, 20, 140f))
        idle(200)
        assertEquals(listOf("to 100", "b0 down", "to 140", "to 140", "b0 up"), mouse)
    }

    /** « Stylet : Souris » (ou Station d'avant) : la pointe posée avec le bouton du stylet tenu fait un clic droit. */
    @Test
    fun asAMouse_theBarrelButtonIsTheRightClick() {
        accepted = false
        stylus.onTouch(ev(MotionEvent.ACTION_DOWN, 0, buttons = MotionEvent.BUTTON_STYLUS_PRIMARY))
        stylus.onTouch(ev(MotionEvent.ACTION_UP, 10))
        idle(200)
        assertEquals(listOf("to 100", "b1 down", "to 100", "b1 up"), mouse)
    }

    @Test
    fun hoverExitRightBeforeTouching_thePenDoesNotLeave() {
        stylus.onHover(ev(MotionEvent.ACTION_HOVER_ENTER, 0, pressure = 0f))
        stylus.onHover(ev(MotionEvent.ACTION_HOVER_EXIT, 5, pressure = 0f))
        stylus.onTouch(ev(MotionEvent.ACTION_DOWN, 6))
        idle(200)
        assertTrue(pens.all { it.second.inRange })
        assertTrue(pens.last().second.contact)
    }

    @Test
    fun eraserAndBarrel() {
        stylus.onTouch(ev(MotionEvent.ACTION_DOWN, 0, tool = MotionEvent.TOOL_TYPE_ERASER, buttons = MotionEvent.BUTTON_STYLUS_PRIMARY))
        assertTrue(pens.single().second.eraser)
        assertTrue(pens.single().second.barrel)
    }

    @Test
    fun aFingerIsNotThePens() {
        assertFalse(stylus.claims(ev(MotionEvent.ACTION_DOWN, 0, tool = MotionEvent.TOOL_TYPE_FINGER)))
        assertFalse(stylus.onHover(ev(MotionEvent.ACTION_HOVER_MOVE, 0, tool = MotionEvent.TOOL_TYPE_MOUSE)))
    }
}
