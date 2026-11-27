---
slug: link-linux-steam-deck
slug_en: link-linux-steam-deck
titre: Use your computer from Linux or a Steam Deck
title: KyberNow Link on Linux and Steam Deck · KyberNow
description: Install KyberNow Link as a Flatpak on a Linux PC, a Steam Deck or a Steam Machine, and play your computer with a controller.
rubrique: Connecter un appareil
rubrique_en: Connect a device
univers: commun
duree: 10 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, profils-et-qualite, installer-station-linux]
---

The same app works on a Linux PC, on the Steam Deck and in the living room, on a Steam Machine or a PC running SteamOS. It comes as a Flatpak package, which installs for you alone, with no administrator rights.

## What the app does

- On a PC, you use it with the mouse and keyboard, with a session bar at the top of the screen.
- On the Deck, it switches to a handheld interface, driven by controller, with a legend for the buttons.
- On a TV, it moves to the living-room interface, readable from three metres away, with its “Ready to play” column: controllers, sound, display and decoders.
- It decodes the picture on the graphics card whenever it can (VA-API), supports variable refresh rate on displays that have it, stereo, 5.1 or 7.1 sound, and four controllers with their rumble.

## On a Linux PC

1. Download the package for your machine: [KyberNow-Link-Linux-x86_64.flatpak](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Linux-x86_64.flatpak) for most PCs, [KyberNow-Link-Linux-aarch64.flatpak](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Linux-aarch64.flatpak) for an ARM machine.
2. Double-click the file: GNOME Software or KDE Discover installs it. You can also open a terminal in the file's folder and type:

   ```sh
   flatpak install --user KyberNow-Link-Linux-x86_64.flatpak
   ```

   The app needs the shared base of Flatpak apps, the org.freedesktop.Platform 25.08 runtime. If it is missing, Flatpak offers to download it from Flathub: accept.
3. Open KyberNow Link from the applications menu.

Flatpak is already installed on most distributions (otherwise, see flatpak.org/setup). If it does not know the Flathub repository yet, add it and install the runtime by hand:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Platform//25.08
```

## On the Steam Deck

You install the app from Desktop Mode, then launch it from Game Mode, like a game.

1. Switch to Desktop Mode: Steam button › Power › Switch to Desktop.
2. With Firefox or another browser, download [KyberNow-Link-Linux-x86_64.flatpak](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Linux-x86_64.flatpak).
3. In the downloads folder, double-click the file: Discover installs it. You can also open Konsole and type:

   ```sh
   flatpak install --user ~/Téléchargements/KyberNow-Link-Linux-x86_64.flatpak
   ```

   (`~/Downloads/` if the Deck is in English.)
4. Add the app to Steam: open Steam in Desktop Mode, Games › Add a Non-Steam Game, tick KyberNow Link, then Add Selected Programs.
5. Go back to Game Mode, with the “Return to Gaming Mode” icon on the desktop. KyberNow Link is in the library, in the Non-Steam tab.
6. On KyberNow Link's page, controller icon › Layout, choose the “Gamepad with Mouse Trackpad” template. The sticks and buttons go to the computer's controller, the right trackpad moves the mouse, and its clicks click.

## In the living room

On a Steam Machine, a PC running SteamOS or Bazzite plugged into the TV, or the Deck in its dock, the installation is the same. The app moves to the living-room interface by itself as soon as the display is larger than 1280 × 800 in Game Mode. App settings › Interface lets you force Desktop, Handheld or Living room.

## Add your computer

1. Launch KyberNow Link. On the same network, your computer shows up by itself: choose it.
2. The app shows a six-digit code. Accept it in Admin, in the Link devices space (on a PC, also in the panel of the icon by the clock). Nothing to type.
3. Choose a profile, then “Connect”.

Over a VPN, choose “Add a computer or a console”, then “By address”: the computer's IP address or name, and Admin's port. Then compare the first four groups of the fingerprint with those Admin shows. On the Deck, type with the on-screen keypad or Steam's keyboard.

## With a controller

On the Deck and in the living room:

- A confirms, B goes back, Y opens the computer's settings, X adds a computer.
- L1 and R1 change the profile, Start opens the app's settings.
- In a session, L1 + R1 + Start + Select held together, or a tap at the top of the screen, opens the session menu. “Keyboard” there opens Steam's keyboard.

On a PC, with the keyboard: Ctrl+Alt+Shift+D disconnects, Ctrl+Alt+Shift+F goes full screen, Ctrl+Alt+Shift+M switches between the desktop mouse and the gaming mouse, and holding Esc for a second releases the mouse. See [Keyboard, mouse and shortcuts](/guide/clavier-et-raccourcis/).

## Recommended settings

- **On the Deck**: the defaults, that is the Game profile, the “Match this display” resolution (1280 × 800 on the Deck, at its refresh rate) and “Frame display” on Auto. “Lighter on battery”, when on, limits the unplugged Deck to 60 frames per second and 20 Mbit/s.
- **On a variable refresh rate display** (FreeSync, VRR): leave “Frame display” on Auto. Each frame shows as soon as it is decoded, and the display follows the stream's pace. The Steam Deck's own screen does not support it.
- **For really sharp text on the desktop**: turn on Computer settings › Picture › Sharp text (4:4:4). It only works in HEVC and needs more bitrate; the processor decodes if the graphics card cannot.
- **The network**: 5 GHz or 6 GHz Wi-Fi close to the router, or an Ethernet cable on the dock.

## Update the app

A Flatpak installed from a file does not update by itself. The app tells you about a new version (“Version X available”): download the new file and install it the same way. Your computers and settings stay.

## If something goes wrong

- **Flatpak cannot find the runtime.** Add the Flathub repository and install org.freedesktop.Platform 25.08, with the two commands above.
- **The Deck's trackpad does not move the mouse.** Check the “Gamepad with Mouse Trackpad” layout on KyberNow Link's page.
- **The app stays in the handheld interface on the TV.** Force Living room in App settings › Interface.
- **To report a problem.** Attach the app's log, in `~/.var/app/fr.leslignesclaires.kybernow/config/kybernow/kybernow.log`.
