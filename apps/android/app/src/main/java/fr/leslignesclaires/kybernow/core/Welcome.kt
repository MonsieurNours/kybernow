package fr.leslignesclaires.kybernow.core

import org.json.JSONArray
import org.json.JSONObject

/*
 * Accueil des apps Link et incitation au compte KyberNow (docs/link-ui.md, « Accueil et compte KyberNow »). Les règles
 * sont celles du cœur (`apps/core/src/welcome.rs`, pont JNI `apps/android/jni/src/welcome.rs`) : quand montrer
 * l'accueil, la carte du compte, le bandeau après un appairage et « Hors de chez vous ? ». L'app garde l'état
 * (`WelcomeState` du cœur, en JSON) ; chaque règle le reçoit et rend le nouvel état.
 *
 * Sans bibliothèque native (tests JVM), les mêmes règles sont appliquées ici ([Fallback]) : jamais une chute de l'app.
 */

/** Ce qu'une règle répond : sa réponse ([value]) et l'état à garder ([state], JSON). */
data class WelcomeStep(val state: String, val value: Boolean)

object Welcome {
    /** Le relais public, pour « Créer un compte » et l'aide de l'écran 2. */
    const val DEFAULT_RELAY = Accounts.DEFAULT_RELAY

    /** Les opérations du cœur (`Native.welcome`). */
    enum class Op(val id: String) {
        SHOULD_SHOW("should_show"),
        DONE("done"),
        CARD_VISIBLE("card_visible"),
        CARD_CLOSE_IS_FINAL("card_close_is_final"),
        CLOSE_CARD("close_card"),
        TAKE_PAIRED_HINT("take_paired_hint"),
        TAKE_AWAY_HINT("take_away_hint"),
    }

    fun nowSecs(): Long = System.currentTimeMillis() / 1000

    /** Applique [op] à [state] ; le cœur d'abord, ses règles recopiées sans bibliothèque native. */
    fun apply(state: String, op: Op, hasAccount: Boolean, computer: String = "", now: Long = nowSecs()): WelcomeStep =
        runCatching { read(Native.welcome(state, op.id, hasAccount, computer, now)) }.getOrNull()
            ?: Fallback.apply(state, op, hasAccount, computer, now)

    internal fun read(text: String?): WelcomeStep? = runCatching {
        val o = JSONObject(text ?: return null)
        WelcomeStep(o.getJSONObject("state").toString(), o.getBoolean("value"))
    }.getOrNull()

    /** `https://{relais}/compte/inscription?lang={fr|en}`, à ouvrir dans le navigateur de l'appareil. */
    fun signUpUrl(relay: String = "", language: String = "fr"): String =
        runCatching { Native.welcomeSignUpUrl(relay) }.getOrNull()?.takeIf { it.isNotBlank() } ?: Fallback.signUpUrl(relay, language)

    /** La page de téléchargement de KyberNow Station (écran 3). */
    val stationUrl: String get() = runCatching { Native.welcomeStationUrl() }.getOrNull()?.takeIf { it.isNotBlank() } ?: Fallback.STATION_URL

    /** Ce que l'écran 3 écrit du lien : `www.kybernow.app`. */
    fun stationLabel(url: String = stationUrl): String = url.substringAfter("://").substringBefore('/').substringBefore('#')

    /** Les règles de `welcome.rs`, recopiées pour les tests JVM (sans `libkybernow.so`). */
    internal object Fallback {
        const val VERSION = 1
        const val CARD_BACK_AFTER_SECS = 30L * 24 * 3600
        const val AWAY_HINT_EVERY_SECS = 24L * 3600
        const val STATION_URL = "https://www.kybernow.app/#telecharger"

        fun signUpUrl(relay: String, language: String): String {
            val host = relay.trim().removePrefix("https://").trimEnd('/').ifEmpty { "share.kybernow.app" }
            return "https://$host/compte/inscription?lang=${if (language == "fr") "fr" else "en"}"
        }

        fun apply(state: String, op: Op, hasAccount: Boolean, computer: String, now: Long): WelcomeStep {
            val o = runCatching { JSONObject(state) }.getOrElse { JSONObject() }
            val seen = o.optInt("welcome_seen", 0)
            val closed = o.optInt("card_closed", 0)
            val closedAt = o.optLong("card_closed_at", 0)
            val paired = o.optJSONArray("paired_hint") ?: JSONArray()
            val away = o.optJSONArray("away_hint") ?: JSONArray()
            val value = when (op) {
                Op.SHOULD_SHOW -> !hasAccount && seen < VERSION
                Op.DONE -> { o.put("welcome_seen", VERSION); true }
                Op.CARD_VISIBLE -> !hasAccount && when (closed) {
                    0 -> true
                    1 -> now - closedAt >= CARD_BACK_AFTER_SECS
                    else -> false
                }
                Op.CARD_CLOSE_IS_FINAL -> closed >= 1
                Op.CLOSE_CARD -> { o.put("card_closed", (closed + 1).coerceAtMost(2)).put("card_closed_at", now); true }
                Op.TAKE_PAIRED_HINT -> if (hasAccount || (0 until paired.length()).any { paired.optString(it) == computer }) false else {
                    o.put("paired_hint", paired.put(computer))
                    true
                }
                Op.TAKE_AWAY_HINT -> if (hasAccount) false else {
                    val i = (0 until away.length()).firstOrNull { away.optJSONArray(it)?.optString(0) == computer }
                    when {
                        i == null -> { o.put("away_hint", away.put(JSONArray().put(computer).put(now))); true }
                        now - away.getJSONArray(i).optLong(1) < AWAY_HINT_EVERY_SECS -> false
                        else -> { away.getJSONArray(i).put(1, now); o.put("away_hint", away); true }
                    }
                }
            }
            return WelcomeStep(o.toString(), value)
        }
    }
}
