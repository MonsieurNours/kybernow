# KyberNow pour l'iPhone et l'iPad

[English](README.en.md) · Français

Client de streaming de KyberNow sur l'iPhone et l'iPad : on retrouve son ordinateur (serveur Kyber 0.29 + console KyberNow) en plein écran, au doigt, au clavier et au trackpad de l'iPad, ou à la manette. Identité v2 ([charte](../../brand/v2/README.md)) ; maquettes : [Claude Design](https://claude.ai/artifact/4ef5m4JSRnzuitJcuCFjMX) (accueil, ajout, passage, session iPhone et iPad, mesures, réglages, états d'exception, icône, lancement).

iOS et iPadOS 18 au moins. Même identifiant que les apps Mac et Apple TV (`fr.leslignesclaires.kybernow` : Trousseau et iCloud partagés) ; distribution par TestFlight (interne). Première version de l'app (jalon 5 de [docs/coeur-apple.md](../../docs/coeur-apple.md)), écrite sur le cœur Rust dès le départ, à essayer sur un appareil.

## Ce que fait l'app

- **Accueil** : l'ordinateur choisi en plein cadre (sa dernière image, un aperçu demandé à sa console, sinon sa signature), son état (en ligne, réseau local ou VPN, hors ligne : image désaturée et « Réessayer »), le profil sous le bouton, la dernière session en pied ; les autres ordinateurs en cartes (appui long : Réglages, Retirer), ceux trouvés sur le réseau. Sur iPad, la colonne « Prêt à jouer » (manettes, sortie du son, dernière latence), qui disparaît en largeur réduite.
- **Ajout** : depuis le Mac (iCloud), sur le réseau (Bonjour), code à six chiffres accepté dans la console, adresse, ou QR code à la caméra (lien `kybernow://`, aussi reçu par AirDrop ou Messages) ; empreinte vérifiée toute seule quand le lien ou l'annonce la donne.
- **Lancement** : l'image fixe du système (encre, symbole éteint), puis 1,8 s d'allumage dans l'app ; avec « Réduire les animations », un fondu.
- **Passage** : l'accueil coupé le long de la couture à 45°, le tube néon qui grésille puis s'allume, les moitiés qui s'écartent sur l'image ; la capsule « Vous entrez dans… » en bas, au pouce, avec Annuler. Sur iPhone, la session est toujours en paysage.
- **Session** : l'image plein cadre, la languette en haut (latence en direct), la barre au toucher ou en glissant depuis le bord haut (latence et courbe, profil, son, pavé tactile ou toucher direct, souris Auto / Bureau / Jeu, écran, Touches, Quitter). Pavé tactile : un doigt déplace le pointeur de l'ordinateur, toucher = clic, deux doigts = clic droit, deux doigts qui glissent = molette, maintenir = glisser. Toucher direct : le doigt est le pointeur. Bouton du clavier au pouce droit ; au-dessus du clavier, une rangée de touches (Échap, Tab, Ctrl, Alt et ⊞ collants, flèches, F1 à F12, Ctrl + Alt + Suppr) ; le texte part en Unicode. Menu Touches (Ctrl + Alt + Suppr, Alt + Tab, ⊞ + D, Impr. écran, Alt + F4, ⊞ + L, gestionnaire des tâches, F11). Manettes Bluetooth (L1 + R1 + Start + Select ouvre la barre et la manette la pilote), clavier et souris ou trackpad de l'iPad (pointeur iPadOS caché, celui de l'ordinateur dessiné). Mesures détaillées en feuille sur iPhone, en panneau sur iPad.
- **États** : en capsule en haut de l'image, jamais en alerte : reconnexion (néon allumé, Quitter), mode de l'ordinateur changé (« Remettre » en profil Bureau), repli quand l'image ne passe pas, manette branchée.
- **Écran externe** (USB-C ou AirPlay, iPhone comme iPad) : l'image passe sur l'écran branché, à sa définition (« comme cet écran » vise alors l'externe, l'écran virtuel est refait à sa taille), et l'appareil devient le pupitre : toute sa surface est le pavé tactile, la barre reste ouverte, les mesures sont affichées sur iPad. Branché en cours de session, l'image y passe sans couper la session ; débranché, elle revient sur l'appareil (« L'image est revenue sur l'iPad »). Hors session, l'écran externe montre le symbole et une phrase d'attente.
- **Stage Manager** : une seule fenêtre ; la session vit dans sa fenêtre, l'image garde son format (bandes d'encre), la définition reste celle choisie au départ. Perdre le premier plan (autre fenêtre, autre app) relâche tout sur l'ordinateur, plafonne le débit et fige l'image (« KyberNow continue en arrière-plan »).
- **Arrière-plan** : tout est relâché sur l'ordinateur, le débit plafonné, l'image figée ; au retour, tout revient.
- **Réglages** : par ordinateur (image, son, entrées, réseau, compte), et ceux de l'app.

