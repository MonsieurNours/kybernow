# PyroWave: the Linux client

English · [Français](pyrowave-client-linux.md)

Client section of [pyrowave.md](pyrowave.md) (the contract and the Station), to be merged into it. Covers the Linux app (`apps/linux`): desktop Linux, Steam Deck and SteamOS in the living room. Raspberry Pi: Pi 5 only (patched dequantiser, fragment path, probe in a separate process), Pi 4 and Pi 3 unavailable: [pyrowave-client-pi.en.md](pyrowave-client-pi.en.md). Interface rules: [link-ui.md](link-ui.md), “PyroWave”; Mac client: [pyrowave-client-mac.en.md](pyrowave-client-mac.en.md).

## The library

- [PyroWave](https://github.com/Themaister/pyrowave) (MIT licence), **Vulkan** path, at the Station's and the Mac's commit (a32b05e, `apps/macos/PyroWave/COMMIT`), with the part of [Granite](https://github.com/Themaister/Granite) (MIT) that `checkout_granite.sh` fetches. `apps/linux/scripts/build.sh` (step 4) fetches it with git, builds it with CMake (target `pyrowave-shared`, the `pyrowave.h` C API) and puts `libpyrowave-shared.so.1`, `pyrowave.h` and Granite's Vulkan headers in the prefix. About thirty seconds; the Flatpak's dependency module caches it along with FFmpeg and libkynput.
- **Shared library, loaded at launch** (`dlopen`): next to the app (`<prefix>/lib`, `/app/lib` in the Flatpak), otherwise the build prefix, otherwise the system path; `KYBERNOW_PYROWAVE_LIB` forces one. Without it, PyroWave is unavailable and nothing else changes. The Vulkan loader is the runtime's (`libvulkan.so.1`, opened by volk inside Granite); no extra permission in the Flatpak (`--device=all` is already there).
- **Requesting PyroWave from Kyber**: `build.sh --kyber-pyrowave` (and `flatpak.sh ARCH --kyber-pyrowave`) applies the Station's kyclient patch 0025 (`VideoCodec::PyroWave`, `fallback_codec`) and turns on the core's `pyrowave` feature (the app's `pyrowave` feature forwards it). Both go together; like the other apps, only on request until they are on everywhere. Without them, the app decodes PyroWave but never requests it.
- **Shared Kyber SDK**: when the shared `third_party/` already carries another piece of work's patches (the branch's patches no longer apply), `KYBERNOW_KYBER_HEAD=1` makes `sources.sh` take the SDK as it is in its git repositories (HEAD of kysdk and its submodules), read-only; `build.sh` then applies the branch's patches.
- `build.sh --rpi` builds it too (the .deb and KyberNow Link OS ship it). Everywhere, `build.sh` patches upstream's dequantiser (`apps/linux/pyrowave/v3dv-dequant.py`: an `r16f` image, required by the Pi's driver, no effect elsewhere; [pyrowave-client-pi.en.md](pyrowave-client-pi.en.md)).

## Decoding (`src/native/pyrowave.c`, `src/device/video.rs`)

