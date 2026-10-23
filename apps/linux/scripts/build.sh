#!/bin/bash
# Compile KyberNow Link pour Linux, sur un Linux (x86_64 ou aarch64) ou dans
# le SDK de freedesktop (flatpak-builder, voir flatpak.sh) :
#
#   1. applique les correctifs client de Kyber (apps/macos/kyber-patches) et
#      celui de webOS qui fait de kynput un client seul (cfg kybernow_webos :
#      sans l'injection d'entrées du serveur, libinput, libevdev, X11) ;
#   2. libkynput (statique, cargo cinstall) dans le préfixe ;
#   3. FFmpeg 7.1.1 réduit au décodage H.264, HEVC et AV1, VA-API compris
#      (statique, LGPL 2.1) dans le préfixe ;
#   4. l'app (cargo build -p kybernow-linux).
#
# Usage : apps/linux/scripts/build.sh [--check] [--debug] [--deps-only] [--prefix DIR] [--install DIR]
#   --deps-only  s'arrête après libkynput et FFmpeg (module du Flatpak mis en cache)
#   --prefix  où poser libkynput et FFmpeg (par défaut third_party/linux-<arch>)
#   --install où copier l'app (bin/kybernow), sinon elle reste dans apps/target
#
# Dépendances (Ubuntu/Debian) : build-essential pkg-config nasm clang libclang-dev
# libsdl2-dev libva-dev libdrm-dev libopus-dev, Rust 1.89 ou après et cargo-c.
# Dans le SDK de freedesktop : extensions rust-stable et llvm.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
KYSDK="$ROOT/third_party/kyber-desktop/kysdk"
ARCH="$(uname -m)"
PREFIX="$ROOT/third_party/linux-$ARCH"
COMMAND=build
PROFILE=release
INSTALL=""
DEPS_ONLY=0
while [ $# -gt 0 ]; do
    case "$1" in
        --check) COMMAND=check ;;
        --debug) PROFILE=debug ;;
        --deps-only) DEPS_ONLY=1 ;;
        --prefix) PREFIX="$2"; shift ;;
        --install) INSTALL="$2"; shift ;;
        *) echo "Option inconnue : $1" >&2; exit 1 ;;
    esac
    shift
done
[ -d "$KYSDK/kyctl" ] || { echo "SDK Kyber introuvable dans $KYSDK (voir docs/build-notes.md)" >&2; exit 1; }
mkdir -p "$PREFIX"
PREFIX="$(cd "$PREFIX" && pwd)"
JOBS="$(nproc 2>/dev/null || echo 4)"
export PATH="$HOME/.cargo/bin:$PATH"

cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing) ' || true; }
}

