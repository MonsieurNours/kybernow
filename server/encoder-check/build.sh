#!/bin/bash
# kybernow-encoder-check (docs/profil-auto.md, « L'encodeur de l'ordinateur »)
# compilé contre le FFmpeg de Kyber de la Station :
#
#   server/encoder-check/build.sh windows <rootfs-win64> <sortie.exe>   (mingw, depuis le Mac)
#   server/encoder-check/build.sh macos   <rootfs-arm64-macos> <sortie>
#   server/encoder-check/build.sh linux   <rootfs-<arch>-linux> <sortie> (dans la VM)
#
# Windows et Mac : FFmpeg en statique (comme kyavserver ; Mac : les dylib du
# rootfs, libopus, prises dans Contents/Frameworks de l'app) ; Linux : les
# bibliothèques du runtime de Kyber, trouvées par rpath à côté d'Admin
# (/usr/lib/kybernow-station/bin → ../runtime/lib).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
TARGET="${1:?cible manquante (windows, macos, linux)}"
ROOTFS="$(cd "${2:?rootfs manquant}" && pwd -P)"
OUTPUT="${3:?sortie manquante}"
mkdir -p "$(dirname "$OUTPUT")"
PC="$ROOTFS/lib/pkgconfig"
for d in "$ROOTFS"/lib64/pkgconfig "$ROOTFS"/lib/*-linux-gnu/pkgconfig; do
    [ -d "$d" ] && PC="$PC:$d"
done
case "$TARGET" in
windows)
    # shellcheck disable=SC2046  # options de pkg-config, mot à mot
    x86_64-w64-mingw32-gcc -O2 -o "$OUTPUT" "$HERE/encoder_check.c" -I"$ROOTFS/include" \
        $(PKG_CONFIG_LIBDIR="$PC" pkg-config --static --libs libavcodec libavutil) -static-libgcc
    x86_64-w64-mingw32-strip --strip-debug "$OUTPUT"
    ;;
macos)
    # shellcheck disable=SC2046
    clang -O2 -mmacosx-version-min=15.0 -o "$OUTPUT" "$HERE/encoder_check.c" -I"$ROOTFS/include" \
        $(PKG_CONFIG_LIBDIR="$PC" pkg-config --static --libs libavcodec libavutil) 2>&1 | grep -Ev "ignoring duplicate libraries|was built for newer .macOS. version" || true
    [ -x "$OUTPUT" ] || exit 1
    # Bibliothèques partagées du rootfs (libopus) : celles de l'app, dans
    # Contents/Frameworks, comme pour kyavserver.
    for dep in $(otool -L "$OUTPUT" | awk 'NR>1 {print $1}' | grep -F "$ROOTFS/" || true); do
        install_name_tool -change "$dep" "@executable_path/../Frameworks/$(basename "$dep")" "$OUTPUT"
    done
    strip -x "$OUTPUT"
    ;;
linux)
    # shellcheck disable=SC2046,SC2016  # $ORIGIN : pour l'éditeur de liens
    cc -O2 -o "$OUTPUT" "$HERE/encoder_check.c" -I"$ROOTFS/include" \
        $(PKG_CONFIG_LIBDIR="$PC" pkg-config --libs libavcodec libavutil) \
        $(for d in "$ROOTFS"/lib "$ROOTFS"/lib64 "$ROOTFS"/lib/*-linux-gnu; do [ -d "$d" ] && printf -- '-Wl,-rpath-link,%s ' "$d"; done) \
        -Wl,-rpath,'$ORIGIN/../runtime/lib:$ORIGIN/../runtime/lib64:$ORIGIN/../runtime/lib/'"$(gcc -print-multiarch 2>/dev/null || echo x)"
    strip --strip-debug "$OUTPUT"
    ;;
*)
    echo "cible inconnue : $TARGET" >&2
    exit 1
    ;;
esac
echo "==> kybernow-encoder-check ($TARGET) : $OUTPUT"
