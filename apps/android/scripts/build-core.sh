#!/bin/bash
# Compile le cœur client (kybernow-core, apps/core) pour Android, avec les
# correctifs KyberNow de kyclient (apps/macos/kyber-patches), et laisse les
# artefacts dans apps/target/<cible>/release. Comme apps/core/scripts/build-apple.sh
# pour les cibles Apple : vérifier que toute la chaîne de dépendances (kyclient
# avec kymux, kynput-rs, kyvlcplayer, rustls, ureq…) compile pour Android, et
# construire libkynput (sans SDL : les manettes passent par l'app).
#
#   apps/android/scripts/build-core.sh                      toutes les cibles
#   apps/android/scripts/build-core.sh --target <cible>     une seule
#   apps/android/scripts/build-core.sh --check              cargo check seul
#
#   aarch64-linux-android       téléphones, tablettes, Shield, Google TV Streamer
#   armv7-linux-androideabi     Chromecast avec Google TV et téléviseurs dont
#                               Android est en 32 bits (puce 64 bits ou non)
#   x86_64-linux-android        émulateur
#
# Android 11 au moins (API 30, la Shield). NDK r27c (27.2.12479018) :
# ANDROID_NDK_HOME, sinon $ANDROID_HOME/ndk/27.2.12479018, sinon le SDK
# d'Android Studio (~/Library/Android/sdk sur Mac, ~/Android/Sdk sur Linux).
# Chaque cible a son rootfs third_party/kyber-desktop/rootfs-<cible> avec
# libkynput et kynput.pc (kynput-sys les lit par pkg-config et bindgen).
#
# S'arrête à la première erreur (correctif inapplicable, compilation ratée).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
# Chemin réel de third_party/ (un lien dans un worktree) : cargo enregistre les
# chemins absolus des sources, un autre chemin lui fait tout recompiler.
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYBER="$THIRD_PARTY/kyber-desktop"
KYSDK="$KYBER/kysdk"
API=30
NDK_VERSION=27.2.12479018
. "$ROOT/scripts/kyber-toolchain.sh"

ALL_TARGETS=(aarch64-linux-android armv7-linux-androideabi x86_64-linux-android)
TARGETS=()
COMMAND=build
while [ $# -gt 0 ]; do
    case "$1" in
        --target) TARGETS+=("${2:-}"); shift ;;
        --check) COMMAND=check ;;
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
    for candidate in "${ANDROID_NDK_HOME:-}" "${ANDROID_HOME:+$ANDROID_HOME/ndk/$NDK_VERSION}" \
        "$HOME/Library/Android/sdk/ndk/$NDK_VERSION" "$HOME/Android/Sdk/ndk/$NDK_VERSION"; do
        [ -n "$candidate" ] && [ -f "$candidate/source.properties" ] && { echo "$candidate"; return 0; }
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
# bindgen (kynput-sys) : la libclang du NDK, qui connaît les cibles Android.
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

build_target() {
    local target=$1 prefix started
    prefix="$KYBER/rootfs-$target"
    use_target "$target"
    ensure_kynput "$target" "$prefix"
    echo "==> cargo $COMMAND ($target, API $API)"
    started=$(date +%s)
    (cd "$APPS" && cargo_quiet cargo "$COMMAND" -p kybernow-core --release --target "$target")
    if [ "$COMMAND" = build ]; then
        local rlib="$APPS/target/$target/release/libkybernow_core.rlib"
        [ -f "$rlib" ] || { echo "$rlib absent" >&2; exit 1; }
        echo "    $rlib ($(du -h "$rlib" | cut -f1), $(( $(date +%s) - started )) s)"
    else
        echo "    $target vérifiée ($(( $(date +%s) - started )) s)"
    fi
}

for target in "${TARGETS[@]}"; do
    build_target "$target"
done
echo "kybernow-core : ${TARGETS[*]}"
