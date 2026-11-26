# Device camera to the computer

English · [Français](webcam.md)

The device's camera (phone, tablet, Mac, PC, USB webcam, iPhone next to the Apple TV) goes to the computer, which sees it as an ordinary webcam, “KyberNow Camera”: Teams, Zoom, Discord, OBS and browsers offer it in their list. The visible rules, for every app, are in [link-ui.en.md](link-ui.en.md), “Device camera”. Here: how it is built and where testing stands.

## The author's choices (26/11/2026)

- Apps: Mac, iPhone and iPad, Android, Windows, Linux (desktop, Steam Deck), Apple TV (Continuity Camera). Not the web client (later), LG TVs or the Raspberry Pi.
- The host and the friend borrowing the computer; never the guests of a share.
- On demand, off at the start of every session; an indicator while it sends.
- Computer setting “Camera resolution”: 720p (default) or 1080p, 30 frames per second.
- List of cameras in the menu, the last one chosen kept.
- App in the background (iPhone, iPad, Android): paused, the computer keeps the last picture.
- Windows Station: Windows 11's virtual camera (`MFCreateVirtualCamera`); on Windows 10, our own DirectShow filter.
- Linux Station: v4l2loopback, otherwise a PipeWire camera.

## The path

```
app: capture → hardware encoder (H.264, HEVC) → core (camera.rs) → kyclient (uplink)
  ══ QUIC, a data stream of its own ══►
Station: kycontroller (uplink.rs) → kybernow-camera (standard input) → decoding → virtual camera
```

### The uplink (patch 0085, shared with the microphone)

Kyber can already open “data endpoints” (kyproto's `register_data_endpoint`): a reliable QUIC stream each way, apart from those of the picture, the sound and the inputs. What goes up therefore never gets ahead of the mouse nor waits for it, and the camera and the microphone don't wait for each other.

- **When the picture starts**, the app says what it can send: `uplink: ["camera"]` in the `/kymux/start_session` request (beside kyctl-common's `StartRequest`, like `hdr`: an older app sends nothing). For each kind whose tool (“sink”) sits next to kycontroller (`kybernow-camera`, `kybernow-micro`), and never for a share guest, the Station prepares a data endpoint and returns it in the response: `uplink: [{"kind": "camera", "endpoint": 12}]`. An older Station answers without `uplink`: the « Caméra » entry doesn't appear.
- **kyclient** connects to each endpoint returned and hands the app an `Uplink` (`Client::uplink("camera")`): `try_send` (never waits: 24 packets queued at most, beyond that `Full`), `try_recv` for the answers. It closes when the picture stops or restarts; the core then turns the camera back on over the new stream.
- **The packets** (the first byte says which):

| Type | Direction | Content |
|---|---|---|
| `START` 0x01 | app → computer | JSON: `{"codec": "h264", "width": 1280, "height": 720, "fps": 30, "resolution": "720p", "device": "iPhone de Marie"}`. Also to change camera or resolution without turning off. |
| `MEDIA` 0x02 | app → computer | flags (u8: 1 keyframe), pts in µs (u64, big endian), then the encoded picture (Annex B, SPS/PPS/VPS parameters with every keyframe) |
| `PAUSE` 0x04 | app → computer | nothing: the computer keeps the last picture |
| `STOP` 0x03 | app → computer | nothing: the tool stops |
| `STATUS` 0x10 | computer → app | a JSON line: `{"state": "started"}`, `{"state": "busy", "device": "Pixel 9"}`, `{"state": "failed", "reason": "…"}`, `{"state": "stopped"}`, `{"keyframe": true}` |

- **One device at a time** per kind on the computer: a second one gets `busy`, with the first one's name.
- **kycontroller** starts the tool on the first `START` (`kybernow-camera --uplink`, next to its executable, or in `KYBERNOW_SINKS_DIR`) and passes it `START`, `MEDIA` and `PAUSE` on its standard input, each preceded by its length (u32, big endian). What the tool writes as JSON on its output (one line per message) goes back to the app as `STATUS`. Its input closes on `STOP`, when the app leaves or when the session ends: it stops by itself (killed after 3 s otherwise). A slow tool slows the writing, so the stream, so the app's queue, which drops frames: nothing piles up.

### The core (`apps/core/src/camera.rs`)

`Session::camera()` returns the session's `CameraLink`; the app calls it from any thread. `Platform::sends_camera()` (true for an app that can capture) makes the core ask for the camera at every picture start; `Platform::on_camera_offer(bool)` says when the entry appears or disappears.

- `start(CameraSettings)` sends `START`; `frame(data, pts_µs, key)` sends a picture and returns `Sent`, `WantKeyframe` (dropped: the next one must be a keyframe) or `Off`; `pause()`, `stop()`; `poll()` returns the answers (`Started`, `Busy`, `Failed`, `Stopped`, `KeyframeWanted`, `Offer`).
- After a dropped frame, nothing goes before a keyframe (the next ones would depend on it), and the target bitrate drops by a quarter (at most once per half second, never under 300 kbit/s); it rises by 10% every 5 s without loss, up to the starting bitrate: 1.5 Mbit/s in 720p, 4 Mbit/s in 1080p, half via the relay (`target_bitrate()`, to give to the encoder).
- Picture restarted (profile changed, recovery): a camera that is on sends `START` again on the new stream and restarts from a keyframe, without a gesture.
- Texts of refusals and of the « Caméra » statistics line: `event_notice`, `device_notice`, `stats_line` (core catalogues, `camera` section).

## Windows Station

Coming in this series: the `kybernow-camera.exe` tool (H.264/HEVC decoding by FFmpeg, NV12 picture in a named shared memory); Windows 11: a COM media source registered by the installer, camera created by `MFCreateVirtualCamera` (“system” lifetime, for all users) and read by Frame Server; Windows 10: a DirectShow source filter written by us, read in the process of the app that films.

## Mac Station

Coming: a CoreMediaIO camera extension in the Station app, fed through its “sink” stream.

## Linux Station

Coming: v4l2loopback (`/dev/videoN`, “KyberNow Camera”), otherwise a PipeWire `Video/Source` node.

## The apps

Coming: capture and hardware encoding in each app.

## Tests to do

1. A Station with this patch facing an app that asks for the camera: “Uplink "camera" ready” then “Uplink "camera" connected” in kycontroller's log; without the tool next to it: “no sink on this computer”.
2. A second device turning its camera on while the first one is sending: notice « {appareil} envoie déjà sa caméra à {nom}. ».
3. Profile changed in a session, camera on: the camera restarts by itself (one keyframe, no gesture).
4. An older Station: the « Caméra » entry doesn't appear, nothing else changes.
