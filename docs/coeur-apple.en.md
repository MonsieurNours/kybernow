# The Rust core in the Apple apps

English · [Français](coeur-apple.md)

The Mac, Apple TV, iPhone and iPad apps rest on the Rust client core (`apps/core`, crate `kybernow-core`), the one the Windows app uses. Everything that does not depend on display lives there: the session (talking to Admin, Kyber, reconnection, keyframe, display, latency, profiles), the inputs and the sound. Decoding (VideoToolbox), rendering (Metal), the views and the system APIs stay in Swift. One code for four apps.

## What goes where

```
Swift (KyberKit, apps)                      Rust (kybernow-core, through kybernow-apple)
──────────────────────                      ──────────────────────────────────────────
VideoPipeline, VideoDecoder, VideoRenderer  session: preparation, connection, display
(VideoToolbox, Metal), AV1                  choice, starting and relaunching the picture,
AudioPlayerCore: Core Audio output          reconnection, virtual display requested again,
InputCore: event capture into the           keyframe (300 ms, 1.5 s), fallback without a
core's input chain                          picture (bitrate, 4:2:0), size of the
StreamView, MetalSurface, Passage,          computer's display, mode policy (“Restore”),
SessionChrome, SessionStats, HomeView…      adapting to the device's screen, heartbeat, end
SessionController: the window, the bar,     console: info, preview, pairing, fingerprint
the capsules, full screen, the Mac's        latency: path of a frame, losses, holes,
screen, the Keychain, the last image        rejected measurements, session median and p95
SessionEngine, ConsoleClientCore,           inputs: retry queue, keyboard, gamepads,
CoreBridge, LatencyReporting: the core      pointer, release all, clipboard
as seen from Swift                          sound: Opus, adaptive buffer, drift, downmix
                                            profiles and bitrate, language, updates
```

The core knows nothing of display: the platform hands it a `Platform` (window, where to send picture and sound packets, the kynput input chain, state callbacks). On the Mac, Apple TV, iPhone and iPad, `SessionEngine.swift` (shared KyberKit) provides it, through the C API.

The Windows app uses the same session, the same input module and the same sound engine; its clipboard stays kynput's. What the session does for the Apple apps (path of a frame and losses in the latency, keyframe request repeated every 300 ms then the stream relaunched at 1.5 s, fallback when no picture comes within 4 s — bitrate brought down to 250 Mbit/s, then 4:2:0, for this session only —, tracking of the computer's display size, mode policy with “Restore”, adapting to a new mode of the device's screen, choosing a display from the bar, bitrate cap, end report) can be switched off or has no effect for Windows, which keeps its own keyframe and first-picture watches.

## Each app

KyberKit is a single module, built by all three Xcode projects, core included: `SessionEngine`, `ConsoleClientCore`, `CoreBridge`, `InputCore`, `AudioPlayerCore` and `LatencyReporting` serve everywhere. Each platform keeps only its session view, its Metal surface and the capture of its events.

- **Mac** (`apps/macos`): AppKit, `StreamView`, `MetalSurface`, `RelativeMouse`, `KeymapMac` (kVK → HID table, ISO layout); gamepads go through SDL in kynput, and the combination is followed in the core's input chain (`gamepad_gate.rs`). Clipboard through `NSPasteboard`.
- **Apple TV** (`apps/tvos`): KyberKit's shared files without the AppKit ones, plus `KyberKitTV`: UIKit session view, Metal surface, `GamepadBridge` and `KeyboardMouse` (GameController).
- **iPhone and iPad** (`apps/ios`): the same shared files, the Apple TV's Metal surface, `KeyboardMouse` and `GamepadBridge`, and the touch session view `KyberKitIOS/StreamView`. Clipboard through `UIPasteboard`, read only when the computer asks for it (`pasteboard_reading_asks_user`). See [apps/ios/README.en.md](../apps/ios/README.en.md).

On Apple TV, iPhone and iPad, `GamepadBridge` follows the combination itself before sending (menu, gamepad as mouse, long press): it keeps a `PadCombo` held by the core (`kn_combo_*`, with its own thread for the 250 ms deadline); the input chain has its combination turned off. The pointer held by the device (remote, gamepad, mouse, touch) is the position of the core's `PointerLayer` (`holds_position`).

The sound, input and clipboard tests are the core's: `cargo test -p kybernow-core` (see [apps/core/README.en.md](../apps/core/README.en.md)).

## The bridge: `apps/apple`, crate `kybernow-apple`

A hand-written C API (`kn_` prefix), `kybernow.h` header produced by cbindgen through `cargo cinstall` (cargo-c). Swift imports it through the `CKyberNow` module (`Vendor/KyberNow/include/module.modulemap`).

API rules:

