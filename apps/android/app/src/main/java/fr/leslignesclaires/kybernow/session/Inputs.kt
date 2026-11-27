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
    const val Q = 0x10
    const val FIVE = 0x06
    val F = intArrayOf(0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x57, 0x58)

    fun press(h: Long, vararg keys: Int) {
        keys.forEach { Native.inputKey(h, it, true) }
        keys.reversed().forEach { Native.inputKey(h, it, false) }
    }
}

/**
 * Raccourcis du menu Touches, envoyés tels quels (sans l'échange de « Ctrl agit comme ⌘ »). Face à une Station Mac,
 * ceux du Mac ([MAC]) : la touche ⊞ y arrive en ⌘ (docs/link-ui.md, « Face à une Station Mac »).
 */
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
    MAC_SWITCH_APP(intArrayOf(Sc.WIN, Sc.TAB)),
    MAC_SPOTLIGHT(intArrayOf(Sc.WIN, Sc.SPACE)),
    MAC_LOCK(intArrayOf(Sc.CTRL, Sc.WIN, Sc.Q)),
    MAC_SCREENSHOT(intArrayOf(Sc.SHIFT, Sc.WIN, Sc.FIVE)),
    MAC_FORCE_QUIT(intArrayOf(Sc.ALT, Sc.WIN, Sc.ESC));

    companion object {
        /** Le menu Touches d'un PC, dans l'ordre de docs/link-ui.md (barre de session, « Touches »). */
        val PC = listOf(CTRL_ALT_DEL, WIN, ALT_TAB, PRINT, WIN_D, WIN_L, ALT_F4, ESC, TASK_MANAGER, F11)
        /** Face à une Station Mac, à la place : ⌘ ⇥, ⌘ Espace, ⌃ ⌘ Q, ⇧ ⌘ 5, ⌥ ⌘ Échap. */
        val MAC = listOf(MAC_SWITCH_APP, MAC_SPOTLIGHT, MAC_LOCK, MAC_SCREENSHOT, MAC_FORCE_QUIT)

        /** Le menu Touches de cet ordinateur ; un invité et un ami qui emprunte ont celui d'un PC ([mac] faux). */
        fun menu(mac: Boolean) = if (mac) MAC else PC

        /** La dernière touche de la rangée du clavier de l'écran : Ctrl+Alt+Suppr, ⌥ ⌘ Échap face à un Mac. */
        fun rowKey(mac: Boolean) = if (mac) MAC_FORCE_QUIT else CTRL_ALT_DEL
    }
}

/**
 * Les modificateurs collants de la rangée du clavier de l'écran et leur nom : « Ctrl », « Alt », « ⊞ », ou face à une
 * Station Mac « ⌃ », « ⌥ », « ⌘ » (mêmes touches envoyées).
 */
