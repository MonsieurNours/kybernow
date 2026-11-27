---
slug: ce-qu-il-vous-faut
titre: Ce qu'il vous faut pour commencer
title: Ce qu'il vous faut pour commencer · KyberNow
description: L'ordinateur à partager, les appareils compatibles et le réseau conseillé pour utiliser KyberNow chez vous comme ailleurs.
rubrique: Commencer
univers: commun
duree: 3 min
connexes: [installer-station-windows, installer-station-mac, installer-station-linux, connecter-un-appareil]
---

KyberNow Station s'installe sur l'ordinateur que vous voulez retrouver, KyberNow Link sur chaque appareil d'où vous voulez l'utiliser.

## Les apps et leur rôle

- **KyberNow Station**, sur l'ordinateur à partager. Elle envoie son image et son son à vos appareils, et reçoit leur clavier, leur souris et leurs manettes.
- **KyberNow Admin**, les réglages de la Station. Il s'installe avec elle et s'ouvre dans un navigateur, sur l'ordinateur ou sur votre téléphone.
- **KyberNow Link**, sur vos appareils : Mac, PC, Linux, Steam Deck, Apple TV, iPhone, iPad, Android, téléviseur LG, Raspberry Pi. Dans un navigateur, il n'y a rien à installer.
- **KyberNow Link OS**, une image complète pour Raspberry Pi : la carte démarre directement sur KyberNow Link.

## L'ordinateur à partager

### Un PC sous Windows

- Windows 10 ou 11, avec un compte administrateur pour l'installation.
- Une carte graphique NVIDIA, AMD ou Intel, même externe (eGPU). Sans carte graphique, c'est le processeur qui compresse l'image, plus lentement, et l'installateur vous le signale.
- Microsoft Edge WebView2, déjà présent sur Windows 11 et sur la plupart des Windows 10. S'il manque, l'installateur ouvre la page de Microsoft pour le télécharger.
- Steam, pour que le son parte vers vos appareils et que le PC reste muet. Sans Steam, le son sort aussi des haut-parleurs du PC.
- Pour le HDR, Windows 11 24H2 et une carte graphique qui encode en 10 bits (HEVC ou AV1).

### Un Mac

- Un Mac à puce Apple (M1 ou plus récent), sous macOS 15 Sequoia ou plus récent.
- Rien à ajouter pour le son ni pour les manettes : la Station installe sa propre sortie son. La puce Apple suffit pour le HDR.

### Un ordinateur sous Linux

- Ubuntu 24.04, Debian 13 ou Fedora 42, ou une version plus récente, sur un processeur x86_64 ou arm64. Bazzite, Fedora Silverblue ou Kinoite et SteamOS ont leur propre installation.
- GNOME ou KDE Plasma 6 sous Wayland, ou Sway et Hyprland. Sous X11, la Station envoie les écrans de l'ordinateur tels qu'ils sont, sans écran virtuel.
- Une carte NVIDIA avec son pilote propriétaire, ou une carte AMD ou Intel. Sinon, le processeur compresse l'image.
- Pour le HDR, choisissez une Station Windows ou Mac.

## Vos appareils

| Appareil | Il vous faut |
|---|---|
| Mac | Une puce Apple, macOS 14 ou plus récent |
| PC Windows | Windows 10 22H2 ou Windows 11, x64 ou ARM64 (les PC ARM64, comme ceux à puce Snapdragon, demandent Windows 11) |
| PC Linux | Un processeur x86_64 ou ARM, et Flatpak |
| Steam Deck, Steam Machine, PC sous SteamOS ou Bazzite | Rien de plus |
| Apple TV | Une Apple TV 4K de 2021 ou plus récente, tvOS 18 ou plus récent |
| iPhone, iPad | iOS 18 ou iPadOS 18, ou plus récent |
| Android | Android 11 ou plus récent : téléphone, tablette, Android TV, Google TV, NVIDIA Shield |
| Téléviseur LG | webOS 5 ou plus récent (modèles 2020 et après), et un ordinateur pour y installer l'app |
| Raspberry Pi | Un Pi 3, 4 ou 5 (Pi 400, Pi 500 et Compute Module compris), et une carte microSD de 8 Go au moins pour KyberNow Link OS |
| Navigateur | Chrome ou Edge de préférence, Safari et Firefox aussi |

Pour jouer à votre console, il vous faut une PS5 ou une PS4 : voir [Jouer à votre PS5 ou PS4](/guide/playstation/).

## Le réseau

- Pour ajouter un appareil, il doit être sur le même réseau que l'ordinateur, ou relié à lui par un VPN. Ensuite, hors de chez vous, il passe par le relais de KyberNow : voir [Retrouver votre ordinateur hors de chez vous](/guide/hors-de-chez-vous/).
- Un câble Ethernet entre l'ordinateur et la box évite les micro-coupures du Wi-Fi.
- Pour l'appareil, un câble aussi quand c'est possible, sinon un Wi-Fi 5 GHz ou 6 GHz, près de la box.

## Le prix et le compte

KyberNow est gratuit et libre (licence AGPL-3.0). Aucun compte en ligne n'est obligatoire : le compte que vous créez à l'installation reste sur l'ordinateur.

Un compte KyberNow, gratuit lui aussi, fait apparaître vos ordinateurs sur tous vos appareils et vous permet de les rejoindre hors de chez vous sans rien régler sur votre box. Voir [Votre compte KyberNow](/guide/compte-kybernow/).

## Par où commencer

1. Installez la Station sur l'ordinateur à partager : [sur Windows](/guide/installer-station-windows/), [sur Mac](/guide/installer-station-mac/) ou [sur Linux](/guide/installer-station-linux/).
2. Installez KyberNow Link sur un appareil et ajoutez-y l'ordinateur : voir [Connecter un appareil à votre ordinateur](/guide/connecter-un-appareil/).
