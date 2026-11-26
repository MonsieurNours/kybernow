#!/bin/bash
# kybernow-camera (docs/webcam.md, « Station ») compilé contre le FFmpeg de
# Kyber de la Station, comme kybernow-encoder-check :
#
#   server/camera/build.sh windows <rootfs-win64> <dossier>   (mingw, depuis le Mac)
#       kybernow-camera.exe, KyberNowCameraSource.dll (Windows 11),
#       KyberNowCameraFilter.dll (Windows 10)
#   server/camera/build.sh macos   <rootfs-arm64-macos> <sortie>
#   server/camera/build.sh linux   <rootfs-<arch>-linux> <sortie> (dans la VM ;
#       PipeWire en plus si libpipewire-0.3 est là)
#
# Windows et Mac : FFmpeg en statique ; Linux : les bibliothèques du runtime
# de Kyber, trouvées par rpath à côté d'Admin.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
TARGET="${1:?cible manquante (windows, macos, linux)}"
ROOTFS="$(cd "${2:?rootfs manquant}" && pwd -P)"
OUTPUT="${3:?sortie manquante}"
PC="$ROOTFS/lib/pkgconfig"
for d in "$ROOTFS"/lib64/pkgconfig "$ROOTFS"/lib/*-linux-gnu/pkgconfig; do
    [ -d "$d" ] && PC="$PC:$d"
done
FFMPEG="libavcodec libswscale libavutil"
case "$TARGET" in
windows)
    mkdir -p "$OUTPUT"
    CXXFLAGS=(-O2 -std=c++17 -Wall -static-libgcc -static-libstdc++)
    # Les deux DLL sont chargées hors du dossier de Kyber (Frame Server, l'app
    # qui filme) : rien d'autre que Windows à côté (runtime MinGW en statique).
    x86_64-w64-mingw32-g++ "${CXXFLAGS[@]}" -static -shared -o "$OUTPUT/KyberNowCameraSource.dll" \
        "$HERE/windows/mfsource.cpp" "$HERE/windows/source.def" -lmfplat -lmfuuid -lole32 -luuid
    x86_64-w64-mingw32-g++ "${CXXFLAGS[@]}" -static -shared -o "$OUTPUT/KyberNowCameraFilter.dll" \
        "$HERE/windows/dshow.cpp" "$HERE/windows/filter.def" -lstrmiids -lole32 -loleaut32 -luuid
    OBJ="$(mktemp -d)"
    x86_64-w64-mingw32-g++ "${CXXFLAGS[@]}" -c -o "$OBJ/install.o" "$HERE/windows/install.cpp"
    for c in camera out_windows; do
        x86_64-w64-mingw32-gcc -O2 -Wall -c -o "$OBJ/$c.o" "$HERE/$c.c" -I"$HERE" -I"$ROOTFS/include"
    done
    # shellcheck disable=SC2046  # options de pkg-config, mot à mot
    x86_64-w64-mingw32-g++ "${CXXFLAGS[@]}" -o "$OUTPUT/kybernow-camera.exe" \
        "$OBJ/camera.o" "$OBJ/out_windows.o" "$OBJ/install.o" \
        $(PKG_CONFIG_LIBDIR="$PC" pkg-config --static --libs $FFMPEG) \
        -lole32 -luuid -ladvapi32 -lpthread
    rm -rf "$OBJ"
    x86_64-w64-mingw32-strip --strip-debug "$OUTPUT"/kybernow-camera.exe "$OUTPUT"/KyberNowCamera*.dll
    ;;
macos)
    mkdir -p "$(dirname "$OUTPUT")"
    # shellcheck disable=SC2046
    clang -O2 -mmacosx-version-min=15.0 -o "$OUTPUT" "$HERE/camera.c" "$HERE/out_macos.c" -I"$HERE" -I"$ROOTFS/include" \
        $(PKG_CONFIG_LIBDIR="$PC" pkg-config --static --libs $FFMPEG) \
        -framework CoreMediaIO -framework CoreMedia -framework CoreVideo -framework CoreFoundation 2>&1 \
        | grep -Ev "ignoring duplicate libraries|was built for newer .macOS. version" || true
    [ -x "$OUTPUT" ] || exit 1
    for dep in $(otool -L "$OUTPUT" | awk 'NR>1 {print $1}' | grep -F "$ROOTFS/" || true); do
        install_name_tool -change "$dep" "@executable_path/../Frameworks/$(basename "$dep")" "$OUTPUT"
    done
    strip -x "$OUTPUT"
    ;;
linux)
    mkdir -p "$(dirname "$OUTPUT")"
    PW=()
    if pkg-config --exists libpipewire-0.3; then
        # shellcheck disable=SC2207
        PW=(-DHAVE_PIPEWIRE $(pkg-config --cflags --libs libpipewire-0.3))
    else
        echo "Attention : libpipewire-0.3 absente, kybernow-camera sans PipeWire (v4l2loopback seul)" >&2
    fi
    # shellcheck disable=SC2046,SC2016  # $ORIGIN : pour l'éditeur de liens
    cc -O2 -o "$OUTPUT" "$HERE/camera.c" "$HERE/out_linux.c" -I"$HERE" -I"$ROOTFS/include" \
        $(PKG_CONFIG_LIBDIR="$PC" pkg-config --libs $FFMPEG) "${PW[@]}" -lpthread \
        $(for d in "$ROOTFS"/lib "$ROOTFS"/lib64 "$ROOTFS"/lib/*-linux-gnu; do [ -d "$d" ] && printf -- '-Wl,-rpath-link,%s ' "$d"; done) \
        -Wl,-rpath,'$ORIGIN/../lib:$ORIGIN/../lib64:$ORIGIN/../lib/'"$(gcc -print-multiarch 2>/dev/null || echo x)"':$ORIGIN/../runtime/lib:$ORIGIN/../runtime/lib64:$ORIGIN/../runtime/lib/'"$(gcc -print-multiarch 2>/dev/null || echo x)"
    strip --strip-debug "$OUTPUT"
    ;;
*)
    echo "cible inconnue : $TARGET" >&2
    exit 1
    ;;
esac
echo "==> kybernow-camera ($TARGET) : $OUTPUT"
