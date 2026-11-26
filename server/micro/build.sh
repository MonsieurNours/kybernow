#!/bin/bash
# kybernow-micro (docs/micro.md) : l'aide qui joue le micro de l'appareil dans
# le micro virtuel de la Station, installée à côté de kycontroller.
#
#   server/micro/build.sh windows <rootfs-win64> <sortie.exe>    (mingw, depuis le Mac)
#   server/micro/build.sh macos   <rootfs-arm64-macos> <sortie>
#   server/micro/build.sh linux   <rootfs-<arch>-linux> <sortie>  (dans la VM)
#
# libopus : en statique sur Windows (libopus.a du rootfs) ; sur Mac, la dylib
# du rootfs, prise dans Contents/Frameworks de l'app comme pour kyavserver ;
# sous Linux, celle du runtime de Kyber, trouvée par rpath
# (runtime/bin → runtime/lib).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
TARGET="${1:?cible manquante (windows, macos, linux)}"
ROOTFS="$(cd "${2:?rootfs manquant}" && pwd -P)"
OUTPUT="${3:?sortie manquante}"
mkdir -p "$(dirname "$OUTPUT")"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
BUILD="${KYBERNOW_MICRO_TARGET:-$HERE/target}"
case "$TARGET" in
windows)
    (cd "$HERE" && CARGO_TARGET_DIR="$BUILD" CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
        KYBERNOW_OPUS_LIB="$ROOTFS/lib" cargo build --release --quiet --target x86_64-pc-windows-gnu)
    cp "$BUILD/x86_64-pc-windows-gnu/release/kybernow-micro.exe" "$OUTPUT"
    ;;
macos)
    (cd "$HERE" && CARGO_TARGET_DIR="$BUILD" PKG_CONFIG_PATH="$ROOTFS/lib/pkgconfig" \
        MACOSX_DEPLOYMENT_TARGET=15.0 cargo build --release --quiet)
    cp "$BUILD/release/kybernow-micro" "$OUTPUT"
    for dep in $(otool -L "$OUTPUT" | awk 'NR>1 {print $1}' | grep -F "$ROOTFS/" || true); do
        install_name_tool -change "$dep" "@executable_path/../Frameworks/$(basename "$dep")" "$OUTPUT"
    done
    ;;
linux)
    PC="$ROOTFS/lib/pkgconfig"
    for d in "$ROOTFS"/lib64/pkgconfig "$ROOTFS"/lib/*-linux-gnu/pkgconfig; do
        [ -d "$d" ] && PC="$PC:$d"
    done
    # shellcheck disable=SC2016  # $ORIGIN : pour l'éditeur de liens
    (cd "$HERE" && CARGO_TARGET_DIR="$BUILD" PKG_CONFIG_PATH="$PC" \
        RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,\$ORIGIN/../lib:\$ORIGIN/../lib64:\$ORIGIN/../lib/$(gcc -print-multiarch 2>/dev/null || echo x)" \
        cargo build --release --quiet)
    cp "$BUILD/release/kybernow-micro" "$OUTPUT"
    ;;
*)
    echo "cible inconnue : $TARGET" >&2
    exit 1
    ;;
esac
echo "==> kybernow-micro ($TARGET) : $OUTPUT"
