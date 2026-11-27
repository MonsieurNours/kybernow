# KyberNow Station for Linux — scope

English · [Français](station-linux.md)

Scoped on 25/11/2026. This document sets the choices, the breakdown and the state of the milestones, like [station-macos.en.md](station-macos.en.md) for the Mac.

## The goal

A computer running Linux becomes a Station like a Windows PC or a Mac: same features, same Admin, and the KyberNow Link apps connect to it without (almost) knowing it is Linux. For gaming (Steam, Proton, Lutris, Heroic) as for remote desktop, with the Game, Desktop and Max profiles.

## Choices (approved by the author on 25/11/2026)

| Topic | Choice |
|---|---|
| Desktops | GNOME and KDE Plasma 6 on Wayland, on par; Sway and Hyprland (wlroots) with their headless outputs (added on 26/11). On X11 and other desktops: the displays as they are, without virtual displays (X11 makes none without a dummy plug or an EDID forced by root) |
| Distributions | Ubuntu 24.04, Debian 13, Fedora 42 and later. Immutable systems (Bazzite, SteamOS) later |
| Packages | A `.deb` and a `.rpm` from the same assembly, with the udev rule and signed updates. No Flatpak (uinput, services and unsandboxed capture do not fit there) |
| Architectures | x86_64 and arm64 |
| Cards and encoders | NVENC (NVIDIA), VA-API (AMD, Intel), x264 as a fallback, picked from the card at installation; frames to the encoder without copies (DMA-BUF); PyroWave in Vulkan |
| Input | uinput (X11 and Wayland), not libei; Xbox controllers through uinput, DualSense through uhid |
| Locking | As on Mac: the session's lock screen is streamed; after a restart, the session must open on its own (automatic login) |
| Services | Kyber, Admin and the tray icon as systemd user services, in the user's graphical session; a separate root service to apply updates |
| Sound | A « KyberNow » PipeWire output with 2, 6 or 8 channels, chosen for the session, then the original output given back (like the Mac's KyberNow output) |
| Engine | Kyber 0.29's Linux server (kycontroller, kyavserver with the libavconv engine, kynputserver), with our common patches and Linux-specific ones. No engine of our own |

## What already exists

- **Kyber can serve from Linux**: capture through NvFBC (its default, NVIDIA only), DRM/KMS, X11, wlroots and GStreamer (libavconv engine); NVENC, VA-API, x264 encoding; keyboard, mouse and controllers through uinput (`kynput/src/linux`). What we add is what makes a Station: Admin, virtual displays, sound, package.
- **Admin already runs outside Windows** (development Mac and Mac Station): system-specific modules sit behind `cfg`s, with a simulated version; the Linux Station adds its real versions, as the Mac did.
- **The apps already read what the Station can do** (`GET /api/stream/info`: `os`, `hdr`, `pyrowave`, `gamepads`, virtual display and sound): a Linux Station describes itself there, and the apps adapt without change ([link-ui.en.md](link-ui.en.md), “With a Linux Station”).

## How it is done

### Displays (GNOME)

GNOME lets no program read the screen directly: everything goes through Mutter, its compositor. The Station has its own agent, `kybernow-display` (`server/linux/display-agent`, Rust), started by Admin as soon as it starts, which speaks the same protocol as the Mac agent (JSON lines: `create`, `remove`, `isolate`, `restore`, `state`, `set_mode`, `displays`…). Admin drives it through `display/linux.rs`, on the same `Backend` as the Mac Station (`display/agent_backend.rs`: requests, timeouts, watch).

- **Virtual displays**: `org.gnome.Mutter.ScreenCast`, `RecordVirtual`, one capture session per display (Mutter does not start a stream added after `Start`), pointer left out of the picture (each device draws its own). Mutter creates the display when the first reader plugs into the PipeWire stream, at the size that reader asks for: Kyber's capture sets the device's resolution. “Platform” display (`is-platform`): GNOME puts its bar and windows on it as on a real display.
- **The computer's displays**: the same API, `RecordMonitor`, without the screen-sharing dialog (the Station is not sandboxed). Captured only when there is no virtual display (sharing from the PC, session without a virtual display).
- **Layout**: `org.gnome.Mutter.DisplayConfig`, `ApplyMonitorsConfig` in temporary mode (never saved). During the session, only the virtual displays stay on the desktop, the main one first and primary, as on Windows and Mac; at scale 2 from 2,048 pixels wide (like the Mac's Retina). Places follow GNOME's layout mode: in pixels with the “physical” layout (`layout-mode` 2), scaled down with the “logical” one. Virtual displays only appear with the first stream: the layout waits for them (an agent thread, every 250 ms). Mutter refuses a negative place: a device display to the left of or above the main one (common on a Mac) shifts the whole layout so it starts at (0, 0); before this fix (26/11), GNOME refused the whole layout and kept the computer's display as primary, with its bar. At the end, the previous layout comes back. Computer lending: the computer's displays stay, on the right.
- **What Kyber reads**: a TOML file of GStreamer sources (`~/.local/share/kybernow/station/kyber-screens.toml`), kept up to date by the agent: display identifier, PipeWire stream (`pipewiresrc target-object=<serial number>`: the node's serial number, read with `pw-dump`, not the identifier Mutter gives, which PipeWire reuses), size, place on the desktop. Kyber rereads it at each display listing, when a stream starts and every 250 ms, and watches it to announce its changes (Linux Kyber patch 0001): a virtual display created, removed or resized appears without restarting Kyber.
- **Identifiers**: `linux:101` for the first virtual display (101, 102… for Kyber), `linux:1`, `linux:2`… for the computer's.
- **Safety net**: virtual displays live with the agent's session-bus connection; if the agent or Admin stops, GNOME removes them and goes back to its saved layout.

### Displays (KDE Plasma)

The agent picks its desktop at startup: GNOME if Mutter is on the session bus, otherwise KDE if KWin gives it its capture protocol (`KYBERNOW_DESKTOP=gnome|kde|sway|hyprland` to force; Sway and Hyprland come first, from `SWAYSOCK`, `HYPRLAND_INSTANCE_SIGNATURE` or `XDG_CURRENT_DESKTOP`). On KDE (`kde.rs`):

- **Virtual displays**: KWin's Wayland protocol `zkde_screencast_unstable_v1` (version 2 and later, Plasma 5.25 and later), `stream_virtual_output`: KWin creates the display “KyberNow-1” (“KyberNow-2”…; Plasma 6 calls it “Virtual-KyberNow-1”) at the given size (in points, with the scale: 2 from 2,048 pixels wide) and its PipeWire stream comes with the `created` event. KWin sets the size at creation: a resolution change during the session remakes the display.
- **The computer's displays**: `stream_output` on their `wl_output` (named by version 4 of `wl_output`), without the portal's dialog.
- **Right to the protocol**: KWin only gives it to a program declared in a `.desktop` file whose `Exec` is the agent and which names the interface (`X-KDE-Wayland-Interfaces=zkde_screencast_unstable_v1`). The package installs `/usr/share/applications/fr.leslignesclaires.kybernow.station.display.desktop`; the test Station, its copy in `~/.local/share/applications`. Without it, the agent says so in Admin's log.
- **Layout**: `kscreen-doctor` (enable, place, scale, priority; displays left out of the layout are disabled); places in points. The list is read from its text output (`-o`, one line per display on Plasma 5.27, one line per property on Plasma 6; a Plasma 6 virtual display says neither “enabled” nor “disabled”): `kscreen-doctor -j` crashes on Plasma 5.27.
- **Display turned off then on**: KWin removes it, then announces it under a new `wl_output` object; the agent follows those removals and retries the capture for a moment at the end of a session.
- **Safety net**: virtual displays live with the agent's Wayland connection; if it stops, KWin removes them.

### Displays (Sway and Hyprland)

wlroots desktops (`wlroots.rs`). No PipeWire, no portal: Kyber captures the compositor's outputs itself (its wayland provider, `wlr-screencopy`), and the displays file names them (`source = "wayland:HEADLESS-1"`, Kyber Linux patch 0013) under the Station's IDs (101, 102…). Kyber's setting does not change (GStreamer): only each display's source says how to capture it.

- **Virtual displays**: headless outputs of the compositor. Sway: `swaymsg create_output` (HEADLESS-n output), mode through `output … mode --custom`, removal through `output … unplug`. Hyprland: `hyprctl output create headless KyberNow-n`, `hyprctl output remove`, mode through `hl.monitor` in Lua (`hyprctl eval`, Hyprland configured in Lua, 0.55 and later, where `keyword` answers "unknown request"; a display's rule is carried over as it is, hence an explicit `disabled=false`), otherwise `hyprctl keyword monitor`. A resolution change during the session changes the output's mode.
- **Computer displays**: listed as they are (`wayland:DP-1`), captured by Kyber without a dialog.
- **Layout**: Sway, `output … enable pos … scale …` then `output … disable` for the others, in one command; Hyprland, a `hyprctl --batch` of the same rules (`hl.monitor` or `keyword monitor`). Places in points; no "primary" display on wlroots: the first one gets the focus.
- **Picture of a still display**: the capture asks for a picture every frame (`copy`, not `copy_with_damage`), otherwise a desktop that does not move sends nothing and the device restarts the stream in a loop; with x264 it uses shared memory (no detour through the graphics card).
- **Safety net**: these outputs belong to the compositor and do not leave with the agent. Their names are kept (`wlroots-ecrans`, next to the displays file): at startup, the agent removes those of an interrupted session and turns the computer's displays back on (`output * enable`, `hyprctl reload`).
- **Startup**: the Station's services follow `graphical-session.target`, which a Sway or Hyprland session only reaches with a launcher that manages it (uwsm, Fedora's sway-systemd); otherwise, in the compositor's configuration, first `exec systemctl --user import-environment WAYLAND_DISPLAY SWAYSOCK HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP` (the agent recognizes the desktop by these variables), then `exec systemctl --user start kybernow-admin kybernow-kyber kybernow-tray`.

### Keyboard, mouse, pointer

kynputserver injects keyboard, mouse and controllers through `/dev/uinput`, on X11 as on Wayland. The package installs a udev rule (`60-kybernow-uinput.rules`, `uaccess`): the user sitting at the computer gets access, without being root. This is a choice (the level of `steam-devices`): any program in the active session can then create a virtual keyboard and type, in a terminal as in a password prompt (STLIN-27). For the same reason, the rule does not open the computer's keyboards and mice (`/dev/input/event*`), see “Lending the computer”.

**Absolute pointer on Wayland** (Linux Kyber patch 0002): on Wayland, no program can place the pointer (XTest only moves Xwayland's). kynput therefore plugs a second uinput device, « KyberNow pointeur », with two absolute axes and a mouse's buttons: udev classes it as an absolute mouse (like a virtual machine's tablet) and the compositor spreads it over its whole desktop. Its axes cover the box around the displays the Station announces (places written by the agent). While it holds the pointer, clicks and wheel go through it (GNOME does not apply another mouse's clicks in that case); a relative move (captured pointer, games) hands back to the relative mouse. On X11, nothing changes (XTest).

### Controllers

Linux Kyber patch 0005. As on Windows and the Mac, a PlayStation controller on the device (Sony: DualShock 3 and 4, DualSense, DualSense Edge) becomes a DualSense on the computer, any other an Xbox 360 controller; Admin's “All controllers as Xbox 360” setting (`gamepad_force_xbox360`) makes them all Xbox.

- **DualSense through uhid** (`kynput/src/linux/uhid_gamepad.rs`): `/dev/uhid` creates a real HID device, a DualSense plugged in over USB (054C:0CE6, descriptor taken from a CFI-ZCT1W, the same as the Mac Station). Linux's `hid-playstation` driver takes it as a real one: controller, sensors and touchpad for evdev, `hidraw` for SDL and Steam; games see a DualSense (PlayStation icons). kynput answers the three reports the driver reads when it opens it (calibration, pairing, firmware), with one MAC address per controller (the driver refuses two controllers with the same address). Rumble comes back in the controller's output reports (0x02) and goes to the device, with no duration, as on the other Stations. The light bar, player LEDs and adaptive triggers do not come back: Kyber's protocol only carries rumble.
- **Xbox 360 through uinput** (already in Kyber, completed): Linux's “endless” rumble (length 0, or 0xFFFF from SDL) sent with no duration; all rumble feedback read at once (the thread read one per wake-up and an effect could wait for the next one); an error on one controller no longer stops all the others.
- **Without uhid** (module missing, permissions): the PlayStation controller becomes an Xbox 360 one, with a warning in kynput's log. The package must open `/dev/uhid` to the user like `/dev/uinput` (`uaccess` udev rule, `uhid` module loaded), and these DualSenses' `hidraw` for SDL and Steam.

kynput's log: “Plug DualSense gamepad 1”, “DualSense 1 started by the kernel”, or “Plug gamepad 1 (device …)” for an Xbox one.

### Pen

Linux Kyber patch 0006, following common patch 0027 ([stylet.en.md](stylet.en.md), “Linux Station”): the Station announces the pen (`PenAvailable`) and receives it as a uinput graphics tablet, « KyberNow stylet » (`BTN_TOOL_PEN`, `BTN_TOOL_RUBBER`, `BTN_TOUCH`, `BTN_STYLUS`, `ABS_PRESSURE`, `ABS_TILT_X/Y`), created at the pen's first packet. udev classes it as a tablet, libinput hands it to the compositor (Wayland: `tablet-v2`, X11: XInput2): pressure, tilt, eraser, button and hover in Krita, GIMP or Inkscape. It covers the box of the announced displays, like the absolute pointer. Rotation does not go through (libinput only reads it on some Wacom pens).

### Clipboard

Linux Kyber patch 0007. On Wayland, a program without a window can neither read nor set the clipboard; Kyber's X11 route (Xwayland) depends on what the compositor copies between X11 and Wayland (KWin only does it while an X11 window has the keyboard). kynput therefore uses the clipboard managers' protocol when the compositor offers it: `ext-data-control-v1` (KDE Plasma 6), otherwise `zwlr-data-control-unstable-v1` (Plasma 5, KWin up to 6.4). Text and HTML, same rights as the other Stations (copy to the device, paste from the device). GNOME offers neither: the Station stays on X11 there, whose clipboard Mutter copies to Wayland and back, and the log says so (“Wayland compositor without data-control (GNOME): clipboard through X11 (Xwayland)”). `KYBERNOW_CLIPBOARD=x11` forces X11 (troubleshooting).

### Sound

Admin (`audio/mod.rs`, `linux` module) creates at the start of the session a « KyberNow » output (PipeWire's `module-null-sink`, through `pactl`) with the device's channel count (2, 6 or 8, PulseAudio order), makes it the default output and moves the playing sounds to it: nothing comes out of the computer's speakers. Kyber captures the default output's monitor. At the end: original output given back, sounds moved back, module removed (and removed at startup if one was left by an interrupted session).

### The device's microphone

[micro.en.md](micro.en.md). The `kybernow-micro` helper (in `runtime/bin`, next to kycontroller; libopus from the runtime) creates if needed, with `pactl`, a null sink `kybernow_micro_entree` and the “Micro KyberNow” source (`kybernow_micro`, `module-remap-source` on its monitor), once for the user's session, then plays the received microphone through `pacat` (pulseaudio-utils, already a dependency). While it is sent, the source is the default microphone (`pactl set-default-source`); the previous one comes back at the end. Built (`cargo check --target aarch64-unknown-linux-gnu`); not tried in the VM yet.

### Encoder

Installation picks the card's (`kybernow-station-setup`): NVIDIA with its proprietary driver (`/proc/driver/nvidia/version` or `libnvidia-encode.so.1`) → NVENC; AMD, Intel, or NVIDIA on nouveau or NVK (no NVENC), with a render node → VA-API; otherwise x264 (tests: `server/linux/scripts/test-setup.sh`). Admin offers these three in Settings › Picture (“VA-API — AMD or Intel card”).

### Admin's texts

Admin gives `platform`: `linux` (`GET /api/session`): the interface and Admin replace each Windows-specific text (Start menu, SudoVDA driver, Steam speakers, Windows session…) with its Linux version, stored under `linux` in the catalogues (`server/web/src/lib/i18n/`, `server/admin/src/i18n/`), and Kyber's settings texts through `schema.linux.json` ([i18n.en.md](i18n.en.md)). Found at the first session from the Mac (25/11/2026): Admin talked about Windows.

### Services and package

- `kybernow-admin.service` and `kybernow-kyber.service`, systemd user services (`/usr/lib/systemd/user`), tied to the graphical session: they start at login and stop with it. Admin drives Kyber through `systemctl --user` (`linux.rs`), and restarts the Station through systemd.
- First start: `kybernow-station-setup` (before Admin) writes Kyber's Linux settings (libavconv engine, GStreamer capture, displays file), a certificate of the Station's own, the card's encoder, the first account (your user name, random password in `~/.local/share/kybernow/station/admin/mot-de-passe`).
- Data: `~/.local/share/kybernow/station/` (`admin/`, `kyber/`, `kyber-screens.toml`).
- Package: `server/linux/scripts/package.sh` → `kybernow-station_<version>_<arch>.deb` and `kybernow-station-<version>.<arch>.rpm` (`/usr/lib/kybernow-station`, user services including the tray icon, root sockets and services for updates and for the input during a loan, udev rule). At installation, the services are enabled for every user (`systemctl --global enable`), except system accounts (`ConditionUser=!@system`: login manager, services); only one session at a time carries the Station (two accounts logged in at once would fight over its ports), the update and input sockets are opened and the udev rule is loaded; a package update keeps all that (the scripts only disable on removal). Tray icon dependencies: GTK 3, AppIndicator (Ayatana or the older one) and WebKitGTK 4.1 (lending screen); recommended: zenity or kdialog, notify-send. Desktop-specific helpers (GNOME's AppIndicator extension, KDE's `kscreen-doctor`) only come on that desktop: the .rpm recommends them conditionally (`if gnome-shell`, `if plasma-workspace`), the .deb suggests them (Debian has no conditions); otherwise the extension, which depends on GNOME Shell, installed all of GNOME and GDM on a KDE Fedora. Admin dependencies: ethtool, GStreamer "good" plugins, hwdata; recommended: iw, xprop, rsvg-convert. Firewall, as on Windows: the package declares the firewalld service `kybernow-station` (8080 TCP and UDP for the stream, 9443 TCP for Admin, 5353 UDP for Bonjour) and the ufw profile "KyberNow Station"; at installation, if firewalld or ufw is running, they are opened (firewalld's default zone), and closed again on removal, which also stops the Station in open sessions (`systemctl --user -M user@`). Without this, Fedora's default zone let the stream and Admin through but not Bonjour: the Station did not appear in the apps. `uinput` and `uhid` (DualSense) loaded at boot (`modules-load.d`), opened to the user by the udev rule along with the DualSense hidraw. The same assembly also gives the immutable systems archive (`server/linux/immutable/make-archive.sh`). Publishing: `server/linux/scripts/release-station.sh` builds the four packages under their fixed names (arm64 in the VM; x86_64 on an x86_64 machine reached by ssh, otherwise in the same VM, in an Ubuntu 24.04 x86_64 through Rosetta: `server/linux/scripts/x86-chroot.sh setup` prepares it once, two-stage debootstrap, the "Build" packages, Rust, Kyber sources copied apart; before each build it checks that Rosetta really runs an x86_64 program, and otherwise waits for the `kybernow-rosetta` service to put it back: an `apt install`, like pi-gen's just before, restarts `systemd-binfmt`, which can wipe it for a moment); `scripts/release.sh` publishes them in every release (`KYBERNOW_STATION_LINUX=0` to leave them out) (see [releases.en.md](releases.en.md)).

### Updates and tray icon

- **Updates** (milestone 6), as on Windows and on the Mac: same minisign-signed manifest, same checks by Admin (signature by the built-in key, version, channel, size and hash of the package), same Updates page and same buttons in the apps. The package is the one for the installed architecture and format: `KyberNow-Station-Linux-{x86_64,arm64}.{deb,rpm}` (`update/linux.rs`, format read from dpkg, otherwise rpm).
- Admin is never root. It downloads and checks in `~/.local/share/kybernow/station/admin/update/<version>/`, then drops its request on the `/run/kybernow-station/update.sock` socket (`kybernow-station-update.socket`, 0660, group `kybernow-station`). Since 2026-11-27 (STLIN-02), only the Station's owner reaches it: the package creates the group (`sysusers.d`), adds the installing account (`sudo`, `pkexec`) and records it as the socket's owner (`/etc/systemd/system/kybernow-station-update.socket.d/owner.conf`, `SocketUser`), which works at once, without logging in again; another local account can no longer ask for a rollback or a beta. Installed another way (without `sudo` or `pkexec`): `sudo usermod -aG kybernow-station YOUR_ACCOUNT`, then log in again. Purging removes `/var/lib/kybernow-station` and the recorded owner (the group stays). systemd then starts, for that connection, the root service `kybernow-station-update@.service` (`kyber-admin update serve`), which trusts nothing in the request: it reads the account making it (`SO_PEERCRED`), only takes files from that account's updates folder, without following links, copies them into its own folder `/var/lib/kybernow-station/update/<version>/` and checks everything again there (signature, version newer than the installed package, channel followed by that account, `min_installer_version`, size and hash). It answers Admin once the request is checked, before stopping anything: a refusal shows in Admin as elsewhere.
- Then it stops that account's tray icon, Kyber and Admin (`runuser … systemctl --user`), installs the package (`apt-get install` for a .deb, which pulls new dependencies, otherwise `dpkg -i`; `dnf install` for an .rpm, otherwise `rpm -Uvh`), restarts the services and waits for Admin to answer in the new version (60 s, at the address of its `listen` setting).
- **Rollback.** Before installing, Admin also downloads the package of the running version (from its published release, checked against its signed manifest) unless the root service already keeps it; the root service checks and keeps it. If Admin doesn't answer in the new version, it reinstalls that one (`--allow-downgrades`, `rpm --oldpackage`). "Go back to the previous version" reinstalls the kept version (`previous`) the same way, checked again; only the two useful versions stay in `/var/lib/kybernow-station/update/`. A Station installed by hand from a release that did not publish Linux packages yet (0.11.0 and earlier) has no rollback for its first update: the log says so.
- Result in `/var/lib/kybernow-station/update/resultat.json` (readable by everyone, same format), which Admin reads at startup; log in `<version>/update.log` next to it, and `journalctl -u 'kybernow-station-update@*'`.
- **Immutable systems.** An Admin installed in the user's folder (`~/.local/lib/kybernow-station/versions/<version>/bin`, see "Immutable systems") takes the `KyberNow-Station-Linux-{x86_64,aarch64}.tar.xz` archive, checked the same way, and hands it to `kybernow-station-home update <archive> <version> --result … --log …`, next to it: no root service; result in its own `update/` folder. "Go back to the previous version": `kybernow-station-home rollback`, to the `previous` link. On an ostree system (`/run/ostree-booted`) where the Station is layered under `/usr` (rpm-ostree), Admin announces the version without installing it: "installs with rpm-ostree".
- **Tray icon by the clock**: the same program as on Windows (`server/tray`, tao and tray-icon), built for Linux (`src/linux.rs`), as the systemd user service `kybernow-tray.service`. An AppIndicator icon (StatusNotifierItem): KDE Plasma shows it by itself, GNOME with the AppIndicator extension (built in on Ubuntu; `gnome-shell-extension-appindicator` elsewhere, recommended by the .rpm and suggested by the .deb). Same menus and texts (`tray.*`), same state and same badge (`src/status.rs`, shared with Windows); a click always opens the menu, and the pages of the Windows panel open in Admin (sharing, lending, updates). No "Start with the computer": the services start with the session. "Reset Admin's password" needs no special rights (Admin runs with your account) and shows the setup code. Confirmations and messages through zenity or kdialog; a new request (device, guest, friend) gives a notification.
- `kybernow-station-setup` writes `~/.local/share/kybernow/station/kybernow.json` (Admin's port), where Admin notes its certificate's fingerprint: the icon only talks to that certificate, as on Windows.

### Admin: wake, preview, graphics card, apps

Admin lives in the user's session: it reads everything itself, with no agent and no root rights (`server/admin/src/linux_*.rs`).

- **Wake on LAN** (`linux_net.rs`): the cards of `/sys/class/net` backed by a real device (`device`), Ethernet before Wi-Fi, the card of the default route (`/proc/net/route`) first; no bridges, no container, virtual machine or VPN cards, no Bluetooth. Hardware address: the card's own, or the permanent address from `ethtool -P` when NetworkManager picked a random one. IPv4 addresses from `ip -4 -o addr`. "Wake allowed": `Wake-on` from `ethtool <card>` contains `g` (magic packet); no if the kernel forbids the card to wake the computer (`device/power/wakeup`); unknown without `ethtool` or for Wi-Fi. Link speed (PyroWave in Auto): `speed` for Ethernet, `iw dev <card> link` for Wi-Fi. Name: the chip's name in the PCI database, then the interface ("Intel Ethernet Controller I225-V (enp3s0)").
- **Home preview** (`linux_screen.rs`): outside a session, the displays agent already holds one PipeWire stream per computer display (written to `kyber-screens.toml`). Admin reads **one frame** of the display placed at (0, 0) with GStreamer (`gst-launch-1.0 pipewiresrc … num-buffers=1 ! videoscale ! jpegenc`), scaled down to the requested width: no screen-sharing dialog, nothing to install beyond what Kyber needs. Kept 10 s, and never taken during a session, as on Windows.
- **Graphics card** (`linux_gpu.rs`): `/sys/class/drm/card<N>` (NVIDIA, AMD, Intel, in that order, as on Windows), marketing name from the PCI database (`pci.ids` from `hwdata` or `pciutils`: "AD102 [GeForce RTX 4090]" → "NVIDIA GeForce RTX 4090"), driver as its version (`/sys/module/nvidia/version`) or "amdgpu (Linux 6.8.0)". Monitoring: NVML for NVIDIA; `amdgpu` load, memory, temperature and power from `/sys`; nothing for Intel (it needs rights the Station doesn't have). A virtual machine card (virtio) isn't shown.
- **App at session start** (`linux_apps.rs`): the catalogue is made of the session's `.desktop` files (`XDG_DATA_HOME`, then `XDG_DATA_DIRS`: packages, Flatpak, Snap), like the desktop's menu: without hidden apps, apps meant for another desktop, or terminal apps; name in the session's language; icon looked up in the icon themes (`hicolor` first) and `pixmaps`, as PNG, an SVG icon converted by `rsvg-convert` when present. Suggestions: Steam Big Picture (Steam as a package or Flatpak), Lutris, Heroic. "Other…": your applications (`~/Applications`), the Desktop and `/opt` (`.desktop` files, AppImages, scripts). Each app runs in **its own systemd user service** (`systemd-run --user`, `kybernow-app-<hash>`): restarting Admin doesn't close it, and closing it at the end of the session stops everything it started; Big Picture is closed with `steam://close/bigpicture`. Wayland doesn't let a window be brought to the front: an app already open stays as it is. Locked session (`LockedHint` from `loginctl`): the app waits for the unlock, 2 minutes at most.
- **Auto mouse** (`linux_screen.rs`): Wayland doesn't tell which window has focus, but Steam games (Proton and most native games) are X11 windows served by Xwayland, for which GNOME and KDE remain the window manager: the active X11 window, its full-screen state and its program are read with `xprop` (package `x11-utils` or `xprop`). A full-screen `steam_app_…` window is a game; browsers and video players aren't. A game in a native Wayland window isn't seen: the mouse stays the desktop one.

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
### Lending the computer

Console mode in the host's session, as on Mac ([pret.en.md](pret.en.md), “Linux Station”): the `kybernow-tray` icon, started by Admin with `--console` in its own systemd unit, puts the friend's launcher (the allowed apps, nothing else of the desktop) full-screen on the virtual display and a cover on each of the computer's displays, takes the physical input (`EVIOCGRAB`) and turns off the desktop's shortcuts (GNOME through `gsettings`, KDE through KGlobalAccel). Admin (`lend_sys.rs`) follows the session through `loginctl`, prevents sleep and locking while lending is on (`systemd-inhibit`, and `gnome-session-inhibit` on GNOME), watches processes through `/proc` and closes what isn't allowed. `lend.available` is true as soon as the agent is next to Admin. **The computer's keyboard and mouse (STLIN-05)**: the package does not open the computer's keyboards and mice to the user (`/dev/input/event*`): otherwise any program in the session could read what is typed. A small root helper takes them (`EVIOCGRAB`) during a loan: `kyber-admin input serve`, started by systemd for each connection on `/run/kybernow-station/input.sock` (`kybernow-station-input.socket`, `Accept=yes`, like updates). Only the Station's account reaches it: the socket is reserved to the owner recorded at installation (`/etc/systemd/system/kybernow-station-input.socket.d/owner.conf`) and to the `kybernow-station` group, and the helper checks the account at the other end (`SO_PEERCRED`) against the owner recorded by updates. It keeps the descriptors: the console agent only receives Enter and Escape (the cover), never the rest of what is typed; uinput and uhid devices (Kyber, the friend) are never taken, nor the power buttons and the lid. The grab lasts as long as the connection: end of the loan (`release`), agent stopped or crashed, session closed, or agent silent for more than 20 s (it sends `ping` every 5 s): the helper stops and the kernel hands the input back to the desktop. Admin plays no part: the input is handed back even if it no longer answers. Hardened service (`DevicePolicy=closed`, `DeviceAllow=char-input r`, no capabilities nor network, read-only system); log: `journalctl -u 'kybernow-station-input@*'`. A keyboard plugged in during the loan is taken within 2 s. Without the helper (installed in the user's folder, on an immutable system: a root service never starts a program the user can replace), the agent takes the input itself if it is allowed to (account in the `input` group); otherwise **the computer's keyboard and mouse are not blocked** and Admin says so before you grant a loan (`input_locked` false and `input_warning` in the loan view, a line in the feed when the loan starts). Admin knows by asking the helper how many devices it could take (`probe`, read again every 30 s at most).

### PyroWave

A separate encoder, in Vulkan (the PyroWave library is Vulkan), on the PipeWire capture's DMA-BUF frames imported without a copy; details in [pyrowave.en.md](pyrowave.en.md), “The Linux Station”.

- `server/linux/scripts/pyrowave-linux.sh` (run by `build-kyber.sh` before FFmpeg) builds `libpyrowave-shared.so` and `pyrowave-device-validation` into Kyber's rootfs; Kyber Linux patches 0010 (FFmpeg encoder `libpyrowave_vulkan`), 0011 (kyavserver, libavconv engine) and 0012 (kycontroller offers it).
- Admin (`pyrowave.rs`) checks once per launch, in the background, the render node's card (`/dev/dri/renderD128`, `KYBERNOW_RENDER_NODE` for another): `pyrowave-device-validation --external --roundtrip --vid … --pid …` (suitable Vulkan 1.3, import of DMA-BUFs and sync files, round trip). `pyrowave.available` is true when it succeeds, otherwise `reason: "vulkan"`.
- A capture without DMA-BUF (virtual machine) goes through the CPU for the YCbCr conversion: PyroWave works, more slowly.

### Live bitrate

Common patch 0026 also covers VA-API (AMD and Intel cards): when the app changes the bitrate (`/set_bitrate`), the encoder's rate control parameters (bitrate, HRD buffer) follow and go with the next frame, without a keyframe; before, they were only sent with IDR frames (every 10 s with the 600-frame GOP). NVENC and VideoToolbox don't change ([debit-automatique.en.md](debit-automatique.en.md)). x264 already follows a bitrate change by itself.

### HDR

The question: can GNOME or KDE give a program a virtual display in BT.2100 (PQ, BT.2020 colours), like the Windows Station's HDR virtual display? **Not yet**, neither of them, so the Linux Station doesn't announce HDR (`hdr` false: apps stay in SDR). Study of 25/11/2026, from both compositors' public interfaces and developers' reports; to confirm in the VMs:

- **GNOME (Mutter 46 to 49)**: HDR exists for physical displays that announce it (EDID), set in Settings › Displays. A virtual display (`org.gnome.Mutter.ScreenCast`, `RecordVirtual`) has no colour property nor EDID: it stays sRGB, and its PipeWire stream only offers 8-bit formats, without colorimetry. It would take a modified Mutter (virtual display with a colour mode, 10-bit stream announced as BT.2100): out of our reach until Mutter does it itself.
- **KDE Plasma 6 (KWin)**: HDR and per-display colour management since Plasma 6.0, for displays that announce it. The `zkde_screencast_unstable_v1` virtual display (`stream_virtual_output`) has no such capabilities (nothing to turn on with `kscreen-doctor output.Virtual-KyberNow-1.hdr.enable`), and KWin's screencast plugin only offers 8-bit DRM formats: even an HDR rendering would be brought back to SDR there.
- **On Kyber's side too**: libavgrab's GStreamer capture can't read 10-bit frames (10-bit RGB and P010 aren't in its format table) nor HDR metadata; it would need the Linux equivalent of common patch 0022 (P010 to HEVC Main 10 with VA-API or NVENC, metadata).
- **Detour ruled out**: capturing a physical HDR display through KMS (kmsgrab) needs root rights (`CAP_SYS_ADMIN`) and makes no virtual display at the device's size.

To redo when Mutter or KWin give a virtual display with a colour description (the `wp_color_management_v1` Wayland protocol on the compositor's side, or a `RecordVirtual` property) and 10-bit PipeWire streams. To check in the VMs meanwhile: `kscreen-doctor -o` (HDR and WCG lines of `Virtual-KyberNow-1`) and the formats of the virtual display's PipeWire node (`pw-dump`).

## Linux-specific Kyber patches

Kept in `server/linux/kyber-patches`, applied after the common patches by `server/linux/scripts/build-kyber.sh` (details in [kyber-changes.en.md](kyber-changes.en.md)):

| Patch | What it changes |
|---|---|
| `0001-kyavservice-ecrans-suivis-linux` | GStreamer sources file (`[kyavserver.gstreamer] sources_file`) reread at each listing and each stream, watched to announce changes; identifier and place of each source |
| `0002-kynput-pointeur-wayland` | Absolute pointer through uinput on Wayland, clicks and wheel through it while it holds the pointer |
| `0003-libavgrab-gstreamer-1-24` | Builds with GStreamer 1.24 (Ubuntu 24.04), which lacks `gst_message_get_details` |
| `0004-kymedia-luac-5-4` | A 5.4 `luac` built by Kyber when the system has none (Fedora 45: Lua 5.5), from Lua's sources |
| `0005-kynput-manettes-linux` | The device's PlayStation controller as a DualSense through uhid (rumble sent back), the others as Xbox 360 through uinput; “All controllers as Xbox 360” honoured; endless Xbox rumble, read at once |
| `0006-kynput-stylet-linux` | Pen as a uinput graphics tablet (pressure, tilt, eraser, button, hover); the Station announces it (`PenAvailable`) |
| `0007-kynput-presse-papiers-wayland` | Clipboard through `ext-data-control-v1` (or `zwlr-data-control`) on Wayland when the compositor offers it (KDE); otherwise X11 through Xwayland (GNOME) |
| `0010-kymedia-ffmpeg-pyrowave-vulkan` | FFmpeg encoder `libpyrowave_vulkan`: the capture's DMA-BUF frames imported in Vulkan, PyroWave on the card (see “PyroWave”) |
| `0011-kyavservice-pyrowave-linux` | kyavserver accepts PyroWave on Linux, with this encoder |
| `0012-kycontroller-pyrowave-linux` | kycontroller offers PyroWave on a Linux Station |
| `0013-kyavservice-ecrans-wayland` | Display `source = "wayland:NAME"` captured by libavgrab (wlr-screencopy) by its name, a picture every frame even when still, shared memory with x264 (Sway, Hyprland) |

## Building

On a Linux machine of the wanted architecture (the « KyberNow Linux » VM for arm64; x86_64 on a PC or in the same VM through Rosetta):

```sh
# Tools (Ubuntu 24.04; similar names on Debian and Fedora)
sudo apt install build-essential cmake nasm bison flex pkg-config pipx lua5.4 python3-jinja2 \
  glslang-tools spirv-tools libclang-dev libssl-dev libv4l-dev libcap-dev libvdpau-dev libnuma-dev \
  libva-dev libdrm-dev libgbm-dev libegl-dev libvulkan-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpipewire-0.3-dev libpulse-dev libasound2-dev \
  libxcb1-dev libxcb-shm0-dev libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shape0-dev libxtst-dev \
  libxkbcommon-dev libwayland-dev wayland-protocols libevdev-dev libudev-dev libinput-dev libgtk-3-dev libwebkit2gtk-4.1-dev
# Fedora 42 and later (no libplacebo-devel: Kyber builds its own)
sudo dnf install gcc gcc-c++ cmake nasm bison flex pipx python3-jinja2 glslang spirv-tools clang-devel \
  openssl-devel libv4l-devel libcap-devel libvdpau-devel numactl-devel libva-devel libdrm-devel \
  mesa-libgbm-devel mesa-libEGL-devel vulkan-loader-devel gstreamer1-devel gstreamer1-plugins-base-devel \
  pipewire-devel pulseaudio-libs-devel alsa-lib-devel libxcb-devel libXtst-devel libXrandr-devel libxkbcommon-devel \
  wayland-devel wayland-protocols-devel libevdev-devel systemd-devel libinput-devel dbus-devel rustup gtk3-devel webkit2gtk4.1-devel \
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

Nothing Linux-specific ([link-ui.en.md](link-ui.en.md), “With a Linux Station”): a PC, whose system is the distribution's name. What is not there yet (HDR) is not announced, and the apps behave as with a Station that does not have it; PyroWave and lending are announced when the graphics card is suitable and when the console mode agent is there.

## Breakdown

1. **Kyber on Linux.** Built with our patches for x86_64 and arm64 (`build-kyber.sh`), without the VLC player. **Done in the arm64 VM (25/11/2026).**
2. **Admin on Linux.** systemd service, Station recognised, restart, encoders: **written**. Wake on LAN (`/sys`, `ethtool`), home preview (one frame of the display's PipeWire stream), graphics card (`/sys/class/drm`, PCI database, NVML, `amdgpu`), app at session start (`.desktop` files, systemd services, Steam Big Picture), Auto mouse for X11 games (Xwayland): **written and tested on the Mac** (source parsing), not yet tried in a VM or on a real machine.
3. **Virtual displays.** GNOME through Mutter and KDE Plasma 6 through KWin: **written and tried in the VMs** (see “Tests”).
4. **Sound.** KyberNow PipeWire output: **written**.
5. **Input.** uinput, udev rule, absolute pointer on Wayland: **written**. DualSense controllers through uhid, pen (uinput tablet, following common patch 0027), clipboard on Wayland (data-control): **written** (patches 0005 to 0007, built for Linux and unit tests on the Mac on 25/11/2026), not tried on Linux yet. udev rule for `/dev/uhid` and `uhid` module loaded at boot: in the package (updates branch).
6. **Package, updates.** `.deb`, `.rpm`, services: **written**. Updates from Admin and from the apps (root service through a socket, rollback), tray icon by the clock, published packages: **written**, tested on the Mac (Admin and tray tests), not tried on Linux yet (see "Tests").
7. **Advanced features.** Share and relay (nothing system-specific in Admin: to try). PC lending (console mode in the host's session, sleep prevented), PyroWave (Vulkan, DMA-BUF import, Linux patches 0010 to 0012), live bitrate with VA-API (common patch 0026): **written** (25/11/2026), not yet built nor tried on Linux. HDR: **not possible yet** (neither GNOME nor KDE give a BT.2100 virtual display, see “HDR”).
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

**Linux Station follow-up (26/11/2026)**, in both VMs, from `main` (Admin, updates, tray icon, inputs, lending, PyroWave, immutable systems):

- **Build** on Ubuntu 24.04 and Fedora 45: Kyber with every patch (common, Linux 0001 to 0012, PyroWave Vulkan), Admin, the display agent and the tray icon; 352 Admin tests and 30 tray tests on Linux; `pyrowave-device-validation` answers on Fedora.
- **Packages**: `.deb`, `.rpm` and the immutable systems archive built on both. `.rpm` installed by dnf on Fedora KDE, `.deb` by apt on Ubuntu GNOME: first initialization, Admin, Kyber and the tray icon start as user services; session through the API (1920 × 1080 virtual screen alone and primary, sound to "KyberNow" in 2 and 6 channels, stream read the way Kyber reads it, everything restored at the end).
- **x86_64**: Kyber, Admin, the agent and the tray icon built in an Ubuntu 24.04 x86_64 through Rosetta (arm64 VM, `x86-chroot.sh`); `.deb`, `.rpm` and archive built, x86-64 programs that start (Admin 0.11.2, Kyber 0.29.0), no missing library. Not tried on a real x86_64 PC.
- **Sway** (1.9, headless, next to GNOME in the Ubuntu VM): two-display session through the API (HEADLESS outputs created, placed, computer display off, everything restored at the end); session from the Mac app with a picture, still desktop included (after the `copy` fix). Hyprland 0.57.2 (built from source, nested in the Fedora VM's KDE): two-display session through the API, with the `.rpm` (displays created, placed, computer display off then back on); session from the Mac app with a picture (26/11).
- **Session from the Mac app** to the Station installed by the `.rpm` on Fedora KDE: it works, keyboard and mouse included (tried by the author); very choppy picture, expected in the VM (x264 on the processor, no graphics card nor DMA-BUF).
- Defects found and fixed: Fedora's firewall hid the Station from the apps (Bonjour closed); the PyroWave Vulkan encoder did not compile (`F_DUPFD_CLOEXEC` hidden by FFmpeg's options); `pyrowave-device-validation` kept the build folder in its search path (refused by rpmbuild); `package.sh` stopped without `lib64` (Ubuntu); the tray icon links WebKitGTK (lending screen) without the packages asking for it; the recommended AppIndicator extension installed all of GNOME and GDM on a KDE Fedora; on the first start of a fresh install, `kybernow-station-setup` stopped before Admin (settings file not written yet); eight Admin tests assumed "not Windows = Mac"; outside a Station, lending was refused on Linux instead of simulated as on the Mac.

**Not tried yet**: updating from Admin (socket, root service, apt and dnf, health check, rollback), looking at the tray icon on screen (its service runs on GNOME and KDE), lending with its console agent; an end-to-end session from a KyberNow Link app (picture, sound, latency), controllers (Xbox and DualSense), the pen, the clipboard on Wayland, VA-API and NVENC (no real graphics card in the VM), x86_64, a real machine.
