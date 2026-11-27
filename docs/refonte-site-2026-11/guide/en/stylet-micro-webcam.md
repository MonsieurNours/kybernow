---
slug: stylet-micro-webcam
slug_en: stylus-microphone-camera
titre: Stylus, microphone, camera and recording
title: Stylus, microphone, camera and recording · KyberNow
description: Draw with a stylus, talk on a call with your device's microphone, appear on camera with its webcam, and record the session on the computer.
rubrique: Pendant la session
rubrique_en: During a session
univers: commun
duree: 6 min
connexes: [partager, preter, clavier-et-raccourcis]
---

Your device's stylus, microphone and camera reach the computer as if they were plugged into it. You can also record the session to a video file on the computer.

## The stylus

The pen of a graphics tablet plugged into your device, an iPad's Apple Pencil or an Android device's stylus reaches the computer as a real pen. It carries pressure, tilt, the eraser, the pen button and hover (the tip close to the surface without touching it). Drawing software uses them for the thickness and opacity of the stroke; other apps see a left click.

There is nothing to set up. You need:

- a KyberNow Link app on Mac, Windows, Linux, iPad or Android;
- an up-to-date Windows, Mac or Linux Station.

The stylus follows the desktop mouse. In game mouse mode, it stays a mouse. On a PC, holding the pen button with the tip down gives a right click. On the iPad, the Apple Pencil always writes where it touches, whatever the touch mode. On Linux, the computer receives everything except the pen's rotation.

A share guest gets the stylus when you give them the keyboard and mouse. In the browser, the stylus stays a mouse.

## The microphone

The device's microphone reaches the computer, so you can talk in a game, a call or a video meeting. It works with the Mac, iPhone and iPad, Android, Windows, Linux and Steam Deck apps, and on a Raspberry Pi with a USB microphone. The Apple TV and LG TVs don't send a microphone.

