# KyberNow Link pour Android et Android TV

[English](README.en.md) · Français

KyberNow Link sur Android : une seule app pour le téléphone, la tablette, le téléviseur (Android TV, Google TV) et la console portable, qui retrouve l'ordinateur (KyberNow Station : serveur Kyber 0.29 + KyberNow Admin) en plein écran, au doigt, à la télécommande, au clavier et à la souris ou à la manette. Identité v2 ([charte](../../brand/v2/README.md)) ; maquettes : [Claude Design](https://claude.ai/artifact/Ug3n57UPSvsrA3Hd9UMawX) (accueil, menu de session, manettes et vibrations, réglages de l'image au téléviseur ; accueil, ajout et session au téléphone ; accueil à la tablette).

**État des essais** : vérifié le 22/10/2026 sur la NVIDIA Shield TV 2019, vers un PC sous Windows en Ethernet : session en 4K à 60 Hz en HEVC (61 images par seconde, son 7.1), découverte et ajout, menu de session, manette Xbox en Bluetooth, télécommande et manette en souris, fin de session ; Halo jouable. Les vibrations de la manette Xbox n'arrivent pas sur la Shield (voir « Vibrations »). Vérifié aussi : une session sur un téléphone Android. Pas encore essayés : le 1080p à 120 Hz, la mise à jour de l'app depuis une version publiée. La décision est dans [PROJECT.md](../../PROJECT.md) (« Android et Android TV »).

## Appareils

