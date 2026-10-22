package fr.leslignesclaires.kybernow.session

import android.os.Handler
import android.os.Looper
import android.view.KeyEvent
import android.view.MotionEvent
import fr.leslignesclaires.kybernow.core.Native
import fr.leslignesclaires.kybernow.core.Pad
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.PadAxes
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.pow
import kotlin.math.sign

/** Scancodes Windows (jeu 1, préfixe E0 pour les touches étendues), ceux qu'attend kynput. */
object Sc {
    const val ESC = 0x01
    const val TAB = 0x0F
    const val ENTER = 0x1C
    const val BACKSPACE = 0x0E
    const val SPACE = 0x39
    const val CTRL = 0x1D
    const val SHIFT = 0x2A
    const val ALT = 0x38
    const val WIN = 0xE05B
    const val DELETE = 0xE053
    const val UP = 0xE048
    const val DOWN = 0xE050
    const val LEFT = 0xE04B
    const val RIGHT = 0xE04D
    const val HOME = 0xE047
    const val END = 0xE04F
    const val PAGE_UP = 0xE049
    const val PAGE_DOWN = 0xE051
    const val PRINT = 0xE037
    const val D = 0x20
    const val L = 0x26
    val F = intArrayOf(0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x57, 0x58)

    fun press(h: Long, vararg keys: Int) {
        keys.forEach { Native.inputKey(h, it, true) }
        keys.reversed().forEach { Native.inputKey(h, it, false) }
    }
}

/** Raccourcis du menu Touches. */
enum class Shortcut(val keys: IntArray) {
    CTRL_ALT_DEL(intArrayOf(Sc.CTRL, Sc.ALT, Sc.DELETE)),
    ALT_TAB(intArrayOf(Sc.ALT, Sc.TAB)),
    WIN(intArrayOf(Sc.WIN)),
    WIN_D(intArrayOf(Sc.WIN, Sc.D)),
    WIN_L(intArrayOf(Sc.WIN, Sc.L)),
    ALT_F4(intArrayOf(Sc.ALT, 0x3E)),
    PRINT(intArrayOf(Sc.PRINT)),
    TASK_MANAGER(intArrayOf(Sc.CTRL, Sc.SHIFT, Sc.ESC)),
    F11(intArrayOf(0x57)),
    ESC(intArrayOf(Sc.ESC)),
}

/**
 * Les manettes vers l'ordinateur : chaque manette a sa place (J1 à J4) ; les
 * touches et les axes partent au cœur (axes filtrés et regroupés par 4 ms
 * là-bas). Le menu prend les manettes : plus rien ne part au jeu.
 */
class PadRouter(private val handle: () -> Long, private val gamepads: Gamepads) {
    /** Le menu est ouvert : les manettes pilotent l'app. */
    var captured = false
    /** Bouton Guide : `game`, `long` (Start tenu 600 ms), `system`. */
    var guide = "game"
    /** La manette pilote le pointeur (« Manette en souris »). */
    var asMouse = false
    private val last = HashMap<Int, PadAxes>()
    private val main = Handler(Looper.getMainLooper())
    private var guideFromStart = false
    private val longGuide = Runnable {
        guideFromStart = true
        val slot = startSlot
        if (slot >= 0) Native.padButton(handle(), slot, Pad.GUIDE, true)
    }
    private var startSlot = -1
    val mouse = PadMouse(handle)

    private fun slot(deviceId: Int): Int = gamepads.byDevice(deviceId)?.slot ?: run { gamepads.refresh(); gamepads.byDevice(deviceId)?.slot ?: -1 }

