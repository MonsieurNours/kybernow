---
slug: partager
slug_en: share
titre: Share your screen with KyberNow Share
title: Share your screen with a friend · KyberNow Share
description: Let up to four guests onto your computer with a code. They watch, or play with you if you hand them the controller.
rubrique: Partager
rubrique_en: Share
univers: commun
duree: 4 min
connexes: [preter, hors-de-chez-vous, compte-kybernow, stylet-micro-webcam]
---

KyberNow Share lets up to four guests onto your computer: they see the screen, hear the sound, and you decide what they can do. To hand the computer to someone while you aren't using it, see [Lend your computer](/guide/preter/) instead.

## Open a share

You open it from your session or from the computer itself.

**From your session**, on any device:

1. In the session bar, choose “Share”.
2. The “Share Bureau-Salon” panel (the computer's name) opens. The share shows your session's main display, with the sound.

**From the computer itself**, without a session:

1. In the KyberNow Station icon's menu (next to the clock, or in the Mac's menu bar), or on Admin's home page, choose “Share a screen…”.
2. Choose the screen to show. The main display is at the top of the list.
3. You carry on using the computer: no virtual display is created and your screens stay on.

The panel shows:

- the six-digit code, for example “123 456”;
- a QR code, and “Copy link”;
- the list of guests, and requests from those arriving from afar.

The code holds as long as the share is open. “New code” changes it, and guests already in stay. “Stop sharing” makes everyone leave; closing your session also stops the share.

<!-- capture : le panneau « Partager » avec le code, le QR code et deux invités -->

## Join a share

In the KyberNow Link app:

1. On the home screen, next to “Add a computer or a console”, choose “Join with a code”.
2. Type the six digits. On iPhone, iPad and Android, you can also switch to “QR code” and scan the host's. The host's link opens this screen directly.
3. Give your first name, choose the “Picture quality”, then “Join”.

In a browser, open the host's link: it opens as a guest, without Admin's password, with the code already filled in. Choose “Join”.

A guest isn't added to their device's list of computers. Nothing is kept after the share, except their first name.

## What a guest can do

Every guest arrives as a spectator. In the panel's list, you give or take back two rights, each without affecting the other:

| Action | Effect |
|---|---|
| None (spectator) | They see the screen and hear the sound, without sending anything to the computer. |
| “Give controllers” | Their controllers become the computer's next controller, one of their own. “Take back controllers” removes them. |
| “Give keyboard and mouse” | They type and move the pointer, and they get the stylus. “Take back keyboard and mouse” removes them. |
| “Remove” | They leave the share. They can come back with the same code, as long as you haven't asked for a “New code”. |

The Station enforces these rights: even if a guest's device tried to send something else, nothing reaches the computer. The clipboard is never shared with a guest, and their camera doesn't go to the computer.

Guests only talk if you turn on “Let guests speak”: their microphone then reaches the computer, for a game or a call. The switch is off for every new share.

Four guests at most: beyond that, the code is refused (“This share is full (four guests at most).”).

> Good to know: someone close to you with a KyberNow account and the “Watch” right comes in as a spectator without a code. See [Your KyberNow account](/guide/compte-kybernow/).

## Each guest's picture

Each guest picks their “Picture quality” on the “Join” screen, then in their bar:

- “Auto” (suggested): based on their device's screen and what its decoder can play;
- “Same as host”;
- “4K · 60 Hz”, “1080p · 60 Hz” or “720p · 30 Hz”.

You always come first. A guest's picture is never above yours, and nothing they do changes your screen or your picture. A guest who follows your picture shares it; the others get a picture prepared for them by the computer, two at most besides yours (only one with an Intel graphics card). When there is no room left, the guest gets the closest open picture, a lighter one, and a message tells them so. A guest who falls behind drops to a lighter picture on their own (“Picture lightened to keep up.”), without you noticing anything. Your panel shows each guest's quality.

A guest without an HDR screen, or in a browser, receives a picture converted to SDR. When you record the session, every guest sees the “Recording” pill.

## When the guest is away from your home

The same code, link and QR code work from anywhere: the guest goes through KyberNow's relay. You accept each guest who comes from the internet:

1. Their request appears in the panel's “Requests” section, with their first name and their app, and a brief notice, for example “Léa wants to join.”
2. Choose “Accept” or “Decline”. Meanwhile they wait (“Waiting for the host to accept…”), for two minutes at most.

On the same network, there is nothing to accept. The relay is set in Admin, Link devices › “Away from home” › “Relay” (KyberNow's public relay by default). It is all explained in [Reach your computer away from home](/guide/hors-de-chez-vous/).

## If something goes wrong

- **“No share with this code on your network.”** Check the six digits, or ask the host for a new code. From another network, the host's computer must be connected to a relay.
- **The panel says “Not reachable away from home yet.”** The relay isn't answering: check “Relay” in Admin › Link devices. Guests on the same network still get in.
- **“The host did not answer.”** The host didn't accept the request within two minutes. Let them know, then try again.
- **“Share” is dimmed.** Its tooltip gives the reason: “Sharing is turned off on this computer (Admin › Link devices).”, or “Sharing needs KyberNow Station 0.8.0 or later.”
- **“Too many code attempts: try again in a minute.”** Wait a minute before typing the code again.
- **“This device cannot play this computer's picture (HEVC).”** The guest's device can't decode any of the possible formats. Try another device, or the browser.
