package fr.leslignesclaires.kybernow.model

import android.view.InputDevice
import org.json.JSONObject

/**
 * « Stylet : Pression / Souris » (docs/link-ui.md, « Stylet » ; docs/stylet.md) : « Pression » (par défaut) envoie le
 * vrai stylet, « Souris » l'envoie comme une souris en position absolue (pointe posée : clic gauche ; bouton du
 * stylet : clic droit ; ni pression ni Windows Ink). Retenu par ordinateur ([HostSettings.pen]) et synchronisé (`pen`).
 * Les clés sont celles du cœur (`input::PenMode`) ; ses textes viennent du cœur ([PenChoice]).
 */
object PenMode {
    const val PRESSURE = "pressure"
    const val MOUSE = "mouse"

    /** Une clé gardée ou reçue ; inconnue (écrite par une version plus récente) : nulle, comme `PenMode::parse`. */
    fun parse(key: String?): String? = key?.takeIf { it == PRESSURE || it == MOUSE }

    /** Le raccourci (Ctrl+Alt+Maj+P) et la tuile basculent. */
    fun toggled(key: String): String = if (key == MOUSE) PRESSURE else MOUSE

    /** L'appareil a un stylet (stylet d'Android, intégré ou Bluetooth) : la tuile se montre avant qu'il ait servi. */
    fun deviceHasPen(): Boolean = runCatching {
        InputDevice.getDeviceIds().any { InputDevice.getDevice(it)?.supportsSource(InputDevice.SOURCE_STYLUS) == true }
    }.getOrDefault(false)
}

/**
 * La ligne « Stylet » telle que le cœur l'écrit (`PenChoice::ui_json`) : [shown] (un stylet a servi ou l'appareil en a
 * un, et l'ordinateur le reçoit ; toujours faux hors session), le choix ([mode]), le titre « Stylet », le sous-titre
 * de la tuile ([label] : « Pression » ou « Souris »), les deux choix (clé, libellé) et l'aide des réglages.
 */
data class PenChoice(
    val shown: Boolean = false,
    val mode: String = PenMode.PRESSURE,
    val title: String = "",
    val label: String = "",
    val choices: List<Pair<String, String>> = emptyList(),
    val help: String = "",
) {
    companion object {
        /** Illisible ou absent (pas de réponse du cœur) : rien à montrer. */
        fun parse(json: String?): PenChoice {
            if (json == null) return PenChoice()
            val o = runCatching { JSONObject(json) }.getOrNull() ?: return PenChoice()
            val a = o.optJSONArray("choices")
            val choices = (0 until (a?.length() ?: 0)).mapNotNull { i ->
                val c = a?.optJSONObject(i) ?: return@mapNotNull null
                val key = PenMode.parse(c.optString("key")) ?: return@mapNotNull null
                key to c.optString("label")
            }
            return PenChoice(
                shown = o.optBoolean("shown"),
                mode = PenMode.parse(o.optString("mode")) ?: PenMode.PRESSURE,
                title = o.optString("title"),
                label = o.optString("label"),
                choices = choices,
                help = o.optString("help"),
            )
        }
    }
}
