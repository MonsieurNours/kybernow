package fr.leslignesclaires.kybernow.session

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import fr.leslignesclaires.kybernow.core.SessionListener
import fr.leslignesclaires.kybernow.model.Gamepads
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import java.util.concurrent.atomic.AtomicLong

/** Étapes de la session, dans le vocabulaire du cœur. */
enum class Phase { PREPARING, CONNECTING, FIRST_IMAGES, STREAMING, RECONNECTING, ENDING, ENDED }

data class Ended(val error: String?, val displayRestored: Boolean?, val audioRestored: Boolean?, val reportError: String?)

/** Mesures de la dernière seconde (barre, menu, panneau des mesures). */
data class Second(
    val latencyMs: Double?,
    val rttMs: Double?,
    val fps: Int,
    val lostFrames: Int,
    val holes: Int,
    val lostPackets: Int,
    val mbps: Double,
    /** Capture → encodage, encodage, envoi, réseau, décodage, affichage (ms). */
    val chain: DoubleArray?,
)

/** Une capsule en haut de l'image (jamais une alerte). */
sealed class Capsule {
    data class Reconnecting(val attempt: Int, val max: Int) : Capsule()
    data object Relaunched : Capsule()
    data class ModeChanged(val width: Int, val height: Int, val back: Boolean) : Capsule()
    data class Fallback(val level: Int, val mbps: Int) : Capsule()
    data class Adapting(val width: Int, val height: Int) : Capsule()
    data class AdaptFailed(val error: String) : Capsule()
    data class GamepadConnected(val name: String, val slot: Int, val motors: Int) : Capsule()
}

data class DisplayInfo(val id: Int, val width: Int, val height: Int)

data class PointerState(val visible: Boolean, val x: Double, val y: Double, val remoteHidden: Boolean, val shownAt: Long)

/**
 * Ce que le cœur dit de la session, rangé pour l'interface. Les rappels
 * arrivent des fils du cœur : tout passe par des `StateFlow` (lecture sûre
 * de tout fil) ou est remonté sur le fil principal.
 */
class SessionState(private val context: Context, private val gamepads: Gamepads) : SessionListener {
    private val main = Handler(Looper.getMainLooper())

    val phase = MutableStateFlow(Phase.PREPARING)
    val ended = MutableStateFlow<Ended?>(null)
    val second = MutableStateFlow<Second?>(null)
    val capsule = MutableStateFlow<Capsule?>(null)
    val restoreOffer = MutableStateFlow<Pair<Int, Int>?>(null)
    val display = MutableStateFlow<DisplayInfo?>(null)
    val displays = MutableStateFlow<List<DisplayInfo>>(emptyList())
    val mode = MutableStateFlow<Triple<Int, Int, Int>?>(null)
    val videoSize = MutableStateFlow<Pair<Int, Int>?>(null)
    val decoder = MutableStateFlow<String?>(null)
    val audioOutput = MutableStateFlow<Pair<Int, Double>?>(null)
    val gameFullscreen = MutableStateFlow(false)
    val pointer = MutableStateFlow(PointerState(false, 0.0, 0.0, false, 0))
    val pointerShape = MutableStateFlow<Pair<Bitmap, Pair<Int, Int>>?>(null)
    /** La combinaison s'est formée : ouvrir le menu. */
    var onCombo: (() -> Unit)? = null
    /** Les entrées ont rejoint l'ordinateur : rebrancher les manettes. */
    var onInputConnected: (() -> Unit)? = null
    /** Premier instant d'image, pour le passage et la durée de la session. */
    val firstImageAt = AtomicLong(0)

    private var capsuleClear: Runnable? = null

    fun show(c: Capsule, ms: Long = 4000) = main.post {
        capsule.value = c
        capsuleClear?.let(main::removeCallbacks)
        if (ms > 0) {
            val r = Runnable { if (capsule.value == c) capsule.value = null }
            capsuleClear = r
            main.postDelayed(r, ms)
        }
    }

    override fun onStatus(kind: Int, attempt: Int, max: Int, error: String?, hasReport: Boolean, displayRestored: Boolean, audioRestored: Boolean, reportError: String?) {
        val p = Phase.entries[kind.coerceIn(0, 6)]
        phase.value = p
        when (p) {
            Phase.RECONNECTING -> show(Capsule.Reconnecting(attempt, max), 0)
            Phase.STREAMING -> {
                firstImageAt.compareAndSet(0, System.currentTimeMillis())
                if (capsule.value is Capsule.Reconnecting) main.post { capsule.value = null }
            }
            Phase.ENDED -> ended.value = Ended(error, if (hasReport) displayRestored else null, if (hasReport) audioRestored else null, reportError)
            else -> {}
        }
    }

    override fun onDisplay(id: Int, width: Int, height: Int) {
        display.value = DisplayInfo(id, width, height)
    }

