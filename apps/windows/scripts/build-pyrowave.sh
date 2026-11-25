#!/bin/bash
# PyroWave (https://github.com/Themaister/pyrowave, MIT) pour l'app Windows :
# libpyrowave-shared-1.dll (chemin Vulkan, Granite partiel de
# checkout_granite.sh, runtime C++ lié en statique), au commit de
# apps/macos/PyroWave/COMMIT (le même que la Station et le Mac), compilée en
# MinGW pour x64 (GCC de Homebrew) ou ARM64 (llvm-mingw). Le chargeur Vulkan
# n'est pas lié : volk ouvre vulkan-1.dll (celle du pilote) au lancement ;
# sans elle, PyroWave est indisponible, l'app tourne. docs/pyrowave-client-windows.md.
#
#   apps/windows/scripts/build-pyrowave.sh [--arch x64|arm64]
#
# Sources dans third_party/pyrowave (partagées avec la Station, qui compile
# dans build-win64) ; compilation dans third_party/pyrowave/build-client-<arch> ;
# la DLL dans third_party/windows-client-<arch>/pyrowave/, lue par build.sh.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
. "$ROOT/scripts/kyber-toolchain.sh"

ARCH=x64
while [ $# -gt 0 ]; do
    case "$1" in
        --arch) ARCH="${2:-}"; shift ;;
        *) echo "Option inconnue : $1" >&2; exit 1 ;;
    esac
    shift
done

PYROWAVE_REPO=https://github.com/Themaister/pyrowave.git
PYROWAVE_COMMIT=$(tr -d ' \n' < "$ROOT/apps/macos/PyroWave/COMMIT")
SRC="$THIRD_PARTY/pyrowave"
BUILD="$SRC/build-client-$ARCH"
OUT="$THIRD_PARTY/windows-client-$ARCH/pyrowave"

case "$ARCH" in
    x64)
        TRIPLE=x86_64-w64-mingw32
        CC="$TRIPLE-gcc"; CXX="$TRIPLE-g++"; RC="$TRIPLE-windres"; STRIP="$TRIPLE-strip"
        SYSROOT=/opt/homebrew/opt/mingw-w64/toolchain-x86_64/$TRIPLE
        PROCESSOR=x86_64
        # pyrowave lie lui-même libstdc++, libgcc et winpthread en statique (GCC).
        LINK_FLAGS=""
        ;;
    arm64)
        TRIPLE=aarch64-w64-mingw32
        LLVM_MINGW="$THIRD_PARTY/toolchains/llvm-mingw-$KYBER_LLVM_MINGW-ucrt-macos-universal"
        [ -x "$LLVM_MINGW/bin/$TRIPLE-clang" ] || { echo "llvm-mingw $KYBER_LLVM_MINGW absent de third_party/toolchains (docs/build-notes.md)" >&2; exit 1; }
        export PATH="$LLVM_MINGW/bin:$PATH"
        CC="$TRIPLE-clang"; CXX="$TRIPLE-clang++"; RC="$TRIPLE-windres"; STRIP=llvm-strip
        SYSROOT="$LLVM_MINGW/$TRIPLE"
        PROCESSOR=aarch64
        # libc++ et libunwind dans la DLL (llvm-mingw les lie en DLL sinon).
        LINK_FLAGS="-static"
        ;;
    *) echo "Architecture inconnue : $ARCH (x64 ou arm64)" >&2; exit 1 ;;
esac
command -v cmake >/dev/null && command -v ninja >/dev/null || { echo "cmake et ninja requis (brew install cmake ninja)" >&2; exit 1; }

# Sources au commit épinglé, Granite partiel au commit de checkout_granite.sh.
if [ ! -d "$SRC/.git" ]; then
    git clone -q "$PYROWAVE_REPO" "$SRC"
fi
if [ "$(git -C "$SRC" rev-parse HEAD)" != "$PYROWAVE_COMMIT" ]; then
    git -C "$SRC" cat-file -e "$PYROWAVE_COMMIT^{commit}" 2>/dev/null || git -C "$SRC" fetch -q origin
    git -C "$SRC" checkout -q "$PYROWAVE_COMMIT"
fi
GRANITE=$(sed -n 's/^GRANITE_COMMIT=//p' "$SRC/checkout_granite.sh")
if [ "$(git -C "$SRC/Granite" rev-parse HEAD 2>/dev/null)" != "$GRANITE" ]; then
    (cd "$SRC" && bash ./checkout_granite.sh >/dev/null 2>&1) || { echo "Granite introuvable (checkout_granite.sh de PyroWave)" >&2; exit 1; }
fi

echo "==> PyroWave ${PYROWAVE_COMMIT:0:7} ($ARCH)"
mkdir -p "$BUILD" "$OUT"
cat > "$BUILD/mingw.cmake" <<EOF
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR $PROCESSOR)
set(CMAKE_C_COMPILER $CC)
set(CMAKE_CXX_COMPILER $CXX)
set(CMAKE_RC_COMPILER $RC)
set(CMAKE_FIND_ROOT_PATH $SYSROOT)
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
EOF
cmake -S "$SRC" -B "$BUILD" -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_TOOLCHAIN_FILE="$BUILD/mingw.cmake" \
    -DCMAKE_SHARED_LINKER_FLAGS="$LINK_FLAGS" >/dev/null
# La bibliothèque seule (ni les essais ni l'outil de vérification).
cmake --build "$BUILD" --target pyrowave-shared | { grep -Ev '^\[[0-9]+/[0-9]+\] (Building|Linking)' || true; }
"$STRIP" -o "$OUT/libpyrowave-shared-1.dll" "$BUILD/libpyrowave-shared-1.dll"
echo "$PYROWAVE_COMMIT" > "$OUT/COMMIT"
ls -la "$OUT/libpyrowave-shared-1.dll"
