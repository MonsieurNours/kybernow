# KyberNow Link pour les téléviseurs LG (webOS)

[English](README.en.md) · Français

KyberNow Link sur les téléviseurs LG : une app **native** (un binaire ARM, pas une page web), sur le même cœur Rust que les autres apps Link (`apps/core`), qui retrouve l'ordinateur (KyberNow Station) en plein écran, à la Magic Remote, au clavier et à la souris ou à la manette. Identité v2 ([charte](../../brand/v2/README.md)) ; maquettes : [Claude Design](https://claude.ai/artifact/2TNZZrYYZpHvdQqLXzSLGv) (accueil, menu de session à la Magic Remote, réglages Image, Réseau et Son, manettes et vibrations). Installation sur le téléviseur : [INSTALLATION.md](INSTALLATION.md) (mode développeur).

**État (23/10/2026)** : installée sur la G2 en mode développeur, l'accueil s'affiche (ordinateurs trouvés, manette, réseau) ; session pas encore essayée. Le binaire se construit pour webOS (armv7, glibc 2.12), le paquet `.ipk` aussi, et l'interface est vérifiée par ses captures (sur l'ordinateur, et par le binaire ARM sous émulation). Téléviseur d'essai : LG OLED G2 83 pouces (2022). La décision est dans [PROJECT.md](../../PROJECT.md) (« Téléviseurs LG (webOS) »).

![Accueil](screenshots/1-accueil.png)

## Téléviseurs

- **webOS 5 au moins** (téléviseurs LG de 2020 et après) : le décodeur NDL DirectMedia en API 2, celle que l'app utilise. Visés d'abord : webOS 22 et après (OLED C2, G2 et suivants), dont les mises à jour webOS Re:New (webOS 24 et 25 sur les modèles 2022).
- **Installation** : en mode développeur (compte LG gratuit, sans root), ou par Homebrew Channel. Le LG Content Store n'est pas visé : il ne publie pas d'app native hors partenaires.
- **Matériel d'essai** : LG OLED G2 83 pouces ; manettes DualSense, Xbox, Shield, Switch Pro.

## Ce que fait l'app

- **Accueil** (`src/ui/home.rs`) : l'ordinateur choisi en plein cadre (l'aperçu d'Admin, assombri hors ligne), son état (sondage d'Admin toutes les 8 s), système, carte graphique et adresse, le mode de l'écran virtuel ; « Se connecter », les profils Jeu, Bureau, Max et « Réglages de l'ordinateur » ; « Réglages » (réglages globaux) en haut à droite, même sans ordinateur ajouté ; tout s'atteint à la croix, sans touche particulière (télécommande tierce, manette) — la touche « … » (jaune ou bleue, Y de la manette) reste un raccourci vers les réglages de l'ordinateur ; colonne « Prêt à jouer » (téléviseur et dalle, réseau et plafond de débit, manettes et leurs vibrations, dernière session) ; les autres ordinateurs, ceux trouvés sur le réseau, et l'ajout. Hors ligne avec une adresse MAC connue : « Réveiller l'ordinateur » (Wake-on-LAN).
- **Ajout** (`src/ui/pair.rs`) : code à six chiffres présenté à chaque Station trouvée sur le réseau (mDNS `_kybernow._tcp`, TXT `name` et `fp`) et accepté dans Admin › Appareils Link (compte Kyber propre au téléviseur) ; ou adresse et port d'Admin au pavé (ou au clavier), puis les quatre premiers groupes de l'empreinte à comparer avec ceux d'Admin, puis le code.
- **Session** (`src/ui/session.rs`, `src/device/`) :
  - « Vous entrez dans… » avec l'étape en cours et Annuler, la couture qui s'allume.
  - **Image** sur le plan vidéo du téléviseur ; la fenêtre de l'app est transparente par-dessus, et ne montre que le menu, les capsules et le pointeur de l'ordinateur.
  - **Menu** par le bouton Retour de la Magic Remote (réglable), l'appui long sur Retour, le bouton rouge, ou L1 + R1 + Start + Select : santé du flux en haut (latence, images par seconde, débit, mode, format, pertes), puis Reprendre, les autres profils, Magic Remote en souris, Manette en souris, Touches (Ctrl + Alt + Suppr, Alt + Tab, touche Windows…), Mesures, Manettes, Réglages et Se déconnecter. Pendant le menu, les manettes pilotent l'app et l'ordinateur relâche tout. Rappel du mode jeu de la télé (voir plus bas), désactivable.
  - **États** en capsule : reconnexion, image relancée, mode de l'ordinateur changé, image allégée, manette branchée.
  - **Fin** : retour à l'accueil, la latence médiane et la durée de la session gardées pour l'accueil ; une erreur s'affiche avec « Retour à l'accueil ».
