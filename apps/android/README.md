# KyberNow Link pour Android et Android TV

[English](README.en.md) · Français

KyberNow Link sur Android : une seule app pour le téléphone, la tablette, le téléviseur (Android TV, Google TV) et la console portable, qui retrouve l'ordinateur (KyberNow Station : serveur Kyber 0.29 + KyberNow Admin) en plein écran, au doigt, à la télécommande, au clavier et à la souris ou à la manette. Identité v2 ([charte](../../brand/v2/README.md)) ; maquettes : [Claude Design](https://claude.ai/artifact/Ug3n57UPSvsrA3Hd9UMawX) (accueil, menu de session, manettes et vibrations, réglages de l'image au téléviseur ; accueil, ajout et session au téléphone ; accueil à la tablette).

**État** : cadrage, maquettes, et le cœur Rust qui compile pour les trois cibles d'Android (`scripts/build-core.sh`, 22/10/2026). Pas encore de code de l'app. La décision est dans [PROJECT.md](../../PROJECT.md) (« Android et Android TV »).

## Appareils

- **Android 11 au moins** (API 30) : la NVIDIA Shield ne dépasse pas Android 11.
- **Téléviseurs** : NVIDIA Shield d'abord, Google TV Streamer, Chromecast avec Google TV, téléviseurs sous Google TV. Le Chromecast avec Google TV et beaucoup de téléviseurs font tourner Android en 32 bits : l'app est publiée en `arm64-v8a` et `armeabi-v7a`.
- **Téléphones et tablettes**, avec l'écran externe en USB-C, Samsung DeX et ChromeOS (fenêtre redimensionnable, clavier et souris).
- **Consoles portables** (Retroid, AYN Odin) et **manettes de téléphone** (Razer Kishi, Backbone) : la manette intégrée est le joueur 1 ; l'interface se pilote au focus, comme au téléviseur.
- **Matériel d'essai** : Shield TV 2019 « tube » (pas de port USB : manettes en Bluetooth seulement), Xiaomi Redmi A5 ; manettes DualSense, Xbox, Shield et Switch Pro. Rien pour essayer une tablette, une console portable, DeX ni ChromeOS : ces parties seront écrites sans être vérifiées.

## Ce que fera l'app

Ce que font déjà les apps Apple TV, iPhone et iPad, Mac et Windows, sur le même cœur :

