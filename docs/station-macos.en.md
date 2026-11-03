# KyberNow Station for Mac — scope

English · [Français](station-macos.md)

Scoped on 2026-11-03. This document sets the choices, the milestones and their status. What is planned lives in [ROADMAP.en.md](../ROADMAP.en.md).

## Goal

A Mac with Apple silicon becomes a Station like a Windows PC: same features, same Admin, same icon, and the KyberNow Link apps connect to it without (almost) knowing it is a Mac. For gaming (Steam for Mac, CrossOver, Game Porting Toolkit) as well as remote desktop, with the Game, Desktop and Max profiles.

## Status

**Milestone 1, in progress (2026-11-03)**: Kyber server built for Apple silicon with the Windows Station's 30 server patches, none changed (`server/macos/scripts/build-kyber.sh`, shared application in `server/kyber-patches/apply.sh`), started as a signed LaunchAgent by `server/macos/scripts/run-kyber.sh`. Tried on the M4 Max, locally with this Mac's Kyber client:

- the Mac's three displays found by ScreenCaptureKit, once Screen Recording is allowed for "KyberNow Station (Kyber)";
- HEVC picture through VideoToolbox (about 29 Mbit/s measured for 20 asked on a very busy screen, kyavserver at 3% CPU), stereo Opus sound at 256 kbit/s (output of the chosen display), keyframe on demand (0005);
- HEVC 4:4:4: the encoder opens (4:4:4 picture actually received: to check with a KyberNow app);
- H.264: refused by VideoToolbox on the main display (−12903), probably beyond 4,096 real pixels wide; the virtual display at the device's resolution will avoid it;
- `kVTCompressionPropertyKey_MaxFrameDelayCount` is not honoured by the HEVC encoder: VideoToolbox may hold frames back, to measure with latency;
- the Kyber client (VLC player) often runs short of sound samples: to revisit with the core's sound engine.

Left to finish the milestone: a picture on another device (KyberNow apps go through Admin, milestone 2; or the original Kyber client on the PC), end-to-end latency, input (kynputserver, Accessibility permission).