    override fun onDisplays(json: String) {
        displays.value = runCatching {
            val a = JSONArray(json)
            (0 until a.length()).map { a.getJSONObject(it).let { o -> DisplayInfo(o.getInt("id"), o.getInt("width"), o.getInt("height")) } }
        }.getOrDefault(emptyList())
    }

    override fun onMode(width: Int, height: Int, refresh: Int, virtualDisplay: Boolean) {
        mode.value = Triple(width, height, refresh)
    }

    override fun onSecond(values: DoubleArray) {
        val chain = if (values[10] > 0.5) values.copyOfRange(11, 17) else null
        second.value = Second(
            latencyMs = values[0].takeIf { it >= 0 },
            rttMs = values[1].takeIf { it >= 0 },
            fps = values[2].toInt(),
            lostFrames = values[5].toInt(),
            holes = values[6].toInt(),
            lostPackets = values[8].toInt(),
            mbps = values[9] * 8 / 1_000_000,
            chain = chain,
        )
    }

    override fun onNotice(kind: Int, width: Int, height: Int, back: Boolean, level: Int, mbps: Int, hasMode: Boolean, error: String?) {
        when (kind) {
            0 -> show(Capsule.Relaunched)
            1 -> show(Capsule.ModeChanged(width, height, back))
            2 -> show(Capsule.Fallback(level, mbps), 6000)
            3 -> show(Capsule.Adapting(width, height))
            4 -> main.post { if (capsule.value is Capsule.Adapting) capsule.value = null }
            5 -> show(Capsule.AdaptFailed(error ?: ""), 6000)
            6 -> restoreOffer.value = if (hasMode) width to height else null
        }
    }

    override fun onGameFullscreen(game: Boolean) {
        gameFullscreen.value = game
    }

    override fun onInputConnected() {
        main.post { onInputConnected?.invoke() }
    }

    override fun onVideoSize(width: Int, height: Int) {
        videoSize.value = width to height
    }

    override fun onDecoder(name: String) {
        decoder.value = name
    }

    override fun onAudioOutput(channels: Int, bufferMs: Double) {
        audioOutput.value = channels to bufferMs
    }

    @Volatile private var rumbleSeen = false

    override fun onRumble(pad: Int, large: Int, small: Int, durationMs: Int) {
        if (!rumbleSeen) {
            rumbleSeen = true
            android.util.Log.i("KyberNow", "vibrations : première reçue de l'ordinateur (rang $pad, $large/$small, $durationMs ms)")
        }
        val p = gamepads.pads.value.firstOrNull { it.slot == pad } ?: run {
            android.util.Log.w("KyberNow", "vibrations : aucune manette au rang $pad")
            return
        }
        gamepads.rumble(p.deviceId, large, small, durationMs)
    }

    override fun onCombo(pad: Int) {
        main.post { onCombo?.invoke() }
    }

    override fun onPointer(hasPosition: Boolean, x: Double, y: Double, remoteHidden: Boolean, width: Int, height: Int, hotX: Int, hotY: Int, pixels: IntArray?) {
        if (pixels != null && width > 0 && height > 0) {
            val bmp = Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888).apply { isPremultiplied = true }
            pointerShape.value = bmp to (hotX to hotY)
        }
        val prev = pointer.value
        val shownAt = if (!remoteHidden && prev.remoteHidden) System.currentTimeMillis() else if (remoteHidden) 0 else prev.shownAt.takeIf { it > 0 } ?: System.currentTimeMillis()
        pointer.value = PointerState(hasPosition, x, y, remoteHidden, shownAt)
    }

    // MARK: - Presse-papiers (lu seulement quand l'ordinateur le demande)

    private val clipboard = context.getSystemService(ClipboardManager::class.java)
    private val clipChanges = AtomicLong(0)
    /** Notre propre écriture : son avis de changement (qui arrive après) ne compte pas. */
    private val ownWrite = java.util.concurrent.atomic.AtomicBoolean(false)
    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        if (!ownWrite.getAndSet(false)) clipChanges.incrementAndGet()
    }

    init {
        main.post { clipboard?.addPrimaryClipChangedListener(clipListener) }
    }

    fun close() {
        main.post { clipboard?.removePrimaryClipChangedListener(clipListener) }
    }

    override fun clipChangeCount(): Long = clipChanges.get()

    override fun clipHasText(): Boolean = clipboard?.primaryClipDescription?.hasMimeType("text/*") == true

    override fun clipText(): String? = runCatching { clipboard?.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString() }.getOrNull()

    override fun clipSetText(text: String) {
        ownWrite.set(true)
        if (runCatching { clipboard?.setPrimaryClip(ClipData.newPlainText("KyberNow", text)) }.isFailure) ownWrite.set(false)
    }
}
