# PyroWave: Apple TV, iPhone and iPad

English · [Français](pyrowave-client-apple.md)

Client section of [pyrowave.en.md](pyrowave.en.md) (the contract and the Station), to be merged into it. Covers the Apple TV app (`apps/tvos`) and the iPhone and iPad app (`apps/ios`). The decoder is the Mac's, in KyberKit: [pyrowave-client-mac.en.md](pyrowave-client-mac.en.md). Rules and texts: [link-ui.en.md](link-ui.en.md), “PyroWave”.

## Where PyroWave will be unavailable

The Metal port needs a GPU of the Apple7 family or later (A14, M1). Elsewhere, the launch probe answers “unavailable” and “Video format” shows PyroWave inactive, with “This Apple TV can't decode PyroWave.”, “This iPhone can't decode PyroWave.” or “This iPad can't decode PyroWave.”.

| Device | PyroWave |
|---|---|
| Apple TV 4K 2022 (A15, AppleTV14,1) | available |
| Apple TV 4K 2021 (A12, AppleTV11,1), Apple TV 4K 2017 (A10X), Apple TV HD (A8) | **unavailable** |
| iPhone 12 and later (A14 onwards), iPhone SE 3rd generation (A15) | available |
| iPhone XS, XR, 11, SE 2nd generation (A12, A13) | **unavailable** |
| iPad Air 4 and later, iPad 10th generation and later, iPad mini 6 and later, iPad Pro M1 and later | available |
| iPad 7th, 8th and 9th generation, iPad mini 5, iPad Air 3, iPad Pro 2018 and 2020 (A10 to A13, A12X/Z) | **unavailable** |
| Simulators (iOS, tvOS) | **unavailable**: the simulator's Metal has no Apple7 family (seen on 25/11/2026: “GPU non pris en charge”) |

Deduced from the GPU family; only the simulator's answer has been seen.

## Building

