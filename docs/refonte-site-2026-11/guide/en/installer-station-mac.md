---
slug: installer-station-mac
slug_en: install-station-mac
titre: Install KyberNow Station on Mac
title: Install KyberNow Station on Mac · KyberNow
description: Install KyberNow Station on a Mac with Apple silicon, grant macOS its three permissions and use this Mac from your other screens.
rubrique: Installer la Station
rubrique_en: Install the Station
univers: commun
duree: 5 min
connexes: [ce-qu-il-vous-faut, connecter-un-appareil, clavier-et-raccourcis, compte-kybernow]
---

Your Mac becomes a Station just like a PC: the same Admin, the same apps to connect to it, with a virtual display sized for each device and sound up to 7.1. Installation takes about two minutes, and nothing changes on the Mac until you click “Install”.

## Before you start

- A Mac with Apple silicon (M1 or later), running macOS 15 Sequoia or later.
- Your macOS account password, asked for once.
- An Ethernet cable to the router, if you can: the installer prefers it to Wi-Fi for gaming.

## Open the installer

1. On the Mac you want to share, download [KyberNow-Station-Mac.dmg](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Mac.dmg) and open it.
2. Launch “Install KyberNow Station”. The app is signed and notarised by Apple, so macOS opens it without a warning.

## Welcome

The installer checks the Mac: Apple chip, macOS version, local network, network ports, displays and sound on your devices. A line in red prevents installation (a Mac with an Intel processor, or a macOS version that is too old).

It chooses two network ports, one for the stream (the picture, sound and controls), the other for KyberNow Admin. “Change” lets you choose others, then “Apply”. Click “Continue”.

If the Station is already on this Mac, a screen asks what to do with its settings. “Keep my settings” keeps the accounts, added devices, certificate and ports: your devices reconnect with nothing to redo. “Start from scratch” creates a new account, new ports and a new certificate, and you will need to add each device again.

## Your account

1. Choose a username.
2. Choose a password of at least 10 characters.
3. Click “Continue”.

This account protects the Mac. You use it to open Admin and to connect from your devices, which can remember it. The Mac keeps only a hash of it, never the password itself. You can create more accounts later in Admin, in the Users space.

## Options

Everything is already set, and you can change any of it later in Admin.

- **Virtual display sized for each device**: the picture arrives at the device's resolution and refresh rate, in HDR if it asks for it.
- **Sound on your devices**: the sound plays on your device, up to 7.1, and the Mac stays silent. A “KyberNow” sound output is added to the Mac.
- **Start with the Mac**: the Station is available as soon as you log in.
- **Keep the Mac reachable**: no sleep during a session, and wake over the network when a device asks.
- **Controllers**: the Mac sees your device's controller as a real controller.

Click “Install”. macOS then asks for your Mac's password, once only: to copy the app into Applications, add the sound output, open the firewall if it is switched on and allow wake over the network.

## Installation

There is nothing for you to do: each step appears as it happens (Kyber engine, the Mac's certificate, KyberNow sound output, start with the Mac, KyberNow Admin, access from your devices).

Meanwhile, the installer offers to attach the Mac to your KyberNow account, as on a PC. This is optional: “Sign in” or “Create an account”, then the relay's page, where you confirm attaching the Mac. The passkey can use Touch ID. “Later” skips this screen; you can do it in Admin › KyberNow account. See [Your KyberNow account](/guide/compte-kybernow/).

## Permissions

macOS asks you for three permissions for “KyberNow Station”. For each one, click “Open setting” or “Allow”, then turn on “KyberNow Station” in the macOS setting. The line ticks itself.

1. **Screen & System Audio Recording**, to send the Mac's picture to your devices.
2. **Accessibility**, so your devices' keyboard and mouse can control the Mac.
3. **Microphone**, to read the session's sound on the KyberNow output, which macOS files with the microphones. No real microphone is listened to.

“Later” skips this screen, but the Station then only half works: no picture, no controls or no sound. You can grant these permissions from the menu bar icon, in the Permissions menu.

<!-- capture: Permissions screen, three lines ticked -->

## This Mac is ready

The final screen gives the Mac's address, to enter on a device that does not find it on its own, a QR code to “Open Admin on your phone”, and the address to open in a browser to use the Mac without an app. “Open Admin” opens the settings in your browser. The first time, the browser shows a security warning, because the Station signs its own connection: accept it.

The KyberNow Station icon is in the menu bar, at the top right, with the Station's status and a way into Admin. Admin has the same spaces as on a PC.

All that is left is to [connect a device](/guide/connecter-un-appareil/).

## During a session

- The Mac creates a virtual display sized for the device and turns off its own displays, as a PC does. They come back on at the end, in the same arrangement.
- The sound goes to the device through the KyberNow output, in stereo, 5.1 or 7.1, and the previous output comes back at the end.
- A locked Mac streams too: you type its password from your device.
- The Station runs in your macOS session. After the Mac restarts, log in once in front of it.
- From a PC or a TV, the ⊞ key acts as ⌘. The session bar's “Keys” menu offers the Mac's key combinations, and the “Ctrl acts as ⌘” setting lets you copy and paste with Ctrl. See [Keyboard, mouse and shortcuts](/guide/clavier-et-raccourcis/).

> Good to know: macOS 15 reminds you from time to time that an app is recording the screen. Confirm: it is the Station.

## If something goes wrong

- **“Installation canceled”.** macOS needs your Mac's password to put the app in Applications and add the sound output. Run the installer again and type it in.
- **No picture on the device.** The Screen Recording permission is missing. Grant it from the menu bar icon, in the Permissions menu.
- **The keyboard and mouse do nothing.** The Accessibility permission is missing.
- **No sound.** The Microphone permission is missing. If the installer says the sound output is in place but macOS does not show it yet, it will appear the next time the Mac starts.
- **Nothing responds after a restart.** Log in on the Mac. The Station starts with your session.
