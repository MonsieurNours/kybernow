# KyberNow releases

English · [Français](CHANGELOG.md)

English translation of `CHANGELOG.md`. The release notes published on GitHub, and shown by the computer's console and the Mac app before installing, come from the French file; add each new section to both files.

## 0.2.1

- Gamepad: it stopped responding after Kyber reconnected (for example when the virtual display is recreated). The app now keeps only one input chain at a time.
- Mac app: after Kyber restarts on the computer, the session resumes on the virtual display instead of ending (the app asks the console for it again). The stats panel no longer makes the picture stutter (no more frosted glass recomputed on every frame).
- Game mode: the computer's pointer (menus, inventories) is drawn by the app at its real position. The computer now also sends the pointer's movements.
- AV1 decoded by the app; choice of the computer's display in the bar (without a virtual display).
- Console: preview of the computer's screen and the name of its system (“Windows 11 Pro 24H2”) for the apps' home screen; the system is also shown on the dashboard.
- Mac app: new identity (v2 guidelines). The home screen shows each computer's image (last image of its session, an up-to-date preview or its signature); the session opens in the same window through a two-halves transition; neon on the important moments; new icon (Icon Composer).
- Mac app: detailed measurements during a session (click the measurements in the bar, or ⌃⌥⌘S): the path of a frame step by step, latency and bitrate over 30 s, losses.
- Mac app: a keyframe lost at start-up no longer leaves the screen black; if the image does not come, the session lightens the stream (bitrate, then 4:2:0).
- Console: new identity and four pages (Overview, Picture and sound, Access, System). The screen preview in large, who is connected and their latency, “End the session”, known devices and what happened today; logs by source; usable on a phone.
- Icon next to the clock: a panel on left click (state, current session, update, server), “Reset the console password”, new symbol.
- Installer: new identity, four steps instead of six (the computer is analysed during the welcome step).
- Mac app: it introduces itself to the console (name, system, latency) and disconnects when the console ends the session.
- New web client: open the computer from a browser (Chrome, Edge, Firefox), with nothing to install, at the console's address followed by `/ecran`, or with “Open in the browser” on the Overview page. Same password as the console; home screen on the screen preview, Desktop and Game profiles, session bar with latency, measurements and Ctrl+Alt+Del; trackpad, direct touch and special keys on tablets and phones; Auto, Desktop or Game mouse as in the app; Safari, iPhone and iPad go through WebSocket. Kyber's original client stays available for troubleshooting.

## 0.2.0

First published release.

**Computer (server and console)**
- Ready-to-install installer: patched Kyber 0.29, administration console, notification area icon, virtual display, gamepads, remote sound.
- Patched Kyber: HD and multichannel sound, continuous low-latency sound (10 ms packets), stable pointer, keyframe on request from an app that lost packets.
- Console: app pairing (Bonjour, QR code, link), detection of full-screen games, one-click updates.

**Mac app**
- Image decoded by the app (VideoToolbox, H.264, HEVC, AV1) and displayed with Metal: minimal latency, variable refresh rate displays followed, 4:4:4.
- Sound decoded by the app (Opus, stereo, 5.1, 7.1), with about twenty milliseconds of buffer.
- The Mac display's real resolution (4K, 5K…); Game, Desktop and Max profiles brought down to what the machine can decode.
- Auto, Desktop or Game mouse; shared clipboard; gamepad and session bar; end-to-end latency measured.
- One-click updates (Sparkle).
