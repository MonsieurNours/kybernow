# KyberNow releases

English · [Français](CHANGELOG.md)

English translation of `CHANGELOG.md`. The release notes published on GitHub, and shown by the computer's console and the Mac app before installing, come from the French file; add each new section to both files.

## 0.2.6

- Computer updates without the risk of leaving everything stopped: the new version is prepared next to the old one and then put in place in one go; if the console does not answer within a minute, the old version comes back by itself and the Updates page says why. The manual update also checks the package's checksum, and “Installed apps” shows the right version.
- Shared computer: KyberNow's data folder (`C:\ProgramData\KyberNow`) is now accessible to administrators only. Another account on the computer can no longer read the password hash there or slip a file into an update.
- Installer: on a laptop without a “High performance” power mode, the installation no longer stops at 96%: the mode is recreated, or left as it is with a warning. The option is unticked if the computer already uses it.
- Installer: Alt+F4 no longer closes the window in the middle of an installation, and KyberNow appears in “Installed apps” as soon as the files are copied: an interrupted installation can be uninstalled normally.
- Installer: run again on a computer where KyberNow is already installed, it offers to keep the settings (accounts, paired devices, ports, certificate, console settings) and replaces only the programs and drivers. “Start from scratch” is still possible, with a warning: devices will have to be paired again.

## 0.2.5

- Apple TV app: gamepads finally respond on the computer. They announced themselves before the inputs were connected, and the computer then ignored their buttons; the remote and the gamepad as a mouse did work.
- Mac and Apple TV apps: every gamepad, PlayStation ones included, becomes an Xbox 360 gamepad on the computer.
- Apple TV app: if the computer announces only screens that are off (0 × 0), the session says so instead of waiting forever.

## 0.2.4

- Steadier 4K picture: the computer no longer drops pieces of a large frame (8 MB QUIC send queues and UDP buffers instead of 1 MB), which made the picture stutter or freeze and then request keyframes one after another.
- Kyber's new capture engine (libavconv, "Capture engine" setting) supported like the historical one: virtual display seen, same display numbers, pointer followed without blinking, 5.1/7.1 sound in 10 ms packets timestamped from the samples.
- Mac app: if the computer announces only screens that are off (0 × 0), the session says so instead of staying on "First frames".

## 0.2.3

- Apple TV app: the pointer no longer disappears when you move it with the remote or a gamepad. The Apple TV now sends its exact position, like the Mac app on the desktop (Windows did not show its pointer again on mere motion), and always draws it. The Bluetooth mouse stays in game mode in a full-screen game.

## 0.2.2

- New Apple TV app (until it reaches the App Store, install it yourself: see https://github.com/MonsieurNours/kybernow/blob/main/apps/tvos/SIDELOAD.en.md): your computer on the TV, with the remote or a gamepad, at up to 4K 60 Hz. Up to four gamepads (Xbox, DualSense…) with rumble, Siri Remote as a mouse, Bluetooth keyboard and mouse, multichannel sound over HDMI, spatial audio in AirPods and a warning when the output adds delay, Top Shelf, per-user profiles on the Apple TV.
- Apple TV app: the gamepad can act as a mouse (session menu, on by default with the Desktop profile): left stick for the pointer, A / ✕ click, X / □ right click, B / ○ middle click, right stick to scroll, d-pad for arrows, L1 / R1 to switch windows (Alt + Tab), Start for the Start menu, Select for Escape.
- Console: code pairing. The Apple TV shows a 6-digit code, you accept it in Access › Devices, and it gets its access without typing anything on the remote.
- Console: the network announcement says whether Kyber is really installed; the apps no longer offer a computer where it is not.
- Mac app: your computers are published to iCloud with their credentials, so the Apple TV finds them on its own ("Synchroniser avec iCloud" setting, ⌘,).
- Console: an update that stops partway no longer blocks the "Install" button ("an installation is already in progress"); the Updates page shows the end of the installation log. Faster unpacking and download of the update (PowerShell without a progress bar). Since 0.2.0, an update started from the console could never start: PowerShell is now launched with a hidden console, and its output is kept.

## 0.2.1

- All graphics cards: the installation chooses the encoder of the most capable card (NVIDIA, otherwise AMD, otherwise Intel, eGPUs included; otherwise the processor). AMF and Quick Sync are no longer presented as experimental, and an update keeps the encoder you chose.
- Gamepad: it stopped responding after Kyber reconnected (for example when the virtual display is recreated). The app now keeps only one input chain at a time.
- Mac app: far fewer lost packets in 4K (8 MB UDP receive buffer instead of 768 KB); the picture no longer stays frozen after the stream restart that follows a lost keyframe.
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
