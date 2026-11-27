---
slug: android
titre: Utiliser votre ordinateur depuis Android
title: KyberNow Link sur Android et Android TV · KyberNow
description: Installez KyberNow sur un téléphone, une tablette ou un téléviseur Android, puis ajoutez votre ordinateur par le réseau, un code ou un QR code.
rubrique: Connecter un appareil
univers: commun
duree: 5 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, profils-et-qualite, hors-de-chez-vous]
---

La même app sert au téléphone, à la tablette et au téléviseur (Android TV, Google TV, NVIDIA Shield), à partir d'Android 11. Au téléviseur, elle va jusqu'à la 4K à 60 Hz, avec le son 7.1 vers un ampli HDMI, les manettes Bluetooth et la télécommande.

## Avant de commencer

- Un appareil sous Android 11 ou plus récent.
- La Station installée sur l'ordinateur à retrouver.
- Pour un téléviseur, un ordinateur sur le même réseau, d'où installer l'app.

L'app s'installe à partir de son fichier, [KyberNow-Link-Android.apk](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Android.apk).

## Sur un téléphone ou une tablette

1. Sur l'appareil, ouvrez le lien de KyberNow-Link-Android.apk, puis le fichier téléchargé.
2. Android vous demande d'autoriser votre navigateur à installer des applications : autorisez-le, revenez en arrière, puis touchez « Installer ».
3. Ouvrez KyberNow.

## Sur un téléviseur

Le plus simple est de passer par un ordinateur du même réseau, avec adb, l'outil d'Android pour les développeurs. Comptez cinq minutes la première fois.

1. Sur le téléviseur, activez les options pour les développeurs : Paramètres › Préférences de l'appareil › À propos (sur Google TV : Paramètres › Système › À propos), puis appuyez sept fois sur Build.
2. Dans les Options pour les développeurs, activez Débogage réseau (ou Débogage ADB), et notez l'adresse du téléviseur.
3. Sur l'ordinateur, téléchargez KyberNow-Link-Android.apk et installez adb : sur un Mac avec Homebrew, `brew install android-platform-tools` ; ailleurs, les SDK Platform-Tools de Google.
4. Reliez l'ordinateur au téléviseur, puis installez l'app, depuis le dossier du fichier téléchargé :

   ```sh
   adb connect 192.168.1.30
   adb install KyberNow-Link-Android.apk
   ```

   Remplacez l'adresse par celle du téléviseur. Au premier `adb connect`, le téléviseur demande d'autoriser le débogage : cochez « Toujours autoriser » et validez. Vous pourrez ensuite désactiver le débogage.
5. Ouvrez KyberNow depuis l'accueil du téléviseur.

## Ajouter votre ordinateur

Au premier lancement, trois écrans d'accueil vous proposent un compte KyberNow, facultatif. Au téléviseur, la connexion au compte se fait par un code que vous approuvez depuis votre téléphone. Voir [Votre compte KyberNow](/guide/compte-kybernow/).

Choisissez ensuite « Ajouter un ordinateur ou une console », puis l'un des onglets :

- **Réseau** : votre ordinateur apparaît tout seul.
- **Code** : touchez « Afficher un code ». L'app affiche un code à six chiffres. Acceptez-le dans Admin, espace Appareils Link (sur un PC, aussi dans le panneau de l'icône près de l'horloge). Rien à taper.
- **Adresse** : l'adresse IP ou le nom de l'ordinateur, et le port d'Admin. C'est le choix à faire par un VPN. Comparez ensuite les quatre groupes de l'empreinte avec ceux qu'affiche Admin › Appareils Link, puis touchez « Ce sont les mêmes ». Entrez le compte de la Station, ou passez « Par un code ».
- **QR code**, sur un téléphone ou une tablette : dans Admin, espace Appareils Link, « Ajouter un appareil » affiche le QR code. Visez-le : l'empreinte de l'ordinateur est vérifiée toute seule.

Choisissez un profil (Jeu, Bureau, Max ou Auto), puis « Se connecter ».

## Pendant la session

- Au téléphone et à la tablette, la barre de session donne le clavier et la manette à l'écran, en disposition « Complète » ou « Simple ». Le stylet d'Android arrive sur l'ordinateur comme un vrai stylet.
- Au téléviseur, la manette et la télécommande pilotent l'app. Associez les manettes en Bluetooth dans les réglages du téléviseur.

Le détail est dans [Clavier, souris et raccourcis](/guide/clavier-et-raccourcis/).

## Réglages conseillés

- **Au téléviseur** : un câble Ethernet si le téléviseur ou la box le permet, sinon un Wi-Fi 5 GHz ou 6 GHz près de la box.
- **Son** : pour le 7.1, passez par un ampli branché en HDMI. En Bluetooth, le son arrive souvent en retard.

## Si ça ne marche pas

- **Android refuse d'installer le fichier.** Autorisez votre navigateur, ou votre gestionnaire de fichiers, à installer des applications dans les paramètres d'Android.
- **`adb connect` ne répond pas.** Vérifiez l'adresse du téléviseur et que le débogage réseau est bien activé.
- **« Aucune Station trouvée sur le réseau pour l'instant ».** L'ordinateur doit être allumé, sur le même réseau, avec KyberNow Station en marche. Par un VPN, passez par l'onglet Adresse.
- **« La caméra est refusée ».** Autorisez la caméra pour KyberNow dans les réglages d'Android.
- **« Le code a expiré : recommencez. »** Affichez un nouveau code et acceptez-le plus vite dans Admin.
- **« Cette Station ne connaît pas les codes ».** Mettez la Station à jour sur l'ordinateur, ou ajoutez-le avec son compte.
