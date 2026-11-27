---
slug: hors-de-chez-vous
slug_en: away-from-home
titre: Reach your computer away from home
title: Reach your computer away from home · KyberNow
description: At a hotel, at work or at a friend's, reach your computer through KyberNow's relay or a VPN, with nothing to open on your router.
rubrique: Hors de chez vous
rubrique_en: Away from home
univers: commun
duree: 5 min
connexes: [compte-kybernow, installer-votre-relais, partager, profils-et-qualite]
---

At home, the picture travels straight from the computer to your device, without going through a server. Elsewhere, your devices find it through KyberNow's relay or through a VPN, with nothing to set up on your router.

## Before you start

- The computer stays on, connected to the internet, with KyberNow Station.
- To play, a connection that sends fast enough from home (your router's upload speed) and a decent network at the other end.

## KyberNow's relay

A relay is a server on the internet that passes the connection between your device and your computer. KyberNow's, share.kybernow.app, is open to everyone, with no bandwidth cap; it is run by Les Lignes Claires, in France. You can also [install your own](/guide/installer-votre-relais/).

The Station is what opens the connection to the relay, and only when it needs to: a share is open, access away from home is on, an invitation is pending. There is nothing to open on your router.

## With a KyberNow account

This is the simplest way. [Attach the computer to your account](/guide/compte-kybernow/): attaching it switches on “Access away from home”. On every device connected to the same account, the computer appears on the home screen and opens from anywhere, with no pairing.

## With a device you have already added

1. First add the device at home, on the home network (see [Connect a device to your computer](/guide/connecter-un-appareil/)).
2. In Admin, under Link devices, in the “Away from home” section, switch on “Access away from home”. It is off to begin with. “Relay reached” appears once the Station has found the relay.
3. Away from home, open KyberNow Link as usual. The computer is marked “Away from home” instead of “Online”, and the session bar shows “Via the relay”.

<!-- capture : Admin, “Away from home” section, with “Relay reached” and the switch on -->

## Add a device while you are away

An invitation adds a device from afar, without anyone having to accept it in front of the computer. Prepare it before you leave, or from [Remote Admin](/guide/familles-et-equipes/).

1. In Admin, under Link devices, in the “Invitations” section, choose “Invite a device…”.
2. Give it a name and a duration: 1, 7 or 30 days. “Create the invitation”.
3. Admin shows a six-digit code and a password. Write the password down: it is shown only once.
4. On the device, in KyberNow Link, “Join with a code”, then the password. The computer is added to the home screen.

Creating an invitation switches on “Access away from home”. At most two invitations can be in use at the same time, five devices each. When the duration runs out, the devices it added lose their access.

## What changes through the relay

- **A little more latency.** The relay adds a round trip, 5 to 20 ms in France depending on the distance.
- **The right profile.** The Max profile is meant for the home network: away from home, choose Game, Desktop or Auto. With “Adaptive bitrate”, on to begin with, the app lowers the bitrate when the network is saturated, then raises it again. A bitrate set by hand must stay below your router's upload speed.
- **Mobile data.** At 20 Mbit/s, an hour of session uses about 9 GB.
- **Networks that filter.** If the network you are on blocks UDP (some hotels, company networks), the connection switches to TCP and the app shows “Via the relay (TCP)”: fine for watching, less good for playing.
- **No restart or update from the app.** Away from home, the app can neither restart nor update the Station. Do it once you are back, or through Remote Admin.
- **Protection against guessing.** After too many failed attempts from one address, the Station blocks it for 10 minutes.
- **In a browser**, the relay is used to join a share and to open Remote Admin. To get your whole computer back, use a KyberNow Link app.

## What the relay sees

In the KyberNow Link apps, the picture, the sound and your keystrokes stay encrypted end to end: they cross the relay without it being able to read them, and the app checks it is really talking to your computer thanks to its certificate's fingerprint, a string of characters that identifies it.

In a browser, with an up-to-date Station and relay, the connection is also encrypted all the way to your computer: the computer has its own certificate, and the relay passes the connection along without being able to open it. The Station watches the public certificate logs and suspends Remote Admin if a certificate it did not request appears for its address. With an older Station, the relay decrypts what goes through in the browser to forward it: in that case, use an app, or update the Station.

The relay sees IP addresses, bitrates, times and share codes. KyberNow's keeps IP addresses in its logs for a few days, never the codes; its privacy page, share.kybernow.app/privacy, gives the details. With an account, it also keeps your account, the rights given, the access log (IP addresses for 7 days) and your synced settings.

## Through a VPN

A VPN is a private, encrypted network between your devices, such as Tailscale or WireGuard. It works without a relay: the picture travels straight from your computer to your device.

1. Install the same VPN on the computer and on the device, signed in to the same account or joined to the same network.
2. Over a VPN, the computer doesn't show up on its own in the app. Choose “Add a computer or a console”, then “By address”: its address in the VPN (for example 100.x.y.z with Tailscale) and the Admin port. You can also scan Admin's QR code, under Link devices, “Add a device”.
3. Compare the first four groups of the fingerprint with the ones Admin shows, then give access as you would at home.

On the Apple TV, iPhone and iPad, the six-digit code doesn't travel over the VPN: enter the username and password of the Station's account instead.

> Good to know: don't open KyberNow's ports to the internet on your router (“port forwarding”). Neither the relay nor a VPN needs it, and your computer stays invisible to the rest of the internet.

## If something goes wrong

- **“Cannot be reached away from home (access turned off in Admin)”.** Switch on “Access away from home” in Admin, in the “Away from home” section.
- **“Cannot be reached: the computer is off or asleep”.** Switch the computer on, or stop it going to sleep while you are away.
- **Admin shows “Relay unreachable”.** Check the address in the “Relay” field and the computer's internet connection.
- **The app shows “Via the relay (TCP)”.** The network you are on blocks UDP. Try another network, your phone's hotspot for example.
- **The picture stutters.** Switch to the Game or Desktop profile, or lower the bitrate in the computer's settings, in the app. At home, an Ethernet cable between the computer and the router helps too.