- The picture thread (one per screen) recognises the `pwv1` format (`0x7077_7631`, the core's `FOURCC_PYROWAVE`) and hands its packets to PyroWave instead of FFmpeg. One PyroWave device for the whole process (creating it compiles the pipelines); its calls go through a lock (several screens).
- **Config packet** (`KNPW`, recognised as a config packet or by its signature; read by the core's `pyrowave::Config`, inside the frame flow state machine `frames::FrameFlow`): chroma, colour, range, resolution; a change rebuilds the decoder. Version 1 (16 bytes) or 2 (48 bytes, with the capture's HDR10 metadata: mastering display, MaxCLL, MaxFALL, given to the HDR picture), which the app asks of the Station (`kyclient::set_pyrowave_header(2)`, see [kyber-changes.en.md](kyber-changes.en.md), “KNPW header version 2”). If it is missing, resolution and chroma come from the stream's sequence header (`Config::from_sequence_header`, SDR, full range), as on the Mac.
- **Decoder that does not open** (no suitable Vulkan, failing graphics card): `Session::pyrowave_failed`, once per session; the core restarts the picture in the format Auto would have taken, and the app shows the notice “Picture in {format}: PyroWave couldn't start on this device.” (`Platform::on_pyrowave_fallback`).
- **Media packet**: one whole frame. `pyrowave_decoder_clear`, the whole packet into `push_packet`, then decoding if the frame is complete or complete enough according to upstream (`decode_is_ready`, partial allowed); otherwise it is skipped. An unreadable frame is skipped too. A hole in the stream does nothing: **no keyframe is ever requested**, the next frame decodes on its own.
- **Simple path (the one written)**: PyroWave decodes on the graphics card, then reads the planes back into CPU memory (`pyrowave_decoder_decode_cpu_buffer_synchronous`), straight into the app's frame buffer:
  - SDR 4:2:0: **I420** (three 8-bit planes), shown by SDL like a CPU-decoded frame (`SDL_PIXELFORMAT_IYUV`);
  - HDR10 4:2:0: **I010** (10 bits in the low bits, scale set by upstream), the same frame as CPU HEVC Main 10 decoding: the Wayland HDR subsurface takes it as is (PQ, BT.2020), otherwise SDL gets it brought down to 8 bits (the existing fallback);
  - 4:4:4 (SDR): three 8-bit planes read back, then converted to **BGRA** by FFmpeg's 4:4:4 conversion (`kn_yuv444_to_bgra` in `decoder.c`, BT.709, range from the config packet). HDR10 in 4:4:4 does not exist (the core forces 4:2:0 in HDR).
- **Timing**: `received` when the packet arrives, `decoded` after the read-back; the statistics' decode time (“Decoding” in the path of a frame) therefore includes the read-back. Frames never shown count in “Lost frames”.
- Upstream's fragment path (mobile GPUs) is only taken when the device prefers it, and never in HDR (it does not know the 10-bit output scale).
- **Zero-copy (not written)**: if the read-back costs more than 1 ms at 1440p on the Steam Deck, the next step is zero-copy sharing: a Vulkan image exported as a dmabuf (`pyrowave_image_create` can import a dmabuf with a DRM modifier) and shown through EGL (`EGL_EXT_image_dma_buf_import`) under SDL, like the HDR subsurface; or rendering the session in Vulkan. Not measurable in the VM (see “Measurements”).

## Capabilities

- **Launch probe** (`system::pyrowave_probe_start`, on a thread while the window opens): the library, a suitable Vulkan 1.3 device, a 128 × 128 decoder, the Mac's test frame (1,812 bytes, Y ramp from 16 to 235, encoded by the port's Metal encoder: the same bitstream) decoded and read back, a few points within 4. The log gives the result, the GPU and the duration (“PyroWave : prêt sur AMD Custom GPU 0405 (RADV VANGOGH) (Vulkan 1.3)”).
- **Unavailable**: the Raspberry Pi 4, 3 and Zero 2 W (not even tried; the Pi 5: [pyrowave-client-pi.en.md](pyrowave-client-pi.en.md)), `KYBERNOW_PYROWAVE=0`, the library missing, no suitable Vulkan 1.3 (`PYROWAVE_ERROR_NO_VULKAN`), software rendering only (llvmpipe/lavapipe: far too slow for a session; `KYBERNOW_PYROWAVE_CPU=1` allows it, for tests), or a wrong test frame. The PyroWave value of “Video format” is then inactive with link-ui's reason: “This device can't decode PyroWave.”.
- `Machine::pyrowave` = the probe **and** the core's `PYROWAVE_KYBER` (the Station's kyclient patch): while the core's `pyrowave` feature is off, the app does not announce PyroWave (the log says so). `Capabilities::pyrowave` goes to the core: in the screens plan, and in `SessionParams::capabilities` of a one-screen session when the probe succeeded.
- **What the Station offers**: `info.pyrowave` from `GET /api/stream/info`, kept in `Host::pyrowave` (`pyrowave` in `kybernow.json`).
- **By cable** (`system::wired_to`): at the start of the session, the interface the system uses to reach the computer (the local address of a “connected” UDP socket, nothing sent, found through `getifaddrs`) is wired (Ethernet type, neither Wi-Fi nor a tunnel) and up. Without a known route, the device's network. For the core, “Auto” is a profile; the Linux app always keeps one: `StreamSettings::wired` is therefore only given when “Video format” is on Auto (otherwise a chosen HEVC would become PyroWave), and it is kept across profile changes in session.

