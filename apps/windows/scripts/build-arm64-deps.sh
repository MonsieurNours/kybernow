#!/bin/bash
# Bibliothèques natives de l'app Windows pour ARM64 (PC Snapdragon), depuis
# macOS : libopus, FFmpeg réduit au décodage (H.264, HEVC, AV1 par D3D11VA, et
# par le processeur), SDL2 (manettes) et kynput.dll. Pour x64, c'est
# server/kyber-patches/build.sh qui les produit (rootfs du serveur).
#
# Sortie : third_party/rootfs-winarm64 (lu par scripts/build.sh --arch arm64).
# Chaîne : llvm-mingw dans third_party/toolchains (docs/build-notes.md).
# À relancer après un changement des correctifs *-kynput-* ou des sources.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
. "$ROOT/scripts/kyber-toolchain.sh"

KYSDK="$THIRD_PARTY/kyber-desktop/kysdk"
LLVM_MINGW="$THIRD_PARTY/toolchains/llvm-mingw-$KYBER_LLVM_MINGW-ucrt-macos-universal"
TRIPLE=aarch64-w64-mingw32
TARGET=aarch64-pc-windows-gnullvm
PREFIX="$THIRD_PARTY/rootfs-winarm64"
WORK="$THIRD_PARTY/windows-client-arm64/work"
OPUS_SRC="$KYSDK/kymedia/subprojects/opus-1.6.1"
FFMPEG_SRC="$KYSDK/kymedia/subprojects/FFmpeg-n8.1"
SDL_TARBALL="$KYSDK/kynput/contrib/work/x86_64-apple-darwin/src/release-2.32.8.tar.gz"
SDL_SHA256=dd35e05644ae527848d02433bec24dd0ea65db59faecf1a0e5d1880c533dac2c
JOBS=$(sysctl -n hw.ncpu)

[ -x "$LLVM_MINGW/bin/$TRIPLE-clang" ] || { echo "llvm-mingw $KYBER_LLVM_MINGW absent de third_party/toolchains (docs/build-notes.md)" >&2; exit 1; }
for d in "$OPUS_SRC" "$FFMPEG_SRC"; do [ -d "$d" ] || { echo "$d introuvable" >&2; exit 1; }; done
[ "$(shasum -a 256 "$SDL_TARBALL" 2>/dev/null | awk '{print $1}')" = "$SDL_SHA256" ] || { echo "Sources de SDL2 absentes ou altérées : $SDL_TARBALL" >&2; exit 1; }

export PATH="$LLVM_MINGW/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
kyber_toolchain_check rustc
mkdir -p "$WORK" "$PREFIX"

# make et make install sur deux lignes : dans « a && b », un échec de « a »
# n'arrête pas un script en set -e.
echo "==> libopus (NEON toujours présent sur ARM64 : pas de détection)"
rm -rf "$WORK/opus" && mkdir -p "$WORK/opus" && cd "$WORK/opus"
"$OPUS_SRC/configure" --host=$TRIPLE --prefix="$PREFIX" --disable-shared --enable-static \
    --disable-doc --disable-extra-programs --disable-rtcd CC=$TRIPLE-clang >/dev/null
make -j"$JOBS" >/dev/null
make install >/dev/null

echo "==> FFmpeg (décodage seulement)"
rm -rf "$WORK/ffmpeg" && mkdir -p "$WORK/ffmpeg" && cd "$WORK/ffmpeg"
"$FFMPEG_SRC/configure" --prefix="$PREFIX" --enable-cross-compile --cross-prefix=$TRIPLE- \
    --cc=$TRIPLE-clang --cxx=$TRIPLE-clang++ --arch=aarch64 --target-os=mingw32 \
    --pkg-config=false --disable-autodetect --enable-w32threads --enable-d3d11va \
    --disable-everything --disable-programs --disable-doc --disable-network \
    --disable-avformat --disable-avfilter --disable-avdevice --disable-swscale \
    --enable-decoder=h264,hevc,av1 \
    --enable-hwaccel=h264_d3d11va,h264_d3d11va2,hevc_d3d11va,hevc_d3d11va2,av1_d3d11va,av1_d3d11va2 \
    --enable-static --disable-shared >/dev/null
make -j"$JOBS" >/dev/null
make install >/dev/null

echo "==> SDL2"
rm -rf "$WORK/sdl" && mkdir -p "$WORK/sdl" && cd "$WORK/sdl"
tar xzf "$SDL_TARBALL" --strip-components=1
mkdir build && cd build
../configure --host=$TRIPLE --prefix="$PREFIX" --disable-static CC=$TRIPLE-clang CXX=$TRIPLE-clang++ >/dev/null
make -j"$JOBS" >/dev/null
make install >/dev/null

echo "==> kynput.dll"
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
export LIBCLANG_PATH="${LIBCLANG_PATH:-/Library/Developer/CommandLineTools/usr/lib}"
export BINDGEN_EXTRA_CLANG_ARGS="--target=$TARGET --sysroot=$LLVM_MINGW/$TRIPLE -I$LLVM_MINGW/$TRIPLE/include"
export CARGO_TARGET_AARCH64_PC_WINDOWS_GNULLVM_LINKER="$TRIPLE-clang"
export CC_aarch64_pc_windows_gnullvm="$TRIPLE-clang"
export AR_aarch64_pc_windows_gnullvm="llvm-ar"
export CARGO_PROFILE_RELEASE_STRIP=false
rm -f "$PREFIX/bin/kynput.dll"
(cd "$KYSDK/kynput" && cargo cinstall -p kynput --target $TARGET --release --library-type cdylib \
    --dlltool llvm-dlltool --prefix="$PREFIX" --features sdl2 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating) ' || true; })
[ -f "$PREFIX/bin/kynput.dll" ] || { echo "kynput.dll n'a pas été produite" >&2; exit 1; }
for f in libavcodec.a libswresample.a libavutil.a libopus.a; do
    [ -f "$PREFIX/lib/$f" ] || { echo "$f n'a pas été produite" >&2; exit 1; }
done
ls -la "$PREFIX/bin" "$PREFIX/lib"/*.a
