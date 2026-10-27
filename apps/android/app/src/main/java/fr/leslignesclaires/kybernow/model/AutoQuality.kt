package fr.leslignesclaires.kybernow.model

import org.json.JSONObject

/**
 * Ce que le décodeur de l'appareil suit : la définition (côté long, côté court) et la fréquence. `0` : rien de
 * connu (aucun décodeur à interroger), le cœur ne borne alors que par l'écran (`DecoderCaps` du cœur).
 */
data class DecoderLimits(val maxWidth: Int, val maxHeight: Int, val maxFps: Int) {
    companion object {
        val NONE = DecoderLimits(0, 0, 0)
    }
}

/**
 * « Auto » de « Qualité de l'image » (docs/link-ui.md, « Partage ») : l'appareil dit ce qu'il sait afficher (son
 * écran : définition native et fréquence) et décoder (ce que `MediaCodec` suit, par
 * `VideoCapabilities.areSizeAndRateSupported`) ; le cœur en tire `auto` (`share::auto_quality`, avec les formats
 * lus) et c'est la Station qui tranche, d'après l'écran du partage : l'image de l'hôte si l'appareil la couvre et
 * suit sa fréquence, sinon 1080p · 60 Hz, sinon 720p · 30 Hz. Ici, seulement la mesure de l'appareil et le JSON
 * que le cœur lit.
 */
object AutoQuality {
    /** Les définitions essayées quand le décodeur ne suit pas l'écran en entier (côté long, côté court), de la plus grande à la plus petite. */
    private val SIZES = listOf(3840 to 2160, 2560 to 1440, 1920 to 1080, 1280 to 720)

    /** Les fréquences essayées en dessous de celle de l'écran, de la plus haute à la plus basse. */
    private val RATES = listOf(240, 165, 144, 120, 90, 75, 60, 50, 48, 30, 24)

    /**
     * Ce que le décodeur suit de cet [screen] : la définition de l'écran si `supports` (largeur, hauteur,
     * fréquence : `areSizeAndRateSupported`) la suit à 30 Hz, sinon la plus grande définition plus petite qu'il
     * suit (720p au pire) ; puis la fréquence de l'écran si le décodeur la suit à cette définition, sinon la plus
     * haute fréquence plus basse qu'il suit (24 Hz au pire). La fréquence se juge à la définition retenue : un
     * décodeur qui fait du 4K à 30 Hz et du 1080p à 120 Hz ne dit pas du 4K à 120 Hz. [anyDecoder] faux : rien de
     * connu ([DecoderLimits.NONE]).
     */
    fun limits(screen: ScreenMode, anyDecoder: Boolean = true, supports: (Int, Int, Int) -> Boolean): DecoderLimits {
        if (!anyDecoder) return DecoderLimits.NONE
        val long = maxOf(screen.width, screen.height)
        val short = minOf(screen.width, screen.height)
        val fps = screen.refresh.coerceAtLeast(1)
        if (long <= 0 || short <= 0) return DecoderLimits.NONE
        val sizes = (listOf(long to short) + SIZES.filter { it.first < long && it.second <= short }).distinct()
        val size = sizes.firstOrNull { supports(it.first, it.second, minOf(fps, 30)) } ?: sizes.last()
        val rates = (listOf(fps) + RATES.filter { it < fps }).distinct()
        val rate = rates.firstOrNull { supports(size.first, size.second, it) } ?: rates.last()
        return DecoderLimits(size.first, size.second, rate)
    }

    /** L'écran de l'appareil tel que le cœur le lit (`DeviceScreen`) : sa définition native et sa fréquence. */
    fun screenJson(screen: ScreenMode): JSONObject = JSONObject()
        .put("x", 0.0).put("y", 0.0)
        .put("width", screen.width.toDouble()).put("height", screen.height.toDouble())
        .put("pixel_width", screen.width).put("pixel_height", screen.height)
        .put("refresh", screen.refresh)
        .put("primary", true)

    /** Ce que l'appareil décode, tel que le cœur le lit (`DecoderCaps`) : les formats, et la définition et la fréquence suivies. */
    fun decoderJson(decoders: Decoders, limits: DecoderLimits): JSONObject = JSONObject()
        .put("hevc", decoders.hevc != null)
        .put("hevc444", decoders.hevc444)
        .put("av1", decoders.av1 != null)
        .put("max_width", limits.maxWidth)
        .put("max_height", limits.maxHeight)
        .put("max_fps", limits.maxFps)
}
