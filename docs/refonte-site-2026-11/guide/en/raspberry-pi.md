---
slug: raspberry-pi
slug_en: raspberry-pi
titre: Plug a Raspberry Pi into the TV
title: KyberNow Link on Raspberry Pi · KyberNow
description: Copy KyberNow Link OS onto a microSD card or install the app on Raspberry Pi OS, and use your computer on the TV.
rubrique: Connecter un appareil
rubrique_en: Connect a device
univers: jeu
connexes: [connecter-un-appareil, clavier-et-raccourcis, profils-et-qualite, mettre-a-jour]
---

Copy the image onto a microSD card and plug the Pi into the TV: it starts straight into KyberNow Link. If you already use Raspberry Pi OS, the app on its own installs there too.

## Before you start

- A Raspberry Pi 3, 4 or 5 (Pi 400, Pi 500 and Compute Module included).
- For KyberNow Link OS, a microSD card of at least 8 GB, or an SSD.
- A controller, a keyboard, or the TV's remote.

The Pi 4 and 5 decode the picture with their chip, in HEVC, up to 4K at 60 Hz; the Pi 3 in H.264. Over HDMI, expect 4K at 60 Hz at most, or 1080p at 120 Hz if the TV allows it, without variable refresh rate.

## KyberNow Link OS, on the microSD card

The Pi is then used for KyberNow alone and starts up like a console: at the TV with a controller, or at a desk with a keyboard and mouse.

1. Download [KyberNow-Link-OS.img.xz](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-OS.img.xz).
2. Open Raspberry Pi Imager, the official tool for preparing cards, then Choose OS › Use custom, and pick the file. Do not customise anything in Imager: everything is set at first start.
3. Plug the Pi into the TV over HDMI (on Pi 4 and 5, the HDMI 0 port, the one closest to the power socket), into the router with an Ethernet cable if you can, then into the power. The KyberNow mark appears, then lights up with a sound.

> Good to know: you can also paste `https://www.kybernow.app/updates/rpi-imager.json` into Imager › Settings › Custom repository. “KyberNow Link OS” then appears in the list of operating systems.

## The first start

1. Put your controller in pairing mode: on an Xbox controller, the pair button near the USB port; on a DualSense, Create + PS; on a Switch Pro, SYNC. The Pi accepts it without a code. A USB controller or a keyboard work too, as does the TV's remote if the TV supports HDMI-CEC, the control of devices plugged in over HDMI.
2. Choose the language.
3. Choose the network. Ethernet is recognised by itself; otherwise, choose the Wi-Fi network and type its password.
4. Choose how you will use it: Living room or Desktop.
5. Choose your computer. A six-digit code appears. Accept it in Admin, in the Link devices space (on a PC, also in the panel of the icon by the clock).

## Day to day

- ☰ on the home screen opens Device settings: network, controllers, display and HDMI-CEC, sound, update, language and advanced settings.
- Holding Start for three seconds switches from Living room to Desktop, and back.
- Guide (or PS) puts the Pi to sleep, and the TV with it through HDMI-CEC. Any button wakes it.
- In a session, L1 + R1 + Start + Select opens the session menu.

## Update KyberNow Link OS

Open Device settings › Update. The new version downloads, as soon as it is out if “Automatic” is ticked. The card has two slots for the system: “Restart on X” starts the new version in whichever slot is free. If it does not make it to the home screen, the Pi goes back to the previous version by itself. Your computers, settings, Wi-Fi and paired controllers stay.

This update needs a Pi 4 or 5. On a Pi 3, copy the new image onto the card.

## The app on its own, on Raspberry Pi OS

1. Download [KyberNow-Link-RaspberryPi-arm64.deb](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-RaspberryPi-arm64.deb), for 64-bit Raspberry Pi OS (Debian 13).
2. Install it with a double-click in the file manager, or in a terminal:

   ```sh
   sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb
   ```

3. KyberNow Link appears in the menu, under Games. Add your computer: it shows up by itself on the same network, and you accept the code in Admin.
4. At the TV, with a controller, choose App settings › Interface › Living room for the interface you can read from three metres away: A confirms, B goes back, Y opens the settings, X adds a computer.

When a new version comes out, the app tells you: install the new file the same way.

> Good to know: on a Pi 5, the app also plays PyroWave, a picture format that takes a quarter of a millisecond at most to compute, at the cost of a bitrate of 200 Mbit/s or more: it needs an Ethernet cable. See [Profiles, video format, bitrate and HDR](/guide/profils-et-qualite/).

## If something goes wrong

- **Nothing shows on the TV.** On Pi 4 and 5, plug the HDMI cable into the HDMI 0 port, the one closest to the power socket.
- **The controller is not recognised.** Put it back in pairing mode, or plug it in over USB.
- **The TV's remote does nothing.** Turn on HDMI-CEC in the TV's settings (each brand gives it its own name).
- **The Pi goes back to the previous version after an update.** The new version did not reach the home screen. Device settings › Update says so; try again with the next version.
- **The computer does not show up.** It must be on, on the same network, with KyberNow Station running.
