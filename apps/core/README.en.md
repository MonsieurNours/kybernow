# KyberNow client core (`kybernow-core`)

English · [Français](README.md)

What KyberNow's clients have in common, written once in Rust: talking to the console, the Kyber session, profiles and bitrate, latency, updates, and since milestone 2 the shared input module and the sound engine of the Apple apps. The Windows app ([apps/windows](../windows/README.en.md)) is the first client using it; the Mac and Apple TV apps ([apps/macos](../macos/README.en.md), [apps/tvos](../tvos/README.en.md)) are migrating to it, milestone 1 (the session, `apps/apple` crate and C bridge) first, then inputs and sound, which Windows adopts in the same milestone in place of its `input.rs` and `audio.rs`.

The core depends on no system: nothing in it calls AppKit, UIKit, Win32 or Core Audio. What the platform provides goes through traits (the window, decoders, sound output and inputs through `session::Platform`; the clock, pasteboard and callbacks to the interface through the input module's traits; the sound output pulls sound from the engine itself). Kyber 0.29's building blocks come from `third_party/kyber-desktop/kysdk` with KyberNow's patches (`apps/macos/kyber-patches`), see [apps/windows/README.en.md](../windows/README.en.md).

## Code layout

| File | Contents |
|---|---|
| `src/console.rs` | Console client (`info`, `prepare`, `heartbeat`, `foreground`, `end`), pinned certificate, `Accept-Language` |
| `src/i18n.rs` | Language (detection, choice), text catalogues, release notes per language ([docs/i18n.en.md](../../docs/i18n.en.md)) |
| `src/session.rs` | A session's flow on a dedicated thread; what the platform provides: `Platform` |
| `src/media.rs` | Image and sound packets delivered by Kyber (patch 0001), Opus header |
| `src/profile.rs` | Profiles, format, bitrate following pixels per second |
| `src/latency.rs` | End-to-end latency, Kyber's clock |
| `src/update.rs` | Updates: signed manifest (minisign), channel, download |
| `src/input/` | Shared input module (below) |
| `src/audio/` | Sound engine (below) |
| `build.rs` | libopus through pkg-config when it knows it; otherwise the executable links it itself |

## Shared input module (`src/input/`)

The Rust port of what KyberKit held in `InputPipeline`, `PacketQueue`, `Keymap`, `PadCombo`, `ReleaseAll`, `PointerLayer` and `ClipboardBridge` (see “Shared input module” in [apps/macos/README.en.md](../macos/README.en.md)), on top of Kyber's kynput chain (`kynput_rs`, `kynput_sys`). Same thresholds, same delays, same rules. Not wired into the apps yet.

| Module | What it holds |
|---|---|
| `clock.rs` | `Clock`: the platform's monotonic clock; `MonotonicClock` (`Instant`), `ManualClock` (tests) |
| `packet.rs` | `OwnedPacket`: a kynput packet whose reference we hold (released at the end, cloned per send attempt, handed to kynput); `PacketSink`: sending into the chain, `KynputSink` (a kynput chain's input sink) |
| `packet_queue.rs` | `PacketQueue`: sending to kynput, refusals counted (the first, then every 100, then the total at the end), fallback queue of the packets that cannot catch up (keys, clicks, buttons, plugs, returns to rest, text, FlushKeys) retried every 3 ms in order until success or 200 ms; “once” packets (position, move, wheel, moving axis) do not leave while it is not empty |
| `keymap.rs` | HID usages → scancodes (`kynput_keycode_usb_to_kyber`), “⌘ sent as Ctrl”, swap of the `@#` and `<>` keys of Apple ISO keyboards (the layout is only asked for those two keys), Caps Lock and Num Lock recognised as toggles, ISO setting for Apple or unknown keyboards |
| `pad_combo.rs` | `PadCombo`: L1 + R1 + Start + Select held 250 ms, held buttons per gamepad, known gamepads, the six axes to bring back to rest on capture (`PadAxis::ALL`), navigation keys; `PadButton`, `PadAxis` in kynput's vocabulary, with their packet values (`kynput_rs` confuses the right stick with the right shoulder in `GamepadButton::to_u32`, so it is bypassed) |
| `release_all.rs` | `ReleaseAll`: what the device holds down (scancodes, buttons, modifiers in the platform's identifier), release everything (buttons then keys), forget, modifiers really held sent again on return |
| `pointer_layer.rs` | `PointerLayer`: position, shape (ARGB pixels as kynput delivers them, `Shape::premultiplied` for CGImage and Win32), hot spot and visibility of the computer's pointer; position held by the device (Apple TV, `holds_position`) that the computer takes back when inputs are idle (0.4 s), screen size change, resend after reconnection; `MouseMode`, `MouseModeRules` (Auto / Desktop / Game, 0.6 s tolerance for a pointer shown again, recheck at 0.7 s); drawing scale and frame |
| `clipboard.rs` | `ClipboardBridge` on the `Pasteboard` trait: text both ways (announce, immediate request, answer, 60 KB), the device's pasteboard looked at twice a second, our own write does not go back; C functions of patch `kyber-patches/0002` |
| `pipeline.rs` | `InputPipeline`: assembles everything on top of a kynput chain: keyboard, absolute and relative mouse, wheel, Unicode text, FlushKeys, gamepads produced by the app (plug, buttons, axes filtered to 1/512 of the travel and grouped by 4 ms, returns to rest retried, reset to rest), release everything; reception of the pointer (shape deduplicated, fully transparent shape discarded), rumble and clipboard |

**Timers.** No module creates a thread or a queue: each exposes `poll()` and `next_deadline()`, and `InputPipeline::poll` calls them all (retries, axes, combo, clipboard, received packets). The platform calls `poll()` by the deadline at the latest from whichever queue it wants, or lets `InputPipeline::spawn_driver` run a thread (`kybernow-entrees`) that sleeps until the deadline or a wake-up (`wake()`, triggered by anything that creates a wait). The tests advance a `ManualClock` and call `poll()` themselves.

**Threads.** Every method can be called from any thread (state under locks). kynput's callbacks arrive on its network thread: the pointer goes to `PointerLayer`, which notifies the view outside its lock; rumble to `InputPlatform::rumble`; clipboard packets are copied and handled by `poll()`, outside the router's lock (the answer would re-enter it, which the Mac app already avoided with its queue).

### What the platform provides

```rust
// Monotonic clock (CLOCK_UPTIME_RAW, QueryPerformanceCounter…).
pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
}

// Text pasteboard (NSPasteboard, UIPasteboard; None on tvOS).
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
}

// Substituted sending (tests, benches).
pub trait PacketSink: Send + Sync {
    fn consume(&self, packet: OwnedPacket) -> bool;
}
```

And, outside the traits: **event capture** (AppKit, GameController, Win32, Raw Input) translated into `InputPipeline` calls (`key`, `mouse`, `mouse_move`, `mouse_button`, `wheel`, `type_text`, `gamepad_*`, `release_all`, `flush_keys`); each key's **HID usage** and the **ISO layout** (`keymap::scancode(hid, command_as_control, || iso)`); **drawing** the pointer on the main thread from `PointerLayer::set_on_changed`; the **mouse mode rule** (`MouseModeRules { needs_hidden_pointer }`, `true` on Mac and Windows, `false` on Apple TV); the Mac's and Windows' SDL gamepads stay read by kynput (the router hook filtering them, `GamepadGate.swift`, is not ported: it stays in the app, or comes with the wiring).

Creation: `InputPipeline::new(Options { display_id, host_size, clipboard, gamepads_from_app }, pointer, platform, clock)` returns an `Arc`; `kynput()` gives the chain to `session::Platform::input_pipeline`; `start_clipboard()` once the network is connected; `stop()` at the end of the session (direct FlushKeys, total losses logged), `shutdown()` to stop kynput's handlers.

## Sound engine (`src/audio/`)

The port of `AudioPlayer.swift` without the output: the platform (Core Audio, WASAPI) pulls sound from the engine from its real-time thread.

| Module | What it holds |
|---|---|
| `opus.rs` | libopus multistream: `Decoder` (stereo, 5.1, 7.1 from the `OpusHead` header of `media.rs`), concealment frame; an encoder for the tests |
| `ring.rs` | Sample ring with one producer (network thread) and one consumer (real-time thread), lock-free |
| `mix.rs` | Downmix (L + 0.7 C + 0.7 left surround and left rear, same on the right, LFE ignored; 7.1 → 5.1; mono = average) and `OutputOrder` (Opus, Windows, Core Audio 5.1 A / 7.1 C, or custom) |
| `engine.rs` | `AudioEngine`: `configure` / `decode` / `conceal` (or `PacketHandler::packet`) from Kyber's network thread, `fill(&mut [f32], frames)` from the real-time thread; adaptive buffer; `AudioStats` once a second |

Buffer: 8 ms margin at the start, growing by 5 ms after an empty buffer (60 ms at most) and coming back down by 1 ms every 2 s without a gap (4 ms at least); first start at the margin plus one packet, resume as soon as the margin is there; an empty buffer is filled with an Opus concealment frame (silence if the decoder is busy with the network thread at that instant); drift caught up by one sample skipped or doubled out of 256 (out of 128 beyond 6 ms of difference) until the second's lowest level is back at the margin; hard cut only beyond 10 ms ahead. A full buffer (output stopped for a second) drops what arrives; Swift kept the most recent, and the hard cut catches up on restart either way.

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

The input tests take over those of `apps/macos/tests/inputs` (keyboard table, ISO, Caps Lock; short, held and fully released combo, setting off; fallback queue: accepted, refused twice then sent in order, refused endlessly, stop; axis filtering and grouping, reset to rest, release everything, text, deduplicated pointer, clipboard both ways), the sound tests the one of `apps/macos/tests/audio` (a 440 Hz A encoded in Opus, stereo and 5.1 at real pace with ± 8 ms of jitter and one loss in 97, 10 ms packets, 0.2% drift both ways, output stalled 200 ms then hard cut, downmix and channel order), with a `ManualClock` advanced by hand: they last a second. `cargo test -p kybernow-core input` or `… audio` for one half.
