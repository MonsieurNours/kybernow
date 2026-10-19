# KyberNow pour macOS

Client de streaming de KyberNow : on retrouve son ordinateur (serveur Kyber 0.29 + console KyberNow) en plein écran sur le Mac, avec l'image, le son, le clavier, la souris et les manettes. Maquettes : [Claude Design](https://claude.ai/artifact/Fxxqv9kPrJsxtcpX7YnNAG). Le code vidéo, audio et la détection des capacités n'utilisent pas AppKit : ils serviront tels quels aux apps iOS et Apple TV.

## Ce que fait l'app

- **Accueil** : l'ordinateur utilisé en dernier en grand, les autres à côté, en ligne ou non (sondage de la console toutes les 8 s).
- **Ajouter un ordinateur** : sur le réseau local (annonce Bonjour `_kybernow._tcp` de la console), par adresse et port de la console, ou par QR code (caméra du Mac) ou lien `kybernow://adresse:port?name=…&fp=sha256:…` affichés dans la console, page Appareils. L'empreinte du certificat de la console est vérifiée : automatiquement si le lien ou l'annonce la donne, sinon à l'œil (les quatre premiers groupes). Puis le compte, vérifié auprès de la console et gardé dans le Trousseau. Un lien `kybernow://` ouvert ailleurs ouvre directement l'ajout.
- **Réglages par ordinateur** : profils Jeu, Bureau et Max (ajuster un réglage rend le profil « Personnalisé ») ; écran de l'ordinateur (comme cet écran, taille de la fenêtre, sur mesure, ou ses écrans tels quels) ; format (H.264, HEVC, AV1), 4:4:4, synchro verticale, débit de 5 à 4 294 Mbit/s avec avertissements ; transport (rapide ou fiable), reconnexion ; ⌘ envoyée comme Ctrl, son (Auto, stéréo, 5.1, 7.1, aucun) ; carte « Ce Mac ».
- **Session** : scène de connexion animée (Core Animation), apparition de l'image ; barre au survol du haut de l'écran (profil, plein écran, déconnexion, mesures) ; fin propre avec vérification que l'ordinateur a retrouvé ses écrans.

| Raccourci | Effet |
|---|---|
| ⌃⌥⌘D | Se déconnecter |
| ⌃⌥⌘F | Plein écran ou fenêtre |
| ⌘ + touche | Ctrl + touche sur l'ordinateur (réglable) |
| L1 + R1 + START + SELECT | La manette pilote la barre de l'app (croix, A, B) ; même combinaison ou barre fermée : elle revient à l'ordinateur |

