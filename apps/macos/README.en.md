# KyberNow for macOS

English · [Français](README.md)

KyberNow's streaming client: you get your computer (Kyber 0.29 server + KyberNow console) in full screen on the Mac, with image, sound, keyboard, mouse and gamepads. v2 identity ([guidelines](../../brand/v2/README.en.md)); mock-ups: [Claude Design](https://claude.ai/artifact/YGA7unwqztvuA26CMtdbv8) (home, first contact, transition, session). The video and audio code and the capability detection do not use AppKit: the Apple TV app ([apps/tvos](../tvos/README.en.md)) reuses them as they are, along with the model (`Model.swift`) and Bonjour discovery (`Discovery.swift`). The few differences are behind `#if os(...)`; after a change to KyberKit, build the Apple TV app too.

## What the app does

- **Home**: the image of the computer shown full frame (very slow zoom, desaturated when offline), its name in large, its system, its graphics card and its address, the virtual display waiting for it, the main button and the profile; on the right, the latency of the last session (median, 95th percentile, duration) on the frame ruler; at the bottom, the other computers with their image (one click shows them), those announced on the network (Bonjour, they light up as they appear) and the add button. Online or not: the console is polled every 8 s.
- **A computer's image** (`HostImagery`), in this order: the last image of its last session, photographed at the end (`KyberClient.snapshot`, 1600 pixels wide, JPEG in `~/Library/Application Support/KyberNow/Images`), can be turned off per computer (“Keep the last image”, which deletes it); otherwise an up-to-date preview requested from its console (`GET /api/stream/preview`, at most once a minute, arriving with a 45° sweep); otherwise its **signature**, two glows whose hue and position are drawn from its console's fingerprint, with the huge symbol as a watermark. The home screen says which one it shows (“Image shown”).
- **Adding a computer**: on the local network (the console's Bonjour announcement `_kybernow._tcp`; those marked `kyber=0`, a console with no Kyber server behind it, are not offered), by the console's address and port, or by QR code (the Mac's camera) or the `kybernow://adresse:port?name=…&fp=sha256:…` link shown in the console, on the Access page. The console's certificate fingerprint is verified: automatically if the link or the announcement gives it, otherwise by eye (the first four groups). Then the account, verified with the console and kept in the Keychain. A `kybernow://` link opened elsewhere opens the add flow directly.
- **Settings per computer**: Game, Desktop and Max profiles (changing a setting makes the profile “Custom”); the computer's display (like this display, if needed “4K at most”, window size, custom, or its displays as they are); format (H.264, HEVC, AV1), 4:4:4, vertical sync, bitrate from 5 to 4,294 Mbit/s with warnings; transport (fast or reliable), reconnection; ⌘ sent as Ctrl, games mouse, gamepad seen by the computer (Xbox 360 or faithful), shared clipboard, sound (Auto, stereo, 5.1, 7.1, none); “This Mac” card.
- **Bitrate**: a profile's bitrate is set for 4K at 60 Hz and follows the session's number of pixels per second (power 0.75, between 0.4 and 3 times). For example, the Max profile at 300 Mbit/s starts at about 460 Mbit/s in 5K at 60 Hz. A bitrate set by hand stays as it is. The settings show the real bitrate.
- **Pointer**: the Mac's pointer takes the shape of the computer's, at the size of a Mac pointer. The Windows cell (32 pixels at 100%, 48 at 150%, 64 at 200%) is brought down to 32 points, and the hotspot follows the same scale. A hide, or a fully transparent shape (video players), never makes it disappear: it keeps the last shape.
- **Nothing stays pressed**: when the window loses the keyboard (⌘Tab, ⌘Space, Spotlight, another app in front), the app releases on the computer what it holds there (keys, modifiers, mouse buttons) and forgets its own state. No kynput `flush_keys` here: it would also release the gamepads, which, read by SDL, keep talking to the computer without the window and would not send their state again (it is still sent at the end of the session). When the window gets the keyboard back, it sends again the modifiers really held at that moment (`NSEvent.modifierFlags`, ⌘ still down after a ⌘Tab); a released modifier is released on the computer whichever side (left or right) held it. Caps Lock: sent as press + release, like a keystroke, never held.
- **Refused packets** (`InputPipeline`): kynput's send queue holds 64 packets and drops without notice, while the protocol is made of deltas (a dropped release = a key stuck until the end of the session). The app counts the refusals (log `entrées : … refusé`, then the total at the end of the session). Packets that cannot catch up on their own (keys, clicks, buttons, gamepad plugs and unplugs, an axis back to rest, text, `flush_keys`) are retried every 3 ms from a small fallback queue, in order, for up to 200 ms; the others (positions, moves, wheel, moving axes) are sent once, and not at all while the fallback queue is not empty (they would overtake what is waiting: an axis back to rest, then its new value). The releases `GamepadGate` injects on capture re-arm their allowance on every attempt: the hook consumes it before the send queue decides, and a packet it discards counts as sent. Gamepads read by SDL do not go through it (kynput sends them itself): `GamepadGate` only discards axes that have not moved (by 1/512 of the travel), which quiets a chatty Bluetooth gamepad.
- **Apple ISO keyboard** (`Keymap`): on a European Apple keyboard (one more key next to left ⇧), the Mac reports the key below Escape (`@#` on an AZERTY, `§±` elsewhere) as `kVK_ISO_Section`, HID usage 0x64, and the one next to ⇧ (`<>`) as `kVK_ANSI_Grave`, usage 0x35: the opposite of what the computer expects (`@` gave `<`, confirmed on 2026-10-20 on an AZERTY MacBook; `hid-apple` quirk, SDL does the same swap). Both usages are swapped when the keyboard type of the event (`kCGKeyboardEventKeyboardType`, then `KBGetLayoutType`) is ISO; an ANSI keyboard plugged in at the same time keeps its keys.
- **Mouse: Auto, Desktop or Game** (settings, session bar, ⌃⌥⌘M to switch between desktop and game).
  - In game mode, the computer's pointer, when it is visible (menus, inventories), is drawn by the app at the position the computer announces (server with `kyber-patches/0006`, 60 positions per second). The shape is only rebuilt if it changes.
  - In game mode, the Mac's pointer is frozen and hidden, and only raw movements are sent (GCMouse, without macOS acceleration; AppKit movements without GCMouse). The mouse becomes absolute again, pointer put back in the same place, when the bar opens or when the window loses focus.
  - Auto (the default) switches to game mode when a game is in full screen on the computer and hides its pointer. The console says so (`GET /api/stream/foreground`, queried every second): foreground window covering the whole screen, or DirectX exclusive full screen. Browsers, video players, slideshows and video calls in full screen do not count. A pointer shown again for less than 0.6 s (menus, Windows flicker) does not make it leave game mode.
  - The pointer being hidden is not enough on its own to trigger game mode: on a computer with no mouse plugged in, Windows hides it and shows it again every half-second, even on the desktop.
- **Introduction to the console**: at the start of a session, the app gives the console a stable identifier (drawn at first launch), the Mac's name, its system, its version and the profile; then its median latency with each heartbeat, and that of the whole session at the end. The console builds its known devices and its event feed from this. If the console asks for the session to end (“End the session”, in the console or the computer's icon), the app learns it at the next heartbeat and disconnects as with Quit.
- **Shared clipboard**: text, both ways, up to 60 KB (`ClipboardBridge`, kynput protocol, C functions of `kyber-patches/0002`). What the computer announces is requested immediately, so that ⌘V works straight away; what the Mac copies is announced (checked twice a second) and sent on request. The console can forbid it in one direction or the other.
- **iCloud for the Apple TV** (`CloudPublisher`): each time a computer is added, changed or removed, the app publishes the list to the iCloud key-value store (key `hosts`: name, address, console port, account, stream port, fingerprint, and `from` = this Mac's name) and copies the password to the iCloud Keychain. The Apple TV app on the same account adds them without typing anything. Computers published by another Mac stay in the list. “Synchroniser avec iCloud” setting in KyberNow › Settings… (⌘,), on by default; turning it off removes this Mac's computers from the list and from the iCloud Keychain (the Apple TV keeps those it already added). It needs the iCloud entitlements, hence a provisioning profile, in place for Debug and Developer ID builds alike (see “Signing and iCloud”).
- **Updates**: Sparkle, daily check and the KyberNow › Check for Updates… menu; `appcast.xml` feed signed with EdDSA, published with each release on GitHub (see [docs/releases.en.md](../../docs/releases.en.md)).
- **Choosing the display**: with the virtual display, the app waits at most 6 s, only once, for Kyber to announce only that display, then takes the display at the requested size placed at (0, 0). Outputs that Windows lists as 0 × 0 (off) are left out; if no usable display remains, the session says so instead of staying on "First frames".
- **Gamepads**: those SDL handles (Xbox, DualSense, DualShock…). Per-computer setting “Manette vue par l'ordinateur” (gamepad seen by the computer): **Xbox 360** (default), they all become Xbox 360 gamepads on the computer, the ones every game recognises; or **Fidèle** (faithful), a Sony gamepad stays a DualShock 4 (touchpad, light bar for the games that use them), the others stay Xbox 360. The choice goes with the session (`gamepad_force_xbox360`) and overrides the console's “all gamepads as Xbox 360” option, which only remains a default for other clients.
- **Gamepad combination** (L1 + R1 + Start + Select): the bar opens and the gamepad drives it (d-pad, A, B); same combination or bar closed, it goes back to the computer. The session controller is what takes the gamepad, not the layer that reads the packets (`GamepadGate`, which only reports the combination): outside a session (connecting, reconnecting, ending), the combination does nothing and the gamepad stays with the computer, instead of going silent until the next combination. When the bar opens, everything is reset on the computer: held buttons released (trigger included) and the six axes at rest, for every gamepad the packet layer has seen; those releases are the only gamepad packets that leave while the bar holds the gamepad. When the picture restarts (profile change from the bar), the bar keeps the gamepad.
- **Reconnection**: during a Kyber reconnection, the app waits: it is the app that restarts the image once Kyber is back. If Kyber restarted on the computer, the virtual display may have gone with it (it always keeps the same number, but the console removes it and turns the physical displays back on). So the app asks the console for it again (`/api/stream/prepare`, no effect if it is still there), waits at most 3 s until Kyber announces only that display, then restarts the image on it. If the computer still refuses the image within 15 s, it makes this request once more instead of cutting the session. The heartbeat also says whether the virtual display is in place (`active`): if it disappeared during the session, the app asks for it again (at most once every 30 s).
- **Panels over the image**: the measurements panel, which stays open, has an opaque background. Frosted glass (`ultraThinMaterial`) would be recomputed by macOS at each frame of the stream (160 per second in 4K) and makes the frame rate drop. The bar, opened briefly, keeps its glass. Starting an image always stops the previous one, input chain included: two chains were fighting over the gamepad (SDL only serves one), and the old one, unplugged, rejected everything.
- **Transition and session**: the session plays in the home window. The home screen is photographed and cut in two along the 45° seam; a neon tube lights up in it during preparation (capsule: preparing, connecting, first frames, Cancel), then the two halves move apart onto the computer's image and the window goes full screen (`Passage.swift`, Core Animation). For the duration of the session, a window delegate hides the menu bar and the Dock in full screen and passes the rest on to SwiftUI's. Bar when hovering at the top of the screen, in glass: lit symbol, live latency and its curve, resolution and format, profile, sound, mouse, clipboard, the computer's display (without a virtual display), full screen, Quit. Clean end, checking that the computer got its displays and its sound back; the halves close on the last image, then the home screen comes back.
- **v2 guidelines** (`Theme.swift`): ink and frost, Spectre signal `#9A8CFF` that only lights up as neon in active moments (standby on hover, flash on click, lit during a connection or a live measurement, ignition on appearance), turned off with “Reduce transparency”; variable Archivo (width 125 for titles) and Martian Mono; Phosphor icons in `Assets.xcassets/Icons` (`scripts/phosphor-icons.mjs`); app icon in Icon Composer format (`KyberNow/AppIcon.icon`: the two halves of the symbol in “Liquid Glass”, the seam's neon on top, ink background), produced from the `brand/v2` geometry by `scripts/app-icon.py` and editable in Icon Composer.

| Shortcut | Effect |
|---|---|
| ⌃⌥⌘D | Disconnect |
| ⌃⌥⌘F | Full screen or window |
| ⌃⌥⌘M | Mouse in game mode (raw movements) or desktop mode |
| ⌃⌥⌘S | Detailed measurements (also: click the measurements in the bar) |
| ⌘ + key | Ctrl + key on the computer (configurable) |
| L1 + R1 + START + SELECT | The gamepad drives the app's bar (D-pad, A, B); same combination or bar closed: it goes back to the computer |

⌃⌥⌘ + letter stays on the Mac (the app's menus) and is never sent to the computer.

## Image and sound

Kyber normally draws the image and plays the sound with VLC. KyberNow decodes them itself (H.264, HEVC, AV1, Opus) for latency and sync: `kyber-patches/0001` adds to libkyclient an output of the packets to the app (see [docs/kyber-changes.en.md](../../docs/kyber-changes.en.md)).

**End-to-end latency and the path of a frame** (`LatencyTracker`)
- Kyber's metrics turned on: the server timestamps each frame at each step (capture, start and end of encoding, sending), and Kyber's pings give the clock offset (median of the last 15).
- The renderer reports the moment each frame is received, decoded and shown, brought onto Kyber's `CLOCK_MONOTONIC` clock.
- Latency = display − (capture − offset), median of each second, in the bar. The session's median goes on the home screen.
- Steps, medians of each second: capture → encoding, encoding, sending, network (sending → complete frame received), decoding, display (decoded → on screen, sync included).
- Losses: a frame sent by the computer that is neither shown nor replaced by a more recent one within the second is lost (missing packets, or discarded while waiting for a keyframe). A frame the display drops in favour of a more recent one (160 fps stream on a 120 Hz display) counts as replaced. Added to these are gaps in the stream, keyframes requested again, and packets lost according to Kyber (`NETWORK_LOCAL` / `NETWORK_REMOTE` totals, every few seconds, with the network round trip).

**Image** (`VideoDecoder`, `VideoRenderer`, `VideoPipeline`)
- VideoToolbox, hardware decoder required (software as a last resort), real-time mode, one frame per call, no queue.
- Output in the Apple silicon lossless compressed formats (`&8v0`, `&8v4`), read without copying by Metal (`CVMetalTextureCache`). YCbCr → RGB conversion in the shader, according to the matrix announced by the stream (709, 601, 2020) and its range.
- Display: always the most recent frame. Each frame leaves for the display as soon as it is decoded; Core Animation shows the most recent one at the refresh and discards the others (counted as replaced). The old “one frame in flight” guard (frame N+1 waited until the display had shown N: a whole refresh of latency, 8 ms at 120 Hz) remains available with `KN_INFLIGHT_GUARD=1`. On a variable refresh rate display (ProMotion, Adaptive-Sync), the display follows the arrival of frames. Vertical sync can be turned off: lowest latency, possible tearing.
- Recovery after a loss: when packets are lost beyond what error correction can repair, VideoToolbox refuses the frames that rely on the lost reference. With the encoder's progressive refresh, no keyframe arrives by itself. The app then keeps the last image and stops sending unusable frames to the decoder. It requests a keyframe from the computer (`kyclient_client_request_keyframe`, server with patch 0005), repeats the request every 300 ms (timer in “common” mode, which also fires while a menu is open) and starts again from the keyframe. With no answer within 1.5 s (computer not up to date), it restarts the stream, which always begins with a keyframe. Inside Kyber the request runs off the main loop, one at a time and given up after one second: a computer that stops answering delays neither the stop nor the reconnection.
- Decoder lifetime: the video pipeline and the audio player handed to Kyber as `userdata` are retained (`PacketSinkLease`) until Kyber's last call (`CLOSED`, patch `kyber-patches/0001`), or 30 s after the stop if the library does not send it. Before, they were released 2 s after the stop while Kyber could still deliver a packet (possible crash on disconnection).
- Initial keyframe lost: a heavy stream (4K at a high refresh rate in 4:4:4) can lose its whole first keyframe, with no gap reported by Kyber. The following frames arrive without a reference: the decoder then requests a keyframe as after a loss, instead of waiting silently.
- Fallback: if no frame is shown 4 s after the stream starts, or if keyframe requests go unanswered before the first frame, the app lightens the stream for this session only (bitrate brought down to 250 Mb/s, then 4:2:0), says so at the top of the screen and notes it in the log (`flux : …`, `repli …`). Changing profile in the bar starts again from the chosen setting.
- Measuring the display: `KN_PRESENT_STATS=1 /Applications/KyberNow.app/Contents/MacOS/KyberNow` writes a `présentation : …` line to the log (category `video`) every second with the frames rendered, those the display confirms it showed, those Core Animation discarded in favour of a newer one (`presentedTime == 0`), and the commit → screen delay (median and 95th percentile), plus the state of vertical sync and of the guard. `KN_INFLIGHT_GUARD=1` puts back the “one frame in flight” guard (frame N+1 waits until the display has shown N) for comparison. What to read: without the guard, the commit → screen delay should fall towards half a refresh interval (≈ 4 ms at 120 Hz) and discarded frames appear as soon as the stream exceeds the display's refresh rate, which is expected; with the guard, the delay is a whole interval. `MTL_HUD_ENABLED=1` (same command) shows the Metal HUD over the image: Metal does not say publicly whether presentation is direct (the image goes to the display without the compositor) or composited, only the HUD shows it.
- AV1 (M3 chips and later): the sequence header is read bit by bit to build the `av1C` description VideoToolbox expects. Samples are stripped of temporal delimiters, and each OBU carries its size (`AV1.swift`). Measured on an M4 Max at 2560 × 1440: 1 ms decoding. VLC is now only a fallback (AV1 on a Mac without a hardware AV1 decoder, which the settings do not offer).

**Sound** (`AudioPlayer`)
- libopus (multistream: stereo, 5.1, 7.1), loss concealment by Opus.
- Core Audio output (`DefaultOutput` on the Mac, `RemoteIO` on iOS/tvOS), 5 ms requests to the sound card.
- Buffer of about twenty milliseconds, against a fixed 50 ms with VLC: its margin grows after a gap, goes down by one millisecond every ten seconds without a gap, and a lasting excess is dropped (drift between the computer's and the Mac's clocks).
- Sound and image each leave as soon as they are ready. The bar shows the offset (sound = buffer + output latency, image = reception → screen), imperceptible below 45 ms. Bluetooth headphones add 150 to 250 ms on the sound side.

**Machine capabilities** (`DecoderCapabilities`, `DisplayInfo`)
- At launch: H.264, HEVC and AV1 hardware decoders, plus a real decode of a 64 × 64 HEVC 4:4:4 frame, requiring hardware.
- The profiles are brought down to them: without hardware 4:4:4, 4:2:0 with a third more bitrate; without HEVC, H.264 with half as much again. The settings grey out what the machine cannot decode.
- Display: the panel's real pixels, taken from its display modes (native mode), up to 7680 × 4320, and 4096 wide at most in H.264 (NVENC limit). In “more space” mode, macOS draws larger than the panel then scales down: a 4K display set to 3008 × 1692 points draws 6016 × 3384, and those extra pixels are not requested from the computer. “4K at most” option, maximum refresh rate, variable refresh rate or not.

Live measurements (session bar): end-to-end latency and its curve, resolution and format. A click on them (or ⌃⌥⌘S) opens the **detailed measurements** at the bottom right (`SessionStats.swift`): the path of a frame as a cascade on a common scale, with the duration of one frame as a dotted line; latency (losses marked under the curve) and bitrate over 30 s; losses over 30 s and since the start; network round trip, image, sound and the sound-image offset. The panel stays open and lets clicks and movements through to the computer, except its close button. In Debug, `KN_STATS_PREVIEW=1` shows it filled with plausible measurements, without a session.

## Code layout

```
apps/macos/
├── project.yml            # projet XcodeGen (cibles KyberKit et KyberNow)
├── KyberKit/              # framework : façade Swift du SDK Kyber
│   ├── KyberClient        # connexion, session, événements, options
│   ├── ConsoleClient      # API de la console (prepare, heartbeat, end, info), empreinte épinglée
│   ├── VideoDecoder / VideoRenderer / VideoPipeline / DecoderCapabilities
│   ├── AudioPlayer
│   ├── LatencyTracker     # latence de bout en bout (mesures de Kyber)
│   ├── StreamView         # vue de session : clavier, souris, curseur
│   ├── MetalSurface       # vue qui porte la couche Metal du rendu
│   ├── RelativeMouse      # souris des jeux (GCMouse)
│   ├── ClipboardBridge    # presse-papiers partagé
│   ├── InputPipeline, Keymap, GamepadGate   # entrées via libkynput
│   └── KyberLog           # journaux vers le journal unifié
├── KyberNow/              # app SwiftUI + AppKit
│   ├── KyberNowApp, Model (ordinateurs, réglages, Trousseau)
│   ├── CloudPublisher     # computers published to iCloud for the Apple TV
│   ├── HomeView, HostImagery, AddHostFlow, SettingsView, AppSettingsView
│   ├── SessionController, SessionChrome, SessionStats, Passage
│   └── Theme, DisplayInfo, Fonts/ (Archivo, Martian Mono, OFL)
├── kyber-patches/         # correctifs KyberNow sur libkyclient et libkynput
├── scripts/               # vendor-kyber.sh, build-kyclient.sh, embed-kyber.sh
├── tests/                 # tests hors ligne (vidéo, son)
└── Vendor/                # SDK Kyber compilé (ignoré par git)
```

In English: `project.yml` is the XcodeGen project (KyberKit and KyberNow targets). `KyberKit/` is the framework, a Swift facade over the Kyber SDK: `KyberClient` (connection, session, events, options), `ConsoleClient` (console API: prepare, heartbeat, end, info; pinned fingerprint), the video classes, `AudioPlayer`, `LatencyTracker` (end-to-end latency from Kyber's metrics), `StreamView` (session view: keyboard, mouse, cursor), `MetalSurface` (view that holds the renderer's Metal layer), `RelativeMouse` (games mouse, GCMouse), `ClipboardBridge` (shared clipboard), `InputPipeline`, `Keymap`, `GamepadGate` (inputs through libkynput) and `KyberLog` (logs to the unified log). `KyberNow/` is the SwiftUI + AppKit app (`Model`: computers, settings, Keychain). `kyber-patches/` holds the KyberNow patches to libkyclient and libkynput, `tests/` the offline tests (video, sound), and `Vendor/` the built Kyber SDK (ignored by git).

Flow of a session: `ConsoleClient.prepare` (virtual display at this Mac's size, sound with the right number of channels) → `KyberClient.connect` → wait for the display list to settle → `startStreaming` → heartbeat every few seconds → `ConsoleClient.end` (the computer gets its displays and its audio output back).

## Building

Prerequisites: Xcode 27, XcodeGen, Rust 1.89 + `cargo-c`, the Kyber 0.29 client built in `third_party/kyber-desktop` (see [docs/build-notes.en.md](../../docs/build-notes.en.md)).

```bash
apps/macos/scripts/vendor-kyber.sh      # SDK Kyber (en-têtes, bibliothèques, plugins VLC) → Vendor/
apps/macos/scripts/build-kyclient.sh    # libkyclient et libkynput avec les correctifs KyberNow → Vendor/
cd apps/macos && xcodegen generate
xcodebuild -project KyberNow.xcodeproj -scheme KyberNow -configuration Debug -derivedDataPath build build
open build/Build/Products/Debug/KyberNow.app
```

(`vendor-kyber.sh` copies the Kyber SDK: headers, libraries, VLC plugins; `build-kyclient.sh` builds libkyclient and libkynput with the KyberNow patches.)

Run `build-kyclient.sh` again after each `vendor-kyber.sh`: without it, the app picks up libraries without the patches. When `Vendor/` changes, quit the app before rebuilding: VLC would crash while loading modules rewritten under its feet (see [build-notes](../../docs/build-notes.en.md)). A rebuild without a change to `Vendor/` no longer touches the app's libraries. Signing: “Apple Development: info@leslignesclaires.fr” certificate, team P6DB682S4K (`project.yml`). It stays the same from one build to the next, which keeps the Keychain from asking again for access to the password; plan a Developer ID for distribution.

### Signing and iCloud

Publishing for the Apple TV requires two entitlements, `com.apple.developer.ubiquity-kvstore-identifier` (`$(TeamIdentifierPrefix)fr.leslignesclaires.kybernow`, the Apple TV app's key-value store) and `keychain-access-groups` (`$(AppIdentifierPrefix)fr.leslignesclaires.kybernow`, data protection keychain). They live in `KyberNow/KyberNow.entitlements`, along with the camera. On macOS these entitlements need an embedded provisioning profile, otherwise the app is killed at launch: in Debug, the “macOS App Development” profile `fr.leslignesclaires.kybernow` (this Mac, info@leslignesclaires.fr certificate); in Release, the “Developer ID” profile `P6DB682S4K.fr.leslignesclaires.kybernow` (all Macs, valid until 2044). Both must be installed on the Mac that builds, otherwise Xcode refuses to sign. The app also checks the entitlements at runtime (`CloudPublisher.entitled`): signed without them, it publishes nothing and says so.

Setup (done on 2026-10-20), to redo for a new build Mac or an expired profile, on [developer.apple.com](https://developer.apple.com/account/resources):

1. App ID `fr.leslignesclaires.kybernow`: the iCloud capability is already there (created for the Apple TV, key-value store included); nothing more to tick, the Keychain group `P6DB682S4K.fr.leslignesclaires.kybernow` comes from the identifier itself.
2. Devices: register this Mac (Provisioning UDID in System Information › Hardware; or let Xcode do it from Settings › Accounts).
3. Profiles: “macOS App Development” for `fr.leslignesclaires.kybernow`, “Apple Development: info@leslignesclaires.fr” certificate, this Mac; “Developer ID” (Mac) for the same identifier, Developer ID Application certificate. For Xcode to find them, copy each downloaded file to `~/Library/Developer/Xcode/UserData/Provisioning Profiles/<UUID>.provisionprofile` (double-clicking is not enough on macOS).
4. Their names go in `project.yml` (`PROVISIONING_PROFILE_SPECIFIER`, per configuration), then `xcodegen generate`.

Xcode puts the profile in the app (`Contents/embedded.provisionprofile`) and adds the App ID and team to its entitlements. `scripts/sign-release.sh` re-signs the app with the entitlements Xcode gave it (no longer with the `.entitlements` file): the Developer ID release keeps the profile and its entitlements through notarisation. The local Keychain item (service `fr.leslignesclaires.kybernow`) does not change: the signature stays the same, and so does access to the password.

## Testing without a remote computer

```bash
apps/macos/tests/video/run.sh   # flux HEVC, H.264 et AV1 générés par ffmpeg : décodage, temps, couleurs rendues, reprise après une perte
apps/macos/tests/audio/run.sh   # Opus stéréo et 5.1 au rythme réel, à-coups et pertes : tampon, trous
apps/macos/tests/clipboard/run.sh   # presse-papiers privé : annonces, demandes, réponses, 60 Ko
apps/macos/tests/screen/run.sh  # fenêtre de 2 × 2,5 s : la surface de session montre vraiment les images (presentedTime), délai commit → écran avec et sans la garde « une image en vol »
```

(Video: HEVC, H.264 and AV1 streams generated by ffmpeg, checking decoding, timing, rendered colours and recovery after a loss. Audio: stereo and 5.1 Opus at real-time pace, with jitter and losses, checking the buffer and gaps. Clipboard: private clipboard, announcements, requests, answers, 60 KB. Screen: two 2.5 s passes in a window, checking that the session surface really shows the frames, `presentedTime`, and comparing the commit → screen delay with and without the “one frame in flight” guard.)

Measured on an M4 Max: 1 to 2 ms decoding at 2560 × 1440 (4:2:0), 1.6 to 2.7 ms in 4:4:4, colours within ±4 levels; sound: 15 to 23 ms of buffer with 20 ms packets, 14 to 22 ms with 10 ms packets (server with 0004), with ± 8 ms of simulated jitter.

## Logs

Console.app, subsystem `fr.leslignesclaires.kybernow`, categories `app`, `kyber` (SDK), `video`, `audio`. From the command line:

```bash
log stream --predicate 'subsystem == "fr.leslignesclaires.kybernow"' --level info
log stream --predicate 'subsystem == "fr.leslignesclaires.kybernow" AND category == "video" AND eventMessage BEGINSWITH "présentation"'   # with KN_PRESENT_STATS=1
```

## Data

- Computers and settings: `UserDefaults` (key `hosts`), read back tolerantly (a newly added setting keeps its default value).
- Computers' images: `~/Library/Application Support/KyberNow/Images/<id>-last.jpg` (last session) and `<id>-preview.jpg` (the console's preview), deleted with the computer.
- Passwords: Keychain, service `fr.leslignesclaires.kybernow`, one item per computer.
- iCloud (sync on, app signed with the entitlements): key-value store, key `hosts` (JSON array of `CloudHost`, format shared with `apps/tvos/KyberNowTV/CloudSync.swift`); passwords in the iCloud Keychain, service `fr.leslignesclaires.kybernow.cloud`, account = the computer's identifier. App settings: `UserDefaults` `icloudSync` (sync) and `icloudPublished` (computers published by this Mac).

## Towards iOS and Apple TV

Reusable as they are: `VideoDecoder`, `VideoRenderer`, `VideoPipeline`, `DecoderCapabilities`, `AudioPlayer`, `LatencyTracker`, `ConsoleClient`, `GamepadGate`. To adapt: `ClipboardBridge` (UIPasteboard instead of NSPasteboard). To write: a UIKit view that holds the Metal layer (equivalent of `MetalSurface`), touch and remote inputs, and building libkyclient, libkynput and libopus for iOS and tvOS (`kysdk/kyctl/build-ios.sh` is a starting point). There is neither VRR nor turning off sync on Apple TV.