1. During a session, choose “Microphone” in the bar (or the “Microphone” tile in tile menus), or use the shortcut <kbd>⌃⌥⌘V</kbd> on Mac, <kbd>Ctrl+Alt+Shift+V</kbd> on Windows and Linux.
2. The first time, the device's system asks for access to the microphone: accept.
3. The notice “Microphone sent to Bureau-Salon.” (your computer's name) confirms it is being sent. While the microphone is sent, the bar's “Microphone” chip stays lit (on iPhone, iPad, Android and in the living-room apps, a small microphone icon).

The microphone is off for the first session with a computer. After that, each session picks up your last choice for that computer. With the microphone off, nothing is sent.

On the computer, while it is being sent, the virtual microphone becomes the default microphone: “CABLE Output” on a PC, “Micro KyberNow” on a Mac and on Linux. A call app that follows the system's microphone picks it up by itself; otherwise, choose it in its settings. The previous microphone comes back at the end.

> Good to know: on a PC, the virtual microphone is VB-CABLE, installed with the Station. VB-CABLE, by VB-Audio Software, is donationware: vb-cable.com.

In the app's Settings, “Microphone” picks the device's microphone (“System default” by default), and “Reduce echo and noise”, when on, removes the computer's sound and the noise around you. Turn it off for a studio microphone.

A friend you lend the computer to can talk just as you do. Share guests can only talk if you turn on “Let guests speak” in the share panel.

## The camera

The device's camera goes to the computer, which sees it as an ordinary webcam, “KyberNow Camera”. Teams, Zoom, Discord, OBS and browsers list it.

It works with the Mac, iPhone and iPad, Android (phones and tablets), Windows and Linux (desktop and Steam Deck) apps, and on the Apple TV with an iPhone or iPad placed nearby, through Continuity Camera (tvOS 17 or later). It works during your sessions and during a loan. Share guests don't send their camera.

1. During a session, open “Camera” in the bar (or the “Camera” tile, or the shortcut <kbd>⌃⌥⌘C</kbd> / <kbd>Ctrl+Alt+Shift+C</kbd>).
2. Pick the camera from the list: “Front camera” or “Back camera” on a phone, otherwise its name. On the Apple TV, “Choose an iPhone or iPad…”.
3. Choose “Turn on camera”. A red dot and a lit “Camera” in the bar show that it is sending; with the bar closed, a small “● Camera” capsule stays at the top left of the picture.
4. In the computer's video-call app, choose “KyberNow Camera”.

The camera is off at the start of every session. On iPhone, iPad and Android, if the app goes into the background, the computer keeps the last frame and sending resumes when you come back.

“Camera resolution”, in the Controls section of the computer settings, is 720p (the default) or 1080p, always at 30 frames per second. At 1080p, the camera needs about 4 Mbit/s towards the computer.

Only one device at a time can send its camera to a computer.

### The first time on the computer

- **Mac**: a notification asks you to allow KyberNow Station's camera extension in System Settings › General › Login Items & Extensions.
- **Linux**: the camera goes through PipeWire, which browsers and OBS read. To make it available to every app, install v4l2loopback, then restart: see [Install KyberNow Station on Linux](/guide/installer-station-linux/).
- **Windows 10**: Zoom, Teams, Discord, OBS, Chrome and Edge see it; the Windows Camera app doesn't. On Windows 11, every app sees it.

## Record the session

The computer records your session's picture and sound to an MP4 file, without re-encoding: the graphics card does no extra work.

1. During a session, choose “Record” in the bar (or <kbd>⌃⌥⌘R</kbd> / <kbd>Ctrl+Alt+Shift+R</kbd>, or the “Record” tile in the living-room apps).
2. The red “Recording · 02:14” pill counts the time. A click on it stops the recording.
3. When it stops, the notice “Recording saved: find it in Admin › Recordings.” appears.

Everyone in the session sees the pill: a share guest reads “Hugo is recording the session.” and can't stop it. Closing the session stops the recording.

In Admin, on the Recordings page, you find the files (“Play”, “Download”, “Delete”), the folder and how long recordings are kept:

- the default folder is Videos › KyberNow on a PC and on Linux, Movies › KyberNow on a Mac; “Change” picks another;
- “Keep” is set to “30 days” by default, or “7 days”, “90 days”, “Forever”: older recordings are deleted;
- “Allow session recording” turns the feature off for the whole computer.

Only the main display is recorded, without the pointer. PyroWave and AV1 can't be recorded: switch to HEVC or H.264. Nothing can be recorded while the computer is lent.

## If something goes wrong

- **“Microphone” is dimmed.** Its tooltip gives the reason. “The computer has no virtual microphone: see Admin › Picture and sound.”: on a PC, choose “Install VB-CABLE” on the “Virtual microphone” card. “Update the Station to send your microphone.”: the Station is out of date.
- **“KyberNow Link can't access the microphone.”** Allow it in the device's privacy settings (System Settings on Mac, Windows privacy settings), then turn “Microphone” on again.
- **The video-call app hears nothing.** Choose “CABLE Output” (PC) or “Micro KyberNow” (Mac, Linux) as the microphone in its settings.
- **“Camera” doesn't appear in the bar.** The computer doesn't offer the camera for this session, or the device has none. Update the Station.
- **“Léa's iPhone is already sending its camera to Bureau-Salon.”** Turn the camera off on the other device.
- **“The camera is being used by another app.”** Close the other app that is filming on your device.
- **“Bureau-Salon could not open its virtual camera.”** On a Mac, check the camera extension's permission; elsewhere, restart the Station.
- **The stylus behaves like a mouse.** The mouse is in game mouse mode, or the Station is out of date. On Windows, a Wacom tablet with “Windows Ink” turned off stays a mouse.
- **“Less than 2 GB free on the computer: can't record.”** Free up some space, or choose another folder in Admin › Recordings.
- **“PyroWave and AV1 can't be recorded: switch to HEVC or H.264.”** Change “Video format” in the computer settings, or the profile.
