# shellcheck shell=bash
# PyroWave (https://github.com/Themaister/pyrowave, MIT) pour la Station
# Windows : sources au commit de tout KyberNow (apps/macos/PyroWave/COMMIT, lu
# par kyber_pyrowave_commit de scripts/kyber-toolchain.sh) dans
# third_party/pyrowave (avec le
# Granite partiel de checkout_granite.sh : volk et les en-têtes Vulkan), puis
# compilation en MinGW de libpyrowave-shared-1.dll (runtime C++ lié en
# statique) et de pyrowave-device-validation.exe, installés dans le rootfs
# Windows avec pyrowave-shared.pc. FFmpeg (correctif 0025-kymedia-pyrowave) s'y
# lie pour son encodeur libpyrowave. docs/pyrowave.md.
#
# À sourcer après avoir défini THIRD_PARTY, ROOTFS et MINGW ; puis appeler
# pyrowave_fetch et pyrowave_build.

# shellcheck source=../../scripts/kyber-toolchain.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/kyber-toolchain.sh"
PYROWAVE_REPO=https://github.com/Themaister/pyrowave.git

pyrowave_fetch() {
    local dir="$THIRD_PARTY/pyrowave"
    PYROWAVE_COMMIT=$(kyber_pyrowave_commit) || exit 1
    echo "==> PyroWave ${PYROWAVE_COMMIT:0:10}"
    if [ ! -d "$dir/.git" ]; then
        git clone -q "$PYROWAVE_REPO" "$dir"
    fi
    if [ "$(git -C "$dir" rev-parse HEAD)" != "$PYROWAVE_COMMIT" ]; then
        git -C "$dir" cat-file -e "$PYROWAVE_COMMIT^{commit}" 2>/dev/null || git -C "$dir" fetch -q origin
        git -C "$dir" checkout -q "$PYROWAVE_COMMIT"
    fi
    # Granite partiel, au commit que nomme checkout_granite.sh de ce commit.
    local granite
    granite=$(sed -n 's/^GRANITE_COMMIT=//p' "$dir/checkout_granite.sh")
    if [ "$(git -C "$dir/Granite" rev-parse HEAD 2>/dev/null)" != "$granite" ]; then
        (cd "$dir" && bash ./checkout_granite.sh >/dev/null 2>&1) || {
            echo "Granite introuvable (checkout_granite.sh de PyroWave)" >&2
            exit 1
        }
    fi
}

pyrowave_build() {
    local dir="$THIRD_PARTY/pyrowave" build="$THIRD_PARTY/pyrowave/build-win64"
    echo "==> PyroWave (libpyrowave-shared-1.dll)"
    command -v cmake >/dev/null || { echo "cmake introuvable (brew install cmake)" >&2; exit 1; }
    mkdir -p "$build"
    cat > "$build/mingw.cmake" <<EOF
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR x86_64)
set(CMAKE_C_COMPILER x86_64-w64-mingw32-gcc)
set(CMAKE_CXX_COMPILER x86_64-w64-mingw32-g++)
set(CMAKE_RC_COMPILER x86_64-w64-mingw32-windres)
set(CMAKE_FIND_ROOT_PATH $MINGW)
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
EOF
    # Seules les cibles de la bibliothèque et de la vérification du
    # périphérique : les essais (d3d12, granite-vulkan complet) ne servent pas
    # à la Station.
    cmake -S "$dir" -B "$build" -G Ninja -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_TOOLCHAIN_FILE="$build/mingw.cmake" -DCMAKE_INSTALL_PREFIX="$ROOTFS" \
        -DCMAKE_INSTALL_LIBDIR=lib -DCMAKE_INSTALL_BINDIR=bin -DCMAKE_INSTALL_INCLUDEDIR=include >/dev/null
    cmake --build "$build" --target pyrowave-shared pyrowave-device-validation
    mkdir -p "$ROOTFS/bin" "$ROOTFS/lib/pkgconfig" "$ROOTFS/include/pyrowave"
    cp "$build/libpyrowave-shared-1.dll" "$build/pyrowave-device-validation.exe" "$ROOTFS/bin/"
    cp "$build/libpyrowave-shared.dll.a" "$ROOTFS/lib/"
    cp "$dir/pyrowave.h" "$ROOTFS/include/pyrowave/"
    # pyrowave.h inclut les en-têtes Vulkan : ceux de Granite, à part du rootfs.
    rm -rf "$ROOTFS/include/pyrowave/vulkan" "$ROOTFS/include/pyrowave/vk_video"
    cp -R "$dir/Granite/third_party/khronos/vulkan-headers/include/vulkan" "$ROOTFS/include/pyrowave/"
    cp -R "$dir/Granite/third_party/khronos/vulkan-headers/include/vk_video" "$ROOTFS/include/pyrowave/" 2>/dev/null || true
    cat > "$ROOTFS/lib/pkgconfig/pyrowave-shared.pc" <<EOF
prefix=$ROOTFS
libdir=\${prefix}/lib
includedir=\${prefix}/include/pyrowave

Name: pyrowave-shared
Description: C API for pyrowave
Version: 1.0.0
Libs: -L\${libdir} -lpyrowave-shared
Cflags: -I\${includedir}
EOF
}