- **Manettes** (`src/device/pads.rs`, `src/ui/pads.rs`) : quatre joueurs au plus dans l'ordre d'arrivée, annoncés à l'ordinateur à chaque démarrage de l'image ; boutons, sticks, gâchettes et croix ; vibrations des deux moteurs demandées par le jeu (et des gâchettes quand la manette en a) ; manette en souris (stick gauche pointeur, RT plus vite, LT plus précis, stick droit molette, A / B / X clics gauche, droit, milieu). La Magic Remote, quand webOS la présente comme une manette, est écartée. Écran de test : touches, sticks et gâchettes en direct, vibration d'essai par moteur et aux gâchettes.
- **Magic Remote, clavier, souris** : hors du jeu, le pointeur choisit ce qu'il survole et OK valide, les flèches vont au plus proche dans leur direction ; en jeu, les flèches et OK partent à l'ordinateur (Retour comme Échap si le réglage le veut), ou la Magic Remote devient la souris de l'ordinateur (OK clic gauche, jaune clic droit, molette) ; un clavier branché envoie ses touches (codes USB convertis en scancodes de Windows par la table du crate `keycode`, celle de kynput).
- **Réglages** (`src/ui/settings.rs`), deux écrans. **Globaux** : ce téléviseur (langue, modèle, dalle, décodeur, réseau, version) ; manettes (celles branchées, test). **Par ordinateur** : profil, définition (comme ce téléviseur, 2160p, 1440p, 1080p, 720p), images par seconde (celle de la dalle, 30, 60, 120), format (Auto : HEVC ; H.264, AV1), débit, rappel du mode jeu, VRR (indisponible, expliqué) ; son (Auto, stéréo, sur l'ordinateur) ; manettes vues comme des Xbox 360, combinaison, Retour de la Magic Remote, test des manettes ; réseau (lien, plafond, transport fiable, reconnexion, adresse d'Admin) ; compte et empreinte, retirer l'ordinateur.
- **Langues** : français et anglais (`src/i18n.rs`, paires écrites sur place), selon la langue du téléviseur ; réglage Auto / Français / English.

## Image, fréquence et mode jeu

