package fr.leslignesclaires.kybernow.core

import org.json.JSONObject
import java.util.Locale

/**
 * Les textes de l'enregistrement, tels que le cœur les écrit dans la langue de l'app (`RecordingState::ui_json`) :
 * `{time}` reste à remplir ([RecordingState.clock]). Jamais d'autres textes dans l'app (docs/link-ui.md,
 * « Enregistrement de la session »).
 */
data class RecordingTexts(
    /** « Enregistrer ». */
    val record: String,
    /** « Enregistrement · {time} ». */
    val pill: String,
    /** « Arrêter l'enregistrement ». */
    val stop: String,
    /** « Sur l'ordinateur » (sous-titre de la tuile). */
    val onComputer: String,
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
            stop = o?.optString("stop").orEmpty(),
            onComputer = o?.optString("on_computer").orEmpty(),
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

    /** La pastille de l'hôte : « Enregistrement · 02:14 ». */
    fun pill(now: Long) = texts.pill.replace("{time}", clock(seconds(now)))

    /** Le bouton ou la tuile pour l'hôte : « Enregistrer », pendant « Arrêter l'enregistrement ». */
    val action get() = if (active) texts.stop else texts.record

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

        /** « 02:14 », « 59:59 », « 1:02:14 » (comme `recording::clock` du cœur). */
        fun clock(seconds: Long): String {
            val s = seconds.coerceAtLeast(0)
            val (h, m, r) = Triple(s / 3600, (s / 60) % 60, s % 60)
            return if (h > 0) String.format(Locale.ROOT, "%d:%02d:%02d", h, m, r) else String.format(Locale.ROOT, "%02d:%02d", m, r)
        }
    }
}
