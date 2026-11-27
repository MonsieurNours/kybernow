---
slug: ce-qu-il-vous-faut
slug_en: what-you-need
titre: What you need to get started
title: What you need to get started · KyberNow
description: The computer to share, the devices that work with it and the network we recommend for using KyberNow at home and away.
rubrique: Commencer
rubrique_en: Get started
univers: commun
duree: 3 min
connexes: [installer-station-windows, installer-station-mac, installer-station-linux, connecter-un-appareil]
---

KyberNow Station goes on the computer you want to reach, and KyberNow Link on each device you want to use it from.

## The apps and what they do

- **KyberNow Station**, on the computer you share. It sends its picture and sound to your devices, and takes in their keyboard, mouse and controllers.
- **KyberNow Admin**, the Station's settings. It is installed with the Station and opens in a browser, on the computer itself or on your phone.
- **KyberNow Link**, on your devices: Mac, PC, Linux, Steam Deck, Apple TV, iPhone, iPad, Android, LG TV, Raspberry Pi. In a browser, there is nothing to install.
- **KyberNow Link OS**, a complete image for the Raspberry Pi: the board starts straight into KyberNow Link.

## The computer you share

### A Windows PC

- Windows 10 or 11, with an administrator account for the installation.
- An NVIDIA, AMD or Intel graphics card, external ones (eGPU) included. Without a graphics card, the processor compresses the picture, more slowly, and the installer tells you so.
- Microsoft Edge WebView2, already on Windows 11 and on most Windows 10 PCs. If it is missing, the installer opens Microsoft's page so you can download it.
- Steam, so the sound goes to your devices and the PC stays quiet. Without Steam, the sound also plays through the PC's speakers.
- For HDR, Windows 11 24H2 and a graphics card that encodes in 10-bit (HEVC or AV1).

### A Mac

- A Mac with Apple silicon (M1 or later), running macOS 15 Sequoia or later.
- Nothing to add for sound or controllers: the Station installs its own sound output. Apple silicon is all you need for HDR.

### A Linux computer

- Ubuntu 24.04, Debian 13 or Fedora 42, or a later release, on an x86_64 or arm64 processor. Bazzite, Fedora Silverblue or Kinoite and SteamOS have their own installation.
- GNOME or KDE Plasma 6 on Wayland, or Sway and Hyprland. On X11, the Station sends the computer's displays as they are, with no virtual display.
- An NVIDIA card with its proprietary driver, or an AMD or Intel card. Otherwise, the processor compresses the picture.
- For HDR, choose a Windows or Mac Station.

## Your devices

| Device | What you need |
|---|---|
| Mac | Apple silicon, macOS 14 or later |
| Windows PC | Windows 10 22H2 or Windows 11, x64 or ARM64 (ARM64 PCs, such as those with a Snapdragon chip, need Windows 11) |
| Linux PC | An x86_64 or ARM processor, and Flatpak |
| Steam Deck, Steam Machine, PC running SteamOS or Bazzite | Nothing else |
| Apple TV | An Apple TV 4K from 2021 or later, tvOS 18 or later |
| iPhone, iPad | iOS 18 or iPadOS 18, or later |
| Android | Android 11 or later: phone, tablet, Android TV, Google TV, NVIDIA Shield |
| LG TV | webOS 5 or later (2020 models onwards), and a computer to install the app from |
| Raspberry Pi | A Pi 3, 4 or 5 (Pi 400, Pi 500 and Compute Module included), and a microSD card of at least 8 GB for KyberNow Link OS |
| Browser | Chrome or Edge ideally, Safari and Firefox too |

To play your console, you need a PS5 or a PS4: see [Play your PS5 or PS4](/guide/playstation/).

## The network

- To add a device, it must be on the same network as the computer, or connected to it through a VPN. After that, away from home, it goes through the KyberNow relay: see [Reach your computer away from home](/guide/hors-de-chez-vous/).
- An Ethernet cable between the computer and your router avoids the brief dropouts of Wi-Fi.
- For the device, a cable too where you can, otherwise 5 GHz or 6 GHz Wi-Fi, close to the router.

## Price and account

KyberNow is free and open source (AGPL-3.0 licence). No online account is required: the account you create during installation stays on the computer.

A KyberNow account, also free, makes your computers appear on all your devices and lets you reach them away from home without changing anything on your router. See [Your KyberNow account](/guide/compte-kybernow/).

## Where to start

1. Install the Station on the computer you want to share: [on Windows](/guide/installer-station-windows/), [on Mac](/guide/installer-station-mac/) or [on Linux](/guide/installer-station-linux/).
2. Install KyberNow Link on a device and add the computer to it: see [Connect a device to your computer](/guide/connecter-un-appareil/).
