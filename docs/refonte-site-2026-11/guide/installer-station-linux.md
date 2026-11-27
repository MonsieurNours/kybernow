---
slug: installer-station-linux
titre: Installer KyberNow Station sur Linux
title: Installer KyberNow Station sur Linux · KyberNow
description: Installez KyberNow Station sur Ubuntu, Debian, Fedora, Bazzite ou SteamOS, puis retrouvez votre compte et ouvrez KyberNow Admin.
rubrique: Installer la Station
univers: commun
duree: 5 min
connexes: [ce-qu-il-vous-faut, connecter-un-appareil, link-linux-steam-deck, mettre-a-jour]
---

Votre ordinateur sous Linux devient une Station, avec le même Admin et les mêmes apps que sur un PC Windows. Elle s'installe en paquet .deb ou .rpm, ou dans votre dossier personnel sur Bazzite, Fedora Silverblue ou Kinoite et SteamOS.

## Avant de commencer

- Ubuntu 24.04, Debian 13 ou Fedora 42, ou une version plus récente. Bazzite, Fedora Silverblue ou Kinoite et SteamOS passent par l'archive, plus bas.
- Un processeur x86_64 (la plupart des PC) ou arm64.
- GNOME ou KDE Plasma 6 sous Wayland, ou Sway et Hyprland, pour l'écran virtuel à la taille de chaque appareil. Sous X11 et les autres bureaux, la Station envoie les écrans de l'ordinateur tels qu'ils sont.
- Votre compte, avec le droit d'utiliser `sudo`. C'est ce compte qui pourra ensuite mettre la Station à jour depuis Admin.

La Station choisit l'encodeur selon votre carte : NVENC pour une carte NVIDIA avec son pilote propriétaire, VA-API pour AMD et Intel, sinon le processeur.

## Ubuntu, Debian, Fedora

1. Téléchargez le paquet de votre système et de votre processeur :

   | | x86_64 | arm64 |
   |---|---|---|
   | Ubuntu, Debian | [KyberNow-Station-Linux-x86_64.deb](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-x86_64.deb) | [KyberNow-Station-Linux-arm64.deb](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-arm64.deb) |
   | Fedora | [KyberNow-Station-Linux-x86_64.rpm](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-x86_64.rpm) | [KyberNow-Station-Linux-arm64.rpm](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-arm64.rpm) |

2. Ouvrez un terminal dans le dossier du téléchargement et installez-le :

   ```sh
   sudo apt install ./KyberNow-Station-Linux-x86_64.deb     # Ubuntu, Debian
   sudo dnf install ./KyberNow-Station-Linux-x86_64.rpm     # Fedora
   ```

   Si le pare-feu du système tourne (firewalld ou ufw), le paquet lui ouvre les ports de la Station.

3. Fermez votre session, puis rouvrez-la : la Station démarre avec elle. Pour la lancer tout de suite, sans fermer la session :

   ```sh
   systemctl --user start kybernow-admin kybernow-kyber kybernow-tray
   ```

## Bazzite, Fedora Silverblue ou Kinoite, SteamOS

Sur ces systèmes, le dossier du système ne se modifie pas. La Station s'installe dans votre dossier personnel, sans redémarrer, et reste en place quand le système se met à jour. Elle se met ensuite à jour depuis Admin, sans mot de passe.

1. Sur SteamOS, passez au mode Bureau : bouton Steam › Alimentation › Passer au bureau. La Station fonctionne dans le mode Bureau.
2. Téléchargez [KyberNow-Station-Linux-x86_64.tar.xz](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-x86_64.tar.xz), ou [KyberNow-Station-Linux-aarch64.tar.xz](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-aarch64.tar.xz) pour une machine ARM.
3. Ouvrez un terminal (Konsole) dans le dossier du téléchargement et tapez :

   ```sh
   tar -xf KyberNow-Station-Linux-x86_64.tar.xz
   ./kybernow-station-*/install.sh
   ```

4. Si votre mot de passe est demandé, c'est pour que le clavier, la souris et les manettes de vos appareils puissent piloter l'ordinateur : une règle du système, posée une fois. Sur la plupart des machines avec Steam, ce n'est pas demandé, Steam l'ayant déjà fait. Sur SteamOS, si vous n'avez jamais choisi de mot de passe, tapez d'abord `passwd`.
5. L'installation affiche l'adresse d'Admin et l'endroit où trouver le mot de passe de votre premier compte.

