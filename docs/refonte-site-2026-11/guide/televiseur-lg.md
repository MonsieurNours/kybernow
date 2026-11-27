---
slug: televiseur-lg
titre: Jouer sur un téléviseur LG
title: KyberNow Link sur téléviseur LG · KyberNow
description: Installez KyberNow Link sur un téléviseur LG webOS en mode développeur et jouez à votre ordinateur en 4K à 120 Hz, à la manette.
rubrique: Connecter un appareil
univers: jeu
duree: 15 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, profils-et-qualite, mettre-a-jour]
---

Sur un téléviseur LG, l'app va jusqu'à la 4K à 120 Hz, avec quatre manettes Bluetooth et leurs vibrations, la Magic Remote en souris et le son 5.1 Dolby vers un ampli. Elle s'installe en mode développeur, que LG ouvre à tous avec un compte gratuit, sans rien modifier au téléviseur.

## Avant de commencer

- Un téléviseur LG sous webOS 5 ou plus récent (modèles 2020 et après).
- Un ordinateur sous Windows, macOS ou Linux, sur le même réseau que le téléviseur.
- Le fichier de l'app : [KyberNow-Link-webOS.ipk](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-webOS.ipk).
- Comptez un quart d'heure la première fois, puis une minute pour chaque mise à jour.

## Le compte de développeur LG

1. Sur l'ordinateur, ouvrez webostv.developer.lge.com et créez un compte : Sign In › Create Account. C'est gratuit.
2. Validez votre adresse e-mail.

## Le mode développeur sur le téléviseur

1. Sur le téléviseur, ouvrez le LG Content Store, cherchez Developer Mode et installez l'app.
2. Ouvrez Developer Mode et connectez-vous avec le compte LG.
3. Activez Dev Mode Status. Le téléviseur redémarre.
4. Rouvrez Developer Mode, activez Key Server, puis notez l'adresse IP du téléviseur et sa phrase secrète (Passphrase).

## Installer l'app

1. Sur l'ordinateur, installez webOS Dev Manager, une app libre pour Windows, Mac et Linux.
2. Ajoutez le téléviseur : Add device › Developer mode, avec l'adresse IP et la phrase secrète.
3. Dans l'onglet Apps, cliquez sur Install, ou glissez le fichier dans la fenêtre, et choisissez KyberNow-Link-webOS.ipk.
4. KyberNow Link apparaît dans la liste des apps du téléviseur.

En ligne de commande, les outils de LG font la même chose (Node.js 18 ou plus récent) :

```sh
npm install -g @webos-tools/cli
ares-setup-device --add tv -i "host=192.168.1.50" -i "port=9922" -i "username=prisoner"
ares-novacom --device tv --getkey
ares-install --device tv KyberNow-Link-webOS.ipk
```

Remplacez `192.168.1.50` par l'adresse du téléviseur. `ares-novacom` demande la phrase secrète.

## Ajouter votre ordinateur

1. Ouvrez KyberNow Link depuis l'accueil du téléviseur.
2. Les ordinateurs du même réseau apparaissent en bas, sous « Sur le réseau ». Choisissez le vôtre : un code à six chiffres s'affiche.
3. Acceptez-le dans Admin, espace Appareils Link (sur un PC, aussi dans le panneau de l'icône près de l'horloge). Le téléviseur reçoit son propre accès, que vous pourrez retirer dans Admin.
4. L'ordinateur s'affiche en grand sur l'accueil : choisissez un profil, puis « Se connecter ».

Si l'ordinateur n'apparaît pas, choisissez « Ajouter un ordinateur ou une console » › « Par adresse et port », tapez l'adresse et le port d'Admin, affichés dans Admin › Appareils Link, sous le QR code, puis comparez l'empreinte avec celle d'Admin.

## Le mode développeur a une durée

Elle s'affiche dans l'app Developer Mode. À zéro, le téléviseur quitte le mode développeur et les apps installées ainsi disparaissent. Le bouton Extend Session la remet au maximum : pensez-y de temps en temps. webOS Dev Manager peut aussi la prolonger.

## Réglages conseillés

- **Mode d'image** : pendant la session, choisissez Optimiseur de jeu (bouton Réglages de la Magic Remote › Image › Mode). Sans lui, le téléviseur ajoute ses traitements d'image, et de la latence.
- **Manettes** : associez-les au téléviseur en Bluetooth, dans ses réglages. DualSense, DualShock 4, Xbox et Switch Pro marchent. Réglages de l'app › Manettes › Tester les manettes montre les touches et essaie les vibrations.
- **Son 5.1 vers un ampli en eARC** : dans l'app, réglages de l'ordinateur › Son › 5.1 Dolby ; sur le téléviseur, Son › Sortie audio numérique › Pass-Through (« Interconnexion »). Sans ce réglage, l'ampli reçoit de la stéréo.
- **Réseau** : la prise Ethernet des téléviseurs LG s'arrête à 100 Mbit/s, et l'app y plafonne le débit à 80 Mbit/s. Le Wi-Fi 5 GHz, près de la box, va souvent plus loin : l'app y monte jusqu'à 95 Mbit/s.

## Mettre à jour l'app

L'app affiche « Version X disponible » sur l'accueil. En mode développeur, elle ne peut pas s'installer elle-même : téléchargez le nouveau KyberNow-Link-webOS.ipk et installez-le comme la première fois. Il remplace l'ancien, et vos ordinateurs restent.

## Si ça ne marche pas

- **L'app ne s'installe pas, ou a disparu.** Le mode développeur a expiré. Rouvrez Developer Mode, réactivez-le et prolongez la session, puis réinstallez l'app.
- **Dev Manager ne joint plus le téléviseur.** Son adresse IP a changé. Modifiez-la dans Dev Manager.
- **Écran noir en session, mais du son.** Le décodeur du téléviseur refuse le flux. Dans les réglages de l'ordinateur, essayez Image › Format vidéo › H.264, ou une définition plus basse.
- **L'ampli reçoit de la stéréo.** Vérifiez Pass-Through sur le téléviseur et 5.1 Dolby dans l'app.