- **Accueil** : l'ordinateur choisi en plein cadre (sa dernière image, sinon l'aperçu d'Admin), son état (réseau local, VPN, hors ligne), système, carte graphique, adresse et écran virtuel qui l'attend ; « Se connecter » et les profils Jeu, Bureau, Max ; colonne « Prêt à jouer » au téléviseur et à la tablette (manettes et leurs moteurs, sortie du son et son retard, mode de l'écran, dernière latence) ; les autres ordinateurs et ceux trouvés sur le réseau.
- **Ajout** : sur le réseau (`_kybernow._tcp` par NsdManager), code à six chiffres accepté dans Admin, QR code à la caméra (lien `kybernow://`, aussi ouvert depuis une autre app), adresse avec les quatre premiers groupes de l'empreinte. Au téléviseur, en plus : il affiche un code, l'app du téléphone le saisit et lui envoie ses ordinateurs, relayés par Admin (à la place d'iCloud ; à écrire côté Station). Mots de passe dans le Keystore d'Android.
- **Passage** et **session** : la couture à 45° et le néon, la capsule « Vous entrez dans… » ; au téléphone, la languette et la barre (latence, profil, pavé tactile ou toucher direct, souris Auto / Bureau / Jeu, écran, son, Touches, Quitter), la rangée de touches au-dessus du clavier (Échap, Tab, Ctrl, Alt et ⊞ collants, flèches, F1 à F12, Ctrl + Alt + Suppr) ; au téléviseur, le menu par le bouton Retour ou L1 + R1 + Start + Select (Reprendre, Manettes, Son, Image, Télécommande en souris, Taper du texte, Touches, Se déconnecter), la santé du flux en haut ; états en capsule, jamais en alerte (reconnexion, mode de l'ordinateur changé, repli, manette branchée).
- **Manettes** : quatre joueurs au plus (XInput côté ordinateur), une cinquième en attente ; manette vue comme Xbox 360 ou fidèle ; manette en souris ; bouton Guide (Jeu, Appui long, Système) ; écran de test (touches, sticks, gâchettes, vibration d'essai).
- **Clavier et souris** : souris brute par la capture du pointeur d'Android en profil Jeu, pointeur de l'ordinateur dessiné par l'app en Bureau ; presse-papiers texte dans les deux sens au téléphone et à la tablette.
- **Arrière-plan** : tout est relâché sur l'ordinateur, le débit plafonné, l'image figée ; au retour, tout revient.
- **Rangée « Reprendre sur un ordinateur »** sur l'accueil d'Android TV, avec la dernière image de chaque ordinateur ; un clic ouvre la session.
- **Réglages** par ordinateur et de l'app ; français et anglais (langue du système, ou Auto / Français / English).

Hors première version : manette à l'écran, mise en page des pliables.

## Image, fréquence et mode jeu

- **Décodage** : le correctif 0001 de kyclient donne les paquets vidéo à l'app ; MediaCodec les décode en mode faible latence (`KEY_LOW_LATENCY`, Android 11) et les rend sur une `SurfaceView`, que l'écran reçoit sans composition (comme la couche Metal seule de l'Apple TV). Rien ne doit rester au-dessus de l'image pendant la session. Les profils se ramènent à ce que l'appareil décode vraiment, vérifié au lancement : la Shield décode HEVC et H.264, pas l'AV1 ni le 4:4:4.
- **Fréquence** : l'app choisit le mode de l'écran (`preferredDisplayModeId`) et la fréquence de sa surface (`Surface.setFrameRate`). La Shield sort en HDMI 2.0 : 4K à 60 Hz ou 1080p à 120 Hz. Au téléphone, le 120 Hz se demande : Android 15 met les jeux à 60 Hz sans demande.
- **Mode jeu du téléviseur** : `Window.setPreferMinimalPostProcessing(true)` (ALLM sur l'HDMI) pendant la session, quand l'écran le permet (`Display.isMinimalPostProcessingSupported`).
- **VRR** : Android ne donne pas la main sur le VRR en HDMI et la Shield n'en a pas ; le réglage est grisé. Sur un écran à fréquence variable (téléphones et tablettes à 120 Hz), chaque image part dès qu'elle est décodée, la plus récente remplace les autres.
- **HDR** : impossible avec Kyber 0.29 (capture en 8 bits), comme sur les autres apps.

## Son

Opus décodé par le moteur du cœur (`apps/core/src/audio`), tiré par AAudio en mode faible latence. Auto suit la sortie : stéréo, 5.1 ou 7.1 en PCM sur un ampli HDMI ; le retard de la sortie (Bluetooth surtout) est mesuré et signalé au-delà de 40 ms ; le son peut rester sur l'ordinateur.

## Manettes et vibrations

Les manettes arrivent par les événements d'Android (`KeyEvent`, `MotionEvent`) et partent au cœur comme sur l'Apple TV (`gamepads_from_app`) : kynput n'utilise pas SDL ici.

Les vibrations du jeu arrivent déjà au cœur (`InputPlatform::rumble`, fil réseau de kynput) ; l'app les joue sur la manette du joueur :

- **Android 12 et après** : `InputDevice.getVibratorManager()` donne chaque moteur de la manette ; le gros moteur grave à gauche et le petit aigu à droite, comme XInput.
- **Android 11 (Shield)** : `InputDevice.getVibrator()`, un seul vibreur quand la manette en expose un, à l'intensité la plus forte des deux moteurs. Beaucoup de manettes Bluetooth n'y vibrent pas ; pas de pilote USB maison.
- L'écran de test affiche pour chaque manette « 2 moteurs », « 1 moteur » ou « sans vibration », et fait vibrer chaque moteur à part.

## Architecture

- **Cœur** : `kybernow-core` (session, Admin, profils, latence, entrées, son), compilé pour Android par le NDK. Kyber prévoit Android dans kyclient, kynput et kyvlcplayer (`target_os = "android"`).
- **Pont JNI** : une crate `cdylib` au-dessus du cœur, l'équivalent de `apps/apple` pour Kotlin, avec libkynput, libopus et les fonctions vides de VLC (référencées par kyvlcplayer, jamais appelées : l'image et le son vont à l'app) dans la même bibliothèque.
- **Certificats** : sur Android, kyclient vérifie les certificats par `rustls-platform-verifier`, à initialiser une fois depuis Kotlin avec le contexte de l'app (`kyclient_init_platform_verifier`) ; la partie Android du vérificateur est embarquée dans l'APK.
- **App** : Kotlin, Jetpack Compose (Compose for TV pour le focus), `SurfaceView` et MediaCodec pour l'image, AAudio pour le son.

## Compiler

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-c --version 0.10.15 --locked
# NDK r27c : Android Studio › SDK Manager › SDK Tools › NDK (Side by side) 27.2.12479018,
# ou ANDROID_NDK_HOME vers un NDK décompressé.
apps/android/scripts/build-core.sh                 # les trois cibles
apps/android/scripts/build-core.sh --target aarch64-linux-android --check
```

`scripts/build-core.sh` applique les correctifs d'`apps/macos/kyber-patches`, compile libkynput sans SDL dans `third_party/kyber-desktop/rootfs-<cible>` (`cargo cinstall`), pose un `libvlc.pc` sans bibliothèque (vlc-rs, dépendance de kyvlcplayer sur Android, ne fait que le demander à pkg-config ; les fonctions de VLC, jamais appelées, seront des fonctions vides dans la bibliothèque JNI, comme sur iOS), puis compile le cœur pour chaque cible, avec le clang du NDK pour API 30 (compilateur C, éditeur de liens, et `--sysroot` pour bindgen). Vérifié le 22/10/2026 avec rustc 1.97.0 et le NDK r27c, sur Linux : les trois cibles compilent (une minute et demie chacune, libkynput comprise). Les sources de Kyber 0.29 vont dans `third_party/kyber-desktop` (voir [docs/build-notes.md](../../docs/build-notes.md)).

## Reste à faire

- Le pont JNI, puis l'app Kotlin.
- Côté Station : relayer les ordinateurs du téléphone vers le téléviseur (code affiché au téléviseur).
- Sur la Shield : quelles manettes vibrent en Bluetooth sous Android 11, le 1080p à 120 Hz de bout en bout, le mode jeu du téléviseur.
