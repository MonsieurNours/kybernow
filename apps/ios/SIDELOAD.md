# Installer KyberNow sur votre iPhone ou votre iPad (sans l'App Store)

[English](SIDELOAD.en.md) · Français

En attendant TestFlight et l'App Store, l'app iPhone et iPad s'installe depuis un Mac, signée avec **votre** compte Apple. Comptez dix minutes la première fois, une minute ensuite.

## Ce qu'il faut

- Un **iPhone** ou un **iPad** sous iOS 18 ou iPadOS 18 au moins, et un câble pour le brancher au Mac (le Wi-Fi suffit ensuite).
- Un **Mac** avec **Xcode** (gratuit, sur l'App Store du Mac ; ouvrez-le une fois pour qu'il finisse de s'installer).
- Un **compte Apple** :
  - **gratuit** (votre identifiant Apple habituel) : ça marche, mais l'app cesse de s'ouvrir au bout de **7 jours** ; il suffit de relancer le script pour la renouveler. Sans iCloud ni Trousseau partagé (l'ajout « Depuis votre Mac » n'est pas proposé) ; trois apps installées ainsi au plus ;
  - **payant** (Apple Developer Program, 99 €/an) : l'app reste installée un an, avec toutes ses fonctions.
- Un ordinateur avec **KyberNow Server** installé (version 0.3.1 ou plus récente).

## 1. Préparer le Mac

1. Dans le Terminal, installez XcodeGen (avec [Homebrew](https://brew.sh)) :

   ```sh
   brew install xcodegen
   ```

2. Récupérez le code de KyberNow :

   ```sh
   git clone https://github.com/MonsieurNours/kybernow.git
   cd kybernow
   ```

3. Dans **Xcode › Réglages › Comptes**, ajoutez votre compte Apple (bouton **+**). Notez l'**identifiant d'équipe** : 10 caractères, par exemple `ABCDE12345`. Il apparaît dans Xcode quand vous choisissez l'équipe pour signer une app ; sur [developer.apple.com/account](https://developer.apple.com/account) (Membership) pour un compte payant. Pour un compte gratuit, c'est l'équipe « (Personal Team) ».

## 2. Préparer l'iPhone ou l'iPad

1. Branchez-le au Mac, déverrouillez-le et acceptez **Se fier à cet ordinateur**.
2. Activez le **mode développeur** : **Réglages › Confidentialité et sécurité › Mode développeur** (l'appareil redémarre). Sans lui, iOS refuse toute app qui ne vient pas de l'App Store.
3. Dans Xcode, **Window › Devices and Simulators** : l'appareil apparaît à gauche. Attendez que Xcode ait fini de le préparer (quelques minutes la première fois).

## 3. Installer l'app

Depuis le dossier `kybernow` :

```sh
# Compte gratuit
apps/ios/scripts/sideload.sh --team ABCDE12345

# Compte payant : toutes les fonctions (iCloud, Trousseau partagé)
apps/ios/scripts/sideload.sh --team ABCDE12345 --full
```

Le script :
1. télécharge le cœur KyberNow et ses bibliothèques déjà compilés pour iOS (`KyberSDK-iOS.zip`, joint aux versions publiées à partir de la 0.3.2 ; `--version X.Y.Z` pour une version précise) ;
2. prépare le projet Xcode à votre nom (identifiant de l'app `kybernow.<votre équipe>`, à changer avec `--bundle` si besoin) ;
3. compile l'app ; Xcode crée lui-même le profil de signature ;
4. l'installe sur l'appareil branché (`--device "Nom de l'iPad"` s'il y en a plusieurs).

## 4. Premier lancement

- Avec un compte gratuit, iOS refuse d'abord d'ouvrir l'app : **Réglages › Général › VPN et gestion de l'appareil**, touchez votre compte sous « App de développeur », puis **Se fier**.
- Ouvrez **KyberNow**, acceptez l'accès au **réseau local** : sans lui, l'app ne trouve pas vos ordinateurs.
- Ajoutez votre ordinateur : **Sur le réseau** (son compte), **Code** (à accepter dans la console de l'ordinateur, espace Accès › Appareils), **QR code** (affiché par la console, à la caméra) ou **Adresse**. « Depuis votre Mac » (iCloud) demande l'installation avec `--full`.

## Renouveler ou mettre à jour

Relancez la même commande après un `git pull`. Avec un compte gratuit, faites-le avant la fin des 7 jours ; vos ordinateurs et réglages restent sur l'appareil.

## En cas de souci

- **« iPhone ou iPad introuvable »** : il n'est pas branché, pas déverrouillé, ou Xcode ne le voit pas encore (Window › Devices and Simulators). Le mode développeur doit être activé.
- **Erreur de signature** : ouvrez `apps/ios/KyberNowIOSSideload.xcodeproj` (produit par le script) dans Xcode, onglet *Signing & Capabilities* de la cible KyberNowIOS : Xcode y explique ce qui manque. Avec un compte gratuit, n'utilisez pas `--full`, et pas plus de trois apps installées de cette façon.
- **« Développeur non approuvé »** à l'ouverture : voir « Premier lancement ».
- **L'app ne trouve pas l'ordinateur** : le serveur doit être allumé, sur le même réseau, avec KyberNow Server installé ; sinon, passez par l'adresse, le code ou le QR code. Par VPN, la découverte ne passe pas : utilisez l'adresse.
- **Compiler soi-même le SDK** (développeurs) : `apps/core/scripts/build-apple.sh --target aarch64-apple-ios` (et `aarch64-apple-ios-sim`), puis `apps/apple/scripts/build-apple-mobile.sh ios`, voir [README.md](README.md).

KyberNow est basé sur [Kyber](https://kyber.tech) (AGPLv3), sans affiliation avec l'équipe de Kyber.