# 1. Correctifs.
for patch in "$APPS"/macos/kyber-patches/*.patch "$APPS"/webos/kyber-patches/*.patch; do
    case "$(basename "$patch")" in
        *-kyclient-*) repo="$KYSDK/kyctl" ;;
        *-kynput-*) repo="$KYSDK/kynput" ;;
        *-kynet-*) repo="$KYSDK/kymux" ;;
        *) echo "Dépôt inconnu pour $(basename "$patch")" >&2; exit 1 ;;
    esac
    # Déjà appliqué (les sources viennent souvent d'un arbre déjà corrigé) : rien à faire.
    if (cd "$repo" && patch -p1 -R --dry-run -s -f < "$patch" >/dev/null 2>&1); then
        continue
    fi
    if (cd "$repo" && patch -p1 --dry-run -s -f -F0 < "$patch" >/dev/null 2>&1); then
        (cd "$repo" && patch -p1 -s -f -F0 < "$patch") && echo "==> $(basename "$patch") appliqué"
    else
        echo "Correctif $(basename "$patch") inapplicable" >&2
        exit 1
    fi
done

# kynput en client seul, comme sur webOS et Android.
export RUSTFLAGS="${RUSTFLAGS:-} --cfg kybernow_webos"

# 2. libkynput.
if [ ! -f "$PREFIX/lib/pkgconfig/kynput.pc" ]; then
    command -v cargo-cinstall >/dev/null || cargo install cargo-c --version 0.10.15 --locked
    echo "==> libkynput dans $PREFIX"
    (cd "$KYSDK/kynput" && cargo_quiet cargo cinstall -p kynput --release --library-type=staticlib --prefix="$PREFIX" --libdir="$PREFIX/lib" --target-dir="$PREFIX/build/kynput")
    [ -f "$PREFIX/lib/pkgconfig/kynput.pc" ] || { echo "libkynput non installée dans $PREFIX" >&2; exit 1; }
fi

# 3. FFmpeg réduit au décodage (VA-API).
FFMPEG_VERSION=7.1.1
if [ ! -f "$PREFIX/lib/libavcodec.a" ]; then
    src="${FFMPEG_SRC:-$PREFIX/build/ffmpeg-$FFMPEG_VERSION}"
    if [ ! -f "$src/configure" ]; then
        echo "==> sources de FFmpeg $FFMPEG_VERSION"
        mkdir -p "$PREFIX/build"
        curl -fsSL "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz" | tar xJ -C "$PREFIX/build"
    fi
    echo "==> libavcodec (décodeurs H.264, HEVC, AV1, VA-API) dans $PREFIX"
    build="$PREFIX/build/ffmpeg-build"
    rm -rf "$build" && mkdir -p "$build"
    (cd "$build" && "$src/configure" --prefix="$PREFIX" --libdir="$PREFIX/lib" \
        --disable-everything --disable-programs --disable-doc --disable-network --disable-autodetect \
        --enable-static --disable-shared --enable-pic --disable-debug \
        --enable-avcodec --enable-avutil --disable-avformat --disable-avfilter --disable-avdevice --disable-swresample --disable-swscale \
        --enable-vaapi --enable-libdrm \
        --enable-decoder=h264 --enable-decoder=hevc --enable-decoder=av1 \
        --enable-hwaccel=h264_vaapi --enable-hwaccel=hevc_vaapi --enable-hwaccel=av1_vaapi \
        --enable-parser=h264 --enable-parser=hevc --enable-parser=av1 >/dev/null \
        && make -j"$JOBS" >/dev/null && make install >/dev/null)
    [ -f "$PREFIX/lib/libavcodec.a" ] || { echo "libavcodec non construite" >&2; exit 1; }
fi

# VLC : jamais appelé (fonctions vides, build.rs).
if [ ! -f "$PREFIX/lib/pkgconfig/libvlc.pc" ]; then
    cat > "$PREFIX/lib/pkgconfig/libvlc.pc" <<'PC'
Name: libvlc
Description: VLC absent (fonctions vides)
Version: 4.0.0
Libs:
Cflags:
PC
fi

[ "$DEPS_ONLY" = 1 ] && { echo "==> dépendances prêtes dans $PREFIX"; exit 0; }

# 4. L'app.
export KYBERNOW_PREFIX="$PREFIX"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export CARGO_PROFILE_RELEASE_STRIP=false
flags=()
[ "$PROFILE" = release ] && flags+=(--release)
echo "==> cargo $COMMAND kybernow-linux ($ARCH, $PROFILE)"
started=$(date +%s)
(cd "$APPS" && cargo_quiet cargo "$COMMAND" -p kybernow-linux "${flags[@]}")
[ "$COMMAND" = build ] || { echo "    vérifié ($(( $(date +%s) - started )) s)"; exit 0; }
bin="${CARGO_TARGET_DIR:-$APPS/target}/$PROFILE/kybernow"
[ -f "$bin" ] || { echo "$bin absent" >&2; exit 1; }
echo "    $bin ($(du -h "$bin" | cut -f1), $(( $(date +%s) - started )) s)"
if [ -n "$INSTALL" ]; then
    install -Dm755 "$bin" "$INSTALL/bin/kybernow"
    echo "    → $INSTALL/bin/kybernow"
fi