    /** Une touche de manette ; `true` si elle est prise (rien d'autre à faire). */
    fun key(e: KeyEvent): Boolean {
        if (e.repeatCount > 0) return true
        gamepads.noteUsed(e.deviceId)
        val h = handle()
        val down = e.action == KeyEvent.ACTION_DOWN
        if (captured) return false
        if (e.keyCode == KeyEvent.KEYCODE_BUTTON_MODE && guide == "system") return false
        val s = slot(e.deviceId)
        if (s < 0) return true
        if (asMouse) return mouse.key(e)
        val trigger = Gamepads.digitalTrigger(e.keyCode)
        if (trigger >= 0) {
            Native.padAxis(h, s, trigger, if (down) 32767 else 0)
            return true
        }
        val b = Gamepads.button(e.keyCode)
        if (b < 0) return false
        // Appui long : Start tenu 600 ms part comme Guide tant qu'on le tient.
        if (guide == "long" && b == Pad.START) {
            if (down) {
                startSlot = s
                guideFromStart = false
                main.postDelayed(longGuide, 600)
            } else {
                main.removeCallbacks(longGuide)
                if (guideFromStart) {
                    Native.padButton(h, s, Pad.GUIDE, false)
                } else {
                    Native.padButton(h, s, Pad.START, true)
                    main.postDelayed({ Native.padButton(handle(), s, Pad.START, false) }, 50)
                }
            }
            return true
        }
        Native.padButton(h, s, b, down)
        return true
    }

    /** Sticks, gâchettes et croix en chapeau. */
    fun motion(e: MotionEvent): Boolean {
        gamepads.noteUsed(e.deviceId)
        if (captured) return false
        val s = slot(e.deviceId)
        if (s < 0) return true
        val now = PadAxes.read(e)
        val before = last[e.deviceId] ?: PadAxes.REST
        last[e.deviceId] = now
        if (asMouse) {
            mouse.axes(now)
            return true
        }
        val h = handle()
        for (axis in Pad.LEFT_X..Pad.R2) {
            val v = now.value(axis)
            if (v != before.value(axis)) Native.padAxis(h, s, axis, v)
        }
        // Croix en chapeau (beaucoup de manettes Bluetooth) : quatre boutons.
        if (now.hatX != before.hatX) {
            if (before.hatX < 0) Native.padButton(h, s, Pad.LEFT, false)
            if (before.hatX > 0) Native.padButton(h, s, Pad.RIGHT, false)
            if (now.hatX < 0) Native.padButton(h, s, Pad.LEFT, true)
            if (now.hatX > 0) Native.padButton(h, s, Pad.RIGHT, true)
        }
        if (now.hatY != before.hatY) {
            if (before.hatY < 0) Native.padButton(h, s, Pad.UP, false)
            if (before.hatY > 0) Native.padButton(h, s, Pad.DOWN, false)
            if (now.hatY < 0) Native.padButton(h, s, Pad.UP, true)
            if (now.hatY > 0) Native.padButton(h, s, Pad.DOWN, true)
        }
        return true
    }

    /** Le menu prend les manettes : l'ordinateur relâche tout (touches, sticks, gâchettes). */
    fun capture() {
        captured = true
        val h = handle()
        gamepads.pads.value.filter { it.slot >= 0 }.forEach { Native.padAxesRest(h, it.slot) }
        Native.inputReleaseAll(h)
        mouse.stop()
    }

    fun release() {
        captured = false
        last.clear()
    }

    /** Places annoncées à l'ordinateur : place → manette (descripteur). */
    private val plugged = HashMap<Int, String>()

    /** Entrées reliées (début, reconnexion, relance) : chaîne neuve, toutes les manettes s'annoncent. */
    fun reconnected() {
        plugged.clear()
        gamepads.refresh()
        sync()
    }

