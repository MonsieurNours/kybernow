package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.hardware.display.DisplayManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.view.Display
import fr.leslignesclaires.kybernow.core.HostInfo
import fr.leslignesclaires.kybernow.core.Native
import org.json.JSONObject

/**
 * Les valeurs HDR d'un écran de l'appareil, en nits (docs/hdr.md, « Réglage du HDR ») : luminosité maximale (petite
 * zone), sur tout l'écran, noir, blanc du bureau. Données à la Station, qui en fait le profil de couleur de l'écran
 * virtuel : les jeux font leur tone mapping pour cet écran. Le cœur les borne.
 */
data class HdrLuminance(val max: Double, val fullFrame: Double, val min: Double, val sdrWhite: Double = SDR_WHITE) {
    fun json(): JSONObject = JSONObject().put("max", max).put("full_frame", fullFrame).put("min", min).put("sdr_white", sdrWhite)

    companion object {
        /** Blanc du bureau : Android ne le donne pas, référence BT.2408 (celle du cœur). */
        const val SDR_WHITE = 203.0

        /**
         * D'après `Display.HdrCapabilities` (`getDesiredMaxLuminance`, `getDesiredMaxAverageLuminance`,
         * `getDesiredMinLuminance`). Une luminosité maximale à 0, négative ou illisible (fréquent : le téléviseur ne la
         * dit pas à la Shield) : `null`, les valeurs par défaut du cœur (1 000 nits). Sans valeur sur tout l'écran : la
         * maximale ; sans noir : 0.
         */
        fun of(max: Float, average: Float, min: Float): HdrLuminance? {
            if (!max.isFinite() || max <= 0f) return null
            val m = max.toDouble()
            val full = average.takeIf { it.isFinite() && it > 0f }?.toDouble()?.coerceAtMost(m) ?: m
            val black = min.takeIf { it.isFinite() && it > 0f }?.toDouble() ?: 0.0
            return HdrLuminance(m, full, black)
        }
    }
}

/** L'écran de l'appareil vu par le HDR : il affiche le HDR10, et ses valeurs (`null` : celles par défaut du cœur). */
data class HdrScreen(val hdr: Boolean, val luminance: HdrLuminance? = null) {
    companion object {
        val SDR = HdrScreen(false)

        /** Les types HDR d'un écran comprennent le HDR10. */
        fun hdr10(types: IntArray?): Boolean = types?.contains(Display.HdrCapabilities.HDR_TYPE_HDR10) == true

        /**
         * L'écran [display] : HDR10 parmi les types de son mode (Android 14) ou de ses capacités HDR, et les
         * luminosités de ses capacités. Un écran sans HDR (l'émulateur) : [SDR].
         */
        fun of(display: Display?): HdrScreen = runCatching {
            val d = display ?: return SDR
            // Les luminosités ne sont que dans `HdrCapabilities` (dépréciées en Android 14 pour les types seulement).
            @Suppress("DEPRECATION") val caps = d.hdrCapabilities
            @Suppress("DEPRECATION") val types = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) d.mode.supportedHdrTypes else caps?.supportedHdrTypes
            if (!hdr10(types)) return SDR
            HdrScreen(true, caps?.let { HdrLuminance.of(it.desiredMaxLuminance, it.desiredMaxAverageLuminance, it.desiredMinLuminance) })
        }.getOrDefault(SDR)

        fun of(context: Context): HdrScreen = of(context.getSystemService(DisplayManager::class.java)?.getDisplay(Display.DEFAULT_DISPLAY))

        /**
         * Suit l'écran [displayId] (téléviseur qui coupe le HDR, autre sortie HDMI, Shield qui change de mode) :
         * [changed] reçoit le nouvel état sur le fil principal, seulement quand il change. Rendre l'objet le retire.
         */
        fun watch(context: Context, displayId: Int, start: HdrScreen, changed: (HdrScreen) -> Unit): AutoCloseable {
            val dm = context.getSystemService(DisplayManager::class.java) ?: return AutoCloseable {}
            var last = start
            val listener = object : DisplayManager.DisplayListener {
                override fun onDisplayAdded(id: Int) {}
                override fun onDisplayRemoved(id: Int) {}
                override fun onDisplayChanged(id: Int) {
                    if (id != displayId) return
                    val now = of(dm.getDisplay(id))
                    if (now != last) {
                        last = now
                        changed(now)
                    }
                }
            }
            dm.registerDisplayListener(listener, Handler(Looper.getMainLooper()))
            return AutoCloseable { dm.unregisterDisplayListener(listener) }
        }
    }
}

