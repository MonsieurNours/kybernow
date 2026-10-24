# Installer KyberNow Link sur votre Apple TV (sans l'App Store)

[English](SIDELOAD.en.md) · Français

L'app Apple TV n'est pas sur l'App Store : elle s'installe depuis un Mac, signée avec **votre** compte Apple. Comptez dix minutes la première fois, une minute ensuite.

## Ce qu'il faut

- Une **Apple TV 4K** (2ᵉ génération, 2021, ou plus récente), à jour (tvOS 18 ou plus récent).
- Un **Mac** sur le même réseau, avec **Xcode** (gratuit, sur l'App Store du Mac ; ouvrez-le une fois pour qu'il finisse de s'installer).
- Un **compte Apple** :
  - **gratuit** (votre identifiant Apple habituel) : ça marche, mais l'app cesse de s'ouvrir au bout de **7 jours** ; il suffit de relancer le script pour la renouveler. Sans iCloud (ajout « Depuis votre Mac »), sans Top Shelf et sans profils par utilisateur de l'Apple TV ;
  - **payant** (Apple Developer Program, 99 €/an) : l'app reste installée un an, avec iCloud, le Top Shelf et les profils par utilisateur. L'ajout « Depuis votre Mac » ne voit que les ordinateurs publiés par une app Mac signée avec la même équipe et le même identifiant d'app : pas ceux de l'app Mac publiée sur le site, dont le stockage iCloud appartient à l'équipe de KyberNow.
- Un ordinateur avec **KyberNow Station** installée (la version 0.2.2 ou plus récente pour l'appairage par code).

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
1. télécharge le SDK de Kyber et le cœur KyberNow déjà compilés pour l'Apple TV (`KyberSDK-tvOS.zip`, joint à chaque version publiée : la dernière, ou `--version X.Y.Z`, 0.3.1 ou plus récente, les archives plus anciennes n'ont pas le cœur), dans `apps/tvos/Vendor` ; s'ils y sont déjà (compilés sur place, ou téléchargés lors d'un passage précédent), il les reprend sans rien télécharger ;
2. prépare le projet Xcode à votre nom (identifiant de l'app `kybernow.<votre équipe>`, à changer avec `--bundle` si besoin) ;
3. compile l'app (configuration Release) ; Xcode crée lui-même le profil de signature ;
4. l'installe sur l'Apple TV appairée (`--device "Nom de l'Apple TV"` s'il y en a plusieurs).

**Réveillez l'Apple TV avant de lancer le script** : en veille, elle accepte l'installation mais refuse d'ouvrir l'app (« System is asleep »).

## 4. Premier lancement

- Ouvrez **KyberNow** depuis l'accueil de l'Apple TV.
- Acceptez l'accès au **réseau local** : sans lui, l'app ne trouve pas vos ordinateurs.
- Ajoutez votre ordinateur : **Code à l'écran** (à accepter dans KyberNow Admin, espace Appareils Link), **Sur le réseau** (son compte, tapé au clavier de l'iPhone), ou **Adresse**.

## Renouveler ou mettre à jour

Relancez la même commande après un `git pull`. Avec un compte gratuit, faites-le avant la fin des 7 jours ; vos ordinateurs et réglages restent sur l'Apple TV.

Le SDK n'est téléchargé que si `apps/tvos/Vendor` manque : pour passer à une nouvelle version, supprimez ce dossier avant de relancer (`rm -rf apps/tvos/Vendor`). Il vient d'une version publiée, alors que `git pull` donne le code le plus récent : si le cœur a changé depuis cette version, la compilation échoue. Prenez alors le code de la version du SDK (`git checkout vX.Y.Z`), ou compilez le SDK vous-même (ci-dessous).

## En cas de souci

- **« Apple TV introuvable »** : elle n'est pas appairée, ou Xcode ne la voit pas (même réseau, Apple TV réveillée). Vérifiez dans Window › Devices and Simulators.
- **La compilation échoue après une mise à jour** : le SDK de `apps/tvos/Vendor` ne correspond plus au code, voir « Renouveler ou mettre à jour ».
- **Erreur de signature** : ouvrez `apps/tvos/KyberNowTVSideload.xcodeproj` (produit par le script) dans Xcode, onglet *Signing & Capabilities* de la cible KyberNowTV : Xcode y explique ce qui manque. Avec un compte gratuit, n'utilisez pas `--full`.
- **L'app ne trouve pas l'ordinateur** : l'ordinateur doit être allumé, sur le même réseau, avec KyberNow Station installée ; sinon, passez par l'adresse (le code ne part qu'aux ordinateurs trouvés sur le réseau). Par VPN, la découverte ne passe pas : utilisez l'adresse.
- **Compiler soi-même le SDK de Kyber** (développeurs) : `apps/tvos/scripts/build-kyber-tvos.sh`, puis le cœur : `apps/apple/scripts/build-apple-mobile.sh tvos`, voir [README.md](README.md).

KyberNow est basé sur [Kyber](https://kyber.tech) (AGPLv3), sans affiliation avec l'équipe de Kyber.
