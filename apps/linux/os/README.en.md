# KyberNow Link OS: the image to flash on a Raspberry Pi

English · [Français](README.md)

A microSD card (or SSD) that boots nothing but KyberNow Link: the Raspberry Pi becomes a streaming console (on a TV, with a controller) or a remote desktop (screen, keyboard, mouse). Scoping in [PROJECT.en.md](../../../PROJECT.en.md) ("Raspberry Pi client"); mock-ups: https://claude.ai/artifact/6crVeg3QyrA5vs8TRhWRft ("KyberNow Link OS" row). Installation: [../INSTALLATION.en.md](../INSTALLATION.en.md) ("KyberNow Link OS").

**Status (23/10/2026)**: written, the image builds in the "KyberNow Linux" VM; **nothing has booted on a Raspberry Pi yet** (no board available): boot, Plymouth, SDL on KMS/DRM, ALSA sound, Bluetooth, CEC, A/B tryboot and the data partition growth remain to be verified on hardware.

## What the image does

- **Base**: Raspberry Pi OS Lite (Debian 13 "trixie", arm64), built by [pi-gen](https://github.com/RPi-Distro/pi-gen) (stage0 to stage2) then our stage `stage-kybernow`; no desktop, no interactive session (`kybernow` account without a password; SSH off, key-only when turned on in Device settings › Advanced).
- **Link alone, full screen**: `kybernow-link.service` runs the app (the .deb from `scripts/deb.sh`, Raspberry Pi's FFmpeg, HEVC V4L2 decoder) on **KMS/DRM** (`SDL_VIDEODRIVER=kmsdrm`, no compositor), sound through **ALSA** on the HDMI socket (`/etc/asound.conf`), `KYBERNOW_OS=1`, data in `/var/lib/kybernow` (its own partition). It restarts if it stops.
- **"Console" boot**: firmware without the rainbow (`disable_splash=1`), quiet kernel, **Plymouth** draws the still of the unlit mark on ink (`mark.png`, rendered by `kybernow --splash`: the first frame of the ignition, pixel for pixel), then the service asks it to stay on screen (`plymouth quit --retain-splash`) while the app takes over the same picture and plays the brand's ignition **with sound** (`src/device/chime.rs`: crackles, pad, A major 9 chord, chime, resolution; synthesis, to be replaced one day by a composed track). When there is something to wait for (network, Bluetooth), the seam fills under the name.
- **First boot** (`src/ui/setup.rs`, the mock-up's screens): **Controller** (Bluetooth listening through `bluetoothctl`, no-code agent, pairing and connection of any controller that shows up; USB, CEC remote or keyboard work too; bilingual screen, English then French), **Language**, **Network** (Ethernet recognised, otherwise the Wi-Fi list through `nmcli` and the password on the keypad), **Use** (Living room or Desktop: Link's interface), **Computer** (adding by code, the usual screen).
- **Device settings** (`src/ui/device.rs`, ☰ on the home screen): Use, Network, Controllers (pair, test), Screen (HDMI mode at next boot, HDMI-CEC), Sound, **Update**, Language, Advanced (SSH, restart, shut down, version). **Start held three seconds** switches Living room ⇄ Desktop, like Steam and Big Picture; the Guide (or PS) button on the home screen puts the device to standby (⏻ too): screen off, Bluetooth on, the TV put to standby through CEC; a key or a button wakes it.
- **HDMI-CEC** (`cec-ctl`, v4l-utils): at launch the device announces itself as "KyberNow Link" to the TV; standby and wake go through CEC; the remote drives the interface (CEC keys reach the kernel as a keyboard: arrows, OK, back). Can be turned off in Screen.
- **A/B update**: see below.

## Card layout (A/B)

`make-ab-image.sh` rebuilds pi-gen's image (two partitions) into six:

| Partition | Role |
|---|---|
| p1 `BOOTA` (FAT32, 256 MB) | boot A, and `autoboot.txt` (the firmware always reads it from partition 1) |
| p2 `rootA` (ext4) | system A (the image's): what the base uses, plus one gigabyte (3.4 GB at most) |
| p3 `BOOTB` (FAT32, 256 MB) | boot B (empty: the first update writes it) |
| p4 | extended |
| p5 `rootB` (ext4) | system B (empty) |
| p6 `data` (ext4, 256 MB) | `/var/lib/kybernow`: computers, settings, log, update images; **grown at first boot** to the end of the card (`kybernow-os-firstboot`) |

The disk identifier is fixed (`0x4b79624e`), so the PARTUUIDs are too (`4b79624e-01` … `-06`), written into `cmdline.txt` and `/etc/fstab`; `/etc/kybernow-os/layout` states the layout, `/etc/kybernow-os/release` the version.

**autoboot.txt**: `[all] tryboot_a_b=1 boot_partition=1` / `[tryboot] boot_partition=3`. The update (`kybernow-os-update`, root through sudo): the app downloads the published release's `KyberNow-Link-OS.img.xz` (checked by the core: signed manifest, size, SHA-256) into the data partition, then `apply` writes the image's partitions 1 and 2 (its slot A) into the free slot **streaming** (`xz -dc | dd`, without decompressing the image on the card), rewrites the new slot's `cmdline.txt` and `fstab` with its PARTUUIDs, writes `autoboot.txt` (default = running, trial = new) and reboots with `reboot "0 tryboot"`. At the next boot, if the app reaches the home screen it calls `commit`: the trial becomes the default; otherwise the tryboot flag lasts one boot only and the firmware goes back to the default by itself. `rollback` switches to the other slot when it holds a system. A/B tryboot needs the firmware of the **Pi 4, 400, CM4, 5, 500 and CM5**; a Pi 3 boots slot A and does not update A/B (the app says so: reflash the card).

## Building

On an arm64 Linux with passwordless sudo (the "KyberNow Linux" VM, Ubuntu 24.04; or a Pi): `apt install quilt parted debootstrap zerofree zip dosfstools libarchive-tools libcap2-bin rsync xz-utils bc gpg pigz xxd arch-test` (pi-gen's `depends`), then:

```sh
apps/linux/scripts/deb.sh            # the package (Raspberry Pi's FFmpeg), in apps/linux/dist
apps/linux/os/build-image.sh         # → apps/linux/dist/KyberNow-Link-OS.img.xz (long: half an hour)

# From the Mac, in the VM (what release.sh does after the Flatpaks and the .deb):
apps/linux/os/release-image.sh [--check] [DIR]   # + KyberNow-Link-OS.img.xz.info (sizes, checksums)
apps/linux/os/imager-json.sh VERSION INFO site/updates/rpi-imager.json   # the repository for Raspberry Pi Imager
```

`build-image.sh` clones pi-gen (`arm64` branch) into `~/kybernow-os/pi-gen`, copies `stage-kybernow` and the .deb there, writes `config` (hostname `kybernow-link`, account `kybernow`, locale `fr_FR.UTF-8` by default, `STAGE_LIST="stage0 stage1 stage2 stage-kybernow"`), runs `build.sh`, then `make-ab-image.sh` on the produced image, and `xz`. The work directory is outside the shared folder (pi-gen mounts file systems).

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
    ├── 00-packages/00-packages  plymouth, bluez, network-manager, alsa, v4l-utils, parted…
    └── 01-link/
        ├── 00-run.sh            files into the rootfs, config.txt, cmdline.txt
        ├── 01-run-chroot.sh     the .deb, the account, mark.png, Plymouth, the services
        └── files/
            ├── systemd/         kybernow-link.service, kybernow-os-firstboot.service
            ├── kybernow-os-update   A/B update (status, apply, commit, rollback)
            ├── kybernow-os-ctl      ssh, reboot, poweroff, standby, cec-name, cec-wake, hdmi, hostname
            ├── kybernow-os-firstboot   data partition growth
            ├── plymouth/        kybernow.plymouth, kybernow.script (mark.png rendered at build time)
            └── etc/             sudoers, asound.conf, bluetooth (JustWorks)
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

Pitfalls met in emulation, fixed in the image: the EGL/GBM/GLES libraries (`libegl1`, `libegl-mesa0`, `libgles2`, `libgbm1`, `libgl1-mesa-dri`) are missing from Lite; `userconfig.service` (Raspberry Pi OS's username wizard) held tty1 and blocked `multi-user.target` (masked); `plymouthd` stays DRM master while it runs and SDL then says "kmsdrm not available" (`plymouth quit --retain-splash` before the app, else `pkill`, as root: `ExecStartPre=+-…`, the `kybernow` account cannot touch it); `bluetoothctl` waits for the bus forever without `bluetoothd` (board without an adapter): the app bounds its calls to 3 s and only says "listening" with an adapter; choosing a card (language, use) continues first boot.

## On the app side

`src/device/os.rs` (`KYBERNOW_OS=1` or `/etc/kybernow-os/release`): `kybernow-os-update` and `kybernow-os-ctl` through sudo, `nmcli`, `bluetoothctl` kept open (`BtListener`), what gets ready at launch (`readiness`). `src/device/chime.rs`: the ignition sound. `src/ui/setup.rs`, `src/ui/device.rs`: the screens. The app as the OS looks for `KyberNow-Link-OS.img.xz` in the published releases (`jobs::asset`); "Automatic" downloads as soon as it is published, installing waits for your "Restart on X" (or standby, to come). Captures: `kybernow --preview` produces `os-00-demarrage-chargement.png`, `os-01-manette.png` … `os-11-reglages-appareil.png`; `kybernow --splash mark.png 1920 1080` renders Plymouth's picture.

## To verify on a board

In order: the image's boot (firmware, Plymouth, service, SDL kmsdrm as user `kybernow`), the Plymouth → app continuity, ALSA sound (stereo, then 5.1 PCM), Bluetooth (no-code pairing of an Xbox and a DualSense), `nmcli`, CEC (name, standby, remote as keys), the A/B update end to end (apply, tryboot, commit, rollback, and a trial that does not boot), the data partition growth, the HDMI mode through `cmdline.txt`.