/**
 * La puce « HDR » d'un ordinateur (docs/link-ui.md, section HDR), telle que le cœur la donne
 * (`profile::hdr_state`) : [shown] faux, aucune trace du HDR (écran SDR) ; [available] faux, puce inactive avec
 * [reason] (texte de link-ui, langue de l'app).
 */
data class HdrState(val shown: Boolean, val available: Boolean, val reason: String? = null) {
    /** Le HDR sera demandé : écran HDR, possible, et le choix [choice] de cet ordinateur allumé. */
    fun on(choice: Boolean) = shown && available && choice

    companion object {
        val HIDDEN = HdrState(false, false)

        /** La réponse du cœur ; illisible : [HIDDEN]. */
        fun parse(json: String?): HdrState = runCatching {
            val o = JSONObject(json ?: return HIDDEN)
            HdrState(o.optBoolean("shown"), o.optBoolean("available"), if (o.isNull("reason")) null else o.optString("reason").takeIf { it.isNotBlank() })
        }.getOrDefault(HIDDEN)
    }
}

/** Les règles du HDR de l'app pour un ordinateur, communes à l'accueil, aux réglages et à la session (docs/hdr.md). */
object Hdr {
    /**
     * Le format de l'image en HDR, comme `profile::hdr_format` du cœur : HEVC ou AV1 choisis restent ; « Auto » (et un
     * profil en H.264) prend HEVC, ou l'AV1 si l'appareil ne lit pas le HEVC. H.264 choisi à la main : pas de HDR.
     */
    fun codec(host: Host, decoders: Decoders): String? = when (SessionMode.codec(host, decoders)) {
        "av1" -> "av1"
        "hevc" -> "hevc"
        else -> when {
            host.settings.codec == "h264" -> null
            decoders.hevc != null -> "hevc"
            decoders.av1 != null && host.av1 -> "av1"
            else -> "hevc"
        }
    }

    /** Le décodeur lit le HDR10 dans le format que la session prendra (`Capabilities.hdr` du cœur). */
    fun decodes(host: Host, decoders: Decoders): Boolean = when (codec(host, decoders)) {
        "av1" -> decoders.av1Hdr
        "hevc" -> decoders.hevcHdr
        else -> false
    }

    /** `{hevc, hevc444, av1, hdr}` : ce que l'appareil décode, pour le cœur. */
    fun capabilities(host: Host, decoders: Decoders): JSONObject = JSONObject()
        .put("hevc", decoders.hevc != null).put("hevc444", decoders.hevc444).put("av1", decoders.av1 != null && host.av1)
        .put("hdr", decodes(host, decoders))

    /**
     * Les réglages que le cœur regarde pour la puce : le format que la session demandera, et le profil sauf avec
     * H.264 choisi à la main (« Le HDR demande le format HEVC ou AV1. ») ; un profil en H.264, faute de HEVC, passe
     * lui-même en HEVC ou AV1.
     */
    fun chipSettings(host: Host, decoders: Decoders): JSONObject = JSONObject().apply {
        put("codec", SessionMode.codec(host, decoders)).put("mbps", 0)
        if (host.settings.codec != "h264") put("profile", host.settings.profile.id)
    }

    /** Le choix « HDR » envoyé au cœur dans les réglages de la session : jamais avec H.264 choisi à la main. */
    fun wanted(host: Host): Boolean = host.settings.hdr && host.settings.codec != "h264"

    /**
     * La puce de cet ordinateur sur cet écran : [HdrState.HIDDEN] sur un écran SDR (rien à demander au cœur), sinon
     * la règle du cœur, d'après l'info de l'ordinateur ([info] nulle : pas encore jointe, « L'ordinateur ne propose
     * pas le HDR. »).
     */
    fun state(host: Host, info: HostInfo?, decoders: Decoders, screen: HdrScreen): HdrState {
        if (!screen.hdr) return HdrState.HIDDEN
        val json = runCatching {
            Native.hdrState(info?.raw ?: "", chipSettings(host, decoders).toString(), capabilities(host, decoders).toString(), true)
        }.getOrNull()
        return HdrState.parse(json)
    }
}