fun stickyKeys(mac: Boolean): List<Pair<Int, String>> =
    if (mac) listOf(Sc.CTRL to "⌃", Sc.ALT to "⌥", Sc.WIN to "⌘") else listOf(Sc.CTRL to "Ctrl", Sc.ALT to "Alt", Sc.WIN to "⊞")

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

    private fun slot(deviceId: Int): Int = gamepads.slotOf(deviceId)

    /** Une touche de manette ; `true` si elle est prise (rien d'autre à faire). */
    fun key(e: KeyEvent): Boolean {
        if (e.repeatCount > 0) return true
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
        if (captured) return false
        val s = slot(e.deviceId)
        // Un appareil qui n'est pas une manette : son mouvement n'est plus avalé (AND-25).
        if (s < 0) return gamepads.byDevice(e.deviceId) != null
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
 * L1 / R1 = Alt + Tab (Maj en plus avec L1). Face à une Station Mac ([mac]) :
 * L1 / R1 tiennent ⌘ au lieu d'Alt (⇧ ⌘ ⇥ / ⌘ ⇥) et Start envoie ⌘ Espace
 * (docs/link-ui.md, « Face à une Station Mac »).
 */
class PadMouse(private val handle: () -> Long) {
    private val main = Handler(Looper.getMainLooper())
    private var axes = PadAxes.REST
    private var running = false
    var hostWidth = 1920.0
    /** L'ordinateur est une Station Mac (jamais pour un invité ni un ami qui emprunte). */
    var mac = false
    /** La touche tenue pour passer d'une fenêtre à l'autre (Alt, ou ⌘ face à un Mac) ; 0 : aucune. */
    private var switcherHeld = 0
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
            // Sticks au repos, molette vide : la boucle s'arrête (125 réveils par seconde du fil principal sinon,
            // AND-27) ; `axes` la relance au prochain mouvement.
            if (x == 0.0 && y == 0.0 && wy == 0.0) running = false
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
            KeyEvent.KEYCODE_BUTTON_START -> Sc.press(h, *startKeys(mac))
            KeyEvent.KEYCODE_BUTTON_SELECT -> Sc.press(h, Sc.ESC)
            else -> {
                if (switcherHeld == 0) { switcherHeld = switcherKey(mac); Native.inputKey(h, switcherHeld, true) }
                if (code == KeyEvent.KEYCODE_BUTTON_L1) Native.inputKey(h, Sc.SHIFT, true)
                Sc.press(h, Sc.TAB)
                if (code == KeyEvent.KEYCODE_BUTTON_L1) Native.inputKey(h, Sc.SHIFT, false)
                main.removeCallbacks(releaseAlt)
                main.postDelayed(releaseAlt, 900)
            }
        }
    }

    /** La touche tenue se relâche telle qu'elle est partie, un instant après la dernière pression. */
    private val releaseAlt = Runnable {
        if (switcherHeld != 0) { Native.inputKey(handle(), switcherHeld, false); switcherHeld = 0 }
    }

    companion object {
        /** La touche que L1 / R1 tiennent : Alt (Alt + Tab), ⌘ face à un Mac (⌘ ⇥, la touche ⊞ pour Kyber). */
        fun switcherKey(mac: Boolean) = if (mac) Sc.WIN else Sc.ALT

        /** Ce que Start envoie : la touche ⊞ seule, « ⌘ Espace » (Spotlight) face à un Mac. */
        fun startKeys(mac: Boolean) = if (mac) intArrayOf(Sc.WIN, Sc.SPACE) else intArrayOf(Sc.WIN)
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

/** Ce que le toucher envoie à l'ordinateur : le cœur ([native]), ou un relevé dans les tests. */
interface PointerOut {
    fun moveTo(x: Double, y: Double)
    fun moveBy(dx: Double, dy: Double)
    /** 0 gauche, 1 droit, 2 milieu. */
    fun button(button: Int, pressed: Boolean)
    fun wheel(dx: Float, dy: Float)

    companion object {
        fun native(handle: () -> Long) = object : PointerOut {
            override fun moveTo(x: Double, y: Double) = Native.pointerMoveTo(handle(), x, y)
            override fun moveBy(dx: Double, dy: Double) = Native.pointerMoveBy(handle(), dx, dy)
            override fun button(button: Int, pressed: Boolean) = Native.inputMouseButton(handle(), button, pressed)
            override fun wheel(dx: Float, dy: Float) = Native.inputWheel(handle(), dx, dy)
        }
    }
}

/** Ce que le stylet envoie à l'ordinateur : le cœur ([native]), ou un relevé dans les tests. */
interface PenOut {
    /** État complet du stylet ; `false` : il part en souris (voir [Native.inputPen]). */
    fun pen(x: Double, y: Double, s: PenSample): Boolean

    companion object {
        fun native(handle: () -> Long) = object : PenOut {
            override fun pen(x: Double, y: Double, s: PenSample) =
                Native.inputPen(handle(), x, y, s.inRange, s.contact, s.pressure, s.hasTilt, s.tilt, s.orientation, s.eraser, s.barrel)
        }
    }
}

/** Un état du stylet lu dans un [MotionEvent]. */
data class PenSample(
    val inRange: Boolean,
    val contact: Boolean,
    val pressure: Float,
    val hasTilt: Boolean,
    val tilt: Float,
    val orientation: Float,
    val eraser: Boolean,
    val barrel: Boolean,
)

/**
 * Stylet d'Android (docs/link-ui.md, « Stylet » ; docs/stylet.md) : outil `TOOL_TYPE_STYLUS` ou `TOOL_TYPE_ERASER`.
 * Il écrit là où il touche, quel que soit le mode du doigt (pavé tactile ou toucher direct) : pression, inclinaison
 * (`AXIS_TILT`, `AXIS_ORIENTATION`), bout gomme, bouton du stylet (`BUTTON_STYLUS_PRIMARY`) et survol
 * (`ACTION_HOVER_*`). Les positions groupées d'un `ACTION_MOVE` (historique) partent toutes. Face à une Station
 * d'avant, en « Stylet : Souris » (docs/link-ui.md) ou en souris des jeux, le cœur rend `false` : souris à cet endroit,
 * clic gauche (clic droit avec le bouton du stylet tenu).
 */
class StylusInput(
    private val out: PenOut,
    private val mouse: PointerOut,
    private val toHost: (Float, Float) -> Pair<Double, Double>?,
) {
    /** Le geste en cours a commencé par le stylet : il est tout entier au stylet (un doigt posé en plus n'y fait rien). */
    private var owned = false
    /** Le stylet qui touche : son identifiant de pointeur dans le geste en cours. */
    private var pointerId = -1
    /** Le stylet est parti en souris (Station d'avant, « Stylet : Souris ») : son clic est tenu. */
    private var mouseDown = false
    /** Le bouton tenu : 0 gauche (pointe posée), 1 droit (pointe posée avec le bouton du stylet). */
    private var mouseButton = 0
    /** Le stylet survole l'image (dernier survol envoyé à l'ordinateur). */
    private var hovering = false
    /** Dernier état envoyé, et où : le départ remis à plus tard part de là. */
    private var last: Triple<Double, Double, PenSample>? = null
    private val main = Handler(Looper.getMainLooper())
    /**
     * Le stylet s'en va, un peu plus tard : Android envoie `HOVER_EXIT` juste avant `ACTION_DOWN`, et `ACTION_UP` juste
     * avant `HOVER_ENTER`. Sans ce délai, l'ordinateur verrait le stylet partir et revenir à chaque trait.
     */
    private val leave = Runnable { leaveNow() }

    /** Le geste qui commence est celui d'un stylet, ou celui en cours l'était. */
    fun claims(e: MotionEvent): Boolean = owned || (e.actionMasked == MotionEvent.ACTION_DOWN && isPen(e, 0))

    /** Contact : `ACTION_DOWN`, `MOVE`, `UP`, `CANCEL` d'un geste commencé par le stylet. */
    fun onTouch(e: MotionEvent): Boolean {
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                if (!isPen(e, 0)) return false
                main.removeCallbacks(leave)
                owned = true
                pointerId = e.getPointerId(0)
                send(e, 0, -1, contact = true, inRange = true)
            }
            MotionEvent.ACTION_MOVE -> {
                val i = e.findPointerIndex(pointerId)
                if (i < 0) return true
                for (h in 0 until e.historySize) send(e, i, h, contact = true, inRange = true)
                send(e, i, -1, contact = true, inRange = true)
            }
            MotionEvent.ACTION_POINTER_UP -> if (e.getPointerId(e.actionIndex) == pointerId) lift(e, e.actionIndex)
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                val i = e.findPointerIndex(pointerId)
                if (i >= 0) lift(e, i)
                if (e.actionMasked == MotionEvent.ACTION_CANCEL) leaveNow()
                pointerId = -1
                owned = false
            }
        }
        return true
    }

    /** Survol : `ACTION_HOVER_ENTER`, `MOVE`, `EXIT` du stylet tout près de l'écran. */
    fun onHover(e: MotionEvent): Boolean {
        if (!isPen(e, 0)) return false
        when (e.actionMasked) {
            MotionEvent.ACTION_HOVER_ENTER, MotionEvent.ACTION_HOVER_MOVE -> {
                main.removeCallbacks(leave)
                hovering = true
                for (h in 0 until e.historySize) send(e, 0, h, contact = false, inRange = true)
                send(e, 0, -1, contact = false, inRange = true)
            }
            MotionEvent.ACTION_HOVER_EXIT -> {
                hovering = false
                main.removeCallbacks(leave)
                main.postDelayed(leave, LEAVE_MS)
            }
            else -> return false
        }
        return true
    }

    /** Le survol n'est plus pour l'ordinateur (barre, menu) : le stylet qui survolait s'en va, là où il est. */
    fun away() {
        if (!hovering || pointerId >= 0) return
        hovering = false
        leaveNow()
    }

    /** Geste abandonné (droit repris) : le cœur lève déjà le stylet, rien ne repart. */
    fun cancel() {
        main.removeCallbacks(leave)
        last = null
        owned = false
        pointerId = -1
        mouseDown = false
        hovering = false
    }

    /** Levé : la pointe ne touche plus, le stylet reste tout près ; s'il ne survole pas, il s'en va peu après. */
    private fun lift(e: MotionEvent, i: Int) {
        send(e, i, -1, contact = false, inRange = true)
        pointerId = -1
        main.removeCallbacks(leave)
        main.postDelayed(leave, LEAVE_MS)
    }

    /** Le stylet s'en va, là où il était. */
    private fun leaveNow() {
        main.removeCallbacks(leave)
        val (x, y, s) = last ?: return
        last = null
        deliver(x, y, s.copy(inRange = false, contact = false, pressure = 0f))
    }

    /** L'inclinaison que l'appareil sait lire, par appareil, lue une fois (pas à chaque échantillon, AND-26). */
    private var tiltDevice = Int.MIN_VALUE
    private var tiltKnown = false
    private fun hasTilt(e: MotionEvent): Boolean {
        if (e.deviceId != tiltDevice) {
            tiltDevice = e.deviceId
            tiltKnown = e.device?.getMotionRange(MotionEvent.AXIS_TILT) != null
        }
        return tiltKnown
    }

    /** Un état du stylet (`h` : position de l'historique, −1 : l'actuelle) vers l'ordinateur, en stylet ou en souris. */
    private fun send(e: MotionEvent, i: Int, h: Int, contact: Boolean, inRange: Boolean) {
        val x = if (h < 0) e.getX(i) else e.getHistoricalX(i, h)
        val y = if (h < 0) e.getY(i) else e.getHistoricalY(i, h)
        val (hx, hy) = toHost(x, y) ?: return
        fun axis(a: Int) = if (h < 0) e.getAxisValue(a, i) else e.getHistoricalAxisValue(a, i, h)
        val pressure = if (h < 0) e.getPressure(i) else e.getHistoricalPressure(i, h)
        val sample = PenSample(
            inRange = inRange,
            contact = contact,
            pressure = pressure,
            hasTilt = hasTilt(e),
            tilt = axis(MotionEvent.AXIS_TILT),
            orientation = axis(MotionEvent.AXIS_ORIENTATION),
            eraser = e.getToolType(i) == MotionEvent.TOOL_TYPE_ERASER,
            barrel = (e.buttonState and MotionEvent.BUTTON_STYLUS_PRIMARY) != 0,
        )
        deliver(hx, hy, sample)
    }

    private fun deliver(hx: Double, hy: Double, sample: PenSample) {
        val inRange = sample.inRange
        val contact = sample.contact
        last = if (inRange) Triple(hx, hy, sample) else null
        if (out.pen(hx, hy, sample)) return
        if (!inRange && !mouseDown) return
        mouse.moveTo(hx, hy)
        if (contact != mouseDown) {
            mouseDown = contact
            // Pointe posée : clic gauche ; avec le bouton du stylet tenu : clic droit. Relâché : celui qui était tenu.
            if (contact) mouseButton = if (sample.barrel) 1 else 0
            mouse.button(mouseButton, contact)
        }
    }

    companion object {
        private const val LEAVE_MS = 120L

        /** Le premier pointeur de l'événement est un stylet (pointe ou gomme). */
        fun isPen(e: MotionEvent): Boolean = isPen(e, 0)

        private fun isPen(e: MotionEvent, i: Int): Boolean {
            if (i < 0 || i >= e.pointerCount) return false
            val t = e.getToolType(i)
            return t == MotionEvent.TOOL_TYPE_STYLUS || t == MotionEvent.TOOL_TYPE_ERASER
        }
    }
}

