# KyberNow for Windows

English · [Français](README.md)

KyberNow's streaming client for Windows 10 (22H2) and 11: your computer (Kyber 0.29 server + KyberNow console) in full screen on a PC, with image, sound, keyboard, mouse and gamepads. Target: a laptop or low-powered computer connecting to a more powerful one. Decisions and milestones: the “Windows client” row of [PROJECT.en.md](../../PROJECT.en.md); mock-ups: https://claude.ai/artifact/JkUTEMQmxKjcYksaXRHgBf.

**Milestone 2 (the app)**: the app opens on its home screen (Tauri 2, Svelte interface in WebView2): your computers, adding them, their settings, the passage into the session and the session bar. The session itself (milestone 1) stays a Win32 window in Direct3D 11; the command line remains for testing.

It is the first client written on the **Rust client core** ([`apps/core`](../core), crate `kybernow-core`), which Android and Linux can use later; the Apple apps stay in Swift.

## What the app does

### Outside a session (Svelte interface, `ui/`)

- **Home**: the chosen computer in full frame, its image (a preview of its display requested from the console, at most once a minute, arriving with a 45° sweep) or else its **signature** (two glows whose hue comes from its console's fingerprint), desaturated when offline; its name, system, graphics card, address; the virtual display waiting for it (“1920 × 1200 · 60 Hz, like this display”); **Connect** and the profile (Game, Desktop, Max); on the right the latency of the last session (median, 95th percentile, duration) on the one-frame ruler; at the bottom your computers, those announced on the network (they light up as they appear) and the add card. Online or not: the console is polled every 8 s.
- **Adding a computer**: find it **on the network** (the console's Bonjour announcement `_kybernow._tcp`, fingerprint included), **with the console's link** (`kybernow://…`, pasted) or **by address** (the console's fingerprint is then shown, the first four groups to compare with the console's); then get access **with a six-digit code** to accept in the console (Access › Devices: the device gets its own account, nothing to type) or **with an account** and its password, checked with the console. No camera QR code.
- **Settings per computer** (saved on each change): profiles, a setting that departs from them (format, colours, bitrate, transport) makes them “Custom”; display (like this display or 4K at most), format (those the card decodes), 4:4:4, synchronised or immediate display, bitrate (real bitrate shown); Auto, Desktop or Game mouse, gamepad seen by the computer (Xbox 360 or faithful), clipboard, sound (Auto, stereo, 5.1, 7.1, none), transport, virtual display; **This PC** card (graphics card, what it decodes, display, sound) and **on battery** (lighten: 30 Hz at most). Rename, remove.
- **Passage**: the home screen split along the 45° seam, a neon tube lit during preparation (capsule: virtual display, connection, first frames, Cancel), then the session window shows in full screen at the first frame and the home screen hides; at the end it comes back, the halves close and the last session is recorded.
- **Session bar**: a transparent window above the session, shown when the pointer touches the top of the screen, hidden when it leaves (the keyboard returns to the session): lit symbol, latency and its curve, resolution, frames per second, decoder, profile, mouse, full screen, Quit.
- **Passwords** in the Windows Credential Manager (`KyberNow:<id>`), never in a file.

### Session

- **Session** (core, like `SessionController.swift`): preparation by the console (`POST /api/stream/prepare`: virtual display at this PC's display resolution and refresh rate, virtual sound output with as many channels as this PC's output), connection to Kyber with the same account, Kyber's certificate required as the console gives it (`kyber_fingerprint`), display choice (waits at most 6 s for Kyber to announce only the virtual display), heartbeat every 5 s carrying the latency (the console can ask there for the session to end), the computer's foreground polled every second (Auto mouse), clean end (`POST /api/stream/end`, session latency). Recovery: Kyber's automatic reconnection, display requested again from the console if Kyber restarted, image restarted (at most 3 times) after an image pipeline error, keyframe requested as soon as a reference frame is missing (every 300 ms until it arrives).
- **Console**: Basic account, `x-kyber-admin` header, certificate pinned by its fingerprint (`sha256:aa:bb:…`, SHA-256 of the certificate). Without a given fingerprint (`--fingerprint`), the first one seen is shown and remembered in `%APPDATA%\KyberNow\consoles.txt`; a different certificate afterwards stops the connection.
- **Profiles** Game, Desktop and Max, brought down to what the graphics card decodes, and **bitrate** following pixels per second: same rules and values as the Mac app (tested in `apps/core`). A format, colour or bitrate option makes the profile “Custom”.
- **Image**: FFmpeg (Kyber's, statically linked) decodes on the graphics card (**D3D11VA**, never leaving the card); at launch, the app really decodes a small frame of each format (H.264, HEVC, HEVC 4:4:4, AV1, in `src/probe/`) to learn what the card can do. If the card does not decode a format, or decodes it into a texture format we cannot show, the app decodes on the **CPU** (slice threads only: frame threads add one frame of delay each). Direct3D 11 rendering: a shader converts YUV (NV12, 4:4:4 AYUV or three planes) to RGB according to the stream's colour space and range, at window size keeping proportions; “flip” swap chain, at most one frame waiting; every packet is decoded but only the newest frame is shown (if others are waiting, this one is skipped: delay does not build up behind vertical sync). Vertical sync by default; `--no-vsync` presents each frame as soon as it is decoded (tearing allowed when Windows allows it, useful with a G-SYNC or FreeSync display).
- **Sound**: libopus (loss concealment included), WASAPI in event-driven shared mode; Windows converts to the output's format. A 30 ms reserve; more than 100 ms behind, the oldest is dropped. Channels put back in Windows order in 5.1 and 7.1.
- **Keyboard**: Windows key codes as they are (set 1, `0xE000` for extended keys, like Kyber's client), without repeats (the computer repeats by itself); Alt and F10 do not open a local menu. Windows shortcuts (Windows key, Alt+Tab) stay on this PC for now (milestone 4).
- **Mouse**: absolute position brought to the computer's pixels, buttons (including both side buttons), wheel. Games mouse (relative, raw movements through Raw Input, pointer hidden and held in the window): automatic when the console sees a full-screen game and the computer hides its pointer, or forced with Ctrl+Alt+Shift+M.
- **Pointer**: the computer's pointer (shape and hotspot) becomes the Windows pointer over the image; a fully transparent shape keeps the previous one.
- **Gamepads and clipboard**: through kynput's client configuration on Windows (SDL for gamepads, seen by the computer as Xbox 360 pads; text clipboard both ways).
- **Refused packets**: kynput's send queue holds 64 packets and refuses without notice when full; refused keys and buttons are retried every 3 ms, in order, for up to 200 ms (like `PacketQueue.swift`), positions, moves and wheel are sent once. The total of refusals goes to the log.
- **Nothing stays pressed**: when the window loses the keyboard, the app releases on the computer the keys and buttons it holds there; at the end of the session, everything is released.
- **Measurements**: the window title gives the end-to-end latency (capture on the computer → presentation here, on Kyber's adjusted clock), the resolution, the decoder and frames per second; the session median and 95th percentile go to the log and to the console.

| Shortcut | Effect |
|---|---|
| Ctrl+Alt+Shift+D | Disconnect (also: closing the window) |
| Ctrl+Alt+Shift+F | Full screen or window |
| Ctrl+Alt+Shift+M | Games or desktop mouse |

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
| `apps/core/src/console.rs` | Console client (`info`, `prepare`, `heartbeat`, `foreground`, `end`), pinned certificate |
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
| `apps/windows/src/window.rs` | Win32 window, keyboard, mouse, pointer, shortcuts, full screen |
| `apps/windows/src/video.rs` | Image thread: packets → decoder → rendering, keyframes |
| `apps/windows/src/decoder.rs` | FFmpeg (D3D11VA or CPU), capability probe |
| `apps/windows/src/render.rs` | Direct3D 11: textures, YUV → RGB shader, swap chain |
| `apps/windows/src/audio.rs` | libopus and WASAPI |
| `apps/windows/src/input.rs` | kynput pipeline: keyboard, mouse, the computer's pointer |
| `apps/windows/src/system.rs` | Log, display, device identity, known consoles |

The core depends on no operating system: another platform provides its window, decoders, sound and inputs through `Platform`.

## Building

Everything builds from the Mac:

```bash
apps/windows/scripts/build.sh                # x64 (x86_64-pc-windows-gnu, Homebrew MinGW, like the server)
apps/windows/scripts/build.sh --arch arm64   # ARM64, Snapdragon PCs (aarch64-pc-windows-gnullvm, llvm-mingw)
```

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
| Credential Manager, `KyberNow:<id>` | Each computer's password |
| `%APPDATA%\KyberNow\device-id` | This device's identifier for the console (drawn once) |
| `%APPDATA%\KyberNow\consoles.txt` | Console fingerprints remembered by the command line (`host:port sha256:…`) |
| `%LOCALAPPDATA%\KyberNow\kybernow.log` | Log of the last session |

## Not yet

- **Milestone 2, remaining**: image of the last session on the home screen (today the console's preview), changing the profile from the bar.
- **Milestone 3**: Windows integration (computers on a right-click of the taskbar icon, notifications, `kybernow://` link), per-user installer, updates.
- **Milestone 4**: Windows shortcuts sent to the computer, on-battery setting, Keys menu, pointer drawn in game mode, pointer scaling, display requested again when the heartbeat says it is gone.
