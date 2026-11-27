---
slug: installer-station-windows
slug_en: install-station-windows
titre: Install KyberNow Station on Windows
title: Install KyberNow Station on Windows · KyberNow
description: Install KyberNow Station on your Windows PC in three minutes, screen by screen, then open KyberNow Admin to set it up.
rubrique: Installer la Station
rubrique_en: Install the Station
univers: commun
duree: 5 min
connexes: [ce-qu-il-vous-faut, connecter-un-appareil, compte-kybernow, mettre-a-jour]
---

The installer sets up the Station on the PC you want to share: the Kyber engine, KyberNow Admin, the virtual display, controllers and sound. It takes about three minutes, and nothing changes on the PC until you click “Install”.

## Before you start

- A PC running Windows 10 or 11, and an administrator account.
- An up-to-date driver for your graphics card (NVIDIA, AMD or Intel).
- Steam installed, so the sound goes to your devices.
- The full list is in [What you need to get started](/guide/ce-qu-il-vous-faut/).

## Open the installer

1. On the PC you want to share, download [KyberNow-Station-Setup.exe](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Setup.exe) and open it.
2. If Windows SmartScreen stops it from opening, click “More info”, then “Run anyway”.
3. Windows asks for administrator permission, because the installer adds drivers and services. The window shows the publisher as unknown: KyberNow's Windows files do not carry an Authenticode signature yet, the signature that names a program's publisher. Accept.

## Welcome

The installer checks the PC, line by line: graphics card, graphics driver, local network, network ports, main display, power mode and sound on your devices. An orange dot marks a piece of advice; it does not block the installation.

It picks two free network ports at random, between 20000 and 44999, avoiding those used by Windows, Steam and Sunshine. A port is a numbered way in to the PC: your devices use one for the stream (the picture, sound and controls) and the other for KyberNow Admin. “Change” lets you choose others, then “Apply”.

If the PC's network is set to “Public” in Windows, the installer will switch it to “Private”: otherwise, your devices would not see the PC.

Click “Continue”.

<!-- capture: Welcome screen with the list of checks -->

## Reinstallation

This screen only appears if the Station is already on the PC.

- “Keep my settings” replaces only the programs and drivers. Accounts, added devices, certificate, Admin settings and ports stay as they are: your devices reconnect with nothing to redo.
- “Start from scratch” creates a new account, new ports and a new certificate. All your devices lose access, and you will need to add them again.

## Your account

1. Choose a username.
2. Choose a password of at least 10 characters. A gauge shows how strong it is.
3. Click “Continue”.

This account stays on the PC. You use it to open Admin and to connect your devices, which can remember it. The PC keeps only a hash of it, never the password itself. You can create more accounts later in Admin, in the Users space.

## Options

Everything is already ticked for you, and you can change any of it later in Admin.

- **Virtual display sized for each device**: the PC creates a display at the resolution and refresh rate of the device that connects. The free driver that creates this display (SudoVDA) is not signed by Microsoft: its certificate is added to the certificates Windows trusts, and removed if you uninstall the Station.
- **Controllers**: the PC sees your device's controller as a real controller (ViGEmBus driver), an Xbox 360 controller by default. You may be asked to restart.
- **Sound on your devices**: the sound plays on your device, up to 7.1, and the PC stays quiet. Steam's virtual speakers take care of it.
- **Start with the computer**: the Station is ready as soon as the PC starts, even before you sign in to Windows.
- **Maximum performance**: the processor and graphics card keep their full speed during a session, so the picture's delay stays steady.

Click “Install”.

## Installation

There is nothing for you to do: each step appears as it happens. The installer puts in place the Kyber engine, the PC's certificate, the Windows service, the virtual display, controllers, sound, your devices' microphone (VB-CABLE) and Admin.

It also opens Windows Firewall, for your private network only: the two ports, plus port 5353, which lets your devices find the PC on their own.

### Meanwhile: your KyberNow account

The installation carries on in the background while the installer offers to attach the PC to a KyberNow account. This is optional: the account makes the PC appear on all your devices, at home or away, and lets you open it to other people.

1. Choose “Sign in” if you already have an account, or “Create an account” (an email address and a password, or a passkey).
2. The relay's page opens in the installer window. Sign in, then confirm attaching the PC. If your account manages a family or a team, choose who the PC belongs to.
3. The installer shows “Attached to your account”, with “KyberNow accounts accepted” and “Away-from-home access on”.

The relay offered is `share.kybernow.app`. “Other relay…” is for when you run [your own relay](/guide/installer-votre-relais/). “Later” skips this screen: you can attach the PC in Admin › KyberNow account. See [Your KyberNow account](/guide/compte-kybernow/).

## This computer is ready

The final screen shows:

- the PC's address, to enter on a device that does not find it on its own;
- a QR code, “Open Admin on your phone”;
- the address to open in a browser to use the PC without an app;
- “Restart now”, if the controllers need it: they will only work after the restart, and everything else already works.

“Open Admin” opens the settings in your browser. The first time, the browser shows a security warning. This is expected: the Station encrypts the connection with a certificate it created itself, which the browser does not know. Accept it to carry on. The KyberNow Link apps recognise this certificate by its fingerprint.

## The icon and Admin

A KyberNow Station icon appears next to the Windows clock. Its dot is green when the Station is running, purple during a session. A click opens a panel with the Station's status and a way into Admin; a right-click opens a menu with “Open Admin”, “Restart the Station”, “Share a screen…” and “Lend the computer…”.

Admin is organised into spaces: Overview, Picture and sound, Recordings, Link devices, Lending, Users, System and KyberNow account.

All that is left is to [connect a device](/guide/connecter-un-appareil/).

> Good to know: to install the Station on a team's PCs without going through these screens, the installer also runs silently, with an enrollment token. See [Families and teams](/guide/familles-et-equipes/).

## If something goes wrong

- **“No compatible graphics card”.** Install the manufacturer's driver (NVIDIA, AMD or Intel), then run the installer again. Without it, the processor compresses the picture, more slowly.
- **“Steam is not installed”.** The sound will also play on the PC. Install Steam, then run the installation again so the sound only goes to your devices.
- **A message mentions Microsoft Edge WebView2.** This Microsoft component is missing from the PC, and the installer needs it to display. Install it from the Microsoft page that opens, then run the installer again.
- **“Kyber is already installed”.** The installer removes it and replaces it with the Station, which contains the same engine.
- **Controllers do not respond.** Restart the PC once.
- **Your devices do not see the PC.** Check they are on the same network. Over a VPN, add the PC by its address: see [Connect a device to your computer](/guide/connecter-un-appareil/).
- **The power mode has not changed.** Windows refused it, and everything else is installed. Choose “High performance” in Windows' power settings.
