---
slug: faq
slug_en: faq
titre: Frequently asked questions
title: Frequently asked questions · KyberNow
description: Price, security, accounts, compatible devices, latency, playing away from home and troubleshooting: answers to the questions we get asked most.
rubrique: FAQ
rubrique_en: FAQ
univers: commun
connexes: [ce-qu-il-vous-faut, connecter-un-appareil, hors-de-chez-vous, compte-kybernow]
---

# Getting to know KyberNow

## What is KyberNow?

KyberNow shows your computer on your other screens and lets you control it, for playing as well as working. You install KyberNow Station on the computer, then the KyberNow Link app on your devices. KyberNow Admin, in the browser, sets up the Station, and KyberNow Share lets someone else in. See [What you need to get started](/guide/ce-qu-il-vous-faut/).

## Is KyberNow really free?

Yes. KyberNow is free, open-source software under the AGPL-3.0 licence, developed by Les Lignes Claires with no commercial aim. There is no subscription and no advertising, and the public relay is open to everyone. The KyberNow account, families and teams are free too. The code is on [GitHub](https://github.com/MonsieurNours/kybernow).

## Why is KyberNow in beta?

KyberNow hasn't reached version 1.0 yet. We use it every day, but settings or screens may still change from one version to the next. Keep the Station and your apps up to date (see [Update KyberNow](/guide/mettre-a-jour/)), and report problems on GitHub or Discord.

## Can KyberNow replace Sunshine and Moonlight?

For many uses, yes. Like them, KyberNow shows your computer on your other devices, in an app or in a browser. It is built on the Kyber engine rather than the GameStream protocol: the picture is always encrypted, the virtual display at the device's size is included, and most devices are added by accepting a code in Admin, with nothing to copy out. The Technology page compares the two in detail.

## Is it cloud gaming?

No. Cloud gaming rents you a computer in a data centre, on a subscription. KyberNow uses yours: you find your games and software just as you installed them, with no queue. At home, the picture goes straight from your computer to your device, without passing through a server.

## How is KyberNow related to Kyber?

KyberNow uses Kyber, the open-source engine that carries the picture, the sound and the controls, built by VLC developers. It adds the Station (installer, virtual displays and sound output), Admin, the KyberNow Link apps and KyberNow Share. KyberNow is neither published nor endorsed by the Kyber team, and is not affiliated with it. Our fixes to the engine are published.

# Account and security

## Do I need a KyberNow account?

No. At home, the app finds the computer by itself and you add it with a six-digit code; sharing and lending work without an account too. The optional account makes your computers appear on all your devices, makes them reachable away from home with nothing to set up, and lets you open them to your family or your team. See [Your KyberNow account](/guide/compte-kybernow/).

## Is KyberNow safe?

The picture, the sound and your keystrokes are encrypted between the computer and the app, and the app checks it is really talking to your computer thanks to its certificate's fingerprint. A new device only gets in if you accept its code in Admin, or through an account you have given rights to. There is nothing to open on your router, and every access stays visible in Admin.

## Can KyberNow's relay see my screen?

Not in the KyberNow Link apps: the picture, the sound and your keystrokes cross it encrypted end to end. In a browser, with an up-to-date Station and relay, the connection is also encrypted all the way to your computer. The relay sees IP addresses, bitrates and share codes. See [Reach your computer away from home](/guide/hors-de-chez-vous/).

## What data does KyberNow collect?

No usage statistics. At home, nothing goes to a server of ours: the picture goes straight from the computer to your devices. Away from home or for a share, the connection goes through the relay, which keeps IP addresses in its logs for a few days, never the codes. With an account, the relay also keeps your account, the rights given, the access log and your synced settings; “Export my data” gives you a copy.

## Can my company use its own directory?

Yes. A team can sign in with the company's directory, through SSO or OAuth: everyone gets in with their work account. An Owner or an Administrator sets it up in the team's console. See [Families and teams](/guide/familles-et-equipes/).

## I've lost a device. What should I do?

In Admin, under Link devices, open the device, then “Remove access”. If it was connected to your account, also choose “Disconnect” next to it, under “Account and security”. Changing the account's password signs out all your devices at once.

## I've forgotten the Admin password. What should I do?

On the computer, open the menu of the KyberNow Station icon, next to the clock, and choose “Reset the Admin password…”. The icon shows a code: Admin opens and asks you for it, along with your new password. The devices you have already added stay added.

## I've forgotten my KyberNow account password. What should I do?

On the sign-in page, choose “Forgot your password?”: a link valid for one hour is sent to your address. The new password signs out your other devices; connect them again with “Connect an account”.

# At home and away

## Can I use KyberNow away from home?

Yes. With a KyberNow account, your computers follow you everywhere. Without an account, add the device at home, then switch on “Access away from home” in Admin. A VPN such as Tailscale or WireGuard works too. The computer must stay on. See [Reach your computer away from home](/guide/hors-de-chez-vous/).

## Do I need to open ports on my router?

No, and it's better not to. The Station is what opens the connection to the relay, and a VPN doesn't need open ports either. That way your computer stays invisible to the rest of the internet.

## How much data does a session use?

It depends on the bitrate you choose. At 20 Mbit/s, an hour of session uses about 9 GB. Away from home, choose the Game or Desktop profile, or leave “Adaptive bitrate” on: the app lowers the bitrate when the network is saturated. See [Profiles, video format, bitrate and HDR](/guide/profils-et-qualite/).

## How much latency should I expect?

On a local network, we measured 9.4 ms between capturing the picture on the PC and displaying it on the device. That is a median: half of the frames arrive faster. At home, latency depends mostly on the graphics card, the network and the device, and the app shows it live. Through the relay, allow 5 to 20 ms more in France.

## Can I install my own relay?

Yes, on a Linux machine with Docker and a domain name. Your shares, your access away from home and, if you wish, your accounts then go through a machine you run yourself. See [Install your own relay](/guide/installer-votre-relais/).

# Computers and devices

## Which computers can run the Station?

A Windows 10 or 11 PC, a Mac with Apple silicon on macOS 15 or later, or a Linux computer: Ubuntu, Debian, Fedora, and also Bazzite, Fedora Silverblue or SteamOS. See [Install on Windows](/guide/installer-station-windows/), [on a Mac](/guide/installer-station-mac/) and [on Linux](/guide/installer-station-linux/).

## Which devices can I use my computer from?

A Mac with Apple silicon, a Windows PC, a Linux PC, the Steam Deck, an Apple TV 4K from 2021 or later, an iPhone or an iPad, an Android device (phone, tablet, TV), an LG TV, a Raspberry Pi, and any recent browser. See [What you need to get started](/guide/ce-qu-il-vous-faut/).

## Do I need a particular graphics card?

No. On a Windows PC, the Station compresses the picture with any NVIDIA, AMD or Intel graphics card, external ones (eGPU) included; without a graphics card, the processor does the work. Only the AV1 format needs an NVIDIA RTX 40 or 50 series card. On a Mac, Apple silicon is enough.

## Which resolutions and refresh rates are possible?

Your screen's: for each session, the computer creates a virtual display at your device's resolution and refresh rate, up to 7680 × 4320 and 500 Hz. Some devices have their own ceiling: 4K at 60 Hz on the Apple TV, the NVIDIA Shield and the Raspberry Pi, 4K at 120 Hz on LG TVs. See [Profiles, video format, bitrate and HDR](/guide/profils-et-qualite/).

## Does KyberNow support HDR?

Yes, when your device's screen is HDR: an “HDR” chip appears on the computer's card, on by default, and games arrive with their colours and brightness intact. On a PC, you need Windows 11 24H2 and a graphics card that encodes in 10 bits. See [Profiles, video format, bitrate and HDR](/guide/profils-et-qualite/).

## What is PyroWave?

A picture format that is compressed and decompressed in a fraction of a millisecond, to shave off even more delay on your home network. It needs a lot of bandwidth, about 300 Mbit/s at 1080p: on “Auto”, the app only picks it when your device is connected by cable and the computer offers it. See [Profiles, video format, bitrate and HDR](/guide/profils-et-qualite/).

## Does the Mac app work on an Intel Mac?

No, it needs a Mac with Apple silicon. On an Intel Mac, open your computer in a browser, ideally Chrome or Edge. See [Use your computer in a browser](/guide/navigateur/).

## Is KyberNow on the App Store?

Yes, for iPhone, iPad and Apple TV: search for “KyberNow”. You need iOS 18 or iPadOS 18 or later, and an Apple TV 4K from 2021 or later. See [Use your computer from an iPhone or iPad](/guide/iphone-ipad/) and [Play on the TV with Apple TV](/guide/apple-tv/).

## Does KyberNow work on Android?

Yes. The same app works on phones, tablets and TVs, from Android 11. It installs from its APK file, published with every version, then updates itself. See [Use your computer from Android](/guide/android/).

## Does KyberNow work on an LG TV?

Yes, on LG TVs running webOS 5 or later (2020 models onwards), up to 4K at 120 Hz, with controllers, the Magic Remote and 5.1 sound to a receiver. The app installs in developer mode from a computer, with a free LG account; remember to extend that mode now and then. See [Play on an LG TV](/guide/televiseur-lg/).

## Does KyberNow work on Linux and the Steam Deck?

Yes. The same app runs on a Linux PC (x86_64 or ARM), on the Steam Deck and on SteamOS in the living room, with an interface suited to each: mouse and keyboard on a PC, the controller on the Deck and on the TV. It installs as a Flatpak package, without administrator rights. See [Use your computer from Linux or a Steam Deck](/guide/link-linux-steam-deck/).

## Does KyberNow work on a Raspberry Pi?

Yes, with a Pi 3, 4 or 5. KyberNow Link OS is copied onto a microSD card: the Pi starts straight into KyberNow Link, like a console plugged into the TV. You can also install the app alone on 64-bit Raspberry Pi OS. See [Plug a Raspberry Pi into the TV](/guide/raspberry-pi/).

## Can I play my PlayStation with KyberNow?

Yes. A PS5 or PS4 at home appears on the KyberNow Link home screen, next to your computers. You pair it once with the code it shows, then “Play”. See [Play your PS5 or PS4](/guide/playstation/).

# Sharing and lending

## Can I share my computer with a friend?

Yes, with KyberNow Share. You open a share, and up to four guests come in with a six-digit code, a link or a QR code, in the app or in a browser, at home or from afar. A guest arrives as a spectator; you give them, or not, the controllers, the keyboard and the mouse. See [Share your screen with KyberNow Share](/guide/partager/).

## Can I lend my computer?

Yes. With a code for one session, a friend's account created in Admin, or the “Borrow to play” right on a KyberNow account, your friend plays from home while you aren't using the computer, with the apps you allow. See [Lend your computer](/guide/preter/).

## Can someone just watch my screen?

Yes, with the “Watch” right on a KyberNow account. The person sees the computer's actual screen, with no keyboard, mouse or controller, and Admin tells you they are watching. See [Your KyberNow account](/guide/compte-kybernow/).

## Can two devices connect at the same time?

Not to the same session: the second one takes over, the first is disconnected, and the virtual display is recreated at the new device's size. To let someone else in without cutting your session, open a share. See [Share your screen with KyberNow Share](/guide/partager/).

## Can I open several computers at once?

Yes, from the Mac, Windows and Linux apps: one window per computer, four at most. Only the window in front receives the keyboard, the mouse and the controllers. See [Several displays and several computers](/guide/plusieurs-ecrans-et-ordinateurs/).

# Troubleshooting

## My screens turn off during a session. Is that normal?

Yes. During the session, the computer shows only the virtual display, at your device's size, and turns off its own screens. They come back on at the end. If the app drops out without warning, the Station waits 20 seconds, then removes the virtual display and turns your screens back on; if needed, Admin has a “Turn my displays back on” button, under Picture and sound.

## Why does the sound still come out of the PC's speakers?

On a Windows PC, the Station sends the sound to your devices through Steam's virtual speakers. Install Steam, then run the KyberNow Station installer again and choose “Keep my settings”: the “Sound on your devices” option becomes available, and your devices stay added. On a Mac and on Linux, the Station installs its own sound output.

## My controller doesn't respond. What can I do?

First check that it is connected to your device, over Bluetooth or by cable; several apps have a screen for testing controllers. On a Windows PC, controllers go through the ViGEmBus driver, installed with the Station; after the first installation, the PC sometimes needs a restart. The computer accepts four controllers at most, and while the session menu is open, the controller drives the app and the game receives nothing.

## The picture stays black or freezes. What can I do?

First update the Station and the app. Then try a lower resolution, or another format (H.264 or HEVC) in the computer's settings, in the app. Connect the computer by Ethernet if you can, and check that no other remote play software is running on it. Admin's logs (System › Logs) help find the cause.

## Why doesn't my computer appear in the app?

Check that the device is on the same network as the computer, and that the Station is running (its icon is next to the clock, or in the menu bar on a Mac). You can also add it “By address”, with the computer's address and the Admin port. See [Connect a device to your computer](/guide/connecter-un-appareil/).

## Where can I ask another question?

In the [help forum on our Discord](https://discord.gg/sVy2mu5WdT), in English or in French. To report a problem, [GitHub issues](https://github.com/MonsieurNours/kybernow/issues) are open to everyone.
