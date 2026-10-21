# The Apple apps on the Rust core

English · [Français](coeur-apple.md)

Decided on 21/10/2026: the Mac, Apple TV and iPhone/iPad apps rest on the Rust client core (`apps/core`, crate `kybernow-core`), the one the Windows client uses. Everything that does not depend on display moves there: the session (console, Kyber, reconnection, keyframe, display, latency, profiles), then the inputs, then the sound. Only decoding (VideoToolbox), rendering (Metal), the views and the system APIs stay in Swift. One code for four apps, and an iPhone app that starts on the core from day one.

## Milestones

| Milestone | Content | Validated by |
|---|---|---|
| 1 | The C bridge (`apps/apple`) and the core's session on **Mac**: `KyberClient`, `ConsoleClient`, `LatencyTracker` and the `SessionController` state machine replaced by the core; decoder, renderer, sound and inputs unchanged. **Written on 21/10/2026** (Mac Debug, Apple TV simulator and Windows x64 client built, core tests green), merged into main; Mac session **validated on 22/10/2026** (after fixing a core lock taken twice) | A complete session from the Mac to the computer, same latencies as 0.3.0 |
| 2 | The **inputs** in Rust (`apps/core/src/input`): retry queue, keyboard, gamepad combo, pointer, release all, clipboard; adopted by the Mac and by Windows. **Written on 21/10/2026** (bridge `kn_input_*`, `kn_pointer_*`, `kn_keymap_*`; `InputCore.swift`; SDL gamepad hook ported into the core, `gamepad_gate.rs`; Windows without its `input.rs`), in use in the Mac session validated on 22/10/2026; Windows client not tried again since | Offline tests ported, Mac and Windows sessions |
| 3 | The **sound** in Rust (`apps/core/src/audio`): Opus, adaptive buffer, drift, downmix; Core Audio and WASAPI only pull samples. **Written on 21/10/2026** (bridge `kn_audio_*`, `AudioPlayerCore.swift`; Windows without the sound decoding of `audio.rs`), in use in the Mac session validated on 22/10/2026; Windows client not tried again since | Offline tests ported, sound-picture gap unchanged |
| 4 | **Apple TV** on the core (nightly build of the core for tvOS, GameController on the input module). **Written on 21/10/2026** (`build-apple-mobile.sh tvos`, `KyberNow.xcframework`; `SessionEngine`, `InputCore`, `AudioPlayerCore` shared by the Mac and the Apple TV; the gamepad bridge's combination on the core, `kn_combo_*`; the Swift versions of session, console, latency, inputs and sound removed), to be tried on an Apple TV (app installed, not launched yet) | A session on an Apple TV |
| 5 | **iPhone and iPad**, from the mock-ups (https://claude.ai/artifact/4ef5m4JSRnzuitJcuCFjMX), on the core from the start. **Written on 21/10/2026** (`apps/ios`: home, adding, launch, transition, touch session, key row and Keys menu, measurements, settings; core built by `build-apple-mobile.sh ios`; external display on iPhone and iPad, Stage Manager; see apps/ios/README.en.md); session **validated on an iPhone 15 Pro Max on 22/10/2026** (after the display fix: Metal layer created in the window); iPad (not in the provisioning profile), external display and Stage Manager not tried yet | Internal TestFlight (to come) |

The Windows client adopts each module in the same milestone as the Mac: no module exists in two versions.

## What goes where (milestone 1)

```
Swift (KyberKit, KyberNow)                 Rust (kybernow-core, through kybernow-apple)
──────────────────────────                 ──────────────────────────────────────────
VideoPipeline, VideoDecoder, VideoRenderer  session: preparation, connection, display
AudioPlayer (until milestone 3)             choice, starting and relaunching the picture,
InputPipeline, PacketQueue, Keymap,         reconnection, virtual display requested again,
PadCombo, ReleaseAll, PointerLayer,         keyframe (300 ms, 1.5 s), fallback without a
ClipboardBridge (until milestone 2)         picture (bitrate, 4:2:0), size of the
StreamView, MetalSurface, Passage,          computer's display, mode policy (“Restore”),
SessionChrome, SessionStats, HomeView…      adapting to the device's screen, heartbeat, end
SessionController: the window, the bar,     console: info, preview, pairing, fingerprint
the capsules, full screen, the Mac's        latency: path of a frame, losses, holes,
screen, the Keychain, the last image        rejected measurements, session median and p95
                                            profiles and bitrate, language, updates
```

Since milestones 2 to 4, `AudioPlayer`, `InputPipeline`, `PacketQueue`, `Keymap`, `PadCombo`, `ReleaseAll`, `PointerLayer` and `ClipboardBridge` no longer exist in Swift (see below).

The core knows nothing of display: the platform hands it a `Platform` (window, where to send picture and sound packets, the kynput input chain, state callbacks). On the Mac, Apple TV, iPhone and iPad, `SessionEngine.swift` (shared KyberKit) provides it, through the C API.

## The bridge: `apps/apple`, crate `kybernow-apple`

A hand-written C API (`kn_` prefix), `kybernow.h` header produced by cbindgen through `cargo cinstall` (cargo-c, like libkyclient), `libkybernow.dylib` library placed in `apps/macos/Vendor` by `apps/apple/scripts/build-apple.sh`; for the Apple TV and iPhone/iPad, a static library in `apps/<platform>/Vendor/KyberNow.xcframework` from `apps/apple/scripts/build-apple-mobile.sh tvos|ios`. Swift imports it through the `CKyberNow` module (`Vendor/KyberNow/include/module.modulemap`).

API rules:

- **Opaque objects** (`KnConsole`, `KnSession`) created and freed by the core; no Rust structure is exposed.
- **Composite results as JSON** (`char *`, freed by `kn_string_free`): console replies, a session's settings. Each second's measurements (`KnSecond`) and packets (`KnPacket`) are flat C structs: hot path, no copy, no allocation.
- **Callbacks** (`KnPlatform`): function pointers and a `userdata`; the core calls them from its own threads (session thread, Kyber's network thread for packets). Swift hops to the main thread for what touches the interface; packets go straight to the decoder.
- **Packets**: `video_packet` and `audio_packet` receive a `KnPacket` (format, configuration or frame, keyframe, pts, data in place, hole with the number of missing samples); the last call of a stream carries `KN_PACKET_CLOSED`, after which the core never calls again: that is where Swift releases the retained object (`PacketSinkLease`, unchanged).
- **Input chain** (milestone 2): `input_pipeline()` returns the kynput chain of a `KnInput` (`kn_input_new`, on the view's `KnPointer` pointer layer, `kn_input_kynput`); the core passes it to Kyber with the picture session. Swift only hands over the captured events (`kn_input_key`, `mouse`, `mouse_button`, `wheel`, `type_text`, `gamepad_*`, held and resynced modifiers) and draws the pointer (`on_changed` of `KnPointerCallbacks`, shape in premultiplied ARGB); interface callbacks (combination, navigation keys, inputs connected, rumble, clipboard) go through `KnInputCallbacks`.
- **Sound** (milestone 3): one `KnAudio` per stream (`kn_audio_new`, the output's channel order) receives the packets (`configure`, `decode`, `conceal`); the output's real-time thread pulls samples with `kn_audio_fill` and measurements with `kn_audio_take_stats`.
- **Latency**: the Metal renderer tells the core each frame shown (`kn_session_frame_displayed`, instants in µs on `CLOCK_MONOTONIC`, `kn_now_micros`), replaced, the bytes received and the holes; every second the core returns a `KnSecond` (median, path of a frame, losses, rejected measurements, round trip, bitrate) through `on_second`.
- **States** (`on_status`): preparing, connecting, first images, picture, reconnecting (attempt, maximum), ending, ended (with the reason when it was not wanted, and the console's report: displays and sound restored).
- **Notices** (`on_notice`): what the session shows in a capsule, without text (Swift translates it): picture relaunched, computer's mode changed or back (with the size), fallback (level), adapting to the screen (requested, done, refused), “Restore” offered or withdrawn.
- **Commands**: stop, keyframe lost or back, new settings (profile from the bar), bitrate cap (occluded window), chosen display, new mode (adapting to the device's screen), mode restore.
- **Log**: the core writes through the `log` crate; `kn_set_log_callback` sends it to `KyberLog` (unified log, `core` category).

## What the core gained in milestone 1

To match the Mac app, `kybernow-core` now carries, beyond what it had for Windows: the path of a frame and the losses in the latency (`latency.rs`, port of `LatencyTracker.swift`), the keyframe request repeated every 300 ms then the stream relaunched at 1.5 s, the fallback when no picture comes within 4 s (bitrate brought down to 250 Mbit/s, then 4:2:0, for this session only), tracking of the computer's display size (decoded frames and Kyber's list), the mode policy with “Restore”, adapting to a new mode of the device's screen, choosing a display from the bar, the bitrate cap, and the end report. All of it can be switched off or has no effect for Windows, which keeps its behaviour (its own keyframe and first-picture watches).

## Building

```bash
apps/macos/scripts/vendor-kyber.sh      # once: Kyber SDK → Vendor/
apps/macos/scripts/build-kyclient.sh    # libkynput with the patches (the app no longer links libkyclient)
apps/apple/scripts/build-apple.sh       # kybernow-core + kybernow-apple → Vendor/Frameworks/libkybernow.dylib, Vendor/KyberNow/include/kybernow.h
cd apps/macos && xcodegen generate && xcodebuild -scheme KyberNow -configuration Debug -derivedDataPath build build
```

`build-apple.sh` builds for `aarch64-apple-darwin` with the repository's stable Rust; the tvOS (nightly, `-Zbuild-std`) and iOS (stable) targets come from `apps/apple/scripts/build-apple-mobile.sh` (milestones 4 and 5); `apps/core/scripts/build-apple.sh` checks the chain for each target and builds libkynput for iOS. The core's tests: `cargo test -p kybernow-core` (see `apps/windows/README.md`).

## The Apple TV app since milestone 4

KyberKit is shared by both Xcode projects, core included: `SessionEngine`, `ConsoleClientCore`, `CoreBridge`, `InputCore`, `AudioPlayerCore` and `LatencyReporting` serve the Mac and the Apple TV (and iPhone and iPad since milestone 5: touch session view `KyberKitIOS/StreamView`, with the Apple TV's Metal surface, `KeyboardMouse` and `GamepadBridge`); each platform keeps only its session view, its Metal surface and the capture of its events (AppKit and `KeymapMac` on Mac, GameController's `GamepadBridge` and `KeyboardMouse` on Apple TV). The old Swift files (`KyberClient`, `ConsoleClient`, `LatencyTracker`, `AudioPlayer`, `InputPipeline`, `PacketQueue`, `Keymap`, `PadCombo`, `PointerLayer`, `ReleaseAll`, `ClipboardBridge`, `GamepadGate`) and their offline tests are gone: the core has its own.

On tvOS, the core is a static library (`Vendor/KyberNow.xcframework`, from `apps/apple/scripts/build-apple-mobile.sh tvos`) linked next to Kyber's SDK for tvOS (`Vendor/KyberSDK.xcframework`), from which the app now only takes libkynput, libopus and VLC's empty functions. `GamepadBridge` follows the combination itself before sending (menu, gamepad as mouse, long press): it keeps a `PadCombo`, now held by the core (`kn_combo_*`, with its own thread for the 250 ms deadline); the Apple TV's input chain has its combination turned off. The pointer held by the Apple TV (remote, gamepad, mouse) is the position of the core's `PointerLayer` (`holds_position`).