- **Opaque objects** (`KnConsole`, `KnSession`, `KnInput`, `KnPointer`, `KnAudio`, `KnCombo`) created and freed by the core; no Rust structure is exposed.
- **Composite results as JSON** (`char *`, freed by `kn_string_free`): Admin's replies, a session's settings. Each second's measurements (`KnSecond`) and packets (`KnPacket`) are flat C structs: hot path, no copy, no allocation.
- **Callbacks** (`KnPlatform`): function pointers and a `userdata`; the core calls them from its own threads (session thread, Kyber's network thread for packets). Swift hops to the main thread for what touches the interface; packets go straight to the decoder.
- **Packets**: `video_packet` and `audio_packet` receive a `KnPacket` (format, configuration or frame, keyframe, pts, data in place, hole with the number of missing samples); the last call of a stream carries `KN_PACKET_CLOSED`, after which the core never calls again for that stream: Swift then stops its decoder or its sound output.
- **Input chain**: `input_pipeline()` returns the kynput chain of a `KnInput` (`kn_input_new`, on the view's `KnPointer` pointer layer, `kn_input_kynput`); the core passes it to Kyber with the picture session. Swift only hands over the captured events (`kn_input_key`, `mouse`, `mouse_button`, `wheel`, `type_text`, `gamepad_*`, held and resynced modifiers) and draws the pointer (`on_changed` of `KnPointerCallbacks`, shape in premultiplied ARGB); interface callbacks (combination, navigation keys, inputs connected, rumble, clipboard) go through `KnInputCallbacks`.
- **Sound**: one `KnAudio` per stream (`kn_audio_new`, the output's channel order) receives the packets (`configure`, `decode`, `conceal`); the output's real-time thread pulls samples with `kn_audio_fill` and measurements with `kn_audio_take_stats`.
- **Latency**: the Metal renderer tells the core each frame shown (`kn_session_frame_displayed`, instants in µs on `CLOCK_MONOTONIC`, `kn_now_micros`), replaced, the bytes received and the holes; every second the core returns a `KnSecond` (median, path of a frame, losses, rejected measurements, round trip, bitrate) through `on_second`.
- **States** (`on_status`): preparing, connecting, first images, picture, reconnecting (attempt, maximum), ending, ended (with the reason when it was not wanted, and Admin's report: displays and sound restored).
- **Notices** (`on_notice`): what the session shows in a capsule, without text (Swift translates it): picture relaunched, computer's mode changed or back (with the size), fallback (level), adapting to the screen (requested, done, refused), “Restore” offered or withdrawn.
- **Commands**: stop, keyframe lost or back, new settings (profile from the bar), bitrate cap (occluded window), chosen display, new mode (adapting to the device's screen), mode restore.
- **Log**: the core writes through the `log` crate; `kn_set_log_callback` sends it to `KyberLog` (unified log, `core` category).

## Building and shipping

The core builds the `kyclient` Rust crate from Kyber's sources (`third_party/kyber-desktop/kysdk`), with the patches in `apps/macos/kyber-patches` (see [kyber-changes.en.md](kyber-changes.en.md)).

**Mac**: dynamic library `libkybernow.dylib`, in `apps/macos/Vendor/Frameworks`, embedded and signed by `embed-kyber.sh` like the other libraries.

```bash
apps/macos/scripts/vendor-kyber.sh      # once: Kyber SDK → Vendor/
apps/macos/scripts/build-kyclient.sh    # libkynput and libkyclient with the patches → Vendor/
apps/apple/scripts/build-apple.sh       # kybernow-core + kybernow-apple → Vendor/Frameworks/libkybernow.dylib, Vendor/KyberNow/include/kybernow.h
cd apps/macos && xcodegen generate && xcodebuild -scheme KyberNow -configuration Debug -derivedDataPath build build
```

`build-apple.sh` builds for `aarch64-apple-darwin` with the repository's stable Rust and refuses to run while the app is open. The bridge links libkynput and libvlc from the Kyber SDK (input chain; player never called).

**Apple TV and iPhone/iPad**: static library in `apps/<platform>/Vendor/KyberNow.xcframework` (device and simulator), from `apps/apple/scripts/build-apple-mobile.sh tvos|ios`, deployment 18.0.

- Apple TV: Rust nightly and `-Zbuild-std` (tier 3 target). libkynput, libopus and VLC's empty functions come from Kyber's SDK for tvOS (`Vendor/KyberSDK.xcframework`, from `apps/tvos/scripts/build-kyber-tvos.sh`), linked alongside.
- iPhone and iPad: stable Rust. No separate Kyber SDK: libkynput (built by `apps/core/scripts/build-apple.sh --target aarch64-apple-ios` and `--target aarch64-apple-ios-sim`), libopus and VLC's empty functions go into the same library as the core; kynput's header goes into `Vendor/include` (`CKyber` module).

`apps/core/scripts/build-apple.sh` checks that the core's dependency chain builds for each Apple target. `scripts/release.sh` builds the core for all three apps; the Apple TV's and the iPhone/iPad's `Vendor/` is published with each release (`KyberSDK-tvOS.zip`, `KyberSDK-iOS.zip`), which the CI and the `sideload.sh` scripts pick up. Details and pitfalls: [build-notes.en.md](build-notes.en.md#the-rust-core-of-the-apple-apps-appsapple).

## Status

- Checked on the hardware: full Mac session, inputs and sound included; session on an iPhone 15 Pro Max (22/10/2026).
- Not tried yet: the Apple TV app (installed on an Apple TV, not launched yet); the iPad (not in the provisioning profile); the external display and Stage Manager; the Windows client with its inputs and sound on the core.
- The iPhone and iPad app is not distributed yet: internal TestFlight to come (App Store profile to be created); installation from a Mac with Xcode in the meantime ([apps/ios/SIDELOAD.en.md](../apps/ios/SIDELOAD.en.md)).
