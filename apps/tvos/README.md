# KyberNow pour l'Apple TV

[English](README.en.md) · Français

Client de streaming de KyberNow sur le téléviseur : on retrouve son ordinateur (serveur Kyber 0.29 + console KyberNow) en plein écran, à la télécommande ou à la manette, avec l'image en 4K à 60 Hz au plus (la limite de l'Apple TV), le son multicanal, et jusqu'à quatre manettes. Identité v2 ([charte](../../brand/v2/README.md)) ; maquette suivie à l'écran près : [Claude Design](https://claude.ai/artifact/RfGbfguTyAP1jjwFASVRyK) (accueil, ajout, Top Shelf, passage, session, manettes, réglages).

**Installer l'app sans l'App Store** : [SIDELOAD.md](SIDELOAD.md).

Apple TV 4K de 2ᵉ génération (A12) et suivantes, tvOS 18 au moins. HEVC et H.264 en matériel ; pas d'AV1 (aucune de ces puces ne le décode en matériel).

## Ce que fait l'app

- **Accueil** (`HomeView`) : l'ordinateur choisi occupe tout l'écran, sa dernière image en fond qui dérive lentement ; son nom en grand, en ligne ou non (réseau local ou VPN), système, carte graphique et adresse, l'écran virtuel qui l'attend ; « Se connecter » (allumé au focus) et les profils Jeu, Bureau et Max. À droite, « Prêt à jouer » : les manettes branchées et leur batterie, la sortie du son (ampli HDMI 7.1, téléviseur, AirPods…) et son retard, l'image de la session et la latence de la dernière. En bas, les ordinateurs : le focus d'une carte change la scène ; ceux annoncés sur le réseau (Bonjour) s'y ajoutent, et l'ajout. Les glyphes d'aide suivent la dernière manette utilisée (télécommande, A / B, ✕ / ○).
- **Focus tvOS** : la veille du néon de la charte. L'élément focalisé grossit (× 1,06), se soulève et s'entoure de givre et d'un halo Spectre (`GlassButton`, `ChoiceButton`, `PrimaryButton` dans `Theme.swift`).
- **Ajouter un ordinateur** (`AddHostView`), sans rien taper à la télécommande de préférence :
  - **Depuis votre Mac** : les ordinateurs publiés par l'app Mac dans iCloud (magasin clé-valeur, clé `hosts`) arrivent avec leur mot de passe (Trousseau iCloud, service `fr.leslignesclaires.kybernow.cloud`) ; ajoutés tout seuls si « Synchroniser avec iCloud » est actif (`CloudSync`). L'app Mac les publie à chaque ajout, modification ou retrait (`CloudPublisher`), si elle est signée avec les droits iCloud (voir [apps/macos/README.md](../macos/README.md#signature-et-icloud)).
  - **Sur le réseau** : annonce Bonjour `_kybernow._tcp` de la console (sauf `kyber=0` : console sans serveur Kyber derrière), puis le compte. tvOS propose la saisie au clavier de l'iPhone et le Trousseau. L'empreinte du certificat vue doit être celle de l'annonce.
  - **Code à l'écran** : l'Apple TV tire un code à 6 chiffres et le présente à toutes les consoles trouvées (`POST /api/pair/request`, puis `GET /api/pair/request/{id}`, dans `ConsoleClient`) ; on l'accepte dans la console de l'ordinateur (Accès › Appareils, ou le panneau de l'icône près de l'horloge) ; l'Apple TV reçoit un compte Kyber à elle, révocable dans Appareils (voir [server/README.md](../../server/README.md)). QR code de la console pour l'ouvrir au téléphone. Une console d'une version précédente ne connaît pas ces appels : l'app le dit (« mettez KyberNow à jour sur l'ordinateur »).
  - **Adresse** : adresse et port de la console (VPN), puis le compte, avec les quatre premiers groupes de l'empreinte à comparer.
- **Top Shelf** (extension `TopShelf/`) : quand KyberNow est dans la rangée du haut de l'accueil de l'Apple TV, « Reprendre sur un ordinateur » montre chaque ordinateur avec sa dernière image ; un clic ouvre directement la session (`kybernow-tv://connect/<id>`). L'app y dépose la liste et les images, dans le conteneur du groupe `group.fr.leslignesclaires.kybernow`.
- **Le passage** (`SessionView`, `Passage`) : comme sur Mac, l'accueil se coupe le long de la couture à 45°, le tube néon s'allume, puis les deux moitiés s'écartent sur l'image. Capsule « Vous entrez dans… » : comment ouvrir le menu, l'écran, le son et les joueurs branchés, le temps jusqu'à la première image, l'écart en direct.
- **En session** : l'image vit dans un `GCEventViewController`. Menu fermé, la manette et la télécommande ne parlent qu'au jeu (le bouton Retour ne fait pas quitter l'app). **Menu** par le bouton Retour de la télécommande ou L1 + R1 + Start + Select (comme sur Mac), chacun désactivable : l'image recule (× 0,94, assombrie), la santé du flux en haut (latence, images par seconde, débit, format, pertes), et Reprendre, Manettes, Son (sortie et son retard, choix AirPods / HomePod / téléviseur), Télécommande en souris, Taper du texte (clavier tvOS ou iPhone, envoyé caractère par caractère), Se déconnecter. Pendant le menu, les manettes pilotent KyberNow et le jeu ne reçoit rien (touches tenues relâchées pour lui).
- **Son** : Opus décodé par l'app (`AudioPlayer` de KyberKit, RemoteIO) ; Auto suit la sortie (stéréo, 5.1 ou 7.1 en LPCM sur un ampli HDMI) ; audio spatial dans les AirPods (contenu multicanal) ; ou le son reste sur l'ordinateur. Alerte quand la sortie ajoute du retard (AirPods, HomePod, AirPlay : au-delà de 40 ms), avec le retard mesuré.
- **Manettes** (`GamepadBridge`, `GamepadsView`) : GameController au lieu de SDL. Xbox, DualSense, DualShock et MFi ; chaque manette est un joueur sur l'ordinateur (J1 à J4), dans l'ordre d'allumage ou à place fixe ; voyant du joueur, barre lumineuse des manettes Sony à la couleur du joueur. Identifiants USB envoyés comme ceux que SDL donne sur Mac : l'ordinateur choisit la même manette virtuelle que pour l'app Mac. Vibrations du jeu sur les deux moteurs (Core Haptics, poignées gauche et droite). Écran de test : touches, sticks, gâchettes en direct, vibration d'essai. Le bouton Home reste à tvOS.
- **Manette en souris** (tuile du menu de session, active d'office en profil Bureau) : stick gauche = pointeur (accélération progressive, LT / L2 pour la précision, RT / R2 pour aller vite, vitesse dans Réglages › Manettes), stick droit = molette, A / ✕ clic gauche, X / □ clic droit, B / ○ clic du milieu, croix = flèches, L1 / R1 = Alt + Tab (Alt tenu tant qu'on enchaîne, Maj en plus avec L1), Start = touche Windows, Select = Échap. Les touches de la combinaison du menu agissent au relâchement, sauf si la combinaison a été formée. Le jeu ne reçoit plus la manette ; coupée, elle redevient une manette, rien ne reste enfoncé.
- **Télécommande en souris** (tuile « Pavé en souris ») : pavé = déplacements relatifs, clic = clic gauche, Lecture/Pause = clic droit ; toujours active en profil Bureau. **Clavier et souris Bluetooth** (`KeyboardMouse`) : touches en usages HID converties en scancodes par kynput, souris en relatif. Dès qu'une souris sert, l'app dessine le pointeur de l'ordinateur à sa place, deux fois plus grand qu'au bureau (lisible à trois mètres).
- **Réglages** (`SettingsView`), par ordinateur : profil ; image (Auto ou 4K, 1440p, 1080p ; 60, 50 ou 30 Hz ; débit de 5 à 4 294 Mbit/s avec avertissement au-delà de 150 ; HEVC ou H.264 ; 4:4:4 si l'Apple TV le décode en matériel ; caler le téléviseur sur la fréquence de la session) ; son ; manettes et télécommande ; réseau (transport, reconnexion, Ethernet ou Wi-Fi) ; compte et iCloud (synchro, dernière image, retirer l'ordinateur).
- **Profils tvOS** : l'app tourne pour l'utilisateur tvOS en cours (`com.apple.developer.user-management` : `runs-as-current-user`) : chaque membre du foyer a ses ordinateurs, ses réglages et son Trousseau. On change d'utilisateur dans le Centre de contrôle de l'Apple TV (tvOS ne laisse pas une app le faire, ni connaître son nom).
- **Session** (`SessionController`) : même déroulé que sur Mac (préparation par la console avec l'écran du téléviseur ramené à 4K60, signal de vie, reconnexion, écran virtuel redemandé, repli quand l'image ne passe pas, fin propre et dernière image gardée).

## Code partagé avec le Mac

KyberKit est le même module que sur Mac. Le projet prend dans `apps/macos/KyberKit` tout ce qui ne dépend pas d'AppKit (connexion, décodage VideoToolbox, rendu Metal, son, latence, console) et le complète par `KyberKitTV/` : vue de session UIKit (`StreamView`, `MetalSurface`), manettes et télécommande (`GamepadBridge`), clavier et souris (`KeyboardMouse`). Les rares différences sont sous `#if os(...)` dans les fichiers communs (pas de presse-papiers sur tvOS, manettes produites par l'app, présentation des images au rafraîchissement suivant). Le modèle (`Model.swift` : ordinateurs, réglages, profils) et la découverte (`Discovery.swift`) viennent de l'app Mac.

## Compiler

```sh
# SDK de Kyber pour tvOS (une fois, puis après un changement de kyber-patches) :
rustup toolchain install nightly --profile minimal --component rust-src
scripts/build-kyber-tvos.sh

# Projet Xcode (XcodeGen) :
xcodegen generate
open KyberNowTV.xcodeproj
```

`scripts/build-kyber-tvos.sh` compile libkynput et libkyclient de Kyber 0.29, avec les correctifs de l'app Mac (`apps/macos/kyber-patches`), pour l'Apple TV et son simulateur (cibles Rust de rang 3 : `-Zbuild-std`, comme le script `build-ios.sh` de Kyber), ainsi que libopus. VLC n'est jamais appelé (l'app décode l'image et le son) : des fonctions vides satisfont l'éditeur de liens. Le tout va dans `Vendor/KyberSDK.xcframework` (arm64 seulement).

Autres scripts : `scripts/phosphor-icons.mjs` (icônes Phosphor du catalogue), `scripts/app-icon.py` (icône en couches pour la parallaxe et images du Top Shelf, depuis la géométrie de `brand/v2`).

Tests à la télécommande (`UITests/`, XCUIRemote) : atteindre les réglages depuis l'accueil, garder un formulaire après le clavier. `xcodebuild test -scheme KyberNowTV -destination 'platform=tvOS Simulator,name=…'`.

En Debug dans le simulateur : `-demo YES` ajoute trois ordinateurs de démonstration ; `-screen add|settings|gamepads|menu|passage` ouvre directement un écran, `-addMethod 0…3` un volet de l'ajout (vérification sans télécommande).

## Signature

Équipe P6DB682S4K, signature automatique. Identifiant `fr.leslignesclaires.kybernow` (le même que l'app Mac : Trousseau et iCloud partagés), extension `fr.leslignesclaires.kybernow.topshelf`. Capacités à activer sur l'identifiant d'app : App Groups (`group.fr.leslignesclaires.kybernow`), iCloud (magasin clé-valeur), User Management.

## Reste à faire

- Valider une session de bout en bout sur une Apple TV (image, son multicanal, manettes, vibrations).
- Vérifier sur une vraie Apple TV que le Trousseau iCloud y apporte bien les mots de passe publiés par le Mac.
- Distribution (TestFlight ou ad hoc).
