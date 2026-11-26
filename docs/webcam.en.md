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
| `MEDIA` 0x02 | app → computer | flags (u8: bit 0 keyframe; bits 1 and 2, the clockwise quarter turns that make the picture upright: Android sends the sensor's picture as it is), pts in µs (u64, big endian), then the encoded picture (Annex B, SPS/PPS/VPS parameters with every keyframe) |
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

## The Station tool (`server/camera`)

`kybernow-camera`, a single C program for the three systems, built against the Station's Kyber FFmpeg (`server/camera/build.sh`, like `kybernow-encoder-check`), placed next to kycontroller:

- `--uplink` (started by kycontroller): reads the packets on its standard input; one thread decodes (libavcodec: H.264 or HEVC, low delay, two slice threads) and brings the picture to the `START` resolution (720p or 1080p) keeping its ratio, centred on dark grey (libswscale); the main thread hands the latest picture to the output 30 times a second, whether a new one arrived or not (filming software wants a steady rate). Before the first picture: dark grey; paused: the last picture; at the end: one second of grey, then it stops. A picture that doesn't decode (lost reference) asks the app for a keyframe (`{"keyframe": true}`, at most once per second).
- `--probe`: `{"ready": true}` if the virtual camera would open, otherwise the reason; `--install` / `--uninstall` (Windows); `--test [720p|1080p]`: 20 s of test pattern, to try the virtual camera without an app.
- Failure to open the virtual camera: `{"state": "failed", "reason": …}`, which the app shows as “{name} could not open its virtual camera.”.

Another `START` (other camera, other resolution) restarts the tool (kycontroller stops the old one first). The `MEDIA` quarter turns are applied before resizing (YUV planes rotated). **Tried on the Mac** (26/11) with a test output that writes the pictures to a file: an ffmpeg 480 × 640 H.264 stream cut into packets as kycontroller passes them, with one quarter turn: sharp, upright picture, grey-bordered at 1280 × 720, 30 frames per second, clean end when the input closes.

## Windows Station

- **The tool** writes the NV12 picture into a named shared memory `Global\KyberNowCameraFrame` (`server/camera/windows/kn_shared.h`: one picture, a counter that is even when it is stable, the time of the last write): it runs as SYSTEM, started by kycontroller like kyavserver. Readable by everyone, low-integrity applications included; writable by SYSTEM and administrators. With no picture for 2 s, readers show grey.
- **Windows 11** (build 22000 and later): `KyberNowCameraSource.dll`, a Media Foundation source (`IMFMediaSourceEx`, `IMFMediaStream2`, `IKsControl` without properties), NV12 1280 × 720 or 1920 × 1080 at 30 frames per second, one picture every 1/30 s whatever the pace of requests. `kybernow-camera --install` registers it (COM, HKLM, `ThreadingModel` Both) then creates the camera with `MFCreateVirtualCamera` (loaded from `mfsensorgroup.dll`), “system” lifetime, for all users: Windows keeps it from one boot to the next and the Frame Server service loads the source when an app films; every app sees it (Windows Camera included). `--uninstall` removes it (`Remove`). MFPlat is loaded on demand: on a Windows N without the Media Feature Pack, the tool says so instead of failing to start.
- **Windows 10**: `KyberNowCameraFilter.dll`, a DirectShow source filter written by us (without the base classes of Microsoft's sample): a YUY2 capture pin (1280 × 720 or 1920 × 1080, 30 frames per second), `IAMStreamConfig` (both sizes), `IKsPropertySet` (capture category), timestamps on the graph clock. `--install` registers it in DirectShow's camera category (`IFilterMapper2`). Zoom, Teams, Discord, OBS, Chrome and Edge see it; apps that only read Media Foundation (Windows Camera) don't.
- Both DLLs link the MinGW runtime statically: they are loaded outside Kyber's folder (Frame Server, the app that films).
- **Installation**: `server/kyber-patches/build.sh` builds the three files into Kyber's overlay; the installer runs `kybernow-camera.exe --install` after installing the files (never blocking: one line in the log on failure), uninstalling runs `--uninstall` before removing the files.

## Mac Station

- **The extension** (`server/macos/camera-extension`, Swift, CMIOExtension): the “KyberNow Camera” device, two streams: the one apps read (NV12, 1280 × 720 or 1920 × 1080, 30 frames per second) and an input (“sink”) where the tool puts its pictures. The latest picture goes out 30 times a second, brought to the format the app chose by `VTPixelTransferSession`; with no picture for 2 s, dark grey. Sandbox, app group `P6DB682S4K.fr.leslignesclaires.kybernow.station.camera`, Mach service of the same name (`CMIOExtensionMachServiceName`).
- **The tool** (`out_macos.c`) finds the device by its name, opens its input stream (direction 1, `CMIOStreamCopyBufferQueue`, `CMIODeviceStartStream`) and puts each picture into it as a `CMSampleBuffer` (“420v” `CVPixelBuffer` on IOSurface); queue full: the picture is skipped.
- **Activation**: the menu bar icon (`server/macos/menubar/Camera.swift`) asks macOS to activate the extension at every launch (`OSSystemExtensionRequest`, replaced by the launched version's). The first time, macOS waits for consent: a notification says where to give it (System Settings › General › Login Items & Extensions › Camera Extensions). The app must be in Applications.
- **Apple rights**: installing a system extension needs `com.apple.developer.system-extension.install`, which only a profile grants. `build-station-app.sh` only puts the extension and the tool in the app (with `station-camera.entitlements` and `embedded.provisionprofile`) when it finds a Developer ID profile of `fr.leslignesclaires.kybernow.station` that allows it (`KYBERNOW_STATION_PROFILE`, otherwise Xcode's profile folders); otherwise the camera is not offered, nothing else changes. **To do once** on the Apple Developer portal (team P6DB682S4K): the `fr.leslignesclaires.kybernow.station` identifier with the “System Extension” capability, the `fr.leslignesclaires.kybernow.station.camera` identifier (no capability), then a Developer ID profile for the first one, installed on the building Mac. This right is open to Developer ID: no request to Apple to wait for, unlike the gamepads. The extension needs no profile.

## Linux Station

- **v4l2loopback** first: the `/dev/videoN` device named “KyberNow Camera” (otherwise the first one of that driver), in YUYV, written with `write()`; Zoom, Discord, Chrome, Firefox and OBS read it. The package installs `modules-load.d/kybernow-camera.conf` (loads the module at boot, if installed), `modprobe.d/kybernow-camera.conf` (`card_label="KyberNow Camera" exclusive_caps=1`: seen as a camera only while the Station writes to it) and a udev rule (`uaccess`: the user's session writes to it without being in the video group). The module isn't in distribution kernels: package `v4l2loopback-dkms` (Ubuntu, Debian; suggested by the .deb) or `akmod-v4l2loopback` (Fedora, RPM Fusion).
- **PipeWire** otherwise (tool built with libpipewire-0.3): a `Video/Source` node “KyberNow Camera” (YUY2), which only exists while the camera is sent; seen by software that reads PipeWire cameras (browsers through the Camera portal, OBS), not by Zoom or Discord.
- `server/linux/scripts/package.sh` builds the tool into `runtime/bin`, next to kycontroller (rpath to the runtime's libraries).

## The apps

| App | Capture and encoding | Interface | State |
|---|---|---|---|
| Mac, iPhone and iPad, Apple TV | `apps/macos/KyberKit/CameraCapture.swift`, shared by the three: AVFoundation (Mac: built-in, USB, iPhone through Continuity Camera; iPhone, iPad: front and back, upright picture through `RotationCoordinator`; Apple TV: Continuity Camera only), 720p or 1080p preset, 30 frames per second, not mirrored; hardware H.264 through `VTCompressionSession` (real time, Main profile, no B-frames, a keyframe every 2 s and on demand, the core's bitrate read every second, capped at 1.5 times over a second), AVCC to Annex B with SPS and PPS before every keyframe. Interruption (background): `kn_camera_pause`, keyframe on return. `SessionEngine`: `cameraOn`, `cameraOff`, `cameraSetResolution`, `onCameraOffer`, `onCameraState`, `onCameraNotice`. Bridge: `apps/apple/src/camera.rs` (`kn_camera_*`), `KnPlatform::sends_camera` and `on_camera_offer`. | Mac: bar chip and menu, ⌃⌥⌘C, capsule, statistics, setting. iPhone, iPad: bar button and menu, capsule, statistics tile, setting. Apple TV: menu tile and tvOS's picker (`continuityDevicePicker`), statistics line, capsule. Setting kept per computer (`camera1080`), offer remembered (`cameraOffered`) so that the setting only shows for computers that offer it. | written, types checked (KyberKit for macOS, iOS and tvOS, the three apps); not built in Xcode, not tried |
| Android | coming in this series | | |
| Windows | coming | | |
| Linux | coming | | |

## Tests to do

1. A Station with this patch facing an app that asks for the camera: “Uplink "camera" ready” then “Uplink "camera" connected” in kycontroller's log; without the tool next to it: “no sink on this computer”.
2. `kybernow-camera --test` on each Station: the test pattern in Windows 11's Camera app, in Zoom on Windows 10, in Photo Booth on the Mac, in `ffplay /dev/videoN` and a browser on Linux (with and without v4l2loopback).
3. A real session: the camera in Teams, Zoom, Discord, OBS; switching camera and resolution in a session; app sent to the background on iPhone (frozen picture) then back.
4. A second device turning its camera on while the first one is sending: notice « {appareil} envoie déjà sa caméra à {nom}. ».
5. Profile changed in a session, camera on: the camera restarts by itself (one keyframe, no gesture).
6. An older Station: the « Caméra » entry doesn't appear, nothing else changes.
7. Windows: install then uninstall (the camera leaves the camera list); Station update while an app films (DLL in use by Frame Server).
8. Mac: first activation (notification, consent in Settings), then app update (extension replaced).
