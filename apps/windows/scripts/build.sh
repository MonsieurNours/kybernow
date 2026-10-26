#!/bin/bash
# Compile l'app Windows de KyberNow depuis macOS et assemble le dossier à
# copier sur le PC : apps/windows/dist/<arch>/ (kybernow.exe et ses DLL).
#
#   apps/windows/scripts/build.sh                 x64 (x86_64-pc-windows-gnu)
#   apps/windows/scripts/build.sh --arch arm64    ARM64 (aarch64-pc-windows-gnullvm)
#   apps/windows/scripts/build.sh --debug         sans optimisation
#
# Prérequis (docs/build-notes.md) : third_party/ en place, les correctifs
# client appliqués par ce script ; pour x64, third_party/rootfs-win64 (FFmpeg,
# libopus) produit par server/kyber-patches/build.sh et la SDL2.dll de Kyber
# 0.29 (third_party/payload) ; pour ARM64, third_party/rootfs-winarm64 (FFmpeg,
# libopus, SDL2) produit par scripts/build-arm64-deps.sh (llvm-mingw). VLC
# n'est pas lié : l'app décode elle-même, ses fonctions sont remplacées par des
# fonctions vides (comme sur l'Apple TV).
#
# kynput.dll est celle du client, construite ici (pas celle du serveur) : sans
# le lecteur de manettes SDL de kynput ni son presse-papiers Windows (cfg
# kybernow_app_clipboard, correctif 0004). Plusieurs ordinateurs à la fois :
# le cœur lit les manettes une seule fois pour toutes les sessions
# (apps/core/src/gamepads, lié à SDL2.dll) et tient un presse-papiers par
# session (apps/core/src/input/clipboard.rs, sur celui de l'app).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYSDK="$THIRD_PARTY/kyber-desktop/kysdk"
. "$ROOT/scripts/kyber-toolchain.sh"

PROFILE=release
ARCH=x64
while [ $# -gt 0 ]; do
    case "$1" in
        --debug) PROFILE=debug ;;
        --arch) ARCH="${2:-}"; shift ;;
        *) echo "Option inconnue : $1" >&2; exit 1 ;;
    esac
    shift
done

# Symboles de libvlc.dll (les mêmes pour toutes les architectures).
VLC_IMPORT="$THIRD_PARTY/rootfs-win64/lib/libvlc.dll.a"
case "$ARCH" in
    x64)
        TARGET=x86_64-pc-windows-gnu
        TRIPLE=x86_64-w64-mingw32
        MINGW=/opt/homebrew/opt/mingw-w64/toolchain-x86_64/$TRIPLE
        ROOTFS="$THIRD_PARTY/rootfs-win64"
        # pkg-config : rien d'autre à lire dans le rootfs du serveur.
        ROOTFS_PC=""
        SDL2_DLL="$THIRD_PARTY/payload/kyber-0.29.0/SDL2.dll"
        # Lien vers SDL2 : l'éditeur de liens de MinGW lie la DLL elle-même.
        SDL2_LINK="$SDL2_DLL"
        CC="$TRIPLE-gcc"; AR="$TRIPLE-ar"; OBJDUMP="$TRIPLE-objdump"; STRIP="$TRIPLE-strip"
        DLLTOOL="$TRIPLE-dlltool"
        RUNTIME_DIRS=("$MINGW/bin" "/opt/homebrew/opt/mingw-w64/toolchain-x86_64/lib/gcc/$TRIPLE/$KYBER_MINGW_GCC" "$MINGW/lib")
        DEPS_HINT="server/kyber-patches/build.sh"
        export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
        ;;
    arm64)
        TARGET=aarch64-pc-windows-gnullvm
        TRIPLE=aarch64-w64-mingw32
        LLVM_MINGW="$THIRD_PARTY/toolchains/llvm-mingw-$KYBER_LLVM_MINGW-ucrt-macos-universal"
        MINGW="$LLVM_MINGW/$TRIPLE"
        ROOTFS="$THIRD_PARTY/rootfs-winarm64"
        ROOTFS_PC="$ROOTFS/lib/pkgconfig"
        SDL2_DLL="$ROOTFS/bin/SDL2.dll"
        # lld ne lie pas une DLL : sa bibliothèque d'import.
        SDL2_LINK="$ROOTFS/lib/libSDL2.dll.a"
        CC="$TRIPLE-clang"; AR=llvm-ar; OBJDUMP=llvm-objdump; STRIP=llvm-strip
        DLLTOOL=llvm-dlltool
        RUNTIME_DIRS=("$MINGW/bin")
        DEPS_HINT="apps/windows/scripts/build-arm64-deps.sh"
        [ -x "$LLVM_MINGW/bin/$CC" ] || { echo "llvm-mingw $KYBER_LLVM_MINGW absent de third_party/toolchains (docs/build-notes.md)" >&2; exit 1; }
        export PATH="$LLVM_MINGW/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
        ;;
    *) echo "Architecture inconnue : $ARCH (x64 ou arm64)" >&2; exit 1 ;;
