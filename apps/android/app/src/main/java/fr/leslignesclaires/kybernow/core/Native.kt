package fr.leslignesclaires.kybernow.core

import android.view.Surface

/**
 * Le pont JNI du cœur Rust (`libkybernow.so`, apps/android/jni). Les objets du
 * cœur passent en `Long` (0 : aucun), libérés par la fonction `…Free`
 * correspondante. Les réponses d'Admin reviennent en JSON :
 * `{"code": 0, "value": …}` ou `{"code": …, "error": "…"}`.
 */
object Native {
    init {
        System.loadLibrary("kybernow")
    }

    /** Journal vers logcat (`KyberNow`) et langue des messages du cœur. */
    @JvmStatic external fun init(language: String)

    /** Horloge des mesures de Kyber (µs, CLOCK_MONOTONIC). */
    @JvmStatic external fun nowMicros(): Long

    // Admin (hors session). Appels bloquants : jamais sur le fil principal.
    @JvmStatic external fun consoleNew(host: String, port: Int, user: String, password: String, pinned: String?): Long
    @JvmStatic external fun consoleFree(handle: Long)
    @JvmStatic external fun consoleFingerprintSeen(handle: Long): String?
    @JvmStatic external fun consoleInfo(handle: Long): String?
    @JvmStatic external fun consoleProbe(handle: Long): String?
    @JvmStatic external fun consolePreview(handle: Long, width: Int): ByteArray?
    @JvmStatic external fun consolePairRequest(handle: Long, code: String, deviceJson: String): String?
    @JvmStatic external fun consolePairStatus(handle: Long, id: String): String?

    // La Station (réglages d'un ordinateur) : redémarrer, mettre à jour, adresse d'Admin.
    // Refus : codes 10 à 15, avec `kind`, `device` et `seconds` (voir [Reply.Failed]).
    @JvmStatic external fun consoleStationRestart(handle: Long, deviceId: String?, force: Boolean): String?
    @JvmStatic external fun consoleStationUpdate(handle: Long, deviceId: String?, force: Boolean): String?
    @JvmStatic external fun consoleAdminUrl(handle: Long): String?

    // Réveil par le réseau (Wake-on-LAN) : les cartes gardées pour l'ordinateur ([WakeTarget.toJson]).
    /** 0 : le paquet magique est parti ; 20 : aucune cible lisible ; 1 : rien n'est parti. Hors du fil principal. */
    @JvmStatic external fun wake(targetsJson: String): Int

