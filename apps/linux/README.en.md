# KyberNow Link for Linux, the Steam Deck, SteamOS and the Raspberry Pi

English · [Français](README.md)

KyberNow Link on Linux: a **native** app on the Rust core of the other Link apps (`apps/core`), bringing up the computer (KyberNow Station: Kyber 0.29 server + KyberNow Admin) full screen, with keyboard and mouse or a controller. One app, three interfaces from the mock-ups ([Claude Design](https://claude.ai/artifact/MgBJNBJ4WEMFvVB6Ujfxhc); Raspberry Pi: https://claude.ai/artifact/6crVeg3QyrA5vs8TRhWRft):

- **Desktop** (GNOME, KDE…): borderless window with its own title bar, mouse and keyboard, segmented settings, session bar at the top;
- **Handheld** (Steam Deck and consoles under gamescope, 1280 × 800): everything by focus and controller, button legend, animated launch;
- **Living room** (SteamOS on a TV, Steam Machine): the same at ten feet, with the controllers line on the home screen.

Under gamescope the app picks the console interface (handheld up to 1280 × 800, living room above), elsewhere the desktop one; App settings › Interface, `--ui desktop|handheld|tv` or `KYBERNOW_UI` force it. Installation: [INSTALLATION.en.md](INSTALLATION.en.md). The decision is in [PROJECT.en.md](../../PROJECT.en.md) ("Linux, Steam Deck and SteamOS client").

**Status**: built (x86_64 and aarch64) and tried in a session on desktop Linux (Ubuntu 24.04 under GNOME, in a virtual machine, CPU decoding); not yet tried on the Steam Deck. **Raspberry Pi**: the .deb package and KyberNow Link OS are built in an ARM64 virtual machine, the image boots under QEMU emulation; nothing has run on a board yet (see "Not yet tried").

## What the app does

- **Launch** (console): the iPhone and iPad sequence (unlit mark, flicker, name and signature, then the home screen takes the mark over); A or B skip it; `KYBERNOW_REDUCE_MOTION=1` tones it down (the mark lights up without flickering, then a fade).
- **Home** (`src/ui/home.rs`), following the screens shared by the Link apps ([docs/link-ui.en.md](../../docs/link-ui.en.md)): the chosen computer full frame (the last image of the last session, else Admin's preview, else its signature), its status ("Online", "Offline", "Searching…"), its name, its system, the display the session will ask for ("1280 × 800 display at 90 Hz, sized for this screen", "1920 × 1080 display at 60 Hz" when custom, "The computer's own displays, as they are" without a virtual display), "Connect" (offline: "Wake the computer" when the app knows one of the computer's network cards, then "Waking the computer…" until it answers, polled every 2 s for 2 min at most; otherwise "Try again"), the profiles and their sentence ("Custom" when the settings move away from them: format, bitrate, 4:4:4 in Game, reliable transport outside Desktop; picking a profile goes back to it), "Settings" (gear button, Y with a controller); in the living room and on KyberNow Link OS, "2 controllers". Nothing technical: the figures are in Settings › Statistics. At the bottom the computers (name and status), those found on the network ("On your network") and adding. With a controller, ‹ › on the main button switch computer.
- **Adding** (`src/ui/pair.rs`): six-digit code shown to every Station found (mDNS `_kybernow._tcp`) and accepted in Admin › Link devices; or Admin's address and port (keyboard, Steam keyboard or on-screen keypad), then the first four groups of the fingerprint to compare.
- **Computer settings** (`src/ui/settings.rs`, `src/ui/station.rs`): on desktop, from the Raspberry mock-up: on the left the profile (a row of buttons) and the sections; on the right the chosen section, one field per setting (label, segments with their "graphics card", "Pi decoder", "processor" tags, unavailable values struck through, help); breadcrumb in the title bar (logo › computer › Settings, the computer goes back home). With a controller, "Settings for {name}" (in French, `i18n::settings_for` elides "de" before a vowel or an h), the sections on the left, the ‹ › values on the right; help wraps rather than being cut. The profile first (Game, Desktop, Max, Custom), then the six shared sections:
  - **Picture**: resolution (match this display, 4K, 1440p, 1080p, 800p, 720p), refresh rate, virtual display, video format (those the graphics card decodes), sharp text (4:4:4, optional, HEVC), bitrate, frame display (Auto, synced, immediate, variable refresh), lighter on battery;
  - **Sound**: Auto, stereo, 5.1, 7.1, on the computer;
  - **Controls**: controller seen by the computer (Xbox 360, As is), L1 + R1 + Start + Select, mouse (Auto, desktop mouse, game mouse), shared clipboard;
  - **Network**: picture transport (Fast, Reliable), reconnect automatically, Wake on LAN ("Ready" and the card learnt from Admin, or "No address yet: connect to this computer once."); as an advanced setting, the MAC address by hand (keyboard or hexadecimal keypad), for a Station that does not give its cards;
  - **Statistics**: the Mac app's "Last session" card (median in large type; a ruler from 0 to 20 ms, more if the measurement goes beyond, 160, 120 and 60 Hz marks, a band from the median to the 95th percentile; "95% of frames under …"; duration and day; resolution, refresh rate, format, bitrate), then "This computer": system, graphics card, Admin's address and port, four groups of the fingerprint;
  - **Station**: version and state ("Up to date", "Version 0.5.1 available", the one that installs with the installer, Station too old, restart or update in progress, done); Admin: "Open Admin" on desktop (`xdg-open`), a QR code of `https://address:port/` on the Deck, in the living room and on KyberNow Link OS (`qrcodegen` crate, modules aligned on pixels); "Restart the Station", "Update the Station" when a version installs from a device; the account ("This device's own access": adding by code gives the device its own account, and Admin 0.5.0 confirms it); "Remove this computer", after confirmation.
- **App settings**: interface, language, keep the last image (for every computer), offered versions, checking for a version, controller test; the "This device" card says what the app found (system, graphics card, decoders, screen, sound; on the Pi, memory, decoders, HDMI output, network, temperature).
- **Station** (`src/device/jobs.rs`, core: `Console::station_restart`, `station_update`): a device in a session refuses first ("{device} is connected", then "Restart anyway" sends the request again with `force`); too soon, a version that needs the installer, Station 0.4.9 or older: the contract's sentence. Once Admin accepts, the app polls the Station every 2 s (`GET /api/stream/info`) until it is back (and on the new version for an update); beyond three minutes (ten for an update): "The Station is taking a while to come back. Open Admin to find out more."; an update that came back on the old version: "The update did not complete. Open Admin to find out more."
- **Wake on LAN** (`src/device/jobs.rs`, core: `kybernow_core::wake`): on each answer from Admin, the app keeps the computer's network cards (MAC address, IPv4 address, broadcast address; an empty list, from a 0.4.9 Station, does not erase them) in `kybernow.json`; "Wake the computer" sends the magic packet to each of them, and to the MAC address entered by hand (with the computer's address for the direct send).
- **Session** (`src/ui/session.rs`, `src/device/`): connecting shows its three steps ("Preparing the display", "Connecting", "First frames") over the computer's background, with the neon in the seam, and Cancel; then the picture alone, with nothing over it when it arrives; a failure before the first frame says "The connection failed" and the reason. When the picture does not come, the core restarts the Station once (last resort): "The Station is restarting…" takes the place of the "Connecting" step, then the steps start again. **Desktop**: bar at the top when the pointer reaches it (latency and graph, resolution, frames per second, decoder, profile, keys, mouse, full screen, Disconnect); **console**: controller menu (stream health, tiles: Resume, profiles, sound, controller as mouse, Steam keyboard, keys, statistics, lighter or controllers, Disconnect); statistics over the picture, on demand; capsules for what changes later (game mouse, controller plugged in, reconnecting); end on error. At the end of a session of at least 3 s, the app keeps for the computer the median, the 95th percentile, the duration, the date, the resolution, the refresh rate, the format and the bitrate (`LastSession`; files written by 0.4.x still load, without a separate refresh rate or format).

## Picture, variable refresh and decoding

- **Decoding** (`src/native/decoder.c`, `src/device/video.rs`): FFmpeg 7.1.1 reduced to decoding, statically linked; **VA-API** when the GPU supports the format (probed at launch through libva: H.264, HEVC, AV1), otherwise the CPU (H.264 and HEVC, slice threads). FFmpeg is built without NVDEC: an NVIDIA card only decodes through a VA-API driver. VA-API frames are copied to NV12 and shown by SDL (one copy per frame: negligible at 1280 × 800). Only the most recent frame is shown.
- **Profiles** brought down to what this machine decodes: HEVC (GPU, else CPU), H.264; AV1 on request when the GPU decodes it; **optional 4:4:4** (Settings › Picture › 4:4:4 colour, HEVC only, off by default): the GPU decodes it when VA-API supports HEVC 4:4:4 (recent Intel), otherwise the CPU; as SDL has no YUV 4:4:4 texture, `decoder.c` converts each frame to BGRA on the decoder thread (fixed point, BT.709 or BT.601, up to four threads). Measured in the VM (Apple M, CPU): 3.4 ms per 1440p frame, decoding and conversion included, against 1.9 ms in 4:2:0. Offline test: `tests/video/run.sh` (colours within ±3, formats, timing).
- **Variable refresh rate (VRR)**: the app reads the `vrr_capable` property of the connected displays through DRM (`src/native/display.c`, libdrm, read-only). Display Auto: if the display announces it, each frame is shown as soon as it is decoded (without waiting for vertical sync) and the compositor (gamescope, KWin, Mutter) paces the display on the stream; otherwise synchronised. Synchronised, Immediate and Variable refresh can be forced in the settings. The Steam Deck (LCD and OLED) has no VRR; a display or TV on the dock may have it.
- **Session mode**: the screen's ("like this screen") or the chosen height, at the screen's or the chosen refresh rate; on battery with "Lighter", 60 fps and 20 Mbit/s at most.
- **TV game mode (ALLM)**: handled by the compositor (gamescope), not by the app.

## Raspberry Pi

The same app, on Raspberry Pi OS (Debian 13 "trixie", arm64), as a `.deb` package, or alone on the card with KyberNow Link OS (below); the decision is in [PROJECT.en.md](../../PROJECT.en.md) ("Raspberry Pi client"), mock-ups: https://claude.ai/artifact/6crVeg3QyrA5vs8TRhWRft.

- **Detection** (`src/device/system.rs`, `pi()`): the device tree (`/proc/device-tree/model` → "Raspberry Pi 5", "Compute Module 5", "Raspberry Pi 3 B+"; `compatible` → generation 5: BCM2712, 4: BCM2711, 3: BCM2837), memory (`/proc/meminfo`, rounded to the GB), V4L2 decoders (`/sys/class/video4linux/*/name`: `rpivid` for HEVC on Pi 4 and 5, `bcm2835-codec` for H.264 on Pi 3 and 4), temperature (thermal zone 0, re-read every 5 s), the OS ("Raspberry Pi OS (Trixie)", from `/etc/rpi-issue`). The connected display's modes are read through DRM (`display.c`, `kn_modes_probe`): highest refresh at 1080p and the largest mode.
- **What the output allows** (`Pi` in `app.rs`): 4K at 60 Hz at most on Pi 4 and 5 (HDMI 2.0), 1080p before; 1080p at 120 Hz when the display announces it (not yet verified on a board); no VRR. Screen, refresh and format choices beyond that are greyed (struck through); tags say who decodes; 4:4:4 goes to the CPU (1080p at most); the help explains ("4K in HEVC only, decoded by the Pi. Above 1080p, H.264 goes through the CPU and stutters").
- **App settings**: the "This device" card gives the model, memory and system, the Pi's decoders, the HDMI output, the network and the temperature; the version carries the architecture ("0.4.8 · aarch64"). Captures: `--preview` produces `rpi-01-accueil.png`, `rpi-07b-reglages-image.png`, `rpi-07f-reglages-statistiques.png`… (simulated machine).
- **Decoding** (`decoder.c`, `KN_HW_V4L2` mode): HEVC through the `hevc_v4l2request` hwaccel of Raspberry Pi's FFmpeg ([jc-kynesim/rpi-ffmpeg](https://github.com/jc-kynesim/rpi-ffmpeg), branch `dev/7.1.1/rpi_import_1`, built by `build.sh --rpi` with `--enable-v4l2-request --enable-sand --enable-libudev`), DRM device (the first `/dev/dri/cardN` that opens), `DRM_PRIME` frames in the SAND layout copied to linear NV12 by `av_hwframe_transfer_data` (one copy per frame, on KyberNow Link OS too); H.264 through the `h264_v4l2m2m` decoder (M2M, in every Linux FFmpeg: `--enable-v4l2-m2m`), frames in memory. On refusal or repeated errors, the CPU decodes. The official FFmpeg (Flatpak) has no V4L2 request hwaccel: the aarch64 Flatpak on a Pi decodes HEVC on the CPU.
- **Package** (`scripts/deb.sh`, on an arm64 Linux: the VM, or a Pi): `build.sh --rpi --install`, launcher, AppStream file, icons, licences, dependencies read from the binary (`dpkg-shlibdeps`), `dpkg-deb` → `dist/KyberNow-Link-RaspberryPi-arm64.deb` (published with every release; `release-flatpaks.sh` builds it in the VM after the Flatpaks) and `kybernow-link_<version>_arm64.deb`. **apt repository**: `scripts/apt-repo.sh` assembles a repository from the .deb files in `dist` (`pool/` and `dists/stable/`, through `dpkg-scanpackages` and `apt-ftparchive`), signed when `KYBERNOW_APT_GPG_KEY` names a GPG key (public key exported as `kybernow.gpg`), to be served as is by a static site; `release.sh` does not call it and no repository is published. The .deb app (outside Flatpak, on arm64) looks for its update in `KyberNow-Link-RaspberryPi-arm64.deb` of the published release and announces it.
- **KyberNow Link OS**, the image to flash: [os/README.en.md](os/README.en.md). Raspberry Pi OS Lite through pi-gen, Link alone full screen on KMS/DRM, Plymouth with the unlit mark then the brand's ignition with sound, first boot (controller, language, network, use, computer), device settings, standby and HDMI-CEC, whole-OS A/B update (tryboot). On the app side: `src/device/os.rs`, `src/device/chime.rs`, `src/ui/setup.rs`, `src/ui/device.rs`; OS mode when `KYBERNOW_OS=1` or `/etc/kybernow-os/release` exists.
- **Not yet verified on a board** (no Raspberry Pi available; everything is built in the Ubuntu ARM64 VM): the HEVC decoder (request API, SAND layout, per-frame latency), H.264 M2M, the 1080p 120 Hz output, multichannel HDMI sound, Bluetooth controllers, the package on Raspberry Pi OS Trixie (dependencies computed on Ubuntu 24.04, names Debian 13 has too: `libsdl2-2.0-0`, `libopus0`, `libva2`, `libva-drm2`, `libdrm2`, `libudev1`).

## Sound

Opus decoded by the core's engine (adaptive buffer, loss concealment, clock drift), pulled by SDL's callback (PipeWire or PulseAudio; ALSA on KyberNow Link OS; 256 frames per callback), in stereo, 5.1 or 7.1 depending on the output (Auto) or the setting, in SDL's channel order. Mutable from the session menu. Sound can stay on the computer.

## Inputs

- **Keyboard**: SDL's USB scancodes converted to Windows scancodes (the `keycode` crate table, kynput's); in full screen during the session the keyboard is grabbed (Alt+Tab, Super… go to the computer). App shortcuts: Ctrl+Alt+Shift + D, F, M.
- **Mouse**: absolute position mapped to the computer's screen; **game mouse** (relative, `SDL_SetRelativeMouseMode`) when a game is full screen on the computer and hides its pointer (Auto, core rules), or forced; holding Esc for a second releases it. The computer's pointer becomes the system pointer at the picture's scale (desktop), or is drawn by the app (game mouse, console).
- **Steam Deck trackpads and touchscreen**: mouse through Steam Input; a touch at the top of the screen opens the menu.
- **Controllers**: SDL GameController, four players, rumble (and triggers), announced once per link; Steam Input presents the Deck's controls as an Xbox controller. Controller as mouse (left stick, RT faster, LT finer, right stick wheel, A / B / X clicks). L1 + R1 + Start + Select: menu.
- **Text**: "Keyboard" in the menu opens Steam's keyboard (`steam://open/keyboard`); characters are then sent as Unicode.
- **Clipboard**: text both ways (the core's bridge; SDL read and written from the main thread).

## Architecture

```
apps/linux/
├── src/
│   ├── main.rs          launch (--preview: screenshots, --ui: interface)
│   ├── app.rs           state, spatial and mouse navigation, settings (no SDL nor core)
│   ├── model.rs         computers, settings ($XDG_CONFIG_HOME/kybernow/kybernow.json, 0600)
│   ├── ui/              drawing (tiny-skia, fontdue): launch, home, adding, settings,
│   │                    controllers, session; logo traced from the brand geometry
│   ├── device/          SDL (window, events, sound), session (core Platform), picture
│   │                    (FFmpeg), machine (DMI, VA-API, DRM), controllers, background jobs, log
│   └── native/          decoder.c (FFmpeg: VA-API, the Pi's V4L2), display.c (VRR and modes through DRM)
├── flatpak/             manifest, .desktop, AppStream, icon, licences
├── os/                  KyberNow Link OS: Raspberry Pi image (pi-gen, A/B, Plymouth, services), see os/README.en.md
├── scripts/             build.sh (--rpi), sources.sh, flatpak.sh, release-flatpaks.sh, deb.sh, apt-repo.sh
└── tests/video/         offline decoder test (run.sh, check.c)
```

- **Interface**: drawn in memory in *points* (the mock-ups': 1280 × 800 for desktop and Deck, 1920 × 1080 for the living room), multiplied by the screen scale (HiDPI); the signed logo is traced from the `brand/v2` paths (like the Windows app's `Mark.svelte`), sharp at any size.
- **Desktop window**: borderless; the app's title bar drags the window, the edges resize it (`SDL_SetWindowHitTest`), double-click maximises. SDL2 uses X11 (Xwayland on GNOME and KDE, gamescope on the Deck); `KYBERNOW_VIDEO=wayland` tries Wayland. The window class is the app ID (`fr.leslignesclaires.kybernow`).
- **kynput**: client only, with the webOS patch (`apps/webos/kyber-patches/0001`, `--cfg kybernow_webos`): without the server's input injection (libinput, libevdev, X11).
- **VLC**: kyvlcplayer references libvlc, never called: `build.rs` compiles an empty function per `libvlc_*` function, as on Android, iOS and webOS.

## Building

```sh
# Screen captures, without SDL nor the core (on a Mac too):
# desktop (2560 × 1600 and a 960 × 600 window), Deck, living room, Raspberry Pi,
# KyberNow Link OS; home (online, offline, Custom, no computer), each computer
# settings section (Network without an address; Statistics with and without a
# session; Station up to date, version available, in progress, done, installer,
# old, confirmations), waking, connecting, Station restarting, failure,
# picture alone, session:
cargo run --release -p kybernow-linux --no-default-features -- --preview /tmp/kn-linux en

# On Linux (Ubuntu 24.04: build-essential pkg-config nasm clang libclang-dev
# libsdl2-dev libva-dev libdrm-dev libopus-dev, Rust 1.89 or later, cargo-c):
apps/linux/scripts/build.sh                    # → apps/target/release/kybernow

# The Flatpak (flatpak-builder, org.freedesktop.Platform 25.08 runtime,
# rust-stable and llvm20 extensions), for this architecture or the other one:
apps/linux/scripts/flatpak.sh                  # → apps/linux/dist/KyberNow-Link-Linux-<arch>.flatpak
apps/linux/scripts/flatpak.sh x86_64           # from an ARM machine: Rosetta or qemu-user-static

# From the Mac, both Flatpaks then the Pi's .deb in the "KyberNow Linux" VM (what release.sh does):
apps/linux/scripts/release-flatpaks.sh [--check] [DIR]

# Raspberry Pi, on an arm64 Linux (the VM, or a Pi; libudev-dev as well):
apps/linux/scripts/build.sh --rpi              # Raspberry Pi's FFmpeg, HEVC V4L2 request decoder
apps/linux/scripts/deb.sh                      # → apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb
apps/linux/scripts/apt-repo.sh [DIR]           # → dist/apt (KYBERNOW_APT_GPG_KEY to sign)

# The decoder, offline (after build.sh; ffmpeg and x265 generate the streams):
apps/linux/tests/video/run.sh [--hw]
```

`build.sh` applies Kyber's client patches, builds libkynput (`cargo cinstall`, static) and FFmpeg 7.1.1 reduced to decoding (VA-API) into a prefix, then the app. `sources.sh` gathers the needed sources only (about a hundred megabytes instead of Kyber's build outputs), which `flatpak.sh` hands to flatpak-builder in two modules: the dependencies (cached while Kyber does not change), then the app. Kyber 0.29's sources go in `third_party/kyber-desktop` (see [docs/build-notes.en.md](../../docs/build-notes.en.md)).

**Build and test machine**: a Parallels VM "KyberNow Linux" on the development Mac (Ubuntu 24.04 ARM64, GNOME, Flatpak), with Rosetta registered in binfmt (`/etc/binfmt.d/rosetta.conf` and a boot service) to build the Steam Deck's x86_64 Flatpak at near-native speed, and the repository shared (`/media/psf/kyber-apps`). Its virtual GPU (virgl) has no VA-API decoding: the CPU decodes there.

## Not yet tried

- On the Steam Deck: an end-to-end session (VA-API on the Van Gogh APU, PipeWire sound, controls and trackpads through Steam Input, Steam keyboard), latency, Gaming Mode.
- On desktop Linux with an AMD or Intel GPU: VA-API in HEVC and AV1, VRR on a FreeSync display, game mouse under Xwayland.
- Raspberry Pi, on a board: see "Not yet verified on a board" above; KyberNow Link OS: [os/README.en.md](os/README.en.md), "Not yet verified on a board".
