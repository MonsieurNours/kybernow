---
slug: profils-et-qualite
slug_en: profiles-and-quality
titre: Profiles, video format, bitrate and HDR
title: Profiles, video format, bitrate and HDR · KyberNow
description: Pick the right profile, video format, PyroWave, bitrate and HDR so the picture keeps pace with your network and your screen.
rubrique: Pendant la session
rubrique_en: During a session
univers: commun
duree: 7 min
connexes: [clavier-et-raccourcis, plusieurs-ecrans-et-ordinateurs, hors-de-chez-vous]
---

A profile sets the resolution, the format, the bitrate and the way losses are repaired in one go. Pick it on the home screen, next to “Connect”, then fine-tune it in the computer settings if you want to.

## At the start and end of a session

When you connect, the computer creates a virtual display at your device's resolution and refresh rate, turns off its own displays and sends the sound to the device (on a PC, if Steam is installed; on a Mac and on Linux, through the “KyberNow” sound output). At the end, the displays and the sound output go back to how they were. The computer is not locked.

The home screen tells you what the session will use, for example “2560 × 1440 display at 120 Hz, sized for this screen”. To leave the computer's displays as they are, turn off “Virtual display” in the Picture section of the computer settings.

## The profiles

| Profile | What it does |
|---|---|
| Game | “Responsiveness first, for gaming.” The lowest latency. Spare data travels with the picture and fills in small losses, without waiting for anything to be sent again. |
| Desktop | “Sharp text, for work.” Full-resolution colour (4:4:4) keeps text sharp. Anything lost is sent again, so nothing is missing from the picture. |
| Max | “The best picture this device can show.” Low latency, sharp text and a high bitrate, with the Game profile's spare data. For a device connected by cable. |
| Auto | “The best settings for this network and this device.” A test of about ten seconds measures your network and your device, then suggests settings. |
| Custom | “Your own settings.” It appears as soon as a setting differs from the chosen profile. |

