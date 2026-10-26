# The Rust core in the Apple apps

English · [Français](coeur-apple.md)

The Mac, Apple TV, iPhone and iPad apps rest on the Rust client core (`apps/core`, crate `kybernow-core`), the one every KyberNow Link app uses ([apps/core/README.en.md](../apps/core/README.en.md)). Everything that does not depend on display lives there: the session (talking to Admin, Kyber, reconnection, keyframe, display, latency, profiles), the inputs and the sound. Decoding (VideoToolbox), rendering (Metal), the views and the system APIs stay in Swift. One code for every app.

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

The Windows app uses the same session, the same input module and the same sound engine; its clipboard stays kynput's. What the session does for the Apple apps (path of a frame and losses in the latency, keyframe request repeated every 300 ms then the stream relaunched at 1.5 s, fallback when no picture comes within 4 s — bitrate brought down to 250 Mbit/s, then 4:2:0, for this session only —, tracking of the computer's display size, mode policy with “Restore”, adapting to a new mode of the device's screen, choosing a display from the bar, bitrate cap, end report) can be switched off or has no effect for Windows, which keeps its own keyframe and first-picture watches. The Android, Linux and webOS apps use the core's watches, like the Apple apps. The connection's last resort (restarting the Station, below) applies to all of them, Windows included.

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
- **Composite results as JSON** (`char *`, freed by `kn_string_free`): Admin's replies, a session's settings. Admin's JPEG preview comes as bytes, freed by `kn_bytes_free`. Each second's measurements (`KnSecond`) and packets (`KnPacket`) are flat C structs: hot path, no copy, no allocation.
- **Callbacks** (`KnPlatform`): function pointers and a `userdata`; the core calls them from its own threads (session thread, Kyber's network thread for packets). Swift hops to the main thread for what touches the interface; packets go straight to the decoder.
- **Packets**: `video_packet` and `audio_packet` receive a `KnPacket` (stream number, format, configuration or frame, keyframe, pts, data in place, hole with the number of missing samples); a new stream number, a new decoder; the last call of a stream carries `KN_PACKET_CLOSED`, after which the core never calls again for that stream: Swift then stops its decoder or its sound output.
- **Input chain**: `input_pipeline()` returns the kynput chain of a `KnInput` (`kn_input_new`, on the view's `KnPointer` pointer layer, `kn_input_kynput`); the core passes it to Kyber with the picture session. Swift only hands over the captured events (`kn_input_key`, `mouse`, `mouse_button`, `wheel`, `type_text`, `gamepad_*`, held and resynced modifiers) and draws the pointer (`on_changed` of `KnPointerCallbacks`, shape in premultiplied ARGB); interface callbacks (combination, navigation keys, inputs connected, rumble, clipboard) go through `KnInputCallbacks`.
- **Sound**: one `KnAudio` per stream (`kn_audio_new`, the output's channel order) receives the packets (`configure`, `decode`, `conceal`); the output's real-time thread pulls samples with `kn_audio_fill` and measurements with `kn_audio_take_stats`.
- **Latency**: the Metal renderer tells the core each frame shown (`kn_session_frame_displayed`, instants in µs on `CLOCK_MONOTONIC`, `kn_now_micros`), replaced, the bytes received and the holes; every second the core returns a `KnSecond` (median, path of a frame, losses, rejected measurements, round trip, bitrate) through `on_second`.
- **States** (`on_status`): preparing, connecting, first images, picture, reconnecting (attempt, maximum), ending, ended (with the reason when it was not wanted, and Admin's report: displays and sound restored).
- **Notices** (`on_notice`): what the session shows in a capsule, without text (Swift translates it): picture relaunched, computer's mode changed or back (with the size), fallback (level), adapting to the screen (requested, done, refused), “Restore” offered or withdrawn; and, on the connection screen, the Station restarting (below).
- **The connection's last resort**: Admin has answered, but the picture does not arrive despite the session's own repairs, or the connection to Kyber fails. The core closes the attempt with Admin, restarts the Station once (`station_restart` without `force`: never if another device is in session, nor too soon, nor with a Station that is too old), waits for it (`info` every 2 s, 90 s at most), then runs the session again from the start, virtual display included. Never once a picture has been shown. The sequence and its safeguards: [apps/core/README.en.md](../apps/core/README.en.md). Swift receives `KN_STATUS_CONNECTING` then the notice `KN_NOTICE_STATION_RESTARTING` (7): the connection screen shows “The Station is restarting…” until the next `on_status` (`KN_STATUS_PREPARING` if it comes back, `KN_STATUS_ENDING` then `KN_STATUS_ENDED` otherwise). It has no C state of its own: `SessionEngine` takes an unknown state for the end of the session; an app that ignores the notice stays on “Connecting”. `recovery.restart_station` set to `false` in the JSON of `kn_session_start` turns it off (true when missing). If the picture still does not come, the end reason says so (“…, even after restarting the Station”), as it does for a Station that is not back in time.
- **Multiple displays** (Mac; contract: [multi-ecrans.en.md](multi-ecrans.en.md), Rust API: [apps/core/README.en.md](../apps/core/README.en.md#multiple-displays-srcscreensrs-srcsessionrs-srcinputpipeliners)): `kn_session_start` also takes `screens` in its JSON: `list`, the device's displays (mirrors grouped; `x`, `y`, `width`, `height`: frame in points, y pointing down, Cocoa's coordinates converted; `pixel_width`, `pixel_height`: native resolution; `refresh`; `primary`: the one with the menu bar), and `limit_4k`. `mode` still describes the primary display. In order, at each image start: `on_display` (the primary display, as before), then `on_screens` (`KnStreamScreen`: `screen`, the index in `list`; `display`, the computer's display with its `id` and size in pixels; `refresh`; `primary`, always the first; and `not_created`, the indexes of the displays the computer could not create: “The computer could not create this display.”; a plan display in neither list: single-display Station, session on the primary display alone), then `input_pipeline`: Swift creates the `KnInput` there on the primary window's pointer layer (the primary display's `display_id`), then calls `kn_input_set_screens` with one `KnInputScreen` per display (`screen`, `display_id`, size, its window's `KnPointer`). Image packets carry `KnPacket::screen`: one decoder and one renderer per display. With the desktop mouse, each window sends `kn_input_mouse_on(i, screen, x, y)` (pixels of its computer display); the computer's pointer arrives through `on_changed` of the layer of the display it is on, hidden on the others (Auto mouse: hidden only if it is everywhere). `kn_session_frame_displayed` for the primary display only; `kn_session_bytes_received` for all; `kn_session_keyframe_lost` from any decoder (the core waits for every stream's first data before asking for a keyframe). Mac displays changed during the session: `kn_session_set_screens(s, json)` (same JSON as `screens`); the connection resumes at `KN_STATUS_PREPARING` on the primary display, then `on_screens` comes back; it also comes back when the computer changes a display's size. Setting and home screen, same rules as Windows and Linux: `kn_screens_setting_enabled(custom_resolution, virtual_display)` (false: “Displays” setting inactive, on “This display”); `kn_screens_session_plan(json, setting, profile, custom_resolution, virtual_display, codec)` returns the plan (at least two displays: `device`, mode, position) or null for a single-display session: it decides whether to pass `screens` to `kn_session_start`, and its length makes the home line; `kn_screens_primary_index(json)`: the display showing the connection screen. With a single-display Station, the session runs on the primary display through the same callbacks (a single `KnStreamScreen`). Without `screens`, nothing changes: `KnPacket::screen` is 0 and `on_screens` is never called.
- **Commands**: stop, keyframe lost or back, new settings (profile from the bar), bitrate cap (occluded window), picture frozen by the app (`kn_session_set_hidden`: the “no picture” watch waits for its return), size of the decoded frames (`kn_session_decoded_size`), chosen display, new mode (adapting to the device's screen), mode restore.
- **Station** (the “Station” section of a computer's settings): the JSON of `kn_console_info` carries `station` (`version`, `update`: `available`, `version`, `installable`, `requires_setup`, `installing`; `restarting`), null with a Station 0.4.9 or older, for which the app offers neither restart nor update. `kn_console_admin_url` returns `https://address:port/`, to open in the browser. `kn_console_station_restart(c, device_id, force, out, error)` restarts the whole Station (the Kyber server and Admin, about a minute) and returns `{"ok": true}`; `kn_console_station_update` (same parameters) starts the same installation as “Install now” in Admin and returns `{"version": …}` in `out`. Both block: 15 s at most for the restart, as long as Admin takes to download and verify the package for the update (a few minutes at most); never on the main thread. `device_id` names the device in Admin's activity feed, `force` cuts off a session in progress. A refusal returns `KN_STATION_IN_SESSION` (a device is in session: call again with `force`), `KN_STATION_TOO_SOON` (one restart per minute, or too many failed attempts), `KN_STATION_NOTHING_TO_INSTALL`, `KN_STATION_REQUIRES_SETUP` (installer, on the computer), `KN_STATION_BUSY` (installation or restart under way) or `KN_STATION_UNSUPPORTED` (Station 0.4.9 or older), with the message in `error` and the details as JSON in `out` (`kind`, `message`, `device`: the device in session, `seconds`: the wait); anything else, the codes of the other calls.
- **Wake on LAN**: the JSON of `kn_console_info` carries `wake`, the computer's network cards, Ethernet first (`mac`, `ipv4`, `broadcast`, `kind`: `ethernet` or `wifi`, `armed`: Windows lets it wake the computer, null if unknown, `name`), an empty list with a Station older than 0.5.0. The app keeps this JSON for each computer when the list is not empty, and hands it to `int32_t kn_wake(const char *targets_json)` to offer “Wake the computer” once it is off (a single target, for example `{"mac": "…", "ipv4": "…"}`, works too). The magic packet goes to each card's subnet broadcast address, to 255.255.255.255 and to its last IPv4 address, directly; `KN_OK` as soon as one packet has gone out, `KN_WAKE_NO_TARGET` without a readable target, `KN_ERROR` if none went out. Quick, but off the main thread. **Pitfall**: on iPhone, iPad and Apple TV, broadcasting requires the `com.apple.developer.networking.multicast` entitlement, which Apple grants on request; without it, broadcasting is refused (often `EHOSTUNREACH`, logged) and only direct sending goes out, which does not always wake the computer (the network must still be able to reach the address). `kn_wake` succeeds anyway: the app has nothing to tell apart. On the Mac there is no restriction (see [apps/core/README.en.md](../apps/core/README.en.md#wake-on-lan-srcwakers)).
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

- Checked on the hardware: full Mac session, inputs and sound included; session on an iPhone 15 Pro Max (22/10/2026); session on an Apple TV and on an iPad; the Windows client with the core's inputs and sound.
- Not tried yet: the external display and Stage Manager.
- The iPhone and iPad app is installed from a Mac with Xcode, signed with one's own Apple account ([apps/ios/SIDELOAD.en.md](../apps/ios/SIDELOAD.en.md)); no TestFlight or App Store.
