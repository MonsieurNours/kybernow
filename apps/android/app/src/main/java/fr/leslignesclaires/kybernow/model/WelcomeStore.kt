package fr.leslignesclaires.kybernow.model

import android.content.Context
import fr.leslignesclaires.kybernow.core.Welcome
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * L'état de l'accueil et de l'incitation au compte (`WelcomeState` du cœur, en JSON), gardé dans les préférences
 * `welcome` (une fois pour l'app ; les listes par ordinateur dedans) ; les règles sont celles du cœur ([Welcome]). Le
 * bandeau « après un appairage » montré en ce moment ([pairedBanner]) reste en mémoire : il ne revient pas après un
 * redémarrage de l'app (le cœur l'a déjà compté).
 */
class WelcomeStore(context: Context) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val _state = MutableStateFlow(prefs.getString(KEY, null) ?: "{}")
    /** L'état, en JSON : l'accueil le suit (la carte du compte change quand on la ferme). */
    val state: StateFlow<String> = _state.asStateFlow()
    private val _pairedBanner = MutableStateFlow<String?>(null)
    /** L'ordinateur sous lequel s'affiche le bandeau « Retrouvez-le aussi hors de chez vous… », s'il y en a un. */
    val pairedBanner: StateFlow<String?> = _pairedBanner.asStateFlow()
    private val _pairedPending = MutableStateFlow<String?>(null)
    /**
     * L'ordinateur qui vient d'être appairé, dont le bandeau attend sa place : un seul bandeau à la fois sous « Se
     * connecter », la mise à jour de la Station d'abord (docs/link-ui.md).
     */
    val pairedPending: StateFlow<String?> = _pairedPending.asStateFlow()

    private fun run(op: Welcome.Op, hasAccount: Boolean, computer: String = "", now: Long = Welcome.nowSecs()): Boolean {
        val step = Welcome.apply(_state.value, op, hasAccount, computer, now)
        if (step.state != _state.value) {
            prefs.edit().putString(KEY, step.state).apply()
            _state.value = step.state
        }
        return step.value
    }

    /** Montrer l'accueil au lancement : jamais vu dans cette version, et sans compte. */
    fun shouldShow(hasAccount: Boolean): Boolean = run(Welcome.Op.SHOULD_SHOW, hasAccount)

    /** L'accueil a été vu (fini, « Passer » ou « Plus tard »). */
    fun done() {
        run(Welcome.Op.DONE, false)
    }

    /** La carte du compte en fin de rangée des ordinateurs. */
    fun cardVisible(hasAccount: Boolean, now: Long = Welcome.nowSecs()): Boolean = run(Welcome.Op.CARD_VISIBLE, hasAccount, now = now)

    /** La croix de la carte : « Ne plus afficher » (vrai) ou « Fermer ». */
    fun cardCloseIsFinal(): Boolean = run(Welcome.Op.CARD_CLOSE_IS_FINAL, false)

    fun closeCard(now: Long = Welcome.nowSecs()) {
        run(Welcome.Op.CLOSE_CARD, false, now = now)
    }

    /**
     * Un ordinateur vient d'être appairé (sans compte) : son bandeau attend de pouvoir s'afficher (pas de bandeau de mise
     * à jour de la Station à la même place) ; rien n'est compté avant ([showPaired]).
     */
    fun paired(hasAccount: Boolean, computer: String) {
        if (!hasAccount) _pairedPending.value = computer
    }

    /**
     * Le bandeau de [computer] peut s'afficher maintenant : le cœur le compte (`take_paired_hint`, une fois par
     * ordinateur) et il s'affiche, ou il a déjà été vu et l'attente s'arrête.
     */
    fun showPaired(hasAccount: Boolean, computer: String) {
        if (_pairedPending.value != computer) return
        _pairedPending.value = null
        if (run(Welcome.Op.TAKE_PAIRED_HINT, hasAccount, computer)) _pairedBanner.value = computer
    }

    /** « Connecter un compte » ou « Plus tard » dans le bandeau : il s'en va. */
    fun dismissPairedBanner() {
        _pairedBanner.value = null
    }

    /** « Hors de chez vous ? » pour cet ordinateur : une fois par jour au plus, sans compte. */
    fun takeAwayHint(hasAccount: Boolean, computer: String, now: Long = Welcome.nowSecs()): Boolean =
        run(Welcome.Op.TAKE_AWAY_HINT, hasAccount, computer, now)

    companion object {
        private const val PREFS = "welcome"
        private const val KEY = "state"
    }
}
