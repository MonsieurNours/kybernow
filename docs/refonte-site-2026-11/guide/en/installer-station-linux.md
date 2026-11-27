---
slug: installer-station-linux
slug_en: install-station-linux
titre: Install KyberNow Station on Linux
title: Install KyberNow Station on Linux · KyberNow
description: Install KyberNow Station on Ubuntu, Debian, Fedora, Bazzite or SteamOS, then find your account and open KyberNow Admin.
rubrique: Installer la Station
rubrique_en: Install the Station
univers: commun
duree: 5 min
connexes: [ce-qu-il-vous-faut, connecter-un-appareil, link-linux-steam-deck, mettre-a-jour]
---

Your Linux computer becomes a Station, with the same Admin and the same apps as a Windows PC. It installs as a .deb or .rpm package, or in your home folder on Bazzite, Fedora Silverblue or Kinoite and SteamOS.

## Before you start

- Ubuntu 24.04, Debian 13 or Fedora 42, or a later release. Bazzite, Fedora Silverblue or Kinoite and SteamOS use the archive, further down.
- An x86_64 processor (most PCs) or arm64.
- GNOME or KDE Plasma 6 on Wayland, or Sway and Hyprland, for the virtual display sized for each device. On X11 and other desktops, the Station sends the computer's displays as they are.
- Your user account, with the right to use `sudo`. This is the account that can then update the Station from Admin.

The Station picks the encoder to suit your card: NVENC for an NVIDIA card with its proprietary driver, VA-API for AMD and Intel, otherwise the processor.

## Ubuntu, Debian, Fedora

1. Download the package for your system and processor:

   | | x86_64 | arm64 |
   |---|---|---|
   | Ubuntu, Debian | [KyberNow-Station-Linux-x86_64.deb](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-x86_64.deb) | [KyberNow-Station-Linux-arm64.deb](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-arm64.deb) |
   | Fedora | [KyberNow-Station-Linux-x86_64.rpm](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-x86_64.rpm) | [KyberNow-Station-Linux-arm64.rpm](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-arm64.rpm) |

2. Open a terminal in the download folder and install it:

   ```sh
   sudo apt install ./KyberNow-Station-Linux-x86_64.deb     # Ubuntu, Debian
   sudo dnf install ./KyberNow-Station-Linux-x86_64.rpm     # Fedora
   ```

   If the system firewall is running (firewalld or ufw), the package opens the Station's ports in it.

3. Log out, then log back in: the Station starts with your session. To launch it straight away, without logging out:

   ```sh
   systemctl --user start kybernow-admin kybernow-kyber kybernow-tray
   ```

## Bazzite, Fedora Silverblue or Kinoite, SteamOS

On these systems, the system folder cannot be changed. The Station installs in your home folder, without a restart, and stays in place when the system updates. It then updates from Admin, with no password.

1. On SteamOS, switch to Desktop Mode: Steam button › Power › Switch to Desktop. The Station works in Desktop Mode.
2. Download [KyberNow-Station-Linux-x86_64.tar.xz](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-x86_64.tar.xz), or [KyberNow-Station-Linux-aarch64.tar.xz](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Linux-aarch64.tar.xz) for an ARM machine.
3. Open a terminal (Konsole) in the download folder and type:

   ```sh
   tar -xf KyberNow-Station-Linux-x86_64.tar.xz
   ./kybernow-station-*/install.sh
   ```

4. If you are asked for your password, it is so your devices' keyboard, mouse and controllers can drive the computer: a system rule, added once. On most machines with Steam you are not asked, because Steam has already done it. On SteamOS, if you have never set a password, type `passwd` first.
5. The installation shows Admin's address and where to find your first account's password.

## Your first account

The Station creates an account the first time it starts. Its username is your Linux username. Its password is picked at random and stored in a file that only you can read:

```sh
cat ~/.local/share/kybernow/station/admin/mot-de-passe
```

You use this account to open Admin and to connect your devices. You can change its password and create more accounts in Admin, in the Users space.

## Open Admin

A KyberNow Station icon appears next to the clock. Its menu offers “Open Admin”, “Restart the Station”, “Share a screen…” and “Lend the computer…”.

- On KDE Plasma, the icon shows up by itself.
- On GNOME, it needs the AppIndicator extension, included as standard on Ubuntu. Elsewhere, install the `gnome-shell-extension-appindicator` package.

Admin opens in your browser. The first time, the browser shows a security warning, because the Station signs its own connection: accept it. Admin has the same spaces as on a PC: Overview, Picture and sound, Recordings, Link devices, Lending, Users, System and KyberNow account. To attach the computer to your account, open KyberNow account: see [Your KyberNow account](/guide/compte-kybernow/).

All that is left is to [connect a device](/guide/connecter-un-appareil/).

## After a restart

The Station runs in your session: it starts when you log in and stops when you log out. A locked screen streams too, and you type the password from your device.

For the Station to be there after a restart with nobody at the computer, turn on automatic login in your system settings. On SteamOS and Bazzite Deck, which start in Game Mode, make the computer start in Desktop Mode:

```sh
steamos-session-select plasma-persistent
```

> Good to know: your devices' camera reaches the computer as “KyberNow Camera”, which browsers and OBS can see. For Zoom, Discord and other apps to see it too, install v4l2loopback (`sudo apt install v4l2loopback-dkms` on Ubuntu and Debian, `sudo dnf install akmod-v4l2loopback` on Fedora, with the RPM Fusion repository), then restart. See [Stylus, microphone, camera and recording](/guide/stylet-micro-webcam/).

## What a Linux Station does

- The virtual display sized for each device, on GNOME, KDE Plasma, Sway and Hyprland, with one display or several.
- Sound in stereo, 5.1 or 7.1, through a “KyberNow” output; the original output comes back at the end.
- Controllers: a PlayStation controller arrives as a DualSense, the others as an Xbox controller.
- The stylus, the clipboard, lending the computer, and PyroWave with a graphics card that supports Vulkan 1.3.

## If something goes wrong

- **No icon by the clock on GNOME.** Install and turn on the AppIndicator extension, then log back in.
- **Your devices cannot find the computer.** Check they are on the same network, and that your firewall lets the Station through. Over a VPN, add the computer by its address: see [Connect a device to your computer](/guide/connecter-un-appareil/).
- **Updating from Admin does not go through.** If the Station was installed some other way than with `sudo` or `pkexec`, your account does not yet have the right to update it. Add it to the Station's group, then log back in:

  ```sh
  sudo usermod -aG kybernow-station YOUR_USERNAME
  ```

- **No virtual display.** The session is on X11 or on a desktop the Station does not support. Log in to a GNOME or KDE Plasma session on Wayland.
- **Two accounts logged in at once.** Only one session at a time carries the Station. Log out of the other.
