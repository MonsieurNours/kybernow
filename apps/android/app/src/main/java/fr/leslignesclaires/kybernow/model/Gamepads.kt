package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.hardware.input.InputManager
import android.os.Build
import android.os.CombinedVibration
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import fr.leslignesclaires.kybernow.core.Pad
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.roundToInt

/** Une manette branchée. `slot` : joueur 0 à 3 sur l'ordinateur, −1 en attente (cinquième). */
data class Gamepad(
    val deviceId: Int,
    val descriptor: String,
    val name: String,
    val vendorId: Int,
    val productId: Int,
    /** Moteurs que l'app peut faire vibrer : 2 (Android 12 et après), 1, ou 0. */
    val motors: Int,
    /** Batterie en %, si la manette la donne. */
    val battery: Int?,
    val slot: Int,
    /** Manette intégrée à l'appareil (console portable, manette de téléphone en USB-C). */
    val builtIn: Boolean,
    /** La manette à l'écran de la session (toucher) : une manette Xbox 360 pour l'ordinateur. */
    val onScreen: Boolean = false,
) {
    val sony get() = vendorId == 0x054C
    val nintendo get() = vendorId == 0x057E
}

/**
 * Places de joueur J1 à J4, dans l'ordre d'arrivée : une manette garde la sienne tant qu'elle est
 * là, une nouvelle prend la première libre, une cinquième attend (−1) qu'une se libère (XInput,
 * côté ordinateur, n'en connaît que quatre).
 */
class PlayerSlots(size: Int = 4) {
    private val slots = arrayOfNulls<String>(size)

    /** Les manettes présentes (descripteurs, dans l'ordre où elles se présentent) → leur place, −1 en attente. */
    fun assign(present: List<String>): Map<String, Int> {
        val here = present.toSet()
        for (i in slots.indices) if (slots[i] != null && slots[i] !in here) slots[i] = null
        for (d in present) {
            if (d in slots) continue
            val free = slots.indexOfFirst { it == null }
            if (free >= 0) slots[free] = d
        }
        return present.associateWith { slots.indexOf(it) }
    }
}

/**
 * Les manettes de l'appareil : détection, places J1 à J4 ([PlayerSlots]), vibrations. La manette
 * à l'écran de la session ([setOnScreen]) y prend sa place comme une manette de plus : annoncée,
 * débranchée et rebranchée par les mêmes chemins (`PadRouter.sync`).
 */
class Gamepads(private val context: Context) : InputManager.InputDeviceListener {
    private val input = context.getSystemService(InputManager::class.java)
    private val main = Handler(Looper.getMainLooper())
    private val _pads = MutableStateFlow<List<Gamepad>>(emptyList())
    val pads: StateFlow<List<Gamepad>> = _pads.asStateFlow()

    /** Le dernier appareil de jeu utilisé (glyphes d'aide : ✕ / A, télécommande). */
    private val _lastUsed = MutableStateFlow<Gamepad?>(null)
    val lastUsed: StateFlow<Gamepad?> = _lastUsed.asStateFlow()

    private val slots = PlayerSlots()
    private var started = 0
    private var onScreen = false

    /** La manette à l'écran s'affiche (première place libre) ou se retire (sa place se libère). */
    fun setOnScreen(shown: Boolean) {
        if (onScreen == shown) return
        onScreen = shown
        refresh()
    }

    fun start() {
        if (started++ > 0) return
        input.registerInputDeviceListener(this, main)
        refresh()
    }

    fun stop() {
        if (--started > 0) return
        input.unregisterInputDeviceListener(this)
    }

    override fun onInputDeviceAdded(deviceId: Int) = refresh()
    override fun onInputDeviceRemoved(deviceId: Int) = refresh()
    override fun onInputDeviceChanged(deviceId: Int) = refresh()

    fun refresh() {
        val devices: List<InputDevice> = InputDevice.getDeviceIds().toList().mapNotNull { id: Int -> InputDevice.getDevice(id) }.filter { d -> isGamepad(d) }
        val slot = slots.assign(devices.map { it.descriptor } + listOfNotNull(ON_SCREEN.takeIf { onScreen }))
        _pads.value = (
            devices.map { d ->
                Gamepad(
                    deviceId = d.id,
                    descriptor = d.descriptor,
                    name = d.name,
                    vendorId = d.vendorId,
                    productId = d.productId,
                    motors = motors(d),
                    battery = battery(d),
                    slot = slot[d.descriptor] ?: -1,
                    builtIn = !d.isExternal,
                )
            } + listOfNotNull(if (onScreen) onScreenPad(slot[ON_SCREEN] ?: -1) else null)
            ).sortedWith(compareBy({ if (it.slot < 0) 99 else it.slot }, { it.name }))
    }

    fun byDevice(deviceId: Int): Gamepad? = _pads.value.firstOrNull { it.deviceId == deviceId }

    fun noteUsed(deviceId: Int) {
        val p = byDevice(deviceId) ?: return
        if (_lastUsed.value?.deviceId != deviceId) _lastUsed.value = p
    }

