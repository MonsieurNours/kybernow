# KyberNow Link for LG TVs (webOS)

English · [Français](README.md)

KyberNow Link on LG TVs: a **native** app (an ARM binary, not a web page), on the same Rust core as the other Link apps (`apps/core`), that brings up the computer (KyberNow Station) full screen, with the Magic Remote, keyboard and mouse, or gamepad. v2 identity ([guidelines](../../brand/v2/README.en.md)); mockups: [Claude Design](https://claude.ai/artifact/2TNZZrYYZpHvdQqLXzSLGv) (home, session menu with the Magic Remote, Picture, Network and Sound settings, controllers and rumble). Installing on the TV: [INSTALLATION.en.md](INSTALLATION.en.md) (developer mode).

**Status (23/10/2026)**: installed on the G2 (webOS 26) in developer mode; end-to-end session at 1440p: picture, sound (stereo), controller, Magic Remote. The binary builds for webOS (armv7, glibc 2.12), so does the `.ipk` package, and the interface is checked through its screenshots (on the computer, and by the ARM binary under emulation). Test TV: LG OLED G2 83 inches (2022). The decision is in [PROJECT.en.md](../../PROJECT.en.md) ("LG TVs (webOS)").

![Home](screenshots/1-accueil.png)

## TVs

- **webOS 5 or later** (LG TVs from 2020 on): the NDL DirectMedia decoder in API 2, the one the app uses. Targeted first: webOS 22 and later (OLED C2, G2 and after), including the webOS Re:New updates (webOS 24 and 25 on 2022 models).
- **Installation**: in developer mode (free LG account, no root), or through Homebrew Channel. The LG Content Store is not targeted: it does not publish native apps from non-partners.
- **Test hardware**: LG OLED G2 83 inches; DualSense, Xbox, Shield and Switch Pro controllers.

## What the app does

- **Home** (`src/ui/home.rs`): the chosen computer full frame (Admin's preview, dimmed when offline), its state (Admin polled every 8 s), system, graphics card and address, the virtual display's mode; "Connect", the Game, Desktop, Max profiles and "Computer settings"; "Settings" (global settings) at the top right, even with no computer added; everything is reachable with the arrows, no special key needed (third-party remote, controller) — the "…" key (yellow or blue, controller Y) stays a shortcut to the computer's settings; a "Ready to play" column (TV and panel, network and bitrate cap, controllers and their rumble, last session); the other computers, those found on the network, and Add. Offline with a known MAC address: "Wake the computer" (Wake-on-LAN).
- **Adding** (`src/ui/pair.rs`): a six-digit code shown to each Station found on the network (mDNS `_kybernow._tcp`, TXT `name` and `fp`) and accepted in Admin › Link devices (a Kyber account of the TV's own); or Admin's address and port on the keypad (or a keyboard), then the first four groups of the fingerprint to compare with Admin's, then the code.
- **Session** (`src/ui/session.rs`, `src/device/`):
  - "Entering…" with the current step and Cancel, the seam lighting up.
  - **Picture** on the TV's video plane; the app's window is transparent on top, and only shows the menu, the capsules and the computer's pointer.
  - **Menu** with the Magic Remote's Back button (configurable), a long press on Back, the red button, or L1 + R1 + Start + Select: stream health at the top (latency, frames per second, bitrate, mode, codec, losses), then Resume, the other profiles, Magic Remote as mouse, Controller as mouse, Keys (Ctrl + Alt + Del, Alt + Tab, Windows key…), Stats, Controllers, Settings and Disconnect. While the menu is open, controllers drive the app and the computer releases everything. A reminder of the TV's game mode (see below), which can be turned off.
  - **States** as capsules: reconnecting, picture restarted, computer mode changed, lighter picture, controller connected.
  - **End**: back home, the session's median latency and duration kept for the home screen; an error shows with "Back home".
- **Controllers** (`src/device/pads.rs`, `src/ui/pads.rs`): up to four players in order of arrival, announced to the computer at each picture start; buttons, sticks, triggers and D-pad; rumble on both motors as the game asks (and on the triggers when the controller has them); controller as mouse (left stick pointer, RT faster, LT finer, right stick wheel, A / B / X left, right, middle click). The Magic Remote, when webOS presents it as a gamepad, is left out. Test screen: buttons, sticks and triggers live, test rumble per motor and on the triggers.
- **Magic Remote, keyboard, mouse**: outside the game, the pointer selects what it hovers and OK confirms, the arrows go to the nearest item in their direction; in the game, arrows and OK go to the computer (Back as Esc if the setting says so), or the Magic Remote becomes the computer's mouse (OK left click, yellow right click, wheel); with the Magic Remote or a controller as the mouse, the computer's pointer is always drawn, since Windows with no mouse plugged in hides it on and off (as on Android); a plugged-in keyboard sends its keys (USB codes converted to Windows scancodes with the `keycode` crate's table, kynput's).
- **Settings** (`src/ui/settings.rs`), two screens. **Global**: this TV (language, model, panel, decoder, network, version); controllers (connected ones, test). **Per computer**: profile, resolution (like this TV, 2160p, 1440p, 1080p, 720p), frame rate (the panel's, 30, 60, 120), codec (Auto: HEVC; H.264, AV1), bitrate, game mode reminder, VRR (unavailable, explained); sound (Auto, stereo, 5.1, on the computer; state of the TV's 5.1 output); controllers seen as Xbox 360, combination, Magic Remote Back, controller test; network (link, cap, reliable transport, reconnection, Admin address); account and fingerprint, remove the computer.
- **Languages**: French and English (`src/i18n.rs`, pairs written in place), following the TV's language; Auto / Français / English setting.

## Picture, frame rate and game mode

- **Session mode**: the panel's (`SDL_webOSGetPanelResolution`, `SDL_webOSGetRefreshRate`), that is 3840 × 2160 at 120 Hz on a G2; or a resolution or frame rate chosen in the settings. The computer's virtual display is created at this mode.
- **30, 60 or 120 frames per second**: the panel does not change its refresh rate for an app (webOS offers no way to); at 30 or 60 fps each frame is shown 4 or 2 times, evenly. 144 Hz: the 2025 panels that have it (not the G2).
- **Codec**: HEVC by default (4K at 120 fps on the decoder of 2022 OLEDs); AV1 only up to 60 fps (beyond, LG TVs show it at 60: moonlight-tv #529, punktfunk); H.264 as an option, not advised above 60 fps.
- **Bitrate**: the profile's bitrate (scaled to pixels per second, like the other apps), capped at 80 Mbit/s on Ethernet (LG's port stops at 100 Mbit/s, about 93 usable) and at 95 on Wi-Fi (the decoder limit moonlight-tv uses on 2022 and later chips). The link is read from `/sys/class/net`.
- **TV game mode**: no API lets an app switch the TV to Game Optimizer; the menu reminds you (Magic Remote Settings button › Picture › Mode › Game Optimizer), and the reminder can be turned off.
- **VRR**: LG keeps it for HDMI inputs, for every internal app, native or web. The computer sends its frames at the panel's pace.
- **HDR**: the decoder can show it (`NDL_DirectVideoSetHDRInfo`, HEVC Main10); impossible with Kyber 0.29 (8-bit capture), as on the other apps.

## Decoding (NDL DirectMedia)

`src/device/ndl.rs`, after moonlight-tv (the ss4s library) and punktfunk, which proved it on TVs:

- `libNDL_directmedia.so.1` opened with `dlopen`, never linked (a direct dependency keeps the app from starting where it is missing).
- `NDL_DirectMediaInit` at the first load, not at app launch.
- Every load carries a **stereo Opus audio plane, primed with empty frames** (`0xec 0xff 0xfe`, 5 ms each): without it, NDL does not pace frames on their timestamps, and the picture stutters above 1080p on a 120 Hz panel. The real sound does not go there (see Sound).
- **The load is only confirmed (`LOADCOMPLETED`) once that plane is fed**: empty frames go out in bursts until the confirmation (in the background past 500 ms), and no frame is given before it (2 s at most). With a single frame, the G2 on webOS 26 stayed on a black picture, with no error; confirmed in 141 ms since (punktfunk, `ndl/v2/load.rs`).
- Timestamps in milliseconds since the load, never going backwards.
- **Never flush** (`NDL_DirectVideoFlushRenderBuffer`) during a session: it cuts the audio plane for the rest of the session. After a loss, no frame is given before the next keyframe, requested again by the core.
- The parameter set (SPS/PPS, VPS, AV1 sequence header) goes before the keyframe that (re)starts the decoder; a new parameter set, codec or size reloads NDL.
- No NDL function is thread-safe: a single lock.
- NDL does not say when a frame is shown: the display time (latency) is estimated from the frames waiting (`NDL_DirectVideoGetRenderBufferLength`) and the panel's refresh rate.

## Sound

Opus decoded by the core's engine (`apps/core/src/audio`: adaptive buffer, loss concealment), pulled by SDL's callback (the TV's PulseAudio, 48 kHz, 512 frames per callback) in **stereo**: a game in 5.1 or 7.1 is mixed down by the engine. The app asks the computer for stereo (Auto and Stereo). Sound can stay on the computer.

**5.1** (Sound › 5.1 setting): the app asks the computer for 6 channels, loads NDL with a 6-channel Opus audio plane (primed with empty 5.1 frames) and hands it the packets **untouched**, stamped on the load's clock, never backwards: the TV decodes and outputs to the receiver over eARC, or mixes for its speakers. NDL decodes a single layout (4 streams, 2 coupled: (L, R), (rear L, rear R), C, LFE); libopus's surround encoder layout, which Kyber sends (order `[0, 4, 1, 2, 3, 5]`), carries the same streams, hence no re-encoding (`media.rs`, `NDL_51_MAPPINGS`). Another stream, or a packet NDL refuses, falls back to SDL in stereo.

- The TV only outputs multichannel when its **Digital Sound Output is set to Pass-Through**: `NDL_DirectAudioSupportMultiChannel` says so (3 = ready; 2 = multichannel receiver connected but not Pass-Through), shown in Settings › Sound and read again every 2 s. Tried on 2026-10-23 (G2, webOS 26, receiver over eARC): 6 channels received from the computer, packets accepted by NDL, sound heard, but the receiver in stereo while the output is not Pass-Through.
- **No raw stream to the receiver**: the computer sends Opus, which no receiver decodes, and NDL's API 2 only takes PCM, MP3 and Opus for sound (AC-3 only exists in API 1, webOS 4's).
- **No 7.1**: NDL decodes 5.1 at most.

## Rumble

The game's rumble reaches the core (`InputPlatform::rumble`) and goes to the player's controller through `SDL_GameControllerRumble` (5 s at most without a duration, (0, 0) stops). On LG TVs, Bluetooth controllers rumble when webOS exposes their motors (moonlight-tv: the DualSense over Bluetooth does not always rumble there); the test screen says "2 motors" or "no rumble" for each controller, and the test rumble checks it.

## Architecture

```
apps/webos/
├── src/
│   ├── main.rs          launch (--preview: screenshots on the computer)
│   ├── app.rs           interface state, spatial navigation, actions (no SDL nor core)
│   ├── model.rs         computers, settings, the TV's identity ($HOME/conf/kybernow.json)
│   ├── i18n.rs          French and English
│   ├── ui/              drawing (tiny-skia, fontdue): home, adding, settings, controllers, session
│   └── device/          the TV: SDL (window, events), NDL (picture), sound, session
│                        (the core's Platform), controllers, background jobs (Admin, mDNS, code), log
├── kyber-patches/       kynput patch for webOS (client only)
├── assets/              brand fonts (static instances), logo
├── webos/               appinfo.json, icons, splash screen
└── scripts/             build.sh, package.sh and ipk.py (package), preview.sh (screenshots),
                         fonts.py and images.py (fonts and images taken from the brand)
```

- **Core**: `kybernow-core` as is; `src/device/session.rs` is the equivalent of Android's JNI bridge (the core's `Platform`: picture, sound, input chain, states, stats, pointer, rumble).
- **SDL2**: webosbrew's webOS version (`release-2.30.12-webos.6`, moonlight-tv's), built by `build.sh` and shipped in the package (`lib/`, found through the `$ORIGIN/../lib` RPATH, as `DT_RPATH` to come before the TV's older SDL). Settings: Back and long Back to the app (`SDL_WEBOS_ACCESS_POLICY_KEYS_BACK`, `_EXIT`), controllers left as controllers (`SDL_WEBOS_CLOUDGAME_ACTIVE`). SDL's functions are declared by hand (`src/device/sdl.rs`).
- **Interface**: drawn in memory (tiny-skia for shapes and images, fontdue for text) at 1920 × 1080, webOS's graphics plane, then sent to the screen through an SDL texture. During the session, the window is cleared to transparent; the computer's pointer is a small separate texture, recomposited without redrawing the interface.
- **Fonts**: Archivo and Martian Mono from `brand/v2/fonts`, as static instances cut down to Latin (`scripts/fonts.py`), since fontdue does not read variable axes.
- **VLC**: kyvlcplayer references libvlc, never called: `build.rs` compiles an empty function for each `libvlc_*` function of vlc-rs, as on Android and iOS.
- **glibc**: the TV has glibc 2.12; `src/glibc_compat.c` (taken from punktfunk) provides `getauxval`, `gettid` and `sendmmsg`, which Rust's standard library asks for.
- **kynput**: on Linux, kynput compiles the server's input injection (libinput, libevdev, X11), missing on the TV; patch `kyber-patches/0001` (cfg `kybernow_webos`) makes it behave as on Android (client only).

## Building

```sh
# Once: Rust target, cargo-c, cmake, Python 3 (and pip install fonttools pillow cairosvg for fonts and images).
rustup target add armv7-unknown-linux-gnueabi
cargo install cargo-c --version 0.10.15 --locked

# The binary and the package (on Linux x86_64: the webOS toolchain is downloaded into third_party/webos-toolchain):
apps/webos/scripts/build.sh                  # → apps/webos/dist/fr.leslignesclaires.kybernow_<version>_arm.ipk
apps/webos/scripts/build.sh --check          # cargo check only

# The screens on the computer, without a TV:
apps/webos/scripts/preview.sh                # → apps/target/webos-preview/*.png
```

`scripts/build.sh` applies the patches of `apps/macos/kyber-patches` then those of `apps/webos/kyber-patches`, builds into `third_party/kyber-desktop/rootfs-armv7-unknown-linux-gnueabi` libkynput (without SDL, `cargo cinstall`) and libopus 1.6.1 (position-independent: the binary is PIE), webOS's SDL2 into `third_party/sdl-webos-dist`, writes `libvlc.pc`, then builds `kybernow-webos` and packages it (`scripts/package.sh`: binary without debug info, SDL2, images, licences). Kyber 0.29's sources go into `third_party/kyber-desktop` (see [docs/build-notes.en.md](../../docs/build-notes.en.md)).

- **Toolchain**: webosbrew's (`openlgtv/buildroot-nc4`, gcc 16, glibc 2.12, armv7 softfp), in `third_party/webos-toolchain` or `WEBOS_TOOLCHAIN`. On an Apple Silicon Mac (tried on 2026-10-23): `arm-webos-linux-gnueabi_sdk-buildroot_darwin-arm64.tar.bz2` from [webosbrew/native-toolchain](https://github.com/webosbrew/native-toolchain) (gcc 12, glibc 2.12), unpacked into `third_party/webos-toolchain-mac`, `./relocate-sdk.sh` once, then `WEBOS_TOOLCHAIN=…/arm-webos-linux-gnueabi_sdk-buildroot apps/webos/scripts/build.sh` (with `/opt/homebrew/opt/rustup/bin` on `PATH` if rustup comes from Homebrew). `scripts/cc.sh` gives it its sysroot and links libstdc++ statically.
- **Rust options**: `-C target-feature=+neon,+vfp3,-soft-float -C target-cpu=cortex-a73` (otherwise the "gnueabi" target emulates floating point: ten times slower, according to punktfunk), `--cfg kybernow_webos`, `--cfg getrandom_backend="linux_raw"`.
- **Package**: `scripts/ipk.py` writes the `.ipk` as `ares-package` does (ar archive: `debian-binary`, `control.tar.gz`, `data.tar.gz` with `usr/palm/applications/<id>/` and `usr/palm/packages/<id>/packageinfo.json`), without LG's tools.
- **Emulation**: `qemu-arm` does not run a binary linked against the TV's glibc 2.12 (its loader crashes, even for a minimal C program); the preview is checked with a static binary (`--no-default-features`, `+crt-static`, the glibc shim as `-C link-arg`, `qemu-arm -cpu max`).

## Log

`/media/developer/temp/kybernow/kybernow.log` on the TV (the previous one as `.1`), next to `kybernow.json` (computers and settings). In developer mode, `HOME` is `/media/developer`, closed to the app's user, and its `/tmp` is emptied on reboot and invisible over ssh: `temp` is open to all, survives reboots and reinstalls, and can be read over ssh. The app takes the first folder it can write to among `$HOME/conf`, this one and `/tmp/kybernow` (`Store::dir`), and standard error: `image :` (NDL), `son :`, `manettes :`, `session :` lines, with the core's messages (logs stay in French). See [INSTALLATION.en.md](INSTALLATION.en.md) to read it.

## To do

- On the G2: an end-to-end session (NDL, sound, controllers, rumble), latency at 4K 120 Hz, 1440p and 1080p, at 30, 60 and 120 fps.
- Magic Remote as mouse: check that the TV still sends motion while its pointer is hidden.
- 5.1 sound up to the receiver: check with Digital Sound Output set to Pass-Through (the receiver must show multichannel), then audio-video sync.
- DualSense: rumble and adaptive triggers through webOS's Bluetooth service (as punktfunk does), when SDL does not provide them.
- Homebrew Channel: a repository for the app (updates).
