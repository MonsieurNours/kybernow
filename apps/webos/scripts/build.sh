#!/bin/bash
# Compile KyberNow Link pour les téléviseurs LG (webOS) : le binaire natif
# kybernow-webos (cœur client apps/core et couche webOS apps/webos), avec les
# correctifs KyberNow de kyclient (apps/macos/kyber-patches), libkynput (sans
# SDL : les manettes passent par l'app), libopus et les fonctions vides de VLC.
# Puis le paquet .ipk (scripts/package.sh), sauf avec --no-package.
#
#   apps/webos/scripts/build.sh               binaire et paquet
#   apps/webos/scripts/build.sh --check       cargo check seul
#   apps/webos/scripts/build.sh --debug       sans optimisation
#   apps/webos/scripts/build.sh --no-package  binaire seul
#
# Chaîne de compilation : celle de webosbrew (openlgtv/buildroot-nc4, gcc 16,
# glibc 2.12, armv7 softfp), dans third_party/webos-toolchain ou WEBOS_TOOLCHAIN ;
# téléchargée si absente (Linux x86_64). Cible Rust armv7-unknown-linux-gnueabi.
#
# S'arrête à la première erreur sans toucher au binaire déjà en place.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
mkdir -p "$ROOT/third_party"
THIRD_PARTY="$(cd "$ROOT/third_party" && pwd -P)"
KYBER="$THIRD_PARTY/kyber-desktop"
KYSDK="$KYBER/kysdk"
OPUS_SRC="$THIRD_PARTY/opus-1.6.1"
TARGET=armv7-unknown-linux-gnueabi
PREFIX="$KYBER/rootfs-$TARGET"
TOOLCHAIN_URL="https://github.com/openlgtv/buildroot-nc4/releases/latest/download/arm-webos-linux-gnueabi_sdk-buildroot-x86_64.tar.gz"
. "$ROOT/scripts/kyber-toolchain.sh"

COMMAND=build
PROFILE=release
PACKAGE=1
while [ $# -gt 0 ]; do
    case "$1" in
        --check) COMMAND=check; PACKAGE=0 ;;
        --debug) PROFILE=debug ;;
        --no-package) PACKAGE=0 ;;
        *) echo "Option inconnue : $1" >&2; exit 1 ;;
    esac
    shift
done

[ -d "$KYSDK/kyctl" ] || { echo "SDK Kyber introuvable dans $KYSDK (voir docs/build-notes.md)" >&2; exit 1; }

# La chaîne de webosbrew.
if [ -z "${WEBOS_TOOLCHAIN:-}" ]; then
    WEBOS_TOOLCHAIN="$THIRD_PARTY/webos-toolchain/arm-webos-linux-gnueabi_sdk-buildroot"
    if [ ! -x "$WEBOS_TOOLCHAIN/bin/arm-webos-linux-gnueabi-gcc" ]; then
        [ "$(uname -s)-$(uname -m)" = Linux-x86_64 ] || {
            echo "Chaîne webOS absente : téléchargez celle de votre système (github.com/webosbrew/native-toolchain) et posez WEBOS_TOOLCHAIN" >&2
            exit 1
        }
        echo "==> chaîne webOS dans $THIRD_PARTY/webos-toolchain"
        mkdir -p "$THIRD_PARTY/webos-toolchain"
        curl -fsSL "$TOOLCHAIN_URL" | tar xz -C "$THIRD_PARTY/webos-toolchain"
        (cd "$WEBOS_TOOLCHAIN" && ./relocate-sdk.sh >/dev/null)
    fi
fi
export WEBOS_TOOLCHAIN
SYSROOT="$WEBOS_TOOLCHAIN/arm-webos-linux-gnueabi/sysroot"
export WEBOS_SYSROOT="$SYSROOT"

