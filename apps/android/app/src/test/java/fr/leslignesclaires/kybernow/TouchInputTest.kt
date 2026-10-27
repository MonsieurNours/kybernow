package fr.leslignesclaires.kybernow

import android.app.Application
import android.os.Looper
import android.view.MotionEvent
import fr.leslignesclaires.kybernow.session.PointerOut
import fr.leslignesclaires.kybernow.session.TouchInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.time.Duration

/** Le toucher sur l'image : clavier sorti, un toucher le replie sans cliquer ; les autres gestes gardent leur effet. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class TouchInputTest {
    private val sent = mutableListOf<String>()
    private val out = object : PointerOut {
        override fun moveTo(x: Double, y: Double) { sent += "to ${x.toInt()},${y.toInt()}" }
        override fun moveBy(dx: Double, dy: Double) { sent += "by" }
        override fun button(button: Int, pressed: Boolean) { sent += "b$button ${if (pressed) "down" else "up"}" }
        override fun wheel(dx: Float, dy: Float) { sent += "wheel" }
    }
    private var keyboard = true
    private var folds = 0
    private val touch = TouchInput(out, { x, y -> x.toDouble() to y.toDouble() }, { 1.0 }).apply {
        keyboardOpen = { keyboard }
        onFold = { folds++ }
    }

    private fun ev(action: Int, t: Long, x: Float = 100f, y: Float = 100f) = MotionEvent.obtain(0, t, action, x, y, 0)

    private fun twoFingers(action: Int, t: Long): MotionEvent {
        val props = Array(2) { i -> MotionEvent.PointerProperties().apply { id = i; toolType = MotionEvent.TOOL_TYPE_FINGER } }
        val coords = Array(2) { i -> MotionEvent.PointerCoords().apply { x = 100f + i * 80; y = 100f } }
        return MotionEvent.obtain(0, t, action, 2, props, coords, 0, 0, 1f, 1f, 0, 0, 0, 0)
    }

    private fun idle(ms: Long) = shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(ms))

    @Test
    fun aTapFoldsTheKeyboardWithoutClicking() {
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.onTouch(ev(MotionEvent.ACTION_UP, 120))
        assertEquals(1, folds)
        assertEquals(emptyList<String>(), sent)
    }

    @Test
    fun keyboardFolded_aTapClicks() {
        keyboard = false
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.onTouch(ev(MotionEvent.ACTION_UP, 120))
        assertEquals(0, folds)
        assertEquals(listOf("b0 down", "b0 up"), sent)
    }

    @Test
    fun aSlideStillMovesThePointer() {
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.onTouch(ev(MotionEvent.ACTION_MOVE, 30, 140f, 100f))
        touch.onTouch(ev(MotionEvent.ACTION_UP, 120, 140f, 100f))
        assertEquals(0, folds)
        assertTrue("by" in sent)
        assertTrue(sent.none { it.startsWith("b0") })
    }

    @Test
    fun holdingStillDrags() {
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        idle(500)
        touch.onTouch(ev(MotionEvent.ACTION_UP, 600))
        assertEquals(0, folds)
        assertEquals(listOf("b0 down", "b0 up"), sent)
    }

    @Test
    fun twoFingersStillRightClick() {
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.onTouch(twoFingers(MotionEvent.ACTION_POINTER_DOWN or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), 20))
        touch.onTouch(twoFingers(MotionEvent.ACTION_POINTER_UP or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), 120))
        touch.onTouch(ev(MotionEvent.ACTION_UP, 140))
        assertEquals(0, folds)
        assertEquals(listOf("b1 down", "b1 up"), sent)
    }

    @Test
    fun directTouch_aTapFolds() {
        touch.direct = true
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.onTouch(ev(MotionEvent.ACTION_UP, 120))
        assertEquals(1, folds)
        assertEquals(emptyList<String>(), sent)
    }

    @Test
    fun directTouch_aSlideDragsFromWhereItStarted() {
        touch.direct = true
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.onTouch(ev(MotionEvent.ACTION_MOVE, 40, 160f, 100f))
        touch.onTouch(ev(MotionEvent.ACTION_UP, 200, 160f, 100f))
        assertEquals(0, folds)
        assertEquals(listOf("to 100,100", "b0 down", "to 160,100", "b0 up"), sent)
    }

    @Test
    fun directTouch_holdingStillPresses() {
        touch.direct = true
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        idle(400)
        touch.onTouch(ev(MotionEvent.ACTION_UP, 500))
        assertEquals(0, folds)
        assertEquals(listOf("to 100,100", "b0 down", "b0 up"), sent)
    }

    // Un invité à qui l'hôte retire le clavier et la souris au milieu d'un geste (KyberNow Share) : rien ne repart après.
    @Test
    fun cancel_aHoldInProgressNeverDrags() {
        keyboard = false
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.cancel()
        idle(600)
        assertEquals(emptyList<String>(), sent)
    }

    @Test
    fun cancel_aDirectHoldInProgressNeverPresses() {
        touch.direct = true
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.cancel()
        idle(600)
        assertEquals(emptyList<String>(), sent)
    }

    @Test
    fun cancel_thenANewGestureWorksAsUsual() {
        keyboard = false
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 0))
        touch.cancel()
        touch.onTouch(ev(MotionEvent.ACTION_DOWN, 1000))
        touch.onTouch(ev(MotionEvent.ACTION_UP, 1100))
        assertEquals(listOf("b0 down", "b0 up"), sent)
    }
}