/**
 * Au doigt. Pavé tactile : un doigt déplace le pointeur de l'ordinateur,
 * toucher = clic, deux doigts = clic droit, deux doigts qui glissent =
 * molette, maintenir puis glisser = glisser-déposer. Toucher direct : le
 * doigt est le pointeur.
 *
 * Clavier de l'écran sorti ([keyboardOpen]) : un toucher sur l'image le replie ([onFold]) et ne
 * clique pas. Les autres gestes gardent leur effet : glisser, deux doigts, maintenir (en toucher
 * direct, le clic part dès que le doigt glisse ou reste posé, depuis l'endroit touché).
 */
class TouchInput(private val out: PointerOut, private val toHost: (Float, Float) -> Pair<Double, Double>?, private val hostScale: () -> Double) {
    var direct = false
    var keyboardOpen: () -> Boolean = { false }
    var onFold: () -> Unit = {}
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
    /** Un deuxième doigt s'est posé : le geste n'est pas un toucher (pas de clic gauche au lever). */
    private var multi = false
    /** Ce doigt peut replier le clavier : rien n'est encore parti à l'ordinateur. */
    private var folding = false
    /** Toucher direct en attente (clavier sorti) : où le clic partira. */
    private var directTarget: Pair<Double, Double>? = null
    // Dernier toucher en toucher direct (position à l'écran, position sur
    // l'ordinateur, heure) : un second toucher rapide et proche reprend sa
    // position. Piège : Windows ne compte un double clic que si le second clic
    // tombe à 4 pixels du premier au plus, et deux touchers d'un doigt ne
    // tombent jamais au même endroit : sans cela, le double clic ne marche pas.
    private var lastTapX = 0f
    private var lastTapY = 0f
    private var lastTapHost: Pair<Double, Double>? = null
    private var lastTapAt = 0L
    private val main = Handler(Looper.getMainLooper())
    private val holdToDrag = Runnable {
        if (!moved && fingers == 1) {
            dragging = true
            folding = false
            out.button(0, true)
        }
    }
    /** Clavier sorti, toucher direct : le doigt resté posé devient le pointeur, bouton enfoncé. */
    private val holdDirect = Runnable { if (folding && !moved && fingers == 1) startDirect() }

