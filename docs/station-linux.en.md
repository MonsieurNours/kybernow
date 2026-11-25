# KyberNow Station for Linux — scope

English · [Français](station-linux.md)

Scoped on 25/11/2026. This document sets the choices, the breakdown and the state of the milestones, like [station-macos.en.md](station-macos.en.md) for the Mac.

## The goal

A computer running Linux becomes a Station like a Windows PC or a Mac: same features, same Admin, and the KyberNow Link apps connect to it without (almost) knowing it is Linux. For gaming (Steam, Proton, Lutris, Heroic) as for remote desktop, with the Game, Desktop and Max profiles.

## Choices (approved by the author on 25/11/2026)

| Topic | Choice |
|---|---|
| Desktops | GNOME and KDE Plasma 6 on Wayland, on par. On X11 and wlroots desktops (Sway, Hyprland): the displays as they are, without virtual displays |
| Distributions | Ubuntu 24.04, Debian 13, Fedora 42 and later. Immutable systems (Bazzite, SteamOS) later |
| Packages | A `.deb` and a `.rpm` from the same assembly, with the udev rule and signed updates. No Flatpak (uinput, services and unsandboxed capture do not fit there) |
| Architectures | x86_64 and arm64 |
| Cards and encoders | NVENC (NVIDIA), VA-API (AMD, Intel), x264 as a fallback, picked from the card at installation; frames to the encoder without copies (DMA-BUF); PyroWave in Vulkan |
| Input | uinput (X11 and Wayland), not libei; Xbox controllers through uinput, DualSense through uhid |
| Locking | As on Mac: the session's lock screen is streamed; after a restart, the session must open on its own (automatic login) |
| Services | Kyber, Admin (and later the tray icon) as systemd user services, in the user's graphical session; a separate root service to apply updates |
| Sound | A « KyberNow » PipeWire output with 2, 6 or 8 channels, chosen for the session, then the original output given back (like the Mac's KyberNow output) |
| Engine | Kyber 0.29's Linux server (kycontroller, kyavserver with the libavconv engine, kynputserver), with our common patches and Linux-specific ones. No engine of our own |

## What already exists

- **Kyber can serve from Linux**: capture through NvFBC (its default, NVIDIA only), DRM/KMS, X11, wlroots and GStreamer (libavconv engine); NVENC, VA-API, x264 encoding; keyboard, mouse and controllers through uinput (`kynput/src/linux`). What we add is what makes a Station: Admin, virtual displays, sound, package.
- **Admin already runs outside Windows** (development Mac and Mac Station): system-specific modules sit behind `cfg`s, with a simulated version; the Linux Station adds its real versions, as the Mac did.
- **The apps already read what the Station can do** (`GET /api/stream/info`: `os`, `hdr`, `pyrowave`, `gamepads`, virtual display and sound): a Linux Station describes itself there, and the apps adapt without change ([link-ui.en.md](link-ui.en.md), “With a Linux Station”).

## How it is done

### Displays (GNOME)

GNOME lets no program read the screen directly: everything goes through Mutter, its compositor. The Station has its own agent, `kybernow-display` (`server/linux/display-agent`, Rust), started by Admin as soon as it starts, which speaks the same protocol as the Mac agent (JSON lines: `create`, `remove`, `isolate`, `restore`, `state`, `set_mode`, `displays`…). Admin drives it through `display/linux.rs`.

- **Virtual displays**: `org.gnome.Mutter.ScreenCast`, `RecordVirtual`, one capture session per display (Mutter does not start a stream added after `Start`), pointer left out of the picture (each device draws its own). Mutter creates the display when the first reader plugs into the PipeWire stream, at the size that reader asks for: Kyber's capture sets the device's resolution. “Platform” display (`is-platform`): GNOME puts its bar and windows on it as on a real display.
- **The computer's displays**: the same API, `RecordMonitor`, without the screen-sharing dialog (the Station is not sandboxed). Captured only when there is no virtual display (sharing from the PC, session without a virtual display).
- **Layout**: `org.gnome.Mutter.DisplayConfig`, `ApplyMonitorsConfig` in temporary mode (never saved). During the session, only the virtual displays stay on the desktop, the main one first and primary, as on Windows and Mac; at scale 2 from 2,048 pixels wide (like the Mac's Retina). Places follow GNOME's layout mode: in pixels with the “physical” layout (`layout-mode` 2), scaled down with the “logical” one. Virtual displays only appear with the first stream: the layout waits for them (an agent thread, every 250 ms). At the end, the previous layout comes back. Computer lending: the computer's displays stay, on the right.
- **What Kyber reads**: a TOML file of GStreamer sources (`~/.local/share/kybernow/station/kyber-screens.toml`), kept up to date by the agent: display identifier, PipeWire stream (`pipewiresrc target-object=<serial number>`: the node's serial number, read with `pw-dump`, not the identifier Mutter gives, which PipeWire reuses), size, place on the desktop. Kyber rereads it at each display listing and each stream, and watches it to announce its changes (Linux Kyber patch 0001): a virtual display created, removed or resized appears without restarting Kyber.
- **Identifiers**: `linux:101` for the first virtual display (101, 102… for Kyber), `linux:1`, `linux:2`… for the computer's.
- **Safety net**: virtual displays live with the agent's session-bus connection; if the agent or Admin stops, GNOME removes them and goes back to its saved layout.

### Displays (KDE Plasma)

The agent picks its desktop at startup: GNOME if Mutter is on the session bus, otherwise KDE if KWin gives it its capture protocol (`KYBERNOW_DESKTOP=gnome|kde` to force). On KDE (`kde.rs`):

- **Virtual displays**: KWin's Wayland protocol `zkde_screencast_unstable_v1` (version 2 and later, Plasma 5.25 and later), `stream_virtual_output`: KWin creates the display “KyberNow-1” (“KyberNow-2”…) at the given size (in points, with the scale: 2 from 2,048 pixels wide) and its PipeWire stream comes with the `created` event. KWin sets the size at creation: a resolution change during the session remakes the display.
- **The computer's displays**: `stream_output` on their `wl_output` (named by version 4 of `wl_output`), without the portal's dialog.
- **Right to the protocol**: KWin only gives it to a program declared in a `.desktop` file whose `Exec` is the agent and which names the interface (`X-KDE-Wayland-Interfaces=zkde_screencast_unstable_v1`). The package installs `/usr/share/applications/fr.leslignesclaires.kybernow.station.display.desktop`; the test Station, its copy in `~/.local/share/applications`. Without it, the agent says so in Admin's log.
- **Layout**: `kscreen-doctor` (enable, place, scale, priority; displays left out of the layout are disabled); places in points. The list is read from its text output (`-o`): `kscreen-doctor -j` crashes on Plasma 5.27.
- **Safety net**: virtual displays live with the agent's Wayland connection; if it stops, KWin removes them.

### Keyboard, mouse, pointer

kynputserver injects keyboard, mouse and controllers through `/dev/uinput`, on X11 as on Wayland. The package installs a udev rule (`60-kybernow-uinput.rules`, `uaccess`): the user sitting at the computer gets access, without being root.

**Absolute pointer on Wayland** (Linux Kyber patch 0002): on Wayland, no program can place the pointer (XTest only moves Xwayland's). kynput therefore plugs a second uinput device, « KyberNow pointeur », with two absolute axes and a mouse's buttons: udev classes it as an absolute mouse (like a virtual machine's tablet) and the compositor spreads it over its whole desktop. Its axes cover the box around the displays the Station announces (places written by the agent). While it holds the pointer, clicks and wheel go through it (GNOME does not apply another mouse's clicks in that case); a relative move (captured pointer, games) hands back to the relative mouse. On X11, nothing changes (XTest).

### Sound

Admin (`audio/mod.rs`, `linux` module) creates at the start of the session a « KyberNow » output (PipeWire's `module-null-sink`, through `pactl`) with the device's channel count (2, 6 or 8, PulseAudio order), makes it the default output and moves the playing sounds to it: nothing comes out of the computer's speakers. Kyber captures the default output's monitor. At the end: original output given back, sounds moved back, module removed (and removed at startup if one was left by an interrupted session).

### Encoder

Installation picks the card's (`kybernow-station-setup`): NVIDIA → NVENC; AMD or Intel with a render node → VA-API; otherwise x264. Admin offers these three in Settings › Picture (“VA-API — AMD or Intel card”).

### Admin's texts

Admin gives `platform`: `linux` (`GET /api/session`): the interface and Admin replace each Windows-specific text (Start menu, SudoVDA driver, Steam speakers, Windows session…) with its Linux version, stored under `linux` in the catalogues (`server/web/src/lib/i18n/`, `server/admin/src/i18n/`), and Kyber's settings texts through `schema.linux.json` ([i18n.en.md](i18n.en.md)). Found at the first session from the Mac (25/11/2026): Admin talked about Windows.

### Services and package

- `kybernow-admin.service` and `kybernow-kyber.service`, systemd user services (`/usr/lib/systemd/user`), tied to the graphical session: they start at login and stop with it. Admin drives Kyber through `systemctl --user` (`linux.rs`), and restarts the Station through systemd.
- First start: `kybernow-station-setup` (before Admin) writes Kyber's Linux settings (libavconv engine, GStreamer capture, displays file), a certificate of the Station's own, the card's encoder, the first account (your user name, random password in `~/.local/share/kybernow/station/admin/mot-de-passe`).
- Data: `~/.local/share/kybernow/station/` (`admin/`, `kyber/`, `kyber-screens.toml`).
- Package: `server/linux/scripts/package.sh` → `kybernow-station_<version>_<arch>.deb` and `kybernow-station-<version>.<arch>.rpm` (`/usr/lib/kybernow-station`, services, udev rule). At installation, the services are enabled for every user (`systemctl --global enable`) and the udev rule is loaded.

## Linux-specific Kyber patches

Kept in `server/linux/kyber-patches`, applied after the common patches by `server/linux/scripts/build-kyber.sh` (details in [kyber-changes.en.md](kyber-changes.en.md)):

| Patch | What it changes |
|---|---|
| `0001-kyavservice-ecrans-suivis-linux` | GStreamer sources file (`[kyavserver.gstreamer] sources_file`) reread at each listing and each stream, watched to announce changes; identifier and place of each source |
| `0002-kynput-pointeur-wayland` | Absolute pointer through uinput on Wayland, clicks and wheel through it while it holds the pointer |
| `0003-libavgrab-gstreamer-1-24` | Builds with GStreamer 1.24 (Ubuntu 24.04), which lacks `gst_message_get_details` |

## Building

On a Linux machine of the wanted architecture (the « KyberNow Linux » VM for arm64; x86_64 on a PC or in the same VM through Rosetta):

```sh
# Tools (Ubuntu 24.04; similar names on Debian and Fedora)
sudo apt install build-essential cmake nasm bison flex pkg-config pipx lua5.4 python3-jinja2 \
  glslang-tools spirv-tools libclang-dev libssl-dev libv4l-dev libcap-dev libvdpau-dev libnuma-dev \
  libva-dev libdrm-dev libgbm-dev libegl-dev libvulkan-dev libplacebo-dev libshaderc-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpipewire-0.3-dev libpulse-dev libasound2-dev \
  libxcb1-dev libxcb-shm0-dev libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shape0-dev libxtst-dev \
  libxkbcommon-dev libwayland-dev wayland-protocols libevdev-dev libudev-dev libinput-dev
pipx install meson==1.11.2 && pipx inject meson jinja2
cargo install cargo-c --version 0.10.15 && rustup toolchain install 1.89.0

# Kyber sources kept apart (as for the Mac), then Kyber with our patches
KYBER_DIR=<copy of kyber-desktop> server/linux/scripts/build-kyber.sh
# Test Station in the session (systemd user services)
KYBER_DIR=<copy> server/linux/scripts/run-station.sh start
# .deb and .rpm packages
KYBER_DIR=<copy> server/linux/scripts/package.sh
```

Pitfalls met: the `luac` of Kyber's Lua subproject does not link on Linux (it is only built when no `luac` is installed: hence `lua5.4`); libplacebo needs jinja2 in meson's Python (`pipx inject`); Kyber's scripts hard-code the `x86_64-linux-gnu` folder (`build-kyber.sh` replaces it with the machine's).

## What the KyberNow Link apps see

Nothing Linux-specific ([link-ui.en.md](link-ui.en.md), “With a Linux Station”): a PC, whose system is the distribution's name. What is not there yet (HDR, PyroWave, pen, lending) is not announced, and the apps behave as with a Station that does not have it.

## Breakdown

1. **Kyber on Linux.** Built with our patches for x86_64 and arm64 (`build-kyber.sh`), without the VLC player. **Done in the arm64 VM (25/11/2026).**
2. **Admin on Linux.** systemd service, Station recognised, restart, encoders: **written**. Wake on LAN (ethtool), home preview, graphics card name, app at session start (`.desktop` files, Steam): **to do**.
3. **Virtual displays.** GNOME through Mutter: **written and tried in the VM** (see “Tests”). KDE through KWin: **written**; capture and access right tried on KWin 5.27, virtual display not yet (see “Tests”).
4. **Sound.** KyberNow PipeWire output: **written**.
5. **Input.** uinput, udev rule, absolute pointer on Wayland: **written**. DualSense controllers through uhid, pen (uinput tablet, following common patch 0027), clipboard on Wayland: to do.
6. **Package, updates.** `.deb`, `.rpm`, services: **written**. Tray icon, update applier (root service), update from Admin: to do.
7. **Advanced features.** Share and relay (nothing system-specific in Admin: to try), PC lending (console mode in the host's session, sleep prevented by `systemd-inhibit`), HDR (GNOME virtual display in BT.2100: GNOME does not offer it yet), PyroWave (Vulkan, DMA-BUF import), live bitrate with VA-API (to add to common patch 0026): to do.
8. **Immutable systems** (Bazzite, SteamOS). To do.

## Tests

Everything in the « KyberNow Linux » Parallels VM (Ubuntu 24.04 arm64, GNOME 46 on Wayland, virgl virtual graphics card, no VA-API or NVENC), see [apps/linux/README.en.md](../apps/linux/README.en.md).

**Tried (25/11/2026)**:

- **Building** Kyber for arm64 with the common patches (0001 to 0027) and the three Linux patches; Admin and the display agent; Admin's tests pass on Linux (322) and on the Mac (326).
- **Test Station** (`run-station.sh start`): Kyber, Admin and the agent start as systemd user services; Admin announces itself through Bonjour (`kybernow-linux`, `kyber=1`); `GET /api/stream/info` gives “Ubuntu 24.04.5 LTS”, the virtual display (up to 4 displays), the sound output, controllers, no HDR, PyroWave or lending. Kyber finds the VM's display through the displays file 250 ms after it starts (patch 0001).
- **Session without an app**, through Admin's API on `localhost` with the test account (`prepare`, heartbeat, `end`) and a GStreamer reader plugged in like Kyber's capture: 1920 × 1080 virtual display created, appeared in GNOME (“Meta-0”), alone on the desktop and primary, the VM's display taken off; sound to the « KyberNow » output in 2 then 6 channels, default output; at the end, the VM's display, layout and original output given back, KyberNow output removed. **Two displays**: 2560 × 1440 at scale 2 at (0, 0) and 1920 × 1080 at (2560, 0), both streams get their frames, same places in Kyber's file.
- **Kyber** on the virtual display (Kyber's RTP stream, in single-client mode for the test): kyavserver opens the “KyberNow” GStreamer source, the x264 encoder and its RTP context; kycontroller then refuses that test stream for lack of a control channel, as expected without an app.
- **Packages**: `.deb` (48 MB) installed with apt: first setup, services started, agent taken from `/usr/lib/kybernow-station`, `/dev/uinput` opened to the user by the udev rule; then uninstalled. `.rpm` built (79 MB), not installed (no Fedora at hand).

Pitfalls found on the way: the app picks its display from Kyber's list as soon as `prepare` answers, before Kyber has reread the displays file (“Unknown GStreamer display id 1”, first session from the Mac): on a Linux Station, `prepare` waits until Kyber lists the created displays (2 s at most, 266 ms measured); `pipewiresrc path=<n>` expects the node's serial number (the first try landed on another node, “not-negotiated”, or on nothing); Admin ends a session without the app's heartbeat (5 s); the previous layout, given back right after a virtual display was removed, was refused (“stale information”): the agent retries with the next state; the VM's GNOME uses the “physical” layout.

**KDE (25/11/2026)**, on a headless KWin 5.27 started next to GNOME in the VM (`kwin_wayland --virtual`, Plasma 6 doesn't exist for Ubuntu 24.04): the agent finds the protocol (version 3), captures KWin's display (stream negotiated at 1920 × 1080 BGRx) and reads its layout through `kscreen-doctor`; KWin's access control refuses the protocol without the `.desktop` file and gives it with it. **The virtual display could not be tried**: this headless KWin refuses it (“Could not find output”); only the DRM backend of a real Plasma session can create one. Two defects found and fixed on the way: a layout without a virtual display disabled the displays on KDE, and a refused virtual display left Kyber without a display.

**Not tried yet**: a real Plasma 6 session (virtual display, layout applied, session from an app), an end-to-end session from a KyberNow Link app (picture, sound, latency), keyboard, absolute pointer and clicks on Wayland, controllers, VA-API and NVENC (no real graphics card in the VM), x86_64, Fedora, a real machine.
