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
#
#   aarch64-linux-android       arm64-v8a : téléphones, tablettes, Shield, Google TV Streamer
#   armv7-linux-androideabi     armeabi-v7a : Chromecast avec Google TV et
#                               téléviseurs dont Android est en 32 bits
#   x86_64-linux-android        x86_64 : émulateur
#
# Android 11 au moins (API 30, la Shield). NDK r27c (27.2.12479018) :
# ANDROID_NDK_HOME, sinon $ANDROID_HOME/ndk/27.2.12479018, sinon le SDK
# d'Android Studio (~/Library/Android/sdk sur Mac, ~/Android/Sdk sur Linux).
# Chaque cible a son rootfs third_party/kyber-desktop/rootfs-<cible> :
# libkynput, libopus et leurs .pc (lus par kynput-sys et le cœur), et un
# libvlc.pc sans bibliothèque. Les sources d'Opus 1.6.1 sont prises dans
# third_party/opus-1.6.1 (clonées depuis GitHub si absentes).
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
API=30
NDK_VERSION=27.2.12479018
. "$ROOT/scripts/kyber-toolchain.sh"

ALL_TARGETS=(aarch64-linux-android armv7-linux-androideabi x86_64-linux-android)
TARGETS=()
COMMAND=build
PROFILE=release
while [ $# -gt 0 ]; do
    case "$1" in
        --target) TARGETS+=("${2:-}"); shift ;;
        --check) COMMAND=check ;;
        --debug) PROFILE=debug ;;
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
    export PKG_CONFIG_LIBDIR="$KYBER/rootfs-$target/lib/pkgconfig"
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
for patch in "$ROOT"/apps/macos/kyber-patches/*.patch; do
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

build_target() {
    local target=$1 prefix started abi lib flags=()
    prefix="$KYBER/rootfs-$target"
    use_target "$target"
    ensure_kynput "$target" "$prefix"
    ensure_opus "$target" "$prefix"
    ensure_libvlc_pc "$prefix"
    [ "$PROFILE" = release ] && flags+=(--release)
    echo "==> cargo $COMMAND kybernow-android ($target, API $API, $PROFILE)"
    started=$(date +%s)
    (cd "$APPS" && cargo_quiet cargo "$COMMAND" -p kybernow-android --lib "${flags[@]}" --target "$target")
    [ "$COMMAND" = build ] || { echo "    $target vérifiée ($(( $(date +%s) - started )) s)"; return 0; }
    lib="$APPS/target/$target/$PROFILE/libkybernow.so"
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
