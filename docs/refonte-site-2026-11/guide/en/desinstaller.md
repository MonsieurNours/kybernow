---
slug: desinstaller
slug_en: uninstall
titre: Uninstall KyberNow
title: Uninstall the Station and the apps · KyberNow
description: Remove KyberNow Station from Windows, a Mac or Linux, then the KyberNow Link app from each device, keeping your settings or not.
rubrique: Entretien
rubrique_en: Maintenance
univers: commun
duree: 5 min
connexes: [mettre-a-jour, compte-kybernow, installer-station-windows]
---

Each KyberNow app is removed like any other app on its system. The Station asks whether you want to keep your settings, which come in handy if you reinstall.

## Before you start

- **Is the computer attached to a KyberNow account?** Detach it first: in Admin, under KyberNow account, “Detach”, or on the account page, “Detach from your account”. Otherwise it stays in your list of computers, offline.
- **Will you reinstall?** Keep your settings. The Station then keeps its identity with the relay: your devices, your share links and access away from home work again without redoing anything.
- **Leaving KyberNow for good?** Delete your account as well, on the account page, “Account and security”, “Delete the account”. Uninstalling the apps doesn't delete it.

## KyberNow Station on Windows

1. Open Windows Settings › Apps › Installed apps.
2. Choose KyberNow Station, then “Uninstall”.
3. Say whether you want to keep your settings.

The uninstaller removes the services, Admin, the virtual display and its certificate, the firewall rules and the Start menu shortcuts. It restores your previous power mode, and sets your network back to “Public” if it had switched it to “Private”. The ViGEmBus controller driver and the VB-CABLE virtual audio cable stay installed, because other software may use them: remove them yourself if you no longer need them.

## KyberNow Station on a Mac

1. Open the KyberNow-Station-Mac.dmg file again, or the menu of the Station's icon, in the menu bar.
2. Run “Uninstall KyberNow Station”.
3. Say whether you want to keep your settings. macOS asks for the password of an administrator of the Mac.

The Station gives you back your original displays and sound output, then removes the app, its sound output, its services and its firewall rules. The permissions given to the Station (screen, accessibility, microphone) are forgotten.

## KyberNow Station on Linux

On Ubuntu, Debian or Fedora, remove the package:

```sh
sudo apt remove kybernow-station     # Ubuntu, Debian
sudo dnf remove kybernow-station     # Fedora
```

Uninstalling stops the Station in open sessions and closes the ports it had opened in the firewall.

On Bazzite, Fedora Silverblue or Kinoite, and SteamOS, the Station lives in your home folder:

```sh
~/.local/lib/kybernow-station/current/bin/kybernow-station-home uninstall
```

Your settings, your accounts and your pairings stay for a future installation; add `--purge` to remove them too.

## The KyberNow Link apps

| Device | How to remove it |
|---|---|
| Mac | Drag KyberNow Link from the Applications folder to the Bin. |
| Windows PC | Windows Settings › Apps › Installed apps › KyberNow Link › “Uninstall”. The app offers to erase your list of computers, their settings and their passwords as well; if you don't, they will be there if you reinstall it. |
| Apple TV | On the Home Screen, press and hold the touch surface on the KyberNow icon, then choose to delete the app. |
| iPhone, iPad | Touch and hold the KyberNow icon, tap “Remove App”, then “Delete App”. |
| Android | Settings › Apps, KyberNow, then “Uninstall”. |
| LG TV | In webOS Dev Manager, Apps tab, or on the command line: `ares-install --device tv --remove fr.leslignesclaires.kybernow`. Leaving developer mode removes it too. |
| Linux, Steam Deck, SteamOS | In GNOME Software or Discover, or in a terminal (see below). On the Deck, also remove KyberNow Link from your Steam library. |
| Raspberry Pi, the app alone | `sudo apt remove kybernow-link`. Your computers, your settings and the log stay in `~/.config/kybernow/`. |
| KyberNow Link OS | It is the card's whole system. To give the Pi another use, copy another image onto the card with Raspberry Pi Imager. |
| Browser | Nothing to uninstall. |

On Linux, in a terminal, the first line removes the app; the second also erases your computers, your settings and the log:

```sh
flatpak uninstall --user fr.leslignesclaires.kybernow
rm -rf ~/.var/app/fr.leslignesclaires.kybernow
```

> Good to know: giving away or selling a device? Remove its access to the computer too. In Admin, under Link devices, open the device, then “Remove access” or “Forget this device”. If it was connected to your account, choose “Disconnect” next to it, under “Account and security”.