    // KyberNow Share (docs/kybernow-share.md). Réponses comme les autres appels d'Admin ;
    // « Rejoindre » ajoute ses refus : 20 aucun partage avec ce code, 21 complet, 22 trop
    // d'essais (`error` : le texte à montrer ; 23, format illisible, seulement si l'appareil ne lit aucun format :
    // jamais, H.264 compte comme toujours lu). Hors du fil principal.
    /**
     * `targets` `[{host, port, fingerprint}]`, `request` `{code, name, device, app, quality}` (et `auto` pour « Auto »),
     * `capabilities` `{hevc, hevc444, av1}` ou `null` (le format que le cœur demande à Kyber : celui de l'hôte s'il est
     * lu, sinon un format lu, dans une image à part) ; `value` : `{target, joined}`, `joined.image` : l'image reçue.
     */
    @JvmStatic external fun shareJoin(targetsJson: String, requestJson: String, capabilitiesJson: String?): String
    /** L'invité repart avant d'ouvrir sa session ; `foundJson` : la `value` de [shareJoin]. */
    @JvmStatic external fun shareLeave(foundJson: String): Int
    /** `{host, port, fingerprint, code}`, ou `null` si ce n'est pas un lien de partage. */
    @JvmStatic external fun shareParseLink(url: String): String?
    @JvmStatic external fun shareNormalizeCode(code: String): String?
    @JvmStatic external fun shareFormatCode(code: String): String
    /** 0 Station trop ancienne, 1 partage coupé sur l'ordinateur, 2 disponible ; `infoJson` : celui de [consoleInfo]. */
    @JvmStatic external fun shareAvailability(infoJson: String): Int
    @JvmStatic external fun shareAvailabilityText(availability: Int): String?
    // Rôle d'un invité : deux droits indépendants. 0 spectateur (aucun), 1 manettes, 2 manettes avec clavier
    // et souris, 3 clavier et souris seuls ; « Joueur » dans la barre : un droit au moins.
    /** Le rôle avec les manettes données ou reprises, le clavier et la souris tels quels (−1 : code inconnu). */
    @JvmStatic external fun shareRoleWithPads(role: Int, pads: Boolean): Int
    /** Le rôle avec le clavier et la souris donnés ou repris, les manettes telles quelles (−1 : code inconnu). */
    @JvmStatic external fun shareRoleWithKbm(role: Int, kbm: Boolean): Int
    // « Qualité de l'image » d'un invité : 0 Auto, 1 comme l'hôte, 2 1080p · 60 Hz, 3 720p · 30 Hz
    // (dans `requestJson` de [shareJoin] : `quality` `auto|host|1080p60|720p30`, et pour Auto `auto`).
    @JvmStatic external fun shareQualityLabel(quality: Int): String?
    /** `screen` `{pixel_width, pixel_height, refresh}`, `decoder` `{hevc, hevc444, av1, max_width, max_height, max_fps}`, `host` l'écran partagé ou `null` ; rend `auto`. */
    @JvmStatic external fun shareAutoQuality(screenJson: String, decoderJson: String, hostJson: String?): String?
    /** « 1080p · 60 Hz » d'une image ([SessionListener.onShareImage], `guests[].image`) ; `null` tant que la taille n'est pas sue (Kyber ne dit rien). */
    @JvmStatic external fun shareImageLabel(imageJson: String): String?
    /** Session d'invité : la qualité choisie ; l'image reçue arrive par [SessionListener.onShareImage]. */
    @JvmStatic external fun sessionSetShareQuality(handle: Long, quality: Int): Boolean
    // Côté hôte (compte de l'appareil en session) ; `role` : un code ci-dessus.
    @JvmStatic external fun consoleShareOpen(handle: Long): String?
    @JvmStatic external fun consoleShareState(handle: Long): String?
    @JvmStatic external fun consoleShareCode(handle: Long): String?
    @JvmStatic external fun consoleShareGuest(handle: Long, guest: String, role: Int): String?
    @JvmStatic external fun consoleShareKick(handle: Long, guest: String): String?
    @JvmStatic external fun consoleShareClose(handle: Long): String?

    // Mises à jour de l'app (réseau : jamais sur le fil principal).
    /** `{"available": {version, channel, notes, page, size}}`, `{"none": true}` ou `{"error": "…"}`. */
    @JvmStatic external fun updateCheck(current: String, beta: Boolean): String
    /** Télécharge et vérifie la version trouvée par [updateCheck] ; `null`, ou le message d'erreur. */
    @JvmStatic external fun updateDownload(dest: String, progress: UpdateProgress?): String?

    /** Octets reçus pendant [updateDownload]. */
    interface UpdateProgress {
        fun onProgress(received: Long)
    }

    /** Réglages d'un profil (`jeu`, `bureau`, `max`) selon les décodeurs de l'appareil. */
    @JvmStatic external fun profileSettings(profile: String, hevc: Boolean, hevc444: Boolean, av1: Boolean): String

