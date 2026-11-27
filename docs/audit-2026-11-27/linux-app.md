# Audit du 27/11/2026 — app Linux (`apps/linux`)

Zone : l'app KyberNow Link pour Linux dans toutes ses formes (bureau, Steam Deck, mode salon, Flatpak 25.08, paquet `.deb` Raspberry Pi), l'image KyberNow Link OS (`apps/linux/os`, pi-gen, A/B, CEC), les scripts de construction (`apps/linux/scripts`) et l'application des correctifs Kyber de l'app. Environ 58 000 lignes, dont 10 000 dans `src/app.rs`.

Méthode : analyse statique seulement (lecture, grep, `git log`), sans compilation, sans essai et sans appareil. Un seul essai a été fait, hors du projet, dans un dossier temporaire : le comportement de `set -o pipefail` avec `xz -dc | head` et `xz -dc | dd count=` (sortie 141, voir LINAPP-01). Chaque constat ci-dessous a été relu dans le code. Le niveau « vérifié » veut dire que le code lu suffit à établir le défaut ; « plausible » veut dire que le défaut dépend du matériel, du pilote ou d'un état qu'on ne peut pas lire. L'audit précédent (`docs/archives/audit-2026-10-20.md`) date d'avant l'app Linux : rien ici n'y était déjà traité.

Chemins abrégés : **app** = `apps/linux`, **F** = `apps/linux/os/stage-kybernow/01-link/files`.

## Synthèse

