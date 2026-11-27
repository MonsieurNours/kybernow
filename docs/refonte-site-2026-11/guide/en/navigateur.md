---
slug: navigateur
slug_en: browser
titre: Use your computer in a browser
title: KyberNow in a browser · KyberNow
description: Open your computer in Chrome, Edge, Safari or Firefox, with nothing to install, from a tablet or a borrowed computer.
rubrique: Connecter un appareil
rubrique_en: Connect a device
univers: commun
duree: 2 min
connexes: [connecter-un-appareil, hors-de-chez-vous, partager, profils-et-qualite]
---

With nothing to install, you can reach your computer in a browser, on any device. It comes in handy for working from a tablet or a borrowed computer.

## Before you start

- A recent browser: Chrome or Edge ideally, Safari and Firefox too.
- A device on the same network as the computer, or connected to it through a VPN.
- The account created when the Station was installed, or another account from Admin › Users.

## Open the session

1. In the browser, open Admin's address followed by `/link`, for example `https://192.168.1.20:31415/link`. The exact address appears at the end of the Station's installation, and in Admin, under Overview › “Open in the browser”.
2. The browser shows a security warning. This is expected: the Station encrypts the connection with a certificate it created itself. Accept it to carry on.
3. Sign in with the Station's username and password.
4. The session opens in the page. Move the pointer to the top of the picture to show the session bar.

<!-- capture: /link page in Chrome, session bar open -->

## Which browser to choose

- **Chrome and Edge** receive the picture directly, with the lowest latency.
- **Safari, on Mac as on iPhone, and Firefox** receive the picture through Admin, with a little more latency.

If the browser cannot start the picture in HEVC, the page switches back to H.264 by itself and tells you with “Picture in H.264: this browser couldn’t start HEVC.”

## The browser and the apps

The browser is a good stand-by, for desktop work above all. For gaming, prefer a KyberNow Link app: the browser only has stereo sound, and it does not ask for the picture again when part of it is lost on the way.

> Good to know: away from home, the browser goes through the KyberNow relay to join a share or open Admin remotely. To reach your whole computer, use a KyberNow Link app, or a VPN. See [Reach your computer away from home](/guide/hors-de-chez-vous/).

A friend you invite to [share your screen](/guide/partager/) can also join the session from their browser, with the share's link.

## If something goes wrong

- **The page does not open.** Check the address and port, and that the device is on the same network as the computer. The address starts with `https://`.
- **“Picture without sound: this browser couldn’t start the audio.”** The browser refused to start the sound. Reload the page, or try Chrome or Edge.
- **The page says Kyber could not start the picture.** Try again in a moment. If the message comes back, copy the “Details” line to report it.
- **The picture stutters on Wi-Fi.** Move closer to the router, or switch to a KyberNow Link app, which repairs the picture when part of it is lost.
