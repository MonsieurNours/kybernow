---
slug: televiseur-lg
slug_en: lg-tv
titre: Play on an LG TV
title: KyberNow Link on LG TV · KyberNow
description: Install KyberNow Link on an LG webOS TV in developer mode and play your computer in 4K at 120 Hz, with a controller.
rubrique: Connecter un appareil
rubrique_en: Connect a device
univers: jeu
duree: 15 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, profils-et-qualite, mettre-a-jour]
---

On an LG TV, the app goes up to 4K at 120 Hz, with four Bluetooth controllers and their rumble, the Magic Remote as a mouse and 5.1 Dolby sound to an amplifier. It installs in developer mode, which LG opens to everyone with a free account, without modifying anything on the TV.

## Before you start

- An LG TV running webOS 5 or later (2020 models onwards).
- A computer running Windows, macOS or Linux, on the same network as the TV.
- The app's file: [KyberNow-Link-webOS.ipk](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-webOS.ipk).
- Allow a quarter of an hour the first time, then a minute for each update.

## The LG developer account

1. On the computer, open webostv.developer.lge.com and create an account: Sign In › Create Account. It is free.
2. Confirm your email address.

## Developer mode on the TV

1. On the TV, open the LG Content Store, search for Developer Mode and install the app.
2. Open Developer Mode and sign in with the LG account.
3. Turn on Dev Mode Status. The TV restarts.
4. Open Developer Mode again, turn on Key Server, then note the TV's IP address and its Passphrase.

## Install the app

1. On the computer, install webOS Dev Manager, a free and open-source app for Windows, Mac and Linux.
2. Add the TV: Add device › Developer mode, with the IP address and the passphrase.
3. In the Apps tab, click Install, or drag the file into the window, and choose KyberNow-Link-webOS.ipk.
4. KyberNow Link appears in the TV's list of apps.

On the command line, LG's tools do the same thing (Node.js 18 or later):

```sh
npm install -g @webos-tools/cli
ares-setup-device --add tv -i "host=192.168.1.50" -i "port=9922" -i "username=prisoner"
ares-novacom --device tv --getkey
ares-install --device tv KyberNow-Link-webOS.ipk
```

Replace `192.168.1.50` with the TV's address. `ares-novacom` asks for the passphrase.

## Add your computer

1. Open KyberNow Link from the TV's home screen.
2. The computers on the same network appear at the bottom, under “On your network”. Choose yours: a six-digit code appears.
3. Accept it in Admin, in the Link devices space (on a PC, also in the panel of the icon by the clock). The TV gets its own access, which you can remove in Admin.
4. The computer shows large on the home screen: choose a profile, then “Connect”.

If the computer does not show up, choose “Add a computer or a console” › “With address and port”, type the address and Admin's port, shown in Admin › Link devices, under the QR code, then compare the fingerprint with the one in Admin.

## Developer mode does not last for ever

The time left shows in the Developer Mode app. When it reaches zero, the TV leaves developer mode and the apps installed this way disappear. The Extend Session button resets it to the maximum: remember to press it from time to time. webOS Dev Manager can extend it too.

## Recommended settings

- **Picture mode**: during the session, choose Game Optimiser (Magic Remote Settings button › Picture › Mode). Without it, the TV adds its own picture processing, and latency.
- **Controllers**: pair them with the TV over Bluetooth, in its settings. DualSense, DualShock 4, Xbox and Switch Pro work. App settings › Controllers › Test controllers shows the buttons and tries the rumble.
- **5.1 sound to an amplifier over eARC**: in the app, the computer's settings › Sound › 5.1 Dolby; on the TV, Sound › Digital Sound Output › Pass Through. Without this setting, the amplifier receives stereo.
- **Network**: the Ethernet port on LG TVs stops at 100 Mbit/s, and the app caps the bitrate there at 80 Mbit/s. 5 GHz Wi-Fi, close to the router, often goes further: the app goes up to 95 Mbit/s on it.

## Update the app

The app shows “Version X available” on the home screen. In developer mode, it cannot install itself: download the new KyberNow-Link-webOS.ipk and install it as you did the first time. It replaces the old one, and your computers stay.

## If something goes wrong

- **The app will not install, or has disappeared.** Developer mode has expired. Open Developer Mode again, turn it back on and extend the session, then reinstall the app.
- **Dev Manager no longer reaches the TV.** Its IP address has changed. Change it in Dev Manager.
- **Black screen in the session, but sound.** The TV's decoder refuses the stream. In the computer's settings, try Picture › Video format › H.264, or a lower resolution.
- **The amplifier receives stereo.** Check Pass Through on the TV and 5.1 Dolby in the app.
