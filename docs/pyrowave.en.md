# PyroWave

English · [Français](pyrowave.md)

[PyroWave](https://github.com/Themaister/pyrowave) (MIT licence) is a wavelet picture format computed in Vulkan on the graphics card: every frame stands alone (no reference frame), encoding and decoding take about 0.1 to 0.25 ms, at the cost of a high bitrate (200 Mbit/s and more). It is one more format for the local network, next to H.264, HEVC and AV1, with no change to the existing streams.

Milestone 1: Windows Station (this document) and Mac client (`apps/core`, Mac decoder); then the Mac Station (Metal encoder, section [The Mac Station](#the-mac-station)) and the Linux Station (Vulkan encoder, section [The Linux Station](#the-linux-station)). The visible texts (Admin, apps) are provisional, waiting for the mock-ups.

## Shared contract

All apps and the Station follow it.

- **Codec identifier** for kymux and kycom: `0x7077_7631` ("pwv1"), next to `KYMUX_CODEC_H264`, `KYMUX_CODEC_H265` and `KYMUX_CODEC_AV1` (`kymux_sink.rs` of the libavconv engine, `packet_sink.c` of txproto). A `PyroWave` variant of the `VideoCodec` enums: `kyavservice-types`, `kyctl-common` (`"PyroWave"` in the JSON of the start request), `kyclient` (and its C API: `KYCLIENT_VIDEO_CODEC_PYRO_WAVE`, the name cbindgen gives), `kyclient-rs`.
- **Config packet** (`is_config`, the encoder's extradata), 16 bytes: `"KNPW"` | version u8 = 1 | chroma u8 (0 = 4:2:0, 1 = 4:4:4) | colour u8 (0 = SDR BT.709, 1 = HDR10 BT.2020 PQ) | range u8 (0 = full, 1 = limited) | width u32 LE | height u32 LE. Sent again on every resolution change (the encoder is then recreated, as for the other formats).
- **Media packet**: one whole frame, the concatenation of the packets of `pyrowave_encoder_packetize`, always flagged as a key frame; pts as today. The decoder takes this concatenation as is (`pyrowave_decoder_push_packet` reads the blocks one after the other). A lost frame is simply skipped: no key frame to ask for.
- **Packet boundary**: 1,152 bytes (the encoder's `packet_size` option). It only splits the stream internally: kynet always reassembles the whole media packet (RaptorQ, `video_unreliable_fec.rs`) before handing it to the client, whatever the size of its datagrams. So it is aligned on nothing on the network side (checked in kyproto's code, not measured).
- **Bitrate**: `maximum_bitstream_size = bitrate_bps / 8 / fps` (a multiple of 4), read again for every frame: a bitrate change during a session (`/set_bitrate`) applies from the next frame.
- **In SDR**: the capture's BGRA, YCbCr BT.709 **full range**. **In HDR10**: the capture already converts to P010 (PQ, BT.2020, **limited range**, patch 0022); PyroWave takes it as is, without conversion.
- **4:4:4**: SDR only (the HDR10 capture is 4:2:0).

## What the Station announces and accepts

1. **Announcement**: KyberNow Admin's `GET /api/stream/info` carries `"pyrowave": { "available": true }`, or `{ "available": false, "reason": … }` with, in this order: `off` (setting `[kyavserver] pyrowave = false`), `windows` (neither Windows, Mac nor Linux), `vulkan` (no suitable Vulkan 1.3 on the encoding graphics card, Windows and Linux), `metal` (Mac Station: unsuitable chip or encoder that does not start, [The Mac Station](#the-mac-station)). An older Station lacks this field: apps then never ask for PyroWave.
2. **Vulkan**: once per graphics card, in the background at start-up, Admin runs PyroWave's tool `pyrowave-device-validation.exe --external --roundtrip --timeout 15 --luid <card>` (Kyber's folder): Vulkan 1.3 with the features PyroWave needs, import of D3D11 textures and fences, an encode then decode round trip. A separate process: a crashing driver does not take Admin down. If no Vulkan card has that LUID (code 2), the tool runs again with the card's PCI vendor and model (`--vid`, `--pid`): Admin's service, in session 0, does not always see the card under the same LUID as Vulkan (test of 26/11/2026, RTX 3080 Ti: code 2 with Admin's LUID, 0 without). What the tool writes goes to Admin's log when the card is not suitable. A success is kept while Admin runs, a failure for 2 minutes (the driver may not be ready when the PC starts); another card gets checked in turn. Linux Station: the same tool, built for Linux, on the render node's card (`--vid`, `--pid`, see [The Linux Station](#the-linux-station)).
3. **Request**: the app asks for `codec: "PyroWave"` in the start request, with `fallback_codec` (its usual format, H.264 without it). kyclient's Rust API has `VideoSessionConfig::fallback_codec`, the C API `kyclient_video_session_config_set_fallback_codec`.
4. **Acceptance**: kycontroller keeps PyroWave only if the Station offers it (Windows, Mac or Linux, and `[kyavserver] pyrowave` true or missing), otherwise it takes the fallback format (log: "PyroWave not offered by this Station: HEVC instead"). kyavserver checks again (H.264 otherwise). The client reads the format that arrives in the stream's codec packet.
5. **Encoder**: PyroWave does not depend on `[kyavserver] encoder` (NVENC, AMF, Quick Sync or x264): the chosen H.264/HEVC encoder stays the one of the other streams.
6. **Maximum bitrate**: `pyrowave.max_mbps` (Mbit/s), present when known: Admin's “Maximum bitrate” setting (`video.pyrowave_max_mbps`, `[kyavserver] pyrowave_max_mbps` read by Admin only: `auto` by default, `300`, `500`, `800`); on Auto, from the Station's network port (`TransmitLinkSpeed` of the first card with an address, Ethernet and the gateway's card first, read again every minute): 80% of its speed on Ethernet, 50% on Wi-Fi (advertised radio rate), rounded down to 50 Mbit/s (1 Gbit/s → 800, 2.5 Gbit/s → 2000, 100 Mbit/s → 50). Unknown port: missing, no cap from the computer.

## Lowered resolution

App rule ([link-ui.en.md](link-ui.en.md), “PyroWave”): under the cap, PyroWave keeps the screen's resolution with fewer bits per pixel, down to the **floor of 0.5 bit per pixel** (0.75 in 4:4:4: ≈ 249 Mbit/s at 4K60); only below that does the app lower the resolution (then the refresh rate), and says so. The computation is in the core (`apps/core/src/pyrowave.rs`, see [apps/core/README.en.md](../apps/core/README.en.md)), the same for every app.

- **Why this floor** (measured on 26/11/2026): three real 4K frames (Tears of Steel, CC-BY), encoded by the Metal encoder and decoded by the app's decoder, compared with the source (VMAF, SSIM, PSNR). At equal bitrate, keeping the resolution beats lowering it to 1.5 bits per pixel then upscaling (Lanczos) at every step: at 4K60, mean VMAF 93.5 vs 92.3 at 250 Mbit/s (1248p), 95.2 vs 94.2 at 500 Mbit/s (1764p), 95.3 vs 94.5 at 600 Mbit/s (1932p); 95.7 at 750 Mbit/s. Film frames, fairly soft: a game with very sharp text is still to be measured. The former rule (lowering from 1.5 bits per pixel) lost sharpness.
- **Full-sharpness bitrate** (what a profile asks for when it can): width × height × refresh × bits per pixel, summed over the screens of a multi-screen session. **1.5 bits per pixel in 4:2:0** (SDR or HDR10: the top of what PyroWave gives at full sharpness, ~1.2 to 1.5); **2.25 in 4:4:4** (half more: twice the colour samples, which weigh less than luminance). That is ≈ 187 Mbit/s at 1080p60, ≈ 747 Mbit/s at 4K60, ≈ 1,120 Mbit/s at 4K60 4:4:4. Values to be confirmed on streams from the Station.
- **Cap**: the smallest of the known ones: the Station's `pyrowave.max_mbps`, the link bitrate the app gives (its port speed, or a measured bitrate), the bitrate set by hand in the app (format without a profile). None: the device's resolution stays.
- **Choice** (below the floor): the largest even resolution that fits at the floor, aspect ratio kept, at the screen's refresh rate, down to 720 lines; then 120, 100, 60, 50 or 30 Hz, again with the largest resolution that fits; nothing fits: 720p at 30 Hz. With several screens, the same scale for all, positions included (they stay edge to edge).
- **Asked of the Station**: it is the virtual display's resolution (`prepare`), at start and on every adaptation; when the cap or the format changes during a session, the virtual display is redone if the resolution must change. Without a virtual display, the Station keeps its screen: only the bitrate is capped. Kyber gets at least the bitrate this resolution needs (profile), never more than the cap.
- **Told to the apps**: notice `ResolutionLowered` (banner “Resolution lowered to {w} × {h} to fit in {n} Mbit/s.”, removed when it comes back) and `Second::lowered` (Statistics line), in the C and JNI bridges and the Windows and Linux apps.

## The Windows Station

- **Library**: `libpyrowave-shared-1.dll` (PyroWave's C API, C++ runtime linked statically, Vulkan loaded on demand by volk: no dependency on `vulkan-1.dll`, kyavserver starts on a PC without Vulkan). Sources at the commit pinned in `server/kyber-patches/pyrowave.sh` (`a32b05e`, 11/11/2026), with the partial Granite of `checkout_granite.sh` (volk, Vulkan headers), in `third_party/pyrowave`; built with MinGW from the Mac by `build.sh`.
- **FFmpeg encoder `libpyrowave`** (patch 0025-kymedia, `libavcodec/libpyrowave_enc.c`, after the `pyrowave` branch of `HansKristian-Work/FFmpeg` for the `AV_CODEC_ID_PYROWAVE` identifier): it takes the capture's D3D11 frames without any CPU copy. At start-up, a Vulkan device on the same graphics card as the capture (LUID of the DXGI adapter, `pyrowave_create_device_by_compat`, Vulkan queue priority after `gpu_priority`), a shared D3D11 fence imported as a timeline semaphore. For every frame: a copy on the graphics card of the captured texture into a shared texture (NT handle, imported once into Vulkan, recreated if the size or format changes), the fence signalled, then `pyrowave_encoder_encode_gpu_scaled`, which waits for the fence, converts and scales down if needed, and `pyrowave_encoder_packetize`.
- **Both engines**: txproto (`create_pyrowave` in `txproto/video.rs`, capture linked straight to the encoder) and libavconv (`encoder_names`, `null` filter). `encode.c` and `enc.c` need no change: the encoder declares itself hardware on D3D11 frames, they give it the capture's frames context.
- **Several displays**: one encoder per display, as for the other formats; each opens its Vulkan device on its display's graphics card.
- **A guest's own scaled-down image** (KyberNow Share): PyroWave scales the image down itself (`out_width`, `out_height` options) instead of `scale_d3d11`; rate capped by the capture (txproto), the encoder's `fps` set to it for the frame budget.
- **Setting**: `[kyavserver] pyrowave` (true by default), Admin key `video.pyrowave`, Windows and Mac (provisional texts, marked `TODO textes pyrowave`). “Maximum bitrate”: `video.pyrowave_max_mbps` (see the announcement, item 6), Windows and Mac; the mock-up's Auto help (“here {n} Mbit/s”) is still to be done in Admin's PyroWave card.
- **Overlay**: `libpyrowave-shared-1.dll` and `pyrowave-device-validation.exe` in `third_party/payload/kyber-overlay/`, hence in the installer and the updates.

## Trying it on the PC

`build.sh` also produces a test that is not shipped: `third_party/pyrowave/test-win64/` (`pyrowave-capture-test.exe`, `pyrowave-device-validation.exe` and the DLLs they need). It captures the main display as the Station does (DXGI duplication, D3D11 frames in BGRA), encodes them with the Station's `libpyrowave` encoder, writes the packets, then decodes the last frame with PyroWave's decoder to check the stream.

1. On the Mac: `server/kyber-patches/build.sh` (or just the test, if the overlay is up to date: see the "Essai PyroWave" section of the script).
2. Copy the folder `third_party/pyrowave/test-win64` to the PC, for instance to `C:\pyrowave-test`.
3. On the PC, **in the logged-in session** (display duplication does not work from a service), open PowerShell in that folder and run:
   ```
   .\pyrowave-capture-test.exe
   ```
   Expected: "Carte : …", "Écran : 3840x2160 …", FFmpeg's line "PyroWave 1.0.0 on …", "En-tête : 4b4e505701000000…" ("KNPW", version 1, 4:2:0, SDR, full range, then the size), "120 images encodées : … ms en moyenne", "Décodée : … (image plausible)", then "OK". Moving a window during the test gives different frames.
4. A guest's scaled-down image: `.\pyrowave-capture-test.exe 120 50 1280 720` (120 frames, 50 Mbit/s, 1280 × 720).
5. Open `pyrowave-test.y4m` in VLC: it is the last frame, decoded.
6. Vulkan as Admin checks it: `.\pyrowave-device-validation.exe --external --roundtrip --timeout 15; $LASTEXITCODE`: 0 expected (2: no suitable Vulkan 1.3, 4: no D3D11 import, 5: wrong round trip).

For the report: the program's whole output, and the engine set in Admin.

Then, once the Station is installed with the overlay (and with a client that knows how to ask): `GET /api/stream/info` says `"pyrowave": { "available": true }`; a PyroWave session writes "PyroWave encoder 3840x2160 4:2:0, SDR (BT.709, full) …" to kyavserver's log, and the "Timing in ms … libpyrowave" line every 5 s.

## The Mac Station

- **Library**: `libkn-pyrowave-metal.a` (`server/macos/pyrowave/build.sh`), the Metal port of PyroWave vendored in `apps/macos/PyroWave` (commit `a32b05e`, the same as the Mac app's decoder) and KyberNow's conversion (`kn_pyrowave_metal.mm`, C interface in `kn_pyrowave_metal.h`), as a static library linked into `libtxproto.dylib` (FFmpeg is static there). No new dylib in the app: only the system's Metal, IOSurface and `libc++`. `build-kyber.sh` builds it into the rootfs before FFmpeg, with `kn-pyrowave-metal.pc`, and extracts FFmpeg again when its wrap changes (as `build.sh` does on Windows).
- **FFmpeg encoder `libpyrowave_metal`** (Mac patch 0016, `libavcodec/libpyrowave_metal_enc.c`): same options and same contract as `libpyrowave`. It takes ScreenCaptureKit's VideoToolbox frames (`CVPixelBuffer` on an IOSurface) without any CPU copy; txproto hands it the capture's frames context (`encode.c` unchanged). Depending on the frame:
  - **NV12** (4:2:0 SDR at the device's resolution, the usual case): ScreenCaptureKit delivers BT.709 full-range NV12 (`pyrowave_nv12` capture option, Mac patches 0017 and 0019), which the Metal port reads as is: no conversion;
  - **BGRA** (4:4:4, or a guest's scaled-down image): a Metal compute pass converts it to BT.709 full-range YCbCr in three R8 planes on IOSurfaces (average of 2 × 2 pixels for 4:2:0 chroma; scaling by linear sampling on the way), which the port then reads;
  - **P010** (the capture's HDR10, Mac patch 0010: PQ, BT.2020, limited range, white already raised): as is; scaled down on the GPU as P010 for a guest.
  PyroWave's encoder has its own Metal queue: the conversion is waited for before it (`waitUntilCompleted`), which costs about 1 ms at 60 fps when the GPU is idle; hence the capture's NV12 in the usual case.
- **The engine**: txproto only (libavconv has no Mac capture), same `create_pyrowave` as on Windows (Mac patch 0017). kycontroller offers PyroWave on Mac (0018).
- **Several displays**: one encoder per display, all on the chip (a single GPU).
- **A guest's own scaled-down image**: scaled by the encoder (`out_width`, `out_height`), rate capped by ScreenCaptureKit (Mac patch 0008).
- **Chip**: Apple7 family or later (`pyrowave_device_is_supported`: every M chip, the only ones the Mac Station runs on). `gpu_priority` is accepted without effect (Metal has no queue priority to set).
- **Admin**: `kybernow-pyrowave-check` (next to Admin in the app, `KYBERNOW_PYROWAVE_CHECK` for another path) checks once per launch, in the background, that the chip fits and that the encoder starts (round trip of a 256 × 256 test pattern encoded then decoded); code 0 good, 2 chip, 4 encoder, 5 wrong round trip. Otherwise `reason: "metal"`. “PyroWave” and “Maximum bitrate” settings as on Windows; on Auto, the Mac's port: negotiated Ethernet speed (`ifconfig`, `media` line), or the Wi-Fi radio rate (`system_profiler SPAirPortDataType`, read at most every 5 minutes) when Wi-Fi is the port in use.
- **Package** (`build-station-app.sh`, notarized as before): `kybernow-pyrowave-check` added to `Contents/MacOS`, signed with the same entitlements as the other executables (hardened runtime; Metal asks for no other); the encoder is in `libtxproto.dylib`, already there. `run-kyber.sh package` puts the tool in Kyber's app, `run-station.sh` next to Admin. Nothing else changes for notarization.
- **kyavserver's log**: "PyroWave encoder 3840x2160 4:2:0, SDR (BT.709, full), input 3840x2160 nv12 … on Apple M4 Max (Metal)" when it opens, then every 600 frames the average size and the conversion and encode times; "Capture: NV12 BT.709 full range for PyroWave" on the capture side.

## Test on the Mac

`server/macos/pyrowave/capture-test.sh` (after `build-kyber.sh`, with the same `KYBER_DIR`): `capture-test.m` captures a display through ScreenCaptureKit as txproto does (NV12, BGRA or PQ P010), feeds the frames to the Station's `libpyrowave_metal` FFmpeg encoder at 60 fps (the latest frame, changed or not), writes the packets, then the Mac app's decoding bench (`apps/macos/tests/pyrowave/bench`, `VideoDecoder` then the Metal decoder) decodes them: times, PSNR of the first frame against what the encoder saw, truncated frame skipped. The terminal needs the “Screen Recording” permission.

Measurements of 25/11/2026 (M4 Max, macOS 27.2, main display captured at the requested resolution, 1.2 bits per pixel at 60 fps, other builds running on the Mac): the Station's time per frame, from handing the frame to the encoder to the packet being ready (mean, 95th percentile), and paced decoding by the Mac app.

| Case | Encode | Decode (Mac app) | PSNR Y / Cb / Cr |
|---|---|---|---|
| 1440p 4:2:0 SDR, capture's NV12 | 1.7 ms (1.8) | 1.2 ms | 52 / 73 / 73 dB |
| 1440p 4:2:0 SDR, converted BGRA | 3.0 ms (5.1) | 1.7 ms | 51 / 66 / 65 dB |
| 1440p 4:4:4 SDR | 3.0 ms (3.7) | 1.7 ms | 51 / 65 / 63 dB |
| 1440p 4:2:0 HDR10 (P010) | 1.7 ms (2.3) | 1.2 ms | 57 / 57 / 57 dB |
| 4K 4:2:0 SDR, capture's NV12 | 2.5 ms (4.3) | 2.4 ms | 61 / 83 / 82 dB |
| 4K 4:2:0 SDR, converted BGRA | 4.2 ms (6.0) | 2.2 ms | 61 / 67 / 67 dB |
| 4K 4:4:4 SDR | 5.4 ms (7.1) | 2.5 ms | 61 / 67 / 64 dB |
| 4K 4:2:0 HDR10 (P010) | 2.2 to 5.2 ms (depending on the run) | 1.5 to 2.1 ms | 60 / 57 / 56 dB |
| 1080p guest (4K scaled down), SDR / HDR10 | 2.3 / 2.8 ms | 1.1 / 1.2 ms | — |

The PSNR compares with the YCbCr the encoder received (computed separately for BGRA): it measures PyroWave at that bitrate, not the conversion. Breakdown for BGRA at 4K 4:2:0 (encoder log, 600 frames): conversion 1.9 ms, of which 0.1 to 0.3 ms of GPU, the rest being the wait between the two queues and the GPU waking up; encode 2.3 ms. Back to back (GPU at full clock), the same frame: conversion 0.7 ms, encode 1.1 ms. Bitrate kept: every frame fills its budget (`bitrate / 8 / fps`) within a few bytes. Admin's check: "PyroWave sur Apple M4 Max : 24528 octets, PSNR 55.2 dB", code 0.

**Real Mac Station → Mac app session**: not done (the author's installed Station and a test Mac app were running on this Mac). Steps:

1. Kyber with PyroWave: `KYBER_DIR=<private copy> server/macos/scripts/build-kyber.sh` (a copy of `third_party/kyber-desktop-macos` made with `cp -cR`, without its `rootfs-arm64-macos` nor `kysdk/kymedia/builddir-macos`).
2. Station: `KYBER_DIR=<private copy> server/macos/scripts/build-station-app.sh` (Developer ID signed, not notarized), then install `server/macos/dist/KyberNow Station.app` with the .dmg's installer (or, without touching the installed Station, a test Station: stop the installed Admin with `launchctl bootout gui/$(id -u)/fr.leslignesclaires.kybernow.station.admin`, then `KYBER_DIR=<private copy> server/macos/scripts/run-station.sh`, which puts `kybernow-pyrowave-check` next to Admin).
3. Check the announcement: `curl -sk -u <account> https://<mac>:<Admin port>/api/stream/info | jq .pyrowave` → `{"available": true, "max_mbps": …}`; Admin's log: "PyroWave : la puce convient".
4. Mac app: built with the kyclient 0025 patch applied and the core's `pyrowave` feature (see [pyrowave-client-mac.en.md](pyrowave-client-mac.en.md)), on a device connected by cable or with “Video format” set to PyroWave.
5. Expected in kyavserver's log (in `~/Library/Logs/KyberNow`, or the test Station's `kyber/log/kyber.log`): "Capture: NV12 BT.709 full range for PyroWave", "PyroWave encoder … (Metal)", then every 600 frames "PyroWave: 600 frames … conversion 0.00 ms, encode … ms". Also try 4:4:4 (sharp text), HDR10, two displays and a Share guest.

## The Linux Station

Kyber Linux patches 0010 to 0012 ([kyber-changes.en.md](kyber-changes.en.md)), with the libavconv engine (PipeWire GStreamer capture, [station-linux.en.md](station-linux.en.md)).

- **Library**: `libpyrowave-shared.so` and `pyrowave-device-validation`, from the same sources at the same commit as on Windows, built for the machine (x86_64 or arm64) by `server/linux/scripts/pyrowave-linux.sh`, which `build-kyber.sh` runs before FFmpeg; installed in Kyber's rootfs (`lib/`, `bin/`), hence in the package (`runtime/`). Only `pyrowave.h` is installed: FFmpeg and PyroWave build with Kyber's Vulkan headers (1.4). Needs `cmake` (and `ninja`, when present).
- **FFmpeg encoder `libpyrowave_vulkan`** (Linux patch 0010, `libavcodec/libpyrowave_vulkan_enc.c`): same options, same header and same packets as `libpyrowave`. It opens a Vulkan device on the render node's card (`render_node`, `/dev/dri/renderD128` by default, `KYBERNOW_RENDER_NODE` for another: PCI vendor and device read in `/sys/class/drm`). Depending on the frame the capture gives:
  - **DMA-BUF** (DRM PRIME, the usual case with a compositor on a real card): PipeWire's buffer is imported as is in Vulkan (`VK_EXT_external_memory_dma_buf`, `VK_EXT_image_drm_format_modifier`, modifier and plane layout given by the capture), once per buffer of PipeWire's pool (eight kept, recognised by their inode). The compositor's rendering is waited for through the buffer's sync file (`DMA_BUF_IOCTL_EXPORT_SYNC_FILE`, kernel 6.0 and later, imported as a semaphore); without it, the driver's implicit sync. PyroWave converts to BT.709 full-range YCbCr and scales a guest's image down on the card (`encode_gpu_scaled`). Formats: XRGB, ARGB, XBGR, ABGR in 8 and 10 bits (16-bit intermediate plane); NV12 and multi-object DMA-BUFs are refused.
  - **Frames in memory** (BGRx, RGBx, BGRA, RGBA: a capture without DMA-BUF, as in a virtual machine): converted to BT.709 full-range YCbCr by the CPU (nearest scaling for a guest), then encoded by the card (`encode_cpu`). Slower, but the same picture.
  - **SDR only**: there is no HDR capture on a Linux Station yet (see [station-linux.en.md](station-linux.en.md), “HDR”).
- **The engine**: libavconv (`encoder_names`, `null` filter: the capture reaches the encoder as is), as on Windows; kycontroller offers PyroWave on Linux (Linux patch 0012), kyavserver accepts it (0011).
- **Several displays**: one encoder per display, each on the same card.
- **Priority**: `gpu_priority` asks for a high or realtime Vulkan queue; Linux only grants it with `CAP_SYS_NICE`, otherwise PyroWave stays at medium priority (the log says so: “queue priority 1 (got 0)”).
- **Admin**: `pyrowave-device-validation --external --roundtrip --timeout 15 --vid <vendor> --pid <device>` (`runtime/bin` next to Admin, `KYBERNOW_PYROWAVE_VALIDATION` for another path), once per launch, in the background, stopped after 20 s: suitable Vulkan 1.3, import of DMA-BUFs (with modifiers) and sync files, round trip. Otherwise `reason: "vulkan"`, same texts as on Windows. “Maximum bitrate” on Auto: the Station's port, when Admin knows it on Linux.
- **kyavserver's log**: “PyroWave 1.0.0 on /dev/dri/renderD128 (1002:744c, Vulkan) …”, “PyroWave encoder 3840x2160 4:2:0, SDR (BT.709, full), input 3840x2160 bgr0 (DMA-BUF) …”, then in verbose mode “DMA-BUF imported: XR24, modifier 0x…” once per buffer and a frame's size every 600.

Not tried yet: the VM has neither a card that runs PyroWave in Vulkan (virgl) nor an importable PipeWire DMA-BUF. To try on a Linux PC with an AMD, Intel or NVIDIA card (see “Test status”).

## Test status

| What | Status |
|---|---|
| `libpyrowave-shared-1.dll` with MinGW from the Mac | built (25/11/2026), no dependency on `vulkan-1.dll` |
| FFmpeg with the `libpyrowave` encoder | built (`--enable-libpyrowave`) |
| Groundwork (kyavservice, kycontroller, kyclient, txproto) and overlay | full `build.sh` from the origin (25/11/2026); kycontroller (149) and Admin (321) tests passed on the Mac; kyavservice tests (`pyrowave.rs`, `hdr.rs`, `config.rs`) built for Windows, not run |
| Encoding a frame captured as D3D11 on a PC | **to try** (previous section) |
| A whole session to the Mac | to try, with the Mac decoder |
| HDR10 (P010), 4:4:4, a guest's scaled-down image in a session, several displays, libavconv engine | to try |
| Lowered resolution (core) and “Maximum bitrate” (Admin) | unit tests passed on the Mac (25/11/2026: core 357, Admin 323); never seen in a session (the core needs the kyclient patch); port speed read by Windows never tried |
| Mac Station: `libpyrowave_metal` encoder and Kyber built | `build-kyber.sh` on a private copy (25/11/2026); kyavservice tests on the Mac (33 of 34: the failure, `rtp_start_audio_error_reports_audio_pipeline_type`, waits for an RTP audio event, unrelated) and kycontroller (111), Admin (324) |
| Mac Station: encoding a real capture, decoded by the Mac app | **done** (25/11/2026, [Test on the Mac](#test-on-the-mac)): 1440p and 4K, 4:2:0 and 4:4:4, SDR and HDR10, 1080p guest |
| Linux Station: PyroWave, `libpyrowave_vulkan` encoder, Admin | **written** (25/11/2026): encoder checked by compiling on the Mac (syntax, kernel and FFmpeg headers), patches dry-run on Kyber's sources, Admin tests passed on the Mac; neither built nor tried on Linux. To try: `build-kyber.sh`, then `pyrowave-device-validation --external --roundtrip` (exit 0), `GET /api/stream/info` (`pyrowave.available`), a PyroWave session from the Mac app (DMA-BUF, 4:4:4, guest, two displays) |
| Mac Station: whole session, several displays, installed Admin | to try (steps above); port speed read on the Mac tried by the tests only |
