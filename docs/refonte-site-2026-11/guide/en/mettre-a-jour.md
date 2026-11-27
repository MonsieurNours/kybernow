---
slug: mettre-a-jour
slug_en: update
titre: Update KyberNow
title: Update the Station and the apps · KyberNow
description: Update the Station in one click from Admin or from your apps, choose whether to get betas and keep each KyberNow Link app up to date.
rubrique: Entretien
rubrique_en: Maintenance
univers: commun
duree: 5 min
connexes: [desinstaller, installer-station-windows, installer-votre-relais, faq]
---

The Station and every KyberNow Link app share the same version number. Keep them on the same version: most update themselves, and the others let you know when a version comes out.

## The Station

### From Admin

Admin checks for a new version every 6 hours.

1. Open Admin, under System, on the “Updates” page. “Check now” looks straight away.
2. Choose “Install now”. Admin downloads the version, checks that it is properly signed by KyberNow, then installs it in about a minute. A session in progress is cut off: Admin asks you first.
3. The page reloads by itself and announces that the new version “is installed”.

If the new version doesn't start properly, the Station goes back to the previous one by itself and the page says so. “Go back to the previous version” does the same on request.

The Station's icon offers it too: next to the clock on Windows (“Update the Station” followed by the version number), in the menu bar on a Mac. It works the same way on a Linux Station, from Admin, including on Bazzite, Fedora Silverblue or SteamOS, with no password.

Some versions are installed with the installer: Admin shows “This version is installed with the installer.” Download it, run it on the computer and keep your settings.

<!-- capture : Admin, System › Updates, with “Install now” -->

### From your apps

When a Station version is available for the chosen computer, the KyberNow Link home screen shows a banner: the new version “is available for this computer. About a minute.”

- “Update” installs the version and follows the Station until it comes back.
- “Later” hides the banner for this version. It comes back after 7 days, or sooner if a newer version comes out.

After the app itself has updated, a notice gathers the Stations that need updating, with “Update all”. The same thing is in the computer's settings, in the Station section, “Update the Station”.

This banner appears on devices added to the computer, and for an account with the “Administer” right. Away from home, the app cannot update the Station: do it once you are back, or open [Remote Admin](/guide/familles-et-equipes/).

### Get betas

In Admin, System › “Updates”, choose the “Beta” channel under “Update channel”. Betas arrive before stable versions; they may still change.

### For troubleshooting, on Windows

If Admin no longer responds, the Station can also be updated from PowerShell, run as administrator:

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

This route checks that the package isn't damaged, without Admin's signature checks. Keep it for troubleshooting.

## The KyberNow Link apps

| Device | How it updates | To get betas |
|---|---|---|
| Mac | By itself: it checks every day, downloads and installs. KyberNow Link › “Check for updates…” checks straight away. | “Get beta versions”, in KyberNow Link › Settings… |
| Windows PC | It checks at launch, then every 6 hours, and offers it to you: “Install and relaunch”, never during a session. | App settings, “Update channel”: “Betas too” |
| Apple TV, iPhone, iPad | Through the App Store, like your other apps. | |
| Android | “Version X” appears on the home screen; a button downloads and installs it. The first time, Android asks you to allow KyberNow to install apps. | App settings › Updates |
| LG TV | “Version X available” appears on the home screen. Install the new KyberNow-Link-webOS.ipk file from the computer, as the first time. | Settings › This TV › Updates |
| Linux, Steam Deck, SteamOS | “Version X available” appears on the home screen. Install the new package over the old one: double-click, or `flatpak install --user` followed by the file. | App settings › “Versions offered” |
| Raspberry Pi, the app alone | “Version X available” appears on the home screen. Install the new package: `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`. | App settings › “Versions offered” |
| Browser | Nothing to do: the page comes from the Station and follows its version. | |

Your computers and your settings stay with every update. For the LG TV, see also [Play on an LG TV](/guide/televiseur-lg/).

### KyberNow Link OS on a Raspberry Pi

On the home screen, ☰ opens the Device settings, then “Update”. The new version is tried out in the other half of the card, and the Pi goes back to the previous version by itself if the new one doesn't reach the home screen. You need a Pi 4 or a Pi 5; on a Pi 3, copy the new image onto the card. A card copied with a version earlier than 0.11.8 needs copying again, once. The details are in [Plug a Raspberry Pi into the TV](/guide/raspberry-pi/).

## Your relay

A relay you run yourself updates itself every night: see [Install your own relay](/guide/installer-votre-relais/).

## If something goes wrong

- **“The update did not complete. Open Admin to find out more.”** Admin's “Updates” page says why. The Station has gone back to the version that worked.
- **“The Station has just restarted. Try again in a minute.”** Wait a minute, then try again.
- **A device is in a session.** The app asks “Update anyway”: the session will be cut off.
- **Admin offers nothing although a version is out.** Choose “Check now”. A beta is only offered on the “Beta” channel.
- **Android refuses to install the update.** An app built separately, with a different signature, must be uninstalled once before you install the published version.
