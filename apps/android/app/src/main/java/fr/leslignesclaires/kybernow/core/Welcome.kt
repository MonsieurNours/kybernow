package fr.leslignesclaires.kybernow.core

import org.json.JSONObject

/*
 * Accueil des apps Link et incitation au compte KyberNow (docs/link-ui.md, « Accueil et compte KyberNow »). Les règles
 * sont celles du cœur (`apps/core/src/welcome.rs`, pont JNI `apps/android/jni/src/welcome.rs`) : quand montrer
 * l'accueil, la carte du compte, le bandeau après un appairage et « Hors de chez vous ? ». L'app garde l'état
 * (`WelcomeState` du cœur, en JSON) ; chaque règle le reçoit et rend le nouvel état.
 */

/** Ce qu'une règle répond : sa réponse ([value]) et l'état à garder ([state], JSON). */
data class WelcomeStep(val state: String, val value: Boolean)

object Welcome {
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

    /**
     * Applique [op] à [state] (`welcome.rs` du cœur, la seule source de ces règles). Sans réponse du cœur : l'état
     * gardé tel quel et « non » (pas d'accueil, ni carte, ni bandeau), au journal.
     */
    fun apply(state: String, op: Op, hasAccount: Boolean, computer: String = "", now: Long = nowSecs()): WelcomeStep =
        read(Core.welcome(state, op.id, hasAccount, computer, now)) ?: WelcomeStep(state, false)

    internal fun read(text: String?): WelcomeStep? = runCatching {
        val o = JSONObject(text ?: return null)
        WelcomeStep(o.getJSONObject("state").toString(), o.getBoolean("value"))
    }.getOrNull()

    /** `https://{relais}/compte/inscription?lang={fr|en}`, à ouvrir dans le navigateur de l'appareil (vide sans le cœur). */
    fun signUpUrl(relay: String = ""): String = Core.welcomeSignUpUrl(relay)

    /** La page de téléchargement de KyberNow Station (écran 3 ; vide sans le cœur). */
    val stationUrl: String get() = Core.welcomeStationUrl()

    /** Ce que l'écran 3 écrit du lien : `www.kybernow.app`. */
    fun stationLabel(url: String = stationUrl): String = url.substringAfter("://").substringBefore('/').substringBefore('#')
}
