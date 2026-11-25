# PyroWave

English · [Français](pyrowave.md)

[PyroWave](https://github.com/Themaister/pyrowave) (MIT licence) is a wavelet picture format computed in Vulkan on the graphics card: every frame stands alone (no reference frame), encoding and decoding take about 0.1 to 0.25 ms, at the cost of a high bitrate (200 Mbit/s and more). It is one more format for the local network, next to H.264, HEVC and AV1, with no change to the existing streams.

Milestone 1: Windows Station (this document) and Mac client (`apps/core`, Mac decoder). The visible texts (Admin, apps) are provisional, waiting for the mock-ups.

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

1. **Announcement**: KyberNow Admin's `GET /api/stream/info` carries `"pyrowave": { "available": true }`, or `{ "available": false, "reason": … }` with, in this order: `off` (setting `[kyavserver] pyrowave = false`), `windows` (Mac Station), `vulkan` (no suitable Vulkan 1.3 on the encoding graphics card). An older Station lacks this field: apps then never ask for PyroWave.
2. **Vulkan**: once per graphics card, in the background at start-up, Admin runs PyroWave's tool `pyrowave-device-validation.exe --external --roundtrip --timeout 15 --luid <card>` (Kyber's folder): Vulkan 1.3 with the features PyroWave needs, import of D3D11 textures and fences, an encode then decode round trip. A separate process: a crashing driver does not take Admin down. The result is kept while Admin runs; another card gets checked in turn.
3. **Request**: the app asks for `codec: "PyroWave"` in the start request, with `fallback_codec` (its usual format, H.264 without it). kyclient's Rust API has `VideoSessionConfig::fallback_codec`, the C API `kyclient_video_session_config_set_fallback_codec`.
4. **Acceptance**: kycontroller keeps PyroWave only if the Station offers it (Windows and `[kyavserver] pyrowave` true or missing), otherwise it takes the fallback format (log: "PyroWave not offered by this Station: HEVC instead"). kyavserver checks again (H.264 otherwise). The client reads the format that arrives in the stream's codec packet.
5. **Encoder**: PyroWave does not depend on `[kyavserver] encoder` (NVENC, AMF, Quick Sync or x264): the chosen H.264/HEVC encoder stays the one of the other streams.
6. **Maximum bitrate**: `pyrowave.max_mbps` (Mbit/s), present when known: Admin's “Maximum bitrate” setting (`video.pyrowave_max_mbps`, `[kyavserver] pyrowave_max_mbps` read by Admin only: `auto` by default, `300`, `500`, `800`); on Auto, from the Station's network port (`TransmitLinkSpeed` of the first card with an address, Ethernet and the gateway's card first, read again every minute): 80% of its speed on Ethernet, 50% on Wi-Fi (advertised radio rate), rounded down to 50 Mbit/s (1 Gbit/s → 800, 2.5 Gbit/s → 2000, 100 Mbit/s → 50). Unknown port: missing, no cap from the computer.

## Lowered resolution

App rule ([link-ui.en.md](link-ui.en.md), “PyroWave”): when the bitrate PyroWave needs goes above the cap, the app lowers the resolution (then the refresh rate), never sharpness, and says so. The computation is in the core (`apps/core/src/pyrowave.rs`, see [apps/core/README.en.md](../apps/core/README.en.md)), the same for every app.

- **Needed bitrate**: width × height × refresh × bits per pixel, summed over the screens of a multi-screen session. **1.5 bits per pixel in 4:2:0** (SDR or HDR10: the top of what PyroWave gives at full sharpness, ~1.2 to 1.5); **2.25 in 4:4:4** (half more: twice the colour samples, which weigh less than luminance). That is ≈ 187 Mbit/s at 1080p60, ≈ 747 Mbit/s at 4K60, ≈ 1,120 Mbit/s at 4K60 4:4:4. Values to be confirmed on streams from the Station.
- **Cap**: the smallest of the known ones: the Station's `pyrowave.max_mbps`, the link bitrate the app gives (its port speed, or a measured bitrate), the bitrate set by hand in the app (format without a profile). None: the device's resolution stays.
- **Choice**: the largest even resolution that fits, aspect ratio kept, at the screen's refresh rate, down to 720 lines; then 120, 100, 60, 50 or 30 Hz, again with the largest resolution that fits; nothing fits: 720p at 30 Hz. With several screens, the same scale for all, positions included (they stay edge to edge).
- **Asked of the Station**: it is the virtual display's resolution (`prepare`), at start and on every adaptation; when the cap or the format changes during a session, the virtual display is redone if the resolution must change. Without a virtual display, the Station keeps its screen: only the bitrate is capped. Kyber gets at least the bitrate this resolution needs (profile), never more than the cap.
- **Told to the apps**: notice `ResolutionLowered` (banner “Resolution lowered to {w} × {h} to fit in {n} Mbit/s.”, removed when it comes back) and `Second::lowered` (Statistics line), in the C and JNI bridges and the Windows and Linux apps.

## The Windows Station

- **Library**: `libpyrowave-shared-1.dll` (PyroWave's C API, C++ runtime linked statically, Vulkan loaded on demand by volk: no dependency on `vulkan-1.dll`, kyavserver starts on a PC without Vulkan). Sources at the commit pinned in `server/kyber-patches/pyrowave.sh` (`a32b05e`, 11/11/2026), with the partial Granite of `checkout_granite.sh` (volk, Vulkan headers), in `third_party/pyrowave`; built with MinGW from the Mac by `build.sh`.
- **FFmpeg encoder `libpyrowave`** (patch 0025-kymedia, `libavcodec/libpyrowave_enc.c`, after the `pyrowave` branch of `HansKristian-Work/FFmpeg` for the `AV_CODEC_ID_PYROWAVE` identifier): it takes the capture's D3D11 frames without any CPU copy. At start-up, a Vulkan device on the same graphics card as the capture (LUID of the DXGI adapter, `pyrowave_create_device_by_compat`, Vulkan queue priority after `gpu_priority`), a shared D3D11 fence imported as a timeline semaphore. For every frame: a copy on the graphics card of the captured texture into a shared texture (NT handle, imported once into Vulkan, recreated if the size or format changes), the fence signalled, then `pyrowave_encoder_encode_gpu_scaled`, which waits for the fence, converts and scales down if needed, and `pyrowave_encoder_packetize`.
- **Both engines**: txproto (`create_pyrowave` in `txproto/video.rs`, capture linked straight to the encoder) and libavconv (`encoder_names`, `null` filter). `encode.c` and `enc.c` need no change: the encoder declares itself hardware on D3D11 frames, they give it the capture's frames context.
- **Several displays**: one encoder per display, as for the other formats; each opens its Vulkan device on its display's graphics card.
- **A guest's own scaled-down image** (KyberNow Share): PyroWave scales the image down itself (`out_width`, `out_height` options) instead of `scale_d3d11`; rate capped by the capture (txproto), the encoder's `fps` set to it for the frame budget.
- **Setting**: `[kyavserver] pyrowave` (true by default), Admin key `video.pyrowave`, Windows only (provisional texts, marked `TODO textes pyrowave`). “Maximum bitrate”: `video.pyrowave_max_mbps` (see the announcement, item 6), Windows only; the mock-up's Auto help (“here {n} Mbit/s”) is still to be done in Admin's PyroWave card.
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