# SDL2 de webosbrew (2.30.12-webos.6, comme moonlight-tv) : la SDL du
# téléviseur est ancienne et sans les réglages de webOS (touches Retour et
# Accueil, manettes en jeu). Compilée une fois, embarquée dans le paquet.
SDL_TAG=release-2.30.12-webos.6
SDL_SRC="$THIRD_PARTY/sdl-webos"
export SDL_PREFIX="$THIRD_PARTY/sdl-webos-dist"
if [ ! -f "$SDL_PREFIX/lib/libSDL2-2.0.so.0" ]; then
    [ -f "$SDL_SRC/CMakeLists.txt" ] || git clone -q --depth 1 --branch "$SDL_TAG" https://github.com/webosbrew/SDL-webOS.git "$SDL_SRC"
    echo "==> SDL2 de webOS ($SDL_TAG) dans $SDL_PREFIX"
    cmake -S "$SDL_SRC" -B "$SDL_SRC/build" -DCMAKE_TOOLCHAIN_FILE="$WEBOS_TOOLCHAIN/share/buildroot/toolchainfile.cmake" \
        -DCMAKE_BUILD_TYPE=Release -DWEBOS=ON -DSDL_OFFSCREEN=OFF -DSDL_DISKAUDIO=OFF -DSDL_DUMMYAUDIO=OFF \
        -DSDL_DUMMYVIDEO=OFF -DSDL_KMSDRM=OFF -DSDL_VENDOR_INFO="webOS Backport" -DSDL_WEBOS_BROKEN_ABI=OFF \
        -DSDL_TEST=OFF -DSDL_STATIC=OFF -DCMAKE_INSTALL_PREFIX="$SDL_PREFIX" >/dev/null
    cmake --build "$SDL_SRC/build" -j"$(nproc 2>/dev/null || echo 4)" >/dev/null
    cmake --install "$SDL_SRC/build" >/dev/null
    [ -f "$SDL_PREFIX/lib/libSDL2-2.0.so.0" ] || { echo "SDL2 de webOS non construite" >&2; exit 1; }
fi

export PATH="$HOME/.cargo/bin:$PATH"
rustup target list --installed 2>/dev/null | grep -qx "$TARGET" || {
    echo "Cible Rust $TARGET absente : rustup target add $TARGET" >&2
    exit 1
}
kyber_toolchain_check rustc

CC_SHIM="$HERE/scripts/cc.sh"
export CC_armv7_unknown_linux_gnueabi="$CC_SHIM"
export CXX_armv7_unknown_linux_gnueabi="$CC_SHIM"
export AR_armv7_unknown_linux_gnueabi="$WEBOS_TOOLCHAIN/bin/arm-webos-linux-gnueabi-gcc-ar"
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABI_LINKER="$CC_SHIM"
# Vrais registres VFP et NEON (la cible Rust « gnueabi » émule le calcul en
# virgule flottante sinon : dix fois plus lent), Cortex-A73 (puces LG depuis
# 2020) ; kynput en client seul (kybernow_webos) ; getrandom 0.3 par l'appel système direct (pas de /dev/urandom dans
# le bac à sable de l'app).
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABI_RUSTFLAGS="-C target-feature=+neon,+vfp3,-soft-float -C target-cpu=cortex-a73 --cfg getrandom_backend=\"linux_raw\" --cfg kybernow_webos"
export BINDGEN_EXTRA_CLANG_ARGS_armv7_unknown_linux_gnueabi="--target=arm-linux-gnueabi --sysroot=$SYSROOT"
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR=
export CARGO_PROFILE_RELEASE_STRIP=false

cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing) ' || true; }
}