## Interface (tiny-skia; Desktop, Deck, Living room)

link-ui's texts, word for word (`src/pyrowave.rs`, without SDL or the core):

- **Video format**: “PyroWave” after AV1, only when the computer offers it (or it was already chosen); inactive with its reason (“{name} doesn't offer PyroWave.”, then “This device can't decode PyroWave.”) at the end of the help. Help texts: PyroWave chosen, Auto by cable, Auto without cable. PyroWave chosen on Wi-Fi or through the relay: the warning follows the help, with an amber tag (“Wi-Fi”, “away from home”). In settings, “by cable” is the device's network (Ethernet up); the session looks at the route to the computer.
- **Sharp text (4:4:4)**: allowed with PyroWave, help “With PyroWave, almost free: only the bitrate goes up.”.
- **In session**, when the decoder says “PyroWave” and the session goes over Wi-Fi or through the relay (never for a guest):
  - the start **banner**, once, under the bar: “PyroWave on Wi-Fi” / “PyroWave away from home”, “The picture may stutter.”, “Switch to HEVC” and a close cross; it goes away by itself after 8 s. On the Deck and in the living room, touch works on it;
  - the **amber chip** in the desktop bar, after “Through the relay”; the format there says “PyroWave”;
  - with a controller (Deck, living room), a session menu **tile** named like the chip, subtitle “Switch to HEVC”.
  - “**Switch to HEVC**” restarts the picture in HEVC for this session only (`Session::set_settings`, cable removed so that “Auto” does not take PyroWave back); the computer's choice stays.
- **The network is saturated** (core notice `PyroWaveStrained`: 5 s in a row of losses or queueing, once per session, host only; [debit-automatique.en.md](debit-automatique.en.md)): the same banner, titled “The network is saturated”, by cable too; with a controller, the same tile (named like the chip on Wi-Fi or through the relay, otherwise “The network is saturated”), subtitle “Switch to HEVC”. Capture `45i-pyrowave-reseau-sature`.
- **Statistics** (session): “Lost frames: 0.8 %” (the computer's frames never shown, as a share of the second's frames; amber above zero) and “Resolution lowered to {w} × {h} to fit in {n} Mbit/s.” when the core lowered the resolution to fit the Station's “Maximum bitrate” (`Second::lowered`, text from `Lowered::text`, kept in `Stats::lowered`; wraps when needed). When it lowers it, the core also says so (`Notice::ResolutionLowered`): the session's capsule, under the PyroWave banner when that one is shown (desktop), at the bottom on the Deck and in the living room.
- Captures: `--preview`, `45-pyrowave-*` (desktop, deck, living room: by cable, Wi-Fi, Auto by cable and without, inactive, session, banner, statistics alone).

## Offline test (`apps/linux/tests/pyrowave/run.sh`)

