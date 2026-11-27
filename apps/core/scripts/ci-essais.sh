#!/bin/bash
# Essais du cœur (apps/core) hors du Mac de l'auteur : intégration continue
# (.github/workflows/coeur.yml), ou tout clone neuf du dépôt.
#
#   apps/core/scripts/ci-essais.sh [arguments de cargo test…]
#
# Le cœur ne compile que contre les sources de Kyber 0.29 avec les correctifs
# KyberNow des apps. Le script prépare ce qu'il faut, dans le dépôt où il est :
#
# 1. third_party/kyber-desktop : cloné depuis gitlab.com/kyber (étiquette
#    0.29.0), avec seulement les sous-modules que lit cargo (kysdk et ses
#    dépôts, vlc-rs, les dépendances de kynput) ; gardé s'il est déjà là.
# 2. Les correctifs des apps, posés comme par les scripts de construction
#    (scripts/kyber-client-patches.sh : ceux de la Station pour kynput,
#    kycontroller…, puis apps/macos/kyber-patches et les *-kyclient-* de
#    server/kyber-patches, PyroWave compris).
# 3. libkynput (API C de kynput, sans SDL) par cargo-c, dans
#    third_party/ci-rootfs, pour kynput-sys.
# 4. Une libvlc factice dans ce même rootfs : kyvlcplayer lie la libvlc 4 de
#    Kyber (avec ses propres ajouts, comme les mesures de l'image), qu'aucun
#    système ne fournit et que les essais n'appellent jamais. Chaque fonction
#    déclarée par vlc-rs et kyvlcplayer y est une fonction qui arrête le
#    processus : un essai qui ouvrirait vraiment VLC échouerait bruyamment.
# 5. cargo test -p kybernow-core --features pyrowave --lib.
#
# À fournir par la machine : git, Rust 1.89 ou plus, cargo-c, pkg-config,
# un compilateur C, libclang (bindgen), libopus ; sous Linux en plus libudev
# et libevdev (kynput). Variables : KYBER_DESKTOP_URL, KYBER_DESKTOP_TAG,
# CI_ROOTFS, et CI_TARGET pour une autre cible que la machine.
#
# CI_TARGET=i686-unknown-linux-gnu (Linux x86_64) : les mêmes essais en
# 32 bits, lancés pour de vrai (`long` et `time_t` de 32 bits, comme webOS et
# Android armeabi-v7a : COEUR-02). La machine fournit alors les bibliothèques
# i386 (libopus, libudev, libevdev) et gcc-multilib ; libkynput et la libvlc
# factice sont faites pour la cible.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
KD="$ROOT/third_party/kyber-desktop"
KYSDK="$KD/kysdk"
URL="${KYBER_DESKTOP_URL:-https://gitlab.com/kyber/apps/kyber-desktop.git}"
TAG="${KYBER_DESKTOP_TAG:-0.29.0}"
TARGET="${CI_TARGET:-}"
PREFIX="${CI_ROOTFS:-$ROOT/third_party/ci-rootfs${TARGET:+-$TARGET}}"
TARGET_ARGS=()
CC_ARGS=()
if [ -n "$TARGET" ]; then
    TARGET_ARGS=(--target "$TARGET")
    case "$TARGET" in
        i686-*) CC_ARGS=(-m32) ;;
    esac
    # pkg-config de la cible : le rootfs, puis les bibliothèques i386 du système.
    export PKG_CONFIG_ALLOW_CROSS=1
    case "$TARGET" in
        i686-unknown-linux-gnu) export PKG_CONFIG_PATH="/usr/lib/i386-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}" ;;
    esac
fi

# Un sous-module à son commit, sans l'historique si le serveur le permet.
submodule() {
    local repo=$1 path=$2
    git -C "$repo" submodule update --init --depth 1 -- "$path" 2>/dev/null ||
        git -C "$repo" submodule update --init -- "$path"
}

