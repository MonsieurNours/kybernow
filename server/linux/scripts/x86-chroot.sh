#!/bin/bash
# Ubuntu 24.04 x86_64 dans la VM arm64 « KyberNow Linux », par Rosetta : de
# quoi compiler la Station Linux x86_64 sans machine x86_64
# (docs/station-linux.md, « Compiler » ; release-station.sh s'en sert quand
# KYBERNOW_STATION_X86_SSH n'est pas réglé). Même Ubuntu, même glibc (2.39)
# que l'arm64 : les paquets et l'archive des systèmes immuables vont aux
# mêmes systèmes.
#
#   x86-chroot.sh setup          crée ou complète le chroot (debootstrap,
#                                paquets de compilation, Rust, cargo-c, meson)
#   x86-chroot.sh run 'COMMANDE' lance COMMANDE dans le chroot, en
#                                l'utilisateur courant, depuis $HOME
#
# À lancer dans la VM. Rosetta doit être dans binfmt (Parallels, « Rosetta
# pour Linux » ; drapeau F, valable dans un chroot). Chroot : $KN_X86_ROOT
# (défaut /srv/kn-amd64). Le dossier personnel du chroot est
# ~/kn-amd64-home (sources de Kyber x86_64 à part : ~/kn-amd64-home/kyber-desktop,
# l'arm64 garde les siennes) ; le dépôt partagé /media/psf/kyber-apps y est
# monté au même endroit.
set -euo pipefail
# Versions de Rust, cargo-c et meson : scripts/kyber-toolchain.sh (TRANS-20).
# shellcheck source=../../../scripts/kyber-toolchain.sh
. "$(cd "$(dirname "$0")/../../.." && pwd)/scripts/kyber-toolchain.sh"

R="${KN_X86_ROOT:-/srv/kn-amd64}"
U="$(id -un)"
HOME_X86="$HOME/kn-amd64-home"
SHARED=/media/psf/kyber-apps

[ -e /proc/sys/fs/binfmt_misc/RosettaLinux ] || { echo "Rosetta absente de binfmt (Parallels : Rosetta pour Linux)" >&2; exit 1; }

mounts() {
    local m
    for m in /proc /sys /dev; do
        mountpoint -q "$R$m" || { sudo mkdir -p "$R$m"; sudo mount --rbind "$m" "$R$m"; sudo mount --make-rslave "$R$m"; }
    done
    mkdir -p "$HOME_X86"
    sudo mkdir -p "$R/home/$U"
    mountpoint -q "$R/home/$U" || sudo mount --bind "$HOME_X86" "$R/home/$U"
    if [ -d "$SHARED" ]; then
        sudo mkdir -p "$R$SHARED"
        mountpoint -q "$R$SHARED" || sudo mount --bind "$SHARED" "$R$SHARED"
    fi
    [ -d "$R/etc" ] && sudo cp /etc/resolv.conf "$R/etc/resolv.conf"
}

unmount() {
    local m
    for m in "$SHARED" "/home/$U" /dev /sys /proc; do
        mountpoint -q "$R$m" 2>/dev/null && sudo umount -R "$R$m" || true
    done
}