    // MARK: - Vibrations

    /** Manettes dont la première vibration est déjà dans le journal. */
    private val rumbleLogged = java.util.Collections.synchronizedSet(mutableSetOf<Int>())

    /**
     * Vibration du jeu : intensités 0 à 65535 (gros moteur grave à gauche,
     * petit aigu à droite, comme XInput). Sans durée, 5 s au plus, comme
     * kynput sur Mac ; (0, 0) l'arrête.
     */
    fun rumble(deviceId: Int, large: Int, small: Int, durationMs: Int) {
        val d = InputDevice.getDevice(deviceId) ?: return
        if (rumbleLogged.add(deviceId)) {
            android.util.Log.i("KyberNow", "vibrations : première demande pour ${d.name} (${motors(d)} vibreur(s) donné(s) par Android, API ${Build.VERSION.SDK_INT})")
        }
        val ms = (if (durationMs <= 0) 5000 else durationMs).toLong()
        val amp = { v: Int -> (v.coerceIn(0, 65535) * 255 / 65535) }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val vm = d.vibratorManager
            val ids = vm.vibratorIds
            if (ids.isEmpty()) return
            if (large == 0 && small == 0) {
                vm.cancel()
                return
            }
            val combo = CombinedVibration.startParallel()
            if (ids.size >= 2) {
                addMotor(combo, ids[0], amp(large), ms)
                addMotor(combo, ids[1], amp(small), ms)
            } else {
                addMotor(combo, ids[0], amp(max(large, small)), ms)
            }
            runCatching { vm.vibrate(combo.combine()) }
        } else {
            @Suppress("DEPRECATION")
            val v = d.vibrator
            if (!v.hasVibrator()) return
            val a = amp(max(large, small))
            if (a == 0) v.cancel() else runCatching { v.vibrate(VibrationEffect.createOneShot(ms, a)) }
        }
    }

    @androidx.annotation.RequiresApi(Build.VERSION_CODES.S)
    private fun addMotor(combo: CombinedVibration.ParallelCombination, id: Int, amplitude: Int, ms: Long) {
        // Un moteur au repos reçoit une vibration nulle (il s'arrête), pas rien.
        val effect = if (amplitude > 0) VibrationEffect.createOneShot(ms, amplitude) else VibrationEffect.createOneShot(1, 1)
        combo.addVibrator(id, effect)
    }

    /** Essai d'un moteur depuis l'écran de test : 0 gauche, 1 droit, 2 les deux. */
    fun test(deviceId: Int, which: Int) {
        val (l, s) = when (which) {
            0 -> 52000 to 0
            1 -> 0 to 52000
            else -> 52000 to 52000
        }
        rumble(deviceId, l, s, 600)
    }

    companion object {
        /** Descripteur et identifiant de la manette à l'écran (aucun appareil d'Android ne les porte). */
        const val ON_SCREEN = "kybernow:manette-a-l-ecran"
        const val ON_SCREEN_ID = Int.MIN_VALUE

        /** La manette à l'écran : une manette Xbox 360 filaire (045E:028E) pour l'ordinateur, même en « Fidèle ». */
        fun onScreenPad(slot: Int) = Gamepad(ON_SCREEN_ID, ON_SCREEN, "", 0x045E, 0x028E, 0, null, slot, builtIn = true, onScreen = true)

        fun isGamepad(d: InputDevice): Boolean {
            if (d.isVirtual) return false
            val s = d.sources
            // Une manette a des sticks (SOURCE_JOYSTICK, axes X et Y). Les
            // télécommandes et les appareils virtuels d'Android
            // (shield-ask-remote, virtual-search…) annoncent parfois
            // SOURCE_GAMEPAD et le bouton A, sans stick.
            if (s and InputDevice.SOURCE_JOYSTICK != InputDevice.SOURCE_JOYSTICK) return false
            return d.getMotionRange(MotionEvent.AXIS_X) != null && d.getMotionRange(MotionEvent.AXIS_Y) != null
        }

        fun isGamepadEvent(event: KeyEvent): Boolean {
            val d = event.device ?: return false
            return isGamepad(d) && (KeyEvent.isGamepadButton(event.keyCode) || isDpad(event.keyCode))
        }

        fun isDpad(keyCode: Int) = keyCode == KeyEvent.KEYCODE_DPAD_UP || keyCode == KeyEvent.KEYCODE_DPAD_DOWN ||
            keyCode == KeyEvent.KEYCODE_DPAD_LEFT || keyCode == KeyEvent.KEYCODE_DPAD_RIGHT

        private fun motors(d: InputDevice): Int = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            d.vibratorManager.vibratorIds.size.coerceAtMost(2)
        } else {
            @Suppress("DEPRECATION")
            if (d.vibrator.hasVibrator()) 1 else 0
        }

        private fun battery(d: InputDevice): Int? {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return null
            val b = d.batteryState
            if (!b.isPresent) return null
            val c = b.capacity
            return if (c.isNaN()) null else (c * 100).roundToInt()
        }

        /** Bouton de manette d'un `KeyEvent` (valeur de [Pad]) ; −1 sinon. */
        fun button(keyCode: Int): Int = when (keyCode) {
            KeyEvent.KEYCODE_BUTTON_A -> Pad.A
            KeyEvent.KEYCODE_BUTTON_B -> Pad.B
            KeyEvent.KEYCODE_BUTTON_X -> Pad.X
            KeyEvent.KEYCODE_BUTTON_Y -> Pad.Y
            KeyEvent.KEYCODE_BUTTON_SELECT, KeyEvent.KEYCODE_BACK -> Pad.BACK
            KeyEvent.KEYCODE_BUTTON_MODE -> Pad.GUIDE
            KeyEvent.KEYCODE_BUTTON_START, KeyEvent.KEYCODE_MENU -> Pad.START
            KeyEvent.KEYCODE_BUTTON_THUMBL -> Pad.LEFT_STICK
            KeyEvent.KEYCODE_BUTTON_THUMBR -> Pad.RIGHT_STICK
            KeyEvent.KEYCODE_BUTTON_L1 -> Pad.L1
            KeyEvent.KEYCODE_BUTTON_R1 -> Pad.R1
            KeyEvent.KEYCODE_DPAD_UP -> Pad.UP
            KeyEvent.KEYCODE_DPAD_DOWN -> Pad.DOWN
            KeyEvent.KEYCODE_DPAD_LEFT -> Pad.LEFT
            KeyEvent.KEYCODE_DPAD_RIGHT -> Pad.RIGHT
            else -> -1
        }

        /** Gâchette numérique (manettes sans axe de gâchette) : axe de [Pad], −1 sinon. */
        fun digitalTrigger(keyCode: Int): Int = when (keyCode) {
            KeyEvent.KEYCODE_BUTTON_L2 -> Pad.L2
            KeyEvent.KEYCODE_BUTTON_R2 -> Pad.R2
            else -> -1
        }
    }
}