## Code

Le module KyberKit est celui du Mac et de l'Apple TV, cœur compris : la session (`SessionEngine`), la console (`ConsoleClientCore`), les entrées (`InputCore` : chaîne kynput, pointeur tenu par l'app, clavier, combinaison, presse-papiers par `UIPasteboard`), le son (`AudioPlayerCore`, sortie RemoteIO), la latence, le décodage VideoToolbox et le rendu Metal viennent d'`apps/macos/KyberKit` ; la surface Metal, le clavier et la souris Bluetooth (`KeyboardMouse`) et les manettes (`GamepadBridge`) de l'Apple TV (`apps/tvos/KyberKitTV`). Propre à iOS : `KyberKitIOS/StreamView.swift`, la vue de session tactile (gestes, clavier de l'écran en `UIKeyInput`, touches collantes, survol au trackpad), qui sait montrer l'image dans une `ImageHostView` posée sur un autre écran (`showImage(in:)`).

L'app (`KyberNowIOS/`) : `AppModel`, `Platform` (écran, son, préférences, images des ordinateurs), `Theme` (jetons de l'Apple TV à l'échelle du pouce), `HomeView`, `LaunchView`, `AddHostView`, `SettingsView`, `AppSettingsView`, `SessionController` (la session vue de l'interface, sur `SessionEngine`), `SessionView` (passage, barre, rangée de touches, menu Touches, capsules, pupitre), `SessionStats`, `ExternalDisplay` (scène de l'écran externe, non interactive, déclarée par `AppDelegate`). Le modèle (`Model.swift`) et la découverte (`Discovery.swift`) viennent de l'app Mac ; la synchro iCloud (`CloudSync.swift`) et les images d'ordinateur (`Backdrops.swift`) de l'Apple TV.

## Compiler

```sh
# libkynput pour iOS (une fois) :
../core/scripts/build-apple.sh --target aarch64-apple-ios
../core/scripts/build-apple.sh --target aarch64-apple-ios-sim

# Le cœur Rust, son pont C, libopus et l'en-tête de kynput (après chaque changement de apps/core ou apps/apple) :
../apple/scripts/build-apple-mobile.sh ios

# Projet Xcode (XcodeGen) :
xcodegen generate
open KyberNowIOS.xcodeproj
```

`build-apple-mobile.sh ios` compile `kybernow-core` et `kybernow-apple` en bibliothèques statiques (iPhone et simulateur), y ajoute libkynput, libopus (compilée par cmake) et les fonctions vides de VLC que kyclient référence sans jamais les appeler, et range le tout dans `Vendor/KyberNow.xcframework`, avec `kybernow.h` (module `CKyberNow`) et `kynput.h` (module `CKyber`, mise en page de la vue et constantes des manettes).

En Debug dans le simulateur : `-demo YES` ajoute trois ordinateurs de démonstration ; `-skipLaunch YES` saute le lancement ; `-screen add|settings|session|bar` ouvre un écran (la session de démonstration montre une mire, sans ordinateur).

## Signature

Équipe P6DB682S4K, signature automatique, identifiant `fr.leslignesclaires.kybernow`. Capacités : iCloud (magasin clé-valeur), partage du Trousseau. Distribution : TestFlight, testeurs internes.

## Reste à faire

- Essayer une session sur un iPhone et un iPad réels (le simulateur ne fait qu'afficher une mire : pas de décodage matériel).
- Écran externe et Stage Manager : écrits, mais le simulateur de ce Mac ne permet pas de brancher un écran ; à essayer sur un iPad et un iPhone (USB-C, AirPlay). Si iOS recopie l'écran au lieu d'ouvrir la scène externe, passer `UIApplicationSupportsMultipleScenes` à `true` dans `project.yml`.
- Stage Manager : pas de bouton « plein écran » dans la barre (iPadOS ne laisse pas une app agrandir sa fenêtre).
- Lire le presse-papiers fait demander à iOS l'accord de coller, à chaque nouveau texte copié pendant la session.
- Envoi sur TestFlight (archive Release et `xcodebuild -exportArchive`), une fois l'app essayée.
