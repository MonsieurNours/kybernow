#!/bin/bash
# Essai hors ligne du décodeur PyroWave de KyberKit (docs/pyrowave.md), sans
# ordinateur distant : une mire (ffmpeg testsrc2, 120 images) encodée par
# l'encodeur Metal du port PyroWave, puis décodée par VideoDecoder (« pwv1 »)
# → PyroWaveDecoder, en 1440p et en 4K, 4:2:0 et 4:4:4, SDR (sortie 8 bits)
# et HDR10 (sortie 10 bits, même source 8 bits étiquetée HDR).
#   run.sh               # tout
#   run.sh 2560x1440     # une définition
# Débit : 1,2 bit par pixel à 60 i/s (≈ 265 Mbit/s en 1440p, 600 Mbit/s en 4K).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="$HERE/../../KyberKit"
PW="$HERE/../../PyroWave"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# Une compilation ratée arrête l'essai (MAC-39 : `| grep error || true`
# laissait lancer un binaire absent ou d'avant).
compile() {
    local log
    if ! log="$(swiftc "$@" 2>&1)"; then
        echo "$log" | grep -E "error" >&2 || echo "$log" >&2
        echo "Compilation de l'essai ratée." >&2
        exit 1
    fi
}
cd "$WORK"

# Le port PyroWave, compilé comme dans KyberKit (ARC, C++17).
for f in pyrowave_common.mm pyrowave_decoder.mm pyrowave_encoder.mm pyrowave_bitstream.cpp; do
    clang++ -std=c++17 -O2 -fobjc-arc -c "$PW/upstream/$f" -o "$f.o"
done
SWIFT=(-O -I "$PW/module" -framework Metal -framework IOSurface -lc++ ./*.o "$HERE/common.swift"
       "$KIT/PyroWaveDecoder.swift" "$KIT/VideoDecoder.swift" "$KIT/AV1.swift")
compile "${SWIFT[@]}" -o encode "$HERE/encode/main.swift"
compile "${SWIFT[@]}" -o bench "$HERE/bench/main.swift"

SIZES=("${1:-2560x1440}")
[ $# -eq 0 ] && SIZES=(2560x1440 3840x2160)
status=0
for size in "${SIZES[@]}"; do
    w=${size%x*}; h=${size#*x}
    mbps=$(( w * h * 60 * 12 / 10 / 1000000 ))
    for c in 420 444; do
        ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=${size}:rate=60" -frames:v 120 \
            -vf "scale=out_color_matrix=bt709:out_range=tv" -pix_fmt "yuv${c}p" -f rawvideo "src.yuv"
        head -c $(( w * h * (c == 420 ? 3 : 6) / 2 )) src.yuv > first.yuv
        ./encode src.yuv "sdr.pyrowave" "$w" "$h" "$c" "$mbps"
        ./encode src.yuv "hdr.pyrowave" "$w" "$h" "$c" "$mbps" hdr > /dev/null
        ./bench sdr.pyrowave first.yuv || status=1
        ./bench hdr.pyrowave first.yuv || status=1
    done
done
exit $status