esac
STUBS="$THIRD_PARTY/windows-client-$ARCH"
DIST="$HERE/dist/$ARCH"

for f in "$ROOTFS/lib/libavcodec.a" "$ROOTFS/lib/libopus.a" "$SDL2_DLL" "$SDL2_LINK" "$VLC_IMPORT"; do
    [ -f "$f" ] || { echo "$f introuvable : lancez d'abord $DEPS_HINT" >&2; exit 1; }
done

kyber_toolchain_check rustc cargo-c
[ "$ARCH" = x64 ] && kyber_toolchain_check mingw

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
if [ ! -f "$STUBS/libvlc.a" ] || [ "$VLC_IMPORT" -nt "$STUBS/libvlc.a" ]; then
    echo "==> Fonctions vides de VLC"
    x86_64-w64-mingw32-nm "$VLC_IMPORT" | awk '$2 == "T" && $3 ~ /^libvlc_/ {print "void *" $3 "(void) { return 0; }"}' | sort -u > "$STUBS/vlc-stubs.c"
    "$CC" -O2 -c "$STUBS/vlc-stubs.c" -o "$STUBS/vlc-stubs.o"
    rm -f "$STUBS/libvlc.a"
    "$AR" rcs "$STUBS/libvlc.a" "$STUBS/vlc-stubs.o"
fi
printf 'Name: libvlc\nDescription: fonctions vides (KyberNow)\nVersion: 3.0.0\nLibs: -L%s -lvlc\nCflags:\n' "$STUBS" > "$STUBS/pkgconfig/libvlc.pc"

