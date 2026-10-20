# KyberNow for Apple TV

English · [Français](README.md)

KyberNow streaming client for the TV: your computer (Kyber 0.29 server + KyberNow console) full screen, with the Siri Remote or a gamepad, at up to 4K 60 Hz (the Apple TV's limit), with multichannel sound and up to four gamepads. Brand v2 ([guidelines](../../brand/v2/README.en.md)); mockup followed screen by screen: [Claude Design](https://claude.ai/artifact/RfGbfguTyAP1jjwFASVRyK) (home, adding, Top Shelf, passage, session, gamepads, settings).

**Installing the app without the App Store**: [SIDELOAD.en.md](SIDELOAD.en.md).

Apple TV 4K 2nd generation (A12) and later, tvOS 18 or later. Hardware HEVC and H.264; no AV1 (none of these chips decodes it in hardware).

## What the app does

- **Home** (`HomeView`): the selected computer fills the screen, its last image slowly drifting in the background; its name in large type, online or not (local network or VPN), system, graphics card and address, the virtual display waiting for it; "Se connecter" (lit when focused) and the Game, Desktop and Max profiles. On the right, "Prêt à jouer" (ready to play): connected gamepads and their battery, the sound output (HDMI 7.1 receiver, TV, AirPods…) and its delay, the session's picture and the last session's latency. At the bottom, the computers: focusing a card changes the scene; those announced on the network (Bonjour) join them, plus the add card. Hint glyphs follow the last gamepad used (remote, A / B, ✕ / ○).
- **tvOS focus**: the brand's neon standby. The focused element grows (× 1.06), lifts and gets a frost outline and a Spectre halo (`GlassButton`, `ChoiceButton`, `PrimaryButton` in `Theme.swift`).
- **Add a computer** (`AddHostView`), preferably without typing on the remote:
  - **From your Mac**: computers published by the Mac app in iCloud (key-value store, key `hosts`) arrive with their password (iCloud Keychain, service `fr.leslignesclaires.kybernow.cloud`); added automatically when "Synchroniser avec iCloud" is on (`CloudSync`). The Mac app publishes them on every addition, change or removal (`CloudPublisher`), when it is signed with the iCloud entitlements (see [apps/macos/README.en.md](../macos/README.en.md#signing-and-icloud)).
  - **On the network**: the console's Bonjour announcement `_kybernow._tcp` (except `kyber=0`: a console with no Kyber server behind it), then the account. tvOS offers typing on the iPhone keyboard and the Keychain. The certificate fingerprint seen must match the announcement.
  - **Code on screen**: the Apple TV draws a 6-digit code and presents it to every console found (`POST /api/pair/request`, then `GET /api/pair/request/{id}`, in `ConsoleClient`); you accept it in the computer's console (Access › Devices, or the panel of the icon by the clock); the Apple TV receives a Kyber account of its own, revocable in Devices (see [server/README.en.md](../../server/README.en.md)). QR code of the console to open it on a phone. A console from an earlier version does not know these calls: the app says so ("update KyberNow on the computer").
  - **Address**: console address and port (VPN), then the account, with the first four groups of the fingerprint to compare.
- **Top Shelf** (`TopShelf/` extension): when KyberNow is in the top row of the Apple TV home screen, "Reprendre sur un ordinateur" shows each computer with its last image; a click opens the session directly (`kybernow-tv://connect/<id>`). The app drops the list and images in the `group.fr.leslignesclaires.kybernow` container.
- **Choosing the display**: as on the Mac, the app waits at most 6 s, only once, for Kyber to announce only the virtual display, then takes the display at the requested size placed at (0, 0). Outputs that Windows lists as 0 × 0 (off) are left out; if no usable display remains, the session says so.
- **The passage** (`SessionView`, `Passage`): as on the Mac, the home screen splits along the 45° seam, the neon tube lights up, then both halves slide apart over the image. "Vous entrez dans…" capsule: how to open the menu, the screen, sound and connected players, time to first image, live latency.
- **In session**: the image lives in a `GCEventViewController`. With the menu closed, gamepads and the remote only talk to the game (Back does not quit the app). **Menu** with the remote's Back button or L1 + R1 + Start + Select (as on the Mac), each can be turned off: the image recedes (× 0.94, dimmed), stream health at the top (latency, frames per second, bitrate, format, losses), and Resume, Gamepads, Sound (output and its delay, AirPods / HomePod / TV picker), Remote as mouse, Type text (tvOS or iPhone keyboard, sent character by character), Disconnect. While the menu is open, gamepads drive KyberNow and the game receives nothing (held buttons are released for it). The session controller is what takes the gamepad when the menu really opens, not `GamepadBridge`, which only reports the combination: outside a session (connecting, reconnecting), the combination does nothing and the gamepad stays with the game, instead of going silent until the next combination; when the picture restarts (profile change from the menu), the menu keeps the gamepad.
- **Sound**: Opus decoded by the app (KyberKit's `AudioPlayer`, RemoteIO); Auto follows the output (stereo, 5.1 or 7.1 LPCM on an HDMI receiver); spatial audio in AirPods (multichannel content); or sound stays on the computer. Warning when the output adds delay (AirPods, HomePod, AirPlay: over 40 ms), with the measured delay.
- **Gamepads** (`GamepadBridge`, `GamepadsView`): GameController instead of SDL. Xbox, DualSense, DualShock and MFi; each gamepad is a player on the computer (P1 to P4), in power-on order or at a fixed slot; player indicator, Sony light bar in the player's colour. They all become Xbox 360 gamepads on the computer, Sony ones included (the session's `gamepad_force_xbox360`). They announce themselves ("plugged") when the inputs reach the computer (`INPUT_SERVICE_CONNECTED`), then on each reconnection: sent earlier, the announcement was lost, and the computer then rejected the buttons of an unknown gamepad. Game rumble on both motors (Core Haptics, left and right handles). Test screen: live buttons, sticks, triggers, test rumble. The Home button stays with tvOS.
- **Gamepad as mouse** (session menu tile, on by default with the Desktop profile): left stick = pointer (progressive acceleration, LT / L2 for precision, RT / R2 to go fast, speed in Settings › Gamepads), right stick = wheel, A / ✕ left click, X / □ right click, B / ○ middle click, d-pad = arrow keys, L1 / R1 = Alt + Tab (Alt held while you keep going, plus Shift with L1), Start = Windows key, Select = Escape. The menu combo's buttons act on release, unless the combo was formed. The game no longer receives the gamepad; turned off, it is a gamepad again, nothing left pressed.
- **Remote as mouse** ("Pavé en souris" tile): touch surface = pointer, click = left click, Play/Pause = right click; always on with the Desktop profile. **Bluetooth keyboard and mouse** (`KeyboardMouse`): HID usages converted to scancodes by kynput.
- **Pointer** (`StreamView`), like the Mac app in desktop mode: the Apple TV holds the pointer position and sends it as absolute (remote, gamepad and mouse move it). Windows hides its pointer with no mouse plugged in, or at the slightest keystroke, and only shows it again on an absolute position: with relative moves it stayed hidden and the app lost it while moving. The app draws it at that position, always visible, with the computer's pointer shape (an arrow until then), twice desktop size (readable from three metres). If the computer moves its pointer itself, the app picks up its position once idle. Only the Bluetooth mouse sends raw motion, when the console sees a full-screen game (`GET /api/stream/foreground`, every second), like the Mac's Auto mode.
- **Settings** (`SettingsView`), per computer: profile; picture (Auto or 4K, 1440p, 1080p; 60, 50 or 30 Hz; bitrate from 5 to 4,294 Mbit/s with a warning above 150; HEVC or H.264; 4:4:4 if the Apple TV decodes it in hardware; match the TV to the session's refresh rate); sound; gamepads and remote; network (transport, reconnection, Ethernet or Wi-Fi); account and iCloud (sync, last image, remove the computer).
- **tvOS profiles**: the app runs as the current tvOS user (`com.apple.developer.user-management`: `runs-as-current-user`): each household member has their own computers, settings and Keychain. Users are switched in the Apple TV's Control Center (tvOS does not let an app do it, nor read the user's name).
- **Session** (`SessionController`): same flow as on the Mac (preparation by the console with the TV's screen capped at 4K60, heartbeat, reconnection, virtual display requested again, fallback when the image does not get through, clean end and last image kept).

## Code shared with the Mac

KyberKit is the same module as on the Mac. The project takes everything in `apps/macos/KyberKit` that does not depend on AppKit (connection, VideoToolbox decoding, Metal rendering, sound, latency, console) and adds `KyberKitTV/`: UIKit session view (`StreamView`, `MetalSurface`), gamepads and remote (`GamepadBridge`), keyboard and mouse (`KeyboardMouse`). The few differences are behind `#if os(...)` in the shared files (no clipboard on tvOS, gamepads produced by the app, frames presented at the next refresh). The model (`Model.swift`: computers, settings, profiles) and discovery (`Discovery.swift`) come from the Mac app.

## Building

```sh
# Kyber SDK for tvOS (once, then after a change to kyber-patches):
rustup toolchain install nightly --profile minimal --component rust-src
scripts/build-kyber-tvos.sh

# Xcode project (XcodeGen):
xcodegen generate
open KyberNowTV.xcodeproj
```

`scripts/build-kyber-tvos.sh` builds Kyber 0.29's libkynput and libkyclient, with the Mac app's patches (`apps/macos/kyber-patches`), for Apple TV and its simulator (tier 3 Rust targets: `-Zbuild-std`, like Kyber's `build-ios.sh`), plus libopus. VLC is never called (the app decodes image and sound): empty functions satisfy the linker. Everything goes into `Vendor/KyberSDK.xcframework` (arm64 only).

Other scripts: `scripts/phosphor-icons.mjs` (Phosphor icons in the catalog), `scripts/app-icon.py` (layered icon for parallax and Top Shelf images, from the `brand/v2` geometry).

Remote tests (`UITests/`, XCUIRemote): reaching settings from the home screen, keeping a form after the keyboard. `xcodebuild test -scheme KyberNowTV -destination 'platform=tvOS Simulator,name=…'`.

Debug builds in the simulator: `-demo YES` adds three demo computers; `-screen add|settings|gamepads|menu|passage` opens a screen directly, `-addMethod 0…3` an add pane (checking without a remote).

## Signing

Team P6DB682S4K, automatic signing. Identifier `fr.leslignesclaires.kybernow` (same as the Mac app: shared Keychain and iCloud), extension `fr.leslignesclaires.kybernow.topshelf`. Capabilities to enable on the App ID: App Groups (`group.fr.leslignesclaires.kybernow`), iCloud (key-value store), User Management.

## To do

- Validate an end-to-end session on an Apple TV (image, multichannel sound, gamepads, rumble).
- Check on a real Apple TV that the iCloud Keychain does bring the passwords published by the Mac.
- Distribution (TestFlight or ad hoc).
