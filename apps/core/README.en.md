# KyberNow Link core (`kybernow-core`)

English · [Français](README.md)

What the KyberNow Link apps have in common, written once in Rust: talking to KyberNow Admin, the Kyber session, profiles and bitrate, latency, updates, the input module, the sound engine, and several computers at once (session registry, single gamepad reader). Every app rests on it: Windows, Linux and webOS ([apps/windows](../windows/README.en.md), [apps/linux](../linux/README.en.md), [apps/webos](../webos/README.en.md)) directly, Android ([apps/android](../android/README.en.md)) through JNI (`apps/android/jni`), Mac, Apple TV, iPhone and iPad ([apps/macos](../macos/README.en.md), [apps/tvos](../tvos/README.en.md), [apps/ios](../ios/README.en.md)) through the C bridge `apps/apple` ([docs/coeur-apple.en.md](../../docs/coeur-apple.en.md)).

The core depends on no system: nothing in it calls AppKit, UIKit, Win32 or Core Audio (only the gamepad reader may open SDL itself, `sdl` feature, on Mac and Windows). What the platform provides goes through traits (the window, decoders, sound output and inputs through `session::Platform`; the clock, pasteboard and callbacks to the interface through the input module's traits; the sound output pulls sound from the engine itself). Kyber 0.29's building blocks come from `third_party/kyber-desktop/kysdk` with KyberNow's patches (`apps/macos/kyber-patches`), see [apps/windows/README.en.md](../windows/README.en.md).

## Code layout

| File | Contents |
|---|---|
| `src/console.rs` | Admin client: session (`prepare`, `heartbeat`, `foreground`, `end`), home and adding (`info`, `probe`, `preview`, `pair_request`, `pair_status`), the Station (`station_restart`, `station_update`, `admin_url`, below), pinned certificate, `Accept-Language` |
| `src/i18n.rs` | Language (detection, choice), text catalogues, release notes per language ([docs/i18n.en.md](../../docs/i18n.en.md)) |
| `src/session.rs` | A session's flow on a dedicated thread, the connection's last resort (below); what the platform provides: `Platform` |
| `src/media.rs` | Image and sound packets delivered by Kyber (patch 0001), with their source in Kyber (`PacketHandler::packet_from`), Opus header |
| `src/profile.rs` | Profiles, format, bitrate following pixels per second |
| `src/screens.rs` | Multiple displays (below): the device's displays, plan of the virtual displays (resolution, refresh rate, layout on the Windows desktop) |
| `src/latency.rs` | End-to-end latency, Kyber's clock |
| `src/update.rs` | Updates (Windows, Android, Linux, webOS): signed manifest (minisign), channel, verified download (size, SHA-256); the app's file is looked up under its current name, then its former names (Windows: `KyberNow-Link-Setup.exe`, then `KyberNow-Client-Setup.exe`, the only name in releases 0.3.1 and earlier) |
| `src/wake.rs` | Wake on LAN (below): `WakeTarget`, magic packet, broadcast and direct sending |
| `src/focus.rs` | Several computers at once (below): the app's session registry, “which one is active”, “Keep sound”, four at most |
| `src/gamepads/` | The app's single gamepad reader (below): fed by its SDL thread (`sdl.rs`, `sdl` feature) or by the app, routing to the sessions |
| `src/journal.rs` | The computer's name at the start of each session's log lines |
| `src/input/` | Shared input module (below) |
| `src/audio/` | Sound engine (below) |
| `build.rs` | libopus through pkg-config when it knows it; otherwise the executable links it itself |

## The Station in a computer's settings

The “Station” section of a computer's settings (Open Admin, Restart the Station, Update the Station) goes through `console.rs`:

- `Info::station` (`GET /api/stream/info`): `StationInfo` with the Station's version, its update (`StationUpdate`: `available`, `version`, `installable`, `requires_setup`, `installing`) and `restarting`. `None` with a Station 0.4.9 or older: the app offers neither restart nor update there.
- `Console::admin_url()`: `https://address:port/` (IPv6 in brackets), to open in the browser; Admin's certificate is self-signed, so the browser asks to accept it the first time.
- `Console::station_restart(device_id, force)`: restarts the whole Station (the Kyber server and Admin, about a minute, Admin does not answer meanwhile). `Console::station_update(device_id, force)`: the same installation as “Install now” in Admin, returns the version being installed; Admin downloads and verifies the package before answering (300 s timeout). `device_id`: the device's identifier, which names it in Admin's activity feed (it grants nothing: any paired device can restart and update); `force`: even if a device is in session, which is cut off.
- Refusals (`StationError`): `InSession` (a device is in session, `device`: its name; call again with `force`), `TooSoon` (one restart per minute, or too many failed attempts from this address; `seconds`), `NothingToInstall` (Station up to date, or version rejected), `RequiresSetup` (the version installs with the installer, on the computer), `Busy` (installation or restart already under way), `Unsupported` (Station 0.4.9 or older: it answers with Admin's page, or 404), `Other` (`ConsoleError`). The message comes from Admin, in the app's language. `kind()` and `to_json()` hand the refusal to the bridges (`kind`, `message`, `device`, `seconds`).

## The connection's last resort: restarting the Station

When a connection really cannot get through, the session restarts the Station itself, like “Restart the Station” in the settings, then tries again (`src/session.rs`, `Recovery::restart_station`, true by default, with no setting in the apps).

- **When.** Admin has answered (`prepare` succeeded) and no picture has been shown yet (`LatencyTracker::any_displayed`), but the picture does not arrive despite the session's own repairs (keyframe requested again, stream restarted three times, fallback: lower bitrate then 4:2:0), Kyber rejects the picture (`StreamingStartFailed`), the picture pipeline fails three times, or no usable display appears; or the connection to Kyber fails after its own attempts: network, timeout, server or protocol error (`ControlPlaneConnectFailed`), cannot connect to the stream, reconnection exhausted, connection closed by Kyber.
- **Never**: Admin unreachable or refusing the account (`prepare` or `end` failing), an account Kyber does not know yet (`Unauthorized`: Admin already restarts Kyber for pending accounts when nobody else is using it), a different Kyber certificate, an error on the device, the user cancelling, an end requested by Admin or by another device taking over, an established session (a picture has been shown: it keeps its own recovery, reconnection included), `restart_station` false.
- **Sequence.** The attempt is closed like the end of a session (`POST /api/stream/end`: displays and sound restored; otherwise Admin would count this device as in session and refuse). Then `station_restart(device_id, force = false)`: Admin notes in its activity feed which device restarted the Station, and refuses if another device is in session (`InSession`), if it is too soon (`TooSoon`), with a Station that is too old (`Unsupported`) or on any other error; the session then fails with its reason, as it would without this last resort. Once accepted, the platform receives `Status::StationRestarting`; the core polls `info` every 2 s until Admin answers with `station.restarting` false, 90 s at most. Once it is back, the session starts over: `Preparing`, `prepare` (the virtual display is created again), Kyber, picture, with the initial settings (the fallback starts from scratch).
- **Once per connection.** If the picture still does not come: “{reason}, even after restarting the Station” (`session.after_station_restart`); if the Station is not back within 90 s: “the Station is taking a while to come back after restarting: open Admin to find out more” (`session.station_not_back`). The user can cancel while waiting: the session stops without an error.
- **For the apps.** `Status::StationRestarting` comes between the current step and `Preparing` (or `Ending`); the connection screen shows “The Station is restarting…” ([docs/link-ui.en.md](../../docs/link-ui.en.md), “Connecting”). An app that does not show it yet stays on the “Connecting” step: Apple receives `KN_STATUS_CONNECTING` then the notice `KN_NOTICE_STATION_RESTARTING` ([docs/coeur-apple.en.md](../../docs/coeur-apple.en.md)), Android state 1 then notice 7 (`SessionListener.NOTICE_STATION_RESTARTING`), the Windows interface `session-status` at `connecting` then the `session-station-restarting` event (`ui/src/lib/api.ts`), Linux and webOS their “Connecting” step. The bridges read `recovery.restart_station` from the session JSON (true when missing); the Windows command line and test benches can turn it off.

## Wake on LAN (`src/wake.rs`)

The apps offer “Wake the computer” when it is offline, with nothing to type: Admin (Station 0.5.0 and later) gives its network cards in `Info::wake`, the app keeps them for each computer and the core sends the magic packet.

- `WakeTarget`: one card, `mac` (`AA:BB:CC:DD:EE:FF`), `ipv4` (its last address), `broadcast` (its subnet's broadcast address), `kind` (`ethernet`, `wifi`), `armed` (Windows lets it wake the computer; `None`: unknown) and `name`. Serialised as is (serde, every field optional except `mac`, unknown fields ignored): the Rust apps store it in their file, the others keep the JSON of `wake`. `Info::wake` is empty with an older Station, and reading is lenient (an unreadable `wake` gives an empty list, the rest of `info` is read). The app replaces the targets it keeps with those of a successful `info` when the list is not empty. `WakeTarget::from_mac(mac, ipv4)`: a target from a typed address (`3C:7C:…`, `3c-7c-…`, `3c7c.3f12.abcd`).
- `send(&targets)`: each card's magic packet (6 × `FF`, then the MAC address 16 times, 102 bytes), over UDP with `SO_BROADCAST`, ports 9 and 7, to `destinations(&targets)`: each subnet's broadcast address (Admin's, otherwise the /24 of the known address), 255.255.255.255, then each card's last IPv4 address, directly. A refusal on one address is logged and stops nothing; `Ok(n)` as soon as one packet has gone out, `WakeError::NoTarget` without a readable MAC address, `WakeError::Network` if none went out (message in the app's language). No network wait, but call it off the UI thread. `targets_from_json` reads, for the bridges, a list of targets, a single target or the whole info.
- `parse_mac`, `format_mac`, `magic_packet`, `broadcast(ip, prefix)`: the building blocks, tested (`cargo test -p kybernow-core wake`); `envoi_reel`, ignored by default, really sends to a dummy card (`-- --ignored`).

**Apple pitfall.** On iPhone, iPad and Apple TV, sending to a broadcast address (the subnet's as well as 255.255.255.255) requires the `com.apple.developer.networking.multicast` entitlement, which Apple grants on request (a form in the developer account, then the entitlement in the `.entitlements` files). Without it, broadcast sending fails, often with `EHOSTUNREACH`. Sending directly to the last IPv4 address works without it, but only wakes the computer if the network can still reach that address (ARP cache of the router or switch, a sleeping card that still answers ARP), which is not always the case after a full shutdown. That is why `send` always tries direct sending as well as broadcast and succeeds as soon as one packet goes out: without the entitlement, waking sometimes works; with it, as everywhere else. On the Mac there is no restriction (the “Local Network” permission, already asked for to reach Admin, is enough).

## Multiple displays (`src/screens.rs`, `src/session.rs`, `src/input/pipeline.rs`)

A session can take all of the device's displays: one virtual display per device display, laid out the same way, each shown in its own window (shared contract: [docs/multi-ecrans.en.md](../../docs/multi-ecrans.en.md)). The Mac, Windows and Linux apps use it; the single-display session keeps exactly its API, and apps that do not handle multiple displays have nothing to change.

**The device's displays and what decides how many.** `Screens { list, current, limit_4k, custom_resolution, setting, max_screens }`: the displays (`DeviceScreen`, below), “This display” (`current`, the display of the app's window, the one of a single-display session), “Up to 4K”, a custom resolution, the “Displays” setting (`DisplaysSetting`: `Auto`, `This`, `All`, stored as `auto`, `this`, `all`), and the number of displays the Station can create (`virtual_display.max_screens` from `GET /api/stream/info`, `VirtualDisplayInfo::max_screens()`; absent: 1). `DeviceScreen { x, y, width, height, pixel_width, pixel_height, refresh, primary }`: the display's frame in the device's logical coordinates, y pointing down (points on the Mac, after converting Cocoa's coordinates; the Windows desktop; `SDL_GetDisplayBounds` on Linux), its native resolution in pixels (before “Up to 4K”), its refresh rate, and whether it is the primary one (none: the first). Mirrored displays count as one: the app passes only one. Both types go through JSON as they are (serde, optional fields) for the bridges.

**The plan** (`screens::plan(&screens, codec)`, without looking at the setting): four displays at most and no more than `max_screens`, the primary first, each with its device display (`device`, index in `list`), its mode and its position in pixels on the Windows desktop (`PlannedScreen`).

- Resolution (`screens::fit`): “Up to 4K” brings each display down to 3840 × 2160, H.264 to 4096 pixels wide (NVIDIA's encoder stops there), otherwise 7680 × 4320; aspect ratio kept, rounded to even, like the Mac app's `DisplayInfo.mode`.
- Refresh rate (`screens::refresh`): the primary display keeps its own; the others run at 60 Hz, less if their display does not go that high.
- Position (`screens::layout`): the primary at (0, 0); the others attached one by one, on the same side, to an already placed neighbour they share an edge with on the device, with the device's offset converted at the neighbour's pixels / logical units scale; at least 64 pixels of shared edge; never any overlap (the display slides along the edge, and among its possible neighbours the core picks the one that makes it slide the least); a display touching nothing attaches to the nearest one, on the side of the largest gap. Beyond four displays (or `max_screens`), the ones taken are the closest to the primary, step by step. `plan_excluding(&screens, codec, &removed)` redoes the plan without displays removed during the session (never the primary), indexes staying the device's.

**A single rule for the number of displays**: `screens::session_plan_for(&screens, &settings, virtual_display_requested)`, the session's calculation, also to use for the home screen and the setting. It counts the virtual display if it is requested and usable (`Screens::virtual_display_available`, from `virtual_display.available` of `info`), and the profile given by `screens::screens_profile(&settings, Screens::capabilities)`: `None` (“Custom”) without a profile, or when the settings differ from `StreamSettings::for_profile` for those capabilities; the profile sent to the Station does not change. A Station that prepares without a virtual display or without `screens`: plan emptied for the whole session, “This display”. Single-display session: the stream's display follows “This display” (`set_screens`), without restarting the image; a single input target: every pointer to the main layer. A session redo waits for the preparation in progress.

**The “Displays” setting, the home screen and the number of displays** ([docs/link-ui.en.md](../../docs/link-ui.en.md)): `DisplaysChoice { setting, profile, custom_resolution, virtual_display }` (`Screens::choice(profile, virtual_display)`). `setting_enabled()`: false with a custom resolution or without a virtual display; the setting is then inactive and shows “This display” (`shown_setting()`). `all_displays()`: All displays; Auto takes every display in Desktop and Max, one in Game and Custom. `screens::session_plan(&screens, profile, virtual_display, codec)` returns the plan when the session will take at least two displays, otherwise `None` (setting, profile, a Station that creates only one, a single display): the home line (“3 displays, sized for this device's screens”: the plan's length) and the windows to open come from it, and the session makes the same calculation. The device's primary display (the one showing the connection screen): `screens::primary_index(&list)`.

**The session**: an app that handles multiple displays always opens its sessions with `Session::start_with_screens(params, screens, platform)`; the core decides the number of displays itself (`session_plan`), at the start and when the profile changes during the session (`set_settings`). `params.mode` is the mode of “This display”, the one of a single-display session; with several displays, the plan replaces it. A single-display session runs on “This display” (`current`), with the same callbacks.

1. `prepare` carries `screens` when the plan has several displays (or one, when the others were removed during the session); `width`, `height` and `refresh` then describe the primary display. The answer says what is in place (`Prepared::screens`, in the request's order, `None` for a display the Station could not create; it is the reference for modes and positions). Without `screens` in the answer (a Station that handles one display only) or without the primary display, the session runs on the primary display, without an error.
2. Each display in place is found in Kyber's list by its position and size (failing that, by its size alone, if only one free display has it). The core waits for all of them to be announced, 6 s at most as for one display, then starts the image on those that are there; if the primary is missing, on one display as before. A display announced later restarts the image on all of them.
3. One video entry per display in Kyber's session, the primary first. At each image start, the platform receives `on_display` (the primary display, as always), then `on_screens(&[StreamScreen], not_created)`, then `input_pipeline()`. `StreamScreen { screen, display, refresh, primary }`: the device display's index, the computer's display as Kyber announces it (`id` for inputs, size in pixels, position on the desktop), its refresh rate, and whether it is the primary. `not_created`: the plan's device displays the computer could not create (`None` in the Station's answer, or never announced by Kyber); their window keeps the connection background with “The computer could not create this display.”. Each stream's packets go to the handler returned by `screen_video_handler(screen)`: one decoder per display. `not_created` also carries the displays removed because they stayed silent (below). A single-display session, a single-display Station or only the primary accepted: one display and an empty `not_created`; the other displays' windows do not open (or close), without a message. The number of displays in the statistics is the list's length.
4. Bitrate: the profile's, computed on the sum of the displays' pixels per second (`StreamSettings::bitrate_for_pixel_rate`), with the same caps; the Station splits it between the displays.
5. Keyframe: `keyframe_lost` and `keyframe_recovered` as for one display, from any decoder. Kyber asks every stream for a keyframe at once, and a request before a stream's first data leaves its encoder silent (a trap already met): the core only asks once every stream has received data (each starts with a keyframe); the watch asks again every 300 ms and restarts the stream at 1.5 s. Restarting the stream restarts every display.
6. Silent display: a secondary stream still without data 4 s after the image starts, while the primary receives some (its encoder did not open, Kyber accepted the session anyway), is removed: the session is redone without it (connection resumed at its first step) and it goes into `not_created`. A refusal from Kyber when starting the image on several displays (`403`, without saying which display; `StreamingStartFailed`): a new attempt with the primary display alone (Station redone with it alone, Kyber reconnected), once; the other windows close as when only the primary is accepted. The primary follows a single-display session's recovery rules.
7. Measurements: `frame_displayed` (latency, first image, “no image” watch) for the primary display only; `bytes_received` for all.
8. The device's displays change during the session (plugged, unplugged, layout or mirroring changed): `Session::set_screens(screens)`; a profile chosen during the session (`set_settings`) may also change the number of displays, which follows at once. If the plan changes, the connection resumes at its first step (`Status::Preparing`, then `Connecting` and `FirstImages`, without an adaptation capsule): the image stops, the Station redoes the displays, then the image starts again, without going back home. A refusal from Admin ends the session with its reason, as a failed connection. `set_mode` (adapting a single-display session) has no effect while the session has several displays: `set_screens` is the one to use.
9. The computer changes a display's size: `on_display` for the primary as before, and `on_screens` again; a display gone from Kyber's list restarts the image.

**Inputs**: `InputPipeline::set_screens(Vec<ScreenTarget>)`, to call in `input_pipeline()` after `on_screens` (and again when `on_screens` comes back). `ScreenTarget { screen, display_id, host_size, pointer }`: the device display's index, its computer display's `id`, its size in pixels, and the pointer layer of that display's view (`None`: the one given to `InputPipeline::new`, the primary display). Then:

- `mouse_on(screen, x, y)` sends an absolute position, in pixels of that display's computer display, with its `display_id`; `mouse(x, y)` stays the primary display's; an unknown display goes to the primary.
- The pointer announced by the computer (shape, position in pixels of its display, visibility) goes to the layer of the display whose identifier it carries: each view draws only its own. A pointer leaving a display is announced hidden there. Auto mouse, a single rule for the three apps: `PointerLayer::summary(layers)` (or `InputPipeline::pointer_summary()`, the main layer and each display's) returns a `PointerSummary { hidden, visible_for }`: hidden only if it is hidden on every display the pointer has already been on (a display never visited, with no announcement from the computer, does not count: it would keep game mode from starting), visible for the longest of their delays; to give to `MouseModeRules::wants_relative`.
- Keyboard, relative mouse, wheel and gamepads do not change.

**For a Windows or Linux app**, in order:

```rust
use kybernow_core::screens::{self, DeviceScreen, DisplaysSetting, Screens};
use kybernow_core::session::{Platform, Session, StreamScreen};
use kybernow_core::input::{InputPipeline, Options, ScreenTarget};

// 1. The device's displays (mirrors grouped), the primary marked, and what decides how many.
let screens = Screens {
    list: vec![DeviceScreen { x, y, width, height, pixel_width, pixel_height, refresh, primary }, /* … */],
    current,                                              // the display of the app's window
    limit_4k, custom_resolution, setting,                 // the computer's settings
    max_screens: info.virtual_display.max_screens(),      // GET /api/stream/info
};
// Home screen and windows: the same calculation as the session.
let plan = screens::session_plan(&screens, params.settings.profile, params.virtual_display, params.settings.codec);
// plan.map(|p| p.len()): “{n} displays, sized for this device's screens”, otherwise the single-display line.
let session = Session::start_with_screens(params, screens, platform);

// 2. The platform (the other methods as for one display).
impl Platform for App {
    fn on_display(&self, d: &Display) { /* the primary display, as before */ }
    fn on_screens(&self, s: &[StreamScreen], not_created: &[usize]) { /* window of display s[i].screen ↔ computer display s[i].display */ }
    fn screen_video_handler(&self, screen: usize) -> Arc<dyn PacketHandler> { /* decoder of `screen`'s window */ }
    fn input_pipeline(&self) -> Pipeline {
        let p = InputPipeline::new(Options { display_id: primary.id, host_size, clipboard, gamepads_from_app }, primary_pointer, callbacks, clock);
        p.set_screens(screens.iter().map(|s| ScreenTarget {
            screen: s.screen, display_id: s.display.id,
            host_size: (s.display.width as u32, s.display.height as u32),
            pointer: Some(window_pointer(s.screen)),
        }).collect());
        p.kynput()
    }
}

// 3. During the session: p.mouse_on(display, x, y) with the desktop mouse; session.frame_displayed(…)
//    for the primary display; session.set_screens(…) when the device's displays change
//    (with the same settings); session.set_settings(…) is enough for a changed profile.
```

## Several computers at once (`src/focus.rs`, `src/gamepads/`, `src/journal.rs`)

An app can hold sessions on several computers, one window per computer (shared contract: [docs/plusieurs-ordinateurs.en.md](../../docs/plusieurs-ordinateurs.en.md)). The Mac, Windows and Linux apps (desktop interface) use it. An app with one session has nothing to change: its session is active and audible; iPhone, iPad, Apple TV, Android and LG TVs stay on one computer without calling anything new.

**The session** holds two states, valid for the whole session: the input chain and the sound engine of every image start take them.

- `Session::set_active(bool)` (active by default). Not active: nothing leaves the device for its computer (keyboard, mouse, wheel, text, gamepad buttons and axes; gamepad plugs, releases and returns to rest always go through), held keys and clicks are released, its gamepads are put at rest (buttons released, axes centred, triggers released), relative mouse is off (the app's setting comes back on return), the clipboard is paused (nothing read from the device, nothing written to it; what the computer announces is ignored, its request gets “nothing”) and rumble is ignored. Active again: the device's clipboard content is announced to the computer. The core finds by itself the chain the platform creates in `Platform::input_pipeline` (on the session thread: every app creates it there); `InputPipeline::set_active` is for a single chain.
- `Session::set_muted(bool)` (not muted by default): sound fades out in 20 ms (`AudioEngine::set_muted`), still decoded and pulled at the same pace: it comes back without any delay. The core finds the engine behind `Platform::audio_handler` when it is an `AudioEngine` (Windows) or a handler that returns it through `PacketHandler::audio_engine`; otherwise the platform gives it through `Session::attach_audio` (Mac: `kn_session_attach_audio`). Independent from the volume and from an app's own “Mute”.
- `Session::set_name(name)`: the computer's name at the start of the session's log lines (“[PC-ATELIER] image: …”), written by its thread, the threads it starts and its input thread; empty: nothing, as before. The registry gives it at registration.

**The registry** `kybernow_core::focus` (`focus::global()`, one per process), the single source of “which one is active”; apps only tell it which window comes to the front:

- `register(&Session, name) -> Result<FocusId, Full>`: registers the session, four at most (`MAX_SESSIONS`, beyond that `Full`); it becomes active, like its window opening on top; already registered: same identifier;
- `activate(id)`: this session's window comes to the front;
- `set_keep_sound(id, bool)`, `keep_sound(id)`: “Keep sound”, the session stays audible without being active (off by default);
- `unregister(id)`: end of the session (window closed, “Disconnect”, end); it goes behind, muted; if it had the hand, the one that had it just before takes it back (the app corrects with `activate` if the system brought another window to the front);
- `active()`, `count()`, `ids()` (opening order), `next()` (“Next computer”: the session opened after the active one, the first after the last), `name(id)`.

On every change it applies to all registered sessions: those losing the hand go behind first (`set_active(false)`, muted unless “Keep sound”), then the active one (`set_active(true)`, sound back): what one releases leaves before what the other receives, and the device's clipboard is never read for two. A session freed without unregistering is forgotten.

**Gamepads**: a single reader per app, `kybernow_core::gamepads` (`gamepads::global()`), taken from the Linux app's. SDL only opens once per process: two kynput chains each reading gamepads through SDL would steal each other's events, and the second would silently lose them.

- **Mac and Windows**: its own SDL thread (`gamepads::start_sdl()`, the core's `sdl` feature, enabled by the `apps/apple` bridge on macOS and by the Windows app; SDL2 linked by `build.rs`), with a libkynput built without its SDL reader (`apps/macos/scripts/build-kyclient.sh`, `apps/windows/scripts/build.sh`). It also starts by itself at the first `InputPipeline` created with `gamepads_from_app: false` when libkynput has no reader (`InputPipeline::core_reads_gamepads`): these apps keep `gamepads_from_app: false`, and the Mac's gamepad hook (combo, capture by the bar, `install_gamepad_gate`) sees these packets like kynput's. Thread `kybernow-manettes`, off the main thread like kynput's (`SDL_JOYSTICK_THREAD` on Windows, no SDL signal handlers); four gamepads at most, numbered in order of arrival (0 to 3); rumble played on that thread (-1: 5 s, like kynput).
- **Linux** (its SDL loop already reads gamepads and windows in the same queue, a second reader would steal its events): the app feeds the reader, `gamepads::plug(id, vendor, product)`, `unplug(id)`, `button(id, PadButton, bool)`, `axis(id, PadAxis, i16)`; rumble comes back through the active session's `InputPlatform::rumble`, as before.
- **Routing**: every gamepad is plugged into every session's input chain (the Station keeps its virtual gamepad, a game does not lose it when switching computers), plugged again when a chain joins its computer (`InputPipeline::network_connected`); buttons and axes only go to active chains; a chain going behind puts its gamepads at rest by itself, and ignores rumble. `gamepads::pads()`: the gamepads present (identifier, vendor, product, name given by SDL).

**Displays**: a session opened while another runs takes “This display”, in a window, whatever the “Displays” setting (two sessions never take the same displays): `screens::screens_for_extra_session(&screens)` gives the displays to pass to `Session::start_with_screens` (`current`: the display of the active window, where the new one opens); `screens::extra_session_window(&display)` gives its window, “centred, two thirds of that display, in 16:9” ([docs/link-ui.en.md](../../docs/link-ui.en.md)): frame in the device's logical coordinates and `Mode` in the display's pixels (for `SessionParams::mode`).

**Clipboard on Windows**: the client's kynput.dll is built without kynput's clipboard (`--cfg kybernow_app_clipboard`, patch 0004: its hidden window only exists once per process); the app gives the core the Windows clipboard (`apps/windows/src/pasteboard.rs`, text), one bridge per chain, paused outside the active session.

**For a Mac, Windows or Linux app**, in order:

```rust
use kybernow_core::{focus, gamepads, screens};

// At launch (Mac, Windows; Linux: gamepads::plug/unplug/button/axis from its SDL loop).
gamepads::start_sdl();

// “Connect” disabled beyond four: “Four computers at most at the same time.”
let full = focus::global().count() >= focus::MAX_SESSIONS;

// One more session: “This display”, in a window, on the active window's display.
let screens = screens::screens_for_extra_session(&screens);
let window = screens::extra_session_window(&screens.list[screens.current]);
params.mode = window.mode;
let session = Session::start_with_screens(params, screens, platform);
let id = focus::global().register(&session, &name)?;  // it becomes active

focus::global().activate(id);                         // its window comes to the front
focus::global().set_keep_sound(id, keep);             // the bar's “Keep sound” button
let next = focus::global().next();                    // ⌃⌥⌘O / Ctrl+Alt+Shift+O: bring its window to the front
focus::global().unregister(id);                       // window closed, “Disconnect”, end of session
```

## Shared input module (`src/input/`)

The inputs of every app (Mac, Apple TV, iPhone and iPad through `apps/apple` and `InputCore.swift`; Android through JNI; Windows, Linux and webOS directly), on top of Kyber's kynput chain (`kynput_rs`, `kynput_sys`): chain and fallback queue, keyboard, gamepad combination, release everything, the computer's pointer, clipboard (see “Inputs” in [apps/macos/README.en.md](../macos/README.en.md)).

| Module | What it holds |
|---|---|
| `clock.rs` | `Clock`: the platform's monotonic clock; `MonotonicClock` (`Instant`), `ManualClock` (tests) |
| `packet.rs` | `OwnedPacket`: a kynput packet whose reference we hold (released at the end, cloned per send attempt, handed to kynput); `PacketSink`: sending into the chain, `KynputSink` (a kynput chain's input sink) |
| `packet_queue.rs` | `PacketQueue`: sending to kynput, refusals counted (the first, then every 100, then the total at the end), fallback queue of the packets that cannot catch up (keys, clicks, buttons, plugs, returns to rest, text, FlushKeys) retried every 3 ms in order until success or 200 ms; “once” packets (position, move, wheel, moving axis) do not leave while it is not empty |
| `keymap.rs` | HID usages → scancodes (`kynput_keycode_usb_to_kyber`), “⌘ sent as Ctrl”, swap of the `@#` and `<>` keys of Apple ISO keyboards (the layout is only asked for those two keys), Caps Lock and Num Lock recognised as toggles, ISO setting for Apple or unknown keyboards |
| `pad_combo.rs` | `PadCombo`: L1 + R1 + Start + Select held 250 ms, held buttons per gamepad, known gamepads, the six axes to bring back to rest on capture (`PadAxis::ALL`), navigation keys; `PadButton`, `PadAxis` in kynput's vocabulary, with their packet values (`kynput_rs` confuses the right stick with the right shoulder in `GamepadButton::to_u32`, so it is bypassed) |
| `release_all.rs` | `ReleaseAll`: what the device holds down (scancodes, buttons, modifiers in the platform's identifier), release everything (buttons then keys), forget, modifiers really held sent again on return |
| `pointer_layer.rs` | `PointerLayer`: position, shape (ARGB pixels as kynput delivers them, `Shape::premultiplied` for CGImage and Win32), hot spot and visibility of the computer's pointer; position held by the device (Apple TV, `holds_position`) that the computer takes back when inputs are idle (0.4 s), screen size change, resend after reconnection; `MouseMode`, `MouseModeRules` (Auto / Desktop / Game, 0.6 s tolerance for a pointer shown again, recheck at 0.7 s); drawing scale and frame |
| `clipboard.rs` | `ClipboardBridge` on the `Pasteboard` trait: text both ways (announce, immediate request, answer, 60 KB), the device's pasteboard looked at twice a second, our own write does not go back; where reading the text disturbs the user (`reading_asks_user`, iOS), it is only read when the computer asks and changes are announced without reading (`has_text`); paused for a session that is not active (`set_paused`: nothing read or written, the device's content announced on resume); C functions of patch `kyber-patches/0002` |
| `gamepad_gate.rs` | The Mac's SDL gamepads (read by the core's reader, or by kynput with a libkynput that has its own; Windows, without this hook): router hook passing every gamepad packet to the combination and dropping, while the app holds the gamepad (bar open), presses and moving axes (navigation keys handed to the app); releases and axes at rest always go through; outside capture, axes unchanged at 1/512 are dropped. `InputPipeline::install_gamepad_gate`, `set_gamepad_captured` (releases and the six axes at rest sent at capture) |
| `pipeline.rs` | `InputPipeline`: assembles everything on top of a kynput chain: keyboard, absolute and relative mouse, wheel, Unicode text, FlushKeys, gamepads produced by the app (`gamepads_from_app`: Apple TV, iPhone, iPad, Android, Linux, webOS; plug, buttons, axes filtered to 1/512 of the travel and grouped by 4 ms, returns to rest retried, reset to rest) or by the core's reader (Mac, Windows), release everything; session active or not (`set_active`, several computers); reception of the pointer (shape deduplicated, fully transparent shape discarded), rumble and clipboard |

**Timers.** No module creates a thread or a queue: each exposes `poll()` and `next_deadline()`, and `InputPipeline::poll` calls them all (retries, axes, combo, clipboard, received packets). The platform calls `poll()` by the deadline at the latest from whichever queue it wants, or lets `InputPipeline::spawn_driver` run a thread (`kybernow-entrees`) that sleeps until the deadline or a wake-up (`wake()`, triggered by anything that creates a wait). The tests advance a `ManualClock` and call `poll()` themselves.

**Threads.** Every method can be called from any thread (state under locks). kynput's callbacks arrive on its network thread: the pointer goes to `PointerLayer`, which notifies the view outside its lock; rumble to `InputPlatform::rumble`; clipboard packets are copied and handled by `poll()`, outside the router's lock (the answer would re-enter it).

### What the platform provides

```rust
// Monotonic clock (CLOCK_UPTIME_RAW, QueryPerformanceCounter…).
pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
}

// Text pasteboard (NSPasteboard, UIPasteboard, Android, Linux; None on tvOS and webOS; kynput's own on Windows).
pub trait Pasteboard: Send + Sync {
    fn change_count(&self) -> u64;
    fn text(&self) -> Option<String>;
    fn set_text(&self, text: &str);
}

// Callbacks to the interface (every method has an empty default).
pub trait InputPlatform: Send + Sync {
    fn rumble(&self, rumble: GamepadRumble) {}  // kynput's network thread
    fn combo_formed(&self, pad: PadId) {}        // the thread calling poll()
    fn connected(&self) {}                       // the thread calling network_connected()
    fn navigate(&self, key: PadKey) {}           // kynput's thread, under the router's lock
}

// Substituted sending (tests, benches).
pub trait PacketSink: Send + Sync {
    fn consume(&self, packet: OwnedPacket) -> bool;
}
```

And, outside the traits: **event capture** (AppKit, GameController, Win32, Raw Input, SDL, Android) translated into `InputPipeline` calls (`key`, `mouse`, `mouse_move`, `mouse_button`, `wheel`, `type_text`, `gamepad_*`, `release_all`, `flush_keys`); each key's **HID usage** and the **ISO layout** (`keymap::scancode(hid, command_as_control, || iso)`); **drawing** the pointer on the main thread from `PointerLayer::set_on_changed`; the **mouse mode rule** (`MouseModeRules { needs_hidden_pointer }`, `true` on Mac, Windows and Linux, `false` on Apple TV, iPhone, iPad and Android); the Mac's and Windows' SDL gamepads are read by the core's single reader (`crate::gamepads`, with a libkynput without its own; by kynput otherwise); on the Mac, the router hook (`install_gamepad_gate`) filters them and hands navigation keys back through `InputPlatform::navigate`.

Creation: `InputPipeline::new(Options { display_id, host_size, clipboard, gamepads_from_app }, pointer, platform, clock)` returns an `Arc`; `kynput()` gives the chain to `session::Platform::input_pipeline`; `start_clipboard()` once the network is connected; `stop()` at the end of the session (direct FlushKeys, total losses logged), `shutdown()` to stop kynput's handlers.

## Sound engine (`src/audio/`)

The sound of every app, without the output: the platform (Core Audio, WASAPI, AAudio, SDL) pulls sound from the engine from its real-time thread. Mac, Apple TV, iPhone and iPad go through `kn_audio_*` (`AudioPlayerCore.swift`), Windows directly (`apps/windows/src/audio.rs`), Android through AAudio (`apps/android/jni/src/audio.rs`), Linux and webOS through SDL (`apps/linux/src/device/media.rs`, `apps/webos/src/device/media.rs`; in 5.1, webOS hands the sound to the TV's player).

| Module | What it holds |
|---|---|
| `opus.rs` | libopus multistream: `Decoder` (stereo, 5.1, 7.1 from the `OpusHead` header of `media.rs`), concealment frame; an encoder for the tests |
| `ring.rs` | Sample ring with one producer (network thread) and one consumer (real-time thread), lock-free |
| `mix.rs` | Downmix (L + 0.7 C + 0.7 left surround and left rear, same on the right, LFE ignored; 7.1 → 5.1; mono = average) and `OutputOrder` (Opus, Windows, Core Audio 5.1 A / 7.1 C, or custom) |
| `engine.rs` | `AudioEngine`: `configure` / `decode` / `conceal` (or `PacketHandler::packet`) from Kyber's network thread, `fill(&mut [f32], frames)` from the real-time thread; adaptive buffer; `AudioStats` once a second; sound muted or back in 20 ms (`set_muted`, several computers) |

Buffer: 8 ms margin at the start, growing by 5 ms after an empty buffer (60 ms at most) and coming back down by 1 ms every 2 s without a gap (4 ms at least); first start at the margin plus one packet, resume as soon as the margin is there; an empty buffer is filled with an Opus concealment frame (silence if the decoder is busy with the network thread at that instant); drift caught up by one sample skipped or doubled out of 256 (out of 128 beyond 6 ms of difference) until the second's lowest level is back at the margin; hard cut only beyond 10 ms ahead. A full buffer (output stopped for a second) drops what arrives; the hard cut catches up on restart.

`fill` takes no blocking lock: the ring is lock-free, the decoder (concealment frame) and the stats are taken by try-lock, and a missed try gives silence for that request. The output's channel count is `out.len() / frames`: the platform changes it by changing its buffer size (AirPods plugged on a 5.1 stream), and gives its latency with `set_output_latency_ms` (the sound card's buffer plus what it reports) for the sound-to-image offset in the stats (`take_stats`).

### What the platform provides

- The output itself: its thread, the number of channels it accepts (or mixes), its channel order (`AudioEngine::with_order`), its latency; recreating it when the system's output changes.
- The clock (`Clock`, the input module's).
- libopus at link time: `build.rs` asks pkg-config for it (`PKG_CONFIG_PATH` towards Kyber's rootfs on the Mac) and settles for a warning otherwise (the Windows app links it statically, `apps/windows/build.rs`).

## Testing

On the Mac, with the Kyber SDK built for macOS (`third_party/kyber-desktop/rootfs-arm64-macos`, produced by the Mac app's scripts):

```bash
cd apps && PKG_CONFIG_PATH=$PWD/../third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib cargo test -p kybernow-core
```

The input tests cover the keyboard table, ISO, Caps Lock; short, held and fully released combo, setting off; fallback queue: accepted, refused twice then sent in order, refused endlessly, stop; axis filtering and grouping, reset to rest, release everything, text, deduplicated pointer, clipboard both ways; the sound tests, a 440 Hz A encoded in Opus, stereo and 5.1 at real pace with ± 8 ms of jitter and one loss in 97, 10 ms packets, 0.2% drift both ways, output stalled 200 ms then hard cut, downmix and channel order. They advance a `ManualClock` by hand: they last a second. `cargo test -p kybernow-core input` or `… audio` for one half. The session tests (`… session`) cover the last resort with a fake console: when it triggers and when it does not, only once, Admin's refusals (always without `force`), the Station coming back, the deadline passing, cancelling while waiting, and the texts. The multi-display tests (`… screens`, `… session`, `… console`, `… input`): the author's layout (XG32UQ 4K primary in the middle, Studio Display 5K on the right, MacBook Pro on the left and lower), with “Up to 4K” and in H.264, stacked displays, mixed scales, an isolated display, a display that would slide over another, four displays at most, and 300 random layouts that Windows must accept (primary at (0, 0), no overlap, all connected); `prepare`'s JSON both ways; displays found in Kyber's list; the setting and the number of displays (profiles, custom resolution, older Station, `max_screens`); a removed display (plan redone, silent display, “not created”); first data noted per stream; positions and pointer routed per display. The several-computers tests (`… focus`, `… gamepads`, `… session`, `… clipboard`, `… audio`, `… screens`, `… journal`), with input chains and sound engines without a network: registration, activation, “Keep sound”, four at most, unregistering (the previous one takes the hand back), a vanished session forgotten; gamepads plugged into every session, buttons and axes to the active one only, put at rest when switching computers, plugged again when a chain joins its computer, rumble from the active one only; keyboard cut in the background and keys released; the session's state taken by the chain and the sound engine of a restarted image; clipboard paused then the device's content announced; sound muted with a 20 ms fade, pulled at the same pace, back; one more session on “This display” and its 16:9 window; the computer's name in the log. The SDL thread itself is not tested here (`cargo check -p kybernow-core --features sdl`): it is checked with a gamepad, in the app.