    /**
     * Les manettes branchées et débranchées depuis la dernière fois : une
     * annonce par arrivée (le cœur ne dédoublonne pas), un retrait par départ.
     * Rend les manettes nouvellement annoncées.
     */
    fun sync(): List<fr.leslignesclaires.kybernow.model.Gamepad> {
        val h = handle()
        val now = gamepads.pads.value.filter { it.slot >= 0 }.associateBy { it.slot }
        for ((slot, desc) in plugged.toMap()) {
            if (now[slot]?.descriptor != desc) {
                Native.padUnplug(h, slot)
                plugged.remove(slot)
            }
        }
        val added = now.values.filter { plugged[it.slot] == null }
        added.forEach {
            Native.padPlug(h, it.slot, it.vendorId, it.productId)
            plugged[it.slot] = it.descriptor
        }
        return added
    }
}

/**
 * Manette en souris : stick gauche = pointeur (accélération progressive,
 * l'écran traversé en 1,5 s à fond), LT pour la précision, RT pour aller
 * vite ; stick droit = molette ; A clic gauche, X clic droit, B clic du
 * milieu ; croix = flèches ; Start = touche Windows, Select = Échap ;
 * L1 / R1 = Alt + Tab (Maj en plus avec L1).
 */
class PadMouse(private val handle: () -> Long) {
    private val main = Handler(Looper.getMainLooper())
    private var axes = PadAxes.REST
    private var running = false
    var hostWidth = 1920.0
    private var altHeld = false
    private var wheelAcc = 0.0
    private val tick = object : Runnable {
        override fun run() {
            val h = handle()
            val (x, y) = curve(axes.lx) to curve(axes.ly)
            if (x != 0.0 || y != 0.0) {
                val boost = 1.0 + axes.r2 / 32767.0 * 1.5 - axes.l2 / 32767.0 * 0.7
                val perTick = hostWidth / (1.5 * 120) * boost
                Native.pointerMoveBy(h, x * perTick, y * perTick)
            }
            val wy = curve(axes.ry)
            if (wy != 0.0) {
                wheelAcc += wy * 0.25
                if (abs(wheelAcc) >= 1) {
                    Native.inputWheel(h, 0f, wheelAcc.toInt().toFloat())
                    wheelAcc -= wheelAcc.toInt()
                }
            }
            if (running) main.postDelayed(this, 8)
        }
    }

    private fun curve(v: Int): Double {
        val n = v / 32767.0
        if (abs(n) < 0.12) return 0.0
        return sign(n) * ((abs(n) - 0.12) / 0.88).pow(2.2)
    }

    fun axes(a: PadAxes) {
        axes = a
        if (!running) {
            running = true
            main.post(tick)
        }
    }

    /** L1 + R1 + Start + Select tenus ensemble : le menu (comme hors du mode souris). */
    var onCombo: (() -> Unit)? = null
    /** Boutons du combo tenus ; `mixed` : plusieurs à la fois depuis le dernier relâché complet. */
    private val held = mutableSetOf<Int>()
    private var mixed = false

    fun key(e: KeyEvent): Boolean {
        val h = handle()
        val down = e.action == KeyEvent.ACTION_DOWN
        // Select et Start arrivent parfois en Retour et Menu (manette Xbox en Bluetooth).
        val code = when (e.keyCode) {
            KeyEvent.KEYCODE_BACK -> KeyEvent.KEYCODE_BUTTON_SELECT
            KeyEvent.KEYCODE_MENU -> KeyEvent.KEYCODE_BUTTON_START
            else -> e.keyCode
        }
        when (code) {
            KeyEvent.KEYCODE_BUTTON_A -> Native.inputMouseButton(h, 0, down)
            KeyEvent.KEYCODE_BUTTON_X -> Native.inputMouseButton(h, 1, down)
            KeyEvent.KEYCODE_BUTTON_B -> Native.inputMouseButton(h, 2, down)
            KeyEvent.KEYCODE_DPAD_UP -> Native.inputKey(h, Sc.UP, down)
            KeyEvent.KEYCODE_DPAD_DOWN -> Native.inputKey(h, Sc.DOWN, down)
            KeyEvent.KEYCODE_DPAD_LEFT -> Native.inputKey(h, Sc.LEFT, down)
            KeyEvent.KEYCODE_DPAD_RIGHT -> Native.inputKey(h, Sc.RIGHT, down)
            // Start, Select, L1, R1 : action au relâché, sauf s'ils ont servi au combo.
            KeyEvent.KEYCODE_BUTTON_START, KeyEvent.KEYCODE_BUTTON_SELECT, KeyEvent.KEYCODE_BUTTON_L1, KeyEvent.KEYCODE_BUTTON_R1 -> comboKey(h, code, down)
        }
        // Les autres boutons ne font rien (pas de Retour ni d'OK d'Android).
        return true
    }

