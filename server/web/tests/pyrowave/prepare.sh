#!/bin/bash
# Fichiers de l'essai hors ligne du décodeur WebGPU (docs/pyrowave-client-web.md) :
# mire testsrc2 de ffmpeg encodée par l'encodeur Metal du port PyroWave
# (outil `encode` de apps/macos/tests/pyrowave), puis première image décodée
# par le décodeur Metal du port (outil `refdump`), la référence.
#   prepare.sh <dossier> [1440p|4k|tout]
# Écrit dans <dossier> : <nom>.pyrowave (paquet KNPW + images), <nom>.ref.f32
# (Y, Cb, Cr de l'image 0 en float32), <nom>.src.yuv (image 0 source, 8 bits).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
MAC="$HERE/../../../../apps/macos"
OUT="$(mkdir -p "$1" && cd "$1" && pwd)"
WHICH="${2:-tout}"
BUILD="$OUT/build"
mkdir -p "$BUILD"
cd "$BUILD"
if [ ! -x refdump ] || [ ! -x encode ]; then
    for f in pyrowave_common.mm pyrowave_decoder.mm pyrowave_encoder.mm pyrowave_bitstream.cpp; do
        clang++ -std=c++17 -O2 -fobjc-arc -c "$MAC/PyroWave/upstream/$f" -o "$f.o"
    done
    SWIFT=(-O -I "$MAC/PyroWave/module" -framework Metal -framework IOSurface -lc++ ./*.o "$MAC/tests/pyrowave/common.swift"
           "$MAC/KyberKit/PyroWaveDecoder.swift" "$MAC/KyberKit/VideoDecoder.swift" "$MAC/KyberKit/AV1.swift")
    swiftc "${SWIFT[@]}" -o encode "$MAC/tests/pyrowave/encode/main.swift" 2>&1 | grep error: || true
    swiftc -O -I "$MAC/PyroWave/module" -framework Metal -lc++ ./*.o "$MAC/tests/pyrowave/common.swift" \
        -o refdump "$HERE/refdump/main.swift" 2>&1 | grep error: || true
fi
cd "$OUT"
SIZES=()
case "$WHICH" in 1440p) SIZES=(2560x1440) ;; 4k) SIZES=(3840x2160) ;; *) SIZES=(2560x1440 3840x2160) ;; esac
FRAMES=30
for size in "${SIZES[@]}"; do
    w=${size%x*}; h=${size#*x}
    # 1,2 bit par pixel à 60 i/s, comme l'essai du Mac.
    mbps=$(( w * h * 60 * 12 / 10 / 1000000 ))
    for c in 420 444; do
        name="${h}p-$c"
        ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=${size}:rate=60" -frames:v $FRAMES \
            -vf "scale=out_color_matrix=bt709:out_range=tv" -pix_fmt "yuv${c}p" -f rawvideo "$BUILD/src.yuv"
        head -c $(( w * h * (c == 420 ? 3 : 6) / 2 )) "$BUILD/src.yuv" > "$name.src.yuv"
        "$BUILD/encode" "$BUILD/src.yuv" "$name.pyrowave" "$w" "$h" "$c" "$mbps"
        "$BUILD/refdump" "$name.pyrowave" "$name.ref.f32" 0
    done
done
rm -f "$BUILD/src.yuv"