| Id | Catégorie | Où | Constat | Effort | Confiance |
|---|---|---|---|---|---|
| LINAPP-01 | bug critique | F/kybernow-os-update:94 | `apply` s'arrête sur SIGPIPE de `xz` (pipefail) : aucune mise à jour A/B ne peut aboutir | S | vérifié |
| LINAPP-02 | bug critique | F/kybernow-os-update:36 ; app/src/device/os.rs:30 | Le dossier d'état de root est aussi le dossier de téléchargement de l'app : téléchargement refusé (EACCES) | S | vérifié |
| LINAPP-03 | bug critique | F/kybernow-os-update:133 ; app/os/make-ab-image.sh:107 | Une mise à jour A/B perd le Wi-Fi, les manettes Bluetooth, SSH, le mode HDMI et le nom | M | vérifié |
| LINAPP-04 | bug critique | app/src/device/mod.rs:2238 | « Prêter » et « Écran » ouverts depuis le menu : clavier et manette partent au PC, le panneau ne répond pas | M | vérifié |
| LINAPP-05 | bug | app/src/device/mod.rs:2238 ; app/src/app.rs:4508 | Menus déroulants de la barre : ni clavier ni Échap, menu qui reste ouvert | S | vérifié |
| LINAPP-06 | bug | app/src/device/mod.rs:1541 | Mode immersif : clavier saisi pendant le menu et les panneaux (contraire à link-ui) | S | vérifié |
| LINAPP-07 | bug | F/kybernow-os-update:153 ; app/src/ui/device.rs:179 | Un essai raté est validé sans rien dire, puis présenté comme « Version précédente » | S | vérifié |
| LINAPP-08 | bug | F/systemd/kybernow-link.service:25 | Aucun chien de garde : un essai qui fige reste à l'écran jusqu'au débranchement | M | plausible |
| LINAPP-09 | bug | F/etc/sudoers-kybernow:2 | Le compte de l'app peut écrire n'importe quelle image en root, sans contrôle de signature | M | vérifié |
| LINAPP-10 | bug | app/os/make-ab-image.sh:53 | Taille de rootA/rootB variable selon la version : une version plus lourde ne tiendra plus | S | plausible |
| LINAPP-11 | bug | F/kybernow-os-update | Pas de verrou : deux `apply` peuvent écrire en même temps | S | plausible |
| LINAPP-12 | bug | app/src/device/video.rs:874 | Matrice BT.601 (JPEG) pour une image BT.709 en plage pleine (PyroWave SDR) ; réglage global de SDL | S | vérifié |
| LINAPP-13 | bug | app/src/native/decoder.c:327 | `EAGAIN` de `avcodec_send_packet` : le paquet est perdu en silence | S | vérifié |
| LINAPP-14 | bug | app/src/native/pen.c:592 | Stylet X11 : une connexion Xlib utilisée par deux fils sans `XInitThreads` | S | vérifié |
| LINAPP-15 | bug | app/src/device/video.rs:229 | File vers le décodeur sans borne : latence et mémoire qui montent si le décodage traîne | M | vérifié |
| LINAPP-16 | bug | app/src/device/video.rs:350 | États collants `software` et `failed` du fil de décodage | S | vérifié |
| LINAPP-17 | bug | app/src/device/system.rs:197 | Pi : HEVC matériel annoncé même si la compilation ne l'a pas | S | vérifié |
| LINAPP-18 | bug | app/src/native/camera.c:397 | Webcam : `bytesperline` ignoré, JPEG sans marge FFmpeg, `malloc` non vérifié | S | vérifié |
| LINAPP-19 | bug | app/src/ui/session.rs:539, 1073 | Barre et menus déroulants qui sortent de la fenêtre (fenêtres de 480 pt, menus non recadrés) | M | vérifié |
| LINAPP-20 | bug | app/src/ui/session.rs:879, 890 | Trois éléments sous la même cible `BarShare` : focus triple, pastilles inatteignables | S | vérifié |
| LINAPP-21 | bug | app/src/app.rs:4115 ; app/src/ui/settings.rs:183 | Défilement des réglages sans borne basse, focus hors écran, clic sous la barre de titre | M | vérifié |
| LINAPP-22 | bug | app/src/app.rs:3572 | Lien `kybernow://` reçu pendant le test Auto ou « Hors de chez vous ? » : saisie avalée | S | vérifié |
| LINAPP-23 | bug | app/src/native/decoder.c:306 | Nom du décodeur faux dans les statistiques (« VA-API » pour un décodage processeur) | S | vérifié |
| LINAPP-24 | bug | app/src/native/pen.c:408 | Stylet X11 : événements maître et esclave reçus en double | S | plausible |
| LINAPP-25 | bug | app/src/device/pads.rs:85 | Manette ouverte jamais refermée sur deux sorties anticipées | S | vérifié |
| LINAPP-26 | bug | app/src/app_time.rs:67 | `relative_day` faux au changement d'année (« hier » le 1er janvier) | S | vérifié |
| LINAPP-27 | bug | app/build.rs:89 | PyroWave posé après une première compilation : jamais vu (`rerun-if-changed` manquant) | S | vérifié |
| LINAPP-28 | bug | app/scripts/build.sh:218 | `PYROWAVE_SRC` et `FFMPEG_SRC` peuvent effacer un clone existant | S | vérifié |
| LINAPP-29 | bug | F/etc/bluetooth-kybernow.conf:5 | `JustWorksRepairing = always` : réassociation silencieuse de tout appareil connu | S | plausible |
| LINAPP-30 | bug | app/os/build-image.sh:80 | Langue, clavier, fuseau et pays Wi-Fi figés sur la France | S | vérifié |
| LINAPP-31 | bug | app/src/native/camera.c:365 | Débit de la webcam réglé en route alors que l'encodeur ne le relit pas | M | plausible |
| LINAPP-32 | optimisation | app/src/native/decoder.c:334 ; app/src/device/video.rs:617 | 4 à 5 copies par image, même pour les images jetées ; pas de VA-API sans copie | M/L | vérifié |
| LINAPP-33 | optimisation | app/src/device/video.rs:947 ; app/src/device/mod.rs:1345 | Conversion 10→8 bits et aperçu de l'accueil sur le fil principal | S | vérifié |
| LINAPP-34 | optimisation | app/src/native/decoder.c:136 | 4:4:4 → BGRA : jusqu'à 3 fils créés et joints par image | S | vérifié |
| LINAPP-35 | optimisation | app/src/ui/canvas.rs:431 ; app/src/device/mod.rs:1058 | Interface : copie de chaque glyphe, redessin complet à 60 i/s d'écrans immobiles, barre mesurée à chaque mouvement | M | vérifié |
| LINAPP-36 | optimisation | app/src/app.rs:3819, 7063 ; app/src/device/mod.rs:1406 | Toutes les lignes de réglages reconstruites à chaque ‹ › ; `pads`/`hits` recalculés par fenêtre | S | vérifié |
| LINAPP-37 | optimisation | F/kybernow-os-update:94-131 | `apply` décompresse l'image trois fois, par blocs de 512 octets | S | vérifié |
| LINAPP-38 | optimisation | app/os/build-image.sh:55 ; app/flatpak/*.yml:37 | pi-gen rejoué en entier, clone non épinglé ; cache Flatpak perdu au moindre changement | M | vérifié |
| LINAPP-39 | optimisation | app/src/main.rs:17 | `preview.rs` (~1 800 lignes de démonstration) compilé dans le binaire livré | S | vérifié |
| LINAPP-40 | factorisation | app/src/synced.rs:50-270 | Réglages synchronisés réécrits à côté de `SyncedValues` du cœur | L | vérifié |
| LINAPP-41 | factorisation | app/src/model.rs:505 | `PsSettings` recopie `PlaySettings` du cœur | M | vérifié |
| LINAPP-42 | factorisation | app/src/device/session.rs:612 ; app/src/autotest.rs | Fonctions identiques dans les apps Linux et webOS | M | vérifié |
| LINAPP-43 | factorisation | app/scripts/build.sh:92 ; scripts/kyber-client-patches.sh | Application des correctifs Kyber en quatre copies | M | vérifié |
| LINAPP-44 | factorisation | app/scripts/build.sh:215 | Commit PyroWave écrit en dur au lieu de lire `apps/macos/PyroWave/COMMIT` | S | vérifié |
| LINAPP-45 | factorisation | app/src/ui/*.rs | `spinner`, `ghost_w`, `primary_w`, `ease`, capsules recopiés entre fichiers | S | vérifié |
| LINAPP-46 | code mort | app/src/ui/canvas.rs:251 ; app/src/ui/icons.rs:7 | 3 fonctions de dessin et 9 icônes jamais utilisées, masquées par `allow(dead_code)` | S | vérifié |
| LINAPP-47 | code mort | app/src/native/decoder.c:415, 430, 439 | `kn_decoder_flush`, `kn_v4l2_request_built`, `kn_v4l2_m2m_built` jamais appelées | S | vérifié |
| LINAPP-48 | code mort | app/src/pyrowave.rs:152 ; app/src/model.rs:527 | `lowered_label`, `PsSettings::height` et `bitrate` (tests seuls) | S | vérifié |
| LINAPP-49 | code mort | F/kybernow-os-ctl:87, 101 | `cec-wake` et `hostname` jamais appelés ; `x-systemd.growfs` redondant | S | vérifié |
| LINAPP-50 | code mort | app/Cargo.toml:15 | Dépendance directe `kynput-sys` jamais référencée (le cœur l'apporte) | S | plausible |
| LINAPP-51 | doc | app/scripts/build.sh:49 ; app/scripts/flatpak.sh:16 | PyroWave actif par défaut, la doc dit « sur demande » | S | vérifié |
| LINAPP-52 | doc | app/INSTALLATION.md:68 ; app/os/README.md:36 | Doc d'OS : retour « tout seul », réglages gardés, `autoboot.txt` « jamais à moitié écrit » | S | vérifié |
| LINAPP-53 | doc | app/src/ui/session.rs:438 ; docs/link-ui.md | Barre de session différente du tableau de link-ui ; traductions anglaises divergentes | M | vérifié |
| LINAPP-54 | doc | app/src/native/pyrowave_none.c:1 ; app/src/ui/lend.rs:31 ; app/os/build-image.sh:44 | Commentaires périmés, « session Windows » face à toute Station, message d'erreur cassé | S | vérifié |

Compte : 4 bugs critiques, 27 bugs, 8 optimisations, 6 factorisations, 5 constats de code mort, 4 de doc.

---

## Bugs critiques

### LINAPP-01 — `apply` s'arrête avant d'écrire : aucune mise à jour A/B ne peut aboutir
- **Catégorie** : bug critique
- **Où** : `F/kybernow-os-update:32` (`set -euo pipefail`), `:94`, `:114`, `:128`, `:131`
- **Description** : `xz -dc "$IMG" | head -c 1048576` (l. 94) et `xz -dc "$IMG" | dd … count=N` (l. 114, 128, 131). Dès que `head` ou `dd` a lu sa part, `xz` reçoit SIGPIPE et rend 141. Avec `pipefail`, le tube échoue et `set -e` termine le script. Essai fait hors du projet, dans un dossier temporaire : `bash -c 'set -euo pipefail; xz -dc z.xz | head -c 1048576 > h; echo atteint'` sort en 141 sans afficher « atteint », et `xz -dc | dd count=10` fait de même. L'historique montre que le piège a déjà été rencontré pour `status` (commit eb91b5e1, « update status sans pipefail »), mais pas pour `apply`. `apply` n'a jamais tourné : en QEMU, `AB_OK=0` le refuse (l. 46-47, 91), et os/README.md:98 dit qu'il n'a pas tourné sur un Pi.
- **Scénario** : Réglages de l'appareil › Mise à jour › installer. `sudo kybernow-os-update apply` sort en 141 à la ligne 94. L'app affiche une erreur à chaque essai, et `/tmp` garde un fichier `head` de 1 Mio de plus à chaque fois.
- **Correctif** : sur ces lignes, sortir du `pipefail` (`set +o pipefail` le temps du tube, puis `[ "${PIPESTATUS[1]}" = 0 ]` et contrôle du nombre d'octets écrits), ou remplacer `head` par `dd … count_bytes` et accepter le 141 de xz. Ajouter une variable réservée à `emulate.sh` (par exemple `KYBERNOW_OS_FORCE_AB=1`) pour pouvoir essayer `apply --no-reboot` en QEMU.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-02 — Le dossier d'état de root est le dossier de téléchargement de l'app
- **Catégorie** : bug critique (et sécurité)
- **Où** : `F/kybernow-os-update:36-37` (`STATE=/var/lib/kybernow/update` ; `mkdir -p` à chaque appel) ; `app/src/device/os.rs:30-33` (`update_dir()` rend le même chemin) ; `app/src/device/jobs.rs:513-515` ; `apps/core/src/update.rs:225` (`File::create(dest)`) ; `app/src/device/mod.rs:1145-1148` (`os::commit()` à chaque arrivée sur l'accueil)
- **Description** : `commit` passe par sudo dès le premier accueil, bien avant tout téléchargement. Root crée alors `/var/lib/kybernow/update` en `root:root 0755`. L'app tourne sous le compte `kybernow` : son `create_dir_all` ne dit rien (le dossier existe), puis `File::create` échoue. Dans l'ordre inverse (l'app crée le dossier la première), root écrit `incomplete`, `pending` et `boot.img` dans un dossier modifiable par `kybernow` : un lien symbolique posé à leur place fait écrire root où l'on veut.
- **Scénario** : premier démarrage, accueil, puis « Télécharger » dans Mise à jour : « …/update/KyberNow-Link-OS.img.xz : Permission denied ».
- **Correctif** : deux dossiers. Les images vont dans `/var/lib/kybernow/update`, créé au compte `kybernow` par `01-run-chroot.sh` ou le premier démarrage. L'état de root va dans un dossier `root 0700` à part (par exemple `/var/lib/kybernow/.os-state`).
- **Effort** : S · **Confiance** : vérifié

### LINAPP-03 — Une mise à jour A/B perd le Wi-Fi, les manettes, SSH, le mode HDMI et le nom
- **Catégorie** : bug critique
- **Où** : `F/kybernow-os-update:133-142` (ne réécrit que `root=` de `cmdline.txt` et `fstab`) ; `app/os/make-ab-image.sh:107-111` (seul `/var/lib/kybernow` est sur la partition de données, sans aucun montage bind) ; `F/kybernow-os-ctl:45`, `:90-99`, `:104`
- **Description** : tout ce que l'appareil règle hors de `/var/lib/kybernow` vit sur la racine de l'emplacement en cours :
  - les réseaux Wi-Fi (`/etc/NetworkManager/system-connections`) ;
  - les associations Bluetooth (`/var/lib/bluetooth`) ;
  - SSH allumé (`systemctl enable ssh`) ;
  - le mode HDMI (`video=` dans `cmdline.txt` de la partition de démarrage en cours) ;
  - le nom d'hôte.

  Le nouvel emplacement repart de l'image publiée. Les clés SSH, elles, sont bien gardées (`/var/lib/kybernow/ssh`), mais le service est éteint.
- **Scénario** : un Pi en Wi-Fi avec une DualSense en Bluetooth se met à jour. L'app arrive à l'accueil, `commit` valide l'essai, et l'appareil se retrouve hors ligne, sans manette, en HDMI auto. Comme LINAPP-01 empêche aujourd'hui toute mise à jour, ce défaut se révélera dès que LINAPP-01 sera corrigé.
- **Correctif** : des montages bind vers la partition de données dans le `fstab` de `make-ab-image.sh` (`system-connections`, `/var/lib/bluetooth`, et un fichier d'état pour SSH, HDMI et le nom), ou une copie de ces éléments vers la nouvelle racine dans `apply`. Reporter aussi `video=…` dans le nouveau `cmdline.txt`.
- **Effort** : M · **Confiance** : vérifié

### LINAPP-04 — « Prêter » et « Écran » ouverts depuis le menu de session ne répondent ni au clavier ni à la manette
- **Catégorie** : bug critique (très probablement la cause des « bugs de menus » signalés sous Linux)
- **Où** : `app/src/device/mod.rs:2238` (`in_game` ne teste que `menu`, `keys` et `share_panel`) ; `app/src/app.rs:6013-6018` (`MenuItem::Screen` : `menu = false`, `watch_panel = true`) ; `app/src/app.rs:6024-6027` (`MenuItem::Lend` : `menu = false` puis `lend_button`) ; la capture de la souris (`device/mod.rs:1732`) ignore aussi ces panneaux
- **Description** : à l'ouverture du panneau, le menu se ferme ; `in_game` redevient vrai, donc les touches et la manette partent au PC au lieu de `app.nav`. Le « B revient au menu » de `session_back` (`app.rs:4535`) est inatteignable.
- **Scénario** : Steam Deck, menu › « Prêter l'ordinateur… ». Le panneau s'affiche, mais A, B et les flèches pilotent le jeu. En souris des jeux, la souris reste capturée : on ne peut même pas cliquer le panneau.
- **Correctif** : une méthode unique `SessionUi::ui_owns_input()` (`menu || keys || share_panel || lend_panel || watch_panel || bar_menu.is_some()`), utilisée partout où `device/mod.rs` décide « jeu ou interface » (lignes ~1665, 1732, 1756, 2155, 2194, 2238) et dans `sync_keyboard_grab` (LINAPP-06).
- **Effort** : M · **Confiance** : vérifié

---

## Bugs

### LINAPP-05 — Menus déroulants de la barre : ni clavier ni Échap
- **Où** : `app/src/device/mod.rs:2238` (même `in_game`, qui ignore `bar_menu`) ; `app/src/app.rs:4508-4565` (`session_back`)
- **Description** : pendant qu'un menu Profil, Ordinateurs, Caméra… est ouvert, les flèches, Entrée et Échap partent au PC. Par `nav`, `session_back` ne ferme que Prêter et Partager ; pour les autres menus, il ouvre le menu de session en laissant `bar` et `bar_menu` ouverts, alors que `open_menu` (`app.rs:4568-4586`) les remet à zéro.
- **Scénario** : clic sur « Profil » : le menu s'ouvre, Échap part au jeu, seul un clic à côté le ferme.
- **Correctif** : LINAPP-04, plus un premier cas `if s.bar_menu.is_some() { s.bar_menu = None; return … }` dans `session_back`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-06 — Mode immersif : le clavier reste saisi pendant un menu ou un panneau
- **Où** : `app/src/device/mod.rs:1536-1545` (`sync_keyboard_grab` n'exclut que `bar_menu` et `share_overlay()`) ; règle dans `docs/link-ui.md` (immersif : « pas un menu, un panneau ou un champ de la barre »)
- **Scénario** : immersif « Toujours », menu de session ouvert. Alt+Tab est saisi par SDL mais n'est pas envoyé (on n'est pas en jeu) : on ne peut plus changer de fenêtre tant que le menu est ouvert.
- **Correctif** : `!s.ui_owns_input()` (LINAPP-04) dans la condition `want`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-07 — Un essai A/B raté est validé sans rien dire, puis présenté comme « Version précédente »
- **Où** : `F/kybernow-os-update:153` ; `app/src/device/mod.rs:1145-1148` ; `app/src/ui/device.rs:179`, `:226-227`
- **Description** : en shell, `A || B && C` se lit `(A || B) && C`. `B` (`booted_partition = CUR_BOOT`) est vrai à chaque démarrage normal, si bien que la condition se résume à « `pending` existe ». Après un essai qui échoue, le firmware revient à l'ancien emplacement. `pending` est resté sur la partition de données, commune aux deux. L'app appelle `commit` à l'accueil, qui efface `pending` et écrit « emplacement A inscrit par défaut ». La rubrique affiche ensuite la version ratée sous « Autre emplacement › Version précédente » et propose « Revenir à la X » vers elle.
- **Correctif** : ne valider que si `tryboot_flag = 1`. Sinon, écrire `failed=<version>` dans l'état, le rendre par `status`, l'afficher (« La mise à jour X n'a pas démarré ; retour à Y ») et ne pas proposer le retour vers cet emplacement.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-08 — Aucun chien de garde pendant un essai
- **Où** : `F/systemd/kybernow-link.service:25-26` (`Restart=always`, `RestartSec=2`) ; aucune occurrence de « watchdog » dans `apps/linux/os`
- **Description** : le retour arrière ne se fait qu'au prochain démarrage. Rien ne redémarre un système qui fige (noyau) ou dont l'app plante en boucle.
- **Scénario** : la nouvelle version plante au lancement : l'image Plymouth reste affichée pour toujours, jusqu'à ce qu'on débranche le Pi. INSTALLATION.md promet pourtant que l'appareil revient tout seul.
- **Correctif** : `RuntimeWatchdogSec=` (chien de garde matériel bcm2835) dans `system.conf.d`, et une unité qui redémarre si `tryboot=1` et `pending` sont encore là 3 à 5 minutes après le démarrage. Aussi `StartLimitIntervalSec=0` sur le service de l'app.
- **Effort** : M · **Confiance** : plausible

### LINAPP-09 — Le compte de l'app peut écrire n'importe quelle image système en root
- **Où** : `F/etc/sudoers-kybernow:2` (aucun argument imposé) ; `F/kybernow-os-update:5-7`, `:90-92` (« déjà vérifiée par l'app »)
- **Description** : tout ce qui contrôle le compte `kybernow` devient root de façon durable. Cela vaut pour du code exposé au réseau (session, relais) comme pour le compte SSH. Entre la vérification par l'app et la lecture par root, le fichier peut aussi être remplacé (course TOCTOU).
- **Correctif** : copier l'image dans le dossier root de LINAPP-02 et y vérifier le manifeste signé avec une clé publique placée dans le rootfs. Limiter sudoers à `apply` avec ce chemin seul.
- **Effort** : M · **Confiance** : vérifié

### LINAPP-10 — Taille de la racine variable d'une version à l'autre
- **Où** : `app/os/make-ab-image.sh:53-55` (`ROOT_MB = utilisé + 1024`, plafonné à 3 200, arrondi à 8) ; contrôle à `F/kybernow-os-update:103-104`
- **Scénario** : la carte a été flashée avec une version dont la racine fait 2 600 Mo ; la suivante en fait 2 616. `apply` refuse : « l'image ne tient pas dans l'emplacement B ». Il faut alors réinstaller la carte.
- **Correctif** : `ROOT_MB=3200` fixe (la carte de 8 Go le permet, d'après le commentaire), ou écrire le système de fichiers puis l'agrandir avec `resize2fs` à la place de `dd` brut.
- **Effort** : S · **Confiance** : plausible (le défaut disparaît si l'image atteint déjà le plafond)

### LINAPP-11 — Deux `apply` peuvent tourner en même temps
- **Où** : `F/kybernow-os-update` (aucun verrou) ; `app/src/device/os.rs:94-99` (un fil par appel)
- **Scénario** : un double appui sur « Installer » : deux `dd` écrivent la même partition.
- **Correctif** : `exec 9>/run/kybernow-os-update.lock; flock -n 9` en tête de script, et un drapeau `applying` testé avant l'envoi côté app.
- **Effort** : S · **Confiance** : plausible

### LINAPP-12 — Matrice de couleurs fausse pour une image BT.709 en plage pleine
- **Où** : `app/src/device/video.rs:874`
- **Description** : `full_range` mène à `SDL_YUV_CONVERSION_JPEG`, c'est-à-dire BT.601 en plage pleine. SDL2 n'a pas de mode « BT.709 plein ». PyroWave SDR est en BT.709 plage pleine (docs/pyrowave.md). De plus, le mode est global à SDL et n'est posé qu'à la création de la texture.
- **Scénario** : en PyroWave SDR, les rouges et les verts dérivent. Une bascule HEVC ↔ PyroWave à la même définition garde l'ancien mode. En multi-sessions, la dernière fenêtre créée impose sa matrice à toutes.
- **Correctif** : retenir `(bt709, full_range)` par écran et reposer le mode avant chaque `SDL_RenderCopy`. Pour BT.709 plein, ramener en plage réduite sur le fil du décodeur, ou passer par le chemin GL de `hdr_gl.c`, qui a ses propres matrices.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-13 — `EAGAIN` du décodeur : le paquet est perdu en silence
- **Où** : `app/src/native/decoder.c:327` (`if (r == AVERROR(EAGAIN)) return 0;`) ; `app/src/device/video.rs:557` (`gate.fed(pts)` comme si l'image était passée)
- **Scénario** : `h264_v4l2m2m` (Pi) a ses tampons d'entrée pleins. Une image de référence disparaît, la reprise sans image clé (`ResumeGate`) se fie à un état faux, et l'image reste abîmée jusqu'à la prochaine image clé.
- **Correctif** : sur `EAGAIN`, vider avec `avcodec_receive_frame` puis renvoyer le même paquet. Ou bien rendre un code dédié et ne pas appeler `fed`.
- **Effort** : S · **Confiance** : vérifié (code ; la fréquence sur le Pi reste à observer)

### LINAPP-14 — Stylet X11 : Xlib utilisé par deux fils sans `XInitThreads`
- **Où** : `app/src/native/pen.c:592-620` (`kn_pen_watch` : `XISelectEvents`, `XFlush` sur le fil principal) et `:446-531` (`x_thread` : `XPending`, `XNextEvent` sur la même connexion) ; aucun `XInitThreads` dans `src`
- **Scénario** : ouvrir une fenêtre d'écran ou de session de plus (`Win::new` → `pen::watch`, `device/mod.rs:323`) pendant que la tablette bouge : file d'événements X corrompue, voire plantage.
- **Correctif** : `XInitThreads` (par `dlsym`, comme le reste de libX11) avant `XOpenDisplay`, ou confier la sélection au fil X par le tube `stop_pipe` qui existe déjà.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-15 — File vers le décodeur sans borne
- **Où** : `app/src/device/video.rs:229` (`channel()`) ; le commentaire des l. 7-8 (« le retard ne s'accumule pas ») ne vaut qu'après le décodage
- **Scénario** : décodage par le processeur (repli du Pi, HEVC 4:4:4 en 4K) plus lent que le flux. Les paquets s'empilent, et la latence et la mémoire montent sans limite jusqu'à la fin de la session.
- **Correctif** : compter les paquets en attente. Au-delà d'un seuil, jeter jusqu'à la prochaine image clé et en demander une (le cœur sait le faire).
- **Effort** : M · **Confiance** : vérifié

### LINAPP-16 — États collants du fil de décodage
- **Où** : `app/src/device/video.rs:350-361`, `:482`, `:524-528`
- **Description** : `software` (repli sur le processeur) n'est jamais remis à faux, même quand le format change. `failed` n'est jamais remis à faux après une ouverture réussie.
- **Scénario** : un repli sur le processeur en H.264 suivi d'un passage en HEVC : le HEVC reste décodé par le processeur. Un second échec d'ouverture n'est jamais signalé : écran noir sans message.
- **Correctif** : remettre les deux à faux sur `Msg::Codec`, et `failed` aussi après un `open()` réussi.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-17 — Pi : HEVC matériel annoncé même si cette compilation ne l'a pas
- **Où** : `app/src/device/system.rs:197-201` (`hw_hevc` ne dépend que de la présence de `rpivid`) ; `app/src/native/decoder.c:430-439` (`kn_v4l2_request_built`, `kn_v4l2_m2m_built`, jamais appelées : voir LINAPP-47)
- **Scénario** : `.deb` construit sans `--rpi` installé sur un Pi : « Auto » et les capacités annoncées comptent un HEVC matériel. Chaque ouverture échoue, puis se replie sur le processeur.
- **Correctif** : `p.v4l2_hevc && kn_v4l2_request_built()`, et la même chose pour le H.264 avec `kn_v4l2_m2m_built()`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-18 — Webcam : pas de ligne, marge JPEG, allocations
- **Où** : `app/src/native/camera.c:397-402` (pas de ligne supposé `width*2` ou `width`, `bytesperline` ignoré) ; `:404-407` (paquet MJPEG pointé dans le tampon `mmap`, sans les `AV_INPUT_BUFFER_PADDING_SIZE` octets que FFmpeg exige) ; `:443-446` et `:221` (`malloc` et `calloc` non vérifiés)
- **Scénario** : un pilote qui aligne ses lignes (caméra CSI du Pi, ISP) donne une image cisaillée. En MJPEG, FFmpeg peut lire au-delà du tampon.
- **Correctif** : garder `bytesperline`, recopier le JPEG dans un tampon complété de la marge, vérifier les allocations.
- **Effort** : S · **Confiance** : vérifié (code ; l'effet dépend du pilote)

### LINAPP-19 — Barre de session et menus déroulants qui sortent de la fenêtre
- **Où** : `app/src/ui/session.rs:539-636` (`BarLayout::measure`) et `:639` (`bar_frame`) ; `:1073-1103` (`dropdown_titled` : `x` et la hauteur ne sont jamais bornés, contrairement à `computers_menu`, l. 1037, et `share::dropdown`) ; `:1095` (libellé sans `text_fit` ; largeur d'aide `r.w - lw - 50` qui peut devenir négative)
- **Description** : les fenêtres de « plusieurs ordinateurs » descendent à 480 × 270 (`device/mod.rs:570`). Avec toutes les icônes, la barre dépasse 750 pt : `x = (w - total)/2` devient négatif. Les menus Caméra (300 pt), Touches (280) et Profil (420) sortent à droite dès 960 pt.
- **Scénario** : une seconde fenêtre d'ordinateur, étroite : « Se déconnecter » est hors de la fenêtre. Le nom d'une webcam longue déborde de son menu.
- **Correctif** : borner `x` à `[8, cv.w - w - 8]` et la hauteur à `cv.h - y - 8` (avec défilement) ; `text_fit` sur les libellés ; au-dessous d'une largeur donnée, regrouper des icônes dans « … ». Ajouter une taille de 480 × 270 aux captures de `preview.rs` (`sizes()`).
- **Effort** : M · **Confiance** : vérifié (code ; pas reproduit à l'écran)

### LINAPP-20 — Focus triple sur « Partager »
- **Où** : `app/src/ui/session.rs:879`, `:890` (les pastilles « n invités » et « n demandes » reçoivent `Target::BarShare`, comme l'icône de la l. 472)
- **Scénario** : au clavier, l'anneau du focus s'allume sur trois éléments. `move_focus` (`app.rs:4339`) part du premier rectangle trouvé : les pastilles ne sont jamais un arrêt.
- **Correctif** : des cibles distinctes (`BarSharePill`, `BarShareRequests`) qui mènent à la même action.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-21 — Réglages : défilement sans fin, focus hors de l'écran, clic sous la barre de titre
- **Où** : `app/src/app.rs:4115` (`scroll` borné à 0 seulement) ; `app/src/ui/settings.rs:183-186` (toutes les lignes inscrivent leur zone de clic, même passées sous la barre de titre) ; `app/src/device/mod.rs:1610` (`update_hit_areas`)
- **Scénario** : à la molette, la page finit vide. Au clavier, le focus descend sous le bord sans faire défiler. Un clic dans le vide de la barre de titre (translucide) change un réglage invisible, au lieu de déplacer la fenêtre.
- **Correctif** : borner `scroll` à `contenu - visible` (hauteur rendue par le dessin), faire défiler jusqu'au focus, ne pas inscrire les rectangles au-dessus de `top`.
- **Effort** : M · **Confiance** : vérifié

### LINAPP-22 — Lien `kybernow://` pendant le test Auto ou « Hors de chez vous ? »
- **Où** : `app/src/app.rs:3572` (`ready` ne teste ni `autotest`, ni `welcome.away`, ni `station_notice`) ; `nav` intercepte tout tant que `autotest` (`app.rs:4176`) ou `welcome.away` (`welcome.rs:269`) existent, alors que ces panneaux ne se dessinent que sur Accueil et Réglages (`ui/mod.rs:146-151`)
- **Scénario** : un lien d'invitation arrive pendant le test Auto : l'écran Rejoindre s'ouvre, mais les caractères tapés pour le prénom sont avalés sans aucun panneau visible.
- **Correctif** : ajouter ces trois conditions à `ready`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-23 — Nom du décodeur faux dans les statistiques
- **Où** : `app/src/native/decoder.c:306-313` (appelé une seule fois, juste après l'ouverture, `video.rs:488`)
- **Description** : `pix_fmt` vaut encore `NONE` au moment de l'appel : le renommage en « CPU » ne se fait jamais. Le `memcpy(d->name, "CPU", 3)` donnerait d'ailleurs « CPUHEVC » sur « Pi HEVC ».
- **Scénario** : du HEVC 4:4:4 sur une carte VA-API sans 4:4:4 est décodé par le processeur, mais les statistiques affichent « VA-API HEVC ».
- **Correctif** : relire le nom après la première image et rappeler `set_codec` ; construire le nom avec `snprintf`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-24 — Stylet X11 : événements reçus en double
- **Où** : `app/src/native/pen.c:408` (`deviceid = XIAllDevices`)
- **Description** : XI2 livre l'événement de l'appareil maître et celui de l'esclave ; le filtre sur `sourceid` laisse passer les deux.
- **Correctif** : ignorer `de->deviceid != de->sourceid`.
- **Effort** : S · **Confiance** : plausible

### LINAPP-25 — Manette ouverte jamais refermée
- **Où** : `app/src/device/pads.rs:85-95` (`return None` si l'instance est déjà connue, `?` s'il n'y a plus de place, sans `SDL_GameControllerClose`)
- **Scénario** : une 5e manette, ou un ajout reçu deux fois : la référence SDL reste ouverte jusqu'à la fin de l'app.
- **Correctif** : chercher la place et l'instance avant d'ouvrir, ou fermer sur chaque sortie.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-26 — `relative_day` faux au changement d'année
- **Où** : `app/src/app_time.rs:67` (`tm_year * 400 + tm_yday`)
- **Scénario** : le 1er janvier, une session de la veille donne 400 − 364 = 36 au lieu de 1. La carte de l'ordinateur (`ui/station.rs:42`) affiche « 31/12 » au lieu de « hier ».
- **Correctif** : comparer la date locale de `now - 86400` (année, mois, jour) à celle de `at`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-27 — `build.rs` ne voit pas PyroWave arriver dans le préfixe
- **Où** : `app/build.rs:89-94` (choix entre `pyrowave.c` et `pyrowave_none.c` selon `include/pyrowave/pyrowave.h`, sans `rerun-if-changed` sur ce fichier ; seuls `src/native`, `PKG_CONFIG_PATH`, `KYBERNOW_PREFIX` et `KYBERNOW_V4L2_REQUEST` sont suivis)
- **Scénario** : PyroWave est posé dans le préfixe après une première compilation : `pyrowave_none.c` reste, et l'app dit PyroWave « indisponible » sans raison visible. Même chose pour un FFmpeg reconstruit.
- **Correctif** : `cargo:rerun-if-changed=` sur l'en-tête de PyroWave et sur `lib/libavcodec.a` du préfixe.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-28 — Variables de sources qui peuvent effacer un clone existant
- **Où** : `app/scripts/build.sh:218-221` (`src="${PYROWAVE_SRC:-…}"` puis `rm -rf "$src"` sans marqueur `.kybernow-<commit>`) ; `:156-159` (même logique pour `FFMPEG_SRC` sans `configure`)
- **Scénario** : un développeur pointe `PYROWAVE_SRC` vers son clone de travail : le script le supprime.
- **Correctif** : ne jamais supprimer un dossier donné par variable ; s'arrêter avec un message.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-29 — Bluetooth : réassociation silencieuse
- **Où** : `F/etc/bluetooth-kybernow.conf:5` (`JustWorksRepairing = always`)
- **Description** : n'importe quel appareil qui se présente avec l'adresse d'une manette connue se réassocie sans confirmation et envoie des entrées. En session, ces entrées pilotent le PC.
- **Correctif** : `confirm`, avec un agent posé par l'app uniquement sur l'écran Manette, comme pour la première association.
- **Effort** : S · **Confiance** : plausible

### LINAPP-30 — Image Link OS figée sur la France
- **Où** : `app/os/build-image.sh:80-84` (`fr_FR`, clavier `fr`, `Europe/Paris`, `WPA_COUNTRY=FR`) ; aucun `timedatectl` ni réglage du domaine radio dans `app/src`
- **Scénario** : hors de France, les canaux Wi-Fi et la puissance suivent les règles françaises, l'heure des journaux et le « hier » de LINAPP-26 sont faux, et le clavier USB est en AZERTY. Le premier démarrage ne choisit que la langue de l'app.
- **Correctif** : au premier démarrage, déduire fuseau et pays de la langue choisie, ou les proposer, puis `timedatectl` et `iw reg set` par `kybernow-os-ctl`.
- **Effort** : S à M · **Confiance** : vérifié

### LINAPP-31 — Débit de la webcam réglé en route sans effet
- **Où** : `app/src/native/camera.c:365-372` (`kn_cam_set_bitrate` chaque seconde) ; `app/src/device/camera.rs` (`run`) ; docs/webcam.md (« le débit ne change pas en route »)
- **Description** : `h264_vaapi` et `h264_v4l2m2m` ne relisent pas `bit_rate` après l'ouverture. Le commentaire du C dit le contraire de la doc.
- **Correctif** : recréer l'encodeur sur une forte baisse, ou au moins aligner le commentaire sur la doc.
- **Effort** : M · **Confiance** : plausible

---

## Optimisations

### LINAPP-32 — 4 à 5 copies par image, même pour les images jetées
- **Où** : `app/src/device/video.rs:250` (`data.to_vec()`), `:547` (concaténation avec les paramètres), `app/src/native/decoder.c:322-324` (`memcpy` dans `av_new_packet`), `:334-367` (`av_hwframe_transfer_data` pour **chaque** image prête alors que seule la dernière est montrée), `video.rs:617-663` (recopie dans `Frame`)
- **Description** : en 4K60 NV12, la recopie dans `Frame` représente à elle seule environ 720 Mo/s. Le commentaire de `decoder.c:8-11` ne juge la copie « négligeable » qu'à la taille du Steam Deck.
- **Correctif** : ne transférer que la dernière trame matérielle (S) ; passer une référence `AVFrame` jusqu'à la texture (M) ; à terme, VA-API sans copie (`vaExportSurfaceHandle` → DMA-BUF → `EGL_EXT_image_dma_buf_import`, sur le modèle de `hdr_gl.c` ; la voie est décrite dans docs/pyrowave-client-pi.md) (L).
- **Effort** : S → L · **Confiance** : vérifié

### LINAPP-33 — Calculs lourds sur le fil principal
- **Où** : `app/src/device/video.rs:851-854`, `:947-991` (`eight_bit` : nouveaux tampons à chaque image, `get` avec contrôle de bornes par pixel) ; `app/src/device/mod.rs:1345-1350` (`video::snapshot` toutes les 2 s, PQ en flottants par pixel en HDR)
- **Scénario** : sous-surface HDR en échec en 4K : des dizaines de millisecondes par image sur le fil de l'interface. Toutes les 2 s, un à-coup visible sur le Pi.
- **Correctif** : faire ces deux calculs sur le fil du décodeur, avec des tampons réutilisés et des boucles par ligne.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-34 — Fils créés à chaque image en 4:4:4
- **Où** : `app/src/native/decoder.c:136-150` (`pthread_create` et `pthread_join` par image ; PyroWave 4:4:4 y passe aussi, `pyrowave.c:302`)
- **Correctif** : un groupe de 3 fils créé à l'ouverture.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-35 — Interface : allocations et redessins inutiles
- **Où** :
  - `app/src/ui/canvas.rs:431-434` : `g.coverage.clone()` pour chaque caractère dessiné. `fonts` et `pix` sont des champs distincts : les deux emprunts peuvent coexister.
  - `app/src/device/mod.rs:1058-1082` : Ajouter, Rejoindre, Manettes, Compte et Accueil en trois écrans sont redessinés toutes les 16 ms, même immobiles. `image_cover_rounded` (`canvas.rs:534`) et `draw_masked` (`:294`) allouent alors une image plein écran par image.
  - `app/src/ui/session.rs:644-670` : `BarLayout::measure` est appelé à chaque mouvement de souris (une vingtaine de `text_width` et une copie de `stats`).
  - `canvas.rs:445` : `text_fit` est en O(n²).
  - `canvas.rs:89` : le cache de glyphes se vide en entier à 6 000 entrées.
- **Correctif** : emprunter le glyphe sans le copier ; garder en cache le fond mis à l'échelle et n'animer que ce qui bouge (ou à 30 i/s) ; garder la mesure de la barre en cache par contenu ; mesurer par dichotomie ; un cache LRU.
- **Effort** : M · **Confiance** : vérifié (coût non mesuré, surtout sensible sur le Pi)

### LINAPP-36 — Réglages et fenêtres : travail refait à chaque événement
- **Où** : `app/src/app.rs:3819`, `:7063` (`focus_adjustable` et `adjust` reconstruisent les lignes de toutes les rubriques, deux fois par appui) ; `app/src/device/mod.rs:1406` (`pads.infos()` par fenêtre redessinée), `:1414` (`hits.clone()` à chaque dessin)
- **Correctif** : ne construire que la rubrique ouverte ; calculer `pads` une fois par tour ; `std::mem::take` pour `hits`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-37 — `apply` décompresse l'image trois fois, par blocs de 512 octets
- **Où** : `F/kybernow-os-update:94`, `:114`/`:128`, `:131`
- **Description** : environ 6 millions d'appels `read`/`write` pour 3 Go, plus un `fsync` final. Sur une carte SD, cela compte en minutes.
- **Correctif** : `bs=4M iflag=skip_bytes,count_bytes,fullblock`, et si possible une seule passe de `xz` (par exemple avec `tee` vers deux `dd`, ou une partition après l'autre dans le même flux). À faire en même temps que LINAPP-01.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-38 — Constructions sans cache utile
- **Où** :
  - `app/os/build-image.sh:55-58`, `:94` : clone `--depth 1` de pi-gen jamais rafraîchi ni épinglé. Avec `scripts/release.sh:588` (empreinte du `.deb`), debootstrap et les stages 0 à 2 sont rejoués à chaque version.
  - `app/flatpak/fr.leslignesclaires.kybernow.yml:37-45` et `app/scripts/sources.sh:22-25` : le module `kybernow-deps` dépend de `build.sh` et de tout le kysdk, donc FFmpeg et PyroWave sont refaits au moindre changement.
  - `yml:29` : `CARGO_HOME=/run/build/…` retélécharge toutes les crates à chaque construction.
- **Correctif** : épingler un commit de pi-gen, réutiliser le rootfs des stages 0 à 2 (`SKIP`/`CONTINUE`) tant qu'il ne change pas ; séparer FFmpeg et PyroWave de kynput en deux modules ; préparer les crates à l'avance (`cargo vendor`).
- **Effort** : M · **Confiance** : vérifié

### LINAPP-39 — L'aperçu compilé dans le binaire livré
- **Où** : `app/src/main.rs:17` (`mod preview;` sans `cfg`), parce que `--splash` y est rangé ; `ui::home_main_label` (`app/src/ui/mod.rs:47`) n'est appelé que par `preview.rs` et double `home::main_label` (`home.rs:174`)
- **Correctif** : sortir `splash` de `preview.rs` et compiler l'aperçu seulement sans la fonctionnalité `device` (ou dans un binaire à part).
- **Effort** : S · **Confiance** : vérifié

---

## Factorisation

### LINAPP-40 — Les réglages synchronisés réécrits à côté du cœur
- **Où** : `app/src/synced.rs:50-270` (`SyncKeys::merge`, `remove_sent`, `apply`, `snap_mbps`) contre `apps/core/src/account/settings.rs` (`SyncedValues::merge_into`, `apply_to`). Le commentaire de `synced.rs:240` reconnaît le parallèle. L'app Windows, elle, se sert des types du cœur (`apps/windows/src/synced.rs`).
- **Risque** : une 7e clé synchronisée ajoutée au cœur sera oubliée ici sans aucun signal. Le test `json_comme_le_coeur` ne couvre que la sérialisation.
- **Correctif** : isoler la partie données du cœur dans un fichier sans dépendance et l'inclure par `#[path]`, comme `welcome_rules` (`main.rs:26`) ; au minimum, un test (avec la fonctionnalité `device`) qui compare les deux implémentations sur les mêmes cas.
- **Effort** : L · **Confiance** : vérifié

### LINAPP-41 — `PsSettings` recopie `PlaySettings` du cœur
- **Où** : `app/src/model.rs:505-552` contre `apps/core/src/playstation/settings.rs:84-160` (mêmes défauts, même règle « personnalisé »)
- **Correctif** : même approche que LINAPP-40. Il faut d'abord séparer la partie données, car le fichier du cœur importe `super::stream::Profile`.
- **Effort** : M · **Confiance** : vérifié

### LINAPP-42 — Fonctions identiques dans les apps Linux et webOS
- **Où** :
  - identiques à la ligne près (vérifié par `diff`) : `join_relays`, `relay_failure_text`, `role_of` (`app/src/device/session.rs:612-651`) et `win_scancode` (`app/src/device/pads.rs:71-76`), avec leurs copies dans `apps/webos/src/device/` ;
  - presque identique : `app/src/autotest.rs` (types `Step`, `State`, `Choice` et textes) et `apps/webos/src/autotest.rs` ;
  - `app_time::date_label` et `weekday_label` existent aussi dans webOS ;
  - le calcul du triplet `hw` est fait trois fois dans l'app (`session.rs:156`, `:892`, `video.rs:743`).
- **Correctif** : `join_relays`, `relay_failure_text` et `win_scancode` dans `kybernow_core` (elles ne dépendent que de ses types) ; une petite crate commune « règles d'interface » sans SDL pour le test Auto et les dates ; `Machine::hw_decoders()`.
- **Effort** : M · **Confiance** : vérifié

### LINAPP-43 — Application des correctifs Kyber en quatre copies
- **Où** : `app/scripts/build.sh:92-118` (mode sans git : `patch -R --dry-run` puis `-F0`), `scripts/kyber-client-patches.sh:22-49`, `scripts/release.sh:374-380`, `app/scripts/sources.sh:42-46`
- **Description** : le mode sans git, utilisé dans le Flatpak (sources copiées par rsync), n'a pas la remise à l'origine du script commun. Lancé à la main sans `KYBERNOW_KYBER_HEAD=1` (seul release.sh le pose), il peut retrouver l'échec « correctif inapplicable » que le script commun a corrigé le 25/11.
- **Correctif** : copier `kyber-client-patches.sh` avec les sources, lui ajouter un mode « archive d'origine » sans git, et mettre `KYBER_HEAD=1` par défaut dans `flatpak.sh`.
- **Effort** : M · **Confiance** : vérifié (duplication) ; l'échec dépend de l'état de `third_party`

### LINAPP-44 — Commit PyroWave en dur
- **Où** : `app/scripts/build.sh:215` (`PYROWAVE_COMMIT=a32b05e…`), répété dans `server/kyber-patches/pyrowave.sh:15` ; le commentaire renvoie à `apps/macos/PyroWave/COMMIT`, que l'app Windows lit bien (`apps/windows/scripts/build-pyrowave.sh:32`)
- **Correctif** : lire `COMMIT` (et le copier dans `sources.sh` pour le Flatpak).
- **Effort** : S · **Confiance** : vérifié

### LINAPP-45 — Petits widgets recopiés dans `ui/`
- **Où** : `spinner` (`account.rs:64`, `ps.rs:142`, `offer.rs:22`), `ghost_w` (`offer.rs:34`, `settings.rs:354`, `welcome.rs:78`, alors que `widgets::ghost_width` existe), `offer::primary_w` (`offer.rs:30`, alors que `widgets::primary_width` existe), `ease` (`session.rs:127`, `launch.rs:41`), et les trois capsules presque identiques (`ui/mod.rs:216` `toast`, `session.rs:1106` `capsule` et `capsule_icon`), toutes sans largeur maximale
- **Correctif** : tout ranger dans `widgets.rs`, avec une seule capsule bornée à `cv.w - 32` par `text_fit`.
- **Effort** : S · **Confiance** : vérifié

L'app Windows est en Svelte et Tauri (`apps/windows/ui`) : aucun widget n'est partageable avec l'interface tiny-skia de Linux. La teinte de la signature (`hue()`, `ui/mod.rs:259`) et les libellés sont réécrits de part et d'autre, ce qui est assumé.

---

## Code mort

Chaque symbole a été cherché dans tout le dépôt, hors `third_party` et `target`.

### LINAPP-46 — Fonctions de dessin et icônes inutilisées
- **Où** : `Canvas::vgradient` (`app/src/ui/canvas.rs:251`), `Canvas::image_height` (`:553`), `Canvas::image` (`:562`) ; `Icon::Play`, `Pointer`, `Image`, `Logout`, `Info`, `Dots`, `Sound`, `Cpu`, `Arrow` : chacune n'apparaît que dans sa propre définition (`icons.rs:72…`), masquée par `#[allow(dead_code)]` (`icons.rs:7`). `theme.rs` porte `#![allow(dead_code)]` alors que toutes ses couleurs servent. `main.rs:4` (`cfg_attr(not(feature = "device"), allow(dead_code))`) masque le code mort dans un `cargo check` sans la fonctionnalité.
- **Correctif** : supprimer, retirer les `allow`, et regarder les avertissements de la compilation avec `device`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-47 — Trois fonctions C jamais appelées
- **Où** : `kn_decoder_flush` (`app/src/native/decoder.c:415`), `kn_v4l2_request_built` (`:430`), `kn_v4l2_m2m_built` (`:439`) ; le bit « HEVC Main 10 » du masque (`:481`) n'est jamais lu
- **Correctif** : brancher les deux sondes (LINAPP-17), sinon supprimer les trois fonctions.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-48 — Rust : fonctions de l'interface sans appelant
- **Où** : `pyrowave::lowered_label` (`app/src/pyrowave.rs:152`, aucun appel, même en test) ; `PsSettings::height` et `bitrate` (`app/src/model.rs:533-552`, appelées par les seuls tests de `ps.rs:1447-1449`, sous un `#[allow(dead_code)]` posé sur tout l'`impl`, l. 527)
- **Correctif** : supprimer `lowered_label` ; passer les deux autres en `#[cfg(test)]` et retirer l'`allow`.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-49 — Sous-commandes et options d'OS inutilisées
- **Où** : `kybernow-os-ctl cec-wake` (`F/kybernow-os-ctl:87-89`, copie exacte de `standby off`) et `hostname` (`:101-105`) : l'app n'appelle que `ssh`, `reboot`, `poweroff`, `standby`, `cec-name`, `cec-off` et `hdmi`. `x-systemd.growfs` (`app/os/make-ab-image.sh:110`) refait ce que fait `kybernow-os-firstboot`.
- **Correctif** : supprimer, ou brancher `hostname` (utile pour LINAPP-03).
- **Effort** : S · **Confiance** : vérifié

### LINAPP-50 — Dépendance directe `kynput-sys` sans usage
- **Où** : `app/Cargo.toml:15`, `:33` ; aucun `kynput_sys` dans `app/src` (l'app passe par `kybernow_core::kynput_rs`, et le cœur dépend déjà de `kynput-sys`)
- **Correctif** : la retirer de la liste et de la fonctionnalité `device`, puis vérifier l'édition de liens.
- **Effort** : S · **Confiance** : plausible (on ne peut pas exclure un besoin d'édition de liens sans compiler)

---

## Doc et cohérence

### LINAPP-51 — PyroWave actif par défaut, la doc dit « sur demande »
- **Où** : `app/scripts/build.sh:49` (`KYBER_PYROWAVE="${KYBER_PYROWAVE:-1}"`), `app/scripts/flatpak.sh:16` (`KP=" --kyber-pyrowave"` d'office) ; « sur demande » dans `build.sh:16-21`, `deb.sh:7-8`, `flatpak.sh:8`, docs/pyrowave-client-pi.md et os/README.md ; `--sans-pyrowave` n'apparaît dans aucune ligne d'usage
- **Correctif** : aligner la doc sur le comportement réel (PyroWave par défaut, `--sans-pyrowave` pour s'en passer).
- **Effort** : S · **Confiance** : vérifié

### LINAPP-52 — La doc de Link OS promet plus que le code
- **Où** : `app/INSTALLATION.md:68` et `app/os/README.md:36`
- **Description** : la doc annonce que l'appareil « revient tout seul » (faux si le système fige : LINAPP-08), que « les réglages restent » (faux : LINAPP-03) et que `autoboot.txt` n'est « jamais à moitié écrit » grâce au renommage. Sur FAT, un renommage n'est pas atomique en cas de coupure de courant.
- **Correctif** : corriger la doc en même temps que les correctifs (règle « doc avec chaque évolution »), en FR et en EN.
- **Effort** : S · **Confiance** : vérifié

### LINAPP-53 — Écarts avec `docs/link-ui.md`
- **Où** : `app/src/ui/session.rs:438-487`, `:735-749` ; `app/src/app.rs:718`
- **Description** :
  - pas d'infobulle « Profil : Jeu, Bureau ou Max » ;
  - « Plein écran » et « Se déconnecter » sans leur raccourci ;
  - la souris est une bascule, pas le menu « Auto / Souris du bureau / Souris des jeux » ;
  - pas de pastille « Presse-papiers » ;
  - le menu Touches a 10 entrées contre 4 dans la doc ;
  - traductions anglaises divergentes pour un même texte : « Par le relais » donne “Through the relay” (`ui/ps.rs:635`, `:668`, `:730`, `ui/autotest.rs:133`) mais “Via the relay” ailleurs et dans link-ui ; aussi « Retour à l'accueil » (“Back home” / “Back to home”), « Tester les manettes » et « Choisir » ;
  - `session.rs:460` traduit par `.replace("Maj", "Shift")`.
- **Correctif** : trancher dans link-ui d'abord (règle « cohérence, sans rapiécer »), puis aligner. Les textes répétés deviennent des constantes `(fr, en)` comme `lend.rs:31`.
- **Effort** : M · **Confiance** : vérifié

### LINAPP-54 — Commentaires périmés et petits textes faux
- **Où** :
  - `app/src/native/pyrowave_none.c:1-2` : « Raspberry Pi, build.sh --rpi : pas de bibliothèque PyroWave », faux depuis que PyroWave se construit aussi avec `--rpi` (docs/pyrowave-client-pi.md).
  - `app/src/ui/lend.rs:31-34` : « Votre ami joue dans votre session Windows » s'affiche aussi face à une Station Mac ou Linux.
  - `app/os/build-image.sh:44` : le `sed` ne rend rien, et le message affiche « apt install ) ».
  - `app/tests/pyrowave/samples-mac.sh:19-21` : les erreurs de `swiftc` sont avalées (`| grep error: || true`).
- **Correctif** : corriger ; pour le prêt, une variante selon le système de la Station, écrite d'abord dans link-ui.
- **Effort** : S · **Confiance** : vérifié (lend : la doc est ambiguë)

---

## Les 10 priorités

1. **LINAPP-01 + LINAPP-02** : rendre la mise à jour de Link OS possible (pipefail, dossier root à part). S chacun, et rien d'autre de Link OS ne se vérifie sans eux.
2. **LINAPP-03** : garder Wi-Fi, Bluetooth, SSH, HDMI et nom à travers une mise à jour, avant la première vraie mise à jour d'un appareil.
3. **LINAPP-04 + 05 + 06** : une seule règle « l'interface a la main » (`ui_owns_input`) pour les entrées, la capture de la souris et le clavier immersif. C'est la cause probable des bugs de menus.
4. **LINAPP-07 + LINAPP-08** : un essai raté se voit et se rattrape seul (commit sur tryboot seulement, chien de garde).
5. **LINAPP-09** : vérifier l'image côté root et restreindre sudoers.
6. **LINAPP-12** : matrice de couleurs juste en PyroWave SDR et par fenêtre.
7. **LINAPP-13 + LINAPP-15 + LINAPP-16** : chemin de décodage robuste (EAGAIN, file bornée, états remis à zéro), surtout sur le Pi.
8. **LINAPP-14** : `XInitThreads` pour le stylet X11.
9. **LINAPP-19 + LINAPP-21** : barre, menus et réglages qui restent dans la fenêtre ; ajouter 480 × 270 aux captures.
10. **LINAPP-40** : brancher les réglages synchronisés sur la logique du cœur, avant qu'une nouvelle clé ne diverge.

Ensuite : LINAPP-32 (copies d'images, puis VA-API sans copie), LINAPP-10, LINAPP-17 et le code mort (LINAPP-46 à 50), qui se règlent vite.

## Ce qui est sain

- **Aucun TODO ni FIXME** dans `apps/linux` (src, os, scripts, build.rs).
- **Pas de découpe UTF-8 fautive** : aucune découpe d'octets `&s[..n]` sur les textes de l'interface ; `text_fit` raccourcit par `String::pop()`.
- **Divisions protégées** dans les courbes de latence et de débit.
- **Pas d'`unwrap` sur des données réseau** dans `src/device` : ce qui vient du cœur ou du réseau est filtré.
- **Disposition A/B bien pensée sur le papier** : `autoboot.txt` pointé sur l'emplacement en cours avant toute écriture, emplacement libre marqué incomplet, p1 réécrite fichier par fichier quand on tourne sur B, PARTUUID fixes (`DISK_ID` constant dans `make-ab-image.sh`), taille de l'image contrôlée avant l'écriture. Il manque les correctifs ci-dessus, pas l'architecture.
- **Scripts shell** : `set -euo pipefail` partout dans les scripts de construction, chemins entre guillemets, `trap` de nettoyage dans `deb.sh` et `make-ab-image.sh`. `kybernow-os-ctl hostname` valide son argument.
- **Wayland** : `wayland_hdr.c` libère ses objets ; le choix entre XInput2 et `tablet-v2` pour le stylet, avec le rejeu en souris sous Wayland, suit docs/stylet.md (le piège Wayland connu est bien traité, à part LINAPP-14 et 24 côté X11).
- **Règles partagées sans recopie quand c'est possible** : `welcome_rules` inclus tel quel depuis le cœur (`main.rs:26`), modèle à suivre pour LINAPP-40 et 41.
- **Traductions** : paires `tr("fr", "en")` sur place, comme le prévoit docs/i18n.md pour Linux ; la langue est transmise au cœur.
- **Sources publiées seules** : `apps/linux/dist` est ignoré par git.
