#!/bin/bash
# Compile l'app Windows de KyberNow depuis macOS et assemble le dossier à
# copier sur le PC : apps/windows/dist/<arch>/ (kybernow.exe et ses DLL).
#
#   apps/windows/scripts/build.sh            x64 (x86_64-pc-windows-gnu)
#   apps/windows/scripts/build.sh --debug    sans optimisation
#
# Prérequis (docs/build-notes.md) : third_party/ en place, les correctifs
# client appliqués par ce script, et third_party/rootfs-win64 (FFmpeg, libopus)
# et third_party/rootfs-win64-kynput (kynput.dll) produits par
# server/kyber-patches/build.sh. VLC n'est pas lié : l'app décode elle-même,
# ses fonctions sont remplacées par des fonctions vides (comme sur l'Apple TV).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYSDK="$THIRD_PARTY/kyber-desktop/kysdk"
. "$ROOT/scripts/kyber-toolchain.sh"

PROFILE=release
for arg in "$@"; do
    case "$arg" in
        --debug) PROFILE=debug ;;
        *) echo "Option inconnue : $arg" >&2; exit 1 ;;
    esac
done

ARCH=x64
TARGET=x86_64-pc-windows-gnu
TRIPLE=x86_64-w64-mingw32
MINGW=/opt/homebrew/opt/mingw-w64/toolchain-x86_64/$TRIPLE
ROOTFS="$THIRD_PARTY/rootfs-win64"
KYNPUT_ROOTFS="$THIRD_PARTY/rootfs-win64-kynput"
KYBER_PAYLOAD="$THIRD_PARTY/payload/kyber-0.29.0"
STUBS="$THIRD_PARTY/windows-client-$ARCH"
DIST="$HERE/dist/$ARCH"

for f in "$ROOTFS/lib/libavcodec.a" "$ROOTFS/lib/libopus.a" "$KYNPUT_ROOTFS/bin/kynput.dll" "$KYBER_PAYLOAD/SDL2.dll"; do
    [ -f "$f" ] || { echo "$f introuvable : lancez d'abord server/kyber-patches/build.sh" >&2; exit 1; }
done

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
kyber_toolchain_check rustc mingw

# Correctifs client de KyberNow (paquets vers l'app, presse-papiers, tampon de
# réception), les mêmes que pour les apps Apple.
for patch in "$APPS"/macos/kyber-patches/*.patch; do
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

# VLC : une fonction vide par export de libvlc.dll (jamais appelée tant que
# l'image et le son vont à l'app), et un libvlc.pc qui la désigne.
mkdir -p "$STUBS/pkgconfig"
if [ ! -f "$STUBS/libvlc.a" ] || [ "$ROOTFS/lib/libvlc.dll.a" -nt "$STUBS/libvlc.a" ]; then
    echo "==> Fonctions vides de VLC"
    "$TRIPLE-nm" "$ROOTFS/lib/libvlc.dll.a" | awk '$2 == "T" && $3 ~ /^libvlc_/ {print "void *" $3 "(void) { return 0; }"}' | sort -u > "$STUBS/vlc-stubs.c"
    "$TRIPLE-gcc" -O2 -c "$STUBS/vlc-stubs.c" -o "$STUBS/vlc-stubs.o"
    rm -f "$STUBS/libvlc.a"
    "$TRIPLE-ar" rcs "$STUBS/libvlc.a" "$STUBS/vlc-stubs.o"
fi
printf 'Name: libvlc\nDescription: fonctions vides (KyberNow)\nVersion: 3.0.0\nLibs: -L%s -lvlc\nCflags:\n' "$STUBS" > "$STUBS/pkgconfig/libvlc.pc"

export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR="$STUBS/pkgconfig:$KYNPUT_ROOTFS/lib/pkgconfig"
export LIBCLANG_PATH="${LIBCLANG_PATH:-/Library/Developer/CommandLineTools/usr/lib}"
export BINDGEN_EXTRA_CLANG_ARGS="--target=$TARGET --sysroot=$MINGW -I$MINGW/include"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$TRIPLE-gcc"
export CC_x86_64_pc_windows_gnu="$TRIPLE-gcc"
export AR_x86_64_pc_windows_gnu="$TRIPLE-ar"
# FFmpeg et libopus statiques, lus par apps/windows/build.rs, dans un dossier
# à part : celui du rootfs contient aussi libvlc.dll.a.
NATIVE_LIB="$STUBS/native"
rm -rf "$NATIVE_LIB"
mkdir -p "$NATIVE_LIB"
for lib in avcodec swresample avutil opus x264 vpl z; do
    ln -s "$ROOTFS/lib/lib$lib.a" "$NATIVE_LIB/lib$lib.a"
done
export KYBERNOW_NATIVE_PREFIX="$ROOTFS"
export KYBERNOW_NATIVE_LIB="$NATIVE_LIB"

cargo_flags=(--target "$TARGET" -p kybernow-windows)
[ "$PROFILE" = release ] && cargo_flags+=(--release)
echo "==> cargo build ($ARCH, $PROFILE)"
(cd "$APPS" && cargo build "${cargo_flags[@]}" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating) ' || true; })

EXE="$APPS/target/$TARGET/$PROFILE/kybernow.exe"
[ -f "$EXE" ] || { echo "kybernow.exe absent" >&2; exit 1; }

echo "==> $DIST"
rm -rf "$DIST"
mkdir -p "$DIST"
cp "$KYNPUT_ROOTFS/bin/kynput.dll" "$KYBER_PAYLOAD/SDL2.dll" "$DIST/"
# Sans les informations de débogage (gardées dans target/).
"$TRIPLE-strip" -o "$DIST/kybernow.exe" "$EXE"
if "$TRIPLE-objdump" -p "$DIST/kybernow.exe" | grep -qi "DLL Name: libvlc"; then
    echo "kybernow.exe dépend de libvlc.dll : les fonctions vides de VLC n'ont pas été liées" >&2
    exit 1
fi
# Runtime MinGW demandé par l'exécutable ou kynput.dll.
for dll in $("$TRIPLE-objdump" -p "$DIST/kybernow.exe" "$DIST/kynput.dll" | awk '/DLL Name:/ {print $3}' | sort -u); do
    for dir in "$MINGW/bin" "/opt/homebrew/opt/mingw-w64/toolchain-x86_64/lib/gcc/$TRIPLE/$KYBER_MINGW_GCC" "$MINGW/lib"; do
        if [ -f "$dir/$dll" ]; then cp "$dir/$dll" "$DIST/"; break; fi
    done
done
ls -la "$DIST"
