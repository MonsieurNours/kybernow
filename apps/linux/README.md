# KyberNow Link pour Linux, le Steam Deck et SteamOS

[English](README.en.md) · Français

KyberNow Link sur Linux : une app **native** sur le cœur Rust des autres apps Link (`apps/core`), qui retrouve l'ordinateur (KyberNow Station : serveur Kyber 0.29 + KyberNow Admin) en plein écran, au clavier et à la souris ou à la manette. Une seule app, trois interfaces d'après les maquettes ([Claude Design](https://claude.ai/artifact/MgBJNBJ4WEMFvVB6Ujfxhc)) :

- **Bureau** (GNOME, KDE…) : fenêtre sans bordure avec sa barre de titre, souris et clavier, réglages en segments, barre de session en haut ;
- **Portable** (Steam Deck et consoles sous gamescope, 1280 × 800) : tout au focus et à la manette, légende des boutons, lancement animé ;
- **Salon** (SteamOS au téléviseur, Steam Machine) : la même à trois mètres, avec la colonne « Prêt à jouer ».

Sous gamescope, l'app prend la console (portable jusqu'à 1280 × 800, salon au-delà), ailleurs le bureau ; Réglages de l'app › Interface, `--ui desktop|handheld|tv` ou `KYBERNOW_UI` la forcent. Installation : [INSTALLATION.md](INSTALLATION.md). La décision est dans [PROJECT.md](../../PROJECT.md) (« Client Linux, Steam Deck et SteamOS »).

**État (23/10/2026)** : écrite, compilée (x86_64 et aarch64) et lancée sur un Linux de bureau (Ubuntu 24.04 sous GNOME, en machine virtuelle) : accueil, réglages, lancement, découverte des Stations ; pas encore essayée en session, ni sur le Steam Deck.

## Ce que fait l'app

- **Lancement** (console) : la séquence de l'iPhone et de l'iPad (symbole éteint, grésillement, nom et signature, puis l'accueil reprend le symbole) ; A ou B la sautent.
- **Accueil** (`src/ui/home.rs`) : l'ordinateur choisi en plein cadre (la dernière image de la dernière session, sinon l'aperçu d'Admin, sinon sa signature), son état, l'écran virtuel qui l'attend (« 1280 × 800 · 90 Hz, comme le Deck », VRR), « Se connecter » et les profils ; sur le bureau la dernière session (latence sur la règle d'une image), au salon « Prêt à jouer » (manettes, son, écran, décodeurs), tous deux sur un panneau d'encre ; en bas les ordinateurs, ceux trouvés sur le réseau et l'ajout. À la manette, ‹ › sur « Se connecter » changent d'ordinateur.
- **Ajout** (`src/ui/pair.rs`) : code à six chiffres présenté à chaque Station trouvée (mDNS `_kybernow._tcp`) et accepté dans Admin › Appareils Link ; ou adresse et port d'Admin (clavier, clavier de Steam ou pavé à la manette), puis les quatre premiers groupes de l'empreinte à comparer.
- **Réglages** (`src/ui/settings.rs`) : par ordinateur, profil ; écran (comme cet écran, 4K, 1440p, 1080p, 800p, 720p), fréquence, format (ceux que la carte décode), 4:4:4 (indisponible), **affichage** (Auto, synchronisé, immédiat, fréquence variable), débit, écran virtuel, alléger sur batterie ; son (Auto, stéréo, 5.1, 7.1, sur l'ordinateur) ; manette vue par l'ordinateur, combinaison, test des manettes ; souris (Auto, bureau, jeu), presse-papiers ; transport, reconnexion ; compte, retirer. Réglages de l'app : interface, langue, dernière image, versions proposées. « Cet ordinateur » / « Cet appareil » dit ce que l'app a trouvé.
- **Session** (`src/ui/session.rs`, `src/device/`) : passage (la couture, le néon, les étapes, Annuler) ; image plein cadre ; **bureau** : barre en haut à l'approche du pointeur (latence et courbe, définition, images par seconde, décodeur, profil, touches, souris, plein écran, Quitter) ; **console** : menu à la manette (santé du flux, tuiles : Reprendre, profils, son, manette en souris, clavier de Steam, touches, mesures, alléger ou manettes, Se déconnecter) ; mesures par-dessus l'image ; capsules (souris des jeux, manette branchée, reconnexion) ; fin sur erreur.

## Image, fréquence variable et décodage

- **Décodage** (`src/native/decoder.c`, `src/device/video.rs`) : FFmpeg 7.1.1 réduit au décodage, lié en statique ; **VA-API** quand la carte sait le format (sondé au lancement par libva : H.264, HEVC, AV1), sinon le processeur (H.264 et HEVC, fils par tranches). Les images VA-API sont recopiées en NV12 et montrées par SDL (une copie par image : négligeable en 1280 × 800 ; import DMA-BUF sans copie à venir pour la 4K à 120 Hz). Seule l'image la plus récente est montrée.
- **Profils** ramenés à ce que cette machine décode : HEVC (carte, sinon processeur), H.264 ; AV1 au choix si la carte le décode ; pas de 4:4:4 (pas encore sur Linux).
- **Fréquence variable (VRR)** : l'app lit la propriété `vrr_capable` des écrans branchés par DRM (`src/native/display.c`, libdrm, lecture seule). Affichage Auto : si l'écran l'annonce, chaque image est montrée dès qu'elle est décodée (sans attendre la synchro verticale) et le compositeur (gamescope, KWin, Mutter) cale l'écran sur la cadence du flux ; sinon synchronisé. Synchronisé, Immédiat et Fréquence variable se forcent dans les réglages. Le Steam Deck (LCD et OLED) n'a pas de VRR ; un écran ou un téléviseur branché au dock peut l'avoir.
- **Mode de la session** : celui de l'écran (« comme cet écran ») ou la hauteur choisie, à la fréquence de l'écran ou choisie ; sur batterie avec « Alléger », 60 i/s et 20 Mbit/s au plus.
- **Mode jeu du téléviseur (ALLM)** : géré par le compositeur (gamescope), pas par l'app.

## Son

Opus décodé par le moteur du cœur (tampon adaptatif, masquage des pertes, dérive des horloges), tiré par le rappel de SDL (PipeWire ou PulseAudio, 256 images par rappel), en stéréo, 5.1 ou 7.1 selon la sortie (Auto) ou le réglage, dans l'ordre des voies de SDL. Coupé depuis le menu de session. Le son peut rester sur l'ordinateur.

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
│   └── native/          decoder.c (FFmpeg, VA-API), display.c (VRR par DRM)
├── flatpak/             manifeste, .desktop, AppStream, icône, licences
└── scripts/             build.sh, sources.sh, flatpak.sh, preview.sh
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
# libsdl2-dev libva-dev libdrm-dev libopus-dev libssl-dev, Rust, cargo-c) :
apps/linux/scripts/build.sh                    # → apps/target/release/kybernow

# Le Flatpak (flatpak-builder, runtime org.freedesktop.Platform 25.08,
# extensions rust-stable et llvm20), pour cette architecture ou l'autre :
apps/linux/scripts/flatpak.sh                  # → apps/linux/dist/KyberNow-Link-Linux-<arch>.flatpak
apps/linux/scripts/flatpak.sh x86_64           # depuis une machine ARM : Rosetta ou qemu-user-static
```

`build.sh` applique les correctifs client de Kyber, compile libkynput (`cargo cinstall`, statique) et FFmpeg 7.1.1 réduit au décodage (VA-API) dans un préfixe, puis l'app. `sources.sh` assemble les seules sources utiles (une centaine de mégaoctets au lieu des compilations de Kyber), que `flatpak.sh` donne à flatpak-builder en deux modules : les dépendances (en cache tant que Kyber ne change pas), puis l'app. Les sources de Kyber 0.29 vont dans `third_party/kyber-desktop` (voir [docs/build-notes.md](../../docs/build-notes.md)).

**Machine de compilation et d'essai** : une VM Parallels « KyberNow Linux » sur le Mac de développement (Ubuntu 24.04 ARM64, GNOME, Flatpak), avec Rosetta enregistré dans binfmt (`/etc/binfmt.d/rosetta.conf` et un service au démarrage) pour construire le Flatpak x86_64 du Steam Deck à vitesse quasi native, et le dépôt partagé (`/media/psf/kyber-apps`). Sa carte graphique virtuelle (virgl) ne décode rien par VA-API : le processeur y décode.

## Reste à faire

- Sur le Steam Deck : une session de bout en bout (VA-API sur la puce Van Gogh, son PipeWire, manette et pavés par Steam Input, clavier de Steam), la latence, le passage en mode Jeu.
- Sur un Linux de bureau avec une carte AMD ou Intel : VA-API en HEVC et AV1, VRR sur un écran FreeSync, souris des jeux sous Xwayland.
- Import DMA-BUF des images VA-API (sans copie) pour la 4K à 120 Hz ; NVIDIA (NVDEC par FFmpeg).
- Un dépôt Flatpak ou Flathub, pour que les mises à jour s'installent seules.