    private fun comboKey(h: Long, code: Int, down: Boolean) {
        if (down) {
            held += code
            if (held.size > 1) mixed = true
            if (held.size == 4) onCombo?.invoke()
            return
        }
        if (!held.remove(code)) return
        val solo = !mixed
        if (held.isEmpty()) mixed = false
        if (!solo) return
        when (code) {
            KeyEvent.KEYCODE_BUTTON_START -> Sc.press(h, Sc.WIN)
            KeyEvent.KEYCODE_BUTTON_SELECT -> Sc.press(h, Sc.ESC)
            else -> {
                if (!altHeld) { Native.inputKey(h, Sc.ALT, true); altHeld = true }
                if (code == KeyEvent.KEYCODE_BUTTON_L1) Native.inputKey(h, Sc.SHIFT, true)
                Sc.press(h, Sc.TAB)
                if (code == KeyEvent.KEYCODE_BUTTON_L1) Native.inputKey(h, Sc.SHIFT, false)
                main.removeCallbacks(releaseAlt)
                main.postDelayed(releaseAlt, 900)
            }
        }
    }

    private val releaseAlt = Runnable {
        if (altHeld) { Native.inputKey(handle(), Sc.ALT, false); altHeld = false }
    }

    fun stop() {
        held.clear()
        mixed = false
        running = false
        axes = PadAxes.REST
        main.removeCallbacks(tick)
        releaseAlt.run()
    }
}

/**
 * Télécommande : par défaut, la croix envoie les flèches et le centre
 * Entrée ; en souris, la croix déplace le pointeur (accélère si on la
 * tient), le centre clique, Lecture/Pause fait le clic droit.
 */
class RemoteRouter(private val handle: () -> Long) {
    var asMouse = false
    var hostWidth = 1920.0

    fun key(e: KeyEvent): Boolean {
        val h = handle()
        val down = e.action == KeyEvent.ACTION_DOWN
        if (asMouse) {
            val step = hostWidth / 60 * (1 + minOf(e.repeatCount, 20) / 4.0)
            when (e.keyCode) {
                KeyEvent.KEYCODE_DPAD_UP -> if (down) Native.pointerMoveBy(h, 0.0, -step)
                KeyEvent.KEYCODE_DPAD_DOWN -> if (down) Native.pointerMoveBy(h, 0.0, step)
                KeyEvent.KEYCODE_DPAD_LEFT -> if (down) Native.pointerMoveBy(h, -step, 0.0)
                KeyEvent.KEYCODE_DPAD_RIGHT -> if (down) Native.pointerMoveBy(h, step, 0.0)
                KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER -> if (e.repeatCount == 0) Native.inputMouseButton(h, 0, down)
                KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE -> if (e.repeatCount == 0) Native.inputMouseButton(h, 1, down)
                else -> return false
            }
            return true
        }
        val sc = when (e.keyCode) {
            KeyEvent.KEYCODE_DPAD_UP -> Sc.UP
            KeyEvent.KEYCODE_DPAD_DOWN -> Sc.DOWN
            KeyEvent.KEYCODE_DPAD_LEFT -> Sc.LEFT
            KeyEvent.KEYCODE_DPAD_RIGHT -> Sc.RIGHT
            KeyEvent.KEYCODE_DPAD_CENTER -> Sc.ENTER
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE -> Sc.SPACE
            else -> return false
        }
        Native.inputKey(h, sc, down)
        return true
    }
}