- **Android 11 au moins** (API 30) : la NVIDIA Shield ne dépasse pas Android 11.
- **Téléviseurs** : NVIDIA Shield d'abord, Google TV Streamer, Chromecast avec Google TV, téléviseurs sous Google TV. Le Chromecast avec Google TV et beaucoup de téléviseurs font tourner Android en 32 bits : l'APK porte `arm64-v8a`, `armeabi-v7a` (et `x86_64` pour l'émulateur).
- **Téléphones et tablettes**. La session est une fenêtre redimensionnable (Samsung DeX, ChromeOS), au clavier et à la souris. L'app ne gère pas d'écran externe à part : pas d'image envoyée seule à l'écran branché en USB-C, pas de mode pupitre.
- **Consoles portables** (Retroid, AYN Odin) et **manettes de téléphone** (Razer Kishi, Backbone) : la manette intégrée est un joueur comme une autre ; l'interface se pilote au focus, comme au téléviseur.
- **Matériel d'essai** : Shield TV 2019 « tube » (pas de port USB : manettes en Bluetooth seulement), Xiaomi Redmi A5 ; manettes DualSense, Xbox, Shield et Switch Pro. Rien pour essayer une tablette, une console portable, DeX ni ChromeOS.

## Ce que fait l'app

- **Accueil** (`ui/home`) : l'ordinateur choisi en plein cadre (sa dernière image, sinon l'aperçu d'Admin ; zoom très lent, désaturée hors ligne), son état (réseau local, VPN, à distance, hors ligne ; sondage d'Admin toutes les 8 s ; compte refusé ou certificat changé signalés sous le nom), système, carte graphique et adresse ; « Se connecter » et les profils Jeu, Bureau, Max ; colonne « Prêt à jouer » au téléviseur et à la tablette (manettes et leurs moteurs, sortie du son, mode de l'écran et fréquence possible, décodeurs, dernière session) ; les autres ordinateurs (appui long : Réglages, Retirer), ceux trouvés sur le réseau, et l'ajout. Hors ligne avec une adresse MAC connue : « Réveiller l'ordinateur » (Wake-on-LAN).
- **Ajout** (`ui/add`) : sur le réseau (`_kybernow._tcp` par NsdManager, TXT `name`, `fp`, `kyber` ; quand Android ne donne qu'une adresse IPv6 de lien local, l'adresse IPv4 est demandée à JmDNS puis par une requête mDNS directe, `model/MdnsLookup.kt`), code à six chiffres présenté à chaque Station trouvée et accepté dans Admin (compte Kyber propre à l'appareil ; onglet par défaut au téléviseur), adresse avec les quatre premiers groupes de l'empreinte à comparer, QR code à la caméra (téléphone, tablette ; CameraX et zxing, sans services Google) et lien `kybernow://` ouvert depuis une autre app. Hors code, l'ordinateur choisi demande ensuite le compte d'Admin (identifiant, mot de passe), ou un code à la place. Mots de passe chiffrés par une clé du Keystore d'Android.
- **Session** (`session/`) :
  - Le **passage** : l'accueil coupé le long de la couture à 45°, le néon, les moitiés qui s'écartent sur la première image ; capsule « Vous entrez dans… » avec l'étape en cours et Annuler.
  - **Image** plein cadre, à son format (bandes d'encre), sur une `SurfaceView`, dont le décodeur rend directement les images ; la vue d'interface et celle du pointeur sont retirées quand rien n'est à montrer.
  - **Téléviseur** : menu par le bouton Retour de la télécommande (réglable) ou L1 + R1 + Start + Select (aussi en manette en souris) : santé du flux en haut (latence, images par seconde, débit, mode, pertes), puis des tuiles qui tiennent toutes à l'écran (cinq par rangée) : Reprendre, autres profils, Son, Télécommande en souris, Manette en souris, Taper du texte, Touches, Mesures, et Se déconnecter en rouge, en dernier. Pendant le menu, les manettes pilotent l'app (A valide, B ferme, la croix déplace ; les autres boutons et le relâché du combo ne font rien) et l'ordinateur relâche tout.
  - **Téléphone et tablette** : languette (latence en direct) et barre (mesures, profils, pavé tactile ou toucher direct, souris Auto / Bureau / Jeu, clavier, Touches, Quitter) ; rangée de touches au-dessus du clavier de l'écran (Échap, Tab, Ctrl, Alt et ⊞ collants, flèches, F1 à F12, Ctrl + Alt + Suppr), texte envoyé en Unicode.
  - **États** en capsule, jamais en alerte : reconnexion (avec Quitter), image relancée, mode de l'ordinateur changé (« Remettre » en profil Bureau), repli, écran virtuel adapté, manette branchée.
  - **Fin** : « Se déconnecter » affiche aussitôt « Déconnexion… » pendant que l'ordinateur remet son écran et son son (quelques secondes) ; l'app revient à l'accueil au plus tard après 6 s. La dernière image est gardée (PixelCopy, 0,8 s au plus), la latence de la session (médiane, p95) et sa durée vont à l'accueil. Une session arrêtée par une erreur laisse une carte (l'erreur, l'écran ou le son de l'ordinateur non rétablis) et « Retour à l'accueil ».
- **Manettes** (`model/Gamepads.kt`, `session/Inputs.kt`) : quatre joueurs au plus dans l'ordre d'arrivée, une cinquième en attente ; annoncées à l'ordinateur à chaque reconnexion ; boutons, sticks, gâchettes (analogiques ou numériques) et croix en chapeau ; Retour et Menu d'une manette partent au jeu comme Select et Start ; bouton Guide (Jeu ; Appui long : Start tenu 600 ms vaut Guide ; Système : il reste à Android) ; manette en souris (stick gauche pointeur, RT plus vite, LT plus précis, stick droit molette, A / X / B clics, croix flèches, Start ⊞, Select Échap, L1 / R1 Alt + Tab ; Start, Select, L1 et R1 agissent au relâché et seulement seuls, pour laisser passer le combo ; Retour et Menu d'une manette Xbox en Bluetooth valent Select et Start). Un appareil n'est une manette que s'il a un stick (les télécommandes et appareils virtuels de la Shield qui se disent manettes sont écartés). Écran de test (`ui/gamepads`) : touches, sticks, gâchettes en direct, moteurs de chaque manette et vibration d'essai par moteur.
- **Clavier, souris, toucher** : clavier physique en scancodes (code Linux de `KeyEvent.getScanCode()` converti par kynput ; une touche sans code matériel part en texte), Ctrl + Alt + Maj + M ouvre le menu ou la barre, Ctrl + Alt + Maj + Q quitte ; souris en position (pointeur de l'ordinateur dessiné par l'app, toujours visible en télécommande ou manette en souris : Windows cache le sien sans souris branchée) ou capturée en mouvements bruts quand un jeu le veut (souris Auto) ; pavé tactile (un doigt = pointeur, toucher = clic, deux doigts = clic droit et molette, maintenir = glisser) ou toucher direct ; télécommande en flèches et Entrée, ou en souris.
- **Presse-papiers** texte dans les deux sens (téléphone, tablette) : le texte de l'appareil n'est lu que quand l'ordinateur colle (Android 12 affiche un avis à chaque lecture).
- **Arrière-plan** : tout est relâché sur l'ordinateur, le débit plafonné à 2 Mbit/s, l'image figée ; la surface retirée, le décodeur est refait au retour sur une image clé.
- **Mises à jour de l'app** (`update/Updates.kt`) : une recherche toutes les 6 heures au plus (au lancement, au retour dans l'app ou pendant qu'elle tourne, si la précédente date de 6 heures ; « Rechercher maintenant » dans les réglages de l'app) ; pastille « Version X » sur l'accueil ; téléchargement à la demande, vérifié par le manifeste signé, puis installation par PackageInstaller (voir « Mises à jour et publication »).
- **Rangée « Reprendre sur un ordinateur »** de l'accueil d'Android TV (`tv/TvChannel.kt`) : chaque ordinateur avec sa dernière image, prêtée au seul lanceur de l'accueil ; un clic ouvre la session (`kybernow://connect/<id>`). Google TV ne montre pas ces rangées.
- **Réglages** par ordinateur (`ui/settings`) : profil ; définition (comme cet écran, 4K, 1440p, 1080p, 720p), fréquence, format, débit, caler l'écran sur la session, mode jeu du téléviseur, 4:4:4 et VRR (information) ; son ; manette vue par l'ordinateur, combinaison, Retour, Guide ; souris, toucher, presse-papiers ; transport fiable, reconnexion, adresse MAC ; compte, retirer l'ordinateur. Réglages de l'app : langue (Auto, Français, English), décodeurs et écran de l'appareil, mises à jour (état, notes, canal stable ou bêta), licences.
- **Langues** : français et anglais (`res/values-fr`, `res/values`, produits par `scripts/strings.py`) ; français si Android est en français, anglais sinon ; choix par app (Android 13 et après dans les réglages d'Android aussi).

Pas de manette à l'écran, ni de mise en page propre aux pliables. Le téléviseur s'ajoute ses ordinateurs lui-même (réseau, code, adresse) : il ne reçoit pas ceux du téléphone.

## Image, fréquence et mode jeu

- **Mode de la session** : « comme cet écran », à la plus haute fréquence qu'il tient ; une fréquence demandée que la définition ne tient pas fait descendre la définition (la Shield, HDMI 2.0 : 4K à 60 Hz ou 1080p à 120 Hz) ; ramené à ce que le décodeur accepte (4096 de large au plus en H.264). L'écran virtuel de l'ordinateur est créé à ce mode.
- **Écran** : au téléviseur, la sortie HDMI prend le mode de la session (`preferredDisplayModeId`, réglage « Caler l'écran ») ; au téléphone, la même définition à la fréquence de la session (le 120 Hz se demande : Android 15 met les jeux à 60 Hz sans demande) ; la surface annonce sa fréquence (`Surface.setFrameRate`).
- **Mode jeu du téléviseur** : `Window.setPreferMinimalPostProcessing(true)` (ALLM sur l'HDMI) quand l'écran le permet.
- **VRR** : Android ne donne pas la main sur le VRR en HDMI et la Shield n'en a pas. Sur un écran à fréquence variable (téléphones et tablettes à 120 Hz), chaque image part dès qu'elle est décodée, la plus récente remplace les autres.
- **Décodage** (`jni/src/video.rs`) : les paquets du correctif 0001 vont à MediaCodec (décodeur matériel du format, `low-latency`, priorité temps réel, clés des fabricants Qualcomm, Exynos et Amlogic qui coupent la remise en ordre, réessayé sans elles si refusé), rendu direct sur la surface. Le jeu de paramètres est placé devant l'image clé qui démarre un décodeur (pas de `csd-0`, quel que soit le format) ; une perte fait attendre l'image clé suivante, redemandée par le cœur ; une surface neuve refait le décodeur. Profils ramenés à ce que l'appareil décode vraiment (MediaCodecList au lancement) : la Shield décode HEVC et H.264, pas l'AV1 ni le 4:4:4.
- **HDR** : impossible avec Kyber 0.29 (capture en 8 bits), comme sur les autres apps.

## Son

Opus décodé par le moteur du cœur (`apps/core/src/audio`), tiré directement par le rappel temps réel d'AAudio (`jni/src/audio.rs`), en mode faible latence et exclusif si possible, sans passer par Java. Ordre des voies de Windows (celui d'Android). Auto suit la sortie : 5.1 ou 7.1 sur un ampli HDMI, stéréo ailleurs ; une sortie qui refuse le multicanal reçoit la stéréo mixée par le moteur ; une sortie qui change (casque, HDMI) est rouverte. Le son peut rester sur l'ordinateur. Sur la Shield en Ethernet 100 Mbit/s, en 4K à 60 Hz, le son perd des paquets.

## Vibrations

Les vibrations du jeu arrivent au cœur (`InputPlatform::rumble`) et sont jouées sur la manette du joueur (`Gamepads.rumble`) :

- **Android 12 et après** : `InputDevice.getVibratorManager()`, deux moteurs (gros moteur grave à gauche, petit aigu à droite, comme XInput).
- **Android 11 (Shield)** : `InputDevice.getVibrator()`, un seul vibreur à l'intensité la plus forte des deux moteurs ; beaucoup de manettes Bluetooth n'y vibrent pas. Vérifié sur la Shield 2019 : la manette Xbox Wireless en Bluetooth est « sans vibration » (Android ne donne aucun vibreur), alors que l'ordinateur envoie bien les vibrations de Halo. Rien à faire côté app : la Shield « tube » n'a pas d'USB, et une app ne peut pas parler directement à une manette associée au système. La DualSense et la manette Shield n'y sont pas encore essayées.
- Sans durée, 5 s au plus (comme kynput sur Mac) ; (0, 0) arrête. L'écran de test dit pour chaque manette « 2 moteurs », « 1 moteur » ou « sans vibration ».
- Journal (`adb logcat -s KyberNow`, lignes `vibrations :`) : première vibration reçue de l'ordinateur, manette introuvable à ce rang, nombre de vibreurs qu'Android donne pour la manette.

## Architecture

```
apps/android/
├── jni/                 crate kybernow-android → libkybernow.so (pont JNI, MediaCodec, AAudio)
├── app/                 l'app (Kotlin, Jetpack Compose)
│   └── src/main/java/fr/leslignesclaires/kybernow/
│       ├── core/        Native (fonctions JNI), SessionListener, ConsoleClient (Admin), DeviceIdentity
│       ├── model/       ordinateurs et mots de passe, découverte, manettes, décodeurs, écran, son, Wake-on-LAN
│       ├── ui/          thème de la charte, icônes, accueil, ajout, réglages, test des manettes
│       ├── session/     SessionActivity, SessionState, entrées, interface de session, pointeur
│       ├── tv/          rangée de l'accueil d'Android TV
│       └── update/      mises à jour de l'app (recherche, téléchargement, PackageInstaller)
└── scripts/             build-native.sh, release-apk.sh (APK publié), strings.py (textes),
                         app-icon.py et brand-drawables.py (dessins de la charte)
```

- **Interface** : Jetpack Compose (Material 3) aux couleurs et polices de la charte ; le focus au téléviseur est celui de Compose lui-même (`ui/Theme.kt`, `FocusCard` et son néon), sans les composants de Compose for TV.
- **Pont JNI** (`jni/`, l'équivalent de `apps/apple` pour Kotlin) : objet Kotlin `fr.leslignesclaires.kybernow.core.Native`, objets du cœur en `Long`, réponses d'Admin en JSON (`{"code": 0, "value": …}`), mêmes paramètres de session que `kn_session_start` des apps Apple, plus `combo`. Les rappels vers Kotlin (`SessionListener`) passent par une référence globale ; chaque fil du cœur s'attache à la JVM et travaille dans un cadre de références locales. Tout le chemin chaud (image, son, chaîne d'entrées) reste en Rust.
- **VLC** : kyvlcplayer référence libvlc, jamais appelée (l'image et le son vont à l'app) : `jni/build.rs` compile une fonction vide par fonction `libvlc_*` de vlc-rs, comme sur iOS. Le script pose un `libvlc.pc` sans bibliothèque pour que vlc-rs le trouve.
- **Certificats** : le cœur donne toujours l'empreinte du certificat de Kyber (celle qu'Admin annonce) : kyclient la vérifie lui-même, sans le vérificateur d'Android (`rustls-platform-verifier`), qui n'est donc pas initialisé.
- **Dessins** : l'icône et la bannière d'Android TV sont celles de l'Apple TV (le symbole et son néon sur l'encre, la lueur Spectre et la couture en filigrane), produites par `scripts/app-icon.py` avec le dessin d'`apps/tvos/scripts/app-icon.py` ; logo signé, symbole et couche monochrome de l'icône convertis des SVG de `brand/v2` par `scripts/brand-drawables.py`, sans retouche. Polices Archivo et Martian Mono variables, de `brand/v2/fonts`.

## Compiler

```sh
# Une fois : cibles Rust, cargo-c, NDK r27c (Android Studio › SDK Manager › SDK Tools › NDK (Side by side)
# 27.2.12479018, ou ANDROID_NDK_HOME), SDK Android (plateforme 36), JDK 17 ou plus, cmake et Ninja (libopus).
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-c --version 0.10.15 --locked

# Bibliothèque native (après chaque changement d'apps/core ou d'apps/android/jni) :
apps/android/scripts/build-native.sh                     # les trois ABI
apps/android/scripts/build-native.sh --target aarch64-linux-android
apps/android/scripts/build-native.sh --check             # cargo check seul ; --debug : sans optimisation

# L'app :
cd apps/android
./gradlew assembleDebug        # app/build/outputs/apk/debug/app-debug.apk
./gradlew assembleRelease      # signée si KYBERNOW_ANDROID_KEYSTORE (et ses mots de passe) est donné
                               # (l'APK publié : scripts/release-apk.sh, voir plus bas)
./gradlew lintDebug
./gradlew recordRoborazziDebug  # captures des écrans principaux, sans appareil → app/screenshots/
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

`scripts/build-native.sh` applique les correctifs d'`apps/macos/kyber-patches`, compile dans `third_party/kyber-desktop/rootfs-<cible>` libkynput sans SDL (`cargo cinstall`) et libopus 1.6.1 (cmake et la chaîne du NDK ; sources du tag `v1.6.1` du dépôt d'Opus, clonées dans `third_party/opus-1.6.1`), pose `libvlc.pc`, puis compile `kybernow-android` avec le clang du NDK pour API 30 et copie `libkybernow.so`, sans informations de débogage, dans `app/src/main/jniLibs/<abi>/` (hors git). Les sources de Kyber 0.29 vont dans `third_party/kyber-desktop` (voir [docs/build-notes.md](../../docs/build-notes.md)). Versions : Android Gradle Plugin 8.13, Gradle 8.14, Kotlin 2.4, Compose (BOM 2026.06) ; numéro de version de l'app celui d'`apps/Cargo.toml`. Gradle déclare aussi Compose for TV 1.0 (`androidx.tv:tv-material`), qu'aucun écran n'emploie.

Captures (`app/src/test/.../Screenshots.kt`) : Robolectric en rendu natif et Roborazzi, avec des données de démonstration et une fausse image d'ordinateur, sans la bibliothèque native ; l'accueil (`HomeContent`), le test des manettes (`GamepadsContent`) et l'interface de session (`SessionOverlay`) reçoivent leurs données en paramètres pour cela. Robolectric télécharge une image d'Android au premier lancement (`android-all-instrumented`, 200 Mo, Maven Central).

**Essayer sur un appareil** (Shield ou téléphone branché à Android Studio, débogage USB ou réseau activé) : `git pull`, `apps/android/scripts/build-native.sh` si `apps/core` ou `apps/android/jni` ont changé, puis ▶ Run dans Android Studio (projet `apps/android`). Sans la bibliothèque native, Gradle refuse de construire (`libkybernow.so absente…`) : une app installée sans elle mourrait au lancement (`UnsatisfiedLinkError`). Le script prend le NDK 27.2.12479018 ; à défaut, le plus récent installé, avec un avertissement. Journal : `adb logcat -s KyberNow` (cœur Rust et app).

Textes : `scripts/strings.py` (les deux langues dans un seul fichier, typographie française comprise) écrit `res/values*/strings.xml`. Dessins : `scripts/app-icon.py` (icône, bannière ; Pillow) et `scripts/brand-drawables.py`.

## Mises à jour et publication

L'app se met à jour seule depuis les versions publiées (`KyberNow-Link-Android.apk`, vérifié par le manifeste signé) : réglages de l'app › Mises à jour, canal stable ou bêta. L'APK publié est construit et signé par `scripts/release-apk.sh`, que lance `scripts/release.sh` (`--check` : clé et outils ; `--fingerprint` : empreinte du certificat, celle de `scripts/release-cert.sha256`, que l'APK doit porter) ; clé de signature et sauvegarde : [docs/releases.md](../../docs/releases.md), « Clé de signature de l'app Android ». Une app installée depuis Android Studio (clé de débogage) ne peut pas recevoir ces mises à jour : la désinstaller une fois, puis installer l'APK publié (`adb install KyberNow-Link-Android.apk`).
