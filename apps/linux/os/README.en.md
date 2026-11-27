# KyberNow Link OS: the image to flash on a Raspberry Pi

English · [Français](README.md)

A microSD card (or SSD) that boots nothing but KyberNow Link: the Raspberry Pi becomes a streaming console (on a TV, with a controller) or a remote desktop (screen, keyboard, mouse). The decision is in [PROJECT.en.md](../../../PROJECT.en.md) ("Raspberry Pi client"); mock-ups: https://claude.ai/artifact/6crVeg3QyrA5vs8TRhWRft ("KyberNow Link OS" row). Installation: [../INSTALLATION.en.md](../INSTALLATION.en.md) ("KyberNow Link OS").

**Status**: the image builds in the "KyberNow Linux" VM and boots under QEMU emulation (below); **nothing has booted on a Raspberry Pi yet** (no board available): the firmware and A/B tryboot, Plymouth, the V4L2 decoder, ALSA sound, Bluetooth and CEC are not verified on hardware (see "Not yet verified on a board").

## What the image does

- **Base**: Raspberry Pi OS Lite (Debian 13 "trixie", arm64), built by [pi-gen](https://github.com/RPi-Distro/pi-gen) (stage0 to stage2) then our stage `stage-kybernow`; no desktop, no interactive session (`kybernow` account without a password; SSH off, key-only: see "SSH" below).
- **Link alone, full screen**: `kybernow-link.service` runs the app (the .deb from `scripts/deb.sh`, Raspberry Pi's FFmpeg, HEVC V4L2 decoder) on **KMS/DRM** (`SDL_VIDEODRIVER=kmsdrm`, no compositor), sound through **ALSA** on the HDMI socket (`/etc/asound.conf`), `KYBERNOW_OS=1`, data in `/var/lib/kybernow` (its own partition). It restarts if it stops.
- **PyroWave** (Vulkan, Mesa v3dv): `libpyrowave-shared.so.1` comes with the .deb, and `00-packages` adds `libvulkan1`, `mesa-vulkan-drivers` and `vulkan-tools` (missing from Lite). The app offers it on the Pi 5 when the probe succeeds (Pi 4 and Pi 3: unavailable); steps on a board in [docs/pyrowave-client-pi.en.md](../../../docs/pyrowave-client-pi.en.md). The app requests it from Kyber by default (`deb.sh`, `build.sh`; `--sans-pyrowave` to do without).
- **"Console" boot**: firmware without the rainbow (`disable_splash=1`), quiet kernel, **Plymouth** draws the still of the unlit mark on ink (`mark.png`, rendered by `kybernow --splash`: the first frame of the ignition, pixel for pixel), then the service asks it to stay on screen (`plymouth quit --retain-splash`) while the app takes over the same picture and plays the brand's ignition **with sound** (`src/device/chime.rs`: crackles, pad, A major 9 chord, chime, resolution, synthesised by the app at launch). When there is something to wait for (network, Bluetooth), the seam fills under the name.
- **First boot** (`src/ui/setup.rs`, the mock-up's screens): **Controller** (Bluetooth listening through `bluetoothctl`, no-code agent: only controllers are paired, trusted and connected, recognised by their Bluetooth class (BlueZ's `input-gaming` icon, gamepad class or appearance, read through `bluetoothctl info`) or by their name ("Controller", DualSense, DualShock, Joy-Con, 8BitDo…); a headset or a phone passing by is left alone; USB, CEC remote or keyboard work too; bilingual screen, English then French), **Language**, **Network** (Ethernet recognised, otherwise the Wi-Fi list through `nmcli` and the password on the keypad), **Use** (Living room or Desktop: Link's interface), **Computer** (adding by code, the usual screen).
- **Device settings** (`src/ui/device.rs`, ☰ on the home screen): Use, Network (Ethernet and its speed, or Wi-Fi and its band, read through `nmcli`), Controllers (pair, test), Screen (HDMI mode at next boot, HDMI-CEC), Sound, **Update**, Language, Advanced (SSH, restart, shut down, version). Outside a session, ☰ **Start**: a short press opens the settings (on the home screen, on release); **held three seconds**, it switches Living room ⇄ Desktop without opening them, like Steam and Big Picture; in a session, Start goes to the game; the Guide (or PS) button on the home screen puts the device to standby (⏻ too): the app shows only ink (the HDMI output stays on), Bluetooth stays on, the TV is put to standby through CEC; a key or a button wakes it, and the TV is turned back on through CEC on this input (CEC on).
- **HDMI-CEC** (`cec-ctl`, v4l-utils, through `kybernow-os-ctl`): at launch the device announces itself as "KyberNow Link" to the TV (playback device, `--playback`); standby (`--standby`) and wake (`--image-view-on`, `--active-source`) go through CEC; the remote drives the interface (CEC keys reach the kernel as a keyboard: arrows, OK, back). Screen › HDMI-CEC turns all of it off: no announcement at launch, the CEC address given back (`cec-ctl --clear`, the remote no longer talks to the device), the device's standby and wake without any message to the TV. Turned back on, it announces the name right away.
- **SSH**: off by default, key-only (`/etc/ssh/sshd_config.d/kybernow.conf`: no password, no root). Authorised keys and the host key live on the data partition (`/var/lib/kybernow/ssh/kybernow/authorized_keys`, `/var/lib/kybernow/ssh/ssh_host_ed25519_key`): A/B updates do not touch them, the device keeps its identity from one version to the next. To add a key without SSH: from a computer, drop your public key (`id_ed25519.pub` renamed **`kybernow-ssh.pub`**, or an `authorized_keys` file) at the root of the card's boot partition (`BOOTA`), then boot the Pi: `kybernow-os-ssh.service` (`kybernow-os-ctl ssh-keys`) adds the keys, deletes the file and turns SSH on. Then `ssh kybernow@<the Pi's address>`. Device settings › Advanced › SSH turns it off or back on; the keys stay.
- **A/B update**: see below.

## Card layout (A/B)

`make-ab-image.sh` rebuilds pi-gen's image (two partitions) into six:

| Partition | Role |
|---|---|
| p1 `BOOTA` (FAT32, 256 MB) | boot A, and `autoboot.txt` (the firmware always reads it from partition 1) |
| p2 `rootA` (ext4) | system A (the image's): 3,200 MB, fixed size (a heavier version always fits in the slot; two fit on an 8 GB card) |
| p3 `BOOTB` (FAT32, 256 MB) | boot B (empty: the first update writes it) |
| p4 | extended |
| p5 `rootB` (ext4) | system B (empty) |
| p6 `data` (ext4, 256 MB) | `/var/lib/kybernow`: computers, settings, log, update images; **grown at first boot** to the end of the card (`kybernow-os-firstboot`) |

The disk identifier is fixed (`0x4b79624e`), so the PARTUUIDs are too (`4b79624e-01` … `-06`), written into `cmdline.txt` and `/etc/fstab`; `/etc/kybernow-os/layout` states the layout, `/etc/kybernow-os/release` the version.

**autoboot.txt**: `[all] tryboot_a_b=1 boot_partition=1` / `[tryboot] boot_partition=3`. The update (`kybernow-os-update`, root through sudo, with only the arguments `status`, `apply`, `commit`, `rollback`: `etc/sudoers-kybernow`, which also pins those of `kybernow-os-ctl`): the app downloads the published release's `KyberNow-Link-OS.img.xz` (checked by the core: signed manifest, size, SHA-256) with `kybernow-release.json` and its signature into `/var/lib/kybernow/update`, then `apply` (no path) redoes every check **as root**, trusting nothing in that folder: the manifest and its signature are copied into root's state folder (`/var/lib/kybernow/.os-state`, root 0700; `/var/lib/kybernow` belongs to root, the app's account only has `config` and `update`: `kybernow-os-update prepare`, when the app starts), checked by `minisign` against the release public key shipped in the system (`/etc/kybernow-os/release.pub`), trusted comment included, and the version must be newer than the running one; then `kybernow-os-image write` reads the image **once**: its partitions 1 and 2 (its slot A) go to the free slot in 4 MiB blocks, and what is written is exactly what is hashed; if the digest is not the manifest's, the slot stays incomplete and is never tried. Then `apply` rewrites the new slot's `cmdline.txt` and `fstab` with its PARTUUIDs, **copies over the settings that live outside the data partition** (NetworkManager Wi-Fi networks, paired controllers from `/var/lib/bluetooth`, SSH on or off, device name, time zone, HDMI mode `video=`), sets `autoboot.txt` (default = current, trial = new) and reboots with `reboot "0 tryboot"`. A lock (`flock`) prevents two writes at once, and the app does not start a second `apply`. **Trial**: if the app reaches the home screen during the trial boot (firmware `tryboot` flag), `commit` makes it the default. If it crashes, if the system freezes (the Pi's hardware watchdog: `RuntimeWatchdogSec=15s`, `etc/watchdog-kybernow.conf`) or if the home screen does not show within 5 minutes (`kybernow-os-trial.timer` → `trial-guard`), the next boot goes back to the default; there, `commit` sees the pending trial without the `tryboot` flag: it records it as failed (`failed`, returned by `status`), the app shows it ("The update X did not start; back to Y") and does not offer to go back to it (`rollback` refuses as well). `rollback` switches to the other slot if it holds a system that booted. **Power cut**: before writing, `apply` points both `autoboot.txt` boots at the current slot and marks the other one incomplete (`status` no longer offers it, `rollback` refuses it) until the end; when the free slot is A (the device runs on B), p1 is not rewritten as a block, which would erase `autoboot.txt`: the image's boot partition goes to a sparse file in the state folder, is mounted, and its files replace p1's, `autoboot.txt` kept; `autoboot.txt` is written next to itself then renamed. On FAT a rename is not atomic: a cut at that instant leaves the old or the new file, both of which point at a slot that boots. A/B tryboot needs the firmware of the **Pi 4, 400, CM4, 5, 500 and CM5**; a Pi 3 boots slot A and does not update A/B (the app says so: reinstall the card). **Cards flashed before 0.11.8 cannot update A/B**: the running slot's script does the writing, and the old one always stopped (`pipefail` on `xz`); they need reflashing once. Tests without a card: `os/tests/os-update.sh` (dummy image, signature, commit, failed trial).

## Building

On an arm64 Linux with passwordless sudo (the "KyberNow Linux" VM, Ubuntu 24.04; or a Pi): `apt install quilt parted debootstrap zerofree zip dosfstools libarchive-tools libcap2-bin rsync xz-utils bc gpg pigz xxd arch-test` (pi-gen's `depends`), then:

```sh
apps/linux/scripts/deb.sh            # the package (Raspberry Pi's FFmpeg), in apps/linux/dist
apps/linux/os/build-image.sh         # → apps/linux/dist/KyberNow-Link-OS.img.xz (long: half an hour)

# From the Mac, in the VM (what release.sh does after the Flatpaks and the .deb):
apps/linux/os/release-image.sh [--check] [DIR]   # + KyberNow-Link-OS.img.xz.info (sizes, checksums)
apps/linux/os/imager-json.sh VERSION INFO site/updates/rpi-imager.json   # the repository for Raspberry Pi Imager
```

`build-image.sh` clones pi-gen, pinned to a Raspberry Pi OS release (`PIGEN_TAG`, currently `2026-10-06-raspios-trixie-arm64`; `KYBERNOW_PIGEN_TAG` to change it), into `~/kybernow-os/pi-gen`, keeps the rootfs of stages 0 to 2 in `~/kybernow-os/pi-gen-work` and reuses it while the tag and language are the same (only our stage is rebuilt; `KYBERNOW_OS_FRESH=1` starts from scratch), copies `stage-kybernow` and the .deb there, writes `config` (hostname `kybernow-link`, account `kybernow`, locale `fr_FR.UTF-8` by default, `STAGE_LIST="stage0 stage1 stage2 stage-kybernow"`), runs `build.sh`, then `make-ab-image.sh` on the produced image, and `xz`. The work directory is outside the shared folder (pi-gen mounts file systems). **Disk space**: pi-gen keeps one copy of the system per stage, about fifteen GB; once the image has been read back (`xz -t`), and at the start of each build (a previous one may have failed first), `build-image.sh` removes everything except stage 2's system (about 3 GB, reused next time) and the latest base image (`deploy/`, for `--ab-only`), plus `~/kybernow-os/pi-gen/work`, pi-gen's former work directory. `build-image.sh --clean` does only this cleanup; `release-image.sh` runs it before measuring the free space (20 GB). A file system still mounted below (pi-gen stopped midway) stops everything, with the `umount` command to run. One build at a time in `~/kybernow-os` (`scripts/verrou.sh`).

```
apps/linux/os/
├── build-image.sh               pi-gen + our stage + A/B + xz
├── make-ab-image.sh             the A/B layout and the data partition (root)
├── release-image.sh             from the Mac, in the VM; .info for Imager
├── imager-json.sh               site/updates/rpi-imager.json
├── emulate.sh                   the image in QEMU, without a Raspberry Pi (in the VM)
├── emulate/                     qmp.py (captures, keys), gbmshim.c (XRGB for virtio-gpu)
└── stage-kybernow/
    ├── prerun.sh, EXPORT_IMAGE
    ├── 00-packages/00-packages  plymouth, bluez, network-manager, alsa, v4l-utils, parted, Vulkan (Mesa)…
    └── 01-link/
        ├── 00-run.sh            files into the rootfs, config.txt, cmdline.txt
        ├── 01-run-chroot.sh     the .deb, the account, mark.png, Plymouth, the services
        └── files/
            ├── systemd/         kybernow-link.service, kybernow-os-firstboot.service, kybernow-os-ssh.service, kybernow-os-trial.timer/.service
            ├── kybernow-os-update   A/B update (status, apply, commit, rollback, prepare, trial-guard)
            ├── kybernow-os-image    single read of the image (digest, partitions), manifest, versions (python3)
            ├── kybernow-os-ctl      ssh, ssh-keys, reboot, poweroff, standby, cec-name, cec-off, hdmi
            ├── kybernow-os-firstboot   data partition growth
            ├── plymouth/        kybernow.plymouth, kybernow.script (mark.png rendered at build time)
            └── etc/             sudoers, polkit (NetworkManager), asound.conf, bluetooth (JustWorks), sshd (keys on the data partition)
```

## Trying the image without a Raspberry Pi (QEMU)

In the VM (`apt install qemu-system-arm gcc python3`):

```sh
apps/linux/os/emulate.sh [apps/linux/dist/KyberNow-Link-OS.img.xz] [--size 8G] [--fresh]
ssh -p 2222 -i ~/kybernow-os/qemu/id_ed25519 kybernow@localhost   # inside the emulated OS (sudo open, for the test)
python3 apps/linux/os/emulate/qmp.py shot home                     # → ~/kybernow-os/qemu/shots/home.png
python3 apps/linux/os/emulate/qmp.py key ret left ret              # keys: ret, up, down, left, right, esc, tab, spc…
apps/linux/os/emulate.sh --stop
```

`emulate.sh` decompresses the image into `~/kybernow-os/qemu/kn.img`, grows it (first boot then grows the data partition) and prepares it by mounting: the VM's kernel replaces the Pi's (QEMU "virt" does not boot a Raspberry Pi kernel; its modules go into the rootfs), ssh with a key created there and sudo for `kybernow`, and `emulate/gbmshim.c` (LD_PRELOAD through a service drop-in) which replaces the ARGB8888 SDL asks for with XRGB8888, the only format the virtual card (`virtio-gpu`) accepts for scanout; Plymouth is cut (`plymouth.enable=0`), the initrd being the VM's (its Ubuntu theme would show, and its plymouthd would keep the screen). QEMU runs without a window: captures and keys go through QMP, the serial log is in `serial.log`. Verified this way on 2026-10-24: the system pi-gen builds boots (accounts, services, Link full screen on KMS/DRM as user `kybernow`), first boot (data partition grown, screens Controller → Language → Network (Ethernet) → Use → Computer, with a keyboard), `kybernow-os-update status`. Not verifiable there: the Pi firmware (`config.txt`, `autoboot.txt`, tryboot), Plymouth and its hand-off to the app, the V4L2 decoder, sound, Bluetooth, CEC.

Pitfalls (seen in emulation, handled in the image): the EGL/GBM/GLES libraries (`libegl1`, `libegl-mesa0`, `libgles2`, `libgbm1`, `libgl1-mesa-dri`) are missing from Lite, `00-packages` adds them; `userconfig.service` (Raspberry Pi OS's username wizard) holds tty1 and blocks `multi-user.target`: it is masked; `plymouthd` stays DRM master while it runs and SDL then says "kmsdrm not available" (`plymouth quit --retain-splash` before the app, else `pkill`, as root: `ExecStartPre=+-…`, the `kybernow` account cannot touch it); `bluetoothctl` waits for the bus forever without `bluetoothd` (board without an adapter): the app bounds its calls to 3 s and only listens with an adapter. At first boot, choosing a card (language, use) moves on to the next step.

## On the app side

`src/device/os.rs` (`KYBERNOW_OS=1` or `/etc/kybernow-os/release`): `kybernow-os-update` and `kybernow-os-ctl` through sudo, `nmcli`, `bluetoothctl` kept open (`BtListener`), what gets ready at launch (`readiness`). `src/device/chime.rs`: the ignition sound. `src/ui/setup.rs`, `src/ui/device.rs`: the screens. The app as the OS looks for `KyberNow-Link-OS.img.xz` in the published releases (`jobs::asset`); "Automatic" downloads as soon as it is published; installing waits for your "Restart on X", standby does not install it. Reaching the home screen after launch calls `commit`. Captures: `kybernow --preview` produces `os-00-demarrage-chargement.png`, `os-01-manette.png` … `os-11-reglages-appareil.png`; `kybernow --splash mark.png 1920 1080` renders Plymouth's picture.

## Not yet verified on a board

None of this has run on a Raspberry Pi: the image's boot by the firmware (`config.txt`, Plymouth, service, SDL kmsdrm as user `kybernow`), the Plymouth → app continuity, the V4L2 decoder, ALSA sound (stereo, then 5.1 PCM), Bluetooth (no-code pairing of an Xbox and a DualSense), `nmcli` over Wi-Fi, CEC (name, standby, wake, remote as keys), the A/B update end to end (apply, tryboot, commit, rollback, and a trial that does not boot; writing p1 file by file from slot B), SSH (importing a key dropped on `BOOTA`), the data partition growth on a real card, the HDMI mode through `cmdline.txt`. Since 0.11.8 (fixes from the 27/11 audit): `apply` end to end with `minisign` and `kybernow-os-image` on the card, the settings copied over (Wi-Fi, Bluetooth controllers, SSH, name, HDMI) after a real trial, the `bcm2835-wdt` watchdog (15 s) and `trial-guard` (5 minutes), the record of a failed trial; only `os/tests/os-update.sh`, `bash -n` and `shellcheck` have run, on the Mac.