⌃⌥⌘ + lettre reste au Mac (menus de l'app) et ne part jamais vers l'ordinateur.

## Image et son

Kyber dessine normalement l'image et joue le son avec VLC. KyberNow les décode lui-même pour la latence et la synchro : `kyber-patches/0001` ajoute à libkyclient une sortie des paquets vers l'app (voir [docs/kyber-changes.md](../../docs/kyber-changes.md)).

**Image** (`VideoDecoder`, `VideoRenderer`, `VideoPipeline`)
- VideoToolbox, décodeur matériel exigé (logiciel en dernier recours), mode temps réel, une image par appel, sans file d'attente.
- Sortie dans les formats compressés sans perte des puces Apple (`&8v0`, `&8v4`), lus sans copie par Metal (`CVMetalTextureCache`). Conversion YCbCr → RGB dans le shader, selon la matrice annoncée par le flux (709, 601, 2020) et sa plage.
- Affichage : toujours l'image la plus récente, au plus une image en attente d'écran. Sur un écran à fréquence variable (ProMotion, Adaptive-Sync), l'écran se cale sur l'arrivée des images. Synchro verticale désactivable : latence la plus basse, déchirure possible.
- AV1 : encore décodé par VLC (description `av1C` à écrire).

**Son** (`AudioPlayer`)
- libopus (multistream : stéréo, 5.1, 7.1), masquage des pertes par Opus.
- Sortie Core Audio (`DefaultOutput` sur Mac, `RemoteIO` sur iOS/tvOS), demandes de 5 ms à la carte son.
- Tampon d'une vingtaine de millisecondes, contre 50 ms fixes chez VLC : sa marge grandit après un trou, redescend d'une milliseconde toutes les dix secondes sans trou, et l'excédent durable est jeté (dérive entre les horloges du PC et du Mac).
- Son et image partent chacun dès qu'ils sont prêts. La barre montre l'écart (son = tampon + latence de la sortie, image = réception → écran), imperceptible sous 45 ms. Un casque Bluetooth ajoute 150 à 250 ms côté son.

**Capacités de la machine** (`DecoderCapabilities`, `DisplayInfo`)
- Au lancement : décodeurs matériels H.264, HEVC et AV1, plus un vrai décodage d'une image HEVC 4:4:4 de 64 × 64 en exigeant le matériel.
- Les profils s'y ramènent : sans 4:4:4 matériel, 4:2:0 avec un tiers de débit en plus ; sans HEVC, H.264 avec moitié plus. Les réglages grisent ce que la machine ne décode pas.
- Écran : définition réelle (5K, 6K au pixel près, jusqu'à 7680 × 4320 ; 4096 de large au plus en H.264, limite de NVENC), option « Au plus 4K », fréquence maximale, fréquence variable ou non.

Mesures en direct (barre de session) : images par seconde, décodage, réception → écran, images remplacées, tampon et sortie du son.

## Organisation du code

```
apps/macos/
├── project.yml            # projet XcodeGen (cibles KyberKit et KyberNow)
├── KyberKit/              # framework : façade Swift du SDK Kyber
│   ├── KyberClient        # connexion, session, événements, options
│   ├── ConsoleClient      # API de la console (prepare, heartbeat, end, info), empreinte épinglée
│   ├── VideoDecoder / VideoRenderer / VideoPipeline / DecoderCapabilities
│   ├── AudioPlayer
│   ├── StreamView         # vue de session : surface Metal, clavier, souris, curseur
│   ├── InputPipeline, Keymap, GamepadGate   # entrées via libkynput
│   └── KyberLog           # journaux vers le journal unifié
├── KyberNow/              # app SwiftUI + AppKit
│   ├── KyberNowApp, Model (ordinateurs, réglages, Trousseau)
│   ├── HomeView, AddHostFlow, SettingsView
│   ├── SessionController, SessionChrome, ConnectScene
│   └── Theme, DisplayInfo, Fonts/ (Bricolage Grotesque, IBM Plex, OFL)
├── kyber-patches/         # correctifs KyberNow sur libkyclient
├── scripts/               # vendor-kyber.sh, build-kyclient.sh, embed-kyber.sh
├── tests/                 # tests hors ligne (vidéo, son)
└── Vendor/                # SDK Kyber compilé (ignoré par git)
```

Déroulé d'une session : `ConsoleClient.prepare` (écran virtuel à la taille de ce Mac, son au bon nombre de voies) → `KyberClient.connect` → attente que la liste d'écrans se stabilise → `startStreaming` → signal de vie toutes les quelques secondes → `ConsoleClient.end` (l'ordinateur retrouve ses écrans et sa sortie audio).

## Construire

Prérequis : Xcode 27, XcodeGen, Rust 1.89 + `cargo-c`, le client Kyber 0.29 compilé dans `third_party/kyber-desktop` (voir [docs/build-notes.md](../../docs/build-notes.md)).

```bash
apps/macos/scripts/vendor-kyber.sh      # SDK Kyber (en-têtes, bibliothèques, plugins VLC) → Vendor/
apps/macos/scripts/build-kyclient.sh    # libkyclient avec les correctifs KyberNow → Vendor/
cd apps/macos && xcodegen generate
xcodebuild -project KyberNow.xcodeproj -scheme KyberNow -configuration Debug -derivedDataPath build build
open build/Build/Products/Debug/KyberNow.app
```

`build-kyclient.sh` est à relancer après chaque `vendor-kyber.sh` : sans lui, l'app reprend une libkyclient sans les correctifs. Signature : certificat « Apple Development: info@leslignesclaires.fr », équipe P6DB682S4K (`project.yml`). Elle reste la même d'une compilation à l'autre, ce qui évite que le Trousseau redemande l'accès au mot de passe ; prévoir un Developer ID pour distribuer.

## Tester sans ordinateur distant

```bash
apps/macos/tests/video/run.sh   # flux HEVC/H.264 générés par ffmpeg : décodage, temps, couleurs rendues
apps/macos/tests/audio/run.sh   # Opus stéréo et 5.1 au rythme réel, à-coups et pertes : tampon, trous
```

Mesuré sur M4 Max : 1 à 2 ms de décodage en 2560 × 1440 (4:2:0), 1,6 à 2,7 ms en 4:4:4, couleurs à ±4 niveaux près ; son : 15 à 23 ms de tampon en paquets de 20 ms, 14 ms en paquets de 10 ms (serveur avec 0004), avec ± 8 ms d'à-coups simulés.

## Journaux

Console.app, sous-système `fr.leslignesclaires.kybernow`, catégories `app`, `kyber` (SDK), `video`, `audio`. En ligne de commande :

```bash
log stream --predicate 'subsystem == "fr.leslignesclaires.kybernow"' --level info
```

## Données

- Ordinateurs et réglages : `UserDefaults` (clé `hosts`), relus avec tolérance (un réglage ajouté garde sa valeur par défaut).
- Mots de passe : Trousseau, service `fr.leslignesclaires.kybernow`, un élément par ordinateur.

## Vers iOS et Apple TV

Réutilisables tels quels : `VideoDecoder`, `VideoRenderer`, `VideoPipeline`, `DecoderCapabilities`, `AudioPlayer`, `ConsoleClient`, `GamepadGate`. À écrire : une vue UIKit qui porte la couche Metal (équivalent de `MetalSurface`), les entrées tactiles et télécommande, et la compilation de libkyclient, libkynput et libopus pour iOS et tvOS (`kysdk/kyctl/build-ios.sh` sert de point de départ). Il n'y a ni VRR ni synchro désactivable sur Apple TV.
