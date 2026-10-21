# KyberNow for Windows

English · [Français](README.md)

KyberNow's streaming client for Windows 10 (22H2) and 11: your computer (Kyber 0.29 server + KyberNow console) in full screen on a PC, with image, sound, keyboard, mouse and gamepads. Target: a laptop or low-powered computer connecting to a more powerful one. Decisions and milestones: the “Windows client” row of [PROJECT.en.md](../../PROJECT.en.md); mock-ups: https://claude.ai/artifact/JkUTEMQmxKjcYksaXRHgBf.

**Milestone 1 (first image)**: a session to one computer, started from the command line. The home screen, adding a computer, the settings and the session bar come with the Tauri app (Svelte in WebView2) of milestone 2.

It is the first client written on the **Rust client core** ([`apps/core`](../core), crate `kybernow-core`), which Android and Linux can use later; the Apple apps stay in Swift.

## What the app does

- **Session** (core, like `SessionController.swift`): preparation by the console (`POST /api/stream/prepare`: virtual display at this PC's display resolution and refresh rate, virtual sound output with as many channels as this PC's output), connection to Kyber with the same account, Kyber's certificate required as the console gives it (`kyber_fingerprint`), display choice (waits at most 6 s for Kyber to announce only the virtual display), heartbeat every 5 s carrying the latency (the console can ask there for the session to end), the computer's foreground polled every second (Auto mouse), clean end (`POST /api/stream/end`, session latency). Recovery: Kyber's automatic reconnection, display requested again from the console if Kyber restarted, image restarted (at most 3 times) after an image pipeline error, keyframe requested as soon as a reference frame is missing (every 300 ms until it arrives).
- **Console**: Basic account, `x-kyber-admin` header, certificate pinned by its fingerprint (`sha256:aa:bb:…`, SHA-256 of the certificate). Without a given fingerprint (`--fingerprint`), the first one seen is shown and remembered in `%APPDATA%\KyberNow\consoles.txt`; a different certificate afterwards stops the connection.
- **Profiles** Game, Desktop and Max, brought down to what the graphics card decodes, and **bitrate** following pixels per second: same rules and values as the Mac app (tested in `apps/core`). A format, colour or bitrate option makes the profile “Custom”.
- **Image**: FFmpeg (Kyber's, statically linked) decodes on the graphics card (**D3D11VA**, never leaving the card); at launch, the app really decodes a small frame of each format (H.264, HEVC, HEVC 4:4:4, AV1, in `src/probe/`) to learn what the card can do. If the card does not decode a format, or decodes it into a texture format we cannot show, the app decodes on the **CPU** (slice threads only: frame threads add one frame of delay each). Direct3D 11 rendering: a shader converts YUV (NV12, 4:4:4 AYUV or three planes) to RGB according to the stream's colour space and range, at window size keeping proportions; “flip” swap chain, at most one frame waiting. Vertical sync by default; `--no-vsync` presents each frame as soon as it is decoded (tearing allowed when Windows allows it, useful with a G-SYNC or FreeSync display).
- **Sound**: libopus (loss concealment included), WASAPI in event-driven shared mode; Windows converts to the output's format. A 30 ms reserve; more than 100 ms behind, the oldest is dropped. Channels put back in Windows order in 5.1 and 7.1.
- **Keyboard**: Windows key codes as they are (set 1, `0xE000` for extended keys, like Kyber's client), without repeats (the computer repeats by itself); Alt and F10 do not open a local menu. Windows shortcuts (Windows key, Alt+Tab) stay on this PC for now (milestone 4).
- **Mouse**: absolute position brought to the computer's pixels, buttons (including both side buttons), wheel. Games mouse (relative, raw movements through Raw Input, pointer hidden and held in the window): automatic when the console sees a full-screen game and the computer hides its pointer, or forced with Ctrl+Alt+Shift+M.
- **Pointer**: the computer's pointer (shape and hotspot) becomes the Windows pointer over the image; a fully transparent shape keeps the previous one.
- **Gamepads and clipboard**: through kynput's client configuration on Windows (SDL for gamepads, seen by the computer as Xbox 360 pads; text clipboard both ways).
- **Nothing stays pressed**: when the window loses the keyboard, the app releases on the computer the keys and buttons it holds there; at the end of the session, everything is released.
- **Measurements**: the window title gives the end-to-end latency (capture on the computer → presentation here, on Kyber's adjusted clock), the resolution, the decoder and frames per second; the session median and 95th percentile go to the log and to the console.

| Shortcut | Effect |
|---|---|
| Ctrl+Alt+Shift+D | Disconnect (also: closing the window) |
| Ctrl+Alt+Shift+F | Full screen or window |
| Ctrl+Alt+Shift+M | Games or desktop mouse |

## Usage

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
| `apps/core/src/console.rs` | Console client (`info`, `prepare`, `heartbeat`, `foreground`, `end`), pinned certificate |
| `apps/core/src/session.rs` | A session's flow on a dedicated thread; what the platform provides: `Platform` |
| `apps/core/src/media.rs` | Image and sound packets delivered by Kyber (patch 0001), Opus header |
| `apps/core/src/profile.rs` | Profiles, format, bitrate following pixels per second |
| `apps/core/src/latency.rs` | End-to-end latency, Kyber's clock |
| `apps/windows/src/main.rs` | Command line, assembly |
| `apps/windows/src/window.rs` | Win32 window, keyboard, mouse, pointer, shortcuts, full screen |
| `apps/windows/src/video.rs` | Image thread: packets → decoder → rendering, keyframes |
| `apps/windows/src/decoder.rs` | FFmpeg (D3D11VA or CPU), capability probe |
| `apps/windows/src/render.rs` | Direct3D 11: textures, YUV → RGB shader, swap chain |
| `apps/windows/src/audio.rs` | libopus and WASAPI |
| `apps/windows/src/input.rs` | kynput pipeline: keyboard, mouse, the computer's pointer |
| `apps/windows/src/system.rs` | Log, display, device identity, known consoles |

The core depends on no operating system: another platform provides its window, decoders, sound and inputs through `Platform`.

## Building

Everything builds from the Mac, for Windows x64 (`x86_64-pc-windows-gnu`, Homebrew MinGW, like the server):

```bash
apps/windows/scripts/build.sh
```

The folder to copy to the PC is `apps/windows/dist/x64/`: `kybernow.exe`, `kynput.dll` and `SDL2.dll` (gamepads). The rest comes with Windows 10 and 11 (Universal C runtime, Direct3D 11, `d3dcompiler_47.dll`).

- **Prerequisites**: `third_party/` in place, and `server/kyber-patches/build.sh` run once (it produces `third_party/rootfs-win64`, with FFmpeg and libopus, and `third_party/rootfs-win64-kynput`, with `kynput.dll`). Tool versions: [docs/build-notes.en.md](../../docs/build-notes.en.md).
- **Kyber patches**: the same as for the Apple apps (`apps/macos/kyber-patches`), applied by the script if they are not.
- **VLC**: kyclient always links it on Windows, but the app decodes by itself: the script replaces each function of `libvlc.dll` with an empty one (as on the Apple TV) and refuses an executable that would still depend on `libvlc.dll`.
- `--debug`: without optimisation.
- **Core tests** (on the Mac):

```bash
cd apps && PKG_CONFIG_PATH=$PWD/../third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib cargo test -p kybernow-core
```

## Testing

Copy `dist/x64` to the PC, then in a terminal:

```
kybernow.exe --probe
kybernow.exe 192.168.1.100:9443 --user <account> --profile bureau
```

On first contact, the console's fingerprint is shown: compare it with the console's (Access). The log is in `%LOCALAPPDATA%\KyberNow\kybernow.log`.

## Data

| File | Contents |
|---|---|
| `%APPDATA%\KyberNow\device-id` | This device's identifier for the console (drawn once) |
| `%APPDATA%\KyberNow\consoles.txt` | Remembered console fingerprints (`host:port sha256:…`) |
| `%LOCALAPPDATA%\KyberNow\kybernow.log` | Log of the last session |

## Not yet

- **Milestone 1**: the ARM64 version (Snapdragon PCs), which needs one more toolchain (llvm-mingw) and FFmpeg, libopus and kynput for ARM64.
- **Milestone 2**: the app (home, adding through the network, a code, a link or an address, settings and the “This PC” card, passage, session bar), passwords in the Windows Credential Manager.
- **Milestone 3**: Windows integration (computers on a right-click of the taskbar icon, notifications, `kybernow://` link), per-user installer, updates.
- **Milestone 4**: Windows shortcuts sent to the computer, on-battery setting, Keys menu, pointer drawn in game mode, fallback queue for packets refused by kynput, pointer scaling.
