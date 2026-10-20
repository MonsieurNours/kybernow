# KyberNow releases

English · [Français](CHANGELOG.md)

English translation of `CHANGELOG.md`. The release notes published on GitHub, and shown by the computer's console and the Mac app before installing, come from the French file; add each new section to both files.

## 0.2.6

- Console: a second device that connects takes over with notice. The first one is told at its next heartbeat and disconnects cleanly, then the virtual display is recreated at the second one's size; before, the two devices kept stealing the display from each other and the first one's disconnection cut the second one's display and sound. The session is now recognised by the device rather than by the account.
- Console: five wrong passwords from the same address make it wait 30 s, then twice as long at each attempt; password checks now run at most two at a time.
- Console: when the service stops (update, shutdown), the displays and the sound are given back to the computer right away; an unreadable setting no longer prevents the console from starting; a display or sound that could not be given back is retried at the next start; a display agent that does not answer is stopped; two simultaneous changes to `kyber_config.toml` no longer overwrite each other.
- Mac and Apple TV apps: no more possible crash on disconnection or profile change. The decoders were released after a fixed delay while Kyber could still deliver them a packet; Kyber now signals its last call. The keyframe request no longer holds Kyber's loop when the computer does not answer (Quit no longer freezes), and its retry also fires while a menu is open.
- libavconv capture engine: the pointer follows the mouse again on displays above 60 Hz (it stayed frozen while the mouse moved), and keeps its shape when Kyber's input service restarts.
- Keyframes requested by the apps after a loss: at most one per session every 150 ms, so a network losing many packets does not weigh down the image of every shared screen.
- libavconv capture engine: `audio_layout = "stereo"` setting (`[kyavserver]` section) to send stereo sound even when Windows outputs 5.1 or 7.1; otherwise Kyber's web client, which only plays stereo, stays silent. The sound also announces its packet size, without which a Kyber client with unreliable sound stopped.
- Computer updates without the risk of leaving everything stopped: the new version is prepared next to the old one and then put in place in one go; if the console does not answer within a minute, the old version comes back by itself and the Updates page says why. The manual update also checks the package's checksum, and “Installed apps” shows the right version.
- Shared computer: KyberNow's data folder (`C:\ProgramData\KyberNow`) is now accessible to administrators only. Another account on the computer can no longer read the password hash there or slip a file into an update.
- Installer: on a laptop without a “High performance” power mode, the installation no longer stops at 96%: the mode is recreated, or left as it is with a warning. The option is unticked if the computer already uses it.
- Installer: Alt+F4 no longer closes the window in the middle of an installation, and KyberNow appears in “Installed apps” as soon as the files are copied: an interrupted installation can be uninstalled normally.
- Installer: run again on a computer where KyberNow is already installed, it offers to keep the settings (accounts, paired devices, ports, certificate, console settings) and replaces only the programs and drivers. “Start from scratch” is still possible, with a warning: devices will have to be paired again.
- More complete uninstall: the virtual display driver also leaves Windows' driver store, the power mode and network category used before KyberNow are put back, and the settings can be kept for later.
- Installer: the interface is extracted under Program Files instead of the temporary folder; the end of the installation checks that the server and the console answer; an official Kyber installation whose uninstall command has arguments is removed properly; standard accounts can no longer pause the services.
- Apple TV app: every picture restart (profile change, fallback) left Bluetooth keyboard and mouse observers behind; they are now removed.
- Mac and Apple TV apps: per-computer setting “Manette vue par l'ordinateur” (gamepad seen by the computer): Xbox 360 (default, recognised by every game) or faithful (a Sony gamepad stays a DualShock 4). It overrides the console's “all gamepads as Xbox 360” option, which only remains a default for other clients.
- Mac app: on a European (ISO) Apple keyboard, the `@#` key gave `<` on the computer and `<>` gave `@`: both are back in place. Apple TV app: “Clavier Apple ISO” setting for Apple Bluetooth keyboards.
- Mac and Apple TV apps: a release refused by the input send queue (64 packets) is no longer lost silently: it is retried for up to 200 ms, and refusals are counted in the log. On the Apple TV, a stick only sends the axis that moved, in 4 ms slices: a DualSense over Bluetooth sent up to 1,500 packets per second.
- Apple TV app: when the app goes to the background (TV button, another app, sleep), nothing stays pressed on the computer any more; on return, the sticks and triggers pick up their real state.
- Mac and Apple TV apps: when the bar or the menu opens, the gamepad is reset on the computer (buttons released, sticks and triggers at rest). A held trigger no longer stays pressed in the game while the menu is open; on the Apple TV, the gamepad as mouse also releases its clicks and keys, and a stick still held when the menu closes picks up right away.
- Mac and Apple TV apps: pressing L1 + R1 + Start + Select during a reconnection or before the first frame no longer leaves the gamepad silent until the next combination. The gamepad is only taken when the bar (Mac) or the menu (Apple TV) really opens, and a picture restart no longer hands it back to the game while the menu is open.
- Mac app: after ⌘Tab, ⌘Space or a trip to another app, the ⌘ key (sent as Ctrl) no longer stays pressed on the computer. Everything is released when the window loses the keyboard, and the modifiers really held are sent again when it gets it back.
- Web client: a click in the session bar (Quit, Sound, Keys…) or on a touch button no longer also clicks on the computer, under the bar.
- Web client on phones and tablets: text typed on the device's keyboard arrives correctly on a computer with an AZERTY layout, with capitals, accents and emoji, and a keyboard attached to the iPad no longer doubles letters. On touch devices the image therefore goes through WebSocket, as on iPhone and iPad.
- Web client: leaving the window (Alt+Tab, ⌘Tab, another tab) or giving the mouse back releases held keys and buttons on the computer: no more stuck Shift, Alt or click.
- Web client: the session holds in a hidden tab (the console no longer thinks it ended after 30 s), and an expired console sign-in is announced clearly instead of a reconnection that fails.
- Web client: the page no longer wrongly switches from WebTransport to WebSocket while the display is being prepared, and closes the old connection before opening the new one (no more doubled inputs). “Kyber is not responding” and “the image does not arrive” are no longer confused.
- Web client: the mouse wheel works in Firefox, and two-finger scrolling on touch devices moves in whole notches. In Firefox the image goes through WebSocket, in H.264 by default.
- Web client: when the browser blocks the sound (Safari, iPhone), a “Tap for sound” chip brings it back, instead of a silent session.
- Web client: when the computer's display changes (size, display removed) or its capture fails, the image comes back by itself, without reloading the page (three attempts for an error). The bar shows connected gamepads, and end messages say what actually happened.
- Web client on touch devices: with the keyboard open, the image keeps the whole height above the row of keys (it dropped to a quarter of the screen), and on an iPad with a keyboard fingers work too. Closing the tab in the middle of a session asks for confirmation.
- Apple TV app: gamepads, remote, Bluetooth keyboard and mouse no longer go through the app's main thread; an interface animation no longer delays a button or a stick by several milliseconds.
- Apple TV app: game rumble is continuous and follows the game's intensity, instead of stuttering at every change; it also starts faster.
- Apple TV app: four gamepads at most on the computer, as with XInput; a fifth one waits (message in the Gamepads screen and during the session) and takes the slot of the first one that disconnects, instead of being announced to a computer that cannot see it.
- Apple TV app: the Siri Remote as mouse accelerates with the finger's speed (slow for aiming, fast to cross) and adapts to the resolution of the computer's screen, instead of a fixed speed that was too slow in 4K; a finger at the centre of the pad is no longer taken for lifted. The gamepad as mouse also crosses the screen in 1.5 s whatever its resolution.
- Apple TV app: the L1 + R1 + Start + Select combo only opens the menu when held for 250 ms; a brief press of the four buttons stays with the game, without releasing anything behind its back.
- Apple TV app: the gamepad's Home button can go to the game as Guide (Steam Big Picture, Game Bar), if tvOS allows it; otherwise, set “Bouton Guide” to long press: Menu or Options held for 0.6 s send Guide. The test screen shows whether Home reaches the app.
- Apple TV app: a Bluetooth mouse's wheel finally scrolls the right way (it scrolled sideways), one notch per notch; buttons 4 and 5 (back, forward) work.
- Apple TV app: gamepad as mouse, pushing the right stick up scrolls up (it was the other way round); a click from the mouse, the remote or the gamepad as mouse no longer lands beside the pointer when the interface is busy, and a held click is no longer forgotten when the menu opens or the gamepad turns off.

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