    private fun startDirect() {
        folding = false
        directTarget?.let { (x, y) -> out.moveTo(x, y) }
        out.button(0, true)
        dragging = true
    }

    /**
     * Le geste en cours est abandonné sans rien envoyer : les minuteries du maintien (glisser-déposer, toucher direct)
     * ne partent pas, le doigt qui reste posé n'est plus un geste. Un invité à qui l'hôte retire le clavier et la souris
     * au milieu d'un geste : le cœur relâche déjà ce qui était tenu, rien ne doit repartir après.
     */
    fun cancel() {
        main.removeCallbacks(holdToDrag)
        main.removeCallbacks(holdDirect)
        moved = false; fingers = 0; twoFingerTap = false; dragging = false; multi = false; folding = false
        directTarget = null
        lastTapAt = 0L
        scrollAcc = 0.0
    }

    fun onTouch(e: MotionEvent): Boolean {
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                downAt = e.eventTime
                downX = e.x; downY = e.y; lastX = e.x; lastY = e.y
                moved = false; fingers = 1; twoFingerTap = false; dragging = false; multi = false
                folding = keyboardOpen()
                if (direct) {
                    val near = e.eventTime - lastTapAt < 500 && hypot(e.x - lastTapX, e.y - lastTapY) < 64
                    val target = if (near) lastTapHost else toHost(e.x, e.y)
                    lastTapHost = target
                    directTarget = target
                    if (folding) main.postDelayed(holdDirect, HOLD_DIRECT_MS) else startDirect()
                } else {
                    main.postDelayed(holdToDrag, 450)
                }
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                fingers = e.pointerCount
                multi = true
                folding = false
                main.removeCallbacks(holdDirect)
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
                        out.wheel(0f, scrollAcc.toInt().toFloat())
                        scrollAcc -= scrollAcc.toInt()
                    }
                    return true
                }
                if (direct) {
                    // Le tremblement du doigt ne déplace pas le pointeur : il ne
                    // suit qu'une fois le doigt vraiment parti (glisser).
                    if (!moved && hypot(e.x - downX, e.y - downY) > 12) moved = true
                    if (moved && folding) {
                        main.removeCallbacks(holdDirect)
                        startDirect()
                    }
                    if (moved) toHost(e.x, e.y)?.let { (x, y) -> out.moveTo(x, y) }
                    return true
                }
                val dx = e.x - lastX
                val dy = e.y - lastY
                lastX = e.x; lastY = e.y
                if (hypot(e.x - downX, e.y - downY) > 12) {
                    moved = true
                    folding = false
                    if (!dragging) main.removeCallbacks(holdToDrag)
                }
                // Accélération : lent pour viser, rapide pour traverser l'écran.
                val speed = hypot(dx, dy)
                val gain = (1.2 + max(0f, speed - 4f) * 0.12) * hostScale()
                out.moveBy(dx * gain, dy * gain)
            }
            MotionEvent.ACTION_POINTER_UP -> {
                if (fingers == 2 && twoFingerTap && e.eventTime - downAt < 300) {
                    out.button(1, true)
                    out.button(1, false)
                }
                fingers = e.pointerCount - 1
                twoFingerTap = false
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                main.removeCallbacks(holdToDrag)
                main.removeCallbacks(holdDirect)
                if (folding && !dragging && fingers <= 1) {
                    // Le toucher replie le clavier, et rien d'autre.
                    folding = false
                    lastTapAt = 0L
                    fingers = 0
                    if (e.actionMasked == MotionEvent.ACTION_UP) onFold()
                    return true
                }
                if (direct && !moved && !multi) {
                    lastTapX = e.x; lastTapY = e.y; lastTapAt = e.eventTime
                } else if (direct) {
                    lastTapAt = 0L
                }
                if (dragging) {
                    out.button(0, false)
                } else if (!moved && !multi && e.eventTime - downAt < 250) {
                    out.button(0, true)
                    out.button(0, false)
                }
                dragging = false
                fingers = 0
            }
        }
        return true
    }

    private companion object {
        /** Clavier sorti, toucher direct : au-delà, le doigt posé n'est plus un toucher, il clique et tient. */
        const val HOLD_DIRECT_MS = 350L
    }
}

