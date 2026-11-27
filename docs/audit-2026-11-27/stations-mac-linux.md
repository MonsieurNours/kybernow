# Audit du 27/11/2026 — Stations Mac et Linux

État lu : `main` à `1fb289c6` (KyberNow 0.11.7). Lecture de code seulement : rien n'a été compilé, lancé ni essayé sur un Mac de Station, une VM ou une machine Linux. Périmètre :

- **Station Mac** (`server/macos`) : agent des écrans `kybernow-display` (CGVirtualDisplay), aide des manettes (CoreHID), pilote son et « Micro KyberNow » (`audio-driver`), extension caméra, icône de la barre des menus, PyroWave Metal, scripts de construction, `crates/`, 21 correctifs Kyber (`server/macos/kyber-patches`).
- **Station Linux** (`server/linux`) : agent des écrans (GNOME, KDE, Sway/Hyprland), paquets deb/rpm, services systemd, service root des mises à jour, règles udev, installation dans le dossier de l'utilisateur (`immutable/`), scripts, 11 correctifs Kyber (`server/linux/kyber-patches`).
- **Admin** (`server/admin/src`), parties Mac et Linux seulement (la Station Windows est auditée à part) : `mac.rs`, `display/mac.rs`, `display/linux.rs`, `update/mac.rs`, `update/linux.rs`, `linux_*.rs`, `micro.rs`, blocs `cfg(macos|linux)`.

L'audit précédent (`docs/archives/audit-2026-10-20.md`) précède les deux Stations : aucun recoupement.

Identifiants : `STMAC-xx` (Mac seul), `STLIN-xx` (Linux seul), `STCOM-xx` (Admin ou correctifs communs aux deux Stations). Confiance : **vérifié** = code relu et défaut certain ; **plausible** = code relu, effet à confirmer sur la machine.

## Synthèse