# 1. Sources de Kyber.
if [ ! -f "$KYSDK/kyctl/kyclient/Cargo.toml" ]; then
    echo "==> kyber-desktop $TAG"
    [ -d "$KD/.git" ] || git clone --quiet --depth 1 --branch "$TAG" "$URL" "$KD"
    submodule "$KD" kysdk
    for r in kyctl kymedia kymux kynput kyutil; do
        submodule "$KYSDK" "$r"
    done
    submodule "$KYSDK/kymedia" subprojects/vlc-rs
    for e in libudev-sys rust-sdl2 vigem-client; do
        submodule "$KYSDK/kynput" "external/$e"
    done
fi

# 2. Correctifs des apps.
# shellcheck source=../../../scripts/kyber-client-patches.sh
. "$ROOT/scripts/kyber-client-patches.sh"
PATCHES=("$ROOT"/apps/macos/kyber-patches/*.patch "$ROOT"/server/kyber-patches/*-kyclient-*.patch)
kyber_apply_client_patches "$KYSDK" "${PATCHES[@]}"

# 3. libkynput.
mkdir -p "$PREFIX/lib/pkgconfig"
stamp="$(kyber_kynput_stamp "$KYSDK")"
if ! kyber_kynput_current "$PREFIX" "$stamp"; then
    echo "==> libkynput dans $PREFIX"
    (cd "$KYSDK/kynput" && cargo cinstall -p kynput ${TARGET_ARGS[@]+"${TARGET_ARGS[@]}"} --release --library-type=staticlib --prefix="$PREFIX" --libdir="$PREFIX/lib")
    [ -f "$PREFIX/lib/pkgconfig/kynput.pc" ] || { echo "libkynput non installée dans $PREFIX" >&2; exit 1; }
    kyber_kynput_mark "$PREFIX" "$stamp"
fi

# 4. libvlc factice.
case "$(uname -s)" in
    Darwin) lib="$PREFIX/lib/libvlc.dylib"; shared=(-dynamiclib -install_name "$PREFIX/lib/libvlc.dylib") ;;
    *) lib="$PREFIX/lib/libvlc.so"; shared=(-shared -fPIC) ;;
esac
stub="$PREFIX/libvlc-factice.c"
{
    cat <<'C'
/* libvlc factice des essais du cœur (apps/core/scripts/ci-essais.sh). */
#include <stdio.h>
#include <stdlib.h>
static void absent(const char *f) { fprintf(stderr, "libvlc factice : %s appelée\n", f); abort(); }
C
    grep -rhoE '\bfn libvlc_[A-Za-z0-9_]+' "$KYSDK/kymedia/subprojects/vlc-rs/src" "$KYSDK/kyctl/kyvlcplayer" |
        sed 's/^fn //' | sort -u | while read -r f; do
            echo "void $f(void) { absent(\"$f\"); }"
        done
} > "$stub.tmp"
if ! cmp -s "$stub.tmp" "$stub" || [ ! -f "$lib" ]; then
    mv "$stub.tmp" "$stub"
    echo "==> libvlc factice ($(grep -c '^void' "$stub") fonctions)"
    "${CC:-cc}" ${CC_ARGS[@]+"${CC_ARGS[@]}"} "${shared[@]}" -o "$lib" "$stub"
else
    rm -f "$stub.tmp"
fi
cat > "$PREFIX/lib/pkgconfig/libvlc.pc" <<EOF
prefix=$PREFIX
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: libvlc
Description: libvlc factice des essais du cœur de KyberNow
Version: 4.0.0-factice
Libs: -L\${libdir} -lvlc
Cflags:
EOF

# 5. Essais.
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
case "$(uname -s)" in
    Darwin) export DYLD_LIBRARY_PATH="$PREFIX/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" ;;
    *) export LD_LIBRARY_PATH="$PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
esac
cd "$ROOT/apps"
echo "==> cargo test -p kybernow-core --features pyrowave --lib ${TARGET_ARGS[*]+"${TARGET_ARGS[*]}"}"
cargo test --locked -p kybernow-core --features pyrowave --lib ${TARGET_ARGS[@]+"${TARGET_ARGS[@]}"} -- "$@"
