package fr.leslignesclaires.kybernow.core

import org.json.JSONObject

/**
 * Les onze textes de l'enregistrement, tels que le cœur les écrit dans la langue de l'app (`RecordingState::ui_json`) :
 * `{time}` reste à remplir par le temps du cœur ([Core.recordingClock]), `{shortcut}` par le raccourci de l'app. Jamais
 * d'autres textes dans l'app (docs/link-ui.md, « Enregistrement de la session »).
 */
data class RecordingTexts(
    /** « Enregistrer ». */
    val record: String,
    /** « Enregistrement · {time} ». */
    val pill: String,
    /** « Enregistrer la session sur l'ordinateur ({shortcut}) » : l'infobulle de « Enregistrer ». */
    val recordTooltip: String,
    /** « Arrêter l'enregistrement ({shortcut}) » : son infobulle pendant l'enregistrement. */
    val stopTooltip: String,
    /** « Arrêter l'enregistrement ». */
    val stop: String,
    /** « Sur l'ordinateur » (sous-titre de la tuile). */
    val onComputer: String,
    /** « Enregistrer la session » : l'entrée d'un menu (et le nom donné aux lecteurs d'écran). */
    val recordMenu: String,
    /** « Arrêter l'enregistrement ({time}) » : de même, pendant l'enregistrement. */
    val stopMenu: String,
    /** Invité : « Enregistrement ». */
    val guestPill: String,
    /** Invité : « {nom} enregistre la session. » */
    val guestTooltip: String,
    /** Coupé dans Admin : « L'enregistrement est coupé sur cet ordinateur (Admin › Enregistrements). » */
    val disabled: String,
) {
    companion object {
        fun parse(o: JSONObject?) = RecordingTexts(
            record = o?.optString("record").orEmpty(),
            pill = o?.optString("pill").orEmpty(),
            recordTooltip = o?.optString("record_tooltip").orEmpty(),
            stopTooltip = o?.optString("stop_tooltip").orEmpty(),
            stop = o?.optString("stop").orEmpty(),
            onComputer = o?.optString("on_computer").orEmpty(),
            recordMenu = o?.optString("record_menu").orEmpty(),
            stopMenu = o?.optString("stop_menu").orEmpty(),
            guestPill = o?.optString("guest_pill").orEmpty(),
            guestTooltip = o?.optString("guest_tooltip").orEmpty(),
            disabled = o?.optString("disabled").orEmpty(),
        )
    }
}

/**
 * L'enregistrement de la session (docs/enregistrement.md), tel que le dit le signe de vie : [available] faux, la
 * Station n'enregistre pas (ni bouton, ni pastille, ni tuile). Le temps est celui de la Station ([elapsed] au moment
 * de l'annonce, [receivedAt] en ms de `SystemClock.elapsedRealtime`), compté ensuite par l'appareil.
 */
data class RecordingState(
    val available: Boolean,
    val allowed: Boolean,
    val active: Boolean,
    val elapsed: Long,
    val texts: RecordingTexts,
    val receivedAt: Long,
) {
    /** Secondes écoulées à [now] (même horloge que [receivedAt]). */
    fun seconds(now: Long): Long = if (!active) 0 else elapsed + ((now - receivedAt).coerceAtLeast(0) / 1000)

    /** Le temps écoulé à [now], écrit par le cœur : « 02:14 ». */
    fun clock(now: Long): String = Core.recordingClock(seconds(now))

    /** La pastille de l'hôte : « Enregistrement · 02:14 ». */
    fun pill(now: Long) = texts.pill.replace("{time}", clock(now))

    /** Le bouton ou la tuile pour l'hôte : « Enregistrer », pendant « Arrêter l'enregistrement ». */
    val action get() = if (active) texts.stop else texts.record

    /** L'infobulle du bouton, avec le raccourci de l'app. */
    fun tooltip(shortcut: String) = (if (active) texts.stopTooltip else texts.recordTooltip).replace("{shortcut}", shortcut)

    /** Le nom du bouton ou de la tuile pour les lecteurs d'écran : « Enregistrer la session », « Arrêter l'enregistrement (02:14) ». */
    fun menuLabel(now: Long) = if (active) texts.stopMenu.replace("{time}", clock(now)) else texts.recordMenu

    companion object {
        /** Le JSON du cœur (`onRecording`, réponse de [Native.consoleRecording]) ; nul s'il ne se lit pas. */
        fun parse(json: String?, now: Long): RecordingState? = runCatching { json?.let { of(JSONObject(it), now) } }.getOrNull()

        fun of(o: JSONObject?, now: Long): RecordingState? = o?.let {
            RecordingState(
                available = it.optBoolean("available"),
                allowed = it.optBoolean("allowed"),
                active = it.optBoolean("active"),
                elapsed = it.optLong("elapsed").coerceAtLeast(0),
                texts = RecordingTexts.parse(it.optJSONObject("texts")),
                receivedAt = now,
            )
        }
    }
}
