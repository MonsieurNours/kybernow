# KyberNow for Windows

English · [Français](README.md)

KyberNow's streaming client for Windows 10 (22H2) and 11: your computer (Kyber 0.29 server + KyberNow console) in full screen on a PC, with image, sound, keyboard, mouse and gamepads. Target: a laptop or low-powered computer connecting to a more powerful one. Decisions and milestones: the “Windows client” row of [PROJECT.en.md](../../PROJECT.en.md); mock-ups: https://claude.ai/artifact/JkUTEMQmxKjcYksaXRHgBf.

**Milestone 2 (the app)**: the app opens on its home screen (Tauri 2, Svelte interface in WebView2): your computers, adding them, their settings, the passage into the session and the session bar. The session itself (milestone 1) stays a Win32 window in Direct3D 11; the command line remains for testing.

It is the first client written on the **Rust client core** ([`apps/core`](../core), crate `kybernow-core`), which Android and Linux can use later; the Apple apps stay in Swift.

## What the app does

### Outside a session (Svelte interface, `ui/`)

- **Home**: the chosen computer in full frame, its image (the last image of your last session, kept on this PC in `%LOCALAPPDATA%\KyberNow\images` and optional in its settings, “Keep the last image”; otherwise a preview of its display requested from the console, at most once a minute; arriving with a 45° sweep) or else its **signature** (two glows whose hue comes from its console's fingerprint), desaturated when offline; its name, system, graphics card, address; the virtual display waiting for it (“1920 × 1200 · 60 Hz, like this display”); **Connect** and the profile (Game, Desktop, Max); on the right the latency of the last session (median, 95th percentile, duration) on the one-frame ruler; at the bottom your computers, those announced on the network (they light up as they appear) and the add card. Online or not: the console is polled every 8 s.
- **Adding a computer**: find it **on the network** (the console's Bonjour announcement `_kybernow._tcp`, fingerprint included), **with the console's link** (`kybernow://…`, pasted) or **by address** (the console's fingerprint is then shown, the first four groups to compare with the console's); then get access **with a six-digit code** to accept in the console (Access › Devices: the device gets its own account, nothing to type) or **with an account** and its password, checked with the console. No camera QR code.
- **Settings per computer** (saved on each change): profiles, a setting that departs from them (format, colours, bitrate, transport) makes them “Custom”; the computer's display (like this display, 4K at most, or **custom**: resolution and refresh rate of your choice, up to 7680 × 4320, with common values), format (those the card decodes), 4:4:4, synchronised or immediate display, bitrate (real bitrate shown); Auto, Desktop or Game mouse, gamepad seen by the computer (Xbox 360 or faithful), clipboard, sound (Auto, stereo, 5.1, 7.1, none), transport, virtual display; **This PC** card (graphics card, what it decodes, display, sound) and **on battery** (lighten: 30 Hz at most). Rename, remove.
- **Passage**: the home screen split along the 45° seam, a neon tube lit during preparation (capsule: virtual display, connection, first frames, Cancel), then, at the first frame shown (not as soon as connected), the session window shows in full screen and the home screen hides, always; with no frame after 12 s, the passage stays on “First frames” with Cancel and says the computer is not sending any image (its log: console, System › Logs) (left in front, it showed the passage opened onto black). If Windows refuses the foreground to the session (launch from the taskbar, a notification, a link or a remote tool), it briefly attaches to the foreground thread to take it, otherwise it flashes in the taskbar; a session to this PC itself is also refused by address (one of its network adapters' addresses); at the end it comes back, the halves close and the last session is recorded.
- **Session bar**: a transparent window above the session, shown when the pointer touches the top of the screen, hidden when it leaves (the keyboard returns to the session): lit symbol, latency and its curve, resolution, frames per second, decoder; **profile** (Game, Desktop, Max menu: saved for this computer and applied at once, new bitrate right away or stream restarted if the format, 4:4:4 or the transport change, as on Mac); **Keys menu** (Ctrl + Alt + Del, ⊞ key alone, Alt + Tab, Print Screen); mouse, full screen, Quit. An open menu makes the bar's window grow downwards while it is shown.
- **Passwords** in the Windows Credential Manager (`KyberNow:<id>`), never in a file.
- **Not onto itself**: connecting to this PC is refused (the virtual display would switch off the displays the app is shown on).
- **Device account not yet known to Kyber**: the console creates the account of a device accepted by code, but Kyber only reads it when it restarts, which the console postpones during a session or an encode. The app says so (“restart the server in the console”) instead of a bare refusal.

### Windows integration (milestone 3)

- **A single app**: a second launch (shortcut, link, taskbar, notification) hands its request to the app already open, which comes to the front, then quits (`Local\KyberNow.App` mutex, per-user local named pipe).
- **`kybernow://` link** (the console's link and QR code): registered for the user (HKCU) by the installer and at each launch; it opens the add flow with the computer and its fingerprint already checked.
- **Taskbar**: on a right-click of the icon (and in the Start menu for the pinned app), your computers, one click starts the session (`kybernow.exe --connect <id>`), and “Add a computer”. The list follows additions, removals and renames.
- **Windows notifications**, only when the app is not in front: session ended (by another device, the console or an error), with “Take back control” which starts the session again; new version ready. Notification identity registered in HKCU (`AppUserModelId\LesLignesClaires.KyberNow`, icon `kybernow.png`).
- **App settings** (gear in the title bar): version, language (Auto, Français, English), offered versions (stable, or betas too), checking for and installing an update, its release notes. A dot on the gear and a banner in the title bar say a version is waiting.

### Installation and updates

- **Installer** `KyberNow-Client-Setup.exe` (`setup/`), in the style of the computer's one (`KyberNow-Server-Setup.exe`): borderless window in Rust, WebView2 and Svelte, side column with the “now” symbol whose seam lights up with the steps (Welcome, Installation, Ready). One file for **x64 and ARM64**: it carries both versions of the app and installs the processor's one (it runs emulated on an ARM PC and installs the ARM64 version there; an emulated x64 app notes it in its log). For the user only, without administrator rights, into `%LOCALAPPDATA%\Programs\KyberNow`.
  - **Welcome**: what it sees of the PC without changing anything (Windows, processor, graphics card, display, network, WebView2, version already installed, app open), “Desktop shortcut” option. An installed version turns the installation into an update (“Update”).
  - **Installation**: closes the app if it is open; lays the files down next to it (`KyberNow.new`) then swaps them with the old version in one go (a failed update leaves it intact; if the old folder is locked, copies over it); installs WebView2 if missing (Microsoft's official installer, for the user); `kybernow://` link, notifications and shortcuts set up by the app itself (`kybernow.exe --setup-shell [--desktop]`); “Installed apps” (HKCU).
  - **Ready**: “Open KyberNow”, and the session shortcuts.
  - `/S`: silent; `/S /UPDATE`: update started by the app (waits for it to close, relaunches it with `--updated`, and reopens it with a message if the installation fails). The installer never keeps the app's folder as its working directory (Windows would refuse to rename it); if the folder stays locked, files are swapped one by one, with rollback. During installation it holds `Local\KyberNow.Setup`: an app reopened meanwhile waits for the end (2 minutes at most). Windows programs are called by their full path (`System32`), not from the installer's folder. The footer is also read after a future Authenticode signature (the PE header's certificate table). ARM64: Windows 11 (Windows 10 on ARM does not emulate the launcher's x64). Logs: `%LOCALAPPDATA%\KyberNow\setup.log` (launcher) and `setup-ui.log` (interface).
  - No Authenticode signature during the beta: Windows (SmartScreen) warns at first launch.
- **Uninstalling** (“Installed apps”, `uninstall.exe --uninstall`, the launcher without its contents): closes the app, clears the taskbar list, offers to also erase computers, settings and passwords (`kybernow.exe --forget-all`), removes shortcuts and registrations, then the folder. The installer's internal options write to `kybernow-setup.log`, not to the app's log.
- **Updates**, like the console (`server/admin/src/update`): the app checks at launch then every 6 hours for the latest version published on GitHub, on its channel; nothing is offered without the version's signed manifest (`kybernow-release.json`, minisign, same key as the console), which must announce the same version and list the installer. The downloaded installer (`KyberNow-Client-Setup.exe`, the same for both architectures) must have exactly the manifest's size and SHA-256 fingerprint. “Install and relaunch” runs it silently (`/S /UPDATE`) and closes the app; it waits for the app to close, replaces its files (not the data) and relaunches it (`--updated`, “KyberNow X is installed”). Not during a session.

### Session

- **Session** (core, like `SessionController.swift`): preparation by the console (`POST /api/stream/prepare`: virtual display at this PC's display resolution and refresh rate, virtual sound output with as many channels as this PC's output), connection to Kyber with the same account, Kyber's certificate required as the console gives it (`kyber_fingerprint`), display choice (waits at most 6 s for Kyber to announce only the virtual display), heartbeat every 5 s carrying the latency (the console can ask there for the session to end), the computer's foreground polled every second (Auto mouse), clean end (`POST /api/stream/end`, session latency). Recovery: Kyber's automatic reconnection, display requested again from the console if Kyber restarted, image restarted (at most 3 times) after an image pipeline error, keyframe requested as soon as a reference frame is missing (every 300 ms until it arrives). If the packet that announces the stream's format is lost (fast transport, with losses), the decoder for the format requested for the session opens on its own at the first frame: before, every frame was silently dropped and the screen stayed black. The log records the announced format, the parameters, the first data and the first keyframe. As on Mac, a keyframe is requested only after a loss, never before the first data (the encoder starts with one), and a stream with no image 4 s after it starts is restarted (which also restarts Kyber's video service on the computer), at most three times before ending the session with the reason.
- **Console**: Basic account, `x-kyber-admin` header, certificate pinned by its fingerprint (`sha256:aa:bb:…`, SHA-256 of the certificate). Without a given fingerprint (`--fingerprint`), the first one seen is shown and remembered in `%APPDATA%\KyberNow\consoles.txt`; a different certificate afterwards stops the connection.
- **Profiles** Game, Desktop and Max, brought down to what the graphics card decodes, and **bitrate** following pixels per second: same rules and values as the Mac app (tested in `apps/core`). A format, colour or bitrate option makes the profile “Custom”.
- **Image**: FFmpeg (Kyber's, statically linked) decodes on the graphics card (**D3D11VA**, never leaving the card; the app gives FFmpeg the Direct3D device and its video interfaces, which FFmpeg 8.1 does not look up by itself: without them it crashed when creating the decoder; without video interfaces, as in a virtual machine, CPU decoding); at launch, the app really decodes a small frame of each format (H.264, HEVC, HEVC 4:4:4, AV1, in `src/probe/`) to learn what the card can do. If the card does not decode a format, or decodes it into a texture format we cannot show, the app decodes on the **CPU** (slice threads only: frame threads add one frame of delay each). Direct3D 11 rendering: a shader converts YUV (NV12, 4:4:4 AYUV or three planes) to RGB according to the stream's colour space and range, at window size keeping proportions; “flip” swap chain, at most one frame waiting; every packet is decoded but only the newest frame is shown (if others are waiting, this one is skipped: delay does not build up behind vertical sync). Vertical sync by default; `--no-vsync` presents each frame as soon as it is decoded (tearing allowed when Windows allows it, useful with a G-SYNC or FreeSync display).
- **Sound**: the core's sound engine (`kybernow_core::audio::AudioEngine`, the same as on Mac and Apple TV: libopus, loss concealment, an adaptive buffer from 4 to 60 ms starting at 8 ms, clock drift caught up smoothly, see [apps/core/README.en.md](../core/README.en.md)); WASAPI in event-driven shared mode pulls its samples from its own thread (`AudioEngine::fill`), with the stream's channels put back in Windows order (5.1, 7.1), and gives it the output latency (its buffer plus what Windows reports); Windows converts to the output's format. The output is reopened when the stream changes its channel count or the output fails. Hovering the bar's latency shows the sound delay to the output and the gaps filled in the last second.
- **Inputs**: through the core's input module (`kybernow_core::input::InputPipeline`, the same as on Mac and Apple TV), one pipeline per image start, created for the displayed screen and stopped with it (FlushKeys, losses to the log); the window captures Win32 and Raw Input events and hands them over. Clock: `Instant` (QueryPerformanceCounter).
- **Keyboard**: Windows key codes as they are (set 1, `0xE000` for extended keys, like Kyber's client), without repeats (the computer repeats by itself); Alt and F10 do not open a local menu. **Windows shortcuts**: in full screen and in the foreground, a low-level keyboard hook sends the computer what Windows would keep for this PC (⊞ key and its combinations, Alt+Tab, Alt+Esc, Ctrl+Esc, Menu key); in a window, they stay on this PC. Ctrl+Alt+Del always stays with Windows: Keys menu of the bar.
- **Mouse**: absolute position brought to the computer's pixels, buttons (including both side buttons), wheel. Games mouse (relative, raw movements through Raw Input, pointer hidden and held in the window): automatic when the console sees a full-screen game and the computer hides its pointer, or forced with Ctrl+Alt+Shift+M. The core's Auto, Desktop, Game rules (`MouseModeRules`, as on Mac): a pointer shown again for less than 0.6 s (menus, flicker) does not leave the games mouse, checked again 0.7 s later.
- **Pointer**: the computer's pointer (shape and hotspot, received and deduplicated by the core's `PointerLayer`) becomes the Windows pointer over the image, **scaled with the image** (this PC's pixels per computer pixel, between 0.5 and 3: it keeps its size relative to the image); a fully transparent shape keeps the previous one; the shape is only rebuilt when it or the scale changes (position packets arrive up to 120 times a second). **In game mode**, when the computer shows its pointer (a game's menus), this PC's pointer draws it where the computer says it is; hidden otherwise.
- **Display requested again**: when the heartbeat says the virtual display is gone (console restarted…), it is requested again and the image restarted, at most once every 30 s (as on Mac). The profile reported by the heartbeat follows changes made from the bar.
- **Gamepads and clipboard**: through kynput's client configuration on Windows (SDL for gamepads, seen by the computer as Xbox 360 pads; text and HTML clipboard both ways). The core's clipboard bridge (`ClipboardBridge`, text only) is not plugged on Windows: kynput already installs its own there, the only possible receiver of its pipeline's clipboard packets.
- **Refused packets**: kynput's send queue holds 64 packets and refuses without notice when full; refused keys and buttons are retried every 3 ms, in order, for up to 200 ms (the core's `PacketQueue`, on its `kybernow-entrees` thread), positions, moves and wheel are sent once. The total of losses goes to the log.
- **Nothing stays pressed**: when the window loses the keyboard, the app releases on the computer the keys and buttons it holds there (`release_all`); the release of a key or button whose press was never sent (an app shortcut) is not sent; at the end of the image and of the session, everything is released.
- **Crashes**: written to the log (panics with the stack, native exceptions with module and offset); the previous launch's log is kept (`kybernow.prev.log`). The graphics card probe runs in a separate process (`kybernow --probe-json`, log `kybernow-probe.log`): if it crashes, the app decodes on the CPU. Each version's unstripped executables give the faulting line (`llvm-addr2line -e kybernow.exe 0x140000000+offset`).
- **Measurements**: the window title gives the end-to-end latency (capture on the computer → presentation here, on Kyber's adjusted clock), the resolution, the decoder and frames per second; the session median and 95th percentile go to the log and to the console.

| Shortcut | Effect |
|---|---|
| Ctrl+Alt+Shift+D | Disconnect (also: closing the window) |
| Ctrl+Alt+Shift+F | Full screen or window |
| Ctrl+Alt+Shift+M | Games or desktop mouse |

## Languages

The app and its installer come in French and English ([docs/i18n.en.md](../../docs/i18n.en.md)).

- **Detection**: Windows display language (`GetUserPreferredUILanguages`); French if it starts with `fr`, English otherwise.
- **Language setting** (app settings): Auto (default, Windows' language), Français, English; saved in `%APPDATA%\KyberNow\app.json` (`language`) and applied at once to both windows (home and session bar), to Rust-side messages, notifications, the taskbar list and the session window title. The installer has no setting: it follows Windows.
- **Console**: every request carries `Accept-Language: fr` or `en` (the app's language), so the console answers in that language.
- **Release notes**: only the part in the app's language is shown (the release text is split at the `<!-- en -->` line: French before, English after; the whole text without it).
- **Catalogues** (same keys, French is the source, a key missing in English falls back to French): interface `ui/src/lib/i18n/{fr,en}.json` (`t()` function), the app's Rust side `src/i18n/{fr,en}.json` (`tr()`, `src/i18n.rs`), core `apps/core/src/i18n/{fr,en}.json` (`apps/core/src/i18n.rs`), installer `setup/src/i18n/` and `setup/ui/src/lib/i18n/`. `npm run check` compares the keys of each pair. Logs and the command line (testing, troubleshooting) stay in French.
- **Imitation** in a browser: the browser's language, or `?lang=en` / `?lang=fr` in the address.

## Usage

`kybernow.exe` without arguments opens the app. On the command line, a direct session (testing, troubleshooting):

```
kybernow <address>[:port] [--user <account>] [--profile jeu|bureau|max] [options]
```

- `<address>:port`: the computer and its console's port (Access, in the console); 9443 by default.
- Password: `--password`, the `KYBERNOW_PASSWORD` variable, otherwise asked.
- `--codec h264|hevc|av1`, `--444` / `--420`, `--mbps <bitrate>`, `--size 1920x1080@60` (otherwise this PC's main display), `--window`, `--no-vsync`, `--no-hw` (CPU decoding), `--no-audio` (sound left on the computer), `--physical` (the computer's displays as they are).
- `--probe`: shows what the graphics card decodes, the display, the sound output and the system, then quits. Needs no computer.

## Code layout

| Folder | Contents |
|---|---|
| `apps/Cargo.toml` | Rust workspace of the clients (`core`, `windows`); points Kyber's crates to `third_party/kyber-desktop/kysdk` |
| `apps/core/src/console.rs` | Console client (`info`, `prepare`, `heartbeat`, `foreground`, `end`), pinned certificate, `Accept-Language` |
| `apps/core/src/i18n.rs` | Language (detection, choice), text catalogues, release notes per language |
| `apps/core/src/session.rs` | A session's flow on a dedicated thread; what the platform provides: `Platform` |
| `apps/core/src/media.rs` | Image and sound packets delivered by Kyber (patch 0001), Opus header |
| `apps/core/src/profile.rs` | Profiles, format, bitrate following pixels per second |
| `apps/core/src/latency.rs` | End-to-end latency, Kyber's clock |
| `apps/windows/src/main.rs` | App or command line |
| `apps/windows/src/ui.rs` | Tauri app: windows, interface commands, computer polling, this PC |
| `apps/windows/src/store.rs` | Computers and settings (`computers.json`), passwords (Credential Manager) |
| `apps/windows/src/discovery.rs` | Computers announced on the network (mDNS) |
| `apps/windows/src/host.rs` | A session on its own thread: window, image, sound, inputs, news for the app |
| `apps/windows/src/plan.rs` | A session's settings (profile, adjustments, display, battery) |
| `apps/windows/src/cli.rs` | Command line |
| `apps/windows/ui/` | Svelte interface: home, add, settings, passage, bar; `src/lib/mock.ts` imitates the app in a browser (`npm run dev`, development only) |
| `apps/windows/src/window.rs` | Win32 window: keyboard and mouse capture into the input module, pointer drawing, shortcuts, full screen |
| `apps/windows/src/video.rs` | Image thread: packets → decoder → rendering, keyframes |
| `apps/windows/src/decoder.rs` | FFmpeg (D3D11VA or CPU), capability probe |
| `apps/windows/src/render.rs` | Direct3D 11: textures, YUV → RGB shader, swap chain |
| `apps/core/src/input/` | Shared input module: kynput pipeline, send queue, the computer's pointer, mouse mode |
| `apps/core/src/audio/` | Sound engine: libopus, adaptive buffer, channel order |
| `apps/windows/src/audio.rs` | WASAPI output pulling sound from the core's engine |
| `apps/windows/src/system.rs` | Log, display, device identity, known consoles |
| `apps/windows/src/i18n.rs` | Windows language, texts of the app's Rust side (`src/i18n/`) |

The core depends on no operating system: another platform provides its window and decoders through `Platform`, input capture (into `InputPipeline`) and the sound output (which pulls sound from `AudioEngine`).

## Building

Everything builds from the Mac:

```bash
apps/windows/scripts/build.sh                # x64 (x86_64-pc-windows-gnu, Homebrew MinGW, like the server)
apps/windows/scripts/build.sh --arch arm64   # ARM64, Snapdragon PCs (aarch64-pc-windows-gnullvm, llvm-mingw)
```

Then the installer, once both folders are built: `apps/windows/scripts/build-installer.sh` writes `apps/windows/dist/KyberNow-Client-Setup.exe` (`[launcher][x64.tar.zst][arm64.tar.zst][64-byte footer “KNCLNT01”]`, see `setup/src/payload.rs`). The `setup/` project has its own `Cargo.lock`, with the server installer's versions (`wry`, `tao`, `zstd`), and is built for x64 only. Its interface (`setup/ui`) can be viewed in a browser with a simulation: `npm --prefix apps/windows/setup/ui run dev`, with `?maj`, `?ouverte`, `?arm`, `?echec` in the address. `scripts/release.sh` builds everything and attaches the installer to each version.

The folder to copy to the PC is `apps/windows/dist/x64/` (or `arm64/`), also as an archive `dist/KyberNow-<version>-windows-<arch>.zip`: `kybernow.exe`, `kynput.dll` and `SDL2.dll` (gamepads), plus `libunwind.dll` on ARM64. The rest comes with Windows 10 and 11 (Universal C runtime, Direct3D 11, `d3dcompiler_47.dll`).

- **x64 prerequisites**: `third_party/` in place, and `server/kyber-patches/build.sh` run once (it produces `third_party/rootfs-win64`, with FFmpeg and libopus, and `third_party/rootfs-win64-kynput`, with `kynput.dll`).
- **ARM64 prerequisites**: llvm-mingw in `third_party/toolchains`, the Rust target `aarch64-pc-windows-gnullvm`, then `apps/windows/scripts/build-arm64-deps.sh` once: libopus, an FFmpeg reduced to decoding (H.264, HEVC, AV1, D3D11VA), SDL2 (kynput's verified sources) and `kynput.dll`, in `third_party/rootfs-winarm64`. Tool versions: [docs/build-notes.en.md](../../docs/build-notes.en.md).
- **Kyber patches**: the same as for the Apple apps (`apps/macos/kyber-patches`), applied by the script if they are not.
- **VLC**: kyclient always links it on Windows, but the app decodes by itself: the script replaces each function of `libvlc.dll` with an empty one (as on the Apple TV) and refuses an executable that would still depend on `libvlc.dll`.
- `--debug`: without optimisation.
- **Core tests** (on the Mac):

```bash
cd apps && PKG_CONFIG_PATH=$PWD/../third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib cargo test -p kybernow-core
```

## Testing

The interface can be worked on in a browser on the Mac, with an imitation of the app: `npm --prefix apps/windows/ui run dev` (or the `windows-ui` configuration in `.claude/launch.json`).

Copy `dist/x64` to the PC (or `dist/arm64` to a Snapdragon PC, or into Windows 11 ARM in a virtual machine on the Mac: it tells whether the app starts and connects, not the latency nor graphics-card decoding), then start `kybernow.exe` (the app), or in a terminal:

```
kybernow.exe --probe
kybernow.exe 192.168.1.100:9443 --user <account> --profile bureau
```

The interface needs Edge's WebView2 runtime, present on Windows 11 and on most Windows 10 machines (milestone 3's installer will install it when needed).

On first contact, the console's fingerprint is shown: compare it with the console's (Access). The log is in `%LOCALAPPDATA%\KyberNow\kybernow.log`.

## Data

| File | Contents |
|---|---|
| `%APPDATA%\KyberNow\computers.json` | The app's computers, their settings and last session |
| `%APPDATA%\KyberNow\app.json` | App settings: update channel (`channel`), language (`language`: `auto`, `fr`, `en`) |
| Credential Manager, `KyberNow:<id>` | Each computer's password |
| `%APPDATA%\KyberNow\device-id` | This device's identifier for the console (drawn once) |
| `%APPDATA%\KyberNow\consoles.txt` | Console fingerprints remembered by the command line (`host:port sha256:…`) |
| `%LOCALAPPDATA%\KyberNow\kybernow.log` | Log of the last session |

## Not yet

- **Milestone 3, remaining**: Authenticode signature (postponed, SignPath targeted).
- **Later**: session on several of this PC's displays.
