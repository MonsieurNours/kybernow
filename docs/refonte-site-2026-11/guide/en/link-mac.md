---
slug: link-mac
slug_en: link-mac
titre: Use your computer from a Mac
title: KyberNow Link on Mac · KyberNow
description: Install KyberNow Link on your Mac, add your computer over the network, by its address or with a QR code, and open the session.
rubrique: Connecter un appareil
rubrique_en: Connect a device
univers: commun
duree: 2 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, plusieurs-ecrans-et-ordinateurs, profils-et-qualite]
---

The Mac app shows your computer full screen or in a window, with the keyboard, mouse, controllers and sound. It stays in the menu bar, ready to reopen a session in one click.

## Before you start

- A Mac with Apple silicon, running macOS 14 or later.
- The Station installed on the computer you want to reach, and the account created when it was installed.

## Install the app

1. Download [KyberNow-Link-macOS.zip](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-macOS.zip) and open it.
2. Drag KyberNow Link into the Applications folder. The app is signed and notarised by Apple.
3. Open it. The first time, three welcome screens offer you a KyberNow account, which is optional: see [Your KyberNow account](/guide/compte-kybernow/).

## Add your computer

On the home screen, choose “Add a computer or a console”, then one of three ways:

- **On this network**: your computer shows up by itself. Click “Add”.
- **By address**: the computer's IP address or name, and Admin's port, shown at the end of the Station's installation and in Admin. This is the one to choose over a VPN. Click “Continue”.
- **QR code or link**: in Admin, in the Link devices space, click “Add a device”, then hold the QR code up to the Mac's camera, or paste the link and click “Continue”.

The “With a friend account” and “PlayStation” tabs are for [borrowing a computer](/guide/preter/) and [playing your console](/guide/playstation/).

## Check the fingerprint

Found on the network or added with the link, the computer is already checked: the app shows “Fingerprint verified: this is the announced computer.” Added by its address, compare the first four groups of the fingerprint with those Admin shows: see [Connect a device to your computer](/guide/connecter-un-appareil/).

## The computer's account

1. Enter the username and password chosen when the Station was installed. The Mac keeps them in its Keychain: next time, one click on “Connect” is enough.
2. Click “Add the computer”.
3. On the home screen, choose a profile (Game, Desktop, Max or Auto), then “Connect”. The session opens.

## During the session

- Move the pointer to the top edge of the picture: the session bar opens, with the profile, sound, mouse, “Statistics” and “Disconnect”.
- The app's shortcuts are ⌃⌥⌘ followed by a letter: D to disconnect, F for full screen, M to switch between the desktop mouse and the gaming mouse, S for statistics.
- In full screen, immersive mode also sends ⌘ ⇥, ⌘ Space and the Mac's other shortcuts to the computer. The first time, macOS asks you to allow KyberNow Link in Accessibility.

The details are in [Keyboard, mouse and shortcuts](/guide/clavier-et-raccourcis/). With several displays, or several computers at once, see [Several displays and several computers](/guide/plusieurs-ecrans-et-ordinateurs/).

## In the menu bar

Closing the home window does not quit the app: it stays in the menu bar. Its menu lists your computers with their state, and “Connect” opens the session with the computer's profile. The app starts with your Mac, without a window, as long as “Open at login” is ticked in the app's Settings. To quit it, choose “Quit KyberNow Link”.

## If something goes wrong

- **“No computer found yet”.** The computer may be on another network, over a VPN, or its Station may not be up to date. Use the By address tab.
- **The camera does not read the QR code.** Allow the camera for KyberNow Link in System Settings › Privacy & Security, or paste the link.
- **The Mac's shortcuts stay on the Mac in full screen.** Allow KyberNow Link in System Settings › Privacy & Security › Accessibility.
- **“Password refused: update it in the settings.”** Enter the new password in the computer's settings, in the Station section.