# Mêmes correctifs que les autres apps (image et son vers l'app, presse-papiers,
# mémoire de réception UDP).
# Puis ceux de webOS (apps/webos/kyber-patches : kynput en client seul).
for patch in "$ROOT"/apps/macos/kyber-patches/*.patch "$HERE"/kyber-patches/*.patch; do
    case "$(basename "$patch")" in
        *-kyclient-*) repo="$KYSDK/kyctl" ;;
        *-kynput-*) repo="$KYSDK/kynput" ;;
        *-kynet-*) repo="$KYSDK/kymux" ;;
        *) echo "Dépôt inconnu pour $(basename "$patch")" >&2; exit 1 ;;
    esac
    if git -C "$repo" apply --check "$patch" 2>/dev/null; then
        git -C "$repo" apply "$patch" && echo "==> $(basename "$patch") appliqué"
    elif ! git -C "$repo" apply --reverse --check "$patch" 2>/dev/null; then
        echo "Correctif $(basename "$patch") inapplicable :" >&2
        git -C "$repo" apply --check "$patch" >&2 || true
        exit 1
    fi
done

# libkynput (API C de kynput, sans SDL), statique.
if [ ! -f "$PREFIX/lib/pkgconfig/kynput.pc" ]; then
    kyber_toolchain_check cargo-c
    echo "==> libkynput ($TARGET) dans $PREFIX"
    mkdir -p "$PREFIX"
    (cd "$KYSDK/kynput" && cargo_quiet cargo cinstall -p kynput --target="$TARGET" \
        --release --library-type=staticlib --prefix="$PREFIX")
    [ -f "$PREFIX/lib/pkgconfig/kynput.pc" ] || { echo "libkynput non installée dans $PREFIX" >&2; exit 1; }
fi

# libopus 1.6.1 (la version de Kyber 0.29), statique.
if [ ! -f "$PREFIX/lib/libopus.a" ]; then
    if [ ! -f "$OPUS_SRC/CMakeLists.txt" ]; then
        echo "==> sources d'Opus 1.6.1 dans $OPUS_SRC"
        git clone -q --depth 1 --branch v1.6.1 https://github.com/xiph/opus.git "$OPUS_SRC"
    fi
    echo "==> libopus ($TARGET) dans $PREFIX"
    build="$(mktemp -d "${TMPDIR:-/tmp}/kybernow-opus.XXXXXX")"
    cmake -S "$OPUS_SRC" -B "$build" -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_TOOLCHAIN_FILE="$WEBOS_TOOLCHAIN/share/buildroot/toolchainfile.cmake" \
        -DCMAKE_INSTALL_PREFIX="$PREFIX" -DCMAKE_INSTALL_LIBDIR=lib -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DOPUS_BUILD_PROGRAMS=OFF -DOPUS_BUILD_TESTING=OFF \
        -DOPUS_BUILD_SHARED_LIBRARY=OFF -DOPUS_DRED=OFF -DOPUS_OSCE=OFF >/dev/null
    cmake --build "$build" --target install -j"$(nproc 2>/dev/null || echo 4)" >/dev/null
    rm -rf "$build"
    [ -f "$PREFIX/lib/libopus.a" ] || { echo "libopus non construite" >&2; exit 1; }
fi

# libvlc.pc sans bibliothèque (fonctions vides dans le binaire, build.rs).
if [ ! -f "$PREFIX/lib/pkgconfig/libvlc.pc" ]; then
    mkdir -p "$PREFIX/lib/pkgconfig"
    cat > "$PREFIX/lib/pkgconfig/libvlc.pc" <<'PC'
# Écrit par apps/webos/scripts/build.sh : VLC n'est jamais appelé sur webOS,
# ses fonctions sont fournies vides par le binaire.
Name: libvlc
Description: VLC absent (fonctions vides)
Version: 4.0.0
Libs:
Cflags:
PC
fi

flags=()
[ "$PROFILE" = release ] && flags+=(--release)
echo "==> cargo $COMMAND kybernow-webos ($TARGET, $PROFILE)"
started=$(date +%s)
(cd "$APPS" && cargo_quiet cargo "$COMMAND" -p kybernow-webos "${flags[@]}" --target "$TARGET")
[ "$COMMAND" = build ] || { echo "    vérifié ($(( $(date +%s) - started )) s)"; exit 0; }
bin="$APPS/target/$TARGET/$PROFILE/kybernow-webos"
[ -f "$bin" ] || { echo "$bin absent" >&2; exit 1; }
echo "    $bin ($(du -h "$bin" | cut -f1), $(( $(date +%s) - started )) s)"
[ "$PACKAGE" = 1 ] && "$HERE/scripts/package.sh" "$PROFILE"
exit 0
