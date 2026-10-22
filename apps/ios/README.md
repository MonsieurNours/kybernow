# KyberNow Link pour l'iPhone et l'iPad

[English](README.en.md) · Français

KyberNow Link, le client de streaming de KyberNow sur l'iPhone et l'iPad : on retrouve son ordinateur (KyberNow Station : serveur Kyber 0.29 + KyberNow Admin) en plein écran, au doigt, au clavier et au trackpad de l'iPad, ou à la manette. Identité v2 ([charte](../../brand/v2/README.md)) ; maquettes : [Claude Design](https://claude.ai/artifact/4ef5m4JSRnzuitJcuCFjMX) (accueil, ajout, passage, session iPhone et iPad, mesures, réglages, états d'exception, icône, lancement).

iOS et iPadOS 18 au moins. Même identifiant que les apps Mac et Apple TV (`fr.leslignesclaires.kybernow` : Trousseau et iCloud partagés) ; pas encore distribuée (TestFlight interne à venir). **Installer l'app sans TestFlight ni App Store** : [SIDELOAD.md](SIDELOAD.md). L'app repose sur le cœur Rust, comme les apps Mac et Apple TV ([docs/coeur-apple.md](../../docs/coeur-apple.md)).

## Ce que fait l'app

- **Accueil** : l'ordinateur choisi en plein cadre (sa dernière image, un aperçu demandé à Admin, sinon sa signature), son état (en ligne, réseau local ou VPN, hors ligne : image désaturée et « Réessayer »), le profil sous le bouton, la dernière session en pied ; les autres ordinateurs en cartes (appui long : Réglages, Retirer), ceux trouvés sur le réseau. Sur iPad, la colonne « Prêt à jouer » (manettes, sortie du son, dernière latence), qui disparaît en largeur réduite.
- **Ajout** : depuis le Mac (iCloud), sur le réseau (Bonjour), code à six chiffres accepté dans KyberNow Admin, adresse, ou QR code à la caméra (lien `kybernow://`, aussi reçu par AirDrop ou Messages) ; empreinte vérifiée toute seule quand le lien ou l'annonce la donne.
- **Lancement** : l'image fixe du système (encre, symbole éteint), puis 1,8 s d'allumage dans l'app ; avec « Réduire les animations », un fondu.
- **Passage** : l'accueil coupé le long de la couture à 45°, le tube néon qui grésille puis s'allume, les moitiés qui s'écartent sur l'image ; la capsule « Vous entrez dans… » en bas, au pouce, avec Annuler. Sur iPhone, la session est toujours en paysage.
- **Session** : l'image plein cadre, la languette en haut (latence en direct), la barre au toucher ou en glissant depuis le bord haut (latence et courbe, profil, son, pavé tactile ou toucher direct, souris Auto / Bureau / Jeu, écran, Touches, Quitter). Pavé tactile : un doigt déplace le pointeur de l'ordinateur, toucher = clic, deux doigts = clic droit, deux doigts qui glissent = molette, maintenir = glisser. Toucher direct : le doigt est le pointeur. Bouton du clavier au pouce droit ; au-dessus du clavier, une rangée de touches (Échap, Tab, Ctrl, Alt et ⊞ collants, flèches, F1 à F12, Ctrl + Alt + Suppr) ; le texte part en Unicode. Menu Touches (Ctrl + Alt + Suppr, Alt + Tab, ⊞ + D, Impr. écran, Alt + F4, ⊞ + L, gestionnaire des tâches, F11). Manettes Bluetooth (L1 + R1 + Start + Select ouvre la barre et la manette la pilote), clavier et souris ou trackpad de l'iPad (pointeur iPadOS caché, celui de l'ordinateur dessiné). Mesures détaillées en feuille sur iPhone, en panneau sur iPad.
- **États** : en capsule en haut de l'image, jamais en alerte : reconnexion (néon allumé, Quitter), mode de l'ordinateur changé (« Remettre » en profil Bureau), repli quand l'image ne passe pas, manette branchée.
- **Écran externe** (USB-C ou AirPlay, iPhone comme iPad) : l'image passe sur l'écran branché, à sa définition (« comme cet écran » vise alors l'externe, l'écran virtuel est refait à sa taille), et l'appareil devient le pupitre : toute sa surface est le pavé tactile, la barre reste ouverte, les mesures sont affichées sur iPad. Branché en cours de session, l'image y passe sans couper la session ; débranché, elle revient sur l'appareil (« L'image est revenue sur l'iPad »). Hors session, l'écran externe montre le symbole et une phrase d'attente.
- **Stage Manager** : une seule fenêtre ; la session vit dans sa fenêtre, l'image garde son format (bandes d'encre), la définition reste celle choisie au départ. Perdre le premier plan (autre fenêtre, autre app) relâche tout sur l'ordinateur, plafonne le débit et fige l'image (« KyberNow Link continue en arrière-plan »).
- **Arrière-plan** : tout est relâché sur l'ordinateur, le débit plafonné, l'image figée ; au retour, tout revient.
- **Réglages** : par ordinateur (image, son, entrées, réseau, compte), et ceux de l'app.

## Code

