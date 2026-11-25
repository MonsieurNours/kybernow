#!/bin/bash
# Archive de KyberNow Station pour les systèmes immuables (Bazzite et les
# Fedora Atomic, mode bureau de SteamOS) : installée dans le dossier de
# l'utilisateur, sans toucher à /usr (docs/station-linux.md, « Systèmes
# immuables »).
#
#   make-archive.sh <arbre|paquet.deb> [dossier de sortie]
#
# <arbre> : l'assemblage de server/linux/scripts/package.sh (usr/lib/...),
# qui appelle ce script après le .deb ; ou un .deb déjà fait (lu par `ar`,
# aussi depuis un Mac). Même contenu que le paquet, rangé autrement :
#
#   kybernow-station-<version>-<arch>/
#     install.sh                 installation (bin/kybernow-station-home install)
#     VERSION, BUILD             version ; architecture et glibc de la machine qui a compilé
#     bin/                       kyber-admin, kybernow-display, kybernow-station-setup,
#                                kybernow-station-home (installation, mise à jour, retrait)
#     runtime/                   Kyber
#     share/systemd-user/        services de l'utilisateur du paquet (chemins /usr réécrits à l'installation)
#     share/applications/        fichier .desktop de KWin (KDE Plasma)
#     share/udev/                règle udev (posée avec sudo seulement si /dev/uinput est fermé)
#
# Sortie : <dossier>/KyberNow-Station-Linux-<x86_64|aarch64>.tar.xz (défaut :
# server/linux/dist), nom fixe d'une version à l'autre comme les autres
# installateurs publiés.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
SRC="${1:?usage : make-archive.sh <arbre|paquet.deb> [dossier de sortie]}"
DIST="${2:-$ROOT/server/linux/dist}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [ -f "$SRC" ]; then
    # Un .deb : archive ar dont data.tar.* contient l'arbre.
    SRC="$(cd "$(dirname "$SRC")" && pwd)/$(basename "$SRC")"
    mkdir -p "$WORK/tree"
    member=$(cd "$WORK" && ar t "$SRC" | grep '^data\.tar' | head -1 || true)
    [ -n "$member" ] || { echo "$SRC : pas de data.tar dans ce .deb" >&2; exit 1; }
    (cd "$WORK" && ar x "$SRC" "$member" && tar -xf "$member" -C tree)
    TREE="$WORK/tree"
    # Version et architecture dans le nom : kybernow-station_<version>_<amd64|arm64>.deb
    # (« ~ » à la place du « - » des préversions).
    name="$(basename "$SRC" .deb)"
    [ -z "${VERSION:-}" ] && [ "${name%%_*}" = kybernow-station ] && VERSION="$(echo "$name" | cut -d_ -f2 | tr '~' '-')"
    case "$name" in *_amd64) ARCH=x86_64 ;; *_arm64) ARCH=aarch64 ;; *) ARCH="${ARCH:-$(uname -m)}" ;; esac
else
    TREE="$(cd "$SRC" && pwd)"
    ARCH="${ARCH:-$(uname -m)}"
fi
[ "$ARCH" = arm64 ] && ARCH=aarch64
LIB="$TREE/usr/lib/kybernow-station"
[ -x "$LIB/bin/kyber-admin" ] || { echo "$SRC : kyber-admin absent (usr/lib/kybernow-station/bin)" >&2; exit 1; }
VERSION="${VERSION:-$(awk -F'"' '/^version/ {print $2; exit}' "$ROOT/server/admin/Cargo.toml")}"

OUT="$WORK/kybernow-station-$VERSION-$ARCH"
mkdir -p "$OUT/share/systemd-user" "$OUT/share/applications" "$OUT/share/udev"
cp -a "$LIB/." "$OUT/"
install -m 755 "$HERE/kybernow-station-home" "$OUT/bin/"
install -m 755 "$HERE/install.sh" "$OUT/"
cp "$TREE"/usr/lib/systemd/user/*.service "$OUT/share/systemd-user/"
cp "$TREE"/usr/share/applications/*.desktop "$OUT/share/applications/" 2>/dev/null || true
cp "$TREE"/usr/lib/udev/rules.d/*.rules "$OUT/share/udev/" 2>/dev/null || true
cp "$TREE/usr/share/doc/kybernow-station/copyright" "$OUT/LICENSE" 2>/dev/null || true
printf '%s\n' "$VERSION" > "$OUT/VERSION"

# glibc de la machine qui a compilé : l'installation refuse un système plus
# ancien (les programmes ne démarreraient pas). Lue seulement quand ce script
# tourne sur cette machine, pas depuis un .deb.
glibc=""
if [ -d "$SRC" ] && command -v getconf >/dev/null && getconf GNU_LIBC_VERSION >/dev/null 2>&1; then
    glibc=$(getconf GNU_LIBC_VERSION | awk '{print $2}')
fi
printf 'arch=%s\nglibc=%s\n' "$ARCH" "$glibc" > "$OUT/BUILD"

# Greffon PipeWire de GStreamer de la machine qui a compilé, en secours :
# utilisé seulement si le système n'en a pas (installation, GST_PLUGIN_PATH
# du service de Kyber).
if [ -d "$SRC" ]; then
    plugin=$(find /usr/lib /usr/lib64 -path '*gstreamer-1.0/libgstpipewire.so' 2>/dev/null | head -1 || true)
    if [ -n "$plugin" ]; then
        mkdir -p "$OUT/runtime/gst-fallback"
        cp -L "$plugin" "$OUT/runtime/gst-fallback/"
    fi
fi

mkdir -p "$DIST"
ARCHIVE="$DIST/KyberNow-Station-Linux-$ARCH.tar.xz"
if tar --version 2>/dev/null | grep -q GNU; then
    XZ_OPT="-6 -T0" tar --owner=0 --group=0 --numeric-owner -cJf "$ARCHIVE" -C "$WORK" "$(basename "$OUT")"
else
    tar --uid 0 --gid 0 -cJf "$ARCHIVE" -C "$WORK" "$(basename "$OUT")"
fi
echo "    $ARCHIVE ($(du -h "$ARCHIVE" | cut -f1))"