- `encode.c`: upstream's Vulkan encoder through its C API (ffmpeg's `testsrc2` pattern, 1.2 bits per pixel, 1,152-byte packets put end to end as the Station does), then `check.c`: the probe, then each frame through `pyrowave.c` as a media packet; PSNR of the first frame, timing paced at 60 fps and back to back, a truncated frame skipped and the next one decoded. 4:2:0, 4:4:4 (BGRA) and HDR10 (I010).
- Without a suitable Vulkan encoder (the VM: lavapipe has subgroups of 4, upstream's encoder wants 16 to 64), `SAMPLES=FOLDER` decodes streams made elsewhere: `tests/pyrowave/samples-mac.sh FOLDER [WxH] [frames]` makes them on an Apple silicon Mac with upstream's Metal encoder (the Mac client's test tool).

## Measurements

“KyberNow Linux” virtual machine (Ubuntu 24.04 ARM64 under Parallels, MacBook Pro M4 Max): its virtual GPU (virgl) has no Vulkan; only **lavapipe** (software Vulkan 1.4, llvmpipe) runs there. The decoder works there (the probe, the Mac's streams), but the timings are the CPU's, not a graphics card's: they say nothing about the Steam Deck, nor about the read-back cost.

25/11/2026, streams encoded by the Mac's Metal encoder, 1280 × 720, 30 frames, 1.2 bits per pixel (66 Mbit/s at 60 fps), `KYBERNOW_PYROWAVE_CPU=1`:

| Format | Output | PSNR (1st frame) | Decode + read-back, paced at 60 fps | Back to back |
|---|---|---|---|---|
| 4:2:0 SDR | I420 | Y 51.9 dB, Cb 58.0, Cr 58.9 | 10.7 ms (max 11.7) | 97 fps |
| 4:2:0 HDR10 | I010 | Y 51.9 dB (brought to 8 bits) | 10.4 ms (max 11.3) | 99 fps |
| 4:4:4 SDR | BGRA | — (middle pixel right: BT.709 blue) | 18.0 ms (max 26.7) | 57 fps |

Probe: 33 ms at app launch (lavapipe refused), 56 to 79 ms when it is allowed. Frame truncated to 60 %: skipped, the next one decoded. The Metal port's bitstream decodes correctly through the Vulkan path (the probe and the PSNRs show it).

## To try on the Steam Deck

1. Build and install the x86_64 Flatpak (`apps/linux/scripts/flatpak.sh x86_64 --kyber-pyrowave` in the VM so that it requests PyroWave, or a release's): `flatpak install --user KyberNow-Link-Linux-x86_64.flatpak`.
2. Start the app once in Desktop mode from a terminal (Konsole): `flatpak run fr.leslignesclaires.kybernow 2>&1 | grep -i pyrowave` should say “PyroWave : prêt sur AMD Custom GPU … (RADV VANGOGH)” and the probe's duration. “décodeur prêt, mais ce cœur ne sait pas encore le demander”: the core's `pyrowave` feature is still off (the Station's kyclient patch not merged yet).
3. The offline test on the Deck (measuring the read-back): the Flatpak has no compiler, so in an Ubuntu 24.04 distrobox (or with the app built by `build.sh` on an x86_64 Linux), `apps/linux/tests/pyrowave/run.sh 2560 1440 120` (upstream's Vulkan encoder runs on RADV); note “décodage, relecture comprise” at 1440p 4:2:0 and 4:4:4. More than 1 ms above decoding alone: write the zero-copy path (dmabuf).
4. With a Station that offers PyroWave (patch 0025 and the core's `pyrowave` feature on): Settings › Picture › Video format › PyroWave; a session by cable (dock and Ethernet), then on Wi-Fi: the banner, the chip (desktop) or the tile (Game mode), “Switch to HEVC”; the statistics (Decoding, Lost frames, Resolution lowered with a low “Maximum bitrate” in Admin). In Game mode (gamescope) and in the living room, the same with a controller.
5. HDR10 in PyroWave on a KDE Plasma 6.3+ desktop with an HDR screen (the Deck OLED in Game mode stays SDR, as with HEVC).
