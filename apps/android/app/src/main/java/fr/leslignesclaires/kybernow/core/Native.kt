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

    /**
     * PyroWave (docs/pyrowave-client-android.md) : la sonde du lancement, hors du fil principal (device Vulkan,
     * décodeur, image de test décodée et relue). `{"available": bool, "report": "GPU, chemin" ou la raison}`.
     */
    @JvmStatic external fun pyrowaveProbe(): String

    /** Horloge des mesures de Kyber (µs, CLOCK_MONOTONIC). */
    @JvmStatic external fun nowMicros(): Long

    // Profil Auto (docs/profil-auto.md ; apps/android/jni/src/autotest.rs).
    /**
     * Lance le test des conditions sur un fil du cœur : [console] est une poignée de `consoleNew…` que le test prend
     * ([ConsoleClient.detach] : ne pas la libérer) ; [deviceJson] `{screen: {width, height, refresh}, caps: {hevc,
     * hevc444, av1, hdr, pyrowave}, hdr, network, wired}` ; [name] le nom de l'ordinateur. [listener] reçoit
     * l'avancement puis la fin, depuis ce fil. 0 si rien n'a pu partir. À rendre par [autotestFree].
     */
    @JvmStatic external fun autotestStart(console: Long, deviceJson: String, name: String, listener: AutotestListener): Long
    /** « Annuler » : le test s'arrête entre deux mesures (`onDone` : `{"cancelled": true}`). */
    @JvmStatic external fun autotestCancel(handle: Long)
    /** Rend la poignée : le test s'arrête et ne dit plus rien. */
    @JvmStatic external fun autotestFree(handle: Long)
    /** Le résumé d'un résultat gardé, dans la langue de l'app : `[{key, title, value, reason}]` ; nul s'il est illisible. */
    @JvmStatic external fun autotestChoices(resultJson: String): String?
    /** Les réglages de session d'Auto pour ce résultat (`AutoResult::settings`, profil `auto`) ; nul s'il est illisible. */
    @JvmStatic external fun autotestSettings(resultJson: String, hdr: Boolean, wired: Boolean): String?
    /**
     * « Appliquer » un nouveau résultat : le plafond de l'encodeur à garder pour l'ordinateur (Mbit/s par écran, 0 : aucun ;
     * `AutoResult::next_encoder_max`), d'après celui gardé ([previous], 0 : aucun) ; un résultat illisible le laisse.
     */
    @JvmStatic external fun autotestEncoderMax(resultJson: String, previous: Int): Int
    /** Le test a-t-il été fait sur ce réseau ([network] : celui du moment, `relay` par le relais) ? */
    @JvmStatic external fun autotestSameNetwork(resultJson: String, network: String?): Boolean

    // Admin (hors session). Appels bloquants : jamais sur le fil principal.
    @JvmStatic external fun consoleNew(host: String, port: Int, user: String, password: String, pinned: String?): Long
    /** Comme [consoleNew], par le relais quand `routeJson` est donné (`{server, station, admin_fp}`) ; erreurs : 30 relais muet, 31 ordinateur éteint ou en veille, 32 accès coupé dans Admin. */
    @JvmStatic external fun consoleNewRelay(host: String, port: Int, user: String, password: String, pinned: String?, routeJson: String?): Long
    // Le relais (docs/relais.md) : la route d'un ordinateur d'après son `info` et l'empreinte connue, la présence chez le relais,
    // `KYBERNOW_RELAY` (0 normal, 1 toujours, 2 jamais), la ligne « Chemin » (0 Direct, 1 Par le relais, 2 Par le relais (TCP)).
    @JvmStatic external fun relayRouteFromInfo(infoJson: String, adminFp: String): String?
    @JvmStatic external fun relayPresence(relay: String, stationsJson: String): String
    @JvmStatic external fun relayMode(): Int
    @JvmStatic external fun relayPathLabel(transport: Int): String?
    /** Le relais sous sa forme normale (`share.kybernow.app` pour le 443, `relais.exemple.org:9443` sinon) ; `null` s'il ne se lit pas. */
    @JvmStatic external fun relayNormalize(addr: String): String?
    /** Le texte de link-ui d'une erreur du relais (30, 31, 32) avec le nom de l'ordinateur ; `null` pour un autre code. */
    @JvmStatic external fun relayErrorText(code: Int, name: String?): String?
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

    // Mise à jour des Stations proposée par l'app (docs/link-ui.md, « Mise à jour des Stations » ; cœur : `station_offer`).
    /** `POST /api/stream/station/check` : `value`, l'objet `station` d'`info`, nul avant la Station 0.11.3. Bloquant (20 s au plus). */
    @JvmStatic external fun consoleStationCheck(handle: Long): String?
    /** `{station, may_update, online, away, in_session, now, memory, app_version}` → `{banner: {kind, version}, chip, notice_version, should_check}`. Pur. */
    @JvmStatic external fun stationOfferEvaluate(inputJson: String): String
    /** Premier lancement après une mise à jour de l'app ([last] : la dernière version lancée, nulle au tout premier lancement). */
    @JvmStatic external fun stationOfferAppUpdated(last: String?, current: String): Boolean

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
    /**
     * « Rejoindre » sur le réseau et hors de chez soi (docs/relais.md) : `relays` `["relais.exemple.org:9443", …]`, les relais
     * des ordinateurs connus (le relais public, `share.kybernow.app`, est toujours interrogé en plus) ; `value` : `{"joined": {target, joined}}` ou `{"pending": {route, guest, secret, expires}}` ;
     * refus en plus : 24 refusé, 25 sans réponse de l'hôte, 26 trop de demandes en attente, 30 relais muet, 31 ordinateur absent ;
     * 29 : un code de prêt de l'ordinateur, `value` `{code, target, route}` à donner à [lendBorrow] ;
     * 33 : un code d'invitation, `value` `{code, target, route, computer}` à donner à [inviteRedeem].
     */
    @JvmStatic external fun shareJoinAnywhere(targetsJson: String, relaysJson: String, requestJson: String, capabilitiesJson: String?): String
    /** Un lien de partage ([shareParseLink]) : le réseau d'abord, puis le relais (`relay`, `sid`) ; même `value`. */
    @JvmStatic external fun shareJoinLink(linkJson: String, requestJson: String, capabilitiesJson: String?): String
    /** L'hôte a-t-il répondu ? Tenu 25 s : `value` `{target, joined}` (accepté), code 27 (en attente : rappeler), ou un refus (24, 25, 28 partage arrêté). */
    @JvmStatic external fun shareWait(pendingJson: String, capabilitiesJson: String?): String
    /** « Annuler » une demande en attente. */
    @JvmStatic external fun shareCancel(pendingJson: String): Int
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
    // « Qualité de l'image » d'un invité : 0 Auto, 1 comme l'hôte, 2 1080p · 60 Hz, 3 720p · 30 Hz, 4 4K · 60 Hz
    // (menus : Auto, comme l'hôte, 4K, 1080p, 720p ; dans `requestJson` de [shareJoin] : `quality`
    // `auto|host|2160p60|1080p60|720p30`, et `auto`).
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
    /** « Accepter » la demande d'un invité venu par le relais (`guests[].pending`) ; `value` : le partage à jour. Refus 409 : quatre places prises. */
    @JvmStatic external fun consoleShareAccept(handle: Long, guest: String): String?
    /** « Refuser » la demande d'un invité venu par le relais ; `value` : le partage à jour. */
    @JvmStatic external fun consoleShareRefuse(handle: Long, guest: String): String?
    @JvmStatic external fun consoleShareKick(handle: Long, guest: String): String?
    @JvmStatic external fun consoleShareClose(handle: Long): String?

    // « Regarder » (droit `view` d'un compte KyberNow, docs/comptes.md ; docs/kybernow-share.md, « Regarder avec un compte »).
    // Réponses comme « Rejoindre » ; refus en plus : 34 Station qui ne sait pas encore regarder, 35 ordinateur prêté,
    // 36 partage coupé sur l'ordinateur (`error` : le texte à montrer). Bloquants (12 s au plus), hors du fil principal.
    /**
     * `access` `{account, station}` ([Accounts.access]), `lan` `{host, port, fingerprint}` (Admin vu sur le réseau) ou
     * `null`, `route` ([accountRoute]) ou `null`, `request` `{app, display?, quality, auto?}`, `capabilities` ou `null` ;
     * `value` : `{target, joined}`, `joined.watch` `{screens, switch, display}`.
     */
    @JvmStatic external fun shareWatch(accessJson: String, lanJson: String?, routeJson: String?, requestJson: String, capabilitiesJson: String?): String?
    /** Comme [shareWatch], par une console de compte déjà ouverte ([consoleNewAccount]). */
    @JvmStatic external fun consoleShareWatch(handle: Long, requestJson: String, capabilitiesJson: String?): String?
    /** « Regarder {nom} » : le titre des étapes de la connexion. */
    @JvmStatic external fun shareWatchTitle(computer: String): String
    /** `action` « Regarder », `stop` « Arrêter de regarder », `screen` « Écran ». */
    @JvmStatic external fun shareWatchWord(word: String): String
    /** Une ligne du menu « Écran » (« Écran 1 — 3840 × 2160 ») d'un `joined.watch.screens[]` ; `null` si illisible. */
    @JvmStatic external fun shareWatchScreenLabel(screenJson: String): String?
    /** Session de qui regarde : l'écran choisi (un `id` de `joined.watch.screens`). */
    @JvmStatic external fun sessionWatchScreen(handle: Long, display: Long)
    /** Session de qui regarde : l'écran regardé (0 hors d'une telle session). */
    @JvmStatic external fun sessionWatchDisplay(handle: Long): Long
    /** L'ordinateur du compte (JSON de [accountComputers]) a le droit « Regarder ». */
    @JvmStatic external fun accountCanWatch(computerJson: String): Boolean
    /** « Regarder » sans « Ouvrir le bureau » : la carte propose « Regarder » à la place de « Se connecter ». */
    @JvmStatic external fun accountWatchOnly(computerJson: String): Boolean

    // Prêt de l'ordinateur (docs/pret.md). L'ami : `target` `{host, port, fingerprint}`, `route` sa route par le relais
    // ou `null` ; refus 40 identifiant ou mot de passe, 41 code qui ne sert plus, 42 déjà prêté, 43 verrouillé, 44 prêt
    // coupé, 45 trop d'essais, 46 refusé, 47 sans réponse, 48 Station sans prêt. Bloquants.
    /** La carte « Ordinateur de {hôte} » : `value` `{state, host, computer, slot}`. */
    @JvmStatic external fun lendStatus(targetJson: String, routeJson: String?, login: String, password: String): String
    /**
     * « Emprunter » : `request` `{"code"}` ou `{"login","password"}`, `name`, `device`, `app`, `screen` `{width, height,
     * refresh}`, `codecs` ; `value` `{"ready": {target, borrowed}}` (session : `borrower.borrowed`) ou `{"pending": {…}}`.
     */
    @JvmStatic external fun lendBorrow(targetJson: String, routeJson: String?, requestJson: String): String
    /** L'hôte a-t-il répondu ? Tenu 30 s : `value` `{target, borrowed}`, code 49 (rappeler), 46, 47. */
    @JvmStatic external fun lendWait(pendingJson: String): String
    /** « Annuler » une demande en attente. */
    @JvmStatic external fun lendCancel(pendingJson: String): Int
    /** « Rendre l'ordinateur » hors d'une session (`loan` : `{target, borrowed}`) ; en session : [sessionGiveBack]. */
    @JvmStatic external fun lendLeave(loanJson: String): String
    /** Un lien d'invitation : `{host, port, fingerprint, login, relay, sid, name, route}` (`name` : le prénom de l'hôte ou `null`) (sert de `target`), ou `null`. */
    @JvmStatic external fun lendParseInvite(url: String): String?
    /** L'état de la carte (`free`, `busy`, `lent`, `locked`, `off`) : « Disponible », « Occupé »… */
    @JvmStatic external fun lendAvailabilityLabel(state: String): String?
    /** Le texte d'un préavis `{reason, at}` ; `name` : l'hôte chez l'ami, l'ami pour `slot_start` chez l'hôte. */
    @JvmStatic external fun lendNoticeText(noticeJson: String, name: String): String?
    // L'hôte, sur la console de l'ordinateur : `value` = la vue du prêt (`enabled`, `state`, `borrower`, `notice`,
    // `requests`, `code`), comme [SessionListener.onLend].
    @JvmStatic external fun consoleLendState(handle: Long): String?
    /** « Créer un code » : `duration` en secondes (0 : « Jusqu'à ce que je reprenne »), `steam` 0 son compte, 1 le vôtre. */
    @JvmStatic external fun consoleLendCode(handle: Long, duration: Long, steam: Int): String?
    @JvmStatic external fun consoleLendCancelCode(handle: Long): String?
    /** « Accepter » (`accept`, avec `steam`) ou « Refuser » une demande (`requests[].id`). */
    @JvmStatic external fun consoleLendAnswer(handle: Long, request: String, accept: Boolean, steam: Int): String?
    /** « Reprendre l'ordinateur » (préavis de 2 min) ou, `now`, « Reprendre tout de suite ». */
    @JvmStatic external fun consoleLendTakeBack(handle: Long, now: Boolean): String?

    // Invitations (docs/invitations.md) : ajouter un ordinateur de loin, sans accord sur place. Bloquant (15 s au plus).
    /**
     * « Ajouter l'ordinateur » : `codeJson` la `value` du code 33 de « Rejoindre », `password` celui de l'invitation,
     * `deviceJson` `{id, name, kind, os, app}` ; `value` `{hostname, address, port, fingerprint, username, password, relay,
     * invite_name, expires, via_relay}` ; refus 60 mot de passe refusé, 61 code qui ne sert plus, 62 invitation complète,
     * 63 trop d'essais, 64 accès hors de chez vous coupé, 65 le reste (`error` : le texte à montrer).
     */
    @JvmStatic external fun inviteRedeem(codeJson: String, password: String, deviceJson: String): String

    // Comptes KyberNow (docs/comptes.md, lot 3 ; textes : docs/link-ui.md, « Comptes KyberNow »). Réponses
    // `{"code": 0, "value": …}` ou `{"code": 70…80, "error", "kind"}` (voir [AccountCode]) ; 1 : JSON illisible.
    // Les appels marqués bloquants : jamais sur le fil principal (8 s au plus chacun).
    /** La forme normale d'un relais tapé (« Un autre relais… ») ; vide : `share.kybernow.app` ; refus 78. */
    @JvmStatic external fun accountNormalizeRelay(relay: String): String
    /** « Se connecter à un compte KyberNow » : `value` `{relay, url, state, callback_scheme}`, à garder jusqu'au retour. */
    @JvmStatic external fun accountSignIn(relay: String, deviceName: String): String
    /** Le retour `kybernow://compte?…` : `value`, le compte à garder entier (jeton compris). 75 : annulé. Bloquant. */
    @JvmStatic external fun accountFinishSignIn(signInJson: String, callback: String): String
    /** Vrai pour un retour de la connexion (`kybernow://compte…`). */
    @JvmStatic external fun accountIsCallback(url: String): Boolean
    /** Android TV : un code, `value` `{relay, code, poll, expires, interval, url, url_complete, url_label, issued_at}`. Bloquant. */
    @JvmStatic external fun accountTvCode(relay: String, deviceName: String): String
    /** Une demande : `value` `{"status":"pending"}` ou `{"status":"approved","account":{…}}` ; 73 échu, 72 refusé. Bloquant. */
    @JvmStatic external fun accountTvPoll(codeJson: String): String
    /** Minutes avant l'échéance du code (« Le code change dans {n} min. »). */
    @JvmStatic external fun accountTvMinutesLeft(codeJson: String): Int
    /** « Se déconnecter » chez le relais (l'app oublie le compte de toute façon). Bloquant. */
    @JvmStatic external fun accountSignOut(accountJson: String): String
    /** Les réglages « Comptes » : `value` `{key, relay, relay_label, account, device, console}`, sans le jeton. */
    @JvmStatic external fun accountSummary(accountJson: String): String
    /** Les ordinateurs du compte : `value` `[{account, relay, station, name, rights, owner, mine, online, admin_fp, attached_at}]` ; 70. Bloquant. */
    @JvmStatic external fun accountComputers(accountJson: String): String
    /** La liste qui arrive (`fresh`) avec les empreintes d'Admin déjà vues (`known`) : `value`, la liste. */
    @JvmStatic external fun accountRememberFingerprints(freshJson: String, knownJson: String): String
    /** Un ordinateur une fois : `paired` `[{id, fingerprint, station}]`, `computers` ceux des comptes ; `value` `{linked: [[id, ordinateur]], added: [ordinateur]}`. */
    @JvmStatic external fun accountMerge(pairedJson: String, computersJson: String): String
    /** La pastille d'origine (« Compte », « Compte · {relais} », l'organisation, « Ouvert par {prénom} ») ; `paired` : « Appairé ». */
    @JvmStatic external fun accountOrigin(computerJson: String, severalRelays: Boolean, paired: Boolean): String
    /** La route par le relais d'un ordinateur du compte (`{server, station, admin_fp}`), nulle sans empreinte d'Admin connue. */
    @JvmStatic external fun accountRoute(computerJson: String): String?
    /**
     * La console d'un ordinateur du compte (comme [consoleNewRelay]) : `access` `{account, station}`, `host` et `port`
     * d'Admin sur le réseau (vides sinon), `pinned` l'empreinte d'Admin, `route` par le relais ([accountRoute]), nul sur
     * le réseau. 0 pour un JSON illisible. Ses refus : 70 et suivants en plus de ceux d'Admin et du relais.
     */
    @JvmStatic external fun consoleNewAccount(host: String, port: Int, pinned: String?, accessJson: String, routeJson: String?): Long

    // Réglages synchronisés (docs/comptes.md, « Réglages synchronisés » ; apps/core/README.md). Réponses comme les
    // autres appels des comptes (`{"code", "value"}`) ; 70 oublier le compte, 71 l'ordinateur n'est plus au compte,
    // 77 relais d'avant les réglages synchronisés (garder les réglages locaux sans rien dire).
    /** Les réglages de tous les ordinateurs du compte : `value` `[{station, updated_at, values}]`. Bloquant. */
    @JvmStatic external fun accountSettingsFetch(accountJson: String): String
    /**
     * Envoie les clés changées (`values`, `{profile, codec, yuv444, mbps, hdr, immersive}` facultatives) posées sur
     * `last` (le dernier `{station, updated_at, values}` connu, nul sinon) : `value`, le nouveau à garder. Bloquant.
     */
    @JvmStatic external fun accountSettingsPut(accountJson: String, station: String, lastJson: String?, valuesJson: String): String
    /** Les clés connues et vérifiées d'un `{station, updated_at, values}` : `value` `{profile, codec, yuv444, mbps, hdr, immersive}`. */
    @JvmStatic external fun accountSettingsKnown(syncedJson: String): String
    /** Les clés à envoyer d'après des réglages de session (`settings` de [sessionStart]) et le mode immersif (nul ici). */
    @JvmStatic external fun accountSettingsFromStream(settingsJson: String, immersive: String?): String
    /** Les réglages de session `base` avec les clés reçues, ramenés à ce que l'appareil décode (`caps`) : `value`, le JSON de `base`. */
    @JvmStatic external fun accountSettingsApply(valuesJson: String, baseJson: String, capsJson: String): String
    /** « Synchroniser les réglages des ordinateurs » et son aide : `[titre, aide]` en JSON. */
    @JvmStatic external fun accountSyncSettingsTexts(): String

    // Mises à jour de l'app (réseau : jamais sur le fil principal).
    /** `{"available": {version, channel, notes, page, size}}`, `{"none": true}` ou `{"error": "…"}`. */
    @JvmStatic external fun updateCheck(current: String, beta: Boolean): String
    /** Télécharge et vérifie la version trouvée par [updateCheck] ; `null`, ou le message d'erreur. */
    @JvmStatic external fun updateDownload(dest: String, progress: UpdateProgress?): String?

    /** Octets reçus pendant [updateDownload]. */
    interface UpdateProgress {
        fun onProgress(received: Long)
    }

    // Face à une Station Mac (docs/link-ui.md, « Face à une Station Mac » ; règles du cœur, `keymap`).
    /** La Station est un Mac : son système (`os` de l'info, « macOS 26.1 ») commence par « macOS ». */
    @JvmStatic external fun stationIsMac(os: String): Boolean
    /** « Ctrl agit comme ⌘ » pour cet ordinateur : face à un Mac seulement, allumé par défaut ; `setting` −1 jamais changé, 0, 1. */
    @JvmStatic external fun controlAsCommand(setting: Int, os: String): Boolean
    /** L'ordinateur crée les manettes de l'appareil (`gamepads` de l'info, JSON de [consoleInfo]) ; vrai sans le champ. */
    @JvmStatic external fun gamepadsAvailable(infoJson: String): Boolean

    /** Réglages d'un profil (`jeu`, `bureau`, `max`) selon les décodeurs de l'appareil. */
    @JvmStatic external fun profileSettings(profile: String, hevc: Boolean, hevc444: Boolean, av1: Boolean): String

    // Session.
    @JvmStatic external fun sessionStart(params: String, surface: Surface?, listener: SessionListener, refresh: Int): Long
    @JvmStatic external fun sessionStop(handle: Long)
    /** Session d'emprunt : « Rendre l'ordinateur » ([sessionStop] ne le rend pas : retour possible pendant 15 min). */
    @JvmStatic external fun sessionGiveBack(handle: Long)
    /** Attend la fin du fil de session puis libère : hors du fil principal. */
    @JvmStatic external fun sessionFree(handle: Long)
    @JvmStatic external fun sessionSetSurface(handle: Long, surface: Surface?, refresh: Int)
    @JvmStatic external fun sessionSetSettings(handle: Long, json: String): Boolean
    @JvmStatic external fun sessionSetBackground(handle: Long, background: Boolean, capBitsPerSecond: Int)
    /** PyroWave : débit du lien vers l'ordinateur en Mbit/s, 0 inconnu (docs/pyrowave.md, « Définition ramenée »). */
    @JvmStatic external fun sessionSetLinkMbps(handle: Long, mbps: Int)
    @JvmStatic external fun sessionPickDisplay(handle: Long, id: Int)
    @JvmStatic external fun sessionSetMode(handle: Long, width: Int, height: Int, refresh: Int)
    @JvmStatic external fun sessionRestoreMode(handle: Long)
    /** [médiane, p95] en ms, ou nul sans mesure. */
    @JvmStatic external fun sessionLatency(handle: Long): DoubleArray?
    /**
     * La ligne « Débit visé » des Statistiques (valeurs 22 à 24 de [SessionListener.onSecond] : débit demandé et du
     * profil en bit/s, raison) : « 42 / 60 Mbit/s · baissé, réseau saturé » ; nul pour une raison inconnue.
     */
    @JvmStatic external fun targetBitrateText(bitrate: Long, profile: Long, reason: Int): String?
    /** Le libellé de la ligne : « Débit visé » / “Target bitrate”. */
    @JvmStatic external fun targetBitrateLabel(): String

    // HDR (docs/hdr.md).
    /** Le choix « HDR » changé en session (barre, menu) : l'image est relancée, [SessionListener.onHdr] suit. */
    @JvmStatic external fun sessionSetHdr(handle: Long, hdr: Boolean)
    /** L'écran de l'appareil gagne ou perd le HDR, ou ses valeurs changent (`luminanceJson` : `{max, full_frame, min, sdr_white}`, nul : par défaut). */
    @JvmStatic external fun sessionScreenHdrChanged(handle: Long, hdr: Boolean, luminanceJson: String?)
    /** Un écran au moins a été en HDR pendant la session (« HDR » de la dernière session). */
    @JvmStatic external fun sessionHdrUsed(handle: Long): Boolean
    /**
     * La puce « HDR » (`profile::hdr_state` du cœur) : `{shown, available, reason, kind}`. `infoJson` : [HostInfo.raw]
     * (vide : pas encore joint) ; `settingsJson` : `codec` et `profile` ; `capabilitiesJson` : `{hevc, hevc444, av1, hdr}`.
     */
    @JvmStatic external fun hdrState(infoJson: String, settingsJson: String, capabilitiesJson: String, screenHdr: Boolean): String?

    // Entrées.
    /** Scancode Windows d'un code de touche Linux (`KeyEvent.getScanCode()`) ; −1 : inconnu. */
    @JvmStatic external fun scancode(evdev: Int): Int
    /** Touche envoyée telle quelle (combinaisons des menus, rangée du clavier de l'écran). */
    @JvmStatic external fun inputKey(handle: Long, scancode: Int, pressed: Boolean)
    /** Touche d'un clavier physique : face à un Mac, Ctrl et ⊞ échangés avec « Ctrl agit comme ⌘ » (le cœur décide). */
    @JvmStatic external fun inputKeyTyped(handle: Long, scancode: Int, pressed: Boolean)
    /** « Ctrl agit comme ⌘ » changé pendant la session : tout de suite. */
    @JvmStatic external fun inputSetControlAsCommand(handle: Long, on: Boolean)
    @JvmStatic external fun inputText(handle: Long, text: String)
    @JvmStatic external fun pointerMoveBy(handle: Long, dx: Double, dy: Double)
    @JvmStatic external fun pointerMoveTo(handle: Long, x: Double, y: Double)
    /**
     * Stylet (docs/link-ui.md, « Stylet »), état complet, en pixels de l'écran de l'ordinateur. `tilt` et `orientation` :
     * `AXIS_TILT` et `AXIS_ORIENTATION` en radians, lus si `hasTilt`. `false` : le stylet part en souris (Station
     * d'avant, souris des jeux), l'app envoie la position et le clic gauche.
     */
    @JvmStatic external fun inputPen(
        handle: Long, x: Double, y: Double, inRange: Boolean, contact: Boolean, pressure: Float,
        hasTilt: Boolean, tilt: Float, orientation: Float, eraser: Boolean, barrel: Boolean,
    ): Boolean
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
        /** Avis `onNotice` : l'ordinateur n'a pas pu passer en HDR, l'image est en SDR (`error` : le texte du cœur ; une fois par session). */
        const val NOTICE_HDR_REFUSED = 8
        /**
         * Avis `onNotice` : PyroWave, définition ramenée à `width` × `height` pour tenir dans `mbps`
         * (`hasMode` ; `error` : le texte du bandeau), ou revenue à celle de l'appareil (`hasMode` faux).
         * Aussi dans `onSecond`, valeurs 18 à 21 (largeur, hauteur, fréquence, plafond ; 0 sans).
         */
        const val NOTICE_RESOLUTION_LOWERED = 9
        /**
         * Avis `onNotice` : PyroWave, le réseau sature depuis 5 s (docs/debit-automatique.md) ; bandeau « Le réseau
         * sature » avec « Passer en HEVC ». Une fois par session, hôte seulement.
         */
        const val NOTICE_PYROWAVE_STRAINED = 10
        /**
         * Avis `onNotice` : l'encodeur de l'ordinateur a refusé l'image, la session a dû redescendre à `mbps` (Mbit/s par
         * écran) : le plafond à garder pour cet ordinateur ([fr.leslignesclaires.kybernow.model.HostSettings.encoderMax]) et
         * à redonner aux sessions suivantes (`encoder_max`). Rien à l'écran.
         */
        const val NOTICE_ENCODER_CEILING = 11
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
     * encodage, envoi, réseau, décodage, affichage (ms), puis l'image en HDR (1/0) ; 18 à 21 la définition
     * ramenée de PyroWave ; 22 à 24 le débit visé (demandé et du profil en bit/s, raison `TargetReason::code` :
     * 0 Auto, 1 baissé, 2 à la main, 3 PyroWave en pause, 4 plafond de l'app, 5 repli, 6 plafond de l'encodeur ; −1 sans).
     */
    fun onSecond(values: DoubleArray)
    /**
     * 0 relancée, 1 mode changé, 2 repli, 3 adaptation, 4 adaptée, 5 adaptation refusée, 6 « Remettre »,
     * 7 la Station redémarre ([NOTICE_STATION_RESTARTING]), 8 l'ordinateur n'a pas pu passer en HDR
     * ([NOTICE_HDR_REFUSED]), 9 définition ramenée ([NOTICE_RESOLUTION_LOWERED]), 10 le réseau sature en PyroWave
     * ([NOTICE_PYROWAVE_STRAINED]), 11 plafond de l'encodeur ([NOTICE_ENCODER_CEILING]).
     */
    fun onNotice(kind: Int, width: Int, height: Int, back: Boolean, level: Int, mbps: Int, hasMode: Boolean, error: String?)
    fun onGameFullscreen(game: Boolean)
    /** HDR : à chaque démarrage de l'image, l'image est en HDR (HDR10) ou non (docs/hdr.md). */
    fun onHdr(hdr: Boolean) {}
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
    /** Par où passe la session (`transport` : `direct`, `relay`, `relay_tcp` ; `relay`, `rtt_client`, `rtt_station` en ms) : au début, puis quand il change. */
    fun onRoute(json: String) {}
    /**
     * Par le relais, la session finit faute de joindre l'ordinateur, juste avant la fin : `code` 32 accès coupé dans
     * Admin, 31 éteint ou en veille, 30 relais muet ; `text` : la raison de la fin (link-ui). Choisir l'état de
     * l'accueil par `code`, jamais par le texte.
     */
    fun onRelayError(code: Int, text: String) {}
    /** Emprunt : le préavis de 2 min (`{reason, at}`, `reason` `taken_back`, `time_up`, `slot_end`) et son avis de link-ui. */
    fun onLendNotice(json: String, text: String) {}
    /**
     * Emprunt : le prêt finit la session, juste avant la fin : `end` 0 repris par l'hôte, 1 durée passée, 2 fin du
     * créneau, 3 rendu depuis le lanceur (fin voulue, « Vous avez rendu l'ordinateur. »), 4 arrêté (« Le prêt s'est
     * arrêté. ») ; `text` : la raison. Rendu par [Native.sessionGiveBack] : pas d'appel. Sans cet appel, une fin laisse revenir 15 min.
     */
    fun onLendEnd(end: Int, text: String) {}
    /** Hôte : la vue du prêt quand elle change (`null` : rien à dire). */
    fun onLend(json: String?) {}
}

/**
 * Le test du profil Auto ([Native.autotestStart]), depuis le fil du cœur : remonter sur le fil principal.
 * [onProgress] : `{"steps": [{step, title, state: waiting|running|done|skipped, text}]}` ; [onDone] : `{"result": …,
 * "choices": […]}`, `{"cancelled": true}` ou `{"error": "Le test n'a pas pu joindre {nom}."}`.
 */
interface AutotestListener {
    fun onProgress(json: String)
    fun onDone(json: String)
}
