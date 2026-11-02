# KyberNow: HDR

English · [Français](hdr.md)

HDR gives the picture richer colours and brighter highlights, on a screen that can show them: a Mac's XDR display, an iPhone, an HDR TV behind an Apple TV, a Shield or the TV's own app. Games and Windows in HDR reach that screen as they are.

This document is the shared contract of the Station (Admin, Kyber), the Rust core and every Link app. The exact texts are in [link-ui.en.md](link-ui.en.md), “HDR” section. It replaces the study of 22/10/2026 ([archives/propositions-2026-10-22.md](archives/propositions-2026-10-22.md), section 1, in French), whose findings in the Kyber 0.29 sources it keeps. Status: **scoped on 02/11/2026, nothing written yet.**

## What the user sees

- **The choice**: an “HDR” chip on the computer's card, on the home screen, next to the profile, only when the device's screen is HDR. It is **on** the first time (HDR is automatic), and the app remembers the choice for that computer. The same switch is in Settings › Picture.
- **Every profile**: Game, Desktop, Max and Custom go HDR. HDR only exists in 4:2:0: with HDR on, “Sharp text (4:4:4)” is inactive and the Desktop profile sends 4:2:0; with HDR off, 4:4:4 comes back. HDR does not turn the profile into “Custom”: it is a choice of its own.
- **During a session**: “HDR” in the bar and in the icon's menu, as a checkbox. Changing it restarts the picture (“First frames” step, one or two seconds), like a profile picked during a session.
- **The computer**: Windows switches the virtual display to HDR by itself when the session starts and back to SDR when it ends. Nothing to set on the PC.
- **Several displays**: each display of the session is HDR if the device screen that shows it is (a Mac with its XDR display and an SDR external monitor: the first in HDR, the other in SDR).
- **The device's screen changes** (window dragged to an SDR screen, TV that turns HDR off): that display's picture goes back to SDR, then to HDR when the screen is HDR again, without a message; the “HDR” choice stays.
- **Share guests**: a guest whose screen is HDR gets HDR when the host is in HDR; a guest without an HDR screen, or in a browser, gets a separate picture converted to SDR by the Station. Guests have no chip: it is automatic. The friend the computer is lent to has the chip and the bar's switch, like a host.
- **Statistics**: “HDR” is added to what was transmitted (“3840 × 2160 · 120 Hz · HEVC · HDR · 80 Mbit/s”).
- **The screen's real brightness**: the computer and games know the brightness of the device's screen, not that of some virtual display; the Windows desktop in HDR no longer looks washed out. It is automatic, and an “HDR calibration” screen lets you tune the screen by eye, as on a console (next section).

## HDR calibration

Windows handles HDR poorly for a virtual display shown on another device: games tone-map from the brightness Windows announces for the virtual display, not that of the screen showing it, and Windows' calibration tool is run in front of a physical display. In HDR, Windows also converts the SDR desktop with the piecewise sRGB curve, which lifts blacks (washed-out picture), and the desktop's white depends on a slider set by hand. KyberNow sets all of this itself, for each session.

- **Screen values**: maximum brightness (small area), full-screen brightness, black, and desktop white, in nits. Each app knows them for each of the device's screens:
  - **automatically** when the system gives them (Apple screens, from their EDR headroom and the display model);
  - otherwise by default: 1,000 nits, 1,000 nits full screen, 0 nits, desktop white at 203 nits (BT.2408 reference);
  - or **by eye**, through the “HDR calibration” screen, which replaces the automatic values for that screen.
- **What the Station does**: for each HDR session, Admin puts on the virtual display a colour profile generated for the device's screen (an ICC profile with an MHC2 tag, the same kind as Windows' tool makes: brightness values, and a curve that brings SDR back to gamma 2.2), associated through Windows' colour APIs, and sets Windows' desktop white (SDR white level). Games then read the real brightness (`IDXGIOutput6::GetDesc1`) and tone-map for that screen; the stream's HDR10 metadata (0022) follows. Everything is put back when the session ends, on restart after a crash and when the display is removed.
- **No extra tone mapping in the Station**: the game does it once, with the right values. On Mac, iPhone and iPad, the system also adapts the picture to the current brightness; on a TV, its game mode (HGIG on LG) does the rest. The Windows, Android and Linux apps (second stage) will have their own curve when the stream goes beyond what their screen shows.
- **Admin setting**: “Desktop in HDR: gamma 2.2 (recommended)” or “Windows sRGB”, Picture and sound page; the profile keeps the brightness values either way.

