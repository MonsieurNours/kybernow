# Patches for Kyber 0.29

Hello,

We run Kyber 0.29 as the streaming host of a Windows PC, with our own clients (desktop, mobile, TV and web) built on kyclient and kyclient_wasm. Along the way we fixed a few defects in Kyber itself. These patches are the ones we believe are useful to every Kyber user, rewritten for upstream: one subject per patch, no change of default behaviour unless the old behaviour is a plain bug, and new behaviours behind options that default to off.

We are happy to sign the CLA, to rework anything, and to split or merge patches the way you prefer.

## Base

Kyber 0.29.0: kyber-desktop `0cf0d03` (tag `0.29.0`), kysdk `ebca32a`. Each file carries the commit it applies to (`base-commit:`), and applies with `git am`:

| Folder | Repository | Base |
|---|---|---|
| `txproto/` | kyber/deps/txproto | `7f84b9e` |
| `libavconv/` | avconv/avconv (kymedia `subprojects/libavconv`) | `5ca80cca50` |
| `kymedia/` | kyber/core/kymedia | `b0efdfa` |
| `kyctl/` | kyber/core/kyctl | `92c632f` |
| `kynput/` | kyber/core/kynput | `7292443` |
| `kyber-desktop/` | kyber/apps/kyber-desktop | `0cf0d03` |

## The patches

**Audio timestamps (bug).** WASAPI packets were stamped with their arrival time. Packets read in a burst got almost the same timestamp, and audio went backward in time for the encoder and the player ("Queue input is backward in time"): choppy, distorted sound. Both engines now timestamp from the sample count, resynchronised on the QPC capture clock that WASAPI reports. Silent packets are sent as zeros (their content is undefined).
- `txproto/0001-wasapi-timestamp-audio-from-the-sample-count.patch`
- `libavconv/0001-avgrab-wasapi-timestamp-audio-from-the-sample-count.patch` (also adds `CAPTURE_SEND_PROVIDER_PTS`, for the "allow using provider timestamps" TODO)

**Frame size of the libavconv audio stream (bug).** The kymux sink of the libavconv engine announces `frame_size: 0`; the unreliable audio protocols assert it is not zero, so a client asking for unreliable audio stops as soon as audio starts.
- `kymedia/0001-kyavservice-avconv-announce-the-audio-frame-size-to-.patch`

**Opus channel layouts (bug).** libopus only takes the Vorbis order for each channel count. Windows' usual "5.1 Surround" (side channels), quadraphonic with side channels, 2.1… made the encoder fail to open: no sound at all. The txproto encoder now keeps a layout libopus takes, resamples other 5.1 and 7.1 to the Vorbis ones (side to back), and downmixes anything else to stereo.
- `txproto/0005-encode-fall-back-to-a-channel-layout-libopus-takes.patch`

**Unplugged display (bug).** When the captured output goes away, DXGI capture retried `DuplicateOutput` every 100 ms forever: the stream froze silently. It now stops with `SP_EVENT_ON_ERROR` once the output scan has marked the entry unplugged, so the application can tell the client and restart elsewhere.
- `txproto/0004-dxgi-stop-with-an-error-when-the-captured-output-is-.patch`

**Pointer shape that cannot be read (bug).** When `GetFramePointerShape()` failed, the cursor update was dropped and its buffer leaked. It is now queued without the shape.
- `txproto/0002-dxgi_cursor-keep-the-pointer-update-when-its-shape-c.patch`

**Pointer hide delay (option, off by default).** Windows hides the pointer for a few frames at each key press or shape change; clients blink and switch mouse modes back and forth. `[kyavserver] cursor_hide_delay_ms` (0 by default) only forwards a hide once the pointer has stayed hidden that long; we use 250 ms. Also fixes two small bugs met on the way (a stale error code in a log, and `err = f() < 0`).
- `txproto/0003-dxgi-add-a-cursor_hide_delay-option.patch` (DXGI capture option)
- `kymedia/0002-kyavservice-txproto-add-a-cursor_hide_delay_ms-setti.patch` (setting, needs the txproto patch; an older txproto ignores the option)
- `kyber-desktop/0001-kyber_config.toml-document-cursor_hide_delay_ms.patch`

**Keyframe on request.** With intra refresh, the encoder never sends a keyframe on its own: a client whose decoder lost a reference has no quick way back (VideoToolbox, for example, rejects every following frame). `POST /force_idr` forces one on the session's streams, with the same scope as `/set_bitrate`, at most one per session every 150 ms.
- `kyctl/0001-kycontroller-add-POST-force_idr.patch`

**Cross build of kynputserver (bug).** Its build script tested the build machine instead of the target, so cross-compiling for Windows from a Mac failed.
- `kynput/0001-kynputserver-only-build-the-macOS-helper-for-a-macOS.patch`

## Testing

All patches apply on 0.29.0 with `git am` and build for Windows from macOS (MinGW-w64 14, Rust 1.89). Equivalent versions of the audio timestamps (both engines), the libavconv frame size, `/force_idr` and a fixed 250 ms hide delay run in our own builds. Not run on hardware yet in this exact form: the unplugged-display error, the Opus layout fallback and the hide delay as an option (same logic as our fixed delay).

## Not included, happy to discuss

- **Datagram send queue.** quinn drops the oldest datagrams once its send queue (1 MiB) is full; a 4K keyframe with its FEC overflowed it while pacing spread it out. We made the queue size configurable and are measuring deep (8 MiB) against short (3 MiB) queues on a saturated Wi-Fi before proposing anything: the deep queue can hold seconds of stale frames.
- **Pointer moves.** We forward pointer moves (position only, up to 120 per second) so that a client in relative mode can draw the host pointer, and we stopped the input server from putting the full bitmap back into every position packet. Proposing it needs a small change in the desktop client first: `windows_statemachine.rs` keeps the last packet as its active cursor, so a position without a shape would make it forget the shape to reapply after a click release.
- **Web engine.** Unicode text input from `beforeinput`/`compositionend`, release of held inputs on `blur`/hidden tab/lost pointer lock, absolute positions while the pointer is not locked, gamepads without the standard mapping, the `requestAnimationFrame` loop that outlives the engine, and decoder recovery in kywebplayer. We ship them in our build and can send them once they have run for a while.
- **Opus settings.** Bitrate by channel count with presets, 10 ms frames and `lowdelay`: useful, but they change the defaults, so we would rather offer them as options.
- **Bitrate split between displays.** kycontroller splits a multi-display session's bitrate evenly; we split it by each display's pixels per second (width × height × refresh rate), evenly when a mode is unknown, at start and on `/set_bitrate`. Neither `StartRequestVideo` nor the display list carries a refresh rate, so we read it from Windows (`EnumDisplaySettingsW`, matched on the desktop position); upstream, a `refresh_rate` in the avservice's display list would be cleaner. It changes the default split, so it would go behind an option.

Thank you for Kyber.