    // Session.
    @JvmStatic external fun sessionStart(params: String, surface: Surface?, listener: SessionListener, refresh: Int): Long
    @JvmStatic external fun sessionStop(handle: Long)
    /** Attend la fin du fil de session puis libère : hors du fil principal. */
    @JvmStatic external fun sessionFree(handle: Long)
    @JvmStatic external fun sessionSetSurface(handle: Long, surface: Surface?, refresh: Int)
    @JvmStatic external fun sessionSetSettings(handle: Long, json: String): Boolean
    @JvmStatic external fun sessionSetBackground(handle: Long, background: Boolean, capBitsPerSecond: Int)
    @JvmStatic external fun sessionPickDisplay(handle: Long, id: Int)
    @JvmStatic external fun sessionSetMode(handle: Long, width: Int, height: Int, refresh: Int)
    @JvmStatic external fun sessionRestoreMode(handle: Long)
    /** [médiane, p95] en ms, ou nul sans mesure. */
    @JvmStatic external fun sessionLatency(handle: Long): DoubleArray?

    // Entrées.
    /** Scancode Windows d'un code de touche Linux (`KeyEvent.getScanCode()`) ; −1 : inconnu. */
    @JvmStatic external fun scancode(evdev: Int): Int
    @JvmStatic external fun inputKey(handle: Long, scancode: Int, pressed: Boolean)
    @JvmStatic external fun inputText(handle: Long, text: String)
    @JvmStatic external fun pointerMoveBy(handle: Long, dx: Double, dy: Double)
    @JvmStatic external fun pointerMoveTo(handle: Long, x: Double, y: Double)
    @JvmStatic external fun inputMouseMove(handle: Long, dx: Int, dy: Int)
    /** 0 gauche, 1 droit, 2 milieu, 3 précédent, 4 suivant. */
    @JvmStatic external fun inputMouseButton(handle: Long, button: Int, pressed: Boolean)
    /** En crans ; positif : vers le bas, vers la droite. */
    @JvmStatic external fun inputWheel(handle: Long, dx: Float, dy: Float)
    @JvmStatic external fun inputSetRelative(handle: Long, on: Boolean)
    @JvmStatic external fun inputReleaseAll(handle: Long)
    /** Règles Auto / Bureau / Jeu ; `mode` : 0 Auto, 1 Bureau, 2 Jeu. */
    @JvmStatic external fun mouseWantsRelative(mode: Int, gameFullscreen: Boolean, remoteHidden: Boolean, visibleForMs: Double, relativeActive: Boolean): Boolean

    // Manettes (boutons et axes : voir [Pad]).
    @JvmStatic external fun padPlug(handle: Long, pad: Int, vendor: Int, product: Int)
    @JvmStatic external fun padUnplug(handle: Long, pad: Int)
    @JvmStatic external fun padButton(handle: Long, pad: Int, button: Int, pressed: Boolean)
    @JvmStatic external fun padAxis(handle: Long, pad: Int, axis: Int, value: Int)
    @JvmStatic external fun padAxesRest(handle: Long, pad: Int)
    @JvmStatic external fun padSetComboEnabled(handle: Long, enabled: Boolean)
}

/** Boutons et axes des manettes, dans l'ordre du cœur (`PadButton::ALL`, `PadAxis::ALL`). */
object Pad {
    const val A = 0
    const val B = 1
    const val X = 2
    const val Y = 3
    const val BACK = 4
    const val GUIDE = 5
    const val START = 6
    const val LEFT_STICK = 7
    const val RIGHT_STICK = 8
    const val L1 = 9
    const val R1 = 10
    const val UP = 11
    const val DOWN = 12
    const val LEFT = 13
    const val RIGHT = 14

    const val LEFT_X = 0
    const val LEFT_Y = 1
    const val RIGHT_X = 2
    const val RIGHT_Y = 3
    const val L2 = 4
    const val R2 = 5
}

/**
 * Rappels du cœur pendant une session, depuis ses propres fils (session,
 * réseau de Kyber, décodeur, entrées) : rester bref, remonter sur le fil
 * principal ce qui touche l'interface.
 */