# SDL2 pour le lecteur de manettes du cœur (apps/core/build.rs, feature sdl),
# dans un dossier qui ne contient qu'elle (celui de Kyber contient aussi
# libvlc.dll, qui prendrait la place des fonctions vides).
mkdir -p "$STUBS/sdl2"
rm -f "$STUBS/sdl2"/*
ln -s "$SDL2_LINK" "$STUBS/sdl2/"
printf 'Name: sdl2\nDescription: SDL2 (manettes, lecteur du cœur)\nVersion: 2.0.0\nLibs: -L%s -lSDL2\nCflags:\n' "$STUBS/sdl2" > "$STUBS/pkgconfig/sdl2.pc"

# kynput.dll du client et son kynput.pc (lu par kynput-sys).
CLIENT_KYNPUT="$STUBS/kynput"

export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR="$STUBS/pkgconfig:$CLIENT_KYNPUT/lib/pkgconfig${ROOTFS_PC:+:$ROOTFS_PC}"
export LIBCLANG_PATH="${LIBCLANG_PATH:-/Library/Developer/CommandLineTools/usr/lib}"
export BINDGEN_EXTRA_CLANG_ARGS="--target=$TARGET --sysroot=$MINGW -I$MINGW/include"
TARGET_ENV=$(echo "$TARGET" | tr 'a-z-' 'A-Z_')
export "CARGO_TARGET_${TARGET_ENV}_LINKER=$CC"
export "CC_${TARGET//-/_}=$CC"
export "AR_${TARGET//-/_}=$AR"
# FFmpeg et libopus statiques, lus par apps/windows/build.rs, dans un dossier
# à part : celui du rootfs contient aussi libvlc.dll.a.
NATIVE_LIB="$STUBS/native"
rm -rf "$NATIVE_LIB"
mkdir -p "$NATIVE_LIB"
# x264, VPL et zlib : seulement dans la libavcodec du serveur (x64).
for lib in avcodec swresample avutil opus x264 vpl z; do
    [ -f "$ROOTFS/lib/lib$lib.a" ] && ln -s "$ROOTFS/lib/lib$lib.a" "$NATIVE_LIB/lib$lib.a"
done
export KYBERNOW_NATIVE_PREFIX="$ROOTFS"
export KYBERNOW_NATIVE_LIB="$NATIVE_LIB"

# Rien d'un build précédent ne doit survivre à un échec de celui-ci.
rm -rf "$DIST"

# kynput.dll du client : sans le lecteur de manettes SDL de kynput (feature
# sdl2 absente) ni son presse-papiers Windows (--cfg kybernow_app_clipboard),
# dans son propre dossier de compilation (celui de kysdk/kynput sert au
# serveur, avec d'autres options).
echo "==> kynput.dll du client ($ARCH, sans manettes SDL ni presse-papiers de kynput)"
rm -f "$CLIENT_KYNPUT/bin/kynput.dll"
(cd "$KYSDK/kynput" && env "CARGO_TARGET_${TARGET_ENV}_RUSTFLAGS=--cfg kybernow_app_clipboard" CARGO_PROFILE_RELEASE_STRIP=false \
    cargo cinstall -p kynput --target "$TARGET" --release --library-type cdylib --dlltool "$DLLTOOL" \
    --prefix="$CLIENT_KYNPUT" --target-dir "$STUBS/kynput-build" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing) ' || true; })
[ -f "$CLIENT_KYNPUT/bin/kynput.dll" ] || { echo "kynput.dll n'a pas été produite" >&2; exit 1; }

# Interface (Svelte), embarquée dans l'exécutable par Tauri.
echo "==> Interface (ui/)"
(cd "$HERE/ui" && npm ci --silent && npm run build --silent)

cargo_flags=(--target "$TARGET" -p kybernow-windows)
[ "$PROFILE" = release ] && cargo_flags+=(--release)
echo "==> cargo build ($ARCH, $PROFILE)"
(cd "$APPS" && cargo build "${cargo_flags[@]}" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating) ' || true; })

EXE="$APPS/target/$TARGET/$PROFILE/kybernow.exe"
[ -f "$EXE" ] || { echo "kybernow.exe absent" >&2; exit 1; }

echo "==> $DIST"
rm -rf "$DIST"
mkdir -p "$DIST"
cp "$CLIENT_KYNPUT/bin/kynput.dll" "$SDL2_DLL" "$DIST/"
# Icône des notifications de Windows (inscrite par l'app au lancement).
cp "$HERE/resources/kybernow.png" "$DIST/"
# Chargeur de WebView2 (l'interface), celui de la version de webview2-com-sys
# du Cargo.lock.
WV2_VERSION=$(awk '/^name = "webview2-com-sys"$/ {getline; gsub(/version = |"/, ""); print}' "$APPS/Cargo.lock")
case "$ARCH" in x64) WV2_ARCH=x64 ;; arm64) WV2_ARCH=arm64 ;; esac
WV2_LOADER=$(ls -d "$HOME"/.cargo/registry/src/index.crates.io-*/webview2-com-sys-"$WV2_VERSION"/$WV2_ARCH/WebView2Loader.dll | head -1)
[ -f "$WV2_LOADER" ] || { echo "WebView2Loader.dll ($WV2_ARCH, webview2-com-sys $WV2_VERSION) introuvable" >&2; exit 1; }
cp "$WV2_LOADER" "$DIST/"
# Sans les informations de débogage (gardées dans target/).
"$STRIP" -o "$DIST/kybernow.exe" "$EXE"
if "$OBJDUMP" -p "$DIST/kybernow.exe" | grep -qi "DLL Name: libvlc"; then
    echo "kybernow.exe dépend de libvlc.dll : les fonctions vides de VLC n'ont pas été liées" >&2
    exit 1
fi
# Runtime de la chaîne (MinGW, libunwind et libc++ de llvm-mingw) demandé par
# l'exécutable ou les DLL.
for dll in $("$OBJDUMP" -p "$DIST/kybernow.exe" "$DIST/kynput.dll" "$DIST/SDL2.dll" | awk '/DLL Name:/ {print $3}' | sort -u); do
    for dir in "${RUNTIME_DIRS[@]}"; do
        if [ -f "$dir/$dll" ]; then cp "$dir/$dll" "$DIST/"; break; fi
    done
done
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$APPS/Cargo.toml" | head -1)
# L'exécutable non allégé, gardé par version et architecture (hors de
# dist/<arch>/, vidé à chaque build) : il donne la ligne d'un plantage d'une
# version publiée (llvm-addr2line). Un nouveau build de la même version le
# remplace.
if [ "$PROFILE" = release ]; then
    mkdir -p "$HERE/dist/symbols"
    cp "$EXE" "$HERE/dist/symbols/kybernow-$VERSION-windows-$ARCH.exe"
    echo "==> dist/symbols/kybernow-$VERSION-windows-$ARCH.exe"
fi
ZIP="$HERE/dist/KyberNow-$VERSION-windows-$ARCH.zip"
rm -f "$ZIP"
(cd "$HERE/dist" && zip -qr "$ZIP" "$ARCH")
ls -la "$DIST"
echo "==> $ZIP"