/**
 * L'état analogique d'une manette, lu dans un `MotionEvent` : sticks
 * (−32768 à 32767, Y positif vers le bas comme SDL et XInput côté kynput),
 * gâchettes (0 à 32767) et croix en chapeau (HAT) traduite en boutons.
 */
data class PadAxes(val lx: Int, val ly: Int, val rx: Int, val ry: Int, val l2: Int, val r2: Int, val hatX: Int, val hatY: Int) {
    fun value(axis: Int) = when (axis) {
        Pad.LEFT_X -> lx
        Pad.LEFT_Y -> ly
        Pad.RIGHT_X -> rx
        Pad.RIGHT_Y -> ry
        Pad.L2 -> l2
        else -> r2
    }

    companion object {
        val REST = PadAxes(0, 0, 0, 0, 0, 0, 0, 0)

        private fun stick(e: MotionEvent, d: InputDevice, axis: Int): Int {
            val range = d.getMotionRange(axis, e.source) ?: return 0
            val v = e.getAxisValue(axis)
            // Zone morte de la manette elle-même (`flat`), au-delà la course entière.
            if (abs(v) <= range.flat) return 0
            return (v * 32767f).roundToInt().coerceIn(-32768, 32767)
        }

        private fun trigger(e: MotionEvent, d: InputDevice, a: Int, b: Int): Int {
            val v = max(
                if (d.getMotionRange(a, e.source) != null) e.getAxisValue(a) else 0f,
                if (d.getMotionRange(b, e.source) != null) e.getAxisValue(b) else 0f,
            )
            return (v.coerceIn(0f, 1f) * 32767f).roundToInt()
        }

        fun read(e: MotionEvent): PadAxes {
            val d = e.device ?: return REST
            // Stick droit : Z / RZ (la plupart), sinon RX / RY.
            val rightZ = d.getMotionRange(MotionEvent.AXIS_Z, e.source) != null && d.getMotionRange(MotionEvent.AXIS_RZ, e.source) != null
            return PadAxes(
                lx = stick(e, d, MotionEvent.AXIS_X),
                ly = stick(e, d, MotionEvent.AXIS_Y),
                rx = stick(e, d, if (rightZ) MotionEvent.AXIS_Z else MotionEvent.AXIS_RX),
                ry = stick(e, d, if (rightZ) MotionEvent.AXIS_RZ else MotionEvent.AXIS_RY),
                l2 = trigger(e, d, MotionEvent.AXIS_LTRIGGER, MotionEvent.AXIS_BRAKE),
                r2 = trigger(e, d, MotionEvent.AXIS_RTRIGGER, MotionEvent.AXIS_GAS),
                hatX = e.getAxisValue(MotionEvent.AXIS_HAT_X).roundToInt(),
                hatY = e.getAxisValue(MotionEvent.AXIS_HAT_Y).roundToInt(),
            )
        }
    }
}
