package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.hardware.display.DisplayManager
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.media.MediaCodecInfo
import android.media.MediaCodecList
import android.view.Display
import kotlin.math.roundToInt

/** Un décodeur matériel trouvé pour un format. */
data class DecoderInfo(val name: String, val maxWidth: Int, val maxHeight: Int, val lowLatency: Boolean)

/** Ce que l'appareil décode en matériel (vérifié au lancement, comme les autres apps). */
data class Decoders(val h264: DecoderInfo?, val hevc: DecoderInfo?, val av1: DecoderInfo?, val hevc444: Boolean) {
    /** Au moins un décodeur matériel est connu : sinon, rien à interroger pour « Auto » ([AutoQuality.limits]). */
    val any get() = h264 != null || hevc != null || av1 != null

    /**
     * Un décodeur matériel de l'appareil suit-il cette définition à cette fréquence ? Ce que `MediaCodec` en dit
     * (`VideoCapabilities.areSizeAndRateSupported`), pour « Auto » de « Qualité de l'image ». Les capacités sont lues
     * une fois par appel de cette fonction : elle rend le test, à garder le temps de plusieurs questions.
     */
    fun follows(): (Int, Int, Int) -> Boolean {
        val infos = runCatching { MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos }.getOrNull() ?: return { _, _, _ -> false }
        val caps = listOf(h264 to "video/avc", hevc to "video/hevc", av1 to "video/av01").mapNotNull { (decoder, mime) ->
            decoder?.let { d -> infos.firstOrNull { it.name == d.name }?.let { runCatching { it.getCapabilitiesForType(mime).videoCapabilities }.getOrNull() } }
        }
        return { width, height, fps ->
            caps.any { runCatching { it.areSizeAndRateSupported(width, height, fps.toDouble()) }.getOrDefault(false) }
        }
    }

    companion object {
        fun detect(): Decoders {
            val infos = MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos.filter { !it.isEncoder && it.isHardwareAccelerated && !it.isAlias }
            fun find(mime: String): DecoderInfo? {
                val c = infos.firstOrNull { info -> info.supportedTypes.any { it.equals(mime, true) } } ?: return null
                val caps = c.getCapabilitiesForType(mime)
                val v = caps.videoCapabilities
                return DecoderInfo(
                    name = c.name,
                    maxWidth = v?.supportedWidths?.upper ?: 0,
                    maxHeight = v?.supportedHeights?.upper ?: 0,
                    lowLatency = caps.isFeatureSupported(MediaCodecInfo.CodecCapabilities.FEATURE_LowLatency),
                )
            }
            val hevc = find("video/hevc")
            // HEVC 4:4:4 (RExt) : profil rarement présent sur Android.
            val rext = infos.filter { info -> info.supportedTypes.any { it.equals("video/hevc", true) } }.any { info ->
                info.getCapabilitiesForType("video/hevc").profileLevels.any { it.profile == RANGE_EXTENSIONS }
            }
            return Decoders(find("video/avc"), hevc, find("video/av01"), hevc != null && rext)
        }

        /** `HEVCProfileMain444` n'existe pas dans le SDK ; profil RExt de la norme. */
        private const val RANGE_EXTENSIONS = 0x4
    }
}

/** Un mode de l'écran de l'appareil. */
data class ScreenMode(val id: Int, val width: Int, val height: Int, val refresh: Int)

/** L'écran de l'appareil : son mode, ceux qu'il sait prendre, le mode jeu (ALLM). */
data class Screen(
    val current: ScreenMode,
    val modes: List<ScreenMode>,
    val minimalPostProcessing: Boolean,
    /** Fréquence variable (téléphones et tablettes : plusieurs fréquences à la même définition). */
    val variableRefresh: Boolean,
) {
    /** La plus haute fréquence que l'écran tient à cette définition (1080p à 120 Hz sur la Shield). */
    fun bestRefresh(width: Int, height: Int): Int =
        modes.filter { it.width >= width && it.height >= height }.maxOfOrNull { it.refresh } ?: current.refresh

    /** Le mode à demander pour une session (définition, fréquence), s'il existe. */
    fun modeFor(width: Int, height: Int, refresh: Int): ScreenMode? =
        modes.filter { it.width == width && it.height == height && it.refresh == refresh }.firstOrNull()

    companion object {
        fun of(display: Display): Screen {
            val cur = display.mode
            val modes = display.supportedModes.map { ScreenMode(it.modeId, it.physicalWidth, it.physicalHeight, it.refreshRate.roundToInt()) }
            val sameSize = modes.filter { it.width == cur.physicalWidth && it.height == cur.physicalHeight }.map { it.refresh }.distinct()
            return Screen(
                current = ScreenMode(cur.modeId, cur.physicalWidth, cur.physicalHeight, cur.refreshRate.roundToInt()),
                modes = modes,
                minimalPostProcessing = display.isMinimalPostProcessingSupported,
                variableRefresh = sameSize.size > 1,
            )
        }

        fun of(context: Context): Screen = of(context.getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY))
    }
}

/** La sortie du son en ce moment : nom, voies possibles, retard probable. */
data class AudioOutput(val label: String, val type: Int, val maxChannels: Int, val bluetooth: Boolean) {
    companion object {
        fun current(context: Context): AudioOutput {
            val am = context.getSystemService(AudioManager::class.java)
            val devices = am.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
            // Priorité à ce qu'Android choisit : Bluetooth, casque, HDMI, puis le reste.
            val order = listOf(
                AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_BLE_SPEAKER,
                AudioDeviceInfo.TYPE_USB_HEADSET, AudioDeviceInfo.TYPE_WIRED_HEADPHONES, AudioDeviceInfo.TYPE_WIRED_HEADSET,
                AudioDeviceInfo.TYPE_HDMI_ARC, AudioDeviceInfo.TYPE_HDMI_EARC, AudioDeviceInfo.TYPE_HDMI,
                AudioDeviceInfo.TYPE_USB_DEVICE, AudioDeviceInfo.TYPE_BUILTIN_SPEAKER, AudioDeviceInfo.TYPE_TELEPHONY,
            )
            val d = order.firstNotNullOfOrNull { t -> devices.firstOrNull { it.type == t } } ?: devices.firstOrNull()
            val channels = d?.channelCounts?.maxOrNull() ?: 2
            val bt = d?.type in setOf(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_BLE_SPEAKER)
            return AudioOutput(d?.productName?.toString()?.takeIf { it.isNotBlank() } ?: "", d?.type ?: 0, channels.coerceAtLeast(2), bt)
        }
    }

    val hdmi get() = type == AudioDeviceInfo.TYPE_HDMI || type == AudioDeviceInfo.TYPE_HDMI_ARC || type == AudioDeviceInfo.TYPE_HDMI_EARC
}
