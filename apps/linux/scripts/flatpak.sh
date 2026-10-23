#!/bin/bash
# Construit le Flatpak de KyberNow Link sur un Linux avec flatpak-builder et
# le runtime org.freedesktop.Platform 25.08 (plus rust-stable et llvm20), pour
# l'architecture de la machine ou celle donnée :
#
#   apps/linux/scripts/flatpak.sh [x86_64|aarch64]
#
# → apps/linux/dist/KyberNow-Link-Linux-<arch>.flatpak, un paquet unique qui
# s'installe par `flatpak install --user <fichier>` (le runtime vient de Flathub).
# Pour x86_64 depuis une machine ARM : binfmt (Rosetta ou qemu-user-static).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="${1:-$(uname -m)}"
WORK="${KYBERNOW_FLATPAK_WORK:-$HOME/.cache/kybernow-flatpak}"
mkdir -p "$WORK" "$HERE/dist"
"$HERE/scripts/sources.sh" --deps "$WORK/src-deps" >/dev/null
"$HERE/scripts/sources.sh" "$WORK/src" >/dev/null
cp "$HERE/flatpak/fr.leslignesclaires.kybernow.yml" "$WORK/"
cd "$WORK"
flatpak-builder --arch="$ARCH" --user --force-clean --disable-rofiles-fuse --state-dir="$WORK/state-$ARCH" \
    --repo="$WORK/repo" "$WORK/build-$ARCH" fr.leslignesclaires.kybernow.yml
OUT="$HERE/dist/KyberNow-Link-Linux-$ARCH.flatpak"
flatpak build-bundle --arch="$ARCH" --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    "$WORK/repo" "$OUT" fr.leslignesclaires.kybernow
echo "$OUT ($(du -h "$OUT" | cut -f1))"
