package fr.leslignesclaires.kybernow.session

import android.content.Context
import android.media.AudioAttributes
import android.os.Build
import android.os.VibrationAttributes
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.HapticFeedbackConstants
import android.view.View
import kotlin.math.max

/**
 * Le retour haptique de la manette à l'écran ([Feel]), et les vibrations du jeu qui visent sa
 * place de joueur, sur le vibreur de l'appareil.
 *
 * Trois chemins, du plus fin au plus simple, tous rapides (un appel au système, sans attente) :
 * - **primitives de composition** (clic, tic : la plupart des téléphones récents) : l'intensité
 *   se règle à chaque niveau ;
 * - sans elles, au niveau **Moyen** : les retours du système (`performHapticFeedback`), réglés
 *   par le fabricant ;
 * - aux niveaux **Faible** et **Fort** : les effets prédéfinis (tic, clic, clic appuyé).
 *
 * Le retour à l'appui suit le réglage du toucher d'Android (usage « toucher ») ; les vibrations
 * du jeu, non (usage « média »). Jamais de vibration continue au toucher : seules celles du jeu
 * durent.
 */
class Haptics(private val view: View) {
    private val vibrator: Vibrator? = vibrator(view.context)
    val available = vibrator?.hasVibrator() == true
    private val primitives = available && runCatching {
        vibrator!!.areAllPrimitivesSupported(VibrationEffect.Composition.PRIMITIVE_CLICK, VibrationEffect.Composition.PRIMITIVE_TICK)
    }.getOrDefault(false)
    private val amplitude = available && vibrator!!.hasAmplitudeControl()
    private val effects = HashMap<Feel, VibrationEffect>()

    /** 0 désactivé, 1 faible, 2 moyen, 3 fort. */
    var level = 2
        set(v) {
            field = v.coerceIn(0, 3)
            effects.clear()
        }

    fun feel(f: Feel) {
        val v = vibrator ?: return
        if (!available || level == 0) return
        runCatching {
            when {
                primitives -> play(v, effects.getOrPut(f) { composition(f) }, touch = true)
                level == 2 -> view.performHapticFeedback(constant(f))
                else -> predefined(f)?.let { id -> play(v, effects.getOrPut(f) { VibrationEffect.createPredefined(id) }, touch = true) }
            }
        }
    }

    /**
     * Vibration du jeu (intensités 0 à 65535, comme XInput ; sans durée, 5 s au plus comme pour
     * une manette) : l'amplitude suit la plus forte des deux quand le vibreur la règle.
     */
    fun rumble(large: Int, small: Int, durationMs: Int) {
        val v = vibrator ?: return
        if (!available) return
        runCatching {
            if (large <= 0 && small <= 0) {
                v.cancel()
                return
            }
            val ms = (if (durationMs <= 0) 5000 else durationMs).toLong()
            val amp = if (amplitude) (max(large, small).coerceIn(0, 65535) * 255 / 65535).coerceAtLeast(1) else VibrationEffect.DEFAULT_AMPLITUDE
            play(v, VibrationEffect.createOneShot(ms, amp), touch = false)
        }
    }

    fun stop() {
        runCatching { vibrator?.cancel() }
    }

    private fun play(v: Vibrator, e: VibrationEffect, touch: Boolean) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            v.vibrate(e, VibrationAttributes.createForUsage(if (touch) VibrationAttributes.USAGE_TOUCH else VibrationAttributes.USAGE_MEDIA))
        } else {
            val usage = if (touch) AudioAttributes.USAGE_ASSISTANCE_SONIFICATION else AudioAttributes.USAGE_GAME
            @Suppress("DEPRECATION")
            v.vibrate(e, AudioAttributes.Builder().setUsage(usage).build())
        }
    }

    private fun composition(f: Feel): VibrationEffect {
        val click = VibrationEffect.Composition.PRIMITIVE_CLICK
        val tick = VibrationEffect.Composition.PRIMITIVE_TICK
        val (primitive, base) = when (f) {
            Feel.CLICK -> click to 0.8f
            Feel.TRIGGER_START -> click to 0.4f
            Feel.TRIGGER_END -> click to 1f
            Feel.STICK_LEAVE -> tick to 0.7f
            Feel.STICK_SECTOR -> tick to 0.3f
            Feel.STICK_EDGE -> tick to 1f
        }
        return VibrationEffect.startComposition().addPrimitive(primitive, base * SCALES[level]).compose()
    }

    private fun constant(f: Feel): Int = when (f) {
        Feel.CLICK -> HapticFeedbackConstants.VIRTUAL_KEY
        Feel.TRIGGER_START -> HapticFeedbackConstants.KEYBOARD_TAP
        Feel.TRIGGER_END -> HapticFeedbackConstants.CONFIRM
        Feel.STICK_LEAVE, Feel.STICK_EDGE -> HapticFeedbackConstants.CLOCK_TICK
        Feel.STICK_SECTOR -> HapticFeedbackConstants.TEXT_HANDLE_MOVE
    }

    /** Faible : des tics (rien pour les directions, sans plus léger qu'un tic) ; fort : des clics. */
    private fun predefined(f: Feel): Int? = if (level == 1) {
        when (f) {
            Feel.TRIGGER_END -> VibrationEffect.EFFECT_CLICK
            Feel.STICK_SECTOR -> null
            else -> VibrationEffect.EFFECT_TICK
        }
    } else {
        when (f) {
            Feel.CLICK, Feel.TRIGGER_END -> VibrationEffect.EFFECT_HEAVY_CLICK
            Feel.TRIGGER_START, Feel.STICK_EDGE -> VibrationEffect.EFFECT_CLICK
            Feel.STICK_LEAVE, Feel.STICK_SECTOR -> VibrationEffect.EFFECT_TICK
        }
    }

    companion object {
        /** Échelle des primitives à chaque niveau. */
        private val SCALES = floatArrayOf(0f, 0.5f, 0.8f, 1f)

        fun vibrator(context: Context): Vibrator? = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            context.getSystemService(VibratorManager::class.java)?.defaultVibrator
        } else {
            @Suppress("DEPRECATION")
            context.getSystemService(Vibrator::class.java)
        }

        /** L'appareil a un vibreur (sinon, le réglage « Retour haptique » le dit, et seul le retour visuel reste). */
        fun available(context: Context) = runCatching { vibrator(context)?.hasVibrator() == true }.getOrDefault(false)
    }
}