- **Mode de la session** : celui de la dalle (`SDL_webOSGetPanelResolution`, `SDL_webOSGetRefreshRate`), soit 3840 × 2160 à 120 Hz sur une G2 ; une définition ou une fréquence choisie dans les réglages. L'écran virtuel de l'ordinateur est créé à ce mode.
- **30, 60 ou 120 images par seconde** : la dalle ne change pas de fréquence pour une app (webOS n'en donne pas le moyen) ; à 30 ou 60 i/s, chaque image y est montrée 4 ou 2 fois, à intervalles réguliers. 144 Hz : les dalles de 2025 qui l'ont (pas la G2).
- **Format** : HEVC d'office (4K à 120 i/s sur le décodeur des OLED de 2022) ; AV1 seulement jusqu'à 60 i/s (au-delà, les LG le montrent à 60 : moonlight-tv #529, punktfunk) ; H.264 au choix, déconseillé au-delà de 60 i/s.
- **Débit** : le débit du profil (proportionné aux pixels par seconde, comme les autres apps), plafonné à 80 Mbit/s en Ethernet (la prise des LG s'arrête à 100 Mbit/s, environ 93 utiles) et à 95 en Wi-Fi (limite du décodeur que retient moonlight-tv sur les puces de 2022 et après). Le lien est lu dans `/sys/class/net`.
- **Mode jeu de la télé** : aucune API ne permet à une app de passer la télé en Optimiseur de jeu ; le menu le rappelle (bouton Réglages de la Magic Remote › Image › Mode › Optimiseur de jeu), et le rappel se coupe.
- **VRR** : LG le réserve aux entrées HDMI, pour toute app interne, native ou web. L'ordinateur envoie ses images au rythme de la dalle.
- **HDR** : le décodeur sait l'afficher (`NDL_DirectVideoSetHDRInfo`, HEVC Main10) ; impossible avec Kyber 0.29 (capture en 8 bits), comme sur les autres apps.

## Décodage (NDL DirectMedia)

`src/device/ndl.rs`, d'après moonlight-tv (bibliothèque ss4s) et punktfunk, qui l'ont éprouvé sur des téléviseurs :

- `libNDL_directmedia.so.1` ouverte par `dlopen`, jamais liée (une dépendance directe empêche l'app de démarrer là où elle manque).
- `NDL_DirectMediaInit` au premier chargement, pas au lancement de l'app.
- Chaque chargement porte un **plan de son Opus stéréo, amorcé d'une trame vide** (`0xec 0xff 0xfe`) : sans lui, NDL ne cale pas les images sur leur horodatage, et l'image saccade au-dessus du 1080p sur une dalle à 120 Hz. Le son réel ne passe pas par là (voir Son).
- Horodatages en millisecondes depuis le chargement, jamais en arrière.
- **Jamais de vidage** (`NDL_DirectVideoFlushRenderBuffer`) en cours de session : il coupe le plan de son pour le reste de la session. Après une perte, aucune image n'est donnée avant l'image clé suivante, redemandée par le cœur.
- Le jeu de paramètres (SPS/PPS, VPS, en-tête AV1) est placé devant l'image clé qui (re)démarre le décodeur ; un nouveau jeu de paramètres, un autre format ou une autre taille rechargent NDL.
- Aucune fonction de NDL n'est sûre entre fils : un seul verrou.
- NDL ne dit pas quand une image est montrée : l'instant d'affichage (latence) est estimé d'après les images en attente (`NDL_DirectVideoGetRenderBufferLength`) et la fréquence de la dalle.

## Son

Opus décodé par le moteur du cœur (`apps/core/src/audio` : tampon adaptatif, masquage des pertes), tiré par le rappel de SDL (PulseAudio du téléviseur, 48 kHz, 512 images par rappel) en **stéréo** : un jeu en 5.1 ou 7.1 est mixé par le moteur. L'app demande la stéréo à l'ordinateur. Le multicanal vers un ampli en eARC passerait par le plan Opus de NDL (5.1 au plus), qu'aucun projet n'a encore vu arriver en 5.1 à un ampli : à essayer. Le son peut rester sur l'ordinateur.

## Vibrations

Les vibrations du jeu arrivent au cœur (`InputPlatform::rumble`) et partent à la manette du joueur par `SDL_GameControllerRumble` (5 s au plus sans durée, (0, 0) arrête). Sur les téléviseurs LG, les manettes Bluetooth vibrent quand webOS donne leurs moteurs (moonlight-tv : la DualSense en Bluetooth n'y vibre pas toujours) ; l'écran de test dit pour chaque manette « 2 moteurs » ou « sans vibration », et la vibration d'essai le vérifie.

## Architecture

```
apps/webos/
├── src/
│   ├── main.rs          lancement (--preview : captures sur l'ordinateur)
│   ├── app.rs           état de l'interface, navigation spatiale, actions (sans SDL ni cœur)
│   ├── model.rs         ordinateurs, réglages, identité du téléviseur ($HOME/conf/kybernow.json)
│   ├── i18n.rs          français et anglais
│   ├── ui/              dessin (tiny-skia, fontdue) : accueil, ajout, réglages, manettes, session
│   └── device/          le téléviseur : SDL (fenêtre, événements), NDL (image), son, session
│                        (Platform du cœur), manettes, tâches de fond (Admin, mDNS, code), journal
├── kyber-patches/       correctif de kynput pour webOS (client seul)
├── assets/              polices de la charte (instances fixes), logo
├── webos/               appinfo.json, icônes, écran de lancement
└── scripts/             build.sh, package.sh et ipk.py (paquet), preview.sh (captures),
                         fonts.py et images.py (polices et images tirées de la charte)
```

- **Cœur** : `kybernow-core` tel quel ; `src/device/session.rs` est l'équivalent du pont JNI d'Android (`Platform` du cœur : image, son, chaîne d'entrées, états, mesures, pointeur, vibrations).
- **SDL2** : la version webOS de webosbrew (`release-2.30.12-webos.6`, celle de moonlight-tv), compilée par `build.sh` et embarquée dans le paquet (`lib/`, trouvée par le RPATH `$ORIGIN/../lib`, en `DT_RPATH` pour passer devant l'ancienne SDL du téléviseur). Réglages : Retour et appui long sur Retour à l'app (`SDL_WEBOS_ACCESS_POLICY_KEYS_BACK`, `_EXIT`), manettes laissées en manettes (`SDL_WEBOS_CLOUDGAME_ACTIVE`). Les fonctions de SDL sont déclarées à la main (`src/device/sdl.rs`).
- **Interface** : dessinée en mémoire (tiny-skia pour les formes et les images, fontdue pour le texte) en 1920 × 1080, le plan graphique de webOS, puis envoyée à l'écran par une texture SDL. Pendant la session, la fenêtre est effacée en transparent ; le pointeur de l'ordinateur est une petite texture à part, recomposée sans redessiner l'interface.
- **Polices** : Archivo et Martian Mono de `brand/v2/fonts`, en instances fixes réduites au latin (`scripts/fonts.py`), car fontdue ne lit pas les axes variables.
- **VLC** : kyvlcplayer référence libvlc, jamais appelée : `build.rs` compile une fonction vide par fonction `libvlc_*` de vlc-rs, comme sur Android et iOS.
- **glibc** : le téléviseur a la glibc 2.12 ; `src/glibc_compat.c` (repris de punktfunk) fournit `getauxval`, `gettid` et `sendmmsg`, que la bibliothèque standard de Rust demande.
- **kynput** : sur Linux, kynput compile l'injection d'entrées du serveur (libinput, libevdev, X11), absente du téléviseur ; le correctif `kyber-patches/0001` (cfg `kybernow_webos`) le fait se comporter comme sur Android (client seul).

## Compiler

```sh
# Une fois : cible Rust, cargo-c, cmake, Python 3 (et pip install fonttools pillow cairosvg pour les polices et les images).
rustup target add armv7-unknown-linux-gnueabi
cargo install cargo-c --version 0.10.15 --locked

# Le binaire et le paquet (sur Linux x86_64 : la chaîne webOS est téléchargée dans third_party/webos-toolchain) :
apps/webos/scripts/build.sh                  # → apps/webos/dist/fr.leslignesclaires.kybernow_<version>_arm.ipk
apps/webos/scripts/build.sh --check          # cargo check seul

# Les écrans sur l'ordinateur, sans téléviseur :
apps/webos/scripts/preview.sh                # → apps/target/webos-preview/*.png
```

`scripts/build.sh` applique les correctifs d'`apps/macos/kyber-patches` puis ceux d'`apps/webos/kyber-patches`, compile dans `third_party/kyber-desktop/rootfs-armv7-unknown-linux-gnueabi` libkynput (sans SDL, `cargo cinstall`) et libopus 1.6.1 (en code indépendant de la position : le binaire est PIE), la SDL2 de webOS dans `third_party/sdl-webos-dist`, pose `libvlc.pc`, puis compile `kybernow-webos` et le met en paquet (`scripts/package.sh` : binaire sans informations de débogage, SDL2, images, licences). Les sources de Kyber 0.29 vont dans `third_party/kyber-desktop` (voir [docs/build-notes.md](../../docs/build-notes.md)).

- **Chaîne** : celle de webosbrew (`openlgtv/buildroot-nc4`, gcc 16, glibc 2.12, armv7 softfp), dans `third_party/webos-toolchain` ou `WEBOS_TOOLCHAIN`. Sur un Mac Apple Silicon (essayé le 23/10/2026) : `arm-webos-linux-gnueabi_sdk-buildroot_darwin-arm64.tar.bz2` de [webosbrew/native-toolchain](https://github.com/webosbrew/native-toolchain) (gcc 12, glibc 2.12), décompressée dans `third_party/webos-toolchain-mac`, `./relocate-sdk.sh` une fois, puis `WEBOS_TOOLCHAIN=…/arm-webos-linux-gnueabi_sdk-buildroot apps/webos/scripts/build.sh` (avec `/opt/homebrew/opt/rustup/bin` dans le `PATH` si rustup vient de Homebrew). `scripts/cc.sh` lui donne sa racine système et lie libstdc++ en statique.
- **Options de Rust** : `-C target-feature=+neon,+vfp3,-soft-float -C target-cpu=cortex-a73` (la cible « gnueabi » émule sinon le calcul en virgule flottante : dix fois plus lent, d'après punktfunk), `--cfg kybernow_webos`, `--cfg getrandom_backend="linux_raw"`.
- **Paquet** : `scripts/ipk.py` écrit le `.ipk` comme `ares-package` (archive ar : `debian-binary`, `control.tar.gz`, `data.tar.gz` avec `usr/palm/applications/<id>/` et `usr/palm/packages/<id>/packageinfo.json`), sans les outils de LG.
- **Émulation** : `qemu-arm` ne lance pas un binaire lié à la glibc 2.12 du téléviseur (son chargeur plante, même pour un programme C minimal) ; l'aperçu se vérifie avec un binaire statique (`--no-default-features`, `+crt-static`, la cale de glibc en `-C link-arg`, `qemu-arm -cpu max`).

## Journal

`$HOME/conf/kybernow.log` dans le dossier de l'app sur le téléviseur (le précédent en `.1`), et la sortie d'erreur : lignes `image :` (NDL), `son :`, `manettes :`, `session :`, avec les messages du cœur. Voir [INSTALLATION.md](INSTALLATION.md) pour le lire.

## Reste à faire

- Sur la G2 : une session de bout en bout (NDL, son, manettes, vibrations), la latence en 4K à 120 Hz, en 1440p et en 1080p, à 30, 60 et 120 i/s.
- Magic Remote en souris : vérifier que le téléviseur envoie toujours les mouvements quand son pointeur est caché.
- Son multicanal par le plan Opus de NDL vers un ampli en eARC.
- Où garder les ordinateurs pour qu'une réinstallation de l'app ne les efface pas.
- DualSense : vibrations et gâchettes adaptatives par le service Bluetooth de webOS (comme punktfunk), quand SDL ne les donne pas.
- Homebrew Channel : dépôt de l'app (mises à jour).
