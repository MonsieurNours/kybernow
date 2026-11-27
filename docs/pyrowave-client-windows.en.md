# PyroWave: the Windows client

English · [Français](pyrowave-client-windows.md)

Client section of [pyrowave.en.md](pyrowave.en.md) (the contract and the Station), to be merged into it. About the Windows app (`apps/windows`, x64 and ARM64); the Mac client is in [pyrowave-client-mac.en.md](pyrowave-client-mac.en.md). Rules and texts: [link-ui.en.md](link-ui.en.md), “PyroWave”.

## The library

- The Vulkan path of [PyroWave](https://github.com/Themaister/pyrowave) (MIT licence), at the commit in `apps/macos/PyroWave/COMMIT` (a32b05e, the same as the Station and the Mac), with the partial Granite of `checkout_granite.sh` (volk, Vulkan headers). Sources in `third_party/pyrowave`, shared with the Station (which builds in `build-win64`).
- `apps/windows/scripts/build-pyrowave.sh [--arch x64|arm64]` (cmake and ninja) builds `libpyrowave-shared-1.dll` in `third_party/pyrowave/build-client-<arch>` and puts it, stripped, in `third_party/windows-client-<arch>/pyrowave/` with its commit. x64: Homebrew MinGW, libstdc++ and libgcc linked statically by the upstream project; ARM64: llvm-mingw, libc++ and libunwind linked statically (`-static`). About 2.2 MB (x64) and 1.7 MB (ARM64); they only depend on Windows and the UCRT.
- `build.sh` rebuilds it when missing or when the commit changed, and copies it into `dist/<arch>/`: it ships in `KyberNow-Link-Setup.exe` with the rest of the folder.
- **Vulkan comes from the driver.** The DLL isn't linked to the Vulkan loader: volk opens `vulkan-1.dll` (installed by the graphics driver) when the app creates the PyroWave device. Without it, without PyroWave's DLL, or on a card that can't run it, PyroWave is only unavailable: the probe tells the core, nothing else changes.

## Decoding (`src/pyrowave.rs`)

- The app opens the DLL itself (`LoadLibraryW`, once per process), reads its functions and checks the API (1.x). The `pyrowave.h` types and the little Vulkan needed are copied by hand (no Vulkan headers at build time).
- **Device.** One PyroWave device per Direct3D 11 device of the session (`Gpu::pyrowave`, created with the first PyroWave stream, shared by every display): `pyrowave_create_device_by_compat` with the LUID of Direct3D's adapter (`IDXGIAdapter::GetDesc`), hence the same card; `pyrowave_device_confirm_interop_support`, graphics queue (`pyrowave_device_set_queue_type`). Fragment path (`fragment_path`) when `pyrowave_decoder_device_prefers_fragment_path` recommends it (Adreno in Snapdragon PCs).
- **Configuration packet** (`KNPW`, read by the core: `kybernow_core::pyrowave::Config`, 16-byte version 1 and 48-byte version 2 with its HDR10 metadata, `Config::hdr10`): recognised by `is_config` or by its signature, as on the Mac. The core asks for version 2 for every app (`pyrowave::ask_header_v2`); `hdr::pyrowave_meta` (tested on the Mac) draws the stream's peak from it for the renderer; an older Station sends version 1. A change recreates the decoder and its textures. If it's missing, resolution and chroma are read from the stream's sequence header (falls back to SDR, full range).
- **Shared textures.** Three Direct3D 11 textures (Y, Cb, Cr; `R8_UNORM` in SDR, `R16_UNORM` in HDR10; half-size chroma in 4:2:0), `SHARED | SHARED_NTHANDLE`, imported into PyroWave through their NT handle (`VK_EXTERNAL_MEMORY_HANDLE_TYPE_D3D11_TEXTURE_BIT`, `MUTABLE_FORMAT`, like `create_pyrowave_image_from_d3d11` in the upstream interop test), one storage view per plane.
- **Synchronisation.** A shared Direct3D 11 fence (`D3D11_FENCE_FLAG_SHARED`), imported as a timeline semaphore (`D3D12_FENCE_BIT`). For each frame: decoding waits for the value Direct3D set after copying the previous frame (images acquired from `QUEUE_FAMILY_IGNORED`: content discarded) and signals the next one while handing the images back to Direct3D (`QUEUE_FAMILY_EXTERNAL`); the video thread waits for that value on the CPU (at most 250 ms, otherwise the frame is skipped: Direct3D never waits for a fence that won't come). The renderer does `ID3D11DeviceContext4::Wait`, copies the three planes into its own textures, then `Signal` and `Flush`.
- **One frame = one packet.** `pyrowave_decoder_clear`, the whole packet into `push_packet`, then `decode_is_ready` (complete, or partial within the upstream threshold); otherwise the frame is skipped. Never a keyframe request: every frame decodes on its own. When other frames are waiting behind (network burst), this one isn't decoded at all.
- **Rendering** (`Renderer::draw_pyrowave`): the shader's “three planes” mode (the one for CPU decoding and 4:4:4), nothing PyroWave-specific. Values: PyroWave writes the normalised value v; in 8 bits, code v · 255; in 16 bits, the Station's P010 convention (10 bits at the top of 16, unit 64/65,535). BT.709 (SDR) or BT.2020 (HDR10) matrix, range from the configuration packet; HDR10: PQ, to the 10-bit PQ swap chain or brought down to SDR like HEVC Main 10; no HDR10 metadata in the stream, default peak (1,000 nits).
- **Timing.** `received` when the packet arrives, `decoded` when the GPU is done (fence seen by the CPU): the statistics' decode time and the frame path account for it as for D3D11VA. The format shown in the bar and statistics is “PyroWave · graphics card”.
- **Video thread** (`src/video.rs`): the announced `pwv1` format opens the PyroWave decoder instead of FFmpeg; a format never announced but requested (PyroWave chosen) opens it anyway, as for the other formats.

## Capabilities and cable

- **Launch probe** (`pyrowave::probe`, in the `--probe-json` probe process, like decoding the other formats): PyroWave device on the card, 128 × 128 decoder, the Mac's test frame really decoded (`src/probe/pyrowave.bin`, 1,812 bytes, Y ramp from 16 to 235, Cb and Cr at 128, encoded by the port's Metal encoder), through the session path (shared textures, fence), then a few points read back, within 4. `pyrowave` in the capabilities sent to the core (`Gpu`, `ThisPc`, `Capabilities`); `kybernow --probe` says “PyroWave : carte graphique (Vulkan)” or the reason. A crashing driver only takes the probe process down.
- **Cable** (`system::wired_to`): the network adapter Windows picks for the computer's address (`GetBestInterfaceEx`, then `GetIfEntry2`) is Ethernet (type 6, 802.3 or unspecified medium); Wi-Fi, mobile broadband or an address that can't be resolved: no. Read when the session starts and given to the core (`StreamSettings::wired`, kept when the profile changes during the session); “Auto” then takes PyroWave if the Station offers it (core rule, `profile::pyrowave_format`). The settings also ask for it (`computer_wired`) for their help texts.
- **Bitrate of a hand-picked PyroWave** (`plan.rs`, the interface's `sessionPlan`): the one “Auto” would take by cable for the requested resolution (the core's `PYROWAVE_MBPS`, `PYROWAVE_MBPS_444`, following pixels per second), not HEVC's. 4:4:4 is allowed with PyroWave.

## Interface

Texts from link-ui, in the app's catalogues (`ui/src/lib/i18n`, `pyrowave` section; `src/i18n`, the banner text).

- **Settings › Picture › Video format**: the “PyroWave” value when the Station offers it (`info.pyrowave`, boolean or `{ available }`) or when it's already chosen; inactive with its reason (“{name} doesn't offer PyroWave.”, “This PC can't decode PyroWave.”). Help: PyroWave chosen; “Auto” by cable or without. Warning under the choice, PyroWave chosen: Wi-Fi, or relay (computer away from home or session through the relay). “Sharp text (4:4:4)” allowed with PyroWave, “almost free” help. The bitrate slider goes up to 1,500 Mbit/s with PyroWave, without the general Wi-Fi warning.
- **Session start banner**: at the first second of PyroWave picture, without cable or through the relay, once per session: the bar's capsule “PyroWave on Wi-Fi” / “PyroWave away from home”, “The picture may stutter.”, a “Switch to HEVC” button (`session_hevc` command: the picture restarts in HEVC for this session, the computer's choice stays), a cross; 8 s. App not in front: the Windows notification of the other notices.
- **Banner when the network is saturated** ([debit-automatique.en.md](debit-automatique.en.md)): the core sends the `PyroWaveStrained` notice after 5 s of saturated network (once per session, host only); `host.rs` passes it to the interface (`pyrowave_strained`), which shows the same banner titled “The network is saturated”, with “Switch to HEVC” and the cross; 8 s. Bar open on this session: it waits until the bar hides, dropped if the session ends or the picture is already in HEVC. App not in front: the Windows notification.
- **Session bar**: amber chip “PyroWave on Wi-Fi” / “PyroWave away from home” when the picture is PyroWave (format of the measurements) without cable or through the relay; the format says “PyroWave”.
- **Statistics**: “Lost frames” line as a share of the frames of the last 30 seconds (“0.8%”); “Resolution lowered to {w} × {h} to fit in {n} Mbit/s.” line ready (`Stats::lowered`), but the core doesn't say yet when it lowers the resolution: never shown for now.
- Interface demo (`npm run dev`): `?pyrowave`, `?pyrowave=wifi`, `?pyrowave=pc`, with `&relais`; `bar.html?pyrowave=wifi` for the banner, `bar.html?pyrowave=sature` for the saturated network one.

