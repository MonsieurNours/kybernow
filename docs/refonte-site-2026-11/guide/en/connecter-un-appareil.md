---
slug: connecter-un-appareil
slug_en: connect-a-device
titre: Connect a device to your computer
title: Connect a device to your computer · KyberNow
description: Install KyberNow Link, add your computer over the network, with a code, an address or a QR code, then open your first session.
rubrique: Connecter un appareil
rubrique_en: Connect a device
univers: commun
duree: 3 min
connexes: [ce-qu-il-vous-faut, compte-kybernow, hors-de-chez-vous, profils-et-qualite]
---

On each device, you install KyberNow Link, then add your computer to it once. From then on, it waits for you on the app's home screen, and “Connect” opens the session.

## How it works

1. The Station is installed and running on the computer: [Windows](/guide/installer-station-windows/), [Mac](/guide/installer-station-mac/) or [Linux](/guide/installer-station-linux/).
2. You install KyberNow Link on the device. Each device has its own article, further down.
3. On the same network, the computer shows up in the app by itself: every Station announces itself on the local network.
4. You give the device access. Depending on the app, you accept in Admin the six-digit code it displays, or you enter the account created when the Station was installed.
5. The computer appears on the app's home screen. “Connect” opens the session.

## The first time you open the app

KyberNow Link starts with three welcome screens. The second offers a KyberNow account: “Connect an account”, “Create an account” or “Continue without an account”. The account is optional. With it, the computers on your account arrive by themselves, with nothing to add. See [Your KyberNow account](/guide/compte-kybernow/).

The third screen, “Add your computer”, shows the computers found on the network, with “Add”. You can also skip these screens with “Skip” and add the computer later, with “Add a computer or a console” on the home screen.

## Accept the code in Admin

This is the simplest way, in the apps that offer it: nothing to type on the device.

1. The app displays a six-digit code and sends it to the Stations on the network.
2. On the computer, open KyberNow Admin. A banner says the device is asking for access to this computer, with the code. Click “View request”. On a PC, the panel of the icon by the clock shows the request too; on a Mac, a notification announces it.
3. In the Link devices space, check the code is the one on the device, then click “Accept”.

The device gets its own access, with no password to type. You can take it away in Link devices, on the device's card, with “Remove access”.

The code is only valid for a few minutes, then the app draws a new one. It only travels over the local network: over a VPN, add the computer by its address.

> Good to know: if a session is running on the computer, the Station waits for it to end before restarting to take the new access into account. The device can connect straight after.

## Ways to add a computer

| Way | When to choose it | What you need |
|---|---|---|
| On the network | The device is at home, on the same network | Nothing: the computer shows up |
| On-screen code | On a TV or a device where typing is a chore | Accept the code in Admin |
| By address | Over a VPN, or on a network where the computer does not announce itself | The computer's IP address or name, and Admin's port |
| QR code or link | A phone, tablet or computer with a camera, or a link you received | Admin › Link devices › “Add a device” |
| From your Mac | Apple TV, iPhone or iPad, when the Mac app already knows the computer | The same iCloud account |
| KyberNow account | On all your devices, at home and away | “Connect an account” in the app |
| Invitation | A device far from home, without you having to accept it | A code and a password given by Admin |

In Admin, the Link devices space shows, under “Add a device”, the pairing QR code, “Copy link” and the computer's fingerprint. The link holds the computer's address and its certificate's fingerprint, never a password.

You create an invitation in the same space, in the “Invitations” section. On the device, you use it with “Join with a code”: see [Add a device while you are away](/guide/hors-de-chez-vous/).

“Join with a code” is also how you join a friend's share or borrow their computer: see [Share your screen with KyberNow Share](/guide/partager/) and [Lend your computer](/guide/preter/).

## What each app offers

| App | Ways to add | Access |
|---|---|---|
| [Mac](/guide/link-mac/) | On this network, By address, QR code or link | The Station's account |
| [Windows](/guide/link-windows/) | On the network, With the pairing link, By address | A code or the Station's account |
| [Linux and Steam Deck](/guide/link-linux-steam-deck/) | On the network, By address | A code or the Station's account |
| [Apple TV](/guide/apple-tv/) | From your Mac, On your network, On-screen code, Address | A code or the Station's account |
| [iPhone and iPad](/guide/iphone-ipad/) | From your Mac, On your network, On-screen code, Address, Pairing QR code | A code or the Station's account |
| [Android](/guide/android/) | Network, Code, Address, QR code | A code or the Station's account |
| [LG TV](/guide/televiseur-lg/) | On the network, With address and port | A code |
| [Raspberry Pi](/guide/raspberry-pi/) | On the network, By address | A code or the Station's account |
| [Browser](/guide/navigateur/) | Admin's address followed by /link | The Station's account |

## Check the fingerprint

Every Station proves who it is with a certificate it created during installation. Its fingerprint is a series of groups of characters.

- Found on the network, or added with a link or a QR code, the computer is already checked: the app knows its fingerprint and compares it by itself.
- Added by its address, the app shows you the fingerprint. Compare the first four groups with those Admin shows, in Link devices, under “Add a device”. If they match, it really is your computer. If they differ, stop there.

## Then, on the home screen

The computer shows with its state: “Online”, “Offline”, or “Away from home” when the app reaches it through the relay. When it is offline, “Wake the computer” turns it on remotely, if Wake on LAN is allowed in Admin › System.

Under “Connect”, choose a profile: Game, Desktop, Max or Auto. See [Profiles, video format, bitrate and HDR](/guide/profils-et-qualite/).

## If something goes wrong

- **The computer does not show up.** Check it is on, that the Station is running and that the device is on the same network. On a PC, the network must be set to “Private” in Windows. Over a VPN, use the address.
- **The code expired, or Admin refused it.** Ask for a new code in the app and accept it more quickly.
- **“Password refused: update it in the settings.”** The account's password was changed in Admin. Enter the new one in the computer's settings, in the Station section.
- **“The Station was reinstalled: check its fingerprint in the settings.”** The Station was reinstalled with “Start from scratch”. Compare the new fingerprint with the one in Admin, then add the computer again.
- **“The connection failed: the computer is not responding”.** Check that it is on and that KyberNow Station is running, then try again.
- **The app asks you to update the Station.** The Station is too old for this way of adding. Update it: see [Update KyberNow](/guide/mettre-a-jour/).
