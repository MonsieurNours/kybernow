---
slug: link-windows
slug_en: link-windows
titre: Use your computer from a Windows PC
title: KyberNow Link on Windows · KyberNow
description: Install KyberNow Link on an x64 or ARM64 Windows PC, with no administrator rights, then add your computer with a code or a link.
rubrique: Connecter un appareil
rubrique_en: Connect a device
univers: commun
duree: 2 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, plusieurs-ecrans-et-ordinateurs, compte-kybernow]
---

The Windows app is mostly for a laptop or a more modest computer, to reach your main computer. The graphics card decodes the picture whenever it can, and we measured about 10 ms of latency on an x64 PC.

## Before you start

- Windows 10 22H2 or Windows 11, on an x64 or ARM64 PC. ARM64 PCs, such as those with a Snapdragon chip, need Windows 11.
- The Station installed on the computer you want to reach.

## Install the app

1. Download [KyberNow-Link-Setup.exe](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Setup.exe) and open it. The same file works for x64 and ARM64 PCs: it installs the right version by itself.
2. If Windows SmartScreen stops it from opening, click “More info”, then “Run anyway”. The installer and the app do not carry an Authenticode signature yet.
3. **Welcome**: the installer checks Windows, the processor, the graphics card, the display, the local network and Microsoft's display engine (WebView2), which it downloads if it is missing. Tick “Desktop shortcut” if you want one, then click “Install”. The app installs for your Windows account only, with no administrator rights.
4. **KyberNow account**, optional: “Sign in” or “Create an account” opens the relay's page in the window, with the passkey through Windows Hello. The app will open signed in, with the computers on your account. “Later” skips this screen.
5. **Ready**: click “Open KyberNow Link”.

## Add your computer

On the home screen, choose “Add a computer or a console”, then one of three ways:

- **On the network**: your computer shows up by itself. Click “Choose”.
- **With the pairing link**: in Admin, in the Link devices space, “Add a device” shows a `kybernow://` link. Click this link on this PC, or paste it into the app: it opens on the adding screen, with the computer and its fingerprint already checked.
- **By address**: the computer's IP address or name, and Admin's port. This is the one to choose over a VPN, such as Tailscale. Then compare the first four groups of the fingerprint with those Admin shows, and click “They match”.

## Give this PC access

- **With a code**: the app shows a six-digit code. Accept it in Admin, in the Link devices space (on a PC, also in the panel of the icon by the clock). Nothing to type: this PC gets its own access, which you can remove from Admin.
- **With an account**: the username and password chosen when the Station was installed, or a device account. Windows stores them in its Credential Manager. Click “Add”.

Then choose a profile (Game, Desktop, Max or Auto), and “Connect”: the session opens.

## During the session

- Move the mouse to the top of the screen to show the session bar.
- The app's shortcuts are Ctrl+Alt+Shift followed by a letter: D to disconnect, F to switch between full screen and a window, M to switch between the desktop mouse and the gaming mouse.
- The bar's “Keys” menu sends “Ctrl + Alt + Del”, “⊞ key alone”, “Alt + Tab” and “Print Screen”.

The details are in [Keyboard, mouse and shortcuts](/guide/clavier-et-raccourcis/).

## By the clock and in the taskbar

Closing the home window does not quit the app: it stays in the notification area, by the clock, sometimes under the ^ arrow. Its menu lists your computers with their state. Right-clicking the KyberNow Link icon in the taskbar also shows your computers: one click starts the session.

## If something goes wrong

- **“This Windows is too old”.** Install Windows updates until you have at least Windows 10 22H2.
- **“Windows basic driver”.** The processor will decode the picture, more slowly. Install the graphics card's driver.
- **“Admin declined the request” or “The request expired”.** Click “New code”, then accept it in Admin.
- **The `kybernow://` link does not open the app.** Open KyberNow Link once to finish its installation, or paste the link into the app.
- **The fingerprint is different.** Stop there. Check the address, or add the computer over the network or with the link.
