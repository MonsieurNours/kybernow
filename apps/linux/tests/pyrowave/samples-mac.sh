#!/bin/bash
# Flux d'essai pour tests/pyrowave/run.sh (SAMPLES=…), encodés sur un Mac à puce
# Apple par l'encodeur Metal de l'amont (le port d'apps/macos/PyroWave, outil
# `encode` de l'essai du client Mac) : mire testsrc2, 1,2 bit par pixel à 60 i/s,
# 4:2:0 et 4:4:4 en SDR, et 4:2:0 étiqueté HDR10 (source 8 bits).
#   apps/linux/tests/pyrowave/samples-mac.sh DOSSIER [LARGEURxHAUTEUR] [IMAGES]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
MAC="$HERE/../../../macos"
OUT="$(mkdir -p "$1" && cd "$1" && pwd)"
SIZE="${2:-2560x1440}"; N="${3:-120}"
w=${SIZE%x*}; h=${SIZE#*x}
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"
for f in pyrowave_common.mm pyrowave_encoder.mm pyrowave_decoder.mm pyrowave_bitstream.cpp; do
    clang++ -std=c++17 -O2 -fobjc-arc -c "$MAC/PyroWave/upstream/$f" -o "$f.o"
done
swiftc -O -I "$MAC/PyroWave/module" -framework Metal -framework IOSurface -lc++ ./*.o "$MAC/tests/pyrowave/common.swift" \
    "$MAC/KyberKit/PyroWaveDecoder.swift" "$MAC/KyberKit/VideoDecoder.swift" "$MAC/KyberKit/AV1.swift" \
    -o encode "$MAC/tests/pyrowave/encode/main.swift" 2>&1 | grep error: || true
mbps=$(( w * h * 60 * 12 / 10 / 1000000 ))
for c in 420 444; do
    ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=${SIZE}:rate=60" -frames:v "$N" \
        -vf "scale=out_color_matrix=bt709:out_range=tv" -pix_fmt "yuv${c}p" -f rawvideo "$OUT/$SIZE-$c.yuv"
    ./encode "$OUT/$SIZE-$c.yuv" "$OUT/$SIZE-$c.pyrowave" "$w" "$h" "$c" "$mbps"
    [ "$c" = 420 ] && ./encode "$OUT/$SIZE-$c.yuv" "$OUT/$SIZE-$c-hdr.pyrowave" "$w" "$h" "$c" "$mbps" hdr > /dev/null
done
ls -la "$OUT"
