# shellcheck shell=bash
# PyroWave (https://github.com/Themaister/pyrowave, MIT) pour la Station
# Linux : mêmes sources, au même commit que la Station Windows
# (server/kyber-patches/pyrowave.sh), compilées pour la machine qui compile :
# libpyrowave-shared.so et pyrowave-device-validation, installés dans le
# rootfs de Kyber avec pyrowave-shared.pc. FFmpeg (correctif Linux
# 0010-kymedia-ffmpeg-pyrowave-vulkan) s'y lie pour son encodeur
# libpyrowave_vulkan ; Admin lance pyrowave-device-validation pour annoncer
# PyroWave. docs/pyrowave.md, « La Station Linux ».
#
# À sourcer depuis build-kyber.sh (ROOT, OUT, ARCH définis), puis appeler
# pyrowave_linux_build. PYROWAVE_DIR : sources à part (défaut
# third_party/pyrowave, partagé avec la Station Windows : dossier de
# compilation à part, build-linux-<arch>).

pyrowave_linux_build() {
    local THIRD_PARTY="${PYROWAVE_THIRD_PARTY:-$ROOT/third_party}"
    # shellcheck source=../../kyber-patches/pyrowave.sh
    . "$ROOT/server/kyber-patches/pyrowave.sh"
    pyrowave_fetch
    local dir="$THIRD_PARTY/pyrowave" build="$THIRD_PARTY/pyrowave/build-linux-$ARCH"
    echo "==> PyroWave (libpyrowave-shared.so, $ARCH)"
    command -v cmake >/dev/null || { echo "cmake introuvable" >&2; exit 1; }
    local gen=()
    command -v ninja >/dev/null && gen=(-G Ninja)
    # Chemin de recherche : $ORIGIN/../lib seul (rpmbuild refuse le dossier de
    # compilation et un chemin vide, que CMAKE_BUILD_RPATH laissait).
    # Seules la bibliothèque et la vérification du périphérique : les essais
    # (SDL, d3d12) ne servent pas à la Station.
    cmake -S "$dir" -B "$build" "${gen[@]}" -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
        -DCMAKE_BUILD_WITH_INSTALL_RPATH=ON -DCMAKE_INSTALL_RPATH='$ORIGIN/../lib' >/dev/null
    cmake --build "$build" --target pyrowave-shared pyrowave-device-validation
    mkdir -p "$OUT/bin" "$OUT/lib/pkgconfig" "$OUT/include/pyrowave"
    # La bibliothèque et ses liens de version (libpyrowave-shared.so.1…).
    cp -P "$build"/libpyrowave-shared.so* "$OUT/lib/"
    cp "$build/pyrowave-device-validation" "$OUT/bin/"
    # pyrowave.h seul : les en-têtes Vulkan sont ceux du rootfs de Kyber
    # (Vulkan-Headers 1.4), avec lesquels FFmpeg compile déjà son Vulkan ;
    # ceux de Granite passeraient devant.
    rm -rf "$OUT/include/pyrowave"
    mkdir -p "$OUT/include/pyrowave"
    cp "$dir/pyrowave.h" "$OUT/include/pyrowave/"
    cat > "$OUT/lib/pkgconfig/pyrowave-shared.pc" <<EOF
prefix=$OUT
libdir=\${prefix}/lib
includedir=\${prefix}/include/pyrowave

Name: pyrowave-shared
Description: C API for pyrowave
Version: 1.0.0
Libs: -L\${libdir} -lpyrowave-shared
Cflags: -I\${includedir}
EOF
}