The profile is kept for each computer. During a session, change it in the bar (the profile's name, then the list): it applies straight away. The Mac and Windows apps also offer it in their icon's menu, in the Mac's menu bar or next to the Windows clock.

> Good to know: with several displays on the device, Desktop and Max use all of them, Game just one. See [Several displays and several computers](/guide/plusieurs-ecrans-et-ordinateurs/).

## The Auto profile

1. Choose “Auto”. The first time, the “Auto profile” panel opens and the test starts.
2. Five steps are ticked off one after another: “Connection” (cable, Wi-Fi or relay), “Latency at rest”, “Bitrate to this device”, “Decoding on this device” and “The computer's encoder”. Each shows its measurement, for example “940 Mbit/s · latency under load +3 ms”.
3. The “Suggested settings” screen lists each choice with its reason: the format, the resolution, sharp text, the bitrate, the picture transport and adaptive bitrate.
4. Choose “Apply”. Nothing changes before that. “Cancel” keeps the previous profile.

The result is kept for this computer: next time, Auto applies without a test. Under the bitrate, the “Bitrate” card shows the date and network of the test (“Chosen by the test of …”) and offers “Run the test again”. If you choose it during a session, the test runs when the session ends and the network is free, and the app tells you so (“The test will run when the session ends.”).

If you connect from a different network from the one tested, a “Different network from the test” banner shows for a few seconds. Adaptive bitrate takes care of it; “Run the test again” measures afresh.

The Auto profile is in every KyberNow Link app except the browser.

## Video format

In the Picture section of the computer settings, “Video format” offers:

| Format | What it is for |
|---|---|
| Auto | The profile's choice. HEVC most of the time, PyroWave with Max and Auto when the device is connected by cable. |
| HEVC | The default format: a sharp picture at a reasonable bitrate, on Wi-Fi as well as away from home. |
| H.264 | The oldest, played by almost every device and browser. No HDR with it. |
| AV1 | The newest, when both the computer's graphics card and your device can handle it. It can't be recorded. |
| PyroWave | Almost no compression delay, at the cost of a lot of bandwidth. For an Ethernet cable. |

PyroWave only appears if the computer knows it. When it is dimmed, its help text gives the reason.

## PyroWave

PyroWave compresses and decompresses each frame in a fraction of a millisecond. In exchange, it sends 5 to 10 times more data than HEVC: about 300 Mbit/s at 1080p. It is made for a device plugged in by Ethernet cable on the same network as the computer.

1. On the computer, open Admin, Picture and sound page. The “PyroWave” card shows “Ready” and the name of the graphics card (or the Mac's chip) when the computer can do it. Turn on “Offer PyroWave”.
2. On the device, leave “Video format” on “Auto” with the Max or Auto profile: the app uses PyroWave when the device is connected by cable, HEVC otherwise. The Game and Desktop profiles stay on HEVC.
3. To force it on Wi-Fi or away from home, choose “PyroWave” in “Video format”. The app warns you: at the start of the session, a “PyroWave on Wi-Fi” or “PyroWave away from home” banner offers “Switch to HEVC”, and an amber chip stays in the bar.

When bandwidth runs short, PyroWave first keeps your screen's resolution, then lowers it if the network or the computer can no longer keep up: a “Resolution lowered to …” banner says so, with the new resolution and the bitrate it fits in. If the network stays saturated for several seconds, “The network is saturated” banner also offers “Switch to HEVC”. With PyroWave, sharp text (4:4:4) costs little more than bandwidth.

Admin also sets PyroWave's “Maximum bitrate”: “Auto” follows the computer's network port, or 300, 500 and 800 Mbit/s.

> Good to know: the Station offers it on Windows and Linux with a graphics card that has Vulkan 1.3, and on a Mac with Apple silicon. In a browser, “Auto” never picks it: choose it by hand. Among Raspberry Pi boards, only the Pi 5 decodes it.

## Bitrate

The bitrate is the amount of picture data sent every second. The higher it is, the finer the picture, and the more network it needs.

“Bitrate” is set to “Auto” (the profile's bitrate) or to a bitrate you choose, for example “80 Mbit/s”. Below it, the “Adaptive bitrate” switch is on by default:

- when the network is saturated (busy Wi-Fi, a shared connection), the app lowers the bitrate, then raises it again as soon as things improve;
- a passing spike changes nothing: it takes two loaded seconds in a row;
- switched off, the bitrate never moves;
- with PyroWave, it pauses, and the resolution drops instead.

The browser keeps a fixed bitrate.

The Statistics show the target bitrate and what sets it, for example “42 / 60 Mbit/s · lowered, network saturated”, “80 Mbit/s · fixed” or “440 Mbit/s · encoder ceiling”. Open them with a click on the measurements in the session bar, or with the shortcut <kbd>⌃⌥⌘S</kbd> on Mac, <kbd>Ctrl+Alt+Shift+S</kbd> on Windows and Linux.

> Good to know: every graphics card has a bitrate above which it refuses to encode. When the computer refuses a frame at the start, the app drops by 50 Mbit/s and remembers that ceiling for this computer, whatever the profile.

## Sharp text and picture transport

“Sharp text (4:4:4)” sends colour at full resolution: text and fine lines stay sharp, at a higher bitrate. It is dimmed when HDR is on.

“Picture transport”, in the Network section of the computer settings:

- “Fast” suits the local network and gaming;
- “Reliable” sends again whatever is lost, at the cost of a little delay. It is the right choice on an uneven network.

## HDR

On an HDR screen (a Mac's XDR display, an iPhone, an iPad, an HDR TV behind an Apple TV or an Android TV device, an LG TV), an “HDR” chip appears on the computer's card, next to the profile. It is on by default and the app remembers your choice for each computer. During a session, “HDR” can be ticked and unticked in the bar; the picture restarts in one to two seconds.

Games and the desktop in HDR arrive as they are. The computer switches its virtual display to HDR at the start of the session and puts it back as it was at the end. Every profile switches to HDR, in 4:2:0: sharp text (4:4:4) comes back when you turn HDR off. HDR doesn't count as part of the profile, so it doesn't switch it to “Custom”.

On the computer's side, you need:

- on a PC, Windows 11 24H2 and a graphics card that encodes in 10 bits (HEVC or AV1);
- on a Mac, nothing more: the Station switches to HDR in the same way.

A Linux Station sends the picture in SDR. In Admin, Picture and sound page, “HDR” is set to “When the device asks” by default, or “Never”.

### Calibrate HDR for your screen

The computer and the games set their picture to your screen's brightness. To adjust it by eye, as on a console, open the app's Settings › “HDR calibration” (or “Calibrate HDR for this screen…” under “HDR”, in the Picture section of the computer settings).

1. “Maximum brightness”: raise it until the symbol in the centre only just disappears.
2. “Full-screen brightness”: the same.
3. “Black”: lower it until the symbol only just disappears.
4. “Desktop white”: choose the brightness of the computer's windows and text.

The calibration applies from the next session. “Back to automatic” clears your values. On a TV, switch to Game mode with HGIG: dynamic tone mapping throws the calibration off.

The Statistics say “HDR” when it is being sent. A share guest without an HDR screen, or in a browser, receives a picture converted to SDR.

## If something goes wrong

- **The “HDR” chip is grey.** Its help text gives the reason: “HDR needs Windows 11 24H2 on the computer.”, “The computer's graphics card doesn't do HDR.”, “HDR needs the HEVC or AV1 format.” (set “Video format” back to “Auto”), “This device can't decode HDR.”
- **“The computer couldn't switch to HDR: picture in SDR.”** Windows refused HDR for this session. The session carries on in SDR; try again next time.
- **“PyroWave” is dimmed.** Its help text says, for example, “Bureau-Salon doesn't offer PyroWave.” (with your computer's name): turn on “Offer PyroWave” in Admin. If the card says “Unavailable”, its message tells you what to do (update the graphics driver, then restart the Station).
- **The picture stutters with PyroWave.** The network can't carry its bitrate: choose “Switch to HEVC”, or connect the device by cable.
- **The picture is blurry when the Wi-Fi is busy.** Adaptive bitrate is lowering the bitrate. The Statistics confirm it (“lowered, network saturated”); move closer to the router or use a cable.
- **The Auto test shows “Update Bureau-Salon to measure the bitrate.”** The Station is out of date: update it, then “Run the test again”.