/**
 * Au doigt. Pavé tactile : un doigt déplace le pointeur de l'ordinateur,
 * toucher = clic, deux doigts = clic droit, deux doigts qui glissent =
 * molette, maintenir puis glisser = glisser-déposer. Toucher direct : le
 * doigt est le pointeur.
 */
class TouchInput(private val handle: () -> Long, private val toHost: (Float, Float) -> Pair<Double, Double>?, private val hostScale: () -> Double) {
    var direct = false
    private var downAt = 0L
    private var downX = 0f
    private var downY = 0f
    private var lastX = 0f
    private var lastY = 0f
    private var moved = false
    private var fingers = 0
    private var twoFingerTap = false
    private var scrollAcc = 0.0
    private var dragging = false
    private val main = Handler(Looper.getMainLooper())
    private val holdToDrag = Runnable {
        if (!moved && fingers == 1) {
            dragging = true
            Native.inputMouseButton(handle(), 0, true)
        }
    }

    fun onTouch(e: MotionEvent): Boolean {
        val h = handle()
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                downAt = e.eventTime
                downX = e.x; downY = e.y; lastX = e.x; lastY = e.y
                moved = false; fingers = 1; twoFingerTap = false; dragging = false
                if (direct) {
                    toHost(e.x, e.y)?.let { (x, y) -> Native.pointerMoveTo(h, x, y) }
                    Native.inputMouseButton(h, 0, true)
                    dragging = true
                } else {
                    main.postDelayed(holdToDrag, 450)
                }
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                fingers = e.pointerCount
                if (fingers == 2) {
                    twoFingerTap = !moved
                    lastY = (e.getY(0) + e.getY(1)) / 2
                    main.removeCallbacks(holdToDrag)
                }
            }
            MotionEvent.ACTION_MOVE -> {
                if (fingers >= 2) {
                    val y = (e.getY(0) + e.getY(1)) / 2
                    val dy = y - lastY
                    lastY = y
                    if (abs(dy) > 2) twoFingerTap = false
                    scrollAcc += -dy / 40.0
                    if (abs(scrollAcc) >= 1) {
                        Native.inputWheel(h, 0f, scrollAcc.toInt().toFloat())
                        scrollAcc -= scrollAcc.toInt()
                    }
                    return true
                }
                if (direct) {
                    toHost(e.x, e.y)?.let { (x, y) -> Native.pointerMoveTo(h, x, y) }
                    return true
                }
                val dx = e.x - lastX
                val dy = e.y - lastY
                lastX = e.x; lastY = e.y
                if (hypot(e.x - downX, e.y - downY) > 12) {
                    moved = true
                    if (!dragging) main.removeCallbacks(holdToDrag)
                }
                // Accélération : lent pour viser, rapide pour traverser l'écran.
                val speed = hypot(dx, dy)
                val gain = (1.2 + max(0f, speed - 4f) * 0.12) * hostScale()
                Native.pointerMoveBy(h, dx * gain, dy * gain)
            }
            MotionEvent.ACTION_POINTER_UP -> {
                if (fingers == 2 && twoFingerTap && e.eventTime - downAt < 300) {
                    Native.inputMouseButton(h, 1, true)
                    Native.inputMouseButton(h, 1, false)
                }
                fingers = e.pointerCount - 1
                twoFingerTap = false
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                main.removeCallbacks(holdToDrag)
                if (dragging) {
                    Native.inputMouseButton(h, 0, false)
                } else if (!moved && fingers <= 1 && e.eventTime - downAt < 250) {
                    Native.inputMouseButton(h, 0, true)
                    Native.inputMouseButton(h, 0, false)
                }
                dragging = false
                fingers = 0
            }
        }
        return true
    }
}