## Offline test (`kybernow --pyrowave-test`)

On the PC, with no computer and no network: the launch probe, then an animated test pattern (scrolling ramp, fine checkerboard, colour bands, limited range) encoded by **the upstream encoder** (the DLL's, `pyrowave_encoder_encode_cpu`, on the same device), one frame = 1,152-byte packets back to back like a Station media packet, decoded by **the app's decoder** (`pyrowave::Decoder`, shared textures, fence) and copied as the renderer does. For 1920 × 1080, 2560 × 1440 and 3840 × 2160, in 4:2:0 and 4:4:4, SDR and HDR10 (16 bits, P010 convention): bitrate at 60 fps, decode time (GPU done) paced at 60 fps (mean, median, maximum), PSNR of the first frame read back from the card, truncated frame skipped and the next one decoded. Options: `--size 1920x1080`, `--420`/`--444`, `--sdr`/`--hdr`, `--frames 120`, `--bpp 1.2`. Exit code 1 if PyroWave is unavailable or the Y PSNR drops below 30 dB.

**Not run yet**: written and built on the Mac (x64 and ARM64) on 25/11/2026, never executed on Windows.

### Steps on the x64 PC

1. `apps/windows/scripts/build.sh`, then copy `apps/windows/dist/x64/` to the PC (or the `KyberNow-Link-Setup.exe` installer, after `--arch arm64` and `build-installer.sh`).
2. In a terminal, in that folder: `kybernow.exe --probe`. Expected: “PyroWave : carte graphique (Vulkan)”; otherwise the reason (missing DLL, no Vulkan, card refused, wrong test frame).
3. `kybernow.exe --pyrowave-test`. Expected: one line per case, no “ÉCHEC”, Y PSNR above roughly 40 dB, decoding around 1 to 2 ms on a desktop card, “image tronquée sautée, suivante décodée”. Keep the output and `%LOCALAPPDATA%\KyberNow\kybernow.log`.
4. In a session (app built with `build.sh`, which applies kyclient patch 0025 and the core's `pyrowave` feature by default; Station offering PyroWave): “Video format” › PyroWave, then the log (“PyroWave : device prêt”, “PyroWave : décodeur …”), the bar (“PyroWave · graphics card”) and Statistics. Unplug the cable (Wi-Fi): settings warning, banner and chip; “Switch to HEVC”.

### In the Windows ARM VM (Parallels)

Same with `dist/arm64/`. Parallels' virtual graphics card probably doesn't provide Vulkan: expected then “PyroWave : non (pas de Vulkan …)” from `--probe`, exit code 1 from `--pyrowave-test`, and above all an app that starts and HEVC sessions that work as before. On a real Snapdragon PC (Adreno), `--pyrowave-test` should say “Chemin par fragments”.

## What's waiting

- Nothing on the code side: “Resolution lowered” comes from the core (`Second::lowered`, `ResolutionLowered` notice, read by `host.rs`). The session test described above remains.
