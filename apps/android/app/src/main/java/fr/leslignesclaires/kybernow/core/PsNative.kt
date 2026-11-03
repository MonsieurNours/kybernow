package fr.leslignesclaires.kybernow.core

import android.view.Surface

/**
 * Les consoles PlayStation dans le cœur Rust (apps/android/jni/src/playstation.rs, docs/playstation.md) : recherche,
 * compte PSN, association, réveil, hors de chez soi, partie, envoi entre appareils. Les appels qui touchent au réseau
 * bloquent : jamais sur le fil principal. Jamais au journal : l'identifiant du compte, les associations.
 */
object PsNative {
    init {
        // La bibliothèque est chargée par `Native` (son initialisation suffit).
        Native.hashCode()
    }

    /** `[{address, host_id, name, model, state, target, system_version, running_app}]`. */
    @JvmStatic external fun search(directJson: String, waitMs: Int): String
    @JvmStatic external fun searchAt(address: String, waitMs: Int): String
    @JvmStatic external fun wake(address: String, registeredJson: String): Boolean

    @JvmStatic external fun loginUrl(): String
    @JvmStatic external fun isRedirect(url: String): Boolean
    /** `{"account": "<base64>", "online_id": …}` ou `{"error": …}`. */
    @JvmStatic external fun account(redirectUrl: String): String
    @JvmStatic external fun accountIdValid(id: String): Boolean
    /** `{"registered": {…}}` ou `{"error": "no_answer" | "wrong_code" | "account" | "network" | "other"}`. */
    @JvmStatic external fun regist(address: String, target: String, account: String, pin: Int): String

    /** `GET /api/consoles` d'une console d'Admin : réponse d'Admin habituelle. */
    @JvmStatic external fun stationConsoles(console: Long): String
    /** `{"code": 0, "handle": …}` ou `{"code": 30 | 31 | 32 | −401 | −404 | 40 | 1, "error": …}`. */
    @JvmStatic external fun remoteOpen(console: Long, hostId: String): String
    @JvmStatic external fun remoteAddress(remote: Long): String?
    @JvmStatic external fun remoteState(remote: Long): String?
    @JvmStatic external fun remoteRoute(remote: Long): String?
    @JvmStatic external fun remoteFree(remote: Long)

    @JvmStatic external fun defaultSettings(): String
    @JvmStatic external fun settingsCustom(json: String): Boolean
    @JvmStatic external fun defaultLayout(): String

    /** 0 si l'adresse ou l'association ne se lisent pas. À terminer par [stop], hors du fil principal. */
    @JvmStatic external fun start(address: String, registeredJson: String, settingsJson: String, hevcDecoder: Boolean, surface: Surface?, listener: PsListener, refresh: Int): Long
    @JvmStatic external fun setSurface(play: Long, surface: Surface?, refresh: Int)
    @JvmStatic external fun stop(play: Long)
    @JvmStatic external fun key(play: Long, usage: Int, pressed: Boolean)
    @JvmStatic external fun mouseButton(play: Long, button: Int, pressed: Boolean)
    @JvmStatic external fun mouseMove(play: Long, dx: Float, dy: Float)
    @JvmStatic external fun releaseAll(play: Long)
    @JvmStatic external fun setLayout(play: Long, json: String): Boolean
    @JvmStatic external fun setIntensities(play: Long, rumble: Int, triggers: Int)
    @JvmStatic external fun setCaptured(play: Long, on: Boolean)
    @JvmStatic external fun setMuted(play: Long, muted: Boolean)
    /** `{width, height, hevc, hdr, target_kbps, fps, mbps, lost, recovered}` depuis l'appel précédent. */
    @JvmStatic external fun stats(play: Long): String?
    @JvmStatic external fun gotoBed(play: Long)
    @JvmStatic external fun keyboardText(play: Long, text: String): Boolean
    @JvmStatic external fun keyboardAccept(play: Long): Boolean
    @JvmStatic external fun keyboardReject(play: Long): Boolean

    // Les manettes de l'app, données au lecteur du cœur le temps d'une partie (boutons et axes comme `Native.pad…`).
    @JvmStatic external fun padPlug(pad: Int, vendor: Int, product: Int)
    @JvmStatic external fun padUnplug(pad: Int)
    @JvmStatic external fun padButton(pad: Int, button: Int, pressed: Boolean)
    @JvmStatic external fun padAxis(pad: Int, axis: Int, value: Int)
    @JvmStatic external fun padTouch(pad: Int, finger: Int, down: Boolean, x: Float, y: Float)
    @JvmStatic external fun padTouchClick(pad: Int, pressed: Boolean)

    // Envoi entre appareils (« Envoyer à un appareil », « Recevoir d'un autre appareil »).
    @JvmStatic external fun handoffReceive(name: String, platform: String): Long
    @JvmStatic external fun handoffCode(receiver: Long): String?
    /** `{"event":"received","bundle":…}`, `{"event":"new_code","code":…}`, `{"event":"expired"}` ou nul. */
    @JvmStatic external fun handoffNext(receiver: Long, timeoutMs: Int): String?
    @JvmStatic external fun handoffStop(receiver: Long)
    /** `[{"name","platform","address":"ip:port"}]`. */
    @JvmStatic external fun handoffDiscover(waitMs: Int): String
    /** `{"ok":true}` ou `{"error":"refused"|"unreachable"}`. */
    @JvmStatic external fun handoffSend(address: String, code: String, bundle: String): String

    // États d'une partie ([PsListener.onPsState]).
    const val CONNECTING = 0
    const val STREAMING = 1
    const val CANT_DISPLAY = 2
    const val CAN_DISPLAY = 3
    const val END_STOPPED = 10
    const val END_IN_USE = 11
    const val END_NOT_PAIRED = 12
    const val END_UNREACHABLE = 13
    const val END_REMOTE = 14

    // Clavier de la console ([PsListener.onPsKeyboard]).
    const val KEYBOARD_OPEN = 0
    const val KEYBOARD_TEXT = 1
    const val KEYBOARD_CLOSED = 2

    /** La Station ne voit pas la console ([remoteOpen]). */
    const val NOT_SEEN = 40
}

/**
 * Les rappels d'une partie, depuis les fils du cœur. Les trois derniers sont ceux du décodeur et du son (mêmes noms que
 * [SessionListener]).
 */
interface PsListener {
    fun onPsState(state: Int, width: Int, height: Int, hdr: Boolean, detail: String)
    /** Options + Create + L1 + R1 tenus 250 ms. */
    fun onPsMenu()
    /** Barre ouverte : touche de navigation de la manette (0 gauche, 1 droite, 2 haut, 3 bas, 4 valider, 5 retour). */
    fun onPsNavigate(key: Int)
    fun onPsKeyboard(event: Int, text: String)
    /** Vibration de la console pour la manette [pad] (0 à 65535 ; [durationMs] 0 : arrêt). */
    fun onPsRumble(pad: Int, large: Int, small: Int, durationMs: Int)
    fun onDecoder(name: String)
    fun onVideoSize(width: Int, height: Int)
    fun onAudioOutput(channels: Int, bufferMs: Double)
}
