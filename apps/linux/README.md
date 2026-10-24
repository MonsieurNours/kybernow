# KyberNow Link pour Linux, le Steam Deck, SteamOS et le Raspberry Pi

[English](README.en.md) · Français

KyberNow Link sur Linux : une app **native** sur le cœur Rust des autres apps Link (`apps/core`), qui retrouve l'ordinateur (KyberNow Station : serveur Kyber 0.29 + KyberNow Admin) en plein écran, au clavier et à la souris ou à la manette. Une seule app, trois interfaces d'après les maquettes ([Claude Design](https://claude.ai/artifact/MgBJNBJ4WEMFvVB6Ujfxhc) ; Raspberry Pi : https://claude.ai/artifact/6crVeg3QyrA5vs8TRhWRft) :

- **Bureau** (GNOME, KDE…) : fenêtre sans bordure avec sa barre de titre, souris et clavier, réglages en segments, barre de session en haut ;
- **Portable** (Steam Deck et consoles sous gamescope, 1280 × 800) : tout au focus et à la manette, légende des boutons, lancement animé ;
- **Salon** (SteamOS au téléviseur, Steam Machine) : la même à trois mètres, avec la colonne « Prêt à jouer ».

Sous gamescope, l'app prend la console (portable jusqu'à 1280 × 800, salon au-delà), ailleurs le bureau ; Réglages de l'app › Interface, `--ui desktop|handheld|tv` ou `KYBERNOW_UI` la forcent. Installation : [INSTALLATION.md](INSTALLATION.md). La décision est dans [PROJECT.md](../../PROJECT.md) (« Client Linux, Steam Deck et SteamOS »).

**État** : compilée (x86_64 et aarch64) et essayée en session sur un Linux de bureau (Ubuntu 24.04 sous GNOME, en machine virtuelle, décodage par le processeur) ; pas encore essayée sur le Steam Deck. **Raspberry Pi** : le paquet .deb et KyberNow Link OS sont construits dans une machine virtuelle ARM64, l'image démarre en émulation QEMU ; rien n'a encore tourné sur une carte (voir « Pas encore essayé »).

## Ce que fait l'app

- **Lancement** (console) : la séquence de l'iPhone et de l'iPad (symbole éteint, grésillement, nom et signature, puis l'accueil reprend le symbole) ; A ou B la sautent ; `KYBERNOW_REDUCE_MOTION=1` l'allège (le symbole s'allume sans grésiller, puis un fondu).
- **Accueil** (`src/ui/home.rs`) : l'ordinateur choisi en plein cadre (la dernière image de la dernière session, sinon l'aperçu d'Admin, sinon sa signature), son état, l'écran virtuel qui l'attend (« 1280 × 800 · 90 Hz, comme le Deck », VRR), « Se connecter » et les profils ; sur le bureau la dernière session (latence sur la règle d'une image), au salon « Prêt à jouer » (manettes, son, écran, décodeurs), tous deux sur un panneau d'encre ; en bas les ordinateurs, ceux trouvés sur le réseau et l'ajout. À la manette, ‹ › sur « Se connecter » changent d'ordinateur.
- **Ajout** (`src/ui/pair.rs`) : code à six chiffres présenté à chaque Station trouvée (mDNS `_kybernow._tcp`) et accepté dans Admin › Appareils Link ; ou adresse et port d'Admin (clavier, clavier de Steam ou pavé à la manette), puis les quatre premiers groupes de l'empreinte à comparer.
- **Réglages** (`src/ui/settings.rs`) : sur le bureau, d'après la maquette Raspberry : à gauche le profil (une rangée de boutons), les rubriques et la carte « Cet appareil » ; à droite la rubrique choisie, un champ par réglage (libellé, segments et leurs étiquettes « VA-API », « décodeur du Pi », « processeur », valeurs indisponibles barrées, aide) ; fil d'Ariane dans la barre de titre (logo › ordinateur › Réglages, l'ordinateur ramène à l'accueil). Par ordinateur, profil ; écran (comme cet écran, 4K, 1440p, 1080p, 800p, 720p), fréquence, format (ceux que la carte décode), couleurs 4:4:4 (en option, HEVC), **affichage** (Auto, synchronisé, immédiat, fréquence variable), débit, écran virtuel, alléger sur batterie ; son (Auto, stéréo, 5.1, 7.1, sur l'ordinateur) ; manette vue par l'ordinateur, combinaison, test des manettes ; souris (Auto, bureau, jeu), presse-papiers ; transport, reconnexion ; compte, réveil par le réseau (l'adresse MAC, au clavier ou au pavé hexadécimal : « Réveiller l'ordinateur » remplace « Se connecter » sur l'accueil quand il est hors ligne), retirer. Réglages de l'app : interface, langue, dernière image, versions proposées, recherche d'une version, test des manettes. « Cet ordinateur » / « Cet appareil » dit ce que l'app a trouvé.
- **Session** (`src/ui/session.rs`, `src/device/`) : passage (la couture, le néon, les étapes, Annuler) ; image plein cadre ; **bureau** : barre en haut à l'approche du pointeur (latence et courbe, définition, images par seconde, décodeur, profil, touches, souris, plein écran, Quitter) ; **console** : menu à la manette (santé du flux, tuiles : Reprendre, profils, son, manette en souris, clavier de Steam, touches, mesures, alléger ou manettes, Se déconnecter) ; mesures par-dessus l'image ; capsules (souris des jeux, manette branchée, reconnexion) ; fin sur erreur.

## Image, fréquence variable et décodage

- **Décodage** (`src/native/decoder.c`, `src/device/video.rs`) : FFmpeg 7.1.1 réduit au décodage, lié en statique ; **VA-API** quand la carte sait le format (sondé au lancement par libva : H.264, HEVC, AV1), sinon le processeur (H.264 et HEVC, fils par tranches). FFmpeg est compilé sans NVDEC : une carte NVIDIA ne décode que par un pilote VA-API. Les images VA-API sont recopiées en NV12 et montrées par SDL (une copie par image : négligeable en 1280 × 800). Seule l'image la plus récente est montrée.
- **Profils** ramenés à ce que cette machine décode : HEVC (carte, sinon processeur), H.264 ; AV1 au choix si la carte le décode ; **4:4:4 en option** (Réglages › Image › Couleurs 4:4:4, HEVC seulement, désactivé par défaut) : la carte le décode si VA-API sait le HEVC 4:4:4 (Intel récentes), sinon le processeur ; SDL n'ayant pas de texture YUV 4:4:4, `decoder.c` convertit chaque image en BGRA sur le fil du décodeur (virgule fixe, BT.709 ou BT.601, jusqu'à quatre fils). Mesuré dans la VM (Apple M, processeur) : 3,4 ms par image 1440p, décodage et conversion compris, contre 1,9 ms en 4:2:0. Test hors ligne : `tests/video/run.sh` (couleurs à ±3, formats, temps).
- **Fréquence variable (VRR)** : l'app lit la propriété `vrr_capable` des écrans branchés par DRM (`src/native/display.c`, libdrm, lecture seule). Affichage Auto : si l'écran l'annonce, chaque image est montrée dès qu'elle est décodée (sans attendre la synchro verticale) et le compositeur (gamescope, KWin, Mutter) cale l'écran sur la cadence du flux ; sinon synchronisé. Synchronisé, Immédiat et Fréquence variable se forcent dans les réglages. Le Steam Deck (LCD et OLED) n'a pas de VRR ; un écran ou un téléviseur branché au dock peut l'avoir.
- **Mode de la session** : celui de l'écran (« comme cet écran ») ou la hauteur choisie, à la fréquence de l'écran ou choisie ; sur batterie avec « Alléger », 60 i/s et 20 Mbit/s au plus.
- **Mode jeu du téléviseur (ALLM)** : géré par le compositeur (gamescope), pas par l'app.

## Raspberry Pi

La même app, sur Raspberry Pi OS (Debian 13 « trixie », arm64), en paquet `.deb`, ou seule sur la carte avec KyberNow Link OS (plus bas) ; la décision est dans [PROJECT.md](../../PROJECT.md) (« Client Raspberry Pi »), maquettes : https://claude.ai/artifact/6crVeg3QyrA5vs8TRhWRft.

- **Détection** (`src/device/system.rs`, `pi()`) : l'arbre des périphériques (`/proc/device-tree/model` → « Raspberry Pi 5 », « Compute Module 5 », « Raspberry Pi 3 B+ » ; `compatible` → génération 5 : BCM2712, 4 : BCM2711, 3 : BCM2837), la mémoire (`/proc/meminfo`, arrondie au Go), les décodeurs V4L2 (`/sys/class/video4linux/*/name` : `rpivid` pour le HEVC des Pi 4 et 5, `bcm2835-codec` pour le H.264 des Pi 3 et 4), la température (zone thermique 0, relue toutes les 5 s), le système (« Raspberry Pi OS (Trixie) », d'après `/etc/rpi-issue`). Les modes de l'écran branché sont lus par DRM (`display.c`, `kn_modes_probe`) : fréquence la plus haute en 1080p et plus grand mode.
- **Ce que la sortie permet** (`Pi` dans `app.rs`) : 4K à 60 Hz au plus sur les Pi 4 et 5 (HDMI 2.0), 1080p avant ; 1080p à 120 Hz si l'écran l'annonce (pas encore vérifié sur une carte) ; pas de VRR. Les choix d'écran, de fréquence et de format au-delà sont grisés (barrés) ; les étiquettes disent qui décode ; le 4:4:4 passe au processeur (1080p au plus) ; les aides expliquent (« 4K seulement en HEVC, décodé par le Pi. Au-delà de 1080p, le H.264 passe par le processeur et saccade »).
- **Accueil** : le panneau « Ce Raspberry Pi » remplace « Dernière session » (décodeurs, écran et sortie HDMI, son, réseau avec la latence de la dernière session) ; la légende dit « Cet appareil : Raspberry Pi 5 · 8 Go · Raspberry Pi OS (Trixie) · HEVC par le décodeur du Pi · PipeWire » ; la version porte l'architecture (« 0.4.8 · aarch64 »). Captures : `--preview` produit `rpi-01-accueil.png`, `rpi-07-reglages-image.png`… (machine simulée).
- **Décodage** (`decoder.c`, mode `KN_HW_V4L2`) : HEVC par le hwaccel `hevc_v4l2request` du FFmpeg de Raspberry Pi ([jc-kynesim/rpi-ffmpeg](https://github.com/jc-kynesim/rpi-ffmpeg), branche `dev/7.1.1/rpi_import_1`, compilé par `build.sh --rpi` avec `--enable-v4l2-request --enable-sand --enable-libudev`), périphérique DRM (le premier `/dev/dri/cardN` qui s'ouvre), images `DRM_PRIME` au format SAND recopiées en NV12 linéaire par `av_hwframe_transfer_data` (une copie par image, sur KyberNow Link OS aussi) ; H.264 par le décodeur `h264_v4l2m2m` (M2M, dans tout FFmpeg Linux : `--enable-v4l2-m2m`), images en mémoire. En cas de refus ou d'erreurs en boucle, le processeur décode. Le FFmpeg officiel (Flatpak) n'a pas le hwaccel V4L2 request : le Flatpak aarch64 sur un Pi décode le HEVC par le processeur.
- **Paquet** (`scripts/deb.sh`, sur un Linux arm64 : la VM, ou un Pi) : `build.sh --rpi --install`, lanceur, fiche AppStream, icônes, licences, dépendances lues sur le binaire (`dpkg-shlibdeps`), `dpkg-deb` → `dist/KyberNow-Link-RaspberryPi-arm64.deb` (publié avec chaque version, `release-flatpaks.sh` le construit dans la VM après les Flatpak) et `kybernow-link_<version>_arm64.deb`. **Dépôt apt** : `scripts/apt-repo.sh` assemble un dépôt depuis les .deb de `dist` (`pool/` et `dists/stable/`, par `dpkg-scanpackages` et `apt-ftparchive`), signé si `KYBERNOW_APT_GPG_KEY` nomme une clé GPG (clé publique exportée en `kybernow.gpg`), à servir tel quel par un site statique ; `release.sh` ne l'appelle pas et aucun dépôt n'est publié. L'app en .deb (hors Flatpak, sur arm64) cherche sa mise à jour dans `KyberNow-Link-RaspberryPi-arm64.deb` de la version publiée et la signale.
- **KyberNow Link OS**, l'image à flasher : [os/README.md](os/README.md). Raspberry Pi OS Lite par pi-gen, Link seul en plein écran sur KMS/DRM, Plymouth avec le symbole éteint puis l'allumage de la charte avec son, premier démarrage (manette, langue, réseau, usage, ordinateur), réglages de l'appareil, veille et HDMI-CEC, mise à jour de tout l'OS en A/B (tryboot). Côté app : `src/device/os.rs`, `src/device/chime.rs`, `src/ui/setup.rs`, `src/ui/device.rs` ; mode OS quand `KYBERNOW_OS=1` ou `/etc/kybernow-os/release` existe.
- **Pas encore vérifié sur une carte** (aucun Raspberry Pi disponible ; tout est compilé dans la VM Ubuntu ARM64) : le décodeur HEVC (API request, format SAND, latence image par image), le H.264 M2M, la sortie 1080p à 120 Hz, le son HDMI multicanal, les manettes Bluetooth, le paquet sur Raspberry Pi OS Trixie (dépendances calculées sur Ubuntu 24.04, des noms que Debian 13 a aussi : `libsdl2-2.0-0`, `libopus0`, `libva2`, `libva-drm2`, `libdrm2`, `libudev1`).

## Son

Opus décodé par le moteur du cœur (tampon adaptatif, masquage des pertes, dérive des horloges), tiré par le rappel de SDL (PipeWire ou PulseAudio ; ALSA sur KyberNow Link OS ; 256 images par rappel), en stéréo, 5.1 ou 7.1 selon la sortie (Auto) ou le réglage, dans l'ordre des voies de SDL. Coupé depuis le menu de session. Le son peut rester sur l'ordinateur.

## Entrées

- **Clavier** : scancodes USB de SDL convertis en scancodes de Windows (table du crate `keycode`, celle de kynput) ; en plein écran pendant la session, le clavier est saisi (Alt+Tab, Super… vont à l'ordinateur). Raccourcis de l'app : Ctrl+Alt+Maj + D, F, M.
- **Souris** : position absolue ramenée à l'écran de l'ordinateur ; **souris des jeux** (relative, `SDL_SetRelativeMouseMode`) quand un jeu est en plein écran sur l'ordinateur et cache son pointeur (Auto, règles du cœur), ou forcée ; Échap tenu une seconde la rend. Le pointeur de l'ordinateur devient celui du système à l'échelle de l'image (bureau), ou est dessiné par l'app (souris des jeux, console).
- **Pavés tactiles et écran tactile du Deck** : souris par Steam Input ; un toucher en haut de l'écran ouvre le menu.
- **Manettes** : SDL GameController, quatre joueurs, vibrations (et gâchettes), annoncées une fois par liaison ; Steam Input présente celles du Deck comme une manette Xbox. Manette en souris (stick gauche, RT plus vite, LT plus précis, stick droit molette, A / B / X clics). L1 + R1 + Start + Select : menu.
- **Texte** : « Clavier » du menu ouvre le clavier de Steam (`steam://open/keyboard`) ; les caractères partent alors en Unicode.
- **Presse-papiers** : texte dans les deux sens (le pont du cœur ; SDL lu et écrit depuis le fil principal).

## Architecture

```
apps/linux/
├── src/
│   ├── main.rs          lancement (--preview : captures, --ui : interface)
│   ├── app.rs           état, navigation spatiale et à la souris, réglages (sans SDL ni cœur)
│   ├── model.rs         ordinateurs, réglages ($XDG_CONFIG_HOME/kybernow/kybernow.json, 0600)
│   ├── ui/              dessin (tiny-skia, fontdue) : lancement, accueil, ajout, réglages,
│   │                    manettes, session ; logo tracé depuis la géométrie de la charte
│   ├── device/          SDL (fenêtre, événements, son), session (Platform du cœur), image
│   │                    (FFmpeg), machine (DMI, VA-API, DRM), manettes, tâches de fond, journal
│   └── native/          decoder.c (FFmpeg : VA-API, V4L2 du Pi), display.c (VRR et modes par DRM)
├── flatpak/             manifeste, .desktop, AppStream, icône, licences
├── os/                  KyberNow Link OS : image Raspberry Pi (pi-gen, A/B, Plymouth, services), voir os/README.md
├── scripts/             build.sh (--rpi), sources.sh, flatpak.sh, release-flatpaks.sh, deb.sh, apt-repo.sh
└── tests/video/         test hors ligne du décodeur (run.sh, check.c)
```

- **Interface** : dessinée en mémoire en *points* (ceux des maquettes : 1280 × 800 pour le bureau et le Deck, 1920 × 1080 pour le salon), multipliés par l'échelle de l'écran (HiDPI) ; le logo signé est tracé depuis les chemins de `brand/v2` (comme `Mark.svelte` de l'app Windows), net à toute taille.
- **Fenêtre du bureau** : sans bordure ; la barre de titre de l'app se saisit pour déplacer la fenêtre, les bords pour la redimensionner (`SDL_SetWindowHitTest`), double clic pour agrandir. SDL2 prend X11 (Xwayland sur GNOME et KDE, gamescope sur le Deck) ; `KYBERNOW_VIDEO=wayland` essaie Wayland. La classe de la fenêtre est l'identifiant de l'app (`fr.leslignesclaires.kybernow`).
- **kynput** : client seul, avec le correctif de webOS (`apps/webos/kyber-patches/0001`, `--cfg kybernow_webos`) : sans l'injection d'entrées du serveur (libinput, libevdev, X11).
- **VLC** : kyvlcplayer référence libvlc, jamais appelée : `build.rs` compile une fonction vide par fonction `libvlc_*`, comme sur Android, iOS et webOS.

## Compiler

```sh
# Captures des écrans, sans SDL ni cœur (sur Mac aussi) :
cargo run -p kybernow-linux --no-default-features -- --preview /tmp/kn-linux fr

# Sur un Linux (Ubuntu 24.04 : build-essential pkg-config nasm clang libclang-dev
# libsdl2-dev libva-dev libdrm-dev libopus-dev, Rust 1.89 ou après, cargo-c) :
apps/linux/scripts/build.sh                    # → apps/target/release/kybernow

# Le Flatpak (flatpak-builder, runtime org.freedesktop.Platform 25.08,
# extensions rust-stable et llvm20), pour cette architecture ou l'autre :
apps/linux/scripts/flatpak.sh                  # → apps/linux/dist/KyberNow-Link-Linux-<arch>.flatpak
apps/linux/scripts/flatpak.sh x86_64           # depuis une machine ARM : Rosetta ou qemu-user-static

# Depuis le Mac, les deux Flatpak puis le .deb du Pi dans la VM « KyberNow Linux » (ce que fait release.sh) :
apps/linux/scripts/release-flatpaks.sh [--check] [DOSSIER]

# Raspberry Pi, sur un Linux arm64 (la VM, ou un Pi ; libudev-dev en plus) :
apps/linux/scripts/build.sh --rpi              # FFmpeg de Raspberry Pi, décodeur HEVC V4L2 request
apps/linux/scripts/deb.sh                      # → apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb
apps/linux/scripts/apt-repo.sh [DOSSIER]       # → dist/apt (KYBERNOW_APT_GPG_KEY pour signer)

# Le décodeur hors ligne (après build.sh ; ffmpeg et x265 génèrent les flux) :
apps/linux/tests/video/run.sh [--hw]
```

`build.sh` applique les correctifs client de Kyber, compile libkynput (`cargo cinstall`, statique) et FFmpeg 7.1.1 réduit au décodage (VA-API) dans un préfixe, puis l'app. `sources.sh` assemble les seules sources utiles (une centaine de mégaoctets au lieu des compilations de Kyber), que `flatpak.sh` donne à flatpak-builder en deux modules : les dépendances (en cache tant que Kyber ne change pas), puis l'app. Les sources de Kyber 0.29 vont dans `third_party/kyber-desktop` (voir [docs/build-notes.md](../../docs/build-notes.md)).

**Machine de compilation et d'essai** : une VM Parallels « KyberNow Linux » sur le Mac de développement (Ubuntu 24.04 ARM64, GNOME, Flatpak), avec Rosetta enregistré dans binfmt (`/etc/binfmt.d/rosetta.conf` et un service au démarrage) pour construire le Flatpak x86_64 du Steam Deck à vitesse quasi native, et le dépôt partagé (`/media/psf/kyber-apps`). Sa carte graphique virtuelle (virgl) ne décode rien par VA-API : le processeur y décode.

## Pas encore essayé

- Sur le Steam Deck : une session de bout en bout (VA-API sur la puce Van Gogh, son PipeWire, manette et pavés par Steam Input, clavier de Steam), la latence, le passage en mode Jeu.
- Sur un Linux de bureau avec une carte AMD ou Intel : VA-API en HEVC et AV1, VRR sur un écran FreeSync, souris des jeux sous Xwayland.
- Raspberry Pi, sur une carte : voir « Pas encore vérifié sur une carte » plus haut ; KyberNow Link OS : [os/README.md](os/README.md), « Pas encore vérifié sur une carte ».