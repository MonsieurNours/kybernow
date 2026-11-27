#!/bin/bash
# Encodeur PyroWave de la Station Mac (docs/pyrowave.md, « La Station Mac ») :
# le port Metal de PyroWave vendu dans apps/macos/PyroWave (au commit de
# apps/macos/PyroWave/COMMIT, le même que le décodeur de l'app Mac) et la
# conversion de KyberNow (kn_pyrowave_metal.mm), en une bibliothèque
# statique que FFmpeg lie (encodeur libpyrowave_metal, correctif Mac 0016),
# plus l'outil de vérification d'Admin.
#
#   build.sh <préfixe>
#
# Écrit <préfixe>/lib/libkn-pyrowave-metal.a, <préfixe>/include/kn_pyrowave_metal.h,
# <préfixe>/lib/pkgconfig/kn-pyrowave-metal.pc (lu par le configure de FFmpeg et
# par meson) et <préfixe>/bin/kybernow-pyrowave-check. Puce Apple, macOS 15+.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
PW="$ROOT/apps/macos/PyroWave/upstream"
PREFIX="${1:?préfixe manquant}"
mkdir -p "$PREFIX"
PREFIX="$(cd "$PREFIX" && pwd -P)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

MIN="-mmacosx-version-min=15.0 -arch arm64"
FLAGS="$MIN -O2 -fPIC -fvisibility=hidden -fobjc-arc -std=c++17 -Wall"
for f in pyrowave_common.mm pyrowave_decoder.mm pyrowave_encoder.mm pyrowave_bitstream.cpp; do
    clang++ $FLAGS -Wno-unused-parameter -I "$PW" -c "$PW/$f" -o "$WORK/${f%.*}.o"
done
# L'interface de KyberNow reste visible : FFmpeg et l'outil l'appellent.
clang++ $FLAGS -fvisibility=default -I "$PW" -c "$HERE/kn_pyrowave_metal.mm" -o "$WORK/kn_pyrowave_metal.o"

mkdir -p "$PREFIX/lib/pkgconfig" "$PREFIX/include" "$PREFIX/bin"
rm -f "$PREFIX/lib/libkn-pyrowave-metal.a"
ar rcs "$PREFIX/lib/libkn-pyrowave-metal.a" "$WORK"/*.o
cp "$HERE/kn_pyrowave_metal.h" "$PREFIX/include/"
FRAMEWORKS="-framework Metal -framework IOSurface -framework CoreVideo -framework Foundation"
cat > "$PREFIX/lib/pkgconfig/kn-pyrowave-metal.pc" <<PC
prefix=$PREFIX
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: kn-pyrowave-metal
Description: KyberNow PyroWave encoder for Apple silicon (Metal port of PyroWave, apps/macos/PyroWave/COMMIT)
Version: 1.0.0
Libs: -L\${libdir} -lkn-pyrowave-metal -lc++ -lobjc $FRAMEWORKS
Cflags: -I\${includedir}
PC

clang $MIN -O2 -I "$HERE" "$HERE/check.c" "$PREFIX/lib/libkn-pyrowave-metal.a" -lc++ -lobjc $FRAMEWORKS \
    -o "$PREFIX/bin/kybernow-pyrowave-check"
echo "==> PyroWave Metal : $PREFIX/lib/libkn-pyrowave-metal.a, $PREFIX/bin/kybernow-pyrowave-check"
