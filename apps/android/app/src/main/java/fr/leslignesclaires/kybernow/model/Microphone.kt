package fr.leslignesclaires.kybernow.model

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceInfo
import android.media.AudioManager
import org.json.JSONObject

/**
 * « Micro » tel que l'interface le montre (docs/micro.md, docs/link-ui.md « Micro ») : `MicUi::ui_json` du cœur, tenu
 * par le pont (rappel `onMicrophone`). Les textes ([label], [tooltip], [tile], [reason]) sont ceux du cœur, dans la langue
 * de l'app ; l'app n'en écrit aucun. Elle capte tant que [capturing].
 */
data class Mic(
    /** Allumé par l'utilisateur. */
    val on: Boolean = false,
    /** `off`, `waiting` (allumé, l'ordinateur n'a pas encore répondu), `on` (pris), `refused`. */
    val state: String = OFF,
    /** La puce est allumée (allumé et pas refusé). */
    val lit: Boolean = false,
    /** Le son part vers l'ordinateur : l'icône près de l'indicateur de la session. */
    val sent: Boolean = false,
    /** La puce se montre (l'appareil a un micro ; jamais chez un invité que l'hôte ne laisse pas parler). */
    val shown: Boolean = true,
    /** Le refus de l'ordinateur (« Mettez à jour la Station pour envoyer le micro. »…) ; nul s'il le prend. */
    val reason: String? = null,
    /** « Micro » ou « Micro coupé ». */
    val label: String = "",
    /** L'infobulle : la raison du refus, ou l'état et le raccourci. */
    val tooltip: String = "",
    /** Le sous-titre de la puce ou de la tuile : « Envoyé », « Coupé », « Envoi… ». */
    val tile: String = "",
) {
    val capturing get() = state == WAITING || state == ON
    val refused get() = state == REFUSED

    companion object {
        const val OFF = "off"
        const val WAITING = "waiting"
        const val ON = "on"
        const val REFUSED = "refused"

        /** Le JSON du pont ; [Mic] coupé, sans texte, s'il ne se lit pas. */
        fun parse(json: String?): Mic {
            val o = runCatching { JSONObject(json ?: return Mic()) }.getOrNull() ?: return Mic()
            return Mic(
                on = o.optBoolean("on"),
                state = o.optString("state").ifEmpty { OFF },
                lit = o.optBoolean("lit"),
                sent = o.optBoolean("sent"),
                shown = o.optBoolean("shown", true),
                reason = if (o.isNull("reason")) null else o.optString("reason").takeIf { it.isNotEmpty() },
                label = o.optString("label"),
                tooltip = o.optString("tooltip"),
                tile = o.optString("tile"),
            )
        }
    }
}

/**
 * Réglages de l'app pour le micro (docs/link-ui.md, « Micro », Réglages de l'app) : le micro choisi (nul : « Celui du
 * système ») et « Réduire l'écho et le bruit » (allumé). Valent pour tous les ordinateurs ; gardés sur l'appareil.
 */
object MicPrefs {
    private fun prefs(context: Context) = context.getSharedPreferences("micro", Context.MODE_PRIVATE)

    /** La clé du micro choisi ([MicInputs.key]) ; nulle : celui du système. Un micro débranché garde sa clé. */
    fun device(context: Context): String? = prefs(context).getString("device", null)
    fun setDevice(context: Context, key: String?) = prefs(context).edit().putString("device", key).apply()

    fun voiceProcessing(context: Context): Boolean = prefs(context).getBoolean("voice", true)
    fun setVoiceProcessing(context: Context, on: Boolean) = prefs(context).edit().putBoolean("voice", on).apply()

    fun granted(context: Context) = context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
}

/** Les micros de l'appareil (`AudioManager.getDevices(GET_DEVICES_INPUTS)`), pour le choix des Réglages de l'app. */
object MicInputs {
    /** Ce qui est un micro qu'on choisit ; `TYPE_BLE_HEADSET` (26) date d'Android 12. */
    private val TYPES = setOf(
        AudioDeviceInfo.TYPE_BUILTIN_MIC, AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_USB_DEVICE,
        AudioDeviceInfo.TYPE_USB_HEADSET, AudioDeviceInfo.TYPE_USB_ACCESSORY, AudioDeviceInfo.TYPE_BLUETOOTH_SCO, 26,
    )

    /**
     * Une clé qui survit au rebranchement (l'identifiant d'Android change) : le type et le nom. Les micros intégrés
     * (bas, dos) se confondent en un seul : celui du système choisit le bon.
     */
    fun key(d: AudioDeviceInfo) = "${d.type}:${d.productName}"

    private fun inputs(context: Context): List<AudioDeviceInfo> =
        context.getSystemService(AudioManager::class.java)?.getDevices(AudioManager.GET_DEVICES_INPUTS)?.filter { it.type in TYPES }.orEmpty()

    /** `clé` → nom du micro, sans doublon ; [fallback] pour un micro sans nom. */
    fun list(context: Context, fallback: String): List<Pair<String, String>> =
        inputs(context).distinctBy(::key).map { key(it) to (it.productName?.toString()?.takeIf { n -> n.isNotBlank() } ?: fallback) }

    /** Le micro choisi s'il est branché ; nul sinon (celui du système, sans oublier le choix). */
    fun preferred(context: Context): AudioDeviceInfo? {
        val key = MicPrefs.device(context) ?: return null
        return inputs(context).firstOrNull { key(it) == key }
    }
}