| Id | Catégorie | Où | Résumé | Effort | Confiance |
|---|---|---|---|---|---|
| STLIN-01 | sécurité (critique) | `server/admin/src/update/linux.rs:316` | Le service root suit un lien posé après sa vérification : un compte local fait copier `/etc/shadow` en 0644 | S | vérifié (code), course plausible |
| STLIN-02 | sécurité | `server/linux/packaging/kybernow-station-update.socket:11` | Socket 0666 : tout compte local déclenche un retour arrière ou une bêta | M | vérifié |
| STLIN-03 | sécurité / bug | `update/linux.rs:615` | Pas de délai de lecture ni de verrou : 4 connexions muettes bloquent les mises à jour, deux demandes en même temps se marchent dessus | S | vérifié |
| STMAC-01 | bug critique | `server/macos/display-agent/main.swift:613` | `kybernow-display lock` ne verrouille pas : la session reste ouverte après un Admin redémarré en plein prêt | S | vérifié |
| STCOM-01 | bug | `server/admin/src/main.rs:762` | Admin ignore SIGTERM sur Mac et Linux : aucun nettoyage à l'arrêt | S | vérifié |
| STMAC-02 | bug | `display-agent/main.swift` (pas de signaux), `display/mac.rs:110` | L'agent des écrans meurt avec Admin (launchd) ou par SIGPIPE sans rallumer les écrans du Mac | S | vérifié (code) |
| STLIN-04 | bug | `server/linux/display-agent/src/main.rs:782` | Même chose sous Linux : sous Sway/Hyprland les écrans restent éteints après un arrêt | S | plausible |
| STMAC-03 | sécurité | `camera-extension/main.swift:251` | N'importe quelle app peut pousser des images dans « KyberNow Camera » | S | vérifié |
| STLIN-05 | bug / doc | `packaging/60-kybernow-uinput.rules` | Prêt du PC : la règle udev pour `/dev/input/event*` annoncée n'existe pas, le clavier de l'hôte n'est jamais bloqué | M | vérifié |
| STLIN-06 | sécurité | kyber-patches Linux `0013…:119` | Sortie Wayland introuvable : la capture prend un autre écran (risque pour invités et prêt) | S | vérifié |
| STCOM-02 | sécurité | `update/acl.rs:65-95` | `secure_dir`/`secure_file` ne font rien hors Windows : clé TLS et réglages en 0644 | S | vérifié |
| STMAC-04 | sécurité | `scripts/build-station-app.sh:237` | Droit `audio-input` donné aux dix exécutables | S | vérifié |
| STMAC-05 | sécurité | `menubar/Notify.swift:29` | « Accepter » un prêt depuis une notification sans déverrouiller | S | plausible |
| STMAC-06 | bug | `admin/src/mac.rs:118`, `update/mac.rs:76`, `station.rs:221` | Domaine launchd pris sur `/dev/console` : agit sur un autre compte (changement rapide d'utilisateur, fenêtre de connexion) | S | vérifié (code) |
| STMAC-07 | bug | `update/mac.rs:81-87` | `launchctl` impossible à lancer = succès | S | vérifié |
| STMAC-08 | bug | `admin/src/display/mac.rs:99-138, 231` | Appels à l'agent sans délai, verrou tenu : Admin peut se figer | M | plausible |
| STMAC-09 | bug | `display-agent/audio.swift:118` | `route` peut retenir KyberNow comme sortie d'avant : Mac muet après la session | S | vérifié |
| STMAC-10 | bug | `display-agent/main.swift:170` | `CGBeginDisplayConfiguration` jamais annulée si un champ manque | S | vérifié |
| STMAC-11 | bug | `display-agent/main.swift:110` | `create` sur un emplacement occupé sans la garde de `remove` | S | plausible |
| STMAC-12 | bug | `display-agent/main.swift:553` | `display-off.json` écrit sans `.atomic` | S | vérifié |
| STMAC-13 | bug | `menubar/main.swift:55` | Notifications reposées avec un son à chaque raté d'Admin | S | vérifié |
| STMAC-14 | bug | `menubar/main.swift:39` | File de relevés de l'icône sans limite quand Admin ne répond plus | S | plausible |
| STMAC-15 | bug | `audio-driver/KyberNowAudio.c:135, 795` | Un seul compteur d'E/S pour les deux appareils du micro | S | vérifié |
| STMAC-16 | bug | kyber-patches Mac `0011…:555` | Aide des manettes morte : manettes muettes jusqu'au prochain branchement | S | vérifié |
| STMAC-17 | bug | kyber-patches Mac `0006…:207-223` | Nombre de voies du son figé à la création de l'entrée | S | plausible |
| STMAC-18 | bug | `scripts/build-station-app.sh:33`, `run-station.sh:96` | `--git-common-dir` relatif résolu depuis le dossier courant | S | vérifié |
| STLIN-07 | bug | `update/linux.rs:391-392, 637` | Paquet copié avant le contrôle du canal ; dossiers laissés par une demande refusée | S | vérifié |
| STLIN-08 | bug | `update/linux.rs:566` | `apt-get` sans `DPkg::Lock::Timeout` | S | vérifié (code) |
| STLIN-09 | bug | `display-agent/src/main.rs:409-414` | Fil de la disposition : course sur `pending`, et nouvel essai toutes les 250 ms sans fin | S | vérifié |
| STLIN-10 | bug | `display-agent/src/gnome.rs:71-101` | Session Mutter et fil perdus quand une capture échoue (×7 essais) | S | vérifié |
| STLIN-11 | bug | `display-agent/src/kde.rs:97-139` | Événements de flux KWin périmés | S | plausible |
| STLIN-12 | bug | kyber-patches Linux `0002…:75-97, 168-189` | Pointeur absolu : échec d'uinput gardé à vie ; appui et relâchement sur deux appareils | S | vérifié |
| STLIN-13 | bug | kyber-patches Linux `0005…:197` | Un échec d'uhid fait passer toutes les DualSense en Xbox jusqu'au redémarrage | S | vérifié |
| STLIN-14 | bug | `packaging/kybernow-station-setup:47` | NVENC choisi dès qu'une carte NVIDIA est vue (nouveau/NVK : pas de NVENC) | S | plausible |
| STLIN-15 | bug | `scripts/package.sh:104` | Services activés pour tous les comptes, sans `ConditionUser` | S | plausible |
| STLIN-16 | bug | `scripts/package.sh:117-139` | Pas de postrm/%postun : `/var/lib/kybernow-station` reste après purge | S | vérifié |
| STLIN-17 | bug | `immutable/kybernow-station-home:212-217` | uhid détecté mais jamais chargé ni déclenché | S | vérifié (code) |
| STLIN-18 | bug | `admin/src/audio/mod.rs:495` | Reprise : `unload-module <ancien numéro>` peut retirer un autre module | S | plausible |
| STLIN-19 | bug | `kybernow-station-setup:19` vs `kybernow-admin.service:11` | `XDG_DATA_HOME` suivi par l'installation, pas par Admin | S | vérifié |
| STLIN-20 | sécurité (basse) | `admin/src/linux_apps.rs:425` | Dossier d'icônes au nom prévisible dans `/tmp` | S | vérifié (code) |
| STLIN-21 | bug | `admin/src/linux_apps.rs:551` | Fermer une app tue un pid peut-être réutilisé | S | plausible |
| STLIN-22 | bug | `scripts/release-station.sh:110` | Sortie muette sous `pipefail` quand le .rpm manque | S | vérifié |
| STLIN-23 | bug (publication) | `scripts/release.sh:589`, `server/linux/scripts/build-kyber.sh` | Cache de la Station Linux aveugle aux correctifs des apps ; pas de verrou ni d'empreinte des correctifs | S | vérifié |
| STCOM-03 | bug | `admin/src/micro.rs:104` | Processus zombie après un délai dépassé | S | vérifié |
| STCOM-04 | bug | `admin/src/update.rs:621` | Paquet téléchargé en mémoire, 120 s au total | M | plausible |
| STCOM-05 | robustesse | `server/kyber-patches/apply.sh:75` et les deux `build-kyber.sh` | `patch` accepte un décalage (fuzz 2) ; état amont jamais vérifié | S | vérifié |
| STMAC-19 | optimisation | kyber-patches Mac `0006`, `0013` | Allocations et journal dans le rappel temps réel de Core Audio | M | plausible |
| STMAC-20 | optimisation | `camera-extension/main.swift:189` | Image grise allouée 30 fois par seconde | S | vérifié |
| STMAC-21 | optimisation | `pyrowave/kn_pyrowave_metal.mm:286` | `waitUntilCompleted` à chaque image | M | plausible |
| STLIN-24 | optimisation / fuite | kyber-patches Linux `0007…:705` | Un fil par collage, sans délai | M | plausible |
| STCOM-06 | factorisation | `display/mac.rs` / `display/linux.rs` | ~250 lignes communes | M | vérifié |
| STCOM-07 | factorisation | `micro.rs`, `linux_screen.rs`, `pyrowave.rs`, `gamepads.rs` | « Lancer avec délai » copié quatre fois | S | vérifié |
| STCOM-08 | factorisation | `mac.rs`, `update/mac.rs`, `station.rs`, `lend_sys.rs`… | Domaine launchd, étiquettes, `home()` en plusieurs copies | S | vérifié |
| STCOM-09 | factorisation | Mac `0011` / Linux `0005` | DualSense en double (descripteur identique, code divergent) | M | vérifié |
| STCOM-10 | factorisation | Mac `0017`/`0018` / Linux `0011`/`0012` | PyroWave : mêmes lignes du 0025 réécrites deux fois | M | vérifié |
| STCOM-11 | factorisation | les deux `build-kyber.sh`, `release.sh:336` | Scripts à ~60 % identiques ; liste d'empreinte en double | S | vérifié |
| STCOM-12 | doc / i18n | `admin/src/display/mod.rs:2427` | « pas encore d'écran virtuel sur Mac » affiché aussi sur Linux | S | vérifié |
| STCOM-13 | code mort / parité | `update.rs:764`, `api.rs:1411` | `launch_applicator` hors Windows mort ; adresse préférée réservée au Mac | S | vérifié |
| STLIN-25 | code mort | `server/linux/scripts/build-kyber.sh:61-73` | Boucle du stylet qui ne fait jamais rien | S | vérifié |
| STMAC-22 | code mort | caméra, agent, PyroWave | `sequence` jamais lu, tables non vidées, P010 pleine plage annoncée réduite | S | vérifié |
| STMAC-23 | sécurité (basse) | `update/mac.rs:232-237` | Contrôle de l'équipe contourné si l'app installée a une signature illisible | S | vérifié |
| STMAC-24 | bug mineur | `admin/src/foreground.rs:221` | Liste « pas un jeu » par noms anglais | M | plausible |
| STMAC-25 | doc | `docs/station-macos.md`, correctifs | Écarts avec le code | S | vérifié |
| STLIN-26 | doc | `INSTALLATION.md`, `PROJECT.md`, `docs/station-linux.md`, caméra | Écarts avec le code | S | vérifié |
| STLIN-27 | sécurité (choix à écrire) | `60-kybernow-uinput.rules:4,6` | uinput/uhid en `uaccess` : tout processus de la session peut taper | S | vérifié |
| STCOM-14 | doc / nommage | correctifs Kyber | Numéros Mac/Linux qui recouvrent les communs ; deux 0085 | S | vérifié |

---

## Station Linux

### STLIN-01 — Le service root copie un fichier lié après la vérification
- **Catégorie** : sécurité (critique). **Effort** : S. **Confiance** : code vérifié, course plausible (facile à gagner en boucle).
- **Où** : `server/admin/src/update/linux.rs:316-329` (`copy_limited`), appelé par `stage` (l. 345-346) ; source = dossier du **demandeur** (`prepare_apply`, l. 390 : `user.data_dir().join("update")`).
- **Description** : `fs::symlink_metadata(from)` vérifie que le fichier n'est pas un lien, puis `fs::File::open(from)` l'ouvre **en suivant les liens**. Entre les deux, le demandeur remplace le fichier. Le contenu est écrit par `durable::write` (`File::create`, donc 0644 sous l'umask 022 de systemd) dans `/var/lib/kybernow-station/update/<v>/` (`StateDirectoryMode=0755`). Quand la vérification de signature échoue, `stage` ne retire que le paquet, pas le manifeste ni la signature.
- **Scénario** : un compte local sans droits envoie `{"action":"apply","version":"99.0.0"}` en boucle sur le socket (0666, STLIN-02) en basculant `~/.local/share/kybernow/station/admin/update/99.0.0/manifest.json` entre un vrai fichier et un lien vers `/etc/shadow` (moins de 256 Ko). Il finit par lire le contenu dans `/var/lib/kybernow-station/update/99.0.0/manifest.json`. Les dossiers parents peuvent aussi être des liens (seul le dernier élément est contrôlé), et une FIFO bloque le service indéfiniment.
- **Correctif** : ouvrir avec `O_NOFOLLOW | O_NONBLOCK` (`OpenOptionsExt::custom_flags`) puis `fstat` sur le descripteur (fichier ordinaire, taille), ou lire en prenant l'uid du demandeur (`setfsuid`) ; créer la réserve en 0700 et ses fichiers en 0600 ; supprimer manifeste et signature quand la vérification échoue.

### STLIN-02 — N'importe quel compte local pilote le service root
- **Catégorie** : sécurité. **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/linux/packaging/kybernow-station-update.socket:11` (`SocketMode=0666`) ; `update/linux.rs:612-663` (`serve`) : `peer_uid` sert à trouver le compte, jamais à le restreindre.
- **Scénario** : un second compte (ou un compte de service qui a un dossier personnel) envoie `{"action":"rollback"}` : root réinstalle la version gardée, plus ancienne, pour tout le système ; ce sont les services **du demandeur** qui sont arrêtés et relancés, et le contrôle de santé lit ses réglages, donc il échoue et déclenche une réinstallation en sens inverse. Variante : ce compte met `update_channel = "beta"` chez lui (`user_channel(&user.data_dir())`, l. 639) et installe une bêta sur la Station du propriétaire.
- **Correctif** : groupe dédié (`SocketGroup=kybernow-station`, `SocketMode=0660`, propriétaire ajouté par `kybernow-station-setup` ou le postinst), ou dans `serve()` n'accepter que l'uid enregistré comme propriétaire (fichier root écrit à la première demande réussie, ou à l'installation).

### STLIN-03 — Service root sans délai ni sérialisation
- **Catégorie** : sécurité (déni de service) / bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `update/linux.rs:615` (`read_line` sans délai ; le délai de 300 s n'existe que côté Admin, l. 257) ; socket `MaxConnections=4` ; aucun `flock` dans `update/linux.rs` ni `update/apply.rs`.
- **Scénario** : quatre connexions ouvertes sans rien envoyer bloquent toutes les mises à jour (« service des mises à jour injoignable »). Deux demandes simultanées (double clic, deux comptes) font chacune `remove_file` puis copie dans le même `update/<v>/`, lancent deux `apt-get`, et un `prune()` peut effacer le dossier de l'autre.
- **Correctif** : `stream.set_read_timeout(Some(10 s))` avant la lecture ; `flock` non bloquant sur `/var/lib/kybernow-station/update/.lock`, réponse « mise à jour déjà en cours ».

### STLIN-04 — Agent des écrans tué sans nettoyage
- **Catégorie** : bug. **Effort** : S. **Confiance** : code vérifié, effet sous wlroots plausible.
- **Où** : `server/linux/display-agent/src/main.rs:782-803` (nettoyage seulement à la fin de l'entrée standard) ; aucun `KillMode`/`ExecStop` dans `kybernow-admin.service`.
- **Scénario** : `systemctl --user stop kybernow-admin` (mise à jour, `update/linux.rs:581-585` ; `stop_services` de `kybernow-station-home` ; prerm ; déconnexion) envoie SIGTERM à tout le groupe : Admin (STCOM-01) et l'agent meurent ensemble. Sous Sway/Hyprland, les sorties sans tête restent et les écrans physiques restent éteints jusqu'au prochain démarrage de l'agent (`wlroots.rs:19-22`) — jamais, après une désinstallation. (GNOME et KDE retirent leurs écrans virtuels avec la connexion D-Bus.)
- **Correctif** : traiter SIGTERM dans l'agent (`signal-hook` → `shutdown()`), ou `KillMode=mixed` pour qu'Admin seul reçoive SIGTERM et ferme l'entrée de l'agent.

### STLIN-05 — Prêt du PC : pas de règle pour `/dev/input/event*`
- **Catégorie** : bug / doc. **Effort** : M. **Confiance** : vérifié.
- **Où** : `docs/station-linux.md:170` et `server/tray/src/lend/system_linux.rs:8-9` (« il faut la règle udev du paquet ») ; `server/linux/packaging/60-kybernow-uinput.rules` ne couvre que `uinput`, `uhid` et les hidraw DualSense ; seule autre règle installée : la caméra (`package.sh:82`).
- **Scénario** : pendant un prêt, `EVIOCGRAB` échoue sur les claviers et souris physiques ; la doc dit « sans elle, le prêt marche sans ce blocage », donc l'hôte (ou un passant) garde la main sur la session prêtée, sans que l'interface le dise.
- **Correctif** : une petite aide root (activée par socket, comme les mises à jour) qui fait le grab et rend le descripteur ; à défaut, une règle `uaccess` sur `ID_INPUT_KEYBOARD`/`ID_INPUT_MOUSE` (tout processus de la session pourrait alors lire le clavier : à peser), et dans tous les cas l'afficher dans Admin et corriger la doc.

### STLIN-06 — Sortie Wayland introuvable : la capture prend un autre écran
- **Catégorie** : sécurité. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/kyber-patches/0013-kyavservice-ecrans-wayland.patch:115-125` : `warn!("Wayland output {name} not found, using another one")` puis repli sur l'identifiant, puis sur la première source.
- **Scénario** : la sortie `HEADLESS-n` d'un invité Share ou d'un prêt disparaît (agent relancé, compositeur qui la retire) : l'invité reçoit l'écran réel de l'hôte.
- **Correctif** : quand `wayland_output` est donné, ne jamais retomber sur une autre source : erreur, la session attend que l'écran revienne.

### STLIN-07 — Paquet copié avant le contrôle du canal, restes des demandes refusées
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `update/linux.rs:391-392` (`stage` copie le paquet entier avant `check_for`) ; l. 637 (`Journal::open(store.dir(version)…)` crée le dossier avant toute vérification) ; `prune()` seulement après un succès.
- **Scénario** : toute demande d'une version bien formée crée `/var/lib/kybernow-station/update/<v>/update.log` (root, disque), et une demande refusée y laisse manifeste et paquet (dizaines de Mo).
- **Correctif** : `check_for` dans `stage`, juste après `verify_kept` ; journal ouvert après validation ; dossier retiré en cas de refus.

### STLIN-08 — `apt-get` n'attend pas le verrou de dpkg
- **Catégorie** : bug. **Effort** : S. **Confiance** : code vérifié, fréquence plausible.
- **Où** : `update/linux.rs:566-570`.
- **Scénario** : Ubuntu avec unattended-upgrades en cours : « Could not get lock /var/lib/dpkg/lock-frontend », mise à jour perdue.
- **Correctif** : `-o DPkg::Lock::Timeout=300` ; côté rpm, `dnf` attend déjà son verrou.

### STLIN-09 — Fil de la disposition : course et nouvel essai sans fin
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/display-agent/src/main.rs:405-416`.
- **Description** : le fil clone `pending`, applique, puis remet `pending = None` sans vérifier qu'aucune disposition plus récente n'a été posée entre-temps (par `isolate`, l. 518-523). En cas d'erreur, il réessaie toutes les 250 ms et écrit au journal à chaque fois. `apply_layout` garde le verrou partagé pendant `desk.apply()` (D-Bus ou `kscreen-doctor`, jusqu'à ~1 s).
- **Scénario** : deux écrans ajoutés coup sur coup → la seconde disposition est effacée ; KDE sans `kscreen-doctor` (seulement « suggéré » par le .deb, `package.sh:154`) → quatre lignes de journal par seconde toute la session.
- **Correctif** : numéro de génération (n'effacer que la même), verrou relâché avant `desk.apply`, essais espacés puis abandon après N échecs.

### STLIN-10 — GNOME : session Mutter et fil perdus sur erreur
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/display-agent/src/gnome.rs:71-101`.
- **Description** : après `CreateSession`, toute erreur (`RecordVirtual`, `Start`, délai de 5 s) sort sans `Stop`, et le fil qui attend `added.next()` reste bloqué. `ensure_sources` réessaie jusqu'à 7 fois par écran (`main.rs:654-662`).
- **Correctif** : garde qui appelle `Session.Stop` sur tout chemin d'erreur ; fil d'attente arrêté avec la session.

### STLIN-11 — KDE : événements de flux périmés
- **Catégorie** : bug. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/linux/display-agent/src/kde.rs:97-106, 139` : événements rangés par `protocol_id`, `Closed` d'un flux établi jamais lu, identifiants réutilisés par KWin.
- **Scénario** : un nouveau flux tombe sur un vieux `Closed` et échoue à tort ; une capture fermée par KWin n'est jamais remarquée.
- **Correctif** : n'enregistrer que les flux en attente ; traiter `Closed` des captures actives.

### STLIN-12 — Pointeur absolu (correctif 0002)
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié (code), effet chez le compositeur plausible.
- **Où** : `server/linux/kyber-patches/0002-kynput-pointeur-wayland.patch:75-97` et `:160-189`.
- **Description** : (a) `static DEVICE: OnceLock<Option<…>>` garde un échec de `plug_device` pour toute la vie de kynputserver : si `/dev/uinput` n'était pas encore ouvert au démarrage, le pointeur absolu ne marche plus jamais (un seul `warn!`) ; (b) les clics passent par le pointeur absolu quand il « tient » le pointeur, et `release()` est appelé à chaque mouvement relatif : un appui sur un appareil peut être relâché sur l'autre.
- **Scénario** : glissé commencé en absolu, pointeur capturé par un jeu, relâchement : « KyberNow pointeur » garde `BTN_LEFT` enfoncé.
- **Correctif** : `Mutex<Option<…>>` avec nouvel essai après délai ; à chaque changement d'appareil, relâcher sur l'ancien les boutons qu'il a appuyés.

### STLIN-13 — Un échec d'uhid est définitif (correctif 0005)
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/kyber-patches/0005-kynput-manettes-linux.patch:197` (`self.uhid_failed = true`, jamais remis à faux) ; `:666` `custom_flags(0o4000 /* O_NONBLOCK */)` en dur.
- **Scénario** : règle udev pas encore appliquée au premier branchement : toutes les DualSense deviennent des manettes Xbox jusqu'au redémarrage de kynputserver.
- **Correctif** : nouvel essai après un délai ; `libc::O_NONBLOCK`.

### STLIN-14 — NVENC choisi sans pilote propriétaire
- **Catégorie** : bug. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/linux/packaging/kybernow-station-setup:44-49` : seul critère, le fabricant `0x10de`.
- **Scénario** : Fedora sans RPM Fusion (nouveau ou NVK) : pas de NVENC, le flux échoue ; aucun repli dans Admin.
- **Correctif** : exiger `/proc/driver/nvidia/version` ou `libnvidia-encode.so.1` (`ldconfig -p`), sinon VA-API puis x264.

### STLIN-15 — Services activés pour tous les comptes
- **Catégorie** : bug. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/linux/scripts/package.sh:104` (`systemctl --global enable …`), sans `ConditionUser` dans les trois services.
- **Scénario** : chaque compte qui ouvre une session crée sa propre Station ; deux sessions ouvertes se disputent les ports 8080/9443 ; un gestionnaire de connexion qui atteint `graphical-session.target` lancerait Admin.
- **Correctif** : `ConditionUser=!@system` dans les trois services, et documenter qu'une seule session à la fois porte la Station.

### STLIN-16 — Désinstallation incomplète
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/scripts/package.sh:117-139` : seulement prerm / `%preun`.
- **Scénario** : après `apt purge` ou `dnf remove`, `/var/lib/kybernow-station` garde les paquets de deux versions (~48 Mo .deb, ~79 Mo .rpm) et les journaux ; pas de `udevadm control --reload` ni `systemctl daemon-reload` après retrait.
- **Correctif** : postrm (`purge` → `rm -rf /var/lib/kybernow-station`) et `%postun` (`$1 = 0`), avec les deux rechargements.

### STLIN-17 — Installation dans le dossier de l'utilisateur : uhid oublié
- **Catégorie** : bug. **Effort** : S. **Confiance** : code vérifié, effet plausible.
- **Où** : `server/linux/immutable/kybernow-station-home:209-217` : le besoin est détecté pour `/dev/uhid` (l. 212), mais la commande ne charge et ne déclenche que `uinput`.
- **Scénario** : Bazzite ou SteamOS sans `steam-devices` : DualSense en Xbox jusqu'au redémarrage.
- **Correctif** : `uinput` et `uhid` dans `modules-load.d`, `modprobe uhid`, `udevadm trigger --name-match=uhid`.

### STLIN-18 — Reprise du son : module retiré par numéro
- **Catégorie** : bug. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/admin/src/audio/mod.rs:495-499` (`pactl unload-module <numéro gardé>`), appelé à la reprise (`recover`, l. 170-178).
- **Scénario** : après un redémarrage de l'ordinateur ou de PipeWire, le numéro gardé désigne un autre module (Bluetooth, annulation d'écho), qui est retiré.
- **Correctif** : retirer par nom (`unload_ours()` existe déjà).

### STLIN-19 — `XDG_DATA_HOME` suivi d'un côté seulement
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/packaging/kybernow-station-setup:19` (`${XDG_DATA_HOME:-$HOME/.local/share}`) contre `kybernow-admin.service:11` (`%h/.local/share/…`) et `update/linux.rs:56`.
- **Scénario** : avec `XDG_DATA_HOME` défini, la première initialisation (mot de passe, clé) se fait dans un dossier qu'Admin ne lit pas.
- **Correctif** : chemin fixe partout (le plus simple, déjà celui du service et du service root).

### STLIN-20 — Dossier d'icônes prévisible dans `/tmp`
- **Catégorie** : sécurité (basse). **Effort** : S. **Confiance** : code vérifié, exploitation plausible.
- **Où** : `server/admin/src/linux_apps.rs:425` : `temp_dir().join("kybernow-apps-linux-{pid}")`, puis `remove_dir_all` et `create_dir_all`.
- **Scénario** : un autre compte crée ce dossier (ou un lien) entre les deux appels et fournit les icônes qu'Admin recopie (`launch.rs:248`).
- **Correctif** : `$XDG_RUNTIME_DIR` ou `tempfile::tempdir()` (sur Mac, `$TMPDIR` est déjà propre au compte).

### STLIN-21 — Fermer une app : pid peut-être réutilisé
- **Catégorie** : bug. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/admin/src/linux_apps.rs:551-553` (`kill -TERM` dès que `/proc/<pid>` existe).
- **Correctif** : comparer `/proc/<pid>/stat` (heure de démarrage) ou `comm` avant de tuer ; mieux, une unité `systemd-run --user --scope` par app lancée.

### STLIN-22 — `release-station.sh` sort sans message
- **Catégorie** : bug (publication). **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/scripts/release-station.sh:109-110` : `rpm="$(remote … "ls $dist/*.rpm 2>/dev/null" | head -1)"` sous `set -euo pipefail`.
- **Scénario** : pas de .rpm → l'affectation échoue et le script sort avant le message prévu l. 111 (« rpmbuild ? »).
- **Correctif** : `|| true` sur les lignes `deb=` et `rpm=`.

### STLIN-23 — Publication : cache et construction de Kyber pour Linux
- **Catégorie** : bug (publication). **Effort** : S. **Confiance** : vérifié.
- **Où** : `scripts/release.sh:589` (`entrees_station_linux`) ; `server/linux/scripts/build-kyber.sh`.
- **Description** : (a) les entrées du cache de l'étape `station_linux` n'incluent ni `apps/macos/kyber-patches` (que `apply.sh` repose toujours, l. 85, et dont dépend le 0027 commun) ni `kyber_tetes linux …`, contrairement à `entrees_station_mac` (l. 583) ; (b) le `build-kyber.sh` Linux n'a ni le verrou `scripts/verrou.sh` (le Mac l'a, l. 53-55) ni l'empreinte `$OUT/.kybernow-patches` (Mac l. 170) : `package.sh` et `run-station.sh` ne regardent que la présence de `kycontroller` ; (c) la remise à l'origine de kyctl/kymux/kynput est faite par l'appelant (`release-station.sh:87-91`) et pas par `build-kyber.sh`, dont l'en-tête la promet.
- **Scénario** : un changement du correctif stylet des apps ne reconstruit pas la Station Linux à la publication suivante (cache réutilisé) ; en développement, un `package.sh` emballe des binaires construits avec d'anciens correctifs ; deux constructions simultanées sur `kyber-desktop-linux` se mélangent.
- **Correctif** : ajouter `apps/macos/kyber-patches` et `kyber_tetes linux kyber-desktop-linux` aux entrées ; verrou et empreinte comme sur Mac, empreinte vérifiée par `package.sh` ; remise à l'origine dans `build-kyber.sh`.

### STLIN-24 — Presse-papiers Wayland : un fil par collage (correctif 0007)
- **Catégorie** : optimisation / fuite. **Effort** : M. **Confiance** : plausible.
- **Où** : `server/linux/kyber-patches/0007-kynput-presse-papiers-wayland.patch:705-707`.
- **Scénario** : une app demande plus de 64 Ko puis ne lit pas le tube : le fil, le descripteur et les données restent jusqu'à l'arrêt de kynputserver.
- **Correctif** : écriture non bloquante dans la boucle `mio`, échéance `PASTE_TIMEOUT`.

### STLIN-25 — Code mort : boucle du stylet dans `build-kyber.sh`
- **Catégorie** : code mort. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/scripts/build-kyber.sh:61-73`.
- **Description** : la boucle pose `apps/macos/kyber-patches/*-kynput-stylet.patch` « s'il manque », mais `kyber_apply_server_patches` (`server/kyber-patches/apply.sh:85-96`) vient de reposer **tous** les correctifs `apps/macos`, ou s'est arrêtée : le `--reverse --check` réussit toujours. Le commentaire (« sous Windows et sur Mac, la copie de Kyber l'a déjà par les scripts des apps ») est périmé.
- **Correctif** : supprimer la boucle.

### STLIN-26 — Écarts de documentation (Linux)
- **Catégorie** : doc. **Effort** : S. **Confiance** : vérifié.
- `server/linux/INSTALLATION.md:12-13` (et `.en.md`) : `kybernow-station_<version>_amd64.deb` / `kybernow-station-<version>.x86_64.rpm`, alors que les fichiers publiés sont `KyberNow-Station-Linux-<arch>.deb/.rpm` (`release-station.sh:112-113`, `update/linux.rs:60-65`) ; l. 5 ne cite que GNOME et KDE (pas Sway ni Hyprland).
- `PROJECT.md:49` (et `.en.md`) : « trois propres à Linux » ; il y en a 11.
- `docs/station-linux.md:65` : le démarrage proposé pour Sway/Hyprland (`exec systemctl --user start …`) oublie `systemctl --user import-environment WAYLAND_DISPLAY SWAYSOCK HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP`, dont dépend la détection (`wlroots.rs:42-58`).
- `docs/station-linux.md:41` et correctif `0001…:120-121` : le fichier des écrans serait relu « à chaque image » ; il l'est toutes les 250 ms et au démarrage du flux. Dans le même correctif (l. 103-107), le commentaire de `x`/`y` est placé au-dessus de `id`.
- `server/camera/linux/modprobe-kybernow-camera.conf:4` et `modules-load-kybernow-camera.conf:2` : « installé dans /etc » ; le paquet installe dans `/usr/lib` (`package.sh:79-82`). `options v4l2loopback devices=1` s'impose à tout le système (la caméra virtuelle d'OBS retrouve un module déjà chargé avec un seul périphérique) : envisager `devices=2` ou un `exclusive_caps` documenté.

### STLIN-27 — uinput et uhid ouverts à la session (choix à écrire)
- **Catégorie** : sécurité (choix assumé). **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/linux/packaging/60-kybernow-uinput.rules:4, 6` (`TAG+="uaccess"`).
- **Description** : tout processus de la session active peut créer un clavier virtuel et taper dans un terminal ou un dialogue polkit. C'est le niveau de `steam-devices`, cohérent avec un service de l'utilisateur, mais la doc ne le dit pas.
- **Correctif** : une ligne dans `docs/station-linux.md` (Entrées), et le garder en tête si STLIN-05 passe par une règle `uaccess` sur les entrées physiques.

---

## Station Mac

### STMAC-01 — `kybernow-display lock` ne verrouille jamais
- **Catégorie** : bug critique (sécurité du prêt). **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/display-agent/main.swift:613` : `["audio", "apps", "launch", "close", "locked"].contains(…)` ; `"lock"` manque, alors que `apps.swift:153` le traite.
- **Scénario** : Admin redémarre en plein prêt ; `lend_api::recover` (`lend_api.rs:323`) appelle `lend_sys::lock_session` → `kybernow-display lock` (`lend_sys.rs:933`). L'agent ne reconnaît pas la commande et passe en mode agent : il rallume les écrans de `display-off.json`, lit une entrée vide (`Command::output` ne donne pas d'entrée), appelle `shutdown()`, qui écrit `{}` dans `virtual-displays.json`, et sort avec 0. Admin journalise une session verrouillée qui ne l'est pas ; seul l'agent de console, s'il tourne encore, verrouille de son côté.
- **Correctif** : ajouter `"lock"` à la liste ; un test qui lance chaque sous-commande d'`appsCommand` éviterait la récidive.

### STMAC-02 — L'agent des écrans meurt sans rendre les écrans du Mac
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié (code).
- **Où** : aucun `signal(SIGPIPE, SIG_IGN)` ni traitement de SIGTERM dans `display-agent/` (alors que `gamepads/main.swift:252` le fait) ; `admin/src/display/mac.rs:110` lance l'agent sans `process_group`, au contraire de l'agent de console (`lend_sys.rs:949`, « pour que launchd ne l'arrête pas avec Admin »).
- **Scénario** : « Redémarrer », mise à jour ou désinstallation pendant une session : launchd arrête le groupe, l'agent meurt, les écrans virtuels disparaissent et les écrans du Mac restent éteints jusqu'au `recover` du prochain Admin (jamais après une désinstallation). Autre cas : Admin tombe pendant un `isolate` (8 s + 4 s par écran), `reply()` écrit dans un tube fermé → SIGPIPE. `docs/station-macos.md:30` promet l'inverse (« par l'agent si Admin s'arrête »).
- **Correctif** : `signal(SIGPIPE, SIG_IGN)` ; sources de signaux (`DispatchSource.makeSignalSource`) pour SIGTERM/SIGINT/SIGHUP qui appellent `agent.shutdown()` ; `process_group(0)` au lancement par Admin.

### STMAC-03 — Caméra : entrée ouverte à toutes les apps
- **Catégorie** : sécurité. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/camera-extension/main.swift:251-254` : `authorizedToStartStream(for:)` rend toujours `true`.
- **Scénario** : n'importe quelle app du Mac ouvre l'entrée (« sink ») de « KyberNow Camera » et y pousse ses images, que FaceTime ou Zoom présentent comme la caméra de l'appareil.
- **Correctif** : pour `direction == .sink`, n'accepter que `client.signingID == "fr.leslignesclaires.kybernow.station.kybernow-camera"` (et l'équipe).

### STMAC-04 — Droit `audio-input` donné à tous les exécutables
- **Catégorie** : sécurité (moindre privilège). **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/scripts/build-station-app.sh:236-238` signe kycontroller, kyavserver, kynputserver, kybernow-pyrowave-check, kybernow-encoder-check, kybernow-micro, kyber-admin, kybernow-display, kybernow-ui et kybernow-console avec `station.entitlements` (`com.apple.security.device.audio-input`).
- **Correctif** : ce droit pour kyavserver (capture) et kybernow-micro seulement ; aucun droit pour les autres.

### STMAC-05 — « Accepter » un prêt sans déverrouiller
- **Catégorie** : sécurité. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/macos/menubar/Notify.swift:29` : `UNNotificationAction(identifier: "accept", …, options: [])`.
- **Scénario** : Mac verrouillé qui affiche ses notifications : un passant accorde l'emprunt.
- **Correctif** : `options: [.authenticationRequired]`.

### STMAC-06 — launchctl vise la session devant l'écran, pas celle de la Station
- **Catégorie** : bug. **Effort** : S. **Confiance** : code vérifié, scénario plausible.
- **Où** : `server/admin/src/mac.rs:118-124`, `update/mac.rs:76-79`, `station.rs:221` : `gui/<propriétaire de /dev/console>` (0 si illisible).
- **Scénario** : changement rapide d'utilisateur ou fenêtre de connexion : l'état de Kyber passe à « non installé », « Redémarrer » ne fait rien, et pendant une mise à jour `stop_all` ne voit aucun service et remplace l'app en marche. `lend_sys.rs:915` traite déjà ce cas.
- **Correctif** : `gui/{getuid()}` dans une seule fonction de `mac.rs` (voir STCOM-08).

### STMAC-07 — `launchctl` impossible à lancer vu comme un succès
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/update/mac.rs:81-87` : repli `ExitStatus::default()`, qui vaut « réussite ».
- **Scénario** : `loaded()` répond toujours vrai : `stop_all` attend 3 × 15 s, `finish` croit les services lancés et ne relance pas la Station.
- **Correctif** : rendre un `Result` (comme `mac.rs:126`).

### STMAC-08 — Appels à l'agent sans délai, verrou tenu
- **Catégorie** : bug. **Effort** : M. **Confiance** : code vérifié, blocage plausible.
- **Où** : `server/admin/src/display/mac.rs:99-138` (`call` : `read_line` sans délai, verrou de l'agent tenu), `:231-240` (`restore` : `child.wait()` sous verrou), `:217` (`remove` vide le rang avant la réponse).
- **Scénario** : agent bloqué dans CoreGraphics (sortie de veille, écran qui ne répond pas) : tout le gestionnaire d'écrans d'Admin se fige, `observe` aussi. Si `remove` échoue, l'agent garde son écran mais Admin l'a oublié.
- **Correctif** : lecture avec délai (fil dédié + canal), `wait` borné puis `kill`, ne vider `created` qu'après succès. À faire une seule fois pour Mac et Linux (STCOM-06).

### STMAC-09 — Sortie d'avant = KyberNow, Mac muet après la session
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/display-agent/audio.swift:118-119` retient la sortie par défaut sans vérifier qu'elle n'est pas `kyberNowUID` ; Linux le fait (`audio/mod.rs:491`, `record.previous != SINK`).
- **Scénario** : second `route` (reconnexion), ou enregistrement perdu après un plantage : `restore` remet KyberNow par défaut, le Mac n'a plus de son.
- **Correctif** : si la sortie d'avant est KyberNow, rendre `""` (et `restore` choisit alors la sortie intégrée).

### STMAC-10 — `isolate` laisse une configuration d'écrans ouverte
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/display-agent/main.swift:170-175` : `CGBeginDisplayConfiguration`, puis `try int(screen, "id")`… dans la boucle ; une exception sort sans `CGCancelDisplayConfiguration`.
- **Correctif** : lire et valider tous les champs avant `Begin`, ou `defer { if !completed { CGCancelDisplayConfiguration(config) } }`.

### STMAC-11 — `create` sur un emplacement occupé sans garde
- **Catégorie** : bug. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/macos/display-agent/main.swift:110` : `virtuals[slot] = nil` sans la garde de `remove` (l. 82-84 : `restore()` d'abord si c'est le dernier écran virtuel et que les écrans du Mac sont éteints).
- **Scénario** : redemande d'écran sur le même rang avec les écrans du Mac éteints : le Mac se retrouve un instant sans écran ; macOS rallume l'intégré et pas les autres (commentaire de `remove`).
- **Correctif** : passer par `remove(slot)` dans `create`.

### STMAC-12 — `display-off.json` non atomique
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/display-agent/main.swift:553` : `.write(to: Self.file)` sans `.atomic` (alors que `writePanelFile`, l. 480, l'utilise).
- **Scénario** : coupure pendant l'écriture : `load()` rend `nil`, les écrans éteints ne sont jamais rallumés par `recover`.
- **Correctif** : `options: .atomic`.

### STMAC-13 — Notifications reposées à chaque raté d'Admin
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/menubar/main.swift:50-55` appelle `notifier.update(s)` même quand `s.state == nil` ; `Notify.swift:76-79` retire alors toutes les notifications connues, qui reviennent « nouvelles », avec le son, au relevé suivant.
- **Correctif** : ne pas appeler `update` quand `state == nil`.

### STMAC-14 — File de relevés de l'icône sans limite
- **Catégorie** : bug. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/macos/menubar/main.swift:39` (relevé toutes les 2 s sur une file série) ; chaque relevé lance deux `launchctl print` et un appel HTTPS de 4 à 5 s (`Station.swift:162, 180`).
- **Scénario** : Admin en marche mais bloqué : un relevé dure ~4 s pour un toutes les 2 s, la file grossit sans fin ; en temps normal, ~1 `launchctl` par seconde en permanence.
- **Correctif** : drapeau « relevé en cours » qui saute les ticks ; intervalle plus long menu fermé.

### STMAC-15 — Pilote son : un compteur d'E/S pour deux appareils
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/audio-driver/KyberNowAudio.c:135` (`gMicIORunning`), lu par `DeviceIsRunning` l. 795 pour les deux appareils du micro, incrémenté/décrémenté l. 927/934.
- **Scénario** : seule la sortie cachée tourne, « Micro KyberNow » se dit pourtant en marche (indicateur de micro, apps qui croient le micro pris).
- **Correctif** : un compteur par appareil (l'horloge commune peut rester partagée). Les deux horloges partagent aussi `gIOMutex`, pris dans `GetZeroTimeStamp` sur les fils temps réel pendant que `StartIO` vide 512 Ko (l. 638, 656, 924, 939) : un verrou par horloge et le `memset` hors verrou. À noter : `update/mac.rs` ne remplace pas le pilote, ce correctif demande une réinstallation (le dire dans les notes).

### STMAC-16 — Aide des manettes morte : manettes muettes (correctif 0011)
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/kyber-patches/0011-kynput-manettes-mac.patch:555-558` (`report` : `if self.helper.is_none() { return Ok(()) }`), `:498-507` (`send` oublie l'aide morte), `:393` (« helper ended » sans prévenir le `Backend`).
- **Scénario** : l'aide KyberNow Manettes plante en pleine partie : le prochain rapport échoue, `helper = None`, puis tous les rapports sont jetés sans bruit jusqu'au prochain branchement de manette.
- **Correctif** : dans `report()`, relancer l'aide après `HELPER_RETRY` et recréer toutes les manettes (comme `plug` le fait déjà pour une aide neuve).

### STMAC-17 — Nombre de voies figé (correctif 0006)
- **Catégorie** : bug. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/macos/kyber-patches/0006-txproto-son-kybernow.patch:207-223` : `entry->channels = kn_ch` seulement à la création de l'entrée ; la branche « entrée existante » ne le met pas à jour, alors que la capture prend le nombre de voies du moment.
- **Scénario** : Kyber démarre en stéréo, Admin passe la sortie KyberNow en 5.1 : l'entrée annonce 2 voies et pousse des trames à 6. Cohérent avec le 5.1/7.1 « pas encore essayé ».
- **Correctif** : remettre à jour `channels`/`ch_layout` (et signaler le changement) dans la branche existante et au démarrage de la capture.

### STMAC-18 — Chemin du dépôt principal faux hors de la racine
- **Catégorie** : bug (outillage). **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/scripts/build-station-app.sh:33`, `run-station.sh:96` : `cd "$(git -C "$ROOT" rev-parse --git-common-dir)/.."` ; dans le dépôt principal git rend `.git`, relatif à `$ROOT`, mais le `cd` part du dossier courant.
- **Scénario** : lancé depuis un autre dossier, le script s'arrête ou tombe sur un autre dépôt. `release.sh` le lance depuis la racine, d'où l'absence de symptôme en publication.
- **Correctif** : `git rev-parse --path-format=absolute --git-common-dir`.

### STMAC-19 — Allocations dans le rappel temps réel (correctifs 0006, 0013)
- **Catégorie** : optimisation. **Effort** : M. **Confiance** : plausible.
- **Où** : `0006-txproto-son-kybernow.patch:120, 138, 150` et `0013-txproto-son-par-10-ms.patch:37, 53` : `av_frame_alloc`, `av_frame_get_buffer`, `av_buffer_allocz` et `sp_log` à chaque trame de 10 ms dans le rappel de Core Audio.
- **Scénario** : sous charge, inversions de priorité sur le fil de Core Audio → craquements ; les pertes en rafale sont aggravées par le journal.
- **Correctif** : anneau sans verrou rempli par le rappel, vidé par un fil ordinaire qui fabrique les `AVFrame` ; journal limité en fréquence.

### STMAC-20 — Caméra : image grise allouée à chaque image
- **Catégorie** : optimisation. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/camera-extension/main.swift:189` (`greyBuffer` : `CVPixelBufferCreate` + `memset` ~3 Mo en 1080p, 30 fois par seconde sans flux de l'appareil) ; boucle `consume()` qui interroge toutes les 5 ms (l. 286).
- **Correctif** : garder l'image grise par taille ; attendre sur un sémaphore plutôt que d'interroger.

### STMAC-21 — PyroWave : attente du processeur à chaque image
- **Catégorie** : optimisation. **Effort** : M. **Confiance** : plausible.
- **Où** : `server/macos/pyrowave/kn_pyrowave_metal.mm:286` : `[cmd waitUntilCompleted]` pour ordonner deux files Metal.
- **Correctif** : `MTLSharedEvent` signalé ici et attendu par la file de PyroWave (gain estimé 0,3 à 1 ms par image, à mesurer).

### STMAC-22 — Code mort et latent (Mac)
- **Catégorie** : code mort. **Effort** : S. **Confiance** : vérifié.
- `camera-extension/main.swift:86, 202` : `sequence` incrémenté, jamais lu.
- `display-agent/main.swift:85` : `remove` laisse `retina`, `modes` et `scaled` de l'écran retiré (petite fuite, et `writePanels` filtre ensuite par `live`).
- `pyrowave/kn_pyrowave_metal.mm:338-339` accepte le P010 pleine plage mais `:438` annonce toujours la plage réduite ; sans effet aujourd'hui (la capture ne demande que `VideoRange`, correctifs 0010 et 0019) : refuser ce format.

### STMAC-23 — Contrôle de l'équipe de signature contournable
- **Catégorie** : sécurité (basse, seconde ligne). **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/update/mac.rs:232-237` : `signature(&layout.install).unwrap_or_default()` puis `!installed_team.is_empty() && …`.
- **Description** : si l'app installée a une signature illisible, n'importe quelle équipe est acceptée. Le zip est déjà couvert par le manifeste signé : c'est la défense en profondeur qui tombe.
- **Correctif** : comparer à l'équipe compilée dans Admin (`P6DB682S4K`) par `codesign --verify -R '=anchor apple generic and certificate leaf[subject.OU] = "…"'`.

### STMAC-24 — Liste « pas un jeu » par noms anglais
- **Catégorie** : bug mineur. **Effort** : M. **Confiance** : plausible.
- **Où** : `server/admin/src/foreground.rs:221-226` (`MAC_NOT_GAMES`) comparée à `kCGWindowOwnerName`, qui est le nom affiché (traduit : « Aperçu », « Musique »).
- **Correctif** : comparer l'identifiant de paquet (`NSRunningApplication` via le pid propriétaire).

### STMAC-25 — Écarts de documentation (Mac)
- **Catégorie** : doc. **Effort** : S. **Confiance** : vérifié.
- `docs/station-macos.md:30` : « par l'agent si Admin s'arrête » (faux, STMAC-02).
- `:72` : « écrans physiques éteints ou recopiés (… à choisir aux essais) » : c'est tranché, éteints.
- `:85` : « applicateur root » : l'applicateur tourne dans la session de l'utilisateur, sans mot de passe (`update/mac.rs:5-20`).
- `:149` : « LaunchDaemon et LaunchAgents » : il n'y a que des LaunchAgents.
- `:15` : « 30 correctifs serveur de Windows » : il y en a 59 aujourd'hui.
- `audio-driver/KyberNowAudio.c:4` : « 48 kHz » ; 44,1 kHz est aussi accepté (l. 64).
- `server/macos/kyber-patches/0020-kynput-stylet-mac.patch:47` : « server/macos/kyber-patches/0016 » ; c'est 0020 (le 0016 est PyroWave Metal).
- `server/macos/scripts/build-kyber.sh:9` : « (s'il y en a) » ; il y en a 21.

---

## Commun aux deux Stations (Admin, correctifs, outillage)

### STCOM-01 — Admin ignore SIGTERM sur Mac et Linux
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/main.rs:760-763` : arrêt propre sur `tokio::signal::ctrl_c()` seulement (Windows passe par `winsvc`).
- **Scénario** : launchd (Mac) et systemd (Linux) arrêtent Admin par SIGTERM : aucun `restore` des écrans, du son (sortie par défaut laissée sur KyberNow) ni du prêt ; c'est le `recover` du démarrage suivant qui rattrape, s'il y en a un. Aggrave STMAC-02 et STLIN-04.
- **Correctif** : attendre aussi `tokio::signal::unix::signal(SignalKind::terminate())` (et `hangup`) dans le futur d'arrêt.

### STCOM-02 — `secure_dir` et `secure_file` ne font rien hors Windows
- **Catégorie** : sécurité. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/update/acl.rs:65-95` (« simulation, développement ») ; `key.pem` (`tls.rs:29`), réglages et jeton local écrits par `durable::write`, donc 0644.
- **Description** : seuls les dossiers parents protègent aujourd'hui : `chmod 700` dans `kybernow-station-setup:61` (Linux, première initialisation seulement) et `~/Library` (Mac). Une installation dans le dossier de l'utilisateur, un `STATION_DIR` choisi ou une copie de sauvegarde exposent la clé TLS.
- **Correctif** : sur Unix, `secure_dir` → 0700 et `secure_file` → 0600 (refus d'un lien, comme sous Windows) ; `durable::write_then` sait déjà poser les droits avant le renommage.

### STCOM-03 — Processus zombie après délai dépassé
- **Catégorie** : bug. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/micro.rs:104-106` : `child.kill(); return None;` sans `wait()`.
- **Scénario** : `kybernow-micro --check` qui dépasse 5 s laisse un zombie à chaque `GET /api/micro`.
- **Correctif** : `child.wait()` après `kill` (comme `pyrowave.rs:287`, `gamepads.rs:68`), ou mieux STCOM-07.

### STCOM-04 — Paquet de mise à jour en mémoire, 120 s au total
- **Catégorie** : optimisation / bug. **Effort** : M. **Confiance** : plausible (selon la taille réelle et le débit).
- **Où** : `server/admin/src/update.rs:621` (`timeout_global(120 s)`), téléchargements vers un `Vec` (l. 458, 561).
- **Scénario** : .rpm (~79 Mo) ou zip de l'app Mac sur une connexion lente (moins de ~6 Mbit/s pour le .rpm) : échec systématique ; pic mémoire égal à la taille du paquet.
- **Correctif** : écriture en flux dans le fichier avec empreinte calculée au fil de l'eau ; délai sans activité plutôt que global.

### STCOM-05 — Application des correctifs : fuzz et état amont
- **Catégorie** : robustesse (construction). **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/kyber-patches/apply.sh:75`, `server/macos/scripts/build-kyber.sh:102`, `server/linux/scripts/build-kyber.sh:108` : `patch -p1 --forward --no-backup-if-mismatch -r -` sans `-F0` ; ni commit de Kyber ni propreté de `KYBER_DIR` vérifiés.
- **Scénario** : un morceau de correctif Mac (écrits sans contexte de fonction, numéros qui dérivent) posé au mauvais endroit avec « fuzz 2 », sans échec ; un correctif retiré laisse ses fichiers créés dans une copie de développement (les arbres neufs de `release.sh` en sont protégés).
- **Correctif** : `-F0` (échec si la sortie contient « fuzz ») ; contrôle `git rev-parse HEAD` contre le commit attendu et `git status --porcelain` sur kymedia, kyctl, kymux, kynput.

### STCOM-06 — `display/mac.rs` et `display/linux.rs` presque identiques
- **Catégorie** : factorisation. **Effort** : M. **Confiance** : vérifié.
- **Où** : `server/admin/src/display/mac.rs` (347 l.) et `display/linux.rs` (310 l.) : 191 lignes de différence seulement ; `Agent`, `agent_path`, `hidpi`, `call`, `isolate_with`, `mode_of`, `create`, `remove`, `observe`, `set_mode`, `displays` en double. Écarts réels : `scales` (Mac), arrêt de l'agent après `restore` (Mac), `set_hdr`/`driver` ; `_start` ignoré sous Linux.
- **Correctif** : un `AgentBackend` commun (nom de l'agent, politique de relance), qui portera STMAC-08 une seule fois.

### STCOM-07 — « Lancer avec délai » copié quatre fois
- **Catégorie** : factorisation. **Effort** : S. **Confiance** : vérifié.
- **Où** : `micro.rs:94-116`, `linux_screen.rs:122-148`, `pyrowave.rs:276-292`, `gamepads.rs:61-71` (boucle `try_wait`/`sleep`/`kill`), dont une copie oublie le `wait` (STCOM-03).
- **Correctif** : `run_with_timeout(Command, Duration) -> Option<(ExitStatus, Vec<u8>)>` dans un module commun.

### STCOM-08 — Petits utilitaires Mac en plusieurs copies
- **Catégorie** : factorisation. **Effort** : S. **Confiance** : vérifié.
- `/dev/console` lu à quatre endroits (`mac.rs:119`, `update/mac.rs:78`, `station.rs:221`, `lend_sys.rs:915`) ; étiquette `fr.leslignesclaires.kybernow.station.admin` en dur dans `station.rs:222` et `update/mac.rs:41` ; `home()` avec repli `/tmp` cinq fois (`update/mac.rs:94`, `recording.rs:100`, `lend_sys.rs:904`, `lend_sys.rs:1102`, `display/mac.rs:51`) ; `own_uid()` (`lend_sys.rs:908`) déduit l'uid du propriétaire de `$HOME` au lieu de `getuid()`.
- **Correctif** : tout dans `mac.rs` (`uid()`, `domain()`, `home()`, constantes d'étiquettes), et l'équivalent dans `linux.rs`.

### STCOM-09 — DualSense en double (Mac 0011, Linux 0005)
- **Catégorie** : factorisation. **Effort** : M. **Confiance** : vérifié.
- **Où** : `DUALSENSE_DESCRIPTOR: [u8; 273]` identique octet pour octet (Mac 0011 l. 648-689, Linux 0005 l. 746-787) ; `PadState`, `hat`, `stick8`, `dualsense_report`, `rumble_from_output` et la liste `SONY` en deux versions qui divergent déjà (`Vec` contre `[u8; 64]`, `trigger` contre `trigger8`).
- **Correctif** : `kynput/src/kybernow_dualsense.rs` dans un correctif commun (`server/kyber-patches`), utilisé par les deux.

### STCOM-10 — PyroWave réécrit deux fois (Mac 0017/0018, Linux 0011/0012)
- **Catégorie** : factorisation. **Effort** : M. **Confiance** : vérifié.
- **Description** : chacun réécrit les mêmes lignes du commun 0025 (`ENCODER` selon `cfg`, `pyrowave()` de kycontroller) ; variable locale `station` sur Mac, constante `crate::pyrowave::STATION` sur Linux, paramètre toujours nommé `windows` ; messages divergents (« Windows or Mac Station only » dans Mac 0017 l. 23, « Windows, Mac or Linux Station only » dans Linux 0011 l. 78).
- **Correctif** : `ENCODER` et `STATION` par `cfg` pour les trois systèmes dans le 0025 commun ; supprimer Mac 0018, Linux 0012 et la moitié de Mac 0017 et Linux 0011.

### STCOM-11 — Deux `build-kyber.sh` à ~60 % identiques, empreinte en double
- **Catégorie** : factorisation. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/macos/scripts/build-kyber.sh` et `server/linux/scripts/build-kyber.sh` (même remise à l'origine, même boucle de correctifs propres, même empreinte du wrap FFmpeg) ; liste d'empreinte tenue à la main dans `build-kyber.sh:170` (Mac) et `scripts/release.sh:336` (« mêmes fichiers, même ordre »), sans `scripts/kyber-toolchain.sh`.
- **Correctif** : fonctions communes dans `server/kyber-patches/apply.sh` (correctifs propres à une Station, empreinte) sourcées par les deux scripts et par `release.sh` ; chaîne d'outils dans l'empreinte. Porte aussi STLIN-23.

### STCOM-12 — Message « pas encore d'écran virtuel sur Mac » affiché sous Linux
- **Catégorie** : doc / i18n. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/display/mod.rs:2427` (`Absent`, `#[cfg(not(windows))]`) → `display.noMacVirtualDisplay` (`i18n/fr.json:62`).
- **Scénario** : Station Linux sans `kybernow-display` : le texte parle du Mac ; sur Mac, « pas encore » est dépassé (c'est l'agent qui manque).
- **Correctif** : clé neutre `display.noVirtualDisplayAgent` (FR et EN).

### STCOM-13 — Code mort et parité
- **Catégorie** : code mort / parité. **Effort** : S/M. **Confiance** : vérifié.
- `server/admin/src/update.rs:764-768` : `launch_applicator` hors Windows, marqué `allow(dead_code)` pour Mac et Linux, ne sert sur aucune plateforme livrée : à supprimer.
- `server/admin/src/api.rs:1411` → `mac::preferred_host`, qui rend `None` hors macOS : une Station Linux en Ethernet et Wi-Fi sur le même réseau garde l'adresse Wi-Fi. `preferred_among` ne dépend pas du système : brancher Linux (`ip -o -4 addr`).

### STCOM-14 — Numérotation des correctifs
- **Catégorie** : doc / nommage. **Effort** : S. **Confiance** : vérifié.
- Les numéros Mac et Linux (0001-0030) recouvrent les communs (0001-0085) et les commentaires citent des numéros nus : dans Mac 0017, « KyberNow (0020) » désigne l'image d'invité commune alors que Mac 0020 est le stylet ; dans Mac 0009, « (0007) » désigne Mac 0007 alors que le commun 0007 est kynet. Côté commun, deux 0085 (`flux-montant`, `micro-droits`) touchent les mêmes fichiers (`uplink.rs`, `kymux_controller/mod.rs`) et ne tiennent que par l'ordre alphabétique.
- **Correctif** : écrire « Mac 00xx » / « Linux 00xx » / « commun 00xx » dans les commentaires ; renuméroter le second 0085 en 0086.

---

## Mes 10 priorités

1. **STLIN-01** — `O_NOFOLLOW` + `fstat` dans `copy_limited`, réserve en 0700/0600, nettoyage sur refus : un compte local lit aujourd'hui n'importe quel fichier root de moins de 256 Ko.
2. **STLIN-02 + STLIN-03** — socket réservé au propriétaire (groupe ou uid enregistré), délai de lecture, `flock` : ferme le service root aux autres comptes.
3. **STMAC-01** — ajouter `"lock"` à la liste des commandes de l'agent : une ligne, et un prêt interrompu ne laisse plus la session ouverte.
4. **STCOM-01 + STMAC-02 + STLIN-04** — SIGTERM traité par Admin et par les deux agents, SIGPIPE ignoré, agent Mac dans son propre groupe : écrans et son rendus à chaque arrêt, mise à jour ou désinstallation.
5. **STMAC-03** — caméra : n'accepter que `kybernow-camera` en entrée.
6. **STLIN-06** — jamais d'autre écran que celui demandé pour un invité ou un prêt.
7. **STLIN-05** — prêt sous Linux : vraie prise de l'entrée physique, ou au moins l'afficher dans Admin et corriger la doc.
8. **STCOM-02 + STMAC-04 + STMAC-05** — droits Unix des secrets, droit `audio-input` restreint, acceptation de prêt avec déverrouillage.
9. **STMAC-16 + STLIN-12 + STLIN-13** — entrées qui ne reviennent jamais après un échec passager (aide des manettes, pointeur absolu, uhid).
10. **STLIN-23 + STCOM-05 + STCOM-11** — construction de Kyber fiable des deux côtés : cache complet, verrou et empreinte sous Linux, `-F0`, scripts mis en commun.

Ensuite, en lot rapide (S) : STMAC-09, -10, -12, -13, STLIN-07, -08, -16, -17, -19, -22, STCOM-03, -12, puis les docs (STMAC-25, STLIN-26). Les factorisations STCOM-06, -09, -10 vont bien avec la vague suivante de travail sur les écrans et les manettes.

## Ce qui est sain

- **Mise à jour Linux, hors STLIN-01/02/03** : manifeste minisign revérifié par root avec la clé compilée, version strictement plus récente, canal, taille et empreinte contrôlés ; `valid_version` partout (pas de traversée de chemin) ; JSON de demande strict ; réponse envoyée avant tout arrêt ; retour arrière automatique sur contrôle de santé raté.
- **Mise à jour Mac** : échange de l'app par renommage, retour arrière, contrôle de santé, plist launchd échappé (`update/mac.rs:319`), applicateur sans mot de passe.
- **Pas de commande construite par concaténation** : tout passe par `Command::args` ; `linux_screen::pipeline` filtre ce qui entre dans GStreamer.
- **Pilote son** : indices des anneaux bornés (`frames > kRingFrames` refusé, coupure en deux `memcpy` correcte), changement de format par `RequestDeviceConfigurationChange`, aucune allocation sur le fil temps réel du pilote.
- **Droits Apple** minimaux pour l'aide des manettes et l'extension caméra (bac à sable) ; `system-extension.install` seulement avec le profil.
- **Icône Mac** : certificat d'Admin épinglé par SHA-256 de la feuille.
- **Scripts** : `set -euo pipefail` partout, chemins entre guillemets, mot de passe d'essai sous `umask 077`, `-nt "$STARTED"` sur les binaires, verrou de construction sur Mac, version des crates corrigées vérifiée (`quinn-udp`, correctif limité aux systèmes Apple), empreinte du wrap FFmpeg et Lua.
- **Agent des écrans Linux** : fichier des écrans écrit puis renommé, rang borné, filet de sécurité wlroots au démarrage, `unwrap` sur verrous seulement.
- **Paquets** : prerm/`%preun` distinguent mise à jour et retrait ; pare-feu ouvert à l'installation et refermé au retrait ; première initialisation idempotente (`chmod 700`, clé en 600).
- **Presse-papiers Wayland** (Linux 0007) : lecture bornée à `MAX_CLIPBOARD_SIZE` avec délai, sélection primaire et notre type MIME filtrés.
- **Correctifs** : aucun correctif Mac ou Linux ne reprend un morceau d'un correctif commun ; les recouvrements de fichiers sont voulus et tiennent ; les `cfg` ajoutés pour le Mac dans les correctifs communs ne cassent pas Windows ; Mac 0005 ne refuse aux invités que ⌃⌘Q et ⇧⌘Q, avec des essais.
- **Aucun TODO ni FIXME** dans `server/macos`, `server/linux` ni dans les parties Mac/Linux d'Admin.
