package fr.leslignesclaires.kybernow.model

import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * L'écran que la session demandera à l'ordinateur : le même calcul pour l'accueil (« Écran de
 * 2560 × 1440 à 120 Hz ») et pour la session (`SessionActivity.start`).
 */
object SessionMode {
    /**
     * Le format que la session demandera : celui du profil (HEVC si l'appareil le décode, sinon
     * H.264, comme `StreamSettings::for_profile` du cœur), ou le réglage s'il est décodable.
     */
    fun codec(host: Host, decoders: Decoders): String {
        val profile = if (decoders.hevc != null) "hevc" else "h264"
        return when (host.settings.codec) {
            "hevc" -> if (decoders.hevc != null) "hevc" else profile
            "h264" -> "h264"
            "av1" -> if (decoders.av1 != null && host.av1) "av1" else profile
            else -> profile
        }
    }

    /**
     * Définition et fréquence : « comme cet écran » par défaut, à la plus haute fréquence qu'il
     * tient. Une fréquence demandée que la définition de l'écran ne tient pas fait descendre la
     * définition (la Shield : 4K à 60 Hz ou 1080p à 120 Hz). Ramenées à ce que le décodeur du
     * format accepte (4096 de large au plus en H.264).
     */
    fun pick(screen: Screen, settings: HostSettings, codec: String, decoders: Decoders): Triple<Int, Int, Int> {
        val cur = screen.current
        val w0 = max(cur.width, cur.height)
        val h0 = min(cur.width, cur.height)
        var (w, h, r) = when {
            settings.resolution == 0 && settings.refresh == 0 -> Triple(w0, h0, screen.bestRefresh(w0, h0))
            settings.resolution == 0 -> {
                val exact = screen.modes.any { max(it.width, it.height) == w0 && it.refresh == settings.refresh }
                if (exact) Triple(w0, h0, settings.refresh)
                else screen.modes.filter { it.refresh >= settings.refresh }.maxByOrNull { it.width * it.height }
                    ?.let { Triple(max(it.width, it.height), min(it.width, it.height), settings.refresh) }
                    ?: Triple(w0, h0, screen.bestRefresh(w0, h0))
            }
            else -> {
                val hh = settings.resolution
                val ww = (hh * w0.toDouble() / h0 / 2).roundToInt() * 2
                Triple(ww, hh, if (settings.refresh > 0) settings.refresh else screen.bestRefresh(ww, hh))
            }
        }
        val dec = when (codec) {
            "hevc" -> decoders.hevc
            "av1" -> decoders.av1
            else -> decoders.h264
        }
        val maxW = min(if (codec == "h264") 4096 else 7680, dec?.maxWidth?.takeIf { it > 0 } ?: 7680)
        val maxH = dec?.maxHeight?.takeIf { it > 0 } ?: 4320
        if (w > maxW || h > maxH) {
            val k = min(maxW.toDouble() / w, maxH.toDouble() / h)
            w = (w * k / 2).roundToInt() * 2
            h = (h * k / 2).roundToInt() * 2
        }
        return Triple(w, h, r.coerceIn(24, 240))
    }

    /** L'écran à la taille de cet appareil : « comme cet écran », sans définition ramenée. */
    fun matchesScreen(screen: Screen, settings: HostSettings, mode: Triple<Int, Int, Int>): Boolean =
        settings.resolution == 0 && mode.first == max(screen.current.width, screen.current.height) &&
            mode.second == min(screen.current.width, screen.current.height)

    /**
     * Débit demandé (Mbit/s) pour un mode, comme `StreamSettings::bitrate` du cœur : celui d'un
     * profil suit les pixels par seconde (puissance 0,75, de 0,4 à 3 fois), un débit choisi reste.
     */
    fun bitrate(baseMbps: Int, fromProfile: Boolean, width: Int, height: Int, refresh: Int): Int {
        val mbps = if (fromProfile) {
            val ratio = width.toDouble() * height * max(refresh, 1) / (3840.0 * 2160.0 * 60.0)
            (baseMbps * Math.pow(ratio, 0.75).coerceIn(0.4, 3.0)).roundToInt()
        } else baseMbps
        return mbps.coerceIn(5, 4294)
    }
}
