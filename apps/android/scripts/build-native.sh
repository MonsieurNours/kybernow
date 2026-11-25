#!/bin/bash
# Compile la bibliothèque native de l'app Android, libkybernow.so : le cœur
# client (kybernow-core, apps/core) et son pont JNI (kybernow-android,
# apps/android/jni), avec les correctifs KyberNow de kyclient
# (apps/macos/kyber-patches), libkynput (sans SDL : les manettes passent par
# l'app), libopus et les fonctions vides de VLC. Chaque bibliothèque va dans
# apps/android/app/src/main/jniLibs/<abi>/, que Gradle embarque.
#
#   apps/android/scripts/build-native.sh                      toutes les cibles
#   apps/android/scripts/build-native.sh --target <cible>     une seule
#   apps/android/scripts/build-native.sh --check              cargo check seul
#   apps/android/scripts/build-native.sh --debug              sans optimisation
#   apps/android/scripts/build-native.sh --no-pyrowave        sans PyroWave
#   apps/android/scripts/build-native.sh --kyber-pyrowave     Kyber demande PyroWave
#
#   aarch64-linux-android       arm64-v8a : téléphones, tablettes, Shield, Google TV Streamer
#   armv7-linux-androideabi     armeabi-v7a : Chromecast avec Google TV et
#                               téléviseurs dont Android est en 32 bits
#   x86_64-linux-android        x86_64 : émulateur
#
# Android 11 au moins (API 30, la Shield). NDK r27c (27.2.12479018) :
# ANDROID_NDK_HOME, sinon $ANDROID_HOME/ndk/27.2.12479018, sinon le SDK
# d'Android Studio (~/Library/Android/sdk sur Mac, ~/Android/Sdk sur Linux).
# Chaque cible a son rootfs third_party/kyber-desktop/rootfs-<cible> (ou
# $KYBERNOW_ANDROID_ROOTFS/rootfs-<cible> : un rootfs à part, pour construire
# sans toucher à celui que partagent les autres constructions) :
# libkynput, libopus et leurs .pc (lus par kynput-sys et le cœur), et un
# libvlc.pc sans bibliothèque. Les sources d'Opus 1.6.1 sont prises dans
# third_party/opus-1.6.1 (clonées depuis GitHub si absentes).
#
# PyroWave (docs/pyrowave-client-android.md) : arm64-v8a et x86_64 ont aussi
# libkybernow_pyrowave.so (PyroWave au commit épinglé de
# server/kyber-patches/pyrowave.sh, sources dans third_party/pyrowave, Granite
# partiel, et le pont Vulkan d'apps/android/jni/pyrowave), construite par CMake
# avec la chaîne du NDK et le glslc du NDK ; libkybernow.so s'y lie
# (fonctionnalité `pyrowave` de kybernow-android). armeabi-v7a : sans PyroWave
# (la sonde dit « indisponible »).
# Le correctif kyclient de PyroWave (0025, `VideoCodec::PyroWave`,
# `fallback_codec`) et la fonctionnalité `pyrowave` du cœur vont ensemble
# (apps/core/README.md, « PyroWave ») : seulement avec --kyber-pyrowave, tant
# qu'ils ne sont pas allumés pour toutes les apps (kyber-desktop est partagé).
#
# S'arrête à la première erreur (correctif inapplicable, compilation ratée)
# sans toucher aux bibliothèques déjà en place.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
# Chemin réel de third_party/ (un lien dans un worktree) : cargo enregistre les
# chemins absolus des sources, un autre chemin lui fait tout recompiler.
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYBER="$THIRD_PARTY/kyber-desktop"
KYSDK="$KYBER/kysdk"
OPUS_SRC="$THIRD_PARTY/opus-1.6.1"
JNILIBS="$HERE/app/src/main/jniLibs"
ROOTFS_BASE="${KYBERNOW_ANDROID_ROOTFS:-$KYBER}"
API=30
NDK_VERSION=27.2.12479018
. "$ROOT/scripts/kyber-toolchain.sh"

ALL_TARGETS=(aarch64-linux-android armv7-linux-androideabi x86_64-linux-android)
TARGETS=()
COMMAND=build
PROFILE=release
PYROWAVE=1
KYBER_PYROWAVE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --target) TARGETS+=("${2:-}"); shift ;;
        --check) COMMAND=check ;;
        --debug) PROFILE=debug ;;
        --no-pyrowave) PYROWAVE=0 ;;
        --kyber-pyrowave) KYBER_PYROWAVE=1 ;;
        *) echo "Option inconnue : $1" >&2; exit 1 ;;
    esac
    shift