## Votre premier compte

La Station crée un compte au premier démarrage. Son identifiant est votre nom d'utilisateur Linux. Son mot de passe est tiré au hasard et rangé dans un fichier que vous seul pouvez lire :

```sh
cat ~/.local/share/kybernow/station/admin/mot-de-passe
```

Ce compte vous sert à ouvrir Admin et à connecter vos appareils. Vous pouvez changer son mot de passe et créer d'autres comptes dans Admin, espace Utilisateurs.

## Ouvrir Admin

Une icône KyberNow Station apparaît près de l'horloge. Son menu propose « Ouvrir Admin », « Redémarrer la Station », « Partager un écran… » et « Prêter l'ordinateur… ».

- Sous KDE Plasma, l'icône s'affiche d'elle-même.
- Sous GNOME, elle demande l'extension AppIndicator, présente d'office sous Ubuntu. Ailleurs, installez le paquet `gnome-shell-extension-appindicator`.

Admin s'ouvre dans votre navigateur. La première fois, celui-ci affiche un avertissement de sécurité, parce que la Station signe elle-même sa connexion : acceptez-le. Admin a les mêmes espaces que sur un PC : Vue d'ensemble, Image et son, Enregistrements, Appareils Link, Prêt, Utilisateurs, Système et Compte KyberNow. Pour rattacher l'ordinateur à votre compte, ouvrez Compte KyberNow : voir [Votre compte KyberNow](/guide/compte-kybernow/).

Il ne reste plus qu'à [connecter un appareil](/guide/connecter-un-appareil/).

## Après un redémarrage

La Station tourne dans votre session : elle démarre quand vous l'ouvrez et s'arrête avec elle. Un écran verrouillé se diffuse aussi, et vous tapez le mot de passe depuis votre appareil.

Pour que la Station soit là après un redémarrage, sans personne devant l'ordinateur, activez l'ouverture automatique de la session dans les réglages de votre système. Sur SteamOS et Bazzite Deck, qui démarrent en mode Jeu, faites démarrer l'ordinateur en mode Bureau :

```sh
steamos-session-select plasma-persistent
```

> Bon à savoir : la caméra de vos appareils arrive sur l'ordinateur sous le nom « KyberNow Camera », vue par les navigateurs et OBS. Pour que Zoom, Discord et les autres apps la voient aussi, installez v4l2loopback (`sudo apt install v4l2loopback-dkms` sous Ubuntu et Debian, `sudo dnf install akmod-v4l2loopback` sous Fedora, avec le dépôt RPM Fusion), puis redémarrez. Voir [Stylet, micro, caméra et enregistrement](/guide/stylet-micro-webcam/).

## Ce que fait une Station Linux

- L'écran virtuel à la taille de chaque appareil, sous GNOME, KDE Plasma, Sway et Hyprland, avec un ou plusieurs écrans.
- Le son en stéréo, 5.1 ou 7.1, par une sortie « KyberNow » ; la sortie d'origine revient à la fin.
- Les manettes : une manette PlayStation arrive en DualSense, les autres en manette Xbox.
- Le stylet, le presse-papiers, le prêt de l'ordinateur et PyroWave avec une carte graphique qui gère Vulkan 1.3.

## Si ça ne marche pas

- **Pas d'icône près de l'horloge sous GNOME.** Installez et activez l'extension AppIndicator, puis rouvrez la session.
- **Vos appareils ne trouvent pas l'ordinateur.** Vérifiez qu'ils sont sur le même réseau, et que votre pare-feu laisse passer la Station. Par un VPN, ajoutez l'ordinateur par son adresse : voir [Connecter un appareil à votre ordinateur](/guide/connecter-un-appareil/).
- **La mise à jour depuis Admin n'aboutit pas.** Si la Station a été installée autrement qu'avec `sudo` ou `pkexec`, votre compte n'a pas encore le droit de la mettre à jour. Ajoutez-le au groupe de la Station, puis rouvrez la session :

  ```sh
  sudo usermod -aG kybernow-station VOTRE_COMPTE
  ```

- **Pas d'écran virtuel.** La session est sous X11 ou sous un bureau que la Station ne gère pas. Ouvrez une session GNOME ou KDE Plasma sous Wayland.
- **Deux comptes ouverts en même temps.** Une seule session à la fois porte la Station. Fermez l'autre.
