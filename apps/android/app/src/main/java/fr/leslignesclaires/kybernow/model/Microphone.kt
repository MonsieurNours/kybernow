package fr.leslignesclaires.kybernow.model

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceInfo
import android.media.AudioManager
import org.json.JSONObject

/**
 * Le micro de l'appareil vu par la session (docs/micro.md, docs/link-ui.md « Micro ») : ce que dit le cœur
 * (`MicState::to_json`, rappel `onMicrophone`). L'app capte tant qu'il est [Waiting] ou [On].
 */
sealed class Mic {
    /** Coupé : rien n'est capté. */
    data object Off : Mic()
    /** Allumé, l'ordinateur n'a pas encore répondu : « Envoi… ». */
    data object Waiting : Mic()
    /** Allumé et pris par l'ordinateur : « Envoyé ». */
    data object On : Mic()
    /**
     * L'ordinateur ne le prend pas : « Micro » estompé, [text] la raison (le texte du cœur, link-ui) ; [reason] :
     * `not_allowed`, `no_device`, `unsupported`, `old_station`.
     */
    data class Refused(val reason: String, val text: String) : Mic()

    val capturing get() = this == On || this == Waiting

    companion object {
        const val NOT_ALLOWED = "not_allowed"

        fun parse(json: String?): Mic {
            val o = runCatching { JSONObject(json ?: return Off) }.getOrNull() ?: return Off
            return when (o.optString("state")) {
                "waiting" -> Waiting
                "on" -> On
                "refused" -> Refused(o.optString("reason"), o.optString("text"))
                else -> Off
            }
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