**Milestone 2, in progress (2026-11-03)**: Admin on Mac. `kyber-admin` recognises a Mac Station (Kyber's LaunchAgent), drives Kyber through launchd, announces itself with `kyber=1` under the Mac's name, gives its real network cards for wake, and `prepare` takes the Mac's display as it is (no virtual display or virtual sound before milestones 3 and 4); details in [server/README.en.md](../server/README.en.md#kybernow-station-for-mac-in-progress). Test Station: `server/macos/scripts/run-station.sh` (certificate of its own, accounts set by `kyber-admin init`). Checked on the M4 Max: Bonjour announcement, `info`, `prepare`, then a Kyber session with the account and fingerprint given by Admin; **picture received on the iPhone by the KyberNow app** (2026-11-03: the Mac's main display in HEVC, 70 Mbit/s asked, stereo Opus sound). Seen during this test: the first stream dropped after 4 s and the app restarted it ("Force IDR failed: Timeout" on Kyber's side), to understand; mouse and keyboard are refused by macOS (`CGPostMouseEvent` → 1002: no Accessibility permission, milestone 5); the home preview fails ("could not create image from display": Admin has no screen recording permission). Left: the home preview, chip monitoring and the icon's local token (still the development Mac's).

**Milestone 5, in progress (2026-11-03)**: mouse and keyboard. Mac-only Kyber patch `server/macos/kyber-patches/0002` (injection through `CGEvent`, see [kyber-changes.en.md](kyber-changes.en.md)); Accessibility permission granted by the author to "KyberNow Station (Kyber)". **Mouse and keyboard checked from the PC** (2026-11-03). Pitfall found during the test: a Mac plugged twice into the same network (Ethernet and Wi-Fi) answered over UDP from its main route's address; the PC, which aimed at the Wi-Fi address (kept since pairing), rejected those answers and never got the picture. Cause: quinn-udp, Kyber's QUIC library, asks for the source address through `IPV6_PKTINFO` (dual-stack socket) or `IP_RECVDSTADDR`, which macOS ignores when sending; only `IP_PKTINFO` is honoured (test of 2026-11-03). Patch `server/macos/crates/quinn-udp-0.5.14-source-mac.patch`: Kyber answers from the address that was called, whichever it is. On top of that, Admin only announces the main route's address (Ethernet before Wi-Fi, as the author wants), reread every 15 s. **Checked from the PC with Wi-Fi on** (2026-11-03: session through the Wi-Fi address, picture, mouse and keyboard). But the whole session then stayed on Wi-Fi: `prepare` now gives `kyber_host`, the Ethernet address, to a device of the same network, and the apps' core takes the picture there (Admin is still reached at the kept address); to be tried with a Windows app built from this branch. Pointer: shape and position announced to the apps by kynput (Mac patch `0003`, see [kyber-changes.en.md](kyber-changes.en.md)), read from the real pointer on a thread (56 × 80 pixels at Retina scale); pointer hidden by a game not followed yet. Left: the clipboard, system keys (⌘⇥), the apps' "Ctrl acts as ⌘" option.

**Milestone 3, in progress (2026-11-03)**: virtual display. Admin starts the `kybernow-display` agent (Swift, `server/macos/display-agent`, driven by `display/mac.rs`): `CGVirtualDisplay` displays at the device's resolution and refresh rate, Retina from 2048 pixels wide (a readable interface on a phone, tablet, Mac or 4K TV; 1080p at 1x), up to 4 displays placed like the device's. During the session, **the Mac's displays mirror the main virtual display** instead of switching off (author's choice, 2026-11-03): only the virtual display is active and main, in 0.4 s. Switching them off (`CGSConfigureDisplayEnabled`, private) was applied by macOS only several seconds late, sometimes only when the program exited. Safety net: the virtual display lives with the agent, which lives with Admin, and changes are made "for this program only"; if either stops, macOS gives the Mac's displays back in place (checked, including a session without heartbeat, closed after 20 s). Mac-only Kyber patch (`server/macos/kyber-patches/0001`, see [kyber-changes.en.md](kyber-changes.en.md)): txproto follows displays that appear and disappear, and announces them in pixels. Checked on the M4 Max: `prepare` at 2868 × 1320 at 120 Hz in 1.8 to 2.3 s, Kyber announces only the virtual display (2868 × 1320 at (0, 0)) and captures it, mode change during the session, end of session. Pitfalls: a Retina display is declared in points (macOS makes the Retina mode out of it; declared in pixels, it takes twice that and its "half" mode is not usable for the desktop); a mirrored display keeps its mode (mirroring is suspended during a change); when the virtual display appears, macOS also changes the mode of the Mac's main display, restored afterwards. **Session from the iPhone at its resolution checked by the author** (2026-11-03: virtual display ready in 1.6 s, 2868 × 1320 at 120 Hz, picture on the first try, HEVC at 56 Mbit/s, the Mac's displays back at the end). Left: multiple displays from a Mac with several screens, HDR of virtual displays (offered later).

## Choices

| Topic | Choice |
|---|---|
| Supported Macs | Apple silicon only (M1 and later), macOS 15 Sequoia or later. No Intel Macs |
| Engine | Kyber 0.29's macOS server (kycontroller, kyavserver with txproto, kynputserver), with our patches. No engine of our own |
| Admin | The same `kyber-admin`, built for macOS, same routes and same interface; Windows-only parts get their Mac equivalent behind the same modules |
| Virtual display | `CGVirtualDisplay`, Apple's private API (the one BetterDisplay and DeskPad use): the device's resolution and refresh rate, up to 4 displays; during the session, the Mac's displays mirror the virtual display (choice of 2026-11-03, instead of switching them off). No App Store, and a macOS release can break it: checked at every beta |
| Sound | A KyberNow virtual audio device from the first version (Core Audio plug-in, 2 to 8 channels), like the virtual speakers on Windows |
| Gamepads | Not in the first version: keyboard and mouse. Apple's entitlement for a virtual HID device (`com.apple.developer.hid.virtual.device`) is requested right away; gamepads come as soon as it is granted |
| Locked Mac | The unlock screen is streamed and the password can be typed. After a restart (FileVault), the session must be opened in front of the Mac or automatic login set |
| First version | Everything the Windows Station does, except gamepads: virtual display and multiple displays, sound, profiles, pairing, wake, Share and relay, Lend the PC, HDR, app at session start, updates from Admin |
| Tests | On the author's Mac (M4 Max) as the Station, to the Apple TV, the iPhone, the PC and the LG TV |

## What already exists

- **Kyber can serve from a Mac** (0.29, "Add macOS server support"): ScreenCaptureKit capture of picture and sound (`txproto/src/iosys_screencapturekit.m`), VideoToolbox encoding (H.264, HEVC), keyboard and mouse injected by `kynput/src/macos/host_keyboard_mouse.rs`, `tech.kyber.server` LaunchAgent in the user's session and at the login window. The libavconv engine has no Mac capture: txproto only.
- **Admin already runs on a Mac** for development, with simulated drivers (virtual display, sound, wake, app catalogue, local token, preview through `screencapture`, system name through `sw_vers`). Windows-only modules sit behind `cfg(windows)`: the simulations get replaced with the real thing.
- **The icon** (`server/tray`) is made of crates that exist on Mac (tao, tray-icon, wry); only its system calls are Windows-specific.
- **The apps already read what the Station can do** in `GET /api/stream/info` (`virtual_display.max_screens`, `speakers`, `hdr`, `wake`, `os`). A Mac Station describes itself there, and the apps adapt without knowing it is a Mac.

## What is missing, piece by piece

| Piece | Windows | Mac |
|---|---|---|
| Services | `kyber` and `kyber-admin` services (SYSTEM), agents started in the user's session | Admin as a LaunchDaemon (root); Kyber and a KyberNow agent as LaunchAgents in the user's session and at the login window |
| Capture | DXGI / WGC (txproto) | ScreenCaptureKit (txproto, exists); HDR through `captureDynamicRange` (macOS 15) |
| Encoder | NVENC, AMF, QSV | VideoToolbox. No AV1 encoding on known Apple chips; HEVC 4:4:4 to check per chip |
| Virtual display | SudoVDA + CCD API | `CGVirtualDisplay` in the agent; physical displays off or mirrored (private display configuration API, picked during tests); the display goes away by itself if the agent dies, which is the safety net |
| Sound | Steam's speakers, `IPolicyConfig` | "KyberNow" Core Audio plug-in (2, 6 or 8 channels) set as output for the session, original output restored at the end; Kyber reads it as a Core Audio input (txproto patch) |
| Keyboard, mouse | kynput (SendInput) | Mac kynput to rework: `CGEvent` instead of `CGPostMouseEvent`, relative moves for games, fine scrolling, system keys (patch 0021); Accessibility permission |
| Pointer | Shapes announced by the capture (0002, 0006) | System pointer shape and position read by kynput and announced as on Windows (Mac patch 0003) |
| Clipboard | kynput | `NSPasteboard` in Mac kynput (text, both ways) |
| Gamepads | ViGEmBus | Virtual HID (Apple entitlement) — later |
| Monitoring | NVML, "GPU Engine" counters | Chip name, encoder and GPU load (IOReport), VideoToolbox sessions |
| Wake | WoL, `powercfg` | WoL with "Wake for network access" (`pmset womp`), adapters read with `getifaddrs` |
| Home preview | Agent, GDI | Agent, `SCScreenshotManager` |
| App at start | Start menu, `ShellExecuteEx` | `/Applications` and their icons, `NSWorkspace`; Steam Big Picture and other suggestions |
| Fullscreen game (Auto mouse) | Agent | Agent: frontmost app in fullscreen, browsers and video players excluded |
| Lend the PC | Console mode in the host's session | Same principle in the host's session |
| Install | Installer (Rust, WebView2) | `.pkg` signed with Developer ID and notarized, then a welcome window that walks through the permissions (Screen Recording, Accessibility) |
| Updates | minisign manifest, SYSTEM applier | Same manifest, root applier, same rollback |

## macOS permissions

- **Screen Recording** and **Accessibility**: granted once, at install, through the welcome window that explains why.
- **macOS 15 monthly reminder** for apps that record the screen: to request from Apple, the `com.apple.developer.persistent-content-capture` entitlement (meant for remote desktop) avoids it. Without it, the Station must say so plainly in Admin and in the icon.
- **Login window**: capture and typing there go through a process started at the login window (`LimitLoadToSessionType` `LoginWindow`, like Kyber's LaunchAgent): check this first, it is the least certain part.

## What the KyberNow Link apps see

As few changes as possible, and only through what the Station announces:

- `GET /api/stream/info` already gives `os` ("macOS 26.1"); it gains `gamepads` (false while gamepads are missing). Living-room apps then say "Gamepads unavailable on this computer" instead of showing the gamepad row.
- **Keyboard**: a PC, phone or TV connecting to a Mac sends Ctrl where the Mac expects ⌘. A "Ctrl acts as ⌘" option, offered only with a Mac Station, with its default in the core.
- **Wake**: the help text talks about the Mac's setting instead of the network adapter and BIOS.
- Rules and texts are written first in [link-ui.en.md](link-ui.en.md), then carried into every app.

## Milestones

Each milestone ends with a real test on the M4 Max.

1. **Kyber on this Mac.** Build kycontroller, kyavserver (txproto) and kynputserver for `aarch64-apple-darwin` with our patches, start them by hand and receive the picture on the Apple TV and the PC. Sort the patches: those for kycontroller, kynet and kyavservice apply everywhere (keyframe, send queue, per-display bitrate, sharing, HD sound…); those for txproto and libavgrab mostly touch DXGI, WGC and WASAPI, to redo one by one for ScreenCaptureKit. Measure encoding and latency in HEVC, and 4:4:4.
2. **Admin on Mac.** LaunchDaemon and LaunchAgents, Station restart, chip monitoring, wake, preview, local token, system name: simulations become real. Pairing, Bonjour and the web client then work as on Windows.
3. **Virtual display.** Swift agent with `CGVirtualDisplay`, one to four displays placed like the device's, the Mac's displays off then restored, mode watch ("Leave it" / "Restore" policy).
4. **Sound.** KyberNow Core Audio plug-in, output switch, read by Kyber, 5.1 and 7.1.
5. **Input.** Mac kynput reworked (CGEvent, relative, system keys), clipboard, pointer, Auto mouse; login window.
6. **Icon, package, updates.** Menu bar icon, notarized `.pkg` with its permissions window, uninstall, update from Admin.
7. **Advanced features.** Share and relay, app at session start, Lend the PC, HDR.
8. **Link apps.** `gamepads`, Ctrl/⌘ option, help texts; then gamepads once Apple agrees.

## Check first

- Capture and typing at the login window of a locked Mac.
- Physical displays off while `CGVirtualDisplay` runs, then restored as they were; four virtual displays at once.
- HDR on a `CGVirtualDisplay` display and its capture.
- HEVC 4:4:4 through VideoToolbox per chip.
- ScreenCaptureKit latency at 120 Hz (queue depth, frame sent as soon as it changes).
- Apple's answer for virtual HID and for the monthly screen recording reminder.