/**
 * Souris capturée (déplacements bruts) : les échantillons regroupés par Android additionnés, et les fractions de
 * pixel gardées pour le déplacement suivant (AND-13), pour qu'un mouvement lent ne s'annule pas à l'arrondi.
 */
class RelativeMotion {
    private var x = 0f
    private var y = 0f

    fun add(dx: Float, dy: Float) {
        x += dx
        y += dy
    }

    /** Les pixels entiers accumulés (s'il y en a), le reste gardé. */
    inline fun take(send: (Int, Int) -> Unit) {
        val ix = wholeX()
        val iy = wholeY()
        if (ix != 0 || iy != 0) send(ix, iy)
    }

    @PublishedApi internal fun wholeX(): Int = x.toInt().also { x -= it }
    @PublishedApi internal fun wholeY(): Int = y.toInt().also { y -= it }

    fun reset() {
        x = 0f
        y = 0f
    }
}

/**
 * Les dernières formes du pointeur, par contenu (taille, point chaud, pixels) : en bureau Windows, la forme change à
 * chaque survol d'un lien ou d'un texte, et chaque changement refaisait une image (AND-26).
 */
class ShapeCache<T>(private val size: Int) {
    private class Key(val w: Int, val h: Int, val hx: Int, val hy: Int, val pixels: IntArray, val hash: Int) {
        override fun hashCode() = hash
        override fun equals(other: Any?) = other is Key && other.hash == hash && other.w == w && other.h == h &&
            other.hx == hx && other.hy == hy && other.pixels.contentEquals(pixels)
    }