- **Decoder**: the Apple TV and iPhone/iPad projects build the Metal port into KyberKit (`pyrowave_common.mm`, `pyrowave_decoder.mm`, `pyrowave_bitstream.cpp`, ARC, C++17, `CPyroWave` module) and `KyberKit/PyroWaveDecoder.swift`, like the Mac. No tvOS- or iOS-specific setting was needed: checked on 25/11/2026 for device and simulator (`appletvos`, `appletvsimulator`, `iphoneos`, `iphonesimulator`, Xcode 26). The port only uses shared storage and `r8Unorm`/`r16Unorm` textures, which the Apple GPUs of Apple TV and iPhone take.
- **Kyber**: the kyclient patch 0025 (`VideoCodec::PyroWave`, `fallback_codec`) and the core's `pyrowave` feature are only applied with an explicit flag, off by default, like Android's `build-native.sh --kyber-pyrowave`: `apps/tvos/scripts/build-kyber-tvos.sh --kyber-pyrowave`, `apps/apple/scripts/build-apple-mobile.sh tvos|ios --kyber-pyrowave` (`kybernow-apple`'s `kyber-pyrowave` feature, which turns on `kybernow-core/pyrowave`), `apps/core/scripts/build-apple.sh --kyber-pyrowave`. Without them, the app decodes PyroWave and announces it to the core, but the core never asks Kyber for it. As `third_party/` is shared by every worktree, the patch is only applied there on request.

## The probe and the cable

- **Probe**: `DecoderCapabilities.prepare()` at launch of both apps, off the main thread (`KyberNowTVApp`, `KyberNowApp`): PyroWave device, 128 × 128 decoder, test image decoded and read back (`PyroWaveDecoder.probe()`, see the Mac client). The result goes to the core in the decoder capabilities (`pyrowave`, `SessionEngine`), and into the decoders summary of the app settings (“…, PyroWave”).
- **Cable**: `NetworkLink` (KyberKit, `PyroWaveSupport.swift`), an `NWPathMonitor` started at launch: connected by cable when the default path goes through a `.wiredEthernet` interface (the Apple TV's Ethernet; a USB-C or Lightning adapter on iPhone and iPad). Sent to the core in the session settings (`SessionEngine.Settings.wired`, `StreamSettings.wired`): “Auto” only takes PyroWave by cable, outside the relay, if the Station offers it and the device decodes it (`profile::pyrowave_format`). The settings read the link on their own (Apple TV: `watchLink`; iPhone, iPad: `SettingsLink`) for their help texts.
- **Station announcement**: `HostInfo.pyrowave` (`PyroWaveOffer`, `pyrowave` of `GET /api/stream/info`, an `{available, reason}` object or a boolean; missing: a Station from before PyroWave, nothing changes in the app).

## Interface

link-ui texts, in the apps' catalogs (the `Localizable.xcstrings` of Apple TV, iPhone, and Mac for the shared `PyroWaveText` texts, `apps/macos/KyberNow/LinkUI.swift`).

- **“Auto” in the Apple apps**: a profile (Game, Desktop, Max); the core picks the format (the core's rule). “Video format” only shows “Auto” and “PyroWave” if the Station knows PyroWave (or it is already chosen); otherwise the list stays as before. “Auto” is checked when a profile is active; picking it from “Custom” brings back the Game profile. Picking a format makes the profile “Custom”. PyroWave chosen: the bitrate “Auto” would take by cable for the session's resolution (`PyroWaveRate`, like the core's `PYROWAVE_MBPS`: 800 Mbit/s at 4K60, ≈ 280 at 1080p60; 1,200 in 4:4:4); leaving it brings the bitrate down to 80 Mbit/s at most.
- **Apple TV, Settings › Picture › Video format** (Apple TV mockup): when PyroWave is shown, the row becomes a focus list on the right (Auto, HEVC, H.264, AV1, PyroWave; a check on the choice), readable from the couch; on the left, the help (reason for an inactive PyroWave, PyroWave chosen, Auto by cable or without) and, with PyroWave chosen on Wi-Fi or away from home, the amber warning under the help. “Sharp text (4:4:4)”: “With PyroWave, almost free: only the bitrate goes up.”. The bitrate no longer warns above 150 Mbit/s in PyroWave (gauge up to 1,500).
- **Apple TV, session**: start-of-session banner (PyroWave chosen, picture received in PyroWave, without cable or through the relay, once per session, 8 s): “PyroWave on Wi-Fi” or “PyroWave away from home”, “The picture may stutter.”, and “Switch to HEVC” behind the Back glyph: the remote has no focus during play, the button is in the menu. Session menu: the amber tile “PyroWave on Wi-Fi” / “PyroWave away from home”, detail “Switch to HEVC”, which restarts the picture in HEVC for this session only (the computer's choice stays; a profile picked afterwards takes over). Stats row: the format says “PyroWave”, and “Lost frames 0.8 %” (share of the frames of the last 30 seconds) replaces the loss count when the picture is in PyroWave.
- **iPhone, iPad, Settings › Picture › Video format**: Auto, HEVC, H.264, AV1, PyroWave (inactive, struck through, with its reason in the help); the amber warning under the choice (`SettingsWarning`, `choice(…, warning:)`); the “Sharp text (4:4:4)” help; in PyroWave, the bitrate is no longer amber above Wi-Fi comfort (the format warning is enough).
- **iPhone, iPad, session**: start-of-session banner, as a session capsule: title, “The picture may stutter.”, “Switch to HEVC” (not for a guest or a friend) and the close button; 8 s. Bar: the amber chip “PyroWave on Wi-Fi” / “PyroWave away from home” next to the stats, which opens them. Stats (iPhone sheet, iPad panel): the “PyroWave” format and the “Lost frames” tile (share of the frames of the last 30 seconds), on top of the losses.
- **Received format**: `SessionEngine.receivedCodec` (last codec packet), read every second: the chip, tile, banner and stats format follow what the Station really sends (PyroWave picked by “Auto”, or its `fallback_codec`).
- **Resolution lowered**: the core says it (`KN_NOTICE_RESOLUTION_LOWERED`, `SessionEngine.Notice.resolutionLowered` in KyberKit); the Apple TV and the iPhone/iPad show its text as a notice for a few seconds (“Resolution lowered to {w} × {h} to fit in {n} Mbit/s.”). Not yet in their statistics (`SessionEngine.lowered` provides it, as on the Mac), nor the link rate given to the core (`LinkRate`, `SessionEngine.setLinkMbps`, wired on the Mac only). Written without being compiled for iPhone and Apple TV (no device libraries in the Mac worktree).

Demo (Debug, simulator): `-demo YES` with `-demoPyroWave wifi|relay` (session in PyroWave: banner, chip or tile, “Lost frames”; Apple TV `-screen passage|menu`, iPhone `-screen bar`), `-demoPyroWaveOffer on|off` (the Tower announces PyroWave), `-demoCodec pyrowave` (PyroWave chosen for the Tower), `-demoPyroWaveDecodes YES` (the simulator counted as decoding PyroWave), `-demoLink Wi-Fi|Ethernet` (Apple TV) or `wifi|ethernet` (iPhone, iPad) for the help texts.

## Code shared with the Mac

The KyberKit and `apps/macos/KyberNow` changes serve all three apps: `VideoCodec.pyrowave`, `HostInfo.pyrowave`, `SessionEngine.Settings.wired` and `receivedCodec`, `PyroWaveSupport.swift` (announcement, link, choice, bitrate, timings), `PyroWaveText` (texts), `ConnectionSettings.decodes444(.pyrowave)`. The Mac app builds with them (checked on 25/11/2026) but does not have the PyroWave interface nor `wired` yet (it sends `false`): that is the Mac client's next step.

## Offline test (`apps/apple/tests/pyrowave`)

The Mac test (`apps/macos/tests/pyrowave`) adapted to devices: a small separate app, **PyroWaveBench** (`fr.leslignesclaires.kybernow.pyrowavebench`, Apple TV and iPhone/iPad), which does not replace the installed KyberNow Link. No ffmpeg on the device: the pattern is drawn in place (scrolling Y ramp, one-pixel checkerboard, colour bars, BT.709 limited range), 60 frames encoded by the port's Metal encoder at 1.2 bits per pixel at 60 fps (one frame = its packets end to end, like a Station media packet), then decoded by `VideoDecoder` (“pwv1”) → `PyroWaveDecoder`. For 1080p and 4K (and 1440p on iPhone, iPad), in 4:2:0 and 4:4:4, SDR then HDR10 (10-bit output): the probe, the decode time paced at 60 fps (mean, median, 95th percentile, maximum), frames per second back to back, the first frame's PSNR, a truncated frame skipped without breaking the next; “OK” (code 0) or “ÉCHEC” (code 1). The simulator has no Apple7 family: the test only runs the probe there (“Sonde : PyroWave indisponible”, seen on 25/11/2026).

`run.sh tv|ios <device> [resolutions]` builds the app, signs it, installs it (`devicectl`) and launches it while reading its output. Signing: Apple TV with tvOS's generic team profile (“tvOS Team Provisioning Profile: *”) when it covers the device, registering nothing; otherwise, and for iPhone and iPad, Xcode's automatic signing (`-allowProvisioningUpdates`), which registers the test app's identifier on the developer account the first time.

**Not measured yet**: on 25/11/2026, PyroWaveBench was built, signed and installed on the “Apple TV du salon” (A15), but tvOS refused to launch it as the Apple TV was asleep (“System is asleep - foreground app launch forbidden”); it was not woken (the TV would have turned on). The iPhone has no generic profile: the test needs the identifier registered (below).

### Steps for the author

On the Mac, in the branch worktree (or after merging), Xcode 26, `xcodegen`:

1. **Apple TV du salon** (A15) awake, on the home screen: `apps/apple/tests/pyrowave/run.sh tv "Apple TV du salon"`. Expected: “GPU : Apple A15 GPU · Apple7 : oui”, “Sonde : PyroWave disponible (… ms)”, then for 1920x1080 and 3840x2160, in 4:2:0 and 4:4:4: “Encodé : 60 images …”, two blocks (SDR, HDR10) with “cadencé 60 i/s : 60/60 images, moyenne … ms”, “à la chaîne : … i/s”, “PSNR Y … dB” (at least 40 dB in Y), “image tronquée puis entière : 1 sortie(s) (tronquée sautée, suivante décodée)”, then “OK”. Keep the whole output (the 4K decode time is the figure to keep). Shorter: `run.sh tv "Apple TV du salon" 3840x2160`. The app stays on the Apple TV (“PyroWaveBench” icon); it can also be launched with the remote: the lines show on screen.
2. **Apple TV de l'atelier** (A12, 2021): `run.sh tv "Apple TV de l'atelier"` (automatic signing: the generic profile does not cover it). Expected: “Apple7 : non”, “Sonde : PyroWave indisponible”, “ÉCHEC” (code 1), nothing more.
3. **iPhone 18 Pro Max** unlocked: `run.sh ios "iPhone 18 Pro Max de Matthieu"` (the first time, Xcode registers `fr.leslignesclaires.kybernow.pyrowavebench` on the team; the iPhone may ask to trust the developer in Settings › General › VPN & Device Management). Expected as on Apple TV, with 2560x1440 too.
4. **The apps** (install: `apps/tvos/SIDELOAD.en.md`, `apps/ios/SIDELOAD.en.md`, or Xcode): open the app, then App settings: the decoders line says “…, PyroWave” on the Apple TV du salon and the iPhone, not on the Apple TV de l'atelier. Log: `log stream --device … --predicate 'subsystem == "fr.leslignesclaires.kybernow"'` (or Console.app): “PyroWave prêt en … ms (…)” or “PyroWave : GPU non pris en charge”, and “lien réseau : Ethernet” or “sans câble”.
5. **A real session**, once the Station runs PyroWave and the SDK is built with `--kyber-pyrowave` (`build-kyber-tvos.sh --kyber-pyrowave`, `build-apple-mobile.sh tvos|ios --kyber-pyrowave`): Apple TV on Ethernet on “Auto” (Game profile): the picture arrives in PyroWave (menu stats: “PyroWave 4:2:0”, “Lost frames”). Then on Wi-Fi, PyroWave chosen by hand: the settings warning, the start banner, the menu tile; “Switch to HEVC”: the picture restarts in HEVC. Same on the iPhone (USB-C Ethernet adapter for “Auto”), with the bar chip.

## Test status

| What | Status |
|---|---|
| Build (device and simulator, tvOS and iOS) | done on 25/11/2026; the Mac app builds too |
| Probe on the simulator | “GPU non pris en charge”, PyroWave unavailable, the app starts (25/11/2026) |
| Interface: settings, banner, tile or chip, stats | seen on the Apple TV 4K and iPhone 16 Pro simulators (tvOS 27.0, iOS 27.0), with the demos (25/11/2026); not seen on iPad, nor on a device |
| Decoding, decode time, HDR10 on a real GPU | **to measure** (steps above) |
| 2021 Apple TV: probe unavailable | deduced, to confirm |
| `wired` through a real cable (Apple TV on Ethernet, USB-C adapter) | to try (the simulator takes the Mac's link) |
| Full session to the Station | to try (patch 0025 and `--kyber-pyrowave`) |

## What is waiting

- The Station's kyclient patch and the core's `pyrowave` feature turned on for every app: then remove the `--kyber-pyrowave` flag.
- “Resolution lowered”: notice wired; statistics line and link rate still to do (as on the Mac).
- The 2022 Apple TV 4K only has Gigabit Ethernet (and only the 128 GB model): PyroWave at 4K60 (800 Mbit/s by default) is close to the limit there; to watch in “Lost frames”.
