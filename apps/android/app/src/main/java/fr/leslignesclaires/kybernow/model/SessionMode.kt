package fr.leslignesclaires.kybernow.model

import fr.leslignesclaires.kybernow.core.Core
import org.json.JSONObject
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * Ce que la session demandera à l'ordinateur : le même calcul pour l'accueil (« Écran de 2560 × 1440 à 120 Hz »), le
 * HDR, un emprunt et la session (`SessionActivity.start`). Le format, le débit, le 4:4:4 et la définition ramenée de
 * l'H.264 sont la règle du cœur (`plan::plan`, comme l'app Windows) ; l'app ne garde que ce que seul Android sait :
 * les modes de son écran ([screenMode]) et les tailles que ses décodeurs acceptent ([fitDecoder]).
 */
object SessionMode {
    /** Le plan du cœur : les réglages de la session (le JSON de `sessionStart`) et l'écran demandé. */
    class Plan(val settings: JSONObject, val mode: Triple<Int, Int, Int>) {
        val codec: String get() = settings.optString("codec")
    }

    /**
     * Les choix de cet ordinateur pour le cœur (`plan::Choices`) : le profil, le format choisi à la main s'il est lu par
     * l'appareil (PyroWave : le cœur le garde au départ si la Station le propose, sinon HEVC), le débit à la main, le
     * transport, le résultat du test d'Auto, « Débit adaptatif », le plafond de l'encodeur. [codec] remplace le format
     * choisi ; [profile] celui de la session (changé dans la barre) ; [hevcInstead] : « Passer en HEVC » du bandeau de
     * PyroWave, pour la fin de la session.
     */
    fun choices(host: Host, decoders: Decoders, profile: Profile = host.settings.profile, hevcInstead: Boolean = false): JSONObject {
        val set = host.settings
        val o = JSONObject().put("profile", profile.id).put("hevc_instead", hevcInstead)
        when (set.codec) {
            "hevc" -> if (decoders.hevc != null) o.put("codec", "hevc")
            "h264" -> o.put("codec", "h264")
            "av1" -> if (decoders.av1 != null && host.av1) o.put("codec", "av1")
            "pyrowave" -> o.put("codec", "pyrowave")
        }
        if (set.mbps > 0) o.put("mbps", set.mbps)
        if (set.transport != "auto") o.put("reliable", set.reliable)
        if (profile == Profile.AUTO) set.autoResult?.let { r -> runCatching { o.put("auto", JSONObject(r)) } }
        o.put("fixed", !set.adaptive)
        set.encoderMax?.let { o.put("encoder_max", it) }
        return o
    }

    /** `{hevc, hevc444, av1, pyrowave}` : ce que l'appareil décode, pour le plan (le HDR ne change pas le format). */
    fun caps(host: Host, decoders: Decoders): JSONObject = JSONObject()
        .put("hevc", decoders.hevc != null).put("hevc444", decoders.hevc444).put("av1", decoders.av1 != null && host.av1)
        .put("pyrowave", PyroWave.decodes)

    /**
     * Le plan de la session pour cet écran : les réglages du cœur et l'écran demandé, ramené à ce que le décodeur du
     * format accepte. Sans réponse du cœur : l'écran tel quel et le format de l'appareil, sans réglage.
     */
    fun plan(host: Host, decoders: Decoders, screen: Screen, profile: Profile = host.settings.profile): Plan =
        plan(host, decoders, screenMode(screen, host.settings), profile)

    /** [plan] pour un écran déjà choisi ([mode] : celui de la session, gardé quand un réglage change en cours de route). */
    fun plan(host: Host, decoders: Decoders, mode: Triple<Int, Int, Int>, profile: Profile, hevcInstead: Boolean = false): Plan {
        val (w, h, r) = mode
        val p = Core.plan(choices(host, decoders, profile, hevcInstead), caps(host, decoders), w, h, r)
        val settings = p?.optJSONObject("settings")
            ?: JSONObject().put("profile", profile.id).put("codec", if (decoders.hevc != null) "hevc" else "h264")
        val m = p?.optJSONObject("mode")
        val planned = if (m != null) Triple(m.optInt("width", w), m.optInt("height", h), m.optInt("refresh", r)) else mode
        return Plan(settings, fitDecoder(planned, settings.optString("codec"), decoders))
    }

    /**
     * Le format que la session demandera, pour la définition et le HDR : celui du plan ; PyroWave se juge comme le
     * HEVC qui le remplace quand la Station ne le propose pas.
     */
    fun codec(host: Host, decoders: Decoders): String {
        val c = Core.plan(choices(host, decoders), caps(host, decoders), 1920, 1080, 60)?.optJSONObject("settings")?.optString("codec")
        return when (c) {
            "hevc", "h264", "av1" -> c
            else -> if (decoders.hevc != null) "hevc" else "h264"
        }
    }

    /**
     * L'écran de l'appareil que la session demandera : « comme cet écran » par défaut, à la plus haute fréquence qu'il
     * tient. Une fréquence demandée que la définition de l'écran ne tient pas fait descendre la définition (la Shield :
     * 4K à 60 Hz ou 1080p à 120 Hz) ; une définition choisie garde les proportions de l'écran.
     */
    fun screenMode(screen: Screen, settings: HostSettings): Triple<Int, Int, Int> {
        val cur = screen.current
        val w0 = max(cur.width, cur.height)
        val h0 = min(cur.width, cur.height)
        val (w, h, r) = when {
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
        return Triple(w, h, r.coerceIn(24, 240))
    }

    /**
     * L'écran ramené à ce que le décodeur matériel du format accepte (sa plus grande taille) ; PyroWave, décodé par la
     * carte graphique, se juge comme le HEVC qui le remplace.
     */
    fun fitDecoder(mode: Triple<Int, Int, Int>, codec: String, decoders: Decoders): Triple<Int, Int, Int> {
        val dec = when (codec) {
            "hevc", "pyrowave" -> decoders.hevc ?: decoders.h264
            "av1" -> decoders.av1
            else -> decoders.h264
        }
        if (dec == null || dec.maxWidth <= 0 || dec.maxHeight <= 0) return mode
        val (maxW, maxH) = dec.maxWidth to dec.maxHeight
        var (w, h, r) = mode
        if (w > maxW || h > maxH) {
            val k = min(maxW.toDouble() / w, maxH.toDouble() / h)
            w = (w * k / 2).roundToInt() * 2
            h = (h * k / 2).roundToInt() * 2
        }
        return Triple(w, h, r)
    }

    /** L'écran à la taille de cet appareil : « comme cet écran », sans définition ramenée. */
    fun matchesScreen(screen: Screen, settings: HostSettings, mode: Triple<Int, Int, Int>): Boolean =
        settings.resolution == 0 && mode.first == max(screen.current.width, screen.current.height) &&
            mode.second == min(screen.current.width, screen.current.height)
}
