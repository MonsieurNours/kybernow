# Installer KyberNow Link sur un téléviseur LG (mode développeur)

[English](INSTALLATION.en.md) · Français

KyberNow Link pour webOS n'est pas dans le LG Content Store : il s'installe en **mode développeur**, que LG ouvre à tous avec un compte gratuit, sans rien modifier au téléviseur (pas de root). Compter un quart d'heure la première fois, puis une minute pour chaque mise à jour.

Il faut :

- un téléviseur LG sous **webOS 5 ou après** (2020 et après ; une OLED G2 de 2022 convient) ;
- un ordinateur (Windows, Mac ou Linux) sur le même réseau que le téléviseur ;
- le paquet de l'app : `fr.leslignesclaires.kybernow_<version>_arm.ipk` (construit par `apps/webos/scripts/build.sh`, voir [README.md](README.md)) ;
- un ordinateur Windows avec KyberNow Station, pour se connecter ensuite.

## 1. Le compte de développeur LG

1. Sur l'ordinateur, ouvrez [webostv.developer.lge.com](https://webostv.developer.lge.com) et créez un compte (« Sign In » › « Create Account »). C'est gratuit.
2. Validez l'adresse e-mail.

## 2. Le mode développeur sur le téléviseur

1. Sur le téléviseur, ouvrez le **LG Content Store** (Apps), cherchez **Developer Mode** et installez l'app.
2. Ouvrez **Developer Mode** et connectez-vous avec le compte de l'étape 1.
3. Activez **Dev Mode Status**. Le téléviseur redémarre.
4. Rouvrez **Developer Mode**. Activez **Key Server**. Notez l'**adresse IP** du téléviseur et la **phrase secrète** (« Passphrase », six caractères) que l'app affiche.

Le mode développeur a une durée, affichée dans l'app (« Remaining time ») : **quand elle arrive à zéro, le téléviseur sort du mode développeur et les apps installées ainsi disparaissent**. Le bouton **Extend Session** de l'app la remet au maximum ; pensez-y de temps en temps (webOS Dev Manager, plus bas, peut aussi la prolonger).

## 3. Installer l'app

Deux façons. La première est la plus simple.

### Avec webOS Dev Manager (fenêtres, sans ligne de commande)

1. Sur l'ordinateur, installez [webOS Dev Manager](https://github.com/webosbrew/dev-manager-desktop/releases) (webosbrew, libre).
2. Ajoutez le téléviseur : **Add device** › **Developer mode**, avec l'adresse IP et la phrase secrète de l'étape 2.
3. Onglet **Apps** › **Install** (ou glissez le fichier dans la fenêtre) : choisissez `fr.leslignesclaires.kybernow_<version>_arm.ipk`.
4. **KyberNow Link** apparaît dans la liste des apps du téléviseur.

### En ligne de commande (outils de LG)

```sh
# Une fois : les outils webOS de LG (Node.js 18 ou plus)
npm install -g @webos-tools/cli

# Une fois : le téléviseur (adresse IP de l'étape 2, port 9922, utilisateur prisoner)
ares-setup-device --add tv -i "host=192.168.1.50" -i "port=9922" -i "username=prisoner"
ares-novacom --device tv --getkey          # demande la phrase secrète de l'étape 2

# À chaque version
ares-install --device tv fr.leslignesclaires.kybernow_0.4.4_arm.ipk
ares-launch --device tv fr.leslignesclaires.kybernow
```

Remplacez `192.168.1.50` par l'adresse du téléviseur. `ares-install --device tv --list` dit ce qui est installé, `ares-install --device tv --remove fr.leslignesclaires.kybernow` retire l'app.

## 4. Premier lancement

1. Ouvrez **KyberNow Link** depuis l'accueil du téléviseur (barre des apps).
2. Les ordinateurs avec KyberNow Station sur le même réseau apparaissent en bas (« Sur le réseau »). Choisissez-en un : un **code à six chiffres** s'affiche.
3. Sur l'ordinateur, ouvrez **KyberNow Admin › Appareils Link** (ou l'icône près de l'horloge) et **acceptez le code**. Le téléviseur reçoit un compte à lui, que vous pouvez retirer dans Admin.
4. L'ordinateur s'affiche en grand sur l'accueil : **Se connecter**.

L'ordinateur n'apparaît pas ? Choisissez **Ajouter** › **Par adresse et port** et tapez l'adresse et le port d'Admin (affichés dans Admin › Appareils Link, sous le QR code), puis comparez l'empreinte affichée avec celle d'Admin.

## 5. Réglages du téléviseur conseillés

- **Mode d'image : Optimiseur de jeu.** Pendant la session, bouton Réglages de la Magic Remote › Image › Mode › Optimiseur de jeu. Aucune app ne peut le choisir à votre place ; sans lui, le téléviseur ajoute ses traitements d'image (et de la latence).
- **Manettes** : associez-les au téléviseur en Bluetooth, dans les réglages du téléviseur (rubrique des appareils ou des manettes ; le chemin varie selon la version de webOS). DualSense, DualShock 4, Xbox et Switch Pro marchent. Dans l'app, **Réglages › Manettes et Magic Remote › Tester les manettes** montre les touches et essaie les vibrations.
- **Réseau** : la prise Ethernet des téléviseurs LG s'arrête à 100 Mbit/s ; l'app y plafonne le débit à 80 Mbit/s. Le Wi-Fi (5 GHz, près du point d'accès) va souvent plus loin : l'app y monte jusqu'à 95 Mbit/s.

## Mettre à jour

Installez le nouveau `.ipk` comme à l'étape 3 : il remplace l'ancien. Une réinstallation peut effacer les ordinateurs ajoutés (ils sont gardés dans le dossier de l'app) : il suffit alors de refaire l'ajout par code.

## En cas de souci

- **L'app ne s'installe pas** : le mode développeur a peut-être expiré (rouvrez Developer Mode, réactivez, prolongez), ou l'adresse IP du téléviseur a changé (`ares-setup-device` ou Dev Manager).
- **Écran noir en session, mais du son** : notez-le avec le journal (ci-dessous) ; le décodeur du téléviseur a refusé le flux. Essayez Réglages › Image › Format : H.264, ou une définition plus basse.
- **Le journal** : l'app écrit `conf/kybernow.log` dans son dossier sur le téléviseur (`/media/developer/apps/usr/palm/applications/fr.leslignesclaires.kybernow/`), ou `/tmp/kybernow/kybernow.log` si ce dossier n'est pas accessible. Pour le lire depuis l'ordinateur :

  ```sh
  ssh -i ~/.ssh/tv_webos -p 9922 prisoner@192.168.1.50 \
      cat /media/developer/apps/usr/palm/applications/fr.leslignesclaires.kybernow/conf/kybernow.log
  ```

  (la clé `tv_webos` est celle qu'a récupérée `ares-novacom --getkey` ; Dev Manager a aussi un terminal et un explorateur de fichiers.)