interface SessionListener {
    companion object {
        /**
         * Avis `onNotice` : dernier recours de la connexion, la Station redémarre (l'image
         * n'arrivait pas, ou Kyber ne répondait pas). L'écran de connexion affiche « La Station
         * redémarre… » jusqu'au prochain `onStatus` (0 si elle revient, 5 puis 6 sinon). Il suit
         * toujours un `onStatus` à 1 (connexion), l'étape montrée tant que l'avis est ignoré.
         */
        const val NOTICE_STATION_RESTARTING = 7
    }

    /** 0 préparation, 1 connexion, 2 premières images, 3 image, 4 reconnexion, 5 fin, 6 terminée. */
    fun onStatus(kind: Int, attempt: Int, max: Int, error: String?, hasReport: Boolean, displayRestored: Boolean, audioRestored: Boolean, reportError: String?)
    fun onDisplay(id: Int, width: Int, height: Int)
    fun onDisplays(json: String)
    fun onMode(width: Int, height: Int, refresh: Int, virtualDisplay: Boolean)
    /**
     * Mesures de la seconde : latence (ms, −1 sans), aller-retour (ms, −1),
     * montrées, mesurées, rejetées, images perdues, trous, demandes d'image
     * clé, paquets perdus, octets, chaîne présente (1/0), puis capture,
     * encodage, envoi, réseau, décodage, affichage (ms).
     */
    fun onSecond(values: DoubleArray)
    /**
     * 0 relancée, 1 mode changé, 2 repli, 3 adaptation, 4 adaptée, 5 adaptation refusée, 6 « Remettre »,
     * 7 la Station redémarre ([NOTICE_STATION_RESTARTING]).
     */
    fun onNotice(kind: Int, width: Int, height: Int, back: Boolean, level: Int, mbps: Int, hasMode: Boolean, error: String?)
    fun onGameFullscreen(game: Boolean)
    fun onInputConnected()
    fun onVideoSize(width: Int, height: Int)
    fun onDecoder(name: String)
    fun onAudioOutput(channels: Int, bufferMs: Double)
    /** Intensités 0 à 65535 (gros moteur grave, petit aigu) ; durée en ms (0 : sans durée, 5 s au plus côté app, voir `Gamepads.rumble`). */
    fun onRumble(pad: Int, large: Int, small: Int, durationMs: Int)
    fun onCombo(pad: Int)
    /** Pointeur de l'ordinateur ; `pixels` (ARGB prémultiplié) seulement quand la forme change. */
    fun onPointer(hasPosition: Boolean, x: Double, y: Double, remoteHidden: Boolean, width: Int, height: Int, hotX: Int, hotY: Int, pixels: IntArray?)
    fun clipChangeCount(): Long
    fun clipHasText(): Boolean
    fun clipText(): String?
    fun clipSetText(text: String)

    // KyberNow Share : rien à faire tant que l'app ne partage pas.
    /** Hôte : le partage et ses invités en JSON (`open`, `code`, `link`, `web`, `display`, `guests`), `null` sans partage ; quand cela change. */
    fun onGuests(json: String?) {}
    /** Invité : son rôle (0 spectateur, 1 manettes, 2 manettes avec clavier et souris, 3 clavier et souris seuls) ; `previous` −1 au départ. */
    fun onShareRole(role: Int, previous: Int) {}
    /** Invité : le partage finit la session (0 arrêté, 1 retiré, 2 format illisible : jamais en pratique), juste avant la fin ; `text` : la raison à montrer. */
    fun onShareEnd(end: Int, text: String) {}
    /**
     * Invité : l'image reçue en JSON (`quality` : celle qu'il a demandée ; `width`, `height`, `fps`, `codec`,
     * `follows_host`, `reason` valent `null` tant que Kyber ne sait rien), celle d'avant (`null` au départ), l'avis
     * bref à montrer ou `null` (« Image allégée pour suivre. », « L'ordinateur ne peut pas faire une image de plus… »).
     */
    fun onShareImage(imageJson: String, previousJson: String?, notice: String?) {}
}
