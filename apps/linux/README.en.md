# KyberNow Link for Linux, the Steam Deck and SteamOS

English · [Français](README.md)

KyberNow Link on Linux: a **native** app on the Rust core of the other Link apps (`apps/core`), bringing up the computer (KyberNow Station: Kyber 0.29 server + KyberNow Admin) full screen, with keyboard and mouse or a controller. One app, three interfaces from the mock-ups ([Claude Design](https://claude.ai/artifact/MgBJNBJ4WEMFvVB6Ujfxhc)):

- **Desktop** (GNOME, KDE…): borderless window with its own title bar, mouse and keyboard, segmented settings, session bar at the top;
- **Handheld** (Steam Deck and consoles under gamescope, 1280 × 800): everything by focus and controller, button legend, animated launch;
- **Living room** (SteamOS on a TV, Steam Machine): the same at ten feet, with the "Ready to play" column.

Under gamescope the app picks the console interface (handheld up to 1280 × 800, living room above), elsewhere the desktop one; App settings › Interface, `--ui desktop|handheld|tv` or `KYBERNOW_UI` force it. Installation: [INSTALLATION.en.md](INSTALLATION.en.md). The decision is in [PROJECT.en.md](../../PROJECT.en.md) ("Linux, Steam Deck and SteamOS client").

**Status (23/10/2026)**: written, built (x86_64 and aarch64) and run on desktop Linux (Ubuntu 24.04 under GNOME, in a virtual machine): home, settings, launch, discovery of Stations; not yet tried in a session, nor on the Steam Deck.

## What the app does

- **Launch** (console): the iPhone and iPad sequence (unlit mark, flicker, name and signature, then the home screen takes the mark over); A or B skip it.
- **Home** (`src/ui/home.rs`): the chosen computer full frame (the last picture of the last session, else Admin's preview, else its signature), its status, the virtual display waiting for it ("1280 × 800 · 90 Hz, like the Deck", VRR), "Connect" and the profiles; on desktop the last session (latency on a one-frame ruler), in the living room "Ready to play" (controllers, sound, screen, decoders), both on an ink panel; at the bottom the computers, those found on the network and adding. With a controller, ‹ › on "Connect" switch computer.
- **Adding** (`src/ui/pair.rs`): six-digit code shown to every Station found (mDNS `_kybernow._tcp`) and accepted in Admin › Link devices; or Admin's address and port (keyboard, Steam keyboard or on-screen keypad), then the first four groups of the fingerprint to compare.
- **Settings** (`src/ui/settings.rs`): per computer, profile; screen (like this screen, 4K, 1440p, 1080p, 800p, 720p), refresh rate, format (those the GPU decodes), 4:4:4 (unavailable), **display** (Auto, synchronised, immediate, variable refresh), bitrate, virtual display, lighter on battery; sound (Auto, stereo, 5.1, 7.1, on the computer); controller seen by the computer, combo, controller test; mouse (Auto, desktop, game), clipboard; transport, reconnection; account, remove. App settings: interface, language, last picture, offered versions. "This computer" / "This device" says what the app found.
- **Session** (`src/ui/session.rs`, `src/device/`): passage (the seam, the neon, the steps, Cancel); full-frame picture; **desktop**: bar at the top when the pointer reaches it (latency and graph, resolution, frames per second, decoder, profile, keys, mouse, full screen, Quit); **console**: controller menu (stream health, tiles: Resume, profiles, sound, controller as mouse, Steam keyboard, keys, stats, lighter or controllers, Disconnect); stats over the picture; capsules (game mouse, controller plugged in, reconnecting); end on error.

## Picture, variable refresh and decoding

- **Decoding** (`src/native/decoder.c`, `src/device/video.rs`): FFmpeg 7.1.1 reduced to decoding, statically linked; **VA-API** when the GPU supports the format (probed at launch through libva: H.264, HEVC, AV1), otherwise the CPU (H.264 and HEVC, slice threads). VA-API frames are copied to NV12 and shown by SDL (one copy per frame: negligible at 1280 × 800; zero-copy DMA-BUF import to come for 4K at 120 Hz). Only the most recent frame is shown.
- **Profiles** brought down to what this machine decodes: HEVC (GPU, else CPU), H.264; AV1 on request when the GPU decodes it; no 4:4:4 (not on Linux yet).
- **Variable refresh rate (VRR)**: the app reads the `vrr_capable` property of the connected displays through DRM (`src/native/display.c`, libdrm, read-only). Display Auto: if the display announces it, each frame is shown as soon as it is decoded (without waiting for vertical sync) and the compositor (gamescope, KWin, Mutter) paces the display on the stream; otherwise synchronised. Synchronised, Immediate and Variable refresh can be forced in the settings. The Steam Deck (LCD and OLED) has no VRR; a display or TV on the dock may have it.
- **Session mode**: the screen's ("like this screen") or the chosen height, at the screen's or the chosen refresh rate; on battery with "Lighter", 60 fps and 20 Mbit/s at most.
- **TV game mode (ALLM)**: handled by the compositor (gamescope), not by the app.

## Sound

Opus decoded by the core's engine (adaptive buffer, loss concealment, clock drift), pulled by SDL's callback (PipeWire or PulseAudio, 256 frames per callback), in stereo, 5.1 or 7.1 depending on the output (Auto) or the setting, in SDL's channel order. Mutable from the session menu. Sound can stay on the computer.

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
│   └── native/          decoder.c (FFmpeg, VA-API), display.c (VRR through DRM)
├── flatpak/             manifest, .desktop, AppStream, icon, licences
└── scripts/             build.sh, sources.sh, flatpak.sh
```

- **Interface**: drawn in memory in *points* (the mock-ups': 1280 × 800 for desktop and Deck, 1920 × 1080 for the living room), multiplied by the screen scale (HiDPI); the signed logo is traced from the `brand/v2` paths (like the Windows app's `Mark.svelte`), sharp at any size.
- **Desktop window**: borderless; the app's title bar drags the window, the edges resize it (`SDL_SetWindowHitTest`), double-click maximises. SDL2 uses X11 (Xwayland on GNOME and KDE, gamescope on the Deck); `KYBERNOW_VIDEO=wayland` tries Wayland. The window class is the app ID (`fr.leslignesclaires.kybernow`).
- **kynput**: client only, with the webOS patch (`apps/webos/kyber-patches/0001`, `--cfg kybernow_webos`): without the server's input injection (libinput, libevdev, X11).
- **VLC**: kyvlcplayer references libvlc, never called: `build.rs` compiles an empty function per `libvlc_*` function, as on Android, iOS and webOS.

## Building

```sh
# Screen captures, without SDL nor the core (on a Mac too):
cargo run -p kybernow-linux --no-default-features -- --preview /tmp/kn-linux en

# On Linux (Ubuntu 24.04: build-essential pkg-config nasm clang libclang-dev
# libsdl2-dev libva-dev libdrm-dev libopus-dev libssl-dev, Rust, cargo-c):
apps/linux/scripts/build.sh                    # → apps/target/release/kybernow

# The Flatpak (flatpak-builder, org.freedesktop.Platform 25.08 runtime,
# rust-stable and llvm20 extensions), for this architecture or the other one:
apps/linux/scripts/flatpak.sh                  # → apps/linux/dist/KyberNow-Link-Linux-<arch>.flatpak
apps/linux/scripts/flatpak.sh x86_64           # from an ARM machine: Rosetta or qemu-user-static
```

`build.sh` applies Kyber's client patches, builds libkynput (`cargo cinstall`, static) and FFmpeg 7.1.1 reduced to decoding (VA-API) into a prefix, then the app. `sources.sh` gathers the needed sources only (about a hundred megabytes instead of Kyber's build outputs), which `flatpak.sh` hands to flatpak-builder in two modules: the dependencies (cached while Kyber does not change), then the app. Kyber 0.29's sources go in `third_party/kyber-desktop` (see [docs/build-notes.en.md](../../docs/build-notes.en.md)).

**Build and test machine**: a Parallels VM "KyberNow Linux" on the development Mac (Ubuntu 24.04 ARM64, GNOME, Flatpak), with Rosetta registered in binfmt (`/etc/binfmt.d/rosetta.conf` and a boot service) to build the Steam Deck's x86_64 Flatpak at near-native speed, and the repository shared (`/media/psf/kyber-apps`). Its virtual GPU (virgl) has no VA-API decoding: the CPU decodes there.

## To do

- On the Steam Deck: an end-to-end session (VA-API on the Van Gogh APU, PipeWire sound, controls and trackpads through Steam Input, Steam keyboard), latency, Gaming Mode.
- On desktop Linux with an AMD or Intel GPU: VA-API in HEVC and AV1, VRR on a FreeSync display, game mouse under Xwayland.
- Zero-copy DMA-BUF import of VA-API frames for 4K at 120 Hz; NVIDIA (NVDEC through FFmpeg).
- A Flatpak repository or Flathub, so updates install by themselves.