Le module KyberKit est celui du Mac et de l'Apple TV, cœur compris : la session (`SessionEngine`), Admin (`ConsoleClientCore`), les entrées (`InputCore` : chaîne kynput, pointeur tenu par l'app, clavier, combinaison, presse-papiers par `UIPasteboard`), le son (`AudioPlayerCore`, sortie RemoteIO), la latence, le décodage VideoToolbox et le rendu Metal viennent d'`apps/macos/KyberKit` ; la surface Metal, le clavier et la souris Bluetooth (`KeyboardMouse`) et les manettes (`GamepadBridge`) de l'Apple TV (`apps/tvos/KyberKitTV`). Propre à iOS : `KyberKitIOS/StreamView.swift`, la vue de session tactile (gestes, clavier de l'écran en `UIKeyInput`, touches collantes, survol au trackpad), qui sait montrer l'image dans une `ImageHostView` posée sur un autre écran (`showImage(in:)`).

L'app (`KyberNowIOS/`) : `KyberNowApp` (avec `AppDelegate` : orientation, scène de l'écran externe), `AppModel`, `Platform` (écran, son, préférences, images des ordinateurs), `Theme` (jetons de l'Apple TV à l'échelle du pouce), `HomeView` et `HomeParts`, `LaunchView`, `AddHostView`, `AddHostPanes` et `AddHostScanner` (caméra), `SettingsView` et `SettingsControls`, `AppSettingsView`, `SessionController` (la session vue de l'interface, sur `SessionEngine`), `SessionView` (passage, barre, rangée de touches, menu Touches, capsules, pupitre), `SessionStats`, `ExternalDisplay` (scène de l'écran externe, non interactive, déclarée par `AppDelegate`). Le modèle (`Model.swift`) et la découverte (`Discovery.swift`) viennent de l'app Mac ; la synchro iCloud (`CloudSync.swift`) et les images d'ordinateur (`Backdrops.swift`) de l'Apple TV.

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

En Debug dans le simulateur : `-demo YES` ajoute trois ordinateurs de démonstration ; `-skipLaunch YES` saute le lancement ; `-screen add|settings|appSettings|session|bar` ouvre un écran (la session de démonstration montre une mire, sans ordinateur).

## Langues

Français et anglais ([docs/i18n.md](../../docs/i18n.md)) : français si iOS est en français, anglais pour toute autre langue. Réglages de l'app › Langue : Auto, Français ou English, pris en compte à la relance. Textes dans les catalogues de chaînes (`KyberNowIOS/Localizable.xcstrings`, `InfoPlist.xcstrings`, et `apps/macos/KyberKit/Localizable.xcstrings` pour KyberKit). Après avoir ajouté des textes : `xcrun xcstringstool sync <catalogue> --stringsdata …` depuis les `.stringsdata` du build (`xcodebuild` ne remplit pas le catalogue lui-même), puis `python3 ../macos/scripts/check-strings.py --fix`.

## Signature

Équipe P6DB682S4K, identifiant `fr.leslignesclaires.kybernow`. Capacités : iCloud (magasin clé-valeur), partage du Trousseau. Distribution prévue : TestFlight, testeurs internes (pas encore de profil App Store) ; en attendant, [SIDELOAD.md](SIDELOAD.md) (`scripts/sideload.sh`, signature avec son propre compte Apple, SDK `KyberSDK-iOS.zip` joint aux versions publiées à partir de la 0.4.0).

En Debug, signature manuelle avec le profil « iOS App Development » `ios.fr.leslignesclaires.kybernow` (créé le 2026-10-21, appareils enregistrés dans le compte), certificat Apple Development. Pour qu'Xcode le trouve, copier le fichier téléchargé dans `~/Library/Developer/Xcode/UserData/Provisioning Profiles/<UUID>.mobileprovision` ; un nouvel appareil demande de l'ajouter au profil, puis de le retélécharger. Installer sur un appareil branché :

```sh
xcodebuild -scheme KyberNowIOS -configuration Debug -derivedDataPath build -destination 'generic/platform=iOS' build
xcrun devicectl device install app --device <UDID> build/Build/Products/Debug-iphoneos/KyberNow.app
```

La Release reste en signature automatique ; l'envoi sur TestFlight demandera un profil App Store.

## Reste à faire

- Session vérifiée sur un iPhone 15 Pro Max (22/10/2026) ; pas encore essayée sur un iPad (aucun iPad dans le profil de provisionnement ; le simulateur ne fait qu'afficher une mire : pas de décodage matériel). Sur iOS, une couche Metal qui a présenté des images avant d'être dans une fenêtre reste hors écran pour toujours (toutes ses images écartées, `presentedTime` à 0, jusqu'à un passage en arrière-plan) : `MetalSurface` crée la sienne une fois dans la fenêtre et y branche le rendu à ce moment-là, et la refait si l'écran écarte tout pendant deux secondes (journal `affichage bloqué`). `KN_PRESENT_STATS=1` (variable d'environnement au lancement par `devicectl`) écrit chaque seconde les images rendues, montrées et écartées.
- Écran externe et Stage Manager : écrits, mais le simulateur ne permet pas de les essayer (Xcode 27.1 sans Simulator.app ; `simctl io … screenConfig` branche un écran, mais iOS recopie l'écran sans donner la scène à l'app) ; à essayer sur un iPhone et un iPad (USB-C, AirPlay). Si iOS recopie l'écran au lieu d'ouvrir la scène externe, passer `UIApplicationSupportsMultipleScenes` à `true` dans `project.yml`.
- Stage Manager : pas de bouton « plein écran » dans la barre (iPadOS ne laisse pas une app agrandir sa fenêtre).
- Le presse-papiers de l'appareil n'est lu que quand l'ordinateur le demande (⌘V ou Ctrl+V là-bas) : le lire fait demander à iOS l'accord de coller, ce qui fait perdre le premier plan à l'app. Les changements sont annoncés sans lecture (`hasStrings`).
- Envoi sur TestFlight (profil App Store à créer, archive Release et `xcodebuild -exportArchive`).
