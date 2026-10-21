# KyberNow for iPhone and iPad

English · [Français](README.md)

KyberNow's streaming client on iPhone and iPad: you get your computer (Kyber 0.29 server + KyberNow console) in full screen, by touch, with the iPad's keyboard and trackpad, or with a gamepad. v2 identity ([guidelines](../../brand/v2/README.en.md)); mock-ups: [Claude Design](https://claude.ai/artifact/4ef5m4JSRnzuitJcuCFjMX) (home, adding, transition, iPhone and iPad session, measurements, settings, exception states, icon, launch).

iOS and iPadOS 18 or later. Same identifier as the Mac and Apple TV apps (`fr.leslignesclaires.kybernow`: shared Keychain and iCloud); not distributed yet (internal TestFlight to come). **Installing the app without TestFlight or the App Store**: [SIDELOAD.en.md](SIDELOAD.en.md). First version of the app (milestone 5 of [docs/coeur-apple.en.md](../../docs/coeur-apple.en.md)), written on the Rust core from the start; session validated on an iPhone 15 Pro Max on 22/10/2026.

## What the app does

- **Home**: the chosen computer full frame (its last image, a preview requested from its console, otherwise its signature), its state (online, local network or VPN, offline: desaturated image and “Retry”), the profile under the button, the last session at the bottom; the other computers as cards (long press: Settings, Remove), those found on the network. On iPad, the “Ready to play” column (gamepads, sound output, last latency), which goes away at reduced width.
- **Adding**: from the Mac (iCloud), on the network (Bonjour), six-digit code accepted in the console, address, or QR code through the camera (`kybernow://` link, also received through AirDrop or Messages); fingerprint checked on its own when the link or the announcement carries it.
- **Launch**: the system's still image (ink, unlit symbol), then 1.8 s of lighting up in the app; with “Reduce Motion”, a fade.
- **Transition**: the home screen cut along the 45° seam, the neon tube flickering then lighting up, the halves parting onto the image; the “You are entering…” capsule at the bottom, under the thumb, with Cancel. On iPhone, the session is always in landscape.
- **Session**: the image full frame, the tab at the top (live latency), the bar by touch or by swiping down from the top edge (latency and curve, profile, sound, trackpad or direct touch, mouse Auto / Desktop / Game, display, Keys, Quit). Trackpad: one finger moves the computer's pointer, tap = click, two fingers = right click, two fingers sliding = wheel, hold = drag. Direct touch: the finger is the pointer. Keyboard button under the right thumb; above the keyboard, a row of keys (Esc, Tab, sticky Ctrl, Alt and ⊞, arrows, F1 to F12, Ctrl + Alt + Del); text goes as Unicode. Keys menu (Ctrl + Alt + Del, Alt + Tab, ⊞ + D, Print Screen, Alt + F4, ⊞ + L, Task Manager, F11). Bluetooth gamepads (L1 + R1 + Start + Select opens the bar and the gamepad drives it), the iPad's keyboard and mouse or trackpad (iPadOS pointer hidden, the computer's drawn). Detailed measurements in a sheet on iPhone, in a panel on iPad.
- **States**: in a capsule at the top of the image, never an alert: reconnection (neon lit, Quit), computer mode changed (“Put back” in the Desktop profile), fallback when the image does not get through, gamepad connected.
- **External display** (USB-C or AirPlay, iPhone as well as iPad): the image moves to the connected display, at its definition (“like this display” then targets the external one, the virtual display is remade at its size), and the device becomes the console: its whole surface is the trackpad, the bar stays open, measurements are shown on iPad. Connected during a session, the image moves without cutting the session; disconnected, it comes back to the device (“The image is back on the iPad”). Outside a session, the external display shows the symbol and a waiting line.
- **Stage Manager**: a single window; the session lives in its window, the image keeps its format (ink bands), the definition stays the one chosen at the start. Losing the foreground (another window, another app) releases everything on the computer, caps the bitrate and freezes the image (“KyberNow keeps running in the background”).
- **Background**: everything is released on the computer, the bitrate capped, the image frozen; on return, everything comes back.
- **Settings**: per computer (image, sound, inputs, network, account), and the app's own.

## Code

The KyberKit module is the Mac's and the Apple TV's, core included: the session (`SessionEngine`), the console (`ConsoleClientCore`), inputs (`InputCore`: kynput chain, pointer held by the app, keyboard, combination, clipboard through `UIPasteboard`), sound (`AudioPlayerCore`, RemoteIO output), latency, VideoToolbox decoding and Metal rendering come from `apps/macos/KyberKit`; the Metal surface, Bluetooth keyboard and mouse (`KeyboardMouse`) and gamepads (`GamepadBridge`) from the Apple TV (`apps/tvos/KyberKitTV`). iOS's own: `KyberKitIOS/StreamView.swift`, the touch session view (gestures, on-screen keyboard as `UIKeyInput`, sticky keys, trackpad hover), which can show the image in an `ImageHostView` placed on another screen (`showImage(in:)`).

The app (`KyberNowIOS/`): `KyberNowApp` (with `AppDelegate`: orientation, external display scene), `AppModel`, `Platform` (screen, sound, preferences, computer images), `Theme` (the Apple TV's tokens at thumb scale), `HomeView` and `HomeParts`, `LaunchView`, `AddHostView`, `AddHostPanes` and `AddHostScanner` (camera), `SettingsView` and `SettingsControls`, `AppSettingsView`, `SessionController` (the session as seen by the interface, on `SessionEngine`), `SessionView` (transition, bar, key row, Keys menu, capsules, console), `SessionStats`, `ExternalDisplay` (the external display's non-interactive scene, declared by `AppDelegate`). The model (`Model.swift`) and discovery (`Discovery.swift`) come from the Mac app; iCloud sync (`CloudSync.swift`) and computer images (`Backdrops.swift`) from the Apple TV.

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

In Debug in the simulator: `-demo YES` adds three demo computers; `-skipLaunch YES` skips the launch; `-screen add|settings|appSettings|session|bar` opens a screen (the demo session shows a test pattern, without a computer).

## Languages

French and English ([docs/i18n.en.md](../../docs/i18n.en.md)): French when iOS is in French, English for any other language. App settings › Language: Auto, Français or English, applied at the next launch. Texts live in the string catalogs (`KyberNowIOS/Localizable.xcstrings`, `InfoPlist.xcstrings`, and `apps/macos/KyberKit/Localizable.xcstrings` for KyberKit). After adding texts: `xcrun xcstringstool sync <catalog> --stringsdata …` from the build's `.stringsdata` (`xcodebuild` does not fill the catalog itself), then `python3 ../macos/scripts/check-strings.py --fix`.

## Signing

Team P6DB682S4K, identifier `fr.leslignesclaires.kybernow`. Capabilities: iCloud (key-value store), Keychain sharing. Planned distribution: TestFlight, internal testers (no App Store profile yet); meanwhile, [SIDELOAD.en.md](SIDELOAD.en.md) (`scripts/sideload.sh`, signed with your own Apple account, `KyberSDK-iOS.zip` SDK attached to releases from 0.3.2 on).

In Debug, manual signing with the “iOS App Development” profile `ios.fr.leslignesclaires.kybernow` (created on 2026-10-21, devices registered in the account), Apple Development certificate. For Xcode to find it, copy the downloaded file to `~/Library/Developer/Xcode/UserData/Provisioning Profiles/<UUID>.mobileprovision`; a new device has to be added to the profile, which is then downloaded again. Installing on a connected device:

```sh
xcodebuild -scheme KyberNowIOS -configuration Debug -derivedDataPath build -destination 'generic/platform=iOS' build
xcrun devicectl device install app --device <UDID> build/Build/Products/Debug-iphoneos/KyberNow.app
```

Release stays on automatic signing; uploading to TestFlight will need an App Store profile.

## To do

- Session successful on an iPhone 15 Pro Max (22/10/2026); not tried on an iPad yet (no iPad in the provisioning profile; the simulator only shows a test pattern: no hardware decoding). On iOS, a Metal layer that presented frames before being in a window stays off screen forever (all its frames discarded, `presentedTime` 0, until the app goes to the background): `MetalSurface` creates its own once in the window and attaches the rendering at that point, and remakes it if the display discards everything for two seconds (log `affichage bloqué`). `KN_PRESENT_STATS=1` (environment variable at launch through `devicectl`) writes every second the frames rendered, shown and discarded.
- External display and Stage Manager: written, but the simulator cannot try them (Xcode 27.1 without Simulator.app; `simctl io … screenConfig` connects a display, but iOS mirrors the screen without giving the scene to the app); to be tried on an iPhone and an iPad (USB-C, AirPlay). If iOS mirrors the screen instead of opening the external scene, set `UIApplicationSupportsMultipleScenes` to `true` in `project.yml`.
- Stage Manager: no “full screen” button in the bar (iPadOS does not let an app enlarge its window).
- The device's clipboard is only read when the computer asks for it (⌘V or Ctrl+V over there): reading it makes iOS ask for permission to paste, which takes the foreground away from the app. Changes are announced without reading (`hasStrings`).
- Uploading to TestFlight (App Store profile to create, Release archive and `xcodebuild -exportArchive`).
