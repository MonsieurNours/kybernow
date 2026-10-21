# Installer KyberNow sur votre Apple TV (sans l'App Store)

[English](SIDELOAD.en.md) · Français

En attendant sa publication sur l'App Store, l'app Apple TV s'installe depuis un Mac, signée avec **votre** compte Apple. Comptez dix minutes la première fois, une minute ensuite.

## Ce qu'il faut

- Une **Apple TV 4K** (2ᵉ génération, 2021, ou plus récente), à jour (tvOS 18 ou plus récent).
- Un **Mac** sur le même réseau, avec **Xcode** (gratuit, sur l'App Store du Mac ; ouvrez-le une fois pour qu'il finisse de s'installer).
- Un **compte Apple** :
  - **gratuit** (votre identifiant Apple habituel) : ça marche, mais l'app cesse de s'ouvrir au bout de **7 jours** ; il suffit de relancer le script pour la renouveler. Sans iCloud (ajout « Depuis votre Mac »), sans Top Shelf et sans profils par utilisateur de l'Apple TV ;
  - **payant** (Apple Developer Program, 99 €/an) : l'app reste installée un an, avec toutes ses fonctions.
- Un ordinateur avec **KyberNow Server** installé (la version 0.2.2 ou plus récente pour l'appairage par code).

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

## 2. Appairer l'Apple TV avec Xcode

1. Sur l'Apple TV : **Réglages › Télécommandes et appareils › App Remote et appareils**. Laissez cet écran ouvert.
2. Sur le Mac, dans Xcode : **Window › Devices and Simulators**. L'Apple TV apparaît à gauche ; cliquez sur **Pair** et tapez le code affiché sur le téléviseur.
3. Attendez que Xcode ait fini de préparer l'Apple TV (quelques minutes la première fois).

## 3. Installer l'app

Depuis le dossier `kybernow` :

```sh
# Compte gratuit
apps/tvos/scripts/sideload.sh --team ABCDE12345

# Compte payant : toutes les fonctions (iCloud, Top Shelf, profils tvOS)
apps/tvos/scripts/sideload.sh --team ABCDE12345 --full
```

Le script :
1. télécharge le SDK de Kyber et le cœur KyberNow déjà compilés pour l'Apple TV (`KyberSDK-tvOS.zip`, joint à chaque version publiée ; le cœur depuis la 0.3.1) ;
2. prépare le projet Xcode à votre nom (identifiant de l'app `kybernow.<votre équipe>`, à changer avec `--bundle` si besoin) ;
3. compile l'app ; Xcode crée lui-même le profil de signature ;
4. l'installe sur l'Apple TV appairée (`--device "Nom de l'Apple TV"` s'il y en a plusieurs).

**Réveillez l'Apple TV avant de lancer le script** : en veille, elle accepte l'installation mais refuse d'ouvrir l'app (« System is asleep »).

## 4. Premier lancement

- Ouvrez **KyberNow** depuis l'accueil de l'Apple TV.
- Acceptez l'accès au **réseau local** : sans lui, l'app ne trouve pas vos ordinateurs.
- Ajoutez votre ordinateur : **Code à l'écran** (à accepter dans la console de l'ordinateur, espace Accès › Appareils), **Sur le réseau** (son compte, tapé au clavier de l'iPhone), ou **Adresse**.

## Renouveler ou mettre à jour

Relancez la même commande après un `git pull`. Avec un compte gratuit, faites-le avant la fin des 7 jours ; vos ordinateurs et réglages restent sur l'Apple TV.

## En cas de souci

- **« Apple TV introuvable »** : elle n'est pas appairée, ou Xcode ne la voit pas (même réseau, Apple TV réveillée). Vérifiez dans Window › Devices and Simulators.
- **Erreur de signature** : ouvrez `apps/tvos/KyberNowTVSideload.xcodeproj` (produit par le script) dans Xcode, onglet *Signing & Capabilities* de la cible KyberNowTV : Xcode y explique ce qui manque. Avec un compte gratuit, n'utilisez pas `--full`.
- **L'app ne trouve pas l'ordinateur** : le serveur doit être allumé, sur le même réseau, avec KyberNow Server installé ; sinon, passez par l'adresse ou le code. Par VPN, la découverte ne passe pas : utilisez l'adresse.
- **Compiler soi-même le SDK de Kyber** (développeurs) : `apps/tvos/scripts/build-kyber-tvos.sh`, puis le cœur : `apps/apple/scripts/build-apple-mobile.sh tvos`, voir [README.md](README.md).

KyberNow est basé sur [Kyber](https://kyber.tech) (AGPLv3), sans affiliation avec l'équipe de Kyber.
