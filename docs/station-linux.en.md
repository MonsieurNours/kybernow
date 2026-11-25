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

- **Virtual displays**: KWin's Wayland protocol `zkde_screencast_unstable_v1` (version 2 and later, Plasma 5.25 and later), `stream_virtual_output`: KWin creates the display “KyberNow-1” (“KyberNow-2”…; Plasma 6 calls it “Virtual-KyberNow-1”) at the given size (in points, with the scale: 2 from 2,048 pixels wide) and its PipeWire stream comes with the `created` event. KWin sets the size at creation: a resolution change during the session remakes the display.
- **The computer's displays**: `stream_output` on their `wl_output` (named by version 4 of `wl_output`), without the portal's dialog.
- **Right to the protocol**: KWin only gives it to a program declared in a `.desktop` file whose `Exec` is the agent and which names the interface (`X-KDE-Wayland-Interfaces=zkde_screencast_unstable_v1`). The package installs `/usr/share/applications/fr.leslignesclaires.kybernow.station.display.desktop`; the test Station, its copy in `~/.local/share/applications`. Without it, the agent says so in Admin's log.
- **Layout**: `kscreen-doctor` (enable, place, scale, priority; displays left out of the layout are disabled); places in points. The list is read from its text output (`-o`, one line per display on Plasma 5.27, one line per property on Plasma 6; a Plasma 6 virtual display says neither “enabled” nor “disabled”): `kscreen-doctor -j` crashes on Plasma 5.27.
- **Display turned off then on**: KWin removes it, then announces it under a new `wl_output` object; the agent follows those removals and retries the capture for a moment at the end of a session.
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

### Immutable systems (Bazzite, Fedora Atomic, SteamOS)

On Bazzite, the Fedora Atomic desktops (Silverblue, Kinoite…) and SteamOS, `/usr` is read-only and replaced as a whole by each system update: the `.deb` and the `.rpm` don't go in as they are. Three ways were studied (milestone 8, 25/11/2026):

| Way | Bazzite, Fedora Atomic | SteamOS (Desktop Mode) | Verdict |
|---|---|---|---|
| **Layered rpm** (`rpm-ostree install ./kybernow-station-….rpm`) | Works with the `.rpm` as is, for every account; reboot, each system update recomposes the image (slower, and blocked if a dependency goes missing); Bazzite advises against layering. The Station can't update itself: there is no `rpm -U`, it takes `rpm-ostree` (root, reboot) | Impossible: pacman is locked, and what it installs is wiped by the next update | Tolerated on Fedora Atomic, not offered |
| **systemd-sysext extension** (image in `/var/lib/extensions`, merged over `/usr`) | Works (root, `extension-release` with `ID=_any`), rebuilt with each Station update | Works in principle, but root at each update, the `systemd-sysext` service to enable, and `/var` belongs to each A/B partition: nothing guarantees the image follows a SteamOS update | No: root every time, and fragile on SteamOS |
| **User's folder** (`~/.local/lib/kybernow-station`) | Yes | Yes | **Chosen** |

Why the user's folder: everything the Station needs is already per user (systemd `--user` services, data in `~/.local/share/kybernow/station`, sound, displays), the home folder survives every system update, and the Station updates without a password, as on the Mac. What remains, with or without root:

| Piece | Without root? | Where |
|---|---|---|
| Kyber, Admin, the display agent | Yes | `~/.local/lib/kybernow-station/versions/<version>`, `current` link (and `previous`) |
| `--user` services | Yes | `~/.config/systemd/user/kybernow-*.service`: the package's, `/usr/lib/kybernow-station` rewritten to `…/current` (an update only changes the link) |
| The agent's right to KWin's display protocol | Yes | `~/.local/share/applications/fr.leslignesclaires.kybernow.station.display.desktop` (KWin reads the user's XDG folders, as for the test Station); `Exec` is the agent's real path, rewritten at each version change, because KWin compares it with the program that connects |
| Sound (PipeWire output), virtual displays (Mutter, KWin), certificate, accounts | Yes | Nothing changed |
| `/dev/uinput` (keyboard, mouse, controllers) | Often already open | Steam's udev rules (`steam-devices`, present on SteamOS and Bazzite) already give `uaccess` to `/dev/uinput`. Otherwise, the Station's rule in `/etc/udev/rules.d` and `uinput` in `/etc/modules-load.d`, with sudo, once: `/etc` is writable and kept across updates on both systems. On SteamOS, a password is needed first (`passwd`) |
| GStreamer's PipeWire plugin | Depends on the system | The system's if there is one; otherwise the build machine's, shipped in the archive (`runtime/gst-fallback`, picked up through `GST_PLUGIN_PATH` in a drop-in of Kyber's service). On Fedora Atomic, `rpm-ostree install pipewire-gstreamer` remains possible |
| Starting on its own after a reboot | System setting | Automatic login, as elsewhere |