done
[ ${#TARGETS[@]} -gt 0 ] || TARGETS=("${ALL_TARGETS[@]}")
for target in "${TARGETS[@]}"; do
    case " ${ALL_TARGETS[*]} " in
        *" $target "*) ;;
        *) echo "Cible inconnue : $target (${ALL_TARGETS[*]})" >&2; exit 1 ;;
    esac
done

[ -d "$KYSDK/kyctl" ] || { echo "SDK Kyber introuvable dans $KYSDK (voir docs/build-notes.md)" >&2; exit 1; }

# Le NDK : variable, SDK déclaré, ou SDK d'Android Studio.
find_ndk() {
    local candidate
    local sdk
    for candidate in "${ANDROID_NDK_HOME:-}" "${ANDROID_HOME:+$ANDROID_HOME/ndk/$NDK_VERSION}" \
        "$HOME/Library/Android/sdk/ndk/$NDK_VERSION" "$HOME/Android/Sdk/ndk/$NDK_VERSION"; do
        [ -n "$candidate" ] && [ -f "$candidate/source.properties" ] && { echo "$candidate"; return 0; }
    done
    # Sinon le plus récent des NDK installés par Android Studio (version signalée plus bas).
    for sdk in "${ANDROID_HOME:-}" "$HOME/Library/Android/sdk" "$HOME/Android/Sdk"; do
        [ -n "$sdk" ] && [ -d "$sdk/ndk" ] || continue
        candidate=$(find -L "$sdk/ndk" -mindepth 1 -maxdepth 1 -type d -exec test -f '{}/source.properties' ';' -print | sort -V | tail -1)
        [ -n "$candidate" ] && { echo "$candidate"; return 0; }
    done
    return 1
}
NDK="$(find_ndk)" || {
    echo "NDK d'Android $NDK_VERSION introuvable : ANDROID_NDK_HOME, ou Android Studio › SDK Manager › SDK Tools › NDK (Side by side) $NDK_VERSION" >&2
    exit 1
}
have_ndk=$(awk -F' *= *' '/^Pkg.Revision/ {print $2}' "$NDK/source.properties")
[ "$have_ndk" = "$NDK_VERSION" ] || echo "Attention : NDK $have_ndk, version vérifiée : $NDK_VERSION" >&2
case "$(uname -s)" in
    Darwin) HOST_TAG=darwin-x86_64 ;;
    Linux) HOST_TAG=linux-x86_64 ;;
    *) echo "Système hôte non pris en charge : $(uname -s)" >&2; exit 1 ;;
esac
LLVM="$NDK/toolchains/llvm/prebuilt/$HOST_TAG"
SYSROOT="$LLVM/sysroot"
[ -x "$LLVM/bin/clang" ] || { echo "clang du NDK absent : $LLVM/bin/clang" >&2; exit 1; }

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
# bindgen (kynput-sys) : la libclang du NDK quand il en a une (macOS), sinon celle du système.
if [ -z "${LIBCLANG_PATH:-}" ]; then
    for dir in "$LLVM/lib" "$LLVM/lib64"; do
        if compgen -G "$dir/libclang.*" >/dev/null; then export LIBCLANG_PATH="$dir"; break; fi
    done
fi
export PKG_CONFIG_ALLOW_CROSS=1

# Sortie de cargo sans les lignes de progression ; avec pipefail, le code de
# retour reste celui de cargo.
cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing) ' || true; }
}

# Triplet de clang pour chaque cible Rust (armv7 : armv7a… côté clang).
clang_triple() {
    case "$1" in
        armv7-linux-androideabi) echo "armv7a-linux-androideabi$API" ;;
        *) echo "$1$API" ;;
    esac
}

# ABI d'Android (dossier de jniLibs) de chaque cible Rust.
abi_of() {
    case "$1" in
        aarch64-linux-android) echo arm64-v8a ;;
        armv7-linux-androideabi) echo armeabi-v7a ;;
        x86_64-linux-android) echo x86_64 ;;
    esac
}

# Compilateur C, archiveur, éditeur de liens et clang de bindgen de la cible,
# sous les noms que lisent cc-rs, cargo et bindgen.
use_target() {
    local target=$1 triple var
    triple="$(clang_triple "$target")"
    var="${target//-/_}"
    export "CC_$var=$LLVM/bin/$triple-clang"
    export "CXX_$var=$LLVM/bin/$triple-clang++"
    export "AR_$var=$LLVM/bin/llvm-ar"
    export "RANLIB_$var=$LLVM/bin/llvm-ranlib"
    export "CARGO_TARGET_$(echo "$var" | tr '[:lower:]' '[:upper:]')_LINKER=$LLVM/bin/$triple-clang"
    export "BINDGEN_EXTRA_CLANG_ARGS_$var=--target=$triple --sysroot=$SYSROOT"
    export PKG_CONFIG_LIBDIR="$ROOTFS_BASE/rootfs-$target/lib/pkgconfig"
    export ANDROID_PLATFORM=$API ANDROID_NDK_HOME="$NDK"
}

