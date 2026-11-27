---
slug: android
slug_en: android
titre: Use your computer from Android
title: KyberNow Link on Android and Android TV · KyberNow
description: Install KyberNow on an Android phone, tablet or TV, then add your computer over the network, with a code or with a QR code.
rubrique: Connecter un appareil
rubrique_en: Connect a device
univers: commun
duree: 5 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, profils-et-qualite, hors-de-chez-vous]
---

The same app works on phones, tablets and TVs (Android TV, Google TV, NVIDIA Shield), from Android 11 onwards. On a TV, it goes up to 4K at 60 Hz, with 7.1 sound to an HDMI amplifier, Bluetooth controllers and the remote.

## Before you start

- A device running Android 11 or later.
- The Station installed on the computer you want to reach.
- For a TV, a computer on the same network to install the app from.

The app installs from its file, [KyberNow-Link-Android.apk](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Android.apk).

## On a phone or tablet

1. On the device, open the KyberNow-Link-Android.apk link, then the downloaded file.
2. Android asks you to allow your browser to install apps: allow it, go back, then tap “Install”.
3. Open KyberNow.

## On a TV

The simplest way is through a computer on the same network, with adb, Android's developer tool. Allow five minutes the first time.

1. On the TV, turn on developer options: Settings › Device Preferences › About (on Google TV: Settings › System › About), then press Build seven times.
2. In Developer options, turn on Network debugging (or ADB debugging), and note the TV's address.
3. On the computer, download KyberNow-Link-Android.apk and install adb: on a Mac with Homebrew, `brew install android-platform-tools`; elsewhere, Google's SDK Platform-Tools.
4. Connect the computer to the TV, then install the app, from the folder of the downloaded file:

   ```sh
   adb connect 192.168.1.30
   adb install KyberNow-Link-Android.apk
   ```

   Replace the address with the TV's. On the first `adb connect`, the TV asks you to allow debugging: tick “Always allow” and confirm. You can turn debugging off afterwards.
5. Open KyberNow from the TV's home screen.

## Add your computer

The first time, three welcome screens offer you a KyberNow account, which is optional. On a TV, you sign in to the account with a code that you approve from your phone. See [Your KyberNow account](/guide/compte-kybernow/).

Then choose “Add a computer or a console”, and one of the tabs:

- **Network**: your computer shows up by itself.
- **Code**: tap “Show a code”. The app shows a six-digit code. Accept it in Admin, in the Link devices space (on a PC, also in the panel of the icon by the clock). Nothing to type.
- **Address**: the computer's IP address or name, and Admin's port. This is the one to choose over a VPN. Then compare the four groups of the fingerprint with those shown in Admin › Link devices, and tap “They match”. Enter the Station's account, or go “With a code”.
- **QR code**, on a phone or tablet: in Admin, in the Link devices space, “Add a device” shows the QR code. Point the camera at it: the computer's fingerprint is checked automatically.

Choose a profile (Game, Desktop, Max or Auto), then “Connect”.

## During the session

- On phones and tablets, the session bar gives you the keyboard and the on-screen controller, with the “Full” or “Simple” layout. An Android stylus reaches the computer as a real stylus.
- On a TV, the controller and the remote drive the app. Pair controllers over Bluetooth in the TV's settings.

The details are in [Keyboard, mouse and shortcuts](/guide/clavier-et-raccourcis/).

## Recommended settings

- **On a TV**: an Ethernet cable if the TV or the router allows it, otherwise 5 GHz or 6 GHz Wi-Fi close to the router.
- **Sound**: for 7.1, go through an amplifier connected over HDMI. Over Bluetooth, the sound often arrives late.

## If something goes wrong

- **Android refuses to install the file.** Allow your browser, or your file manager, to install apps in Android's settings.
- **`adb connect` does not respond.** Check the TV's address and that network debugging is turned on.
- **“No Station found on your network yet”.** The computer must be on, on the same network, with KyberNow Station running. Over a VPN, use the Address tab.
- **“Camera access is denied”.** Allow the camera for KyberNow in Android's settings.
- **“The code expired: start again.”** Show a new code and accept it more quickly in Admin.
- **“This Station does not know codes”.** Update the Station on the computer, or add the computer with its account.