**Archive**: `server/linux/immutable/make-archive.sh` rearranges the package's tree (or an existing `.deb`, readable from a Mac too) into `KyberNow-Station-Linux-<x86_64|aarch64>.tar.xz`: `install.sh`, `VERSION`, `BUILD` (architecture and glibc of the build machine), `bin/` (with `kybernow-station-home`), `runtime/`, `share/systemd-user`, `share/applications`, `share/udev`. `package.sh` calls it after the `.deb`. The programs need a glibc at least as recent as the build machine's: the published archive is built on Ubuntu 24.04 (glibc 2.39, the oldest of the targeted systems), never on Fedora 45; installation refuses an older system and says so.

**Installation**: `./install.sh` in the extracted folder (`bin/kybernow-station-home install`): checks the architecture and glibc, copies into `versions/<version>`, `current` link, services and `.desktop` written, missing programs reported (`pactl`, `pw-dump`, GStreamer, `kscreen-doctor` on KDE), udev rule only if `/dev/uinput` (or `/dev/uhid`, if the rule names it) is closed, services enabled and started in the graphical session. The first start is the package's (`kybernow-station-setup`). If the package is also installed in the system, installation says so: the services in the user's folder take precedence.

**Update** (no root): Admin downloads `KyberNow-Station-Linux-<arch>.tar.xz`, checks it against the signed manifest, then runs `kybernow-station-home update <archive> <version> --result <…>/resultat.json --log <…>`. The script detaches from Admin (`kybernow-station-update` unit through `systemd-run --user`, a copy of itself outside the version folders), extracts into `versions/`, checks the version, architecture and glibc, stops the services, switches `current`, rewrites the services and `.desktop`, restarts, and waits for Admin to answer in the new version (`/api/session`, 60 s). Otherwise it puts the other version back. Result in the same format as the Mac applier (`version`, `ok`, `rolled_back`, `undo_failed`, `message`, `log`). Two versions kept: “Go back to the previous version” is `kybernow-station-home rollback`. Choosing the path (user's folder or package) is up to the applier, from where Admin runs; on an ostree system with the Station layered in `/usr`, Admin announces the new version without installing it.

**Removal**: `kybernow-station-home uninstall` (settings, accounts and pairings kept; `--purge` to remove them). The udev rule, if it was added, is removed with the command shown.

**SteamOS**: the Station runs in Desktop Mode (KDE Plasma). In Game Mode, gamescope offers the agent neither virtual displays nor capture: the Station doesn't stream there. For it to be there after a reboot, SteamOS must boot into Desktop Mode (`steamos-session-select plasma-persistent`). Same for the Bazzite images that boot into Game Mode (Bazzite Deck); Bazzite's desktop images (KDE, GNOME) behave like Fedora.

**Not tried**: no Bazzite, Fedora Atomic or SteamOS VM or machine. Tried on the Mac with stand-ins for `systemctl` and `curl`: building the archive (from a tree and from a `.deb`), installation, a successful update, an update refused by the health check then the old version put back, an archive of another version refused, going back to the previous version, removal. To check on a real machine: KWin's access control with the `.desktop` in the user's folder, `steam-devices` and `/dev/uinput`, the fallback GStreamer plugin, SteamOS's glibc, `graphical-session.target` in Desktop Mode and Game Mode.

## Linux-specific Kyber patches

Kept in `server/linux/kyber-patches`, applied after the common patches by `server/linux/scripts/build-kyber.sh` (details in [kyber-changes.en.md](kyber-changes.en.md)):

| Patch | What it changes |
|---|---|
| `0001-kyavservice-ecrans-suivis-linux` | GStreamer sources file (`[kyavserver.gstreamer] sources_file`) reread at each listing and each stream, watched to announce changes; identifier and place of each source |
| `0002-kynput-pointeur-wayland` | Absolute pointer through uinput on Wayland, clicks and wheel through it while it holds the pointer |
| `0003-libavgrab-gstreamer-1-24` | Builds with GStreamer 1.24 (Ubuntu 24.04), which lacks `gst_message_get_details` |
| `0004-kymedia-luac-5-4` | A 5.4 `luac` built by Kyber when the system has none (Fedora 45: Lua 5.5), from Lua's sources |

## Building

On a Linux machine of the wanted architecture (the « KyberNow Linux » VM for arm64; x86_64 on a PC or in the same VM through Rosetta):

```sh
# Tools (Ubuntu 24.04; similar names on Debian and Fedora)
sudo apt install build-essential cmake nasm bison flex pkg-config pipx lua5.4 python3-jinja2 \
  glslang-tools spirv-tools libclang-dev libssl-dev libv4l-dev libcap-dev libvdpau-dev libnuma-dev \
  libva-dev libdrm-dev libgbm-dev libegl-dev libvulkan-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpipewire-0.3-dev libpulse-dev libasound2-dev \
  libxcb1-dev libxcb-shm0-dev libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shape0-dev libxtst-dev \
  libxkbcommon-dev libwayland-dev wayland-protocols libevdev-dev libudev-dev libinput-dev
# Fedora 42 and later (no libplacebo-devel: Kyber builds its own)
sudo dnf install gcc gcc-c++ cmake nasm bison flex pipx python3-jinja2 glslang spirv-tools clang-devel \
  openssl-devel libv4l-devel libcap-devel libvdpau-devel numactl-devel libva-devel libdrm-devel \
  mesa-libgbm-devel mesa-libEGL-devel vulkan-loader-devel gstreamer1-devel gstreamer1-plugins-base-devel \
  pipewire-devel pulseaudio-libs-devel alsa-lib-devel libxcb-devel libXtst-devel libXrandr-devel libxkbcommon-devel \
  wayland-devel wayland-protocols-devel libevdev-devel systemd-devel libinput-devel dbus-devel rustup \
  libXrandr-devel clang20-devel
pipx install meson==1.11.2 && pipx inject meson jinja2
cargo install cargo-c --version 0.10.15 --locked --features vendored-openssl && rustup toolchain install 1.89.0

# Kyber sources kept apart (as for the Mac), then Kyber with our patches
KYBER_DIR=<copy of kyber-desktop> server/linux/scripts/build-kyber.sh
# Test Station in the session (systemd user services)
KYBER_DIR=<copy> server/linux/scripts/run-station.sh start
# .deb and .rpm packages
KYBER_DIR=<copy> server/linux/scripts/package.sh
```

Pitfalls met:

- `luac`: txproto needs a 5.4 `luac`. Ubuntu has one (`lua5.4`); Fedora 45 only has Lua 5.5. Linux patch 0004 makes Kyber build its own `luac` from Lua's sources (linked to `liblua.so`, it did not link) when no `luac5.4` is installed.
- Vulkan: without `libXrandr-devel`, the Vulkan loader Kyber builds doesn't configure, and txproto, then built without Vulkan, doesn't build.
- libplacebo: a system `libplacebo-devel` (Fedora 45: 7.360) takes precedence over the one Kyber builds (7.351) and txproto no longer builds; don't install it.
- libclang: libavconv's Rust bindings (bindgen 0.69) misread the headers with libclang 21 or later (Fedora 45: clang 23, “AVConvHandle is defined multiple times”); `build-kyber.sh` takes LLVM 20's, 19's or 18's if installed (`clang20-devel`).
- Position-independent code: Fedora's GCC doesn't produce PIE code by default, unlike Ubuntu's; libvpl's C++ objects left read-only relocations and FFmpeg no longer linked `libavutil.so`. `build-kyber.sh` passes `-fPIC` everywhere.
- OpenSSL 4 (Fedora 45): cargo-c installs with its bundled OpenSSL (`--features vendored-openssl`).
- libplacebo needs jinja2 in meson's Python (`pipx inject`); Kyber's scripts hard-code the `x86_64-linux-gnu` folder (`build-kyber.sh` replaces it with the machine's).

## What the KyberNow Link apps see

Nothing Linux-specific ([link-ui.en.md](link-ui.en.md), “With a Linux Station”): a PC, whose system is the distribution's name. What is not there yet (HDR, PyroWave, pen, lending) is not announced, and the apps behave as with a Station that does not have it.

## Breakdown

1. **Kyber on Linux.** Built with our patches for x86_64 and arm64 (`build-kyber.sh`), without the VLC player. **Done in the arm64 VM (25/11/2026).**
2. **Admin on Linux.** systemd service, Station recognised, restart, encoders: **written**. Wake on LAN (ethtool), home preview, graphics card name, app at session start (`.desktop` files, Steam): **to do**.
3. **Virtual displays.** GNOME through Mutter and KDE Plasma 6 through KWin: **written and tried in the VMs** (see “Tests”).
4. **Sound.** KyberNow PipeWire output: **written**.
5. **Input.** uinput, udev rule, absolute pointer on Wayland: **written**. DualSense controllers through uhid, pen (uinput tablet, following common patch 0027), clipboard on Wayland: to do.
6. **Package, updates.** `.deb`, `.rpm`, services: **written**. Tray icon, update applier (root service), update from Admin: to do.
7. **Advanced features.** Share and relay (nothing system-specific in Admin: to try), PC lending (console mode in the host's session, sleep prevented by `systemd-inhibit`), HDR (GNOME virtual display in BT.2100: GNOME does not offer it yet), PyroWave (Vulkan, DMA-BUF import), live bitrate with VA-API (to add to common patch 0026): to do.
8. **Immutable systems** (Bazzite, Fedora Atomic, SteamOS). Installation in the user's folder, no root except the udev rule when Steam hasn't already set one, password-free updates (`server/linux/immutable`): **written**, tried on the Mac with stand-ins, never on those systems (see “Immutable systems”).

## Tests

Everything in the « KyberNow Linux » Parallels VM (Ubuntu 24.04 arm64, GNOME 46 on Wayland, virgl virtual graphics card, no VA-API or NVENC), see [apps/linux/README.en.md](../apps/linux/README.en.md).

**Tried (25/11/2026)**:

- **Building** Kyber for arm64 with the common patches (0001 to 0027) and the three Linux patches; Admin and the display agent; Admin's tests pass on Linux (322) and on the Mac (326).
- **Test Station** (`run-station.sh start`): Kyber, Admin and the agent start as systemd user services; Admin announces itself through Bonjour (`kybernow-linux`, `kyber=1`); `GET /api/stream/info` gives “Ubuntu 24.04.5 LTS”, the virtual display (up to 4 displays), the sound output, controllers, no HDR, PyroWave or lending. Kyber finds the VM's display through the displays file 250 ms after it starts (patch 0001).
- **Session without an app**, through Admin's API on `localhost` with the test account (`prepare`, heartbeat, `end`) and a GStreamer reader plugged in like Kyber's capture: 1920 × 1080 virtual display created, appeared in GNOME (“Meta-0”), alone on the desktop and primary, the VM's display taken off; sound to the « KyberNow » output in 2 then 6 channels, default output; at the end, the VM's display, layout and original output given back, KyberNow output removed. **Two displays**: 2560 × 1440 at scale 2 at (0, 0) and 1920 × 1080 at (2560, 0), both streams get their frames, same places in Kyber's file.
- **Kyber** on the virtual display (Kyber's RTP stream, in single-client mode for the test): kyavserver opens the “KyberNow” GStreamer source, the x264 encoder and its RTP context; kycontroller then refuses that test stream for lack of a control channel, as expected without an app.
- **Packages**: `.deb` (48 MB) installed with apt: first setup, services started, agent taken from `/usr/lib/kybernow-station`, `/dev/uinput` opened to the user by the udev rule; then uninstalled. `.rpm` built (79 MB), not installed (no Fedora at hand).

Pitfalls found on the way: the app picks its display from Kyber's list as soon as `prepare` answers, before Kyber has reread the displays file (“Unknown GStreamer display id 1”, first session from the Mac): on a Linux Station, `prepare` waits until Kyber lists the created displays (2 s at most, 266 ms measured); `pipewiresrc path=<n>` expects the node's serial number (the first try landed on another node, “not-negotiated”, or on nothing); Admin ends a session without the app's heartbeat (5 s); the previous layout, given back right after a virtual display was removed, was refused (“stale information”): the agent retries with the next state; the VM's GNOME uses the “physical” layout.

**KDE Plasma 6 (25/11/2026)**, in a second Parallels VM « KyberNow Linux KDE » (Fedora 45 KDE arm64, Plasma and KWin 6.8.1, Wayland, automatic login; installed without a window through Anaconda and a kickstart file, ssh with the other VM's key):

- building Kyber, Admin and the agent on Fedora 45 (pitfalls above: Lua 5.5, libclang 23, position-independent code, libplacebo, libXrandr, OpenSSL 4, `lib64`);
- test Station: Kyber, Admin and the agent start, the agent recognises KDE; KWin's access control gives the protocol thanks to the `.desktop` file;
- session through Admin's API (no app): 1920 × 1080 virtual display created by KWin, alone and primary, the VM's display turned off; two displays, 2560 × 1440 at scale 2 at (0, 0) and 1920 × 1080 at (1280, 0) in points; sound to the « KyberNow » output; the virtual display's stream read by GStreamer the way Kyber reads it; at the end, the VM's display comes back and Kyber captures it again;
- first, on a headless KWin 5.27 in the Ubuntu VM: capture and access control; that KWin cannot create virtual displays.

Defects found and fixed on the way: a layout without a virtual display disabled the displays on KDE; a refused virtual display left Kyber without a display; Plasma 6's “Virtual-KyberNow-1” name wasn't recognised; Plasma 6's `kscreen-doctor` output (one property per line) wasn't read; a display turned back on was looked up under its old object.

**Not tried yet**: a session from an app on KDE, an end-to-end session from a KyberNow Link app (picture, sound, latency), keyboard, absolute pointer and clicks on Wayland, controllers, VA-API and NVENC (no real graphics card in the VM), x86_64, Fedora, a real machine.
