#!/bin/bash
# Construit le Flatpak de KyberNow Link sur un Linux avec flatpak-builder et
# le runtime org.freedesktop.Platform 25.08 (plus rust-stable et llvm20), pour
# l'architecture de la machine ou celle donnée :
#
#   apps/linux/scripts/flatpak.sh [x86_64|aarch64] [--sans-pyrowave]
#
# PyroWave est demandé à Kyber par défaut (build.sh --kyber-pyrowave) ;
# --sans-pyrowave construit l'app qui le décode sans le demander.
#
# → apps/linux/dist/KyberNow-Link-Linux-<arch>.flatpak, un paquet unique qui
# s'installe par `flatpak install --user <fichier>` (le runtime vient de Flathub).
# Pour x86_64 depuis une machine ARM : binfmt (Rosetta ou qemu-user-static).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="$(uname -m)"
KP=" --kyber-pyrowave"
for a in "$@"; do
    case "$a" in
        --kyber-pyrowave) KP=" --kyber-pyrowave" ;;
        --sans-pyrowave) KP=" --sans-pyrowave" ;;
        *) ARCH="$a" ;;
    esac
done
WORK="${KYBERNOW_FLATPAK_WORK:-$HOME/.cache/kybernow-flatpak}"
# Le SDK de Kyber pris dans ses dépôts git, à l'origine (sources.sh) : build.sh
# y pose les correctifs sans rien deviner de ce que portait third_party/
# (LINAPP-43). KYBERNOW_KYBER_HEAD=0 : la copie de third_party/ telle quelle.
export KYBERNOW_KYBER_HEAD="${KYBERNOW_KYBER_HEAD:-1}"
mkdir -p "$WORK" "$HERE/dist"
"$HERE/scripts/sources.sh" --media "$WORK/src-media" >/dev/null
"$HERE/scripts/sources.sh" --deps "$WORK/src-deps" >/dev/null
"$HERE/scripts/sources.sh" "$WORK/src" >/dev/null
# --kyber-pyrowave : le module de l'app compile avec (les dépendances ne changent pas).
sed "s|build.sh --prefix /app/lib/kybernow-sdk --install /app|build.sh$KP --prefix /app/lib/kybernow-sdk --install /app|" \
    "$HERE/flatpak/fr.leslignesclaires.kybernow.yml" > "$WORK/fr.leslignesclaires.kybernow.yml"
cd "$WORK"
# --ccache : /run/ccache garde aussi les crates de cargo d'une construction à l'autre (build.sh).
flatpak-builder --arch="$ARCH" --user --force-clean --disable-rofiles-fuse --ccache --state-dir="$WORK/state-$ARCH" \
    --repo="$WORK/repo" "$WORK/build-$ARCH" fr.leslignesclaires.kybernow.yml
OUT="$HERE/dist/KyberNow-Link-Linux-$ARCH.flatpak"
flatpak build-bundle --arch="$ARCH" --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    "$WORK/repo" "$OUT" fr.leslignesclaires.kybernow
echo "$OUT ($(du -h "$OUT" | cut -f1))"