setup() {
    # $R/debootstrap reste tant que le second temps n'a pas abouti.
    if [ ! -x "$R/usr/bin/apt-get" ] || [ -d "$R/debootstrap" ]; then
        # debootstrap ne reconnaît pas Rosetta (« amd64: not supported on
        # this machine/kernel ») : en deux temps, le second dans le chroot,
        # /proc monté (Rosetta lit /proc/self/exe).
        unmount
        # Jamais d'effacement sous un montage resté en place (un processus encore
        # dans le chroot) : /dev de la VM et le dépôt partagé du Mac y sont.
        if [ -d "$R" ] && findmnt -rn -o TARGET | grep -qE "^$R(/|$)"; then
            echo "Montages encore actifs sous $R (processus dans le chroot ?) : rien n'est effacé." >&2
            findmnt -rn -o TARGET | grep -E "^$R(/|$)" >&2
            exit 1
        fi
        sudo rm -rf --one-file-system "$R"
        sudo debootstrap --foreign --arch=amd64 --variant=buildd --components=main,universe noble "$R" http://archive.ubuntu.com/ubuntu
        mounts
        sudo chroot "$R" /debootstrap/debootstrap --second-stage
    fi
    printf '%s\n' "deb http://archive.ubuntu.com/ubuntu noble main universe" \
        "deb http://archive.ubuntu.com/ubuntu noble-updates main universe" \
        "deb http://security.ubuntu.com/ubuntu noble-security main universe" | sudo tee "$R/etc/apt/sources.list" >/dev/null
    mounts
    sudo chroot "$R" bash -c "id $U >/dev/null 2>&1 || useradd -u $(id -u) -M -d /home/$U -s /bin/bash $U; chown $U: /home/$U"
    # Les outils de « Compiler » (docs/station-linux.md), plus git, curl, rpm.
    sudo chroot "$R" bash -c 'export DEBIAN_FRONTEND=noninteractive; dpkg --configure -a; apt-get update -q && apt-get install -y -q --no-install-recommends \
      build-essential cmake ninja-build nasm bison flex pkg-config pipx lua5.4 python3-jinja2 git curl wget ca-certificates rpm xz-utils file \
      glslang-tools spirv-tools libclang-dev libssl-dev libv4l-dev libcap-dev libvdpau-dev libnuma-dev \
      libva-dev libdrm-dev libgbm-dev libegl-dev libvulkan-dev \
      libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpipewire-0.3-dev libpulse-dev libasound2-dev \
      libxcb1-dev libxcb-shm0-dev libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shape0-dev libxtst-dev libxrandr-dev \
      libxkbcommon-dev libwayland-dev wayland-protocols libevdev-dev libudev-dev libinput-dev libdbus-1-dev \
      libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev'
    # shellcheck disable=SC2016  # $(…) et ~ développés dans le chroot
    run "RUSTC=$KYBER_RUSTC CARGO_C=$KYBER_CARGO_C MESON=$KYBER_MESON"'
      [ -x ~/.cargo/bin/rustup ] || curl -sSf https://sh.rustup.rs | sh -s -- -y -q --default-toolchain "$RUSTC" --profile minimal
      . ~/.cargo/env
      rustup toolchain install "$RUSTC" --profile minimal
      command -v cargo-cbuild >/dev/null || cargo install -q cargo-c --version "$CARGO_C" --locked --features vendored-openssl
      [ -x ~/.local/bin/meson ] || { pipx install "meson==$MESON" && pipx inject meson jinja2; }
      echo "chroot $(uname -m) : $(rustc --version), meson $(~/.local/bin/meson --version)"'
    # Sources de Kyber x86_64 : une copie de celles de l'arm64, sans rien de
    # compilé (dossiers de construction, rootfs, contrib/work de kynput : le
    # SDL2 configuré ailleurs y garde un chemin absolu et ne se recompile pas).
    if [ ! -d "$HOME_X86/kyber-desktop" ] && [ -d "$HOME/kn-station/kyber-desktop" ]; then
        rsync -a --exclude 'builddir-*' --exclude 'rootfs-*' --exclude target --exclude 'build-linux-*' \
            --exclude 'kysdk/kynput/contrib/work' "$HOME/kn-station/kyber-desktop/" "$HOME_X86/kyber-desktop/"
    fi
}

run() {
    mounts
    sudo chroot --userspec="$U:$U" "$R" env -i HOME="/home/$U" USER="$U" TERM="${TERM:-dumb}" \
        PATH="/home/$U/.local/bin:/home/$U/.cargo/bin:/usr/local/bin:/usr/bin:/bin" \
        bash -lc "set -e; cd ~; $1"
}

case "${1:-}" in
    setup) setup ;;
    run) run "${2:?commande}" ;;
    *) echo "usage : $0 setup | run 'COMMANDE'" >&2; exit 2 ;;
esac