**The “HDR calibration” screen** (apps, only on an HDR screen): in the App settings (the values belong to the device, not to a computer), and through a “Calibrate HDR for this screen…” link under the “HDR” switch in Settings › Picture. Four steps, each a pattern shown in HDR by the app itself, without a computer:

1. **Maximum brightness**: a small bright area with a barely darker symbol; raise until the symbol just disappears.
2. **Full-screen brightness**: the same pattern over the whole screen.
3. **Black**: a symbol barely lighter than black; lower until it just disappears.
4. **Desktop white**: a window and text as on the computer; pick the brightness that suits you.

Touch, arrows, scroll wheel or controller (left, right) change the value; “Next”, “Back”, “Done”, “Cancel”, and “Back to automatic”. The values are kept for that screen of the device and apply to the next session. A session in progress keeps its own.

## When HDR is not possible

The chip stays visible but inactive, with the reason in its help, in this order:

| Case | Reason |
|---|---|
| Station without HDR (older version, or Admin set to “Never”) | “The computer doesn't offer HDR.” |
| Windows older than Windows 11 24H2 | “HDR needs Windows 11 24H2 on the computer.” |
| Encoder without 10-bit (x264, card too old) | “The computer's graphics card doesn't do HDR.” |
| Video format set to H.264 | “HDR needs the HEVC or AV1 format.” (“Auto” picks HEVC or AV1 when HDR is on.) |
| Device decoder without 10-bit HEVC or 10-bit AV1 | “This device can't decode HDR.” |
| No virtual display, PC screen in SDR | “The computer's display isn't in HDR.” (without a virtual display, Admin leaves the PC's displays alone) |

If Windows refuses to switch the virtual display to HDR when the session starts, the session goes on in SDR and the app says so once: “The computer couldn't switch to HDR: picture in SDR.”

## The chain

### Station: Kyber (patches 0022)

Finding in Kyber 0.29: both engines capture through `IDXGIOutput1::DuplicateOutput`, which returns a desktop that Windows has brought down to 8-bit SDR, and refuse any format other than BGRA; FFmpeg forces the BT.601 matrix for any RGB input given to NVENC. HDR therefore needs a 16-bit float capture and a conversion done by Kyber itself.

1. **FP16 capture**, only for a display requested in HDR: `IDXGIOutput5::DuplicateOutput1` with `[R16G16B16A16_FLOAT, B8G8R8A8_UNORM]` in txproto (`iosys_dxgi.c`) and libavgrab (`dxgi.c`); the Windows Graphics Capture agent of the secondary displays (0018) opens its `Direct3D11CaptureFramePool` as `R16G16B16A16Float` and its ring of shared textures in that format. Without HDR: today's path, unchanged.
2. **Conversion on the GPU**, right after capture: one shared HLSL compute shader (linear scRGB × 80/10,000 nits → BT.2020 primaries → PQ → non-constant BT.2020 Y'CbCr, limited range, 10-bit → P010, chroma averaged 2 × 2), compiled at run time by `d3dcompiler_47.dll` (present on every Windows 10 and 11). No curve on the HDR picture: above 10,000 nits, the value is clipped. Frames tagged `BT2020` / `SMPTE2084` / `BT2020_NCL` / MPEG range.
3. **Encoders**: NVENC, AMF and QSV take D3D11 P010 frames: HEVC Main 10, 10-bit AV1. No H.264 in HDR, no x264 in HDR, no 4:4:4 in HDR. Static metadata (mastering display and MaxCLL SEI) from the luminance Windows gives for the virtual display (`IDXGIOutput6::GetDesc1`), for TVs that expect it.
4. **Separate SDR picture** (0020): for a guest without HDR, the same FP16 texture goes through a second shader: SDR white at the value of Windows' “SDR content brightness” slider, a soft curve above it (knee at 75% of white, compression up to 4 times white), BT.709 primaries, gamma, 8-bit NV12, then today's downscaling.
5. **Session flag**, per display: `hdr` along the path of `yuv444` (kyclient's `VideoSessionConfig` and its C API, kycontroller's start request, `VideoStreamConfig`, the `KymuxStartVideo` IPC, both engines' `VideoConfig`). Transport has nothing to learn: VUI and SEI travel in the stream.

Patches: `0022-txproto-hdr`, `0022-libavgrab-hdr`, `0022-kyavservice-hdr`, `0022-kycontroller-hdr`, `0022-kyclient-hdr` (and the WGC part in Admin, outside Kyber). Both capture patches can go upstream to Kyber as they are.

### Station: Admin

- **Setting** (Picture and sound page): “HDR”: “When the device asks” (default) or “Never”.
- **What the Station announces**: `GET /api/stream/info` gains `hdr: { available: bool, reason?: "off" | "windows" | "encoder" | "display" }`; `reason` gives the row of the table above (`display`: no virtual display, PC screen in SDR, read by the agent and kept for 30 s).
- **Preparation**: `prepare` receives `hdr: bool` for each display; Admin turns HDR on for the virtual display it created (`DISPLAYCONFIG_SET_HDR_STATE`, Windows 11 24H2), waits for Windows to confirm (2 s at most), and answers, for each display, the HDR it got. When the session ends, when Admin restarts after a crash, and when the display is removed: HDR put back as it was. Without a virtual display, Admin changes nothing and answers the HDR the PC's display already has.
- **Log**: one line per display: HDR asked, obtained, luminance given by Windows, capture format.

### Rust core and apps

- **Core** (`apps/core`): the `hdr` setting (on by default) is kept for each computer; the session sends `hdr` for each display when the setting is on, the device's screen is HDR, the Station announces it and the decoder can follow; it then forces 4:2:0 and HEVC or AV1. The app tells the core, for each display, “HDR or not”, and warns it when that changes; the core restarts that display's picture.
- **Apple** (Mac, iPhone, iPad, Apple TV): HDR screen when its potential EDR headroom is above 1 (`maximumPotentialExtendedDynamicRangeColorComponentValue` on Mac, `potentialEDRHeadroom` on iPhone and iPad); on Apple TV, when the TV accepts HDR10 (`AVPlayer.eligibleForHDRPlayback`). Rendering: `CAMetalLayer` in `rgba16Float`, `itur_2100_PQ` colour space, `wantsExtendedDynamicRangeContent`, `edrMetadata`; `VideoRenderer`'s shader already reads 10-bit planes and the BT.2020 matrix, it writes the PQ values as they are. Apple TV: asks the TV to switch to HDR through `AVDisplayManager` (`AVDisplayCriteria` with a PQ/BT.2020 HEVC format description), tvOS “Match Dynamic Range” setting required, otherwise HDR follows the output chosen in tvOS.
- **LG TVs**: decoding through the TV's video pipeline (NDL), already in place; HDR10 metadata given to the TV through `NDL_DirectVideoSetHDRInfo` so it switches to HDR mode; HDR screen from the capabilities webOS announces. To check first on the G2: it is the least certain point.
- **Second stage**: Android (`Display.getHdrCapabilities`, `MediaCodec` with `KEY_COLOR_TRANSFER` ST 2084, output to a `SurfaceView`); Windows (`R10G10B10A2` swap chain, `SetColorSpace1(RGB_FULL_G2084_NONE_P2020)`, `SetHDRMetaData`, HDR screen from `IDXGIOutput6`); Linux (Wayland colour management, to be studied), Raspberry Pi after. The browser stays in SDR.

## Interfaces between the pieces

Set before writing, so that Kyber, Admin, the core and the apps move forward at the same time.

**Kyber (0022).** `kyclient::VideoSessionConfig` gains `hdr: bool` (C API `kyclient_video_session_config_set_hdr`, `kyclient-rs`: `set_hdr`); it follows `yuv444` to kycontroller (start request, `hdr` field per display), `VideoStreamConfig` and `KymuxStartVideo` (`hdr`). The **host**'s flag decides the display's capture (FP16 or not); each client's flag says what it shows: a client with `hdr: false` on a display captured in HDR gets a separate SDR picture (0020), a client with `hdr: true` shares the HDR picture. `hdr: true` with H.264, x264 or `yuv444`: refused by kyavservice, the display goes back to SDR and kycontroller logs it. WGC ring (0018): the shared memory header gains `format` (a `DXGI_FORMAT`); the agent captures as `R16G16B16A16Float` when the display is HDR for Windows (`IDXGIOutput6::GetDesc1`, `RGB_FULL_G2084_NONE_P2020` colour space), as BGRA otherwise, with no flag to pass to it; kyavserver follows the header's format.

**Admin.** `GET /api/stream/info`: `hdr: { available: bool, reason?: "off" | "windows" | "encoder" | "display" }`. `POST /api/stream/prepare`: `hdr: bool` (single-display session) and `screens[i].hdr: bool`, with `luminance` (one display) and `screens[i].luminance`: `{ "max": 1000, "full_frame": 1000, "min": 0.0, "sdr_white": 203 }` in nits (absent: the HDR calibration defaults); answer: `hdr: [bool]`, the HDR obtained for each display, in the order of `screens` (one element for one display); Admin turns HDR on for the virtual display **before** starting the WGC agents and answering. `POST /api/stream/hdr` (device in session only, otherwise `409`): `{ "device_id": "…", "hdr": [bool], "luminance": [object or null] }`, answer `{ "hdr": [bool] }`, for the bar's chip and a device screen that changes. Lending: `POST /api/lend/borrow` gains `screen.hdr` and `screen.luminance` (same shape), and the friend's virtual display is prepared in HDR the same way. Admin puts on the colour profile and the desktop white together with HDR, before the WGC agents. Admin settings: `hdr: "auto" | "never"` (“When the device asks”, “Never”) and `hdr_desktop_gamma: "2.2" | "srgb"` (“Desktop in HDR”). Original HDR state recorded for each display (`hdr-session.json`) and put back at the end, on restart after a crash and when the display is removed.

**Core.** `StreamSettings.hdr: bool` (`SettingsJson.hdr`, absent: `false`): the app's “HDR” choice for that computer, which the app keeps (on by default on an HDR screen). `Capabilities.hdr: bool`: the decoder reads HEVC Main 10 or 10-bit AV1 and the app can show it. `DeviceScreen.hdr: bool` (several displays) and `SessionParams.screen_hdr: bool` (one display): the device's screen is HDR. `profile::hdr_state(info, settings, caps, screen_hdr) -> HdrState { shown, available, reason }` gives the chip (hidden, active, inactive with the reason translated by the core's catalogue); the session uses it to send `hdr` to `prepare` and to kyclient, and then forces 4:2:0 and HEVC or AV1. During a session: `Session::set_hdr(bool)` (bar, menu) and `Session::screen_hdr_changed(screen, bool)` (device screen) call `POST /api/stream/hdr` and restart the picture of the displays concerned; `Platform::on_hdr(&[bool])` tells the app, at each picture start, which displays are HDR (the renderer switches chain); the notice “The computer couldn't switch to HDR: picture in SDR.” goes through `on_notice`. Statistics: `hdr` in the second and in the last session. Screen values: `HdrLuminance { max, full_frame, min, sdr_white }` (nits, `HdrLuminance::default()` = the defaults), in `DeviceScreen.luminance` and `SessionParams.screen_luminance` (`Option`, absent: defaults); sent to `prepare`, to `POST /api/stream/hdr` (`screen_hdr_changed` also takes the new screen's values) and to `POST /api/lend/borrow`, whose borrow request also carries `screen.hdr` (the friend of a loan has HDR like a host). The app keeps the calibrated values for each of the device's screens; the core clamps them (max 100 to 10,000, full screen ≤ max, black 0 to 1, white 80 to 480). C bridge: `kn_session_set_hdr`, `kn_session_screen_hdr_changed` (with the values), `kn_hdr_state` and the `on_hdr` callback, and the luminance fields in screens and settings; JNI in the second stage.

## Delivery

1. **First version**: Station (Kyber, Admin), core, Apple apps and LG TVs. Tests: the Mac's XDR display, Apple TV on an HDR TV, LG G2, the PC's NVIDIA card.
2. **Next version**: Android, Windows, Linux.

Estimated cost of the first version: about 27 days, 9 of them for HDR calibration (the 22/10 study counted 14, before the WGC capture of secondary displays, the guests' separate picture, the LG app and HDR calibration).

To check first on the PC: that Windows applies the MHC2 profile to the SudoVDA virtual display and that `GetDesc1` returns its brightness values. Otherwise, the other way is for the virtual display to announce these values itself (modified driver, see “Virtual display without workarounds” in the roadmap).