    private val map = object : LinkedHashMap<Key, T>(size, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, T>?) = this.size > this@ShapeCache.size
    }

    @Synchronized
    fun get(w: Int, h: Int, hotX: Int, hotY: Int, pixels: IntArray, make: () -> T): T {
        val key = Key(w, h, hotX, hotY, pixels, (((w * 31 + h) * 31 + hotX) * 31 + hotY) * 31 + pixels.contentHashCode())
        return map.getOrPut(key, make)
    }

    @Synchronized
    fun count() = map.size
}

/**
 * Le pavé tactile d'une DualSense vers la console (docs/playstation.md) : Android le voit comme un appareil à part
 * (`SOURCE_TOUCHPAD`, même fabricant et même produit que la manette). Deux doigts au plus, positions ramenées à 0..1,
 * et le clic du pavé.
 */
class PadTouchpad(
    private val gamepads: Gamepads,
    private val touch: (slot: Int, finger: Int, down: Boolean, x: Float, y: Float) -> Unit,
    private val click: (slot: Int, pressed: Boolean) -> Unit,
) {
    private var clicked = false

    fun motion(e: MotionEvent): Boolean {
        if (!e.isFromSource(android.view.InputDevice.SOURCE_TOUCHPAD)) return false
        val d = e.device ?: return false
        val slot = gamepads.pads.value.filter { it.vendorId == d.vendorId && it.productId == d.productId && it.slot >= 0 }.singleOrNull()?.slot ?: return false
        val rx = d.getMotionRange(MotionEvent.AXIS_X, e.source)
        val ry = d.getMotionRange(MotionEvent.AXIS_Y, e.source)
        val ending = when (e.actionMasked) {
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> -2
            MotionEvent.ACTION_POINTER_UP -> e.actionIndex
            else -> -1
        }
        for (i in 0 until e.pointerCount) {
            val finger = e.getPointerId(i)
            if (finger > 1) continue
            val down = ending != -2 && ending != i
            touch(slot, finger, down, normalize(e.getX(i), rx?.min ?: 0f, rx?.max ?: 0f), normalize(e.getY(i), ry?.min ?: 0f, ry?.max ?: 0f))
        }
        val now = e.buttonState and MotionEvent.BUTTON_PRIMARY != 0
        if (now != clicked) {
            clicked = now
            click(slot, now)
        }
        return true
    }

    companion object {
        fun normalize(v: Float, min: Float, max: Float): Float = if (max > min) ((v - min) / (max - min)).coerceIn(0f, 1f) else 0f
    }
}
