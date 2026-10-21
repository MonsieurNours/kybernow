# KyberNow for iPhone and iPad

English · [Français](README.md)

KyberNow's streaming client on iPhone and iPad: you get your computer (Kyber 0.29 server + KyberNow console) in full screen, by touch, with the iPad's keyboard and trackpad, or with a gamepad. v2 identity ([guidelines](../../brand/v2/README.en.md)); mock-ups: [Claude Design](https://claude.ai/artifact/4ef5m4JSRnzuitJcuCFjMX) (home, adding, transition, iPhone and iPad session, measurements, settings, exception states, icon, launch).

iOS and iPadOS 18 or later. Same identifier as the Mac and Apple TV apps (`fr.leslignesclaires.kybernow`: shared Keychain and iCloud); distributed through TestFlight (internal). First version of the app (milestone 5 of [docs/coeur-apple.en.md](../../docs/coeur-apple.en.md)), written on the Rust core from the start, to be tried on a device.

## What the app does

- **Home**: the chosen computer full frame (its last image, a preview requested from its console, otherwise its signature), its state (online, local network or VPN, offline: desaturated image and “Retry”), the profile under the button, the last session at the bottom; the other computers as cards (long press: Settings, Remove), those found on the network. On iPad, the “Ready to play” column (gamepads, sound output, last latency), which goes away at reduced width.
- **Adding**: from the Mac (iCloud), on the network (Bonjour), six-digit code accepted in the console, address, or QR code through the camera (`kybernow://` link, also received through AirDrop or Messages); fingerprint checked on its own when the link or the announcement carries it.
- **Launch**: the system's still image (ink, unlit symbol), then 1.8 s of lighting up in the app; with “Reduce Motion”, a fade.
- **Transition**: the home screen cut along the 45° seam, the neon tube flickering then lighting up, the halves parting onto the image; the “You are entering…” capsule at the bottom, under the thumb, with Cancel. On iPhone, the session is always in landscape.
- **Session**: the image full frame, the tab at the top (live latency), the bar by touch or by swiping down from the top edge (latency and curve, profile, sound, trackpad or direct touch, mouse Auto / Desktop / Game, display, Keys, Quit). Trackpad: one finger moves the computer's pointer, tap = click, two fingers = right click, two fingers sliding = wheel, hold = drag. Direct touch: the finger is the pointer. Keyboard button under the right thumb; above the keyboard, a row of keys (Esc, Tab, sticky Ctrl, Alt and ⊞, arrows, F1 to F12, Ctrl + Alt + Del); text goes as Unicode. Keys menu (Ctrl + Alt + Del, Alt + Tab, ⊞ + D, Print Screen, Alt + F4, ⊞ + L, Task Manager, F11). Bluetooth gamepads (L1 + R1 + Start + Select opens the bar and the gamepad drives it), the iPad's keyboard and mouse or trackpad (iPadOS pointer hidden, the computer's drawn). Detailed measurements in a sheet on iPhone, in a panel on iPad.
- **States**: in a capsule at the top of the image, never an alert: reconnection (neon lit, Quit), computer mode changed (“Put back” in the Desktop profile), fallback when the image does not get through, gamepad connected.
- **Background**: everything is released on the computer, the bitrate capped, the image frozen; on return, everything comes back.
- **Settings**: per computer (image, sound, inputs, network, account), and the app's own.

## Code

The KyberKit module is the Mac's and the Apple TV's, core included: the session (`SessionEngine`), the console (`ConsoleClientCore`), inputs (`InputCore`: kynput chain, pointer held by the app, keyboard, combination, clipboard through `UIPasteboard`), sound (`AudioPlayerCore`, RemoteIO output), latency, VideoToolbox decoding and Metal rendering come from `apps/macos/KyberKit`; the Metal surface, Bluetooth keyboard and mouse (`KeyboardMouse`) and gamepads (`GamepadBridge`) from the Apple TV (`apps/tvos/KyberKitTV`). iOS's own: `KyberKitIOS/StreamView.swift`, the touch session view (gestures, on-screen keyboard as `UIKeyInput`, sticky keys, trackpad hover).

The app (`KyberNowIOS/`): `AppModel`, `Platform` (screen, sound, preferences, computer images), `Theme` (the Apple TV's tokens at thumb scale), `HomeView`, `LaunchView`, `AddHostView`, `SettingsView`, `AppSettingsView`, `SessionController` (the session as seen by the interface, on `SessionEngine`), `SessionView` (transition, bar, key row, Keys menu, capsules), `SessionStats`. The model (`Model.swift`) and discovery (`Discovery.swift`) come from the Mac app; iCloud sync (`CloudSync.swift`) and computer images (`Backdrops.swift`) from the Apple TV.

## Building

```sh
# libkynput for iOS (once):
../core/scripts/build-apple.sh --target aarch64-apple-ios
../core/scripts/build-apple.sh --target aarch64-apple-ios-sim

# The Rust core, its C bridge, libopus and kynput's header (after every change to apps/core or apps/apple):
../apple/scripts/build-apple-mobile.sh ios

# Xcode project (XcodeGen):
xcodegen generate
open KyberNowIOS.xcodeproj
```

`build-apple-mobile.sh ios` builds `kybernow-core` and `kybernow-apple` as static libraries (iPhone and simulator), adds libkynput, libopus (built with cmake) and VLC's empty functions that kyclient references without ever calling them, and puts it all into `Vendor/KyberNow.xcframework`, with `kybernow.h` (`CKyberNow` module) and `kynput.h` (`CKyber` module, view layout and gamepad constants).

In Debug in the simulator: `-demo YES` adds three demo computers; `-skipLaunch YES` skips the launch; `-screen add|settings|session|bar` opens a screen (the demo session shows a test pattern, without a computer).

## Signing

Team P6DB682S4K, automatic signing, identifier `fr.leslignesclaires.kybernow`. Capabilities: iCloud (key-value store), Keychain sharing. Distribution: TestFlight, internal testers.

## To do

- Try a session on a real iPhone and iPad (the simulator only shows a test pattern: no hardware decoding).
- External display (image on the connected display, iPad as a console) and Stage Manager (window): in the mock-ups, not written yet.
- Reading the clipboard makes iOS ask for permission to paste, for each new text copied during the session.
- Uploading to TestFlight (Release archive and `xcodebuild -exportArchive`), once the app has been tried.