kyber_toolchain_check rustc
for target in "${TARGETS[@]}"; do
    if ! rustup target list --installed 2>/dev/null | grep -qx "$target"; then
        echo "Cible Rust $target absente : rustup target add $target" >&2
        exit 1
    fi
done

# Mêmes correctifs que les autres apps (image et son vers l'app, presse-papiers,
# mémoire de réception UDP) : le cœur compile contre cette API de kyclient.
. "$ROOT/scripts/kyber-client-patches.sh"
PATCHES=("$ROOT"/apps/macos/kyber-patches/*.patch)
for patch in "$ROOT"/server/kyber-patches/*-kyclient-*.patch; do
    case "$(basename "$patch")" in
        *-kyclient-pyrowave.patch) [ "$KYBER_PYROWAVE" = 1 ] || continue ;;
    esac
    PATCHES+=("$patch")
done
kyber_apply_client_patches "$KYSDK" "${PATCHES[@]}" || exit 1

# libkynput (API C de kynput, sans SDL) dans le rootfs de la cible.
ensure_kynput() {
    local target=$1 prefix=$2
    [ -f "$prefix/lib/pkgconfig/kynput.pc" ] && return 0
    kyber_toolchain_check cargo-c
    echo "==> libkynput ($target) dans $prefix"
    mkdir -p "$prefix"
    (cd "$KYSDK/kynput" && cargo_quiet cargo cinstall -p kynput --target="$target" \
        --release --library-type=staticlib --prefix="$prefix")
    [ -f "$prefix/lib/pkgconfig/kynput.pc" ] || { echo "libkynput ($target) non installée dans $prefix" >&2; exit 1; }
}

# libopus (moteur de son du cœur), statique, dans le rootfs de la cible.
# Sources : le tag v1.6.1 du dépôt d'Opus (la version de Kyber 0.29).
ensure_opus() {
    local target=$1 prefix=$2 build
    [ -f "$prefix/lib/pkgconfig/opus.pc" ] && [ -f "$prefix/lib/libopus.a" ] && return 0
    if [ ! -f "$OPUS_SRC/CMakeLists.txt" ]; then
        echo "==> sources d'Opus 1.6.1 dans $OPUS_SRC"
        git clone -q --depth 1 --branch v1.6.1 https://github.com/xiph/opus.git "$OPUS_SRC"
    fi
    echo "==> libopus ($target) dans $prefix"
    build="$(mktemp -d "${TMPDIR:-/tmp}/kybernow-opus.XXXXXX")"
    cmake -S "$OPUS_SRC" -B "$build" -G Ninja -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_TOOLCHAIN_FILE="$NDK/build/cmake/android.toolchain.cmake" \
        -DANDROID_ABI="$(abi_of "$target")" -DANDROID_PLATFORM=$API \
        -DCMAKE_INSTALL_PREFIX="$prefix" -DOPUS_BUILD_PROGRAMS=OFF -DOPUS_BUILD_TESTING=OFF \
        -DOPUS_BUILD_SHARED_LIBRARY=OFF -DOPUS_DRED=OFF -DOPUS_OSCE=OFF >/dev/null
    cmake --build "$build" --target install >/dev/null
    rm -rf "$build"
    [ -f "$prefix/lib/libopus.a" ] || { echo "libopus ($target) non construite" >&2; exit 1; }
}

# libvlc.pc sans bibliothèque : vlc-rs (dépendance de kyvlcplayer sur Android)
# ne fait que la demander à pkg-config. VLC n'est jamais appelé (l'image et le
# son vont à l'app) : ses fonctions sont des fonctions vides compilées dans
# la bibliothèque JNI (apps/android/jni/build.rs), comme sur iOS.
ensure_libvlc_pc() {
    local prefix=$1
    [ -f "$prefix/lib/pkgconfig/libvlc.pc" ] && return 0
    mkdir -p "$prefix/lib/pkgconfig"
    cat > "$prefix/lib/pkgconfig/libvlc.pc" <<'EOF'
# Écrit par apps/android/scripts/build-native.sh : VLC n'est jamais appelé sur
# Android, ses fonctions sont fournies vides par la bibliothèque JNI.
Name: libvlc
Description: VLC absent (fonctions vides)
Version: 4.0.0
Libs:
Cflags:
EOF
}

# PyroWave : sources au commit épinglé (pyrowave_fetch de la Station).
if [ "$PYROWAVE" = 1 ]; then
    . "$ROOT/server/kyber-patches/pyrowave.sh"
    pyrowave_fetch
fi

# libkybernow_pyrowave.so pour une cible (arm64-v8a, x86_64) : dans
# target/pyrowave-android/<abi>/, puis sans symboles dans jniLibs/<abi>/.
# Rend vrai si elle est prête (la cible se lie alors à elle).
build_pyrowave() {
    local target=$1 abi build glslc
    [ "$PYROWAVE" = 1 ] || return 1
    case "$target" in
        aarch64-linux-android|x86_64-linux-android) ;;
        *) rm -f "$JNILIBS/$(abi_of "$target")/libkybernow_pyrowave.so"; return 1 ;;
    esac
    abi="$(abi_of "$target")"
    build="${CARGO_TARGET_DIR:-$APPS/target}/pyrowave-android/$abi"
    glslc="$NDK/shader-tools/$HOST_TAG/glslc"
    [ -x "$glslc" ] || { echo "glslc du NDK absent : $glslc" >&2; exit 1; }
    command -v cmake >/dev/null && command -v ninja >/dev/null || { echo "cmake et ninja nécessaires (brew install cmake ninja)" >&2; exit 1; }
    echo "==> PyroWave ($abi)"
    cmake -S "$HERE/jni/pyrowave" -B "$build" -G Ninja -DCMAKE_BUILD_TYPE=Release -Wno-dev \
        -DCMAKE_TOOLCHAIN_FILE="$NDK/build/cmake/android.toolchain.cmake" -DANDROID_ABI="$abi" \
        -DANDROID_PLATFORM=$API -DANDROID_STL=c++_static \
        -DPYROWAVE_DIR="$THIRD_PARTY/pyrowave" -DGLSLC="$glslc" >/dev/null
    cmake --build "$build" --target kybernow_pyrowave kn_pyrowave_test | { grep -E 'error|FAILED' || true; }
    [ -f "$build/libkybernow_pyrowave.so" ] || { echo "libkybernow_pyrowave.so ($abi) non construite" >&2; exit 1; }
    mkdir -p "$JNILIBS/$abi"
    "$LLVM/bin/llvm-strip" --strip-all -o "$JNILIBS/$abi/libkybernow_pyrowave.so.new" "$build/libkybernow_pyrowave.so"
    mv "$JNILIBS/$abi/libkybernow_pyrowave.so.new" "$JNILIBS/$abi/libkybernow_pyrowave.so"
    export KYBERNOW_PYROWAVE_LIB="$build"
    echo "    $abi/libkybernow_pyrowave.so ($(du -h "$JNILIBS/$abi/libkybernow_pyrowave.so" | cut -f1))"
}

build_target() {
    local target=$1 prefix started abi lib flags=()
    prefix="$ROOTFS_BASE/rootfs-$target"
    use_target "$target"
    ensure_kynput "$target" "$prefix"
    ensure_opus "$target" "$prefix"
    ensure_libvlc_pc "$prefix"
    [ "$PROFILE" = release ] && flags+=(--release)
    if build_pyrowave "$target"; then flags+=(--features pyrowave); fi
    [ "$KYBER_PYROWAVE" = 1 ] && flags+=(--features kyber-pyrowave)
    echo "==> cargo $COMMAND kybernow-android ($target, API $API, $PROFILE)"
    started=$(date +%s)
    (cd "$APPS" && cargo_quiet cargo "$COMMAND" -p kybernow-android --lib "${flags[@]}" --target "$target")
    [ "$COMMAND" = build ] || { echo "    $target vérifiée ($(( $(date +%s) - started )) s)"; return 0; }
    lib="${CARGO_TARGET_DIR:-$APPS/target}/$target/$PROFILE/libkybernow.so"
    [ -f "$lib" ] || { echo "$lib absent" >&2; exit 1; }
    abi="$(abi_of "$target")"
    mkdir -p "$JNILIBS/$abi"
    # Sans les informations de débogage dans l'APK (gardées dans target/).
    "$LLVM/bin/llvm-strip" --strip-debug -o "$JNILIBS/$abi/libkybernow.so.new" "$lib"
    mv "$JNILIBS/$abi/libkybernow.so.new" "$JNILIBS/$abi/libkybernow.so"
    echo "    $abi/libkybernow.so ($(du -h "$JNILIBS/$abi/libkybernow.so" | cut -f1), $(( $(date +%s) - started )) s)"
}

for target in "${TARGETS[@]}"; do
    build_target "$target"
done
echo "libkybernow.so : ${TARGETS[*]}"
