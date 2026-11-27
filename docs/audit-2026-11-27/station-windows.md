# Audit du 27/11/2026 — Station Windows

Zone : `server/admin` (service d'Admin, partagé par `cfg` avec les Stations Mac et Linux), `server/tray` (icône et agent de console), `server/installer` (installateur, `update.ps1`), `server/camera`, `server/micro`, `server/encoder-check`, `server/build-windows.sh`.

Méthode : analyse statique seulement (lecture, grep, git), sans compilation ni essai. Quatre passes en parallèle (mise à jour et installateur, sécurité de l'API, icône/caméra/micro, robustesse/code mort/docs), puis revérification à la main de chaque constat retenu. « Vérifié » : le code en cause a été relu et le défaut est dans le code. « Plausible » : le code est relu, mais le scénario dépend du comportement de Windows ou d'une course à l'exécution.

Ce qui était signalé le 20/10 et qui est réglé n'est pas repris (liste en fin de rapport).

## Synthèse

| ID | Catégorie | Constat | Où | Effort | Confiance |
|---|---|---|---|---|---|
| STWIN-01 | bug critique | Coupure pendant la bascule de mise à jour : Station sans `kyber\`, sans retour arrière | `admin/src/update/apply.rs:396` | M | vérifié |
| STWIN-02 | bug critique | Aides micro/caméra et DLL caméra jamais arrêtées : bascule et installateur échouent, services déjà supprimés | `admin/src/update/apply.rs:756`, `installer/src/install.rs:152` | M | vérifié (blocage plausible) |
| STWIN-03 | bug critique | Service `kyber-admin` sans relance automatique, et aucun filet contre une panique | `admin/src/winsvc.rs:40` | S | vérifié |
| STWIN-04 | sécurité | `device_id` cru sans vérifier le compte : partager, enregistrer, couper ou prêter la session d'un autre | `admin/src/activity.rs:356` | S | vérifié |
| STWIN-05 | sécurité | Compte Windows standard : rattache la Station non rattachée à son compte ou à son relais, puis Admin | `admin/src/accounts.rs:611` | M | vérifié |
| STWIN-06 | sécurité | Codes de prêt et de partage à 6 chiffres, 24 h, limite par adresse exacte, nature du code révélée | `admin/src/lend.rs:784`, `share.rs:456` | M | vérifié |
| STWIN-07 | sécurité | Fin de prêt : Admin (SYSTEM) tue un pid repris par un autre processus | `admin/src/lend_sys.rs:577` | S | vérifié |
| STWIN-08 | sécurité | Règles du prêt contournables par `\steamapps\` et `C:\Windows\Temp` | `admin/src/lend_sys.rs:102` | M | vérifié |
| STWIN-09 | sécurité | Station Linux : service root de mise à jour, lecture de fichiers root par course sur un lien | `admin/src/update/linux.rs:315` | S | vérifié |
| STWIN-10 | sécurité | Comptes Kyber : force brute sans limite depuis le réseau local | `admin/src/api.rs:826` | S | vérifié |
| STWIN-11 | sécurité | Installateur élevé : outils lancés par leur nom nu (recherche dans le dossier de `Setup.exe`) | `installer/src/sys.rs:11` | S | vérifié (exploitation plausible) |
| STWIN-12 | sécurité | Image de la caméra lisible par tout processus (WD, AppContainer) | `camera/windows/kn_shared.h:30` | M | vérifié |
| STWIN-13 | sécurité | Secrets de Kyber copiés dans `.staging` avec les droits hérités | `admin/src/update/apply.rs:531` | S | plausible |
| STWIN-14 | sécurité | Jeton local et jetons Steam de l'hôte lisibles par l'ami pendant le prêt | `admin/src/local_token.rs`, `tray/src/lend/store.rs:145` | M | plausible |
| STWIN-15 | sécurité | Enregistrement : SYSTEM écrit et efface dans un dossier de l'utilisateur (jonction) | `admin/src/recording.rs:77` | M | plausible |
| STWIN-16 | sécurité | Tout appareil appairé a les pouvoirs de l'hôte pour le prêt | `admin/src/lend_api.rs:965` | S | vérifié (choix à confirmer) |
| STWIN-17 | sécurité | Le relais déchiffre Admin et signe les jetons : confiance totale, non dite | `relay/tunnel/src/msg.rs:94` | L | vérifié (modèle) |
| STWIN-18 | sécurité | Durcissements mineurs (7 points) | divers | S | vérifié |
| STWIN-19 | bug | Caméra Windows 11 : vue démappée pendant la lecture, Frame Server peut tomber | `camera/windows/mfsource.cpp:148` | S | vérifié |
| STWIN-20 | bug | Filtre Windows 10 : fuite de 3 Mo + vue + handle par instance ; arrêt avant `Decommit` | `camera/windows/dshow.cpp:186` | S | vérifié / plausible |
| STWIN-21 | bug | Caméra Windows 11 : `RequestSample` bloque, `last_`/`queue_` hors verrou | `camera/windows/mfsource.cpp:152` | S | plausible |
| STWIN-22 | bug | Micro par défaut mal rendu avec plusieurs micros ou une aide tuée | `micro/src/out/windows.rs:219` | M | vérifié |
| STWIN-23 | bug | Relance automatique de Kyber coupée pour de bon si la mise à jour s'interrompt | `admin/src/update/apply.rs:923` | S | vérifié |
| STWIN-24 | bug | Deux installations de mise à jour peuvent partir en même temps | `admin/src/update.rs:390` | S | vérifié |
| STWIN-25 | bug | Téléchargement du paquet : délai global de 120 s, tout en mémoire | `admin/src/update.rs:619` | S | vérifié |
| STWIN-26 | bug | VB-CABLE réinstallé à chaque version malgré un refus ; marque posée même en échec | `admin/src/micro.rs:59` | S | vérifié |
| STWIN-27 | bug | L'installateur ignore `*.prev`, `*.bad`, `.staging` : retour arrière vers une version périmée | `installer/src/install.rs` | S | vérifié |
| STWIN-28 | bug | `update.ps1` accepte une version plus ancienne | `installer/update.ps1:198` | S | vérifié |
| STWIN-29 | bug | Installateur : `kybernow.json` et sauvegarde écrits sans `durable` | `installer/src/install.rs:683` | S | vérifié |
| STWIN-30 | bug | Valeur `Run` dans HKCU du compte élevé (pas réglé depuis le 20/10) | `installer/src/install.rs:23` | S | vérifié |
| STWIN-31 | bug | Réglages illisibles = mot de passe Admin perdu ; JSON abîmés vidés en silence | `admin/src/settings.rs:63` | M | vérifié |
| STWIN-32 | bug | Panique sans compte dans `web_login_start` (`origin[8..]`) | `admin/src/accounts.rs:1042` | S | vérifié |
| STWIN-33 | bug | Agent de console jugé muet quand Admin est lent : prêt arrêté | `tray/src/lend/agent.rs:395` | M | plausible |
| STWIN-34 | bug | Réinitialisation du mot de passe en élévation « par-dessus l'épaule » | `tray/src/password.rs:83` | M | plausible |
| STWIN-35 | bug | Interface d'installation bloquée ouverte après une panique | `installer/src/bin/ui.rs:301` | S | plausible |
| STWIN-36 | bug | Petits bugs (lancement hors prêt, id non encodé, icône ratée retenue, détection caméra Win10→11, handle du micro) | divers | S | vérifié |
| STWIN-37 | optimisation | Moniteur GPU : PDH + instantané des processus chaque seconde, 24 h/24 | `admin/src/gpu.rs:575` | M | vérifié |
| STWIN-38 | optimisation | Appels bloquants et écritures forcées sous verrou dans des gestionnaires async | `admin/src/api.rs:362`, `main.rs:724` | M | vérifié |
| STWIN-39 | optimisation | `/api/status` (toutes les 3 s) relit tout `kyber_config.toml` | `admin/src/api.rs:363` | S | vérifié |
| STWIN-40 | optimisation | Sondages sans objet : PlayStation, `state.json` réécrit, `kybernow.json` relu | `admin/src/consoles.rs:265` | S | vérifié |
| STWIN-41 | optimisation | GitHub sans ETag ; manifeste retéléchargé à chaque vérification | `admin/src/update.rs:647` | S | vérifié |
| STWIN-42 | optimisation | `build-windows.sh` recompile tout ; `sync` fichier par fichier en plus du volume | `server/build-windows.sh:12` | S | vérifié |
| STWIN-43 | factorisation | Listes processus / SDDL / relance en 4 copies déjà divergentes | `apply.rs:755`, `install.rs:30`, `update.ps1:49` | M | vérifié |
| STWIN-44 | factorisation | 6 boucles « `try_wait` + délai », 7 `CREATE_NO_WINDOW`, stderr lu trop tard | `admin/src/encoder.rs:182`… | S | vérifié |
| STWIN-45 | factorisation | Trois façons de relancer les services | `admin/src/service.rs:36`, `station.rs:255`, `tray/src/services.rs:183` | M | vérifié |
| STWIN-46 | factorisation | Doublons dans l'icône et entre icône et Admin (i18n, `wide`, `log`, handles) | `tray/src/*.rs` | S–L | vérifié |
| STWIN-47 | code mort | `/api/lend/catalog` et `Role::from_rights` | `admin/src/api.rs:185`, `share.rs:92` | S | vérifié |
| STWIN-48 | doc | Libellés PyroWave « provisoire » visibles en version stable | `admin/src/schema.rs:190` | S | vérifié |
| STWIN-49 | doc | `docs/pret.md` : 30 s au lieu de 35 s pour `ready` | `docs/pret.md:97` | S | vérifié |
| STWIN-50 | doc | Scripts qui masquent l'échec de clang ; trois correctifs numérotés 0085 ; fichiers qui s'accumulent | `encoder-check/build.sh:32`… | S | vérifié |

Décompte : 3 bugs critiques, 15 sécurité, 18 bugs, 6 optimisations, 4 factorisations, 1 code mort, 3 doc.

---

## Bugs critiques

### STWIN-01 — Coupure pendant la bascule : Station cassée, sans retour arrière
- **Catégorie** : bug critique. **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/admin/src/update/apply.rs:396-411` (`switch`), `:96-98` (`has_previous`), `server/admin/src/update.rs:189-217` (`Updater::new`).
- **Description** : la bascule renomme chaque partie (`kyber`, `admin`, `tray`) en `.prev`, puis met le remplaçant à sa place. Aucun marqueur « bascule en cours » n'est écrit avant :
  ```rust
  if had { remove_dir(&old)?; rename_retry(&current, &old)?; }
  swaps.push(Swap { … });
  rename_retry(incoming, &current)?;
  ```
  `has_previous()` exige que les trois `.prev` existent.
- **Scénario** : le courant coupe (ou le PC gèle, comme le 03/11) entre `kyber → kyber.prev` et `.staging\X\kyber → kyber`. Au redémarrage, `kyber\` n'existe plus : le service Kyber ne démarre pas. Admin (ancienne version) démarre, mais un seul `.prev` existe : aucun bouton « Revenir à la version précédente ». Si la coupure tombe après `admin`, la nouvelle console refuse de réinstaller la même version (`apply.rs:486`, « pas plus récente »). L'installateur en « Conserver mes réglages » exige `kyber\kyber_config.toml`, resté dans `kyber.prev` : il ne reste que « Repartir de zéro ».
- **Correctif** : écrire de façon durable `update\bascule.json` (plan, sens, version) avant la première rename, l'effacer après le contrôle de santé. Au démarrage, Admin (et l'installateur) qui le trouvent terminent le retour arrière : toute partie absente est reprise de son `.prev` ou du `.staging`.

### STWIN-02 — Fichiers verrouillés : aides micro/caméra et DLL caméra
- **Catégorie** : bug critique. **Effort** : M. **Confiance** : listes vérifiées, blocage du renommage plausible.
- **Où** : `server/admin/src/update/apply.rs:756` (`KYBER_PROCESSES` : `kyavserver`, `kycontroller`, `kynputserver`, `kyclient`, `kyservice`) ; `server/installer/src/install.rs:471` et `server/installer/update.ps1:52` (mêmes listes, sans `kybernow-micro.exe` ni `kybernow-camera.exe`) ; `server/installer/src/uninstall.rs:35` (seule la désinstallation arrête la caméra) ; `server/camera/windows/install.cpp:111,157` (DLL inscrites en COM depuis `kyber\`).
- **Description** : `kybernow-micro.exe` et `kybernow-camera.exe` tournent sous `kyber\` pendant une session ; `KyberNowCameraSource.dll` (Frame Server) et `KyberNowCameraFilter.dll` (toute app qui filme, Windows 10) sont chargées par d'autres processus. Rien ne les arrête avant de renommer `kyber\`.
- **Scénario** : mise à jour lancée pendant que Teams utilise « KyberNow Camera » : `rename_retry(kyber → kyber.prev)` échoue dix fois et tout est annulé, à chaque essai. Ou le renommage passe (un renommage de dossier est parfois permis) mais `remove_dir(kyber.prev)` échoue à chaque mise à jour suivante. Dans l'installateur, c'est pire : `prepare()` arrête **et supprime** les services (`install.rs:152`, `:466-470`) avant l'extraction (`:161`) ; une DLL verrouillée fait échouer l'extraction et laisse une Station sans services, à moitié remplacée. `docs/webcam.md:103` liste d'ailleurs ce cas comme essai à faire.
- **Correctif** : ajouter les deux aides aux quatre listes (voir STWIN-43) ; arrêter `FrameServer` pendant la bascule ; pour les DLL encore chargées, `MoveFileEx` en `.old` + `MOVEFILE_DELAY_UNTIL_REBOOT`. Dans l'installateur, extraire (ou au moins vérifier qu'on peut écrire) avant de supprimer les services.

### STWIN-03 — `kyber-admin` jamais relancé, panique sans filet
- **Catégorie** : bug critique. **Effort** : S (relance) + M (verrous). **Confiance** : vérifié.
- **Où** : `server/admin/src/winsvc.rs:40-63` (création du service, sans actions d'échec) ; `server/installer/src/install.rs:545` (`sc failure` posé pour `kyber` seulement) ; aucun `panic::set_hook` ni `catch_unwind` dans `server/admin/src` ; `Cargo.toml` sans `panic = "abort"` ; environ 590 `lock().unwrap()` hors tests.
- **Description** : si Admin s'arrête sur une erreur, Windows ne le relance pas (sous Linux, `Restart=on-failure` existe). Et une panique dans un gestionnaire pendant qu'un `Mutex` est tenu empoisonne le verrou : le processus reste vivant, mais tout appel suivant à ce verrou panique (et les fils `supervise` de `display/mod.rs:2334-2364` ou `audio/mod.rs:269` meurent sans bruit).
- **Scénario** : une panique dans une route d'affichage (verrou `Manager.session`) → tous les `heartbeat` paniquent, plus de chien de garde de l'écran virtuel, Admin « vivant » mais inutilisable jusqu'au redémarrage du PC. Rien n'est écrit dans `kyber-admin.log` (la trace part sur stderr).
- **Correctif** : `sc failure kyber-admin reset= 86400 actions= restart/5000/restart/5000/restart/30000` dans l'installateur et `winsvc::install` (et reposé au démarrage d'Admin). Un `panic::set_hook` qui écrit dans le journal puis `process::exit(1)` (le gestionnaire de services relance). Un utilitaire `lock()` qui ignore l'empoisonnement (`unwrap_or_else(PoisonError::into_inner)`).

---

## Sécurité

### STWIN-04 — `device_id` cru sans vérifier le compte
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/activity.rs:356-360` :
  ```rust
  fn is_live(live: &Live, user: &str, device_id: Option<&str>) -> bool {
      match device_id { Some(id) => live.device_id == id, None => live.user == user }
  }
  ```
  Même schéma : `share_api.rs:691-699` (`host_session`), `recording.rs:525-531`, `lend_api.rs:1004-1007` (`from_session`), `api.rs:1700-1704` (heartbeat), `api.rs:1794` (end).
- **Scénario** : un autre appareil appairé (compte Kyber, ou compte KyberNow avec le droit Session, y compris par le relais) qui connaît le `device_id` de l'appareil en session peut ouvrir un partage de cette session, lancer son enregistrement, la couper, ou créer un code de prêt « qui vaut accord ». Le repli `compte:{user}` (`activity.rs:129`) se devine.
- **Correctif** : exiger `live.user == user` en plus du `device_id` dans `is_live` et les gardes équivalentes.

### STWIN-05 — Un compte Windows standard prend la main par le rattachement
- **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/admin/src/accounts.rs:611-634` (`local_relay`, `local_attach`), `:666` (`local_enroll`) : garde `local_guard` seulement (boucle locale + jeton local). Le jeton est copié dans `%LOCALAPPDATA%\KyberNow\local-token` du compte de la console, quel qu'il soit (`local_token.rs`). `set_relay_locally` (`:575-579`) ne refuse que si la Station est déjà rattachée.
- **Scénario** : PC familial, Station laissée « Plus tard ». Un enfant sur un compte standard appelle `POST /api/local/account/attach` (son compte KyberNow) ou `PUT /api/local/account/relay` (son propre relais, qui signera ce qu'il veut), puis « Se connecter avec KyberNow » donne une session Admin complète (le compte rattaché est accepté).
- **Correctif** : réserver ces routes à l'installateur : jeton à usage unique déposé par le processus élevé, ou vérification que l'appelant est administrateur ; sinon, exiger la session d'Admin.

### STWIN-06 — Codes de prêt et de partage trop faibles
- **Effort** : M. **Confiance** : vérifié (débit réel plausible).
- **Où** : `server/admin/src/lend.rs:784-785` et `share.rs:456-458` (`{:06}`), `lend.rs:32` (`CODE_TTL` = 24 h), `lend.rs:488-501` (`count_try` : 10 essais/min par `peer.to_canonical()`), `share_api.rs:1006` (`"lend": true` révélé sur un mauvais code de partage).
- **Scénario** : par le relais, l'identifiant de la Station figure dans les liens de prêt. Un attaquant tourne dans son /64 IPv6 et devine le code (10⁶ possibilités, 24 h). `join` et `borrow` ont des compteurs séparés et `join` dit si le code est un code de prêt : le débit double. Si l'ordinateur est libre ou si le code vient de la session, l'ami entre sans accord. Sur le réseau local, le partage entre sans accord de l'hôte (`share.rs:867-895`).
- **Correctif** : codes de 8 à 10 caractères ou durée plus courte ; compteur par /64 et compteur global d'échecs (comme `Relay::begin_attempt_at`) partagés entre `join` et `borrow` ; ne plus révéler `lend` avant succès.

### STWIN-07 — Fin de prêt : un pid repris est tué en SYSTEM
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/lend_sys.rs:577` (`started2.lock().unwrap().insert(pid)`), `:594-600` (`close_started` : `image_path(pid)` puis `terminate(pid)`), `:541` (`seen` : ensemble de pids) ; côté agent `server/tray/src/lend/system.rs:464-475` (cache par pid jamais vidé).
- **Scénario** : une app ouverte pendant le prêt se ferme ; Windows redonne son pid à une app de l'hôte ou à un service tiers sous Program Files. À la fin, Admin le tue avec les droits SYSTEM. Inversement, un nouveau processus de l'ami qui reprend un pid de `seen` n'est jamais jugé. `HostLine` (`:160-170`) se protège déjà avec l'instant de création ; `started` et `seen` non.
- **Correctif** : clé `(pid, born(pid))` partout, et revérifier la session avant `terminate`.

### STWIN-08 — Règles du prêt contournables
- **Effort** : M. **Confiance** : règle vérifiée, exploitation plausible.
- **Où** : `server/admin/src/lend_sys.rs:99-110` : `path.contains("\\steamapps\\")` donne le statut d'app (donc `cmd`/`powershell` permis comme enfants, `HELPERS`) ; tout ce qui est sous `self.system` (`c:\windows\`) est permis.
- **Scénario** : l'ami télécharge un exécutable dans `Téléchargements\steamapps\x.exe`, ou le dépose dans `C:\Windows\Temp` (accessible en écriture à un compte standard) : il est permis, ses enfants aussi.
- **Correctif** : `steamapps` seulement sous les bibliothèques de `libraryfolders.vdf` ; pour Windows, `System32`, `SysWOW64`, `WinSxS` et une liste d'exclusions (`Temp`, `Tasks`, `tracing`…).

### STWIN-09 — Station Linux : lecture de fichiers root par le service de mise à jour
- **Effort** : S. **Confiance** : vérifié (course plausible). Code d'Admin partagé, chemin Linux seulement.
- **Où** : `server/admin/src/update/linux.rs:315-329` (`copy_limited` : `symlink_metadata` puis `File::open`, qui suit les liens), `:345-347` ; `server/linux/packaging/kybernow-station-update.socket:11` (`SocketMode=0666`), `kybernow-station-update@.service:16` (`StateDirectoryMode=0755`) ; `:608-650` (`Rollback` accepté de tout compte).
- **Scénario** : un compte local quelconque envoie `Apply` pour une version et échange en boucle `~/.local/share/…/update/<v>/manifest.json` contre un lien vers `/etc/shadow`. Le contenu (≤ 256 Kio) est recopié dans `/var/lib/kybernow-station/update/<v>/`, lisible par tous, et reste si `verify_kept` échoue. Tout compte peut aussi déclencher un retour arrière.
- **Correctif** : `O_NOFOLLOW` puis `fstat` du descripteur ; fichiers en `0600`, `StateDirectoryMode=0700` ; effacer manifeste et signature en cas d'échec ; n'accepter que l'uid propriétaire de l'installation.

### STWIN-10 — Comptes Kyber : force brute sans limite sur le réseau local
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/api.rs:826-838` (`stream_user` → `kyber.verify_login`, SHA-256 imposé par Kyber) ; routes `/api/stream/*` publiques dans `require_session`.
- **Scénario** : depuis le Wi-Fi, `GET /api/stream/info` avec `Authorization: Basic` en rafale, sans compteur (le relais, lui, limite).
- **Correctif** : passer `stream_user` par `auth.note_failure` / `locked_for` (par /64).

### STWIN-11 — Installateur élevé : outils par leur nom nu
- **Effort** : S. **Confiance** : code vérifié, exploitation plausible.
- **Où** : `server/installer/src/sys.rs:11` (`Command::new(program)` avec `sc`, `reg`, `netsh`, `powershell.exe`, `taskkill`, `certutil`, `pnputil`, `powercfg`) ; `server/installer/src/bin/setup.rs:59` (`cmd`), `:168`.
- **Scénario** : sous Windows, Rust cherche d'abord dans le dossier de l'exécutable courant. En `/quiet` (et pour la vérification de WebView2), c'est `Setup.exe` dans Téléchargements : un `reg.exe` déposé à côté s'exécute en administrateur. L'applicateur de mise à jour, lui, passe déjà par `system32(tool)` (`apply.rs:783`).
- **Correctif** : chemins complets sous `%SystemRoot%\System32` dans `sys::run`.

### STWIN-12 — Image de la caméra lisible par tous
- **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/camera/windows/kn_shared.h:30` : `D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GR;;;WD)(A;;GR;;;AC)…`.
- **Scénario** : tout processus de tout compte, y compris une app AppContainer sans le droit `webcam` ou une app refusée dans Paramètres › Caméra, ouvre `Global\KyberNowCameraFrame` et lit l'image de la caméra de l'appareil, sans témoin.
- **Correctif** : Windows 11 : lecture réservée au compte de Frame Server (à confirmer : `NT SERVICE\FrameServer` ou LocalService) et à SY. Windows 10 : WD reste nécessaire pour le filtre, à documenter. Retirer AC dans les deux cas.

### STWIN-13 — Secrets de Kyber dans `.staging` avec les droits hérités
- **Effort** : S. **Confiance** : plausible (héritage de l'ACL de Program Files par `fs::copy`).
- **Où** : `server/admin/src/update/apply.rs:531-538` (`copy_tree(&current, &target, …)`), `:552-554` (dossier gardé si `undo_failed`).
- **Scénario** : `kyber_config.toml`, `kybertest_key.pem` et le certificat copiés dans `.staging\<version>` sont lisibles par Users jusqu'au prochain `secure_secrets`, et sans limite si le retour arrière rate ou si le courant coupe (`.staging` n'est jamais purgé pour une autre version).
- **Correctif** : `acl::secure_file` sur ces fichiers juste après la copie ; ACL protégée sur `.staging` ; purge de `.staging\*` au démarrage d'Admin.

### STWIN-14 — Ce que l'ami peut lire pendant le prêt
- **Effort** : M. **Confiance** : plausible.
- **Où** : `server/admin/src/local_token.rs` (jeton dans `%LOCALAPPDATA%` du compte de la console, qui est aussi celui du prêt) ; `server/tray/src/lend/store.rs:145-155` (`config.vdf`, `local.vdf` déplacés en `*.kybernow` à côté de l'original).
- **Scénario** : le prêt se fait dans la session de l'hôte. Un navigateur permis lit `local-token` puis appelle `127.0.0.1` pour accepter l'appairage de son propre appareil (`/api/local/pair/{id}/accept`) ou rattacher la Station (STWIN-05) ; ou lit `config.vdf.kybernow` (jetons Steam de l'hôte).
- **Correctif** : renouveler le jeton au début du prêt sans le remettre dans le profil, refuser les routes locales sensibles pendant un prêt ; confier les fichiers Steam mis de côté à Admin (dossier SYSTEM).

### STWIN-15 — Enregistrement : SYSTEM écrit dans un dossier de l'utilisateur
- **Effort** : M. **Confiance** : plausible.
- **Où** : `server/admin/src/recording.rs:77-82` (`Videos` du profil de la session, sinon `C:\Users\Public\Videos`), `create_dir_all` dans `start` (`:287`), `cleanup` (`:438-456`).
- **Scénario** : un compte local remplace `Videos\KyberNow` par une jonction vers un dossier protégé : Kyber (SYSTEM) y crée ses MP4 et `cleanup` y efface des `KyberNow *.mp4`.
- **Correctif** : refuser un dossier qui est (ou contient) un point d'analyse ; ouvrir sans suivre les liens ; ou faire créer le fichier par l'agent dans la session.

### STWIN-16 — Tout appareil appairé est « l'hôte » du prêt
- **Effort** : S. **Confiance** : vérifié, choix de conception à confirmer.
- **Où** : `server/admin/src/lend_api.rs:965-967` (`host()` = n'importe quel `stream_user`), `:1074-1092`.
- **Scénario** : l'Apple TV appairée par code, ou un compte KyberNow avec le droit Session, crée un code de prêt de 24 h, accepte des demandes ou reprend l'ordinateur, y compris par le relais.
- **Correctif** : réserver aux appareils « propriétaire » ou au droit Administrer.

### STWIN-17 — Le relais est de confiance totale
- **Effort** : L. **Confiance** : vérifié (modèle de confiance).
- **Où** : `server/relay/tunnel/src/msg.rs:94-96` (`AdminHttp` : « déchiffré par le relais : HTTP/1.1 en clair ») ; `server/admin/src/relay.rs` (`guard`) ; `accounts.rs` (`web_login_return` accepte un jeton `via: "relay"`).
- **Scénario** : qui compromet le relais public récupère les cookies Admin des Stations qui ont « Admin à distance » et peut fabriquer des jetons Administrer/Session pour toutes les Stations rattachées.
- **Correctif** : à court terme, le dire dans l'interface d'« Admin à distance » et dans `docs/relais.md` ; à terme, TLS de bout en bout jusqu'à la Station ou seconde preuve locale.

### STWIN-18 — Durcissements mineurs
- **Effort** : S chacun. **Confiance** : vérifié.
- `admin/src/api.rs:352-356` : `change_password` sans `check_lockout` ni `note_failure`.
- `admin/src/auth.rs:144` : code d'initialisation comparé par `eq_ignore_ascii_case` (temps variable) ; utiliser `lend::same` après mise en majuscules.
- `admin/src/auth.rs:98,106` : compteur d'échecs par adresse exacte, entrées jamais oubliées tant que `count > 0` (table qui grossit avec des IPv6 tournantes).
- `admin/src/pairing.rs:30-33` : 6 demandes d'appairage en attente au total ; trois adresses du réseau bloquent tout nouvel appairage.
- `admin/src/logs.rs:123` : `:` non refusé ; `dir.join("C:x.log")` sort du dossier (route réservée à Admin).
- `admin/src/kyber_config.rs:328-331` : si la pose des droits échoue, `save` avertit et laisse `kyber_config.toml` lisible.
- `installer/src/install.rs:725-729` : chemins insérés dans un script PowerShell entre `'…'` sans doubler les apostrophes (`apply.rs:851` le fait).

---

## Bugs

### STWIN-19 — Caméra Windows 11 : vue démappée pendant la lecture
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/camera/windows/mfsource.cpp:148-169` (`RequestSample` lâche `lock_` puis `kn_shared_read(&mapping_, &view_, …)`), `:237-253` (`Shutdown` fait `UnmapViewOfFile(view_)` et `CloseHandle(mapping_)` sous verrou).
- **Scénario** : l'app qui filme se ferme pendant une demande d'image : le `memcpy` lit une vue démappée et Frame Server tombe (toutes les caméras disparaissent pour toutes les apps). Après `Shutdown`, `kn_shared_read` peut aussi rouvrir la mémoire et la laisser fuir.
- **Correctif** : lire sous `lock_` (ou garder une référence de vie) et revérifier `queue_` après la prise du verrou.

### STWIN-20 — Filtre DirectShow (Windows 10) : fuites et arrêt fragile
- **Effort** : S. **Confiance** : fuites vérifiées, arrêt plausible.
- **Où** : `server/camera/windows/dshow.cpp:412-414` (`nv12_ = malloc(KN_SHARED_FRAME_MAX)`, ouverture de `mapping_`/`view_`) ; `~Pin()` (`:186-191`) ne libère rien de cela (les seuls `CloseHandle` du fichier sont `:696` et `:714`) ; `:580-582` (`stop_thread()` avant `pin_.Decommit()`), `:708-715` (attente de 5 s puis `CloseHandle(thread_)` même si le fil vit).
- **Scénario** : Zoom ou Teams recréent le graphe à chaque changement de caméra : +3 Mo et une vue à chaque fois. Si le fil est bloqué dans `GetBuffer`, `Stop` gèle 5 s puis l'objet est détruit sous le fil (accès après libération).
- **Correctif** : libérer `nv12_`, la vue et le handle dans `~Pin` ; vérifier le `malloc` ; `Decommit` avant de signaler l'arrêt et ne pas détruire si l'attente échoue.

### STWIN-21 — Caméra Windows 11 : `RequestSample` bloquant, champs hors verrou
- **Effort** : S. **Confiance** : plausible.
- **Où** : `mfsource.cpp:152-157` (`Sleep` jusqu'à 33 ms dans l'appel ; `last_` lu et écrit hors `lock_`) ; `:328-329`, `:344-346`, `:124` (`queue_` lu sans verrou alors que `Shutdown` le libère).
- **Correctif** : rendre l'image sans attendre (le client donne le rythme) ; copier `queue_` sous verrou avec `AddRef`, comme dans `GetEvent`.

### STWIN-22 — Micro par défaut mal rendu
- **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/micro/src/out/windows.rs:210-228` (`make_default` : une 2ᵉ instance voit CABLE déjà par défaut et garde `previous = None`), `:230-245` (`stop` : la 1ʳᵉ qui s'arrête rend l'ancien micro) ; `Player` n'a pas de `Drop` ; le contrôleur tue l'aide (`kill_on_drop`).
- **Scénario** : deux appareils envoient leur micro : le premier qui coupe rend le micro d'origine alors que l'autre parle encore. Si l'aide est tuée (fin de session brutale), le micro par défaut de l'hôte reste « CABLE Output » pour de bon.
- **Correctif** : compteur partagé (mutex nommé + fichier) pour que la dernière instance rende le micro ; « micro d'avant » écrit sur disque et rendu par `--check` ou au démarrage d'Admin.

### STWIN-23 — Relance automatique de Kyber coupée pour de bon
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/update/apply.rs:923` (`run("sc.exe", &NO_RECOVERY)` dans `stop_server`), `:962` (remise seulement dans `start_services`).
- **Scénario** : gel, coupure ou applicateur tué entre les deux : Kyber ne se relance plus jamais après un plantage, jusqu'à la prochaine mise à jour ou réinstallation.
- **Correctif** : reposer `KYBER_RECOVERY` au démarrage d'Admin (sans effet si déjà en place).

### STWIN-24 — Deux installations de mise à jour en même temps
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/update.rs:390-397` (vérification de `installing` sous un verrou) puis `:411-416` (`installing = Some(…)` sous un second verrou).
- **Scénario** : Admin, l'icône et une app Link lancent « Installer » presque en même temps : les deux passent ; le second efface le téléchargement du premier (`:446`) ou échoue sur la copie de `kyber-admin-maj.exe` en cours d'exécution et remet `installing = None` pendant qu'un applicateur tourne. Aucun verrou entre processus non plus avec `update.ps1` ou l'installateur.
- **Correctif** : vérifier et poser `installing` dans la même prise du verrou ; mutex nommé `Global\KyberNowUpdate` dans l'applicateur, `update.ps1` et l'installateur.

### STWIN-25 — Téléchargement limité à 120 s au total
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/update.rs:619-625` (`timeout_global(Some(Duration::from_secs(120)))`, agent partagé), `:458` (`durable::write(&zip, get(package_url, …)?)`).
- **Scénario** : le paquet fait ~60 Mo ; sous 4 Mbit/s il faut plus de 120 s : la mise à jour échoue toujours sur une connexion lente, et le zip entier passe en mémoire.
- **Correctif** : écrire en flux dans un fichier temporaire, délai par lecture seulement pour le paquet.

### STWIN-26 — VB-CABLE réinstallé malgré un refus
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/micro.rs:59-82` : marque `micro-vbcable-{CARGO_PKG_VERSION}`, installation `-i -h` si l'aide ne le voit pas, marque écrite quel que soit le résultat ; `:95-115` : stderr en `piped` lu seulement après la fin, `kill` sans `wait`. L'installateur respecte `req.microphone` (`install.rs:306`) mais ne l'enregistre pas.
- **Scénario** : micro décoché à l'installation ou VB-CABLE désinstallé exprès : à la mise à jour suivante, Admin installe le pilote en silence, en SYSTEM. À l'inverse, une installation ratée (délai dépassé) pose la marque : pas de nouvel essai avant la version suivante. Les marques s'accumulent dans ProgramData.
- **Correctif** : enregistrer le choix `microphone` dans `kybernow.json` et n'installer que s'il est vrai ; marque unique, écrite seulement sur succès ; lire stderr dans un fil (ou `Stdio::null()`).

### STWIN-27 — L'installateur ignore `.prev`, `.bad`, `.staging`
- **Effort** : S. **Confiance** : vérifié (aucune occurrence dans `installer/src/install.rs`).
- **Scénario** : après une réinstallation (même « Repartir de zéro »), Admin propose « Revenir à la version précédente » vers un `kyber.prev` périmé, qui remet l'ancien certificat et l'ancienne clé TLS : les appareils voient un certificat changé (TOFU).
- **Correctif** : l'installateur supprime `*.prev`, `*.bad` et `.staging` (ou les range dans ProgramData).

### STWIN-28 — `update.ps1` accepte une version plus ancienne
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/installer/update.ps1:198-205` : seul `min_installer_version` est comparé à `Get-InstalledVersion`, jamais `$version`.
- **Scénario** : un utilisateur bêta lance `irm …/releases/latest/download/update.ps1 | iex` et revient en silence à la dernière stable, plus ancienne. L'applicateur d'Admin, lui, refuse (`apply.rs:486`).
- **Correctif** : refuser si `[version]$version -le [version]$installed`, sauf `-Force`.

### STWIN-29 — Installateur : écritures non durables
- **Effort** : S. **Confiance** : code vérifié, effet d'un gel plausible.
- **Où** : `server/installer/src/install.rs:683` (`std::fs::write(… "kybernow.json" …)`), `:571` (`std::fs::copy` de la sauvegarde), extraction sans `sync` (`payload.rs:77`).
- **Scénario** : le gel du 03/11, mais pendant une réinstallation : `kybernow.json` rempli de zéros = icône sans port, désinstallation qui ne remet plus l'alimentation ni le réseau. Les leçons de la 0.9.4 (`durable::write`) ne sont appliquées qu'à la mise à jour.
- **Correctif** : réutiliser `durable` (temporaire + `sync` + renommage) et `FlushFileBuffers` du volume en fin d'extraction.

### STWIN-30 — Démarrage de l'icône dans la ruche du compte élevé
- **Effort** : S. **Confiance** : vérifié (signalé le 20/10, toujours là).
- **Où** : `server/installer/src/install.rs:23` (`RUN_KEY = HKCU\…\Run`), `:733`.
- **Scénario** : UAC « par-dessus l'épaule » (identifiants d'un autre administrateur) : l'icône démarre à l'ouverture de session de l'administrateur, pas de l'utilisateur.
- **Correctif** : `HKLM\Software\Microsoft\Windows\CurrentVersion\Run`.

### STWIN-31 — Réglages et données abîmés : remise à zéro silencieuse
- **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/admin/src/settings.rs:63-78` (fichier illisible mis de côté puis `Self::fresh()`) ; `read(...).ok().and_then(from_slice).unwrap_or_default()` dans `accounts.rs:199`, `invites.rs:168`, `lend.rs:149` et `:352`, `lend_last.rs:45`, `activity.rs:185-192`.
- **Scénario** : `kyber-admin.toml` illisible → hash du mot de passe Admin, relais, compte et enregistrement perdus, console repassée en « code d'initialisation ». `accounts.json` illisible (ou une entrée au format plus récent après un retour arrière) → liste vidée, y compris `blocked_accounts` (un compte bloqué ne l'est plus), et la sauvegarde suivante écrase l'original sans trace.
- **Correctif** : garder la dernière version lue sans erreur (`.bak`) et la recharger avant les valeurs par défaut ; un seul `store::load_json` qui journalise, met de côté et ne remplace qu'ensuite.

### STWIN-32 — Panique sans compte dans `web_login_start`
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/accounts.rs:1042` : `origin[8..].contains([…])`.
- **Scénario** : `{"origin":"http://é"}` → l'octet 8 tombe au milieu de « é » : panique « not a char boundary » (la connexion meurt, la trace part sur stderr). Et `http://@hôte` passe le filtre, car `[8..]` saute le 8ᵉ caractère d'une adresse en `http://`.
- **Correctif** : `strip_prefix("https://").or_else(|| strip_prefix("http://"))` puis contrôle du reste.

### STWIN-33 — Agent de console jugé muet quand Admin est lent
- **Effort** : M. **Confiance** : plausible.
- **Où** : `server/admin/src/lend.rs:1190` (muet au-delà de 10 s), `lend_api.rs:1521-1530` (une relance puis arrêt du prêt) ; `server/tray/src/lend/agent.rs` : un seul fil pour `poll` (délai 5 s, `:395`), `post` (10 s, `:410`), `played`, icônes (5 s chacune, `:570-575`) ; nouvel agent qui attend le verrou 15 s (`:172`) puis sort.
- **Scénario** : Admin occupé (écriture durable sous verrou, STWIN-38) : quelques icônes lentes suffisent à dépasser 10 s sans `beat` ; la relance échoue sur le verrou de l'ancien agent, le prêt s'arrête en pleine partie.
- **Correctif** : un fil dédié au `beat`, délais plus courts pour `played` et les icônes.

### STWIN-34 — Réinitialisation du mot de passe en élévation « par-dessus l'épaule »
- **Effort** : M. **Confiance** : plausible.
- **Où** : `server/tray/src/password.rs:83-87` (code écrit dans `handoff_path()`, `%LOCALAPPDATA%` du processus élevé), `:49` et `:34`.
- **Scénario** : compte standard + identifiants d'un administrateur : le code atterrit dans le profil de l'administrateur, l'icône ne le trouve pas (`codeNotHandedOver`). Un échec dans la partie élevée sort en code 1, affiché « erreur système 1 : Fonction incorrecte ».
- **Correctif** : passer le chemin du profil (ou le SID) en argument ; ACL sur le fichier ; code de sortie propre.

### STWIN-35 — Fenêtre d'installation bloquée après une panique
- **Effort** : S. **Confiance** : plausible.
- **Où** : `server/installer/src/bin/ui.rs:301-320` : `*installing.lock().unwrap() = false` n'est atteint que si `run_install` ne panique pas ; `:228` refuse la fermeture tant que le drapeau est vrai.
- **Correctif** : garde `Drop` qui remet le drapeau, `lock()` tolérant à l'empoisonnement.

### STWIN-36 — Petits bugs
- **Effort** : S chacun. **Confiance** : vérifié.
- `admin/src/lend_api.rs:1400-1435` : `console_launch` ne vérifie pas qu'un prêt est en cours (le détenteur du jeton local lance les apps permises ou Big Picture hors prêt).
- `tray/src/console.rs:318` : `format!("/api/local/pair/{id}/{action}")` sans encodage (les routes du partage passent par `path_part`).
- `tray/src/lend/agent.rs:560-567` : une icône ratée une fois (Admin occupé) est retenue `None` pour toute la session.
- `camera/windows/install.cpp:187-195` : après une mise à niveau Windows 10 → 11, la caméra est dite « pas installée » (seule la clé de la source Windows 11 est regardée) ; `LoadLibraryW(L"mfsensorgroup.dll")` (`:93`) jamais libérée.
- `micro/src/out/windows.rs:133-137` : les `?` après `CreateEventW` sortent sans `CloseHandle` ; `:150` `jitter.lock().unwrap()` bloquant alors que `out::pull` (`out/mod.rs:53`) existe (même chose sous Linux, `out/linux.rs:106`).

---

## Optimisations

### STWIN-37 — Moniteur GPU en continu
- **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/admin/src/gpu.rs:575-603` : PDH « GPU Engine » + `process_ids` (instantané de tous les processus) chaque seconde, 24 h/24 ; PowerShell `Get-CimInstance` toutes les 5 min (`:607`).
- **Correctif** : 5 à 10 s hors session, 1 s seulement quand l'affichage, l'audio ou un partage sont actifs.

### STWIN-38 — Bloquant et écritures forcées sur le fil async
- **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/admin/src/api.rs:362` (`status`) et `:425` (`local_state`) appellent `s.service.state()` en direct (`launchctl print` sur Mac, `systemctl --user show` sous Linux) et `system::computer_name()` (`scutil` sur Mac ; aussi `share_api.rs:90`) ; `main.rs:724` (`update_settings`, 23 appels) écrit en `durable` sous le `Mutex<Settings>` ; `activity.rs:208-216` réécrit les 300 événements de `events.json`, forcés sur disque, à chaque événement.
- **Scénario** : sous Windows, `FlushFileBuffers` + `MOVEFILE_WRITE_THROUGH` à chaque événement retardent `heartbeat` et `summary` ; `/api/status` est appelée toutes les 3 s par onglet ouvert (`server/web/src/App.svelte:144`).
- **Correctif** : `blocking(...)` pour les appels système ; cache de `computer_name` ; `events.json` en écriture simple ou groupée ; réglages copiés sous verrou puis écrits hors verrou.

### STWIN-39 — `/api/status` relit toute la configuration de Kyber
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/api.rs:363-369` : `s.kyber.values()` analyse tout `kyber_config.toml` pour le seul port, alors que `KyberConfig::port()` existe (`kyber_config.rs:137`). Sur Mac, `gpu::status` relance `sysctl` à chaque appel (`gpu.rs:262`).
- **Correctif** : `port()` ; `OnceLock` pour `sysctl`.

### STWIN-40 — Sondages sans objet
- **Effort** : S. **Confiance** : vérifié.
- `admin/src/consoles.rs:265` : diffusion UDP PlayStation toutes les 30 s même sans console jamais vue (ne lancer qu'après une console vue ou demandée).
- `admin/src/foreground.rs:131` et `display/watch.rs:133` (agents) : `state.json` réécrit toutes les 0,4 s et 1 s même sans changement.
- `tray/src/console.rs:30-54` : `kybernow.json` relu à chaque `port()`/`fingerprint()`, plusieurs fois par seconde (cache par date de modification).
- `admin/src/launch.rs:517` : un `spawn_blocking` toutes les 2 s même sans app lancée.

### STWIN-41 — GitHub interrogé sans requête conditionnelle
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/update.rs:647-656` (`releases?per_page=30`, notes complètes), `:587-597` (`verify_release` retélécharge manifeste et signature à chaque vérification dès qu'une version plus récente existe).
- **Scénario** : fréquence raisonnable (6 h ; apps au plus toutes les 10 min), mais tout compte dans la limite anonyme de 60/h par IP, partagée par toutes les Stations d'un même accès.
- **Correctif** : `If-None-Match` avec l'ETag (un 304 ne compte pas) ; garder le manifeste vérifié pour une même version.

### STWIN-42 — Construction et extraction plus lentes que nécessaire
- **Effort** : S. **Confiance** : vérifié.
- `server/build-windows.sh:12` (`npm ci` à chaque fois) et `:16` (`touch src/assets.rs`, qui force la recompilation d'Admin) : seulement si `package-lock.json` ou `web/dist` ont changé.
- `admin/src/update/apply.rs:218` (`sync_all` par fichier à l'extraction) et `copy_tree` → `durable::copy` → `sync_file`, en plus de `flush_volume` (`:463`) : ne forcer chaque fichier que si `flush_volume` échoue.

---

## Factorisation

### STWIN-43 — Listes du service en quatre copies, déjà divergentes
- **Effort** : M. **Confiance** : vérifié.
- **Où** : processus à arrêter : `admin/src/update/apply.rs:756`, `installer/src/install.rs:471`, `installer/src/uninstall.rs:31-35`, `installer/update.ps1:52` (seule la désinstallation connaît `kybernow-camera.exe`) ; SDDL des services : `apply.rs:755`, `install.rs:30`, `update.ps1:49` ; actions de relance : `apply.rs:761`, `install.rs:545` ; `acl.rs` en deux copies (Admin et installateur) dont les commentaires diffèrent ; contrôle de santé écrit deux fois (`apply.rs:968-1004` et `admin_answers`, `:309-337`).
- **Correctif** : un module partagé (`#[path]` ou petit crate `server/common`) ; un test qui compare la liste de `update.ps1` à celle du Rust. C'est ce qui a produit STWIN-02.

### STWIN-44 — Lancement de processus avec délai, six fois
- **Effort** : S. **Confiance** : vérifié.
- **Où** : boucles `try_wait` + délai dans `admin/src/encoder.rs:197`, `micro.rs:99`, `pyrowave.rs:288` et `:354`, `gamepads.rs:61`, `linux_screen.rs:131` (`micro.rs:104` tue sans `wait`, `encoder.rs:182-210` lit stderr trop tard, comme STWIN-26) ; `CREATE_NO_WINDOW` défini dans `update.rs:746`, `pyrowave.rs:203`, `wake.rs:268`, `update/apply.rs:763`, `gpu.rs:570`, en littéral dans `encoder.rs:191` et `micro.rs:88`.
- **Correctif** : un `proc::run_with_timeout(cmd, délai) -> Option<Output>` (stderr lu dans un fil, `CREATE_NO_WINDOW`, `wait` après `kill`).

### STWIN-45 — Trois façons de relancer les services
- **Effort** : M. **Confiance** : vérifié.
- **Où** : `admin/src/service.rs:36` (async, sans arrêt forcé), `admin/src/station.rs:255-320` (avec `TerminateProcess` d'un service bloqué), `tray/src/services.rs:183` et `tray/src/linux.rs:116`.
- **Scénario** : Admin bloqué en cours d'arrêt : « Relancer » dans l'icône échoue après 30 s là où l'assistant d'Admin aurait fini de force.
- **Correctif** : l'icône passe par `kyber-admin restart-station`, comme `POST /api/station/restart`.

### STWIN-46 — Doublons dans l'icône et avec Admin
- **Effort** : S (petits) à L (i18n). **Confiance** : vérifié.
- `fn wide()` quatre fois : `tray/src/app.rs:49`, `services.rs:35`, `startup.rs:24`, `lend/system.rs:44`.
- `log` (rotation à 1 Mo), `unix_now`, `dir` recopiés dans `tray/src/lend/system.rs:49-69`, `system_mac.rs:49-68`, `system_linux.rs:58-77` : à remonter dans `lend/mod.rs`.
- Échappement HTML `esc` en double (`tray/src/panel.html:173`, `lend/launcher.html:227`).
- Deux moteurs i18n parallèles (`tray/src/i18n.rs`, `admin/src/i18n.rs`) : petite crate commune, plus tard.
- Handles Win32 fermés à la main après chaque `OpenProcess` / `CreateToolhelp32Snapshot`, avec trois copies de `snapshot()` (`gpu.rs`, `lend_sys.rs`, `launch.rs`) : rien ne fuit aujourd'hui, mais le prochain `?` fuira ; un `OwnedHandle` et un seul `process_snapshot()`.
- Constat positif : le service n'est pas copié ailleurs. C'est un seul crate `kyber-admin` découpé par `cfg` ; `server/macos/crates` ne contient qu'un correctif `quinn-udp`, `server/linux` l'agent d'affichage, l'empaquetage et des scripts.

---

## Code mort

### STWIN-47 — `/api/lend/catalog` et `Role::from_rights`
- **Effort** : S. **Confiance** : vérifié par grep dans tout le dépôt hors `third_party`.
- `server/admin/src/api.rs:185` : `.route("/api/lend/catalog", get(apps))`, alias de `/api/apps`. `git grep -n "lend/catalog"` ne trouve que cette ligne et le test de garde `relay.rs:1518`. Le web (`server/web/src/lib/api.ts:872`) appelle `/api/apps`. Retirer la route et l'entrée du test.
- `server/admin/src/share.rs:92` : `Role::from_rights` n'est appelé que dans `share.rs:1310`, sous `#[cfg(test)]`. Le passer en `#[cfg(test)]` ou le supprimer.
- Pour le reste : sur les 695 `pub fn` d'Admin, aucun n'est sans appel (`git grep -w -c <nom> -- server ':!**/dist/**'`) ; idem dans `server/tray`, `server/installer`, `server/micro`, `server/camera`. `server/admin/dev-data/` et `server/installer/dist/` sont ignorés par git (`dist/update.ps1` est une copie produite, identique à `update.ps1`).

---

## Doc et dette

### STWIN-48 — Libellés PyroWave « provisoire » en version stable
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/schema.rs:190` (`// TODO textes pyrowave`), `:200-203` (`label: "PyroWave (provisoire)"`, `help: "Texte provisoire : …"`, `advice: "Texte provisoire : laissez activé."`) ; même chose dans `i18n/schema.en.json:78`, `schema.linux.json:27`, `schema.macos.json:34`. C'est le seul TODO/FIXME/XXX/HACK de `server/admin`, `server/tray` et `server/installer`.
- **Correctif** : écrire les vrais textes (FR et EN) d'après `docs/pyrowave.md` et `docs/ton-du-site.md`.

### STWIN-49 — `docs/pret.md` : délai de `ready`
- **Effort** : S. **Confiance** : vérifié.
- **Où** : `docs/pret.md:97` et `docs/pret.en.md:97` disent « 30 s au plus » ; `docs/pret.md:109` et le code (`server/admin/src/lend_api.rs:48`, `READY_WAIT = 35 s`) disent 35 s.
- **Correctif** : 35 s à la ligne 97, en français et en anglais. Le reste des docs relues concorde (routes citées dans `server/README(.en).md`, `docs/{pret,enregistrement,micro,webcam}(.en).md` toutes présentes ; valeurs de période et de rétention conformes).

### STWIN-50 — Scripts et fichiers
- **Effort** : S. **Confiance** : vérifié.
- `server/encoder-check/build.sh:32-34` et `server/camera/build.sh:53-57` : `clang … | grep -Ev … || true` puis `[ -x "$OUTPUT" ] || exit 1` : si la compilation échoue et qu'une sortie précédente existe, la publication part avec l'ancien binaire (même défaut que celui de `build.sh` corrigé après le 20/10). `rm -f "$OUTPUT"` avant clang, ou `${PIPESTATUS[0]}`.
- `server/kyber-patches/` : trois correctifs numérotés `0085` (`0085-kyclient-flux-montant`, `0085-kycontroller-flux-montant`, `0085-kycontroller-micro-droits`, ce dernier modifiant un fichier créé par le précédent) : l'ordre ne tient qu'à l'ordre alphabétique. Renuméroter.
- Fichiers qui s'accumulent : `kyber_config.toml.reinstallation-{stamp}` (`installer/src/install.rs:570`) et la copie de la désinstallation (`uninstall.rs:67`) échappent à la rotation de `kyber_repair::rotate` (préfixe `kyber_config-`) ; marques `micro-vbcable-<version>` jamais purgées ; `.staging\kyber-admin-maj.exe` (~32 Mo) laissé après chaque mise à jour.

---

## Mes 10 priorités

1. **STWIN-01** : marqueur de bascule et reprise au démarrage. C'est le scénario du 03/11 (gel pendant la mise à jour) qui reste sans issue.
2. **STWIN-02 + STWIN-43** : une seule liste partagée des processus (avec micro et caméra), arrêt de Frame Server, et installateur qui ne supprime plus les services avant d'avoir pu écrire.
3. **STWIN-03** : relance automatique de `kyber-admin`, crochet de panique vers le journal, verrous tolérants. Petit effort, gros gain de robustesse.
4. **STWIN-04** : `live.user == user` dans `is_live` et les gardes voisines. Une ligne, faille réelle entre appareils.
5. **STWIN-05 + STWIN-14** : routes locales de rattachement réservées à l'installateur, jeton local retiré pendant un prêt.
6. **STWIN-06 + STWIN-10** : codes plus longs et compteurs par /64 et globaux (prêt, partage, comptes Kyber).
7. **STWIN-07 + STWIN-08** : pid + instant de création, et règles du prêt sans `\steamapps\` ni `C:\Windows\` en vrac.
8. **STWIN-19 + STWIN-20 + STWIN-12** : la caméra peut faire tomber Frame Server, fuit sous Windows 10 et est lisible par tous.
9. **STWIN-23 + STWIN-24 + STWIN-25 + STWIN-28** : quatre corrections S qui rendent la mise à jour fiable sur connexion lente et en cas de double clic.
10. **STWIN-31** : réglages et comptes jamais remis à zéro en silence (copie de la dernière bonne version).

Hors liste mais à faire au passage : STWIN-09 (Station Linux, effort S), STWIN-48 (textes PyroWave visibles par tous).

## Ce qui est sain

- **Réglé depuis le 20/10** : ACL de ProgramData posée à l'installation et à chaque démarrage, sans suivre les liens (la course vérification/extraction est fermée) ; clé JWT sortie de `kyber_config.toml`, fichier réservé à SYSTEM et Administrateurs ; `update.ps1` avec `try/catch/finally` et retour arrière ; étape « alimentation » non bloquante ; Alt+F4 ignoré pendant l'installation ; « Conserver mes réglages » à la réinstallation ; SHA256SUMS : limite assumée et documentée pour le chemin à la main, Admin passe par le manifeste signé ; `Settings::save` atomique.
- **Mise à jour** : signature minisign avec clé compilée, `allow_legacy = false`, commentaire de confiance, version identique au tag, canal, taille et empreinte contrôlés ; paquet revérifié par l'applicateur ; version égale ou plus ancienne refusée ; extraction du zip qui refuse chemins sortants, noms réservés, flux NTFS, liens et doublons ; `version.txt` contrôlé avant tout arrêt ; outils lancés par chemins complets System32, arguments un par un ; `durable::write`/`rename` (`MOVEFILE_WRITE_THROUGH`, `FlushFileBuffers`) pour toutes les données importantes d'Admin.
- **API** : argon2 avec sel (Admin, amis, invitations) et calcul factice si le compte n'existe pas ; comparaisons à temps constant (jeton local, secrets d'invité, cookies, codes, empreintes) ; jetons de 256 bits ; CSRF couvert (`SameSite=Strict`, `HttpOnly`, en-tête `x-kyber-admin` sur toute écriture, aucune couche CORS) ; garde du relais en liste blanche (`Class::Never` par défaut) avec essais limités par /64 et plafond global ; aucun `Command` ne passe d'entrée utilisateur à un shell ; noms de fichiers contrôlés (enregistrements, icônes, `within_roots`) ; corps de requête bornés.
- **Prêt et icône** : routes `/api/local/lend/console*` en boucle locale + jeton ; certificat d'Admin épinglé par l'icône ; crochet clavier qui ignore les touches injectées ; lanceur qui échappe ce qu'il insère et limite la navigation ; état du prêt écrit atomiquement, `put_aside` qui n'écrase jamais une copie mise de côté ; `HostLine` qui tient compte de l'instant de création ; garde « une console à la fois » correcte.
- **Caméra et micro** : écriture de la mémoire partagée réservée à SY/BA avec marqueur d'intégrité basse, `Global\` impossible à créer par un compte standard (pas d'injection d'images), en-tête validé (magic, version, ≤ 1920×1080, âge, compteur pair/impair) ; décompte de références COM correct là où il a été lu ; paquets micro bornés à 64 Kio, tampon de gigue borné à 60 ms et testé.
- **Code** : aucune fonction publique morte hors STWIN-47 ; un seul TODO ; tests isolés sous `#[cfg(test)]` ; pas de copie du service entre Windows, Mac et Linux.
