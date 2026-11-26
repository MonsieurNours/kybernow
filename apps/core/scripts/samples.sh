#!/bin/bash
# Extraits du test de décodage du profil Auto (docs/profil-auto.md,
# « Le décodage ») : apps/core/samples/*.kns, embarqués par le cœur
# (`autotest::samples`).
#
# 3840 × 2160, mire testsrc2 à 60 i/s. HEVC 4:2:0 et 4:4:4 (x265), AV1
# (SVT-AV1), H.264 (x264) : une image clé puis 19 images, sans image B, en
# faible délai comme les encodeurs de la Station, en-têtes répétés devant
# l'image clé. PyroWave 4:2:0 et 4:4:4 : une image (intra seul ; l'app la
# décode en boucle), par l'encodeur Metal de l'app Mac (Mac à puce Apple).
#
#   apps/core/scripts/samples.sh
#
# Prérequis : ffmpeg avec libx265, libx264 et libsvtav1 ; Xcode.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
CORE="$HERE/.."
MAC="$CORE/../macos"
OUT="$CORE/samples"
mkdir -p "$OUT"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

SIZE=3840x2160; W=3840; H=2160; N=20
SRC=(-f lavfi -i "testsrc2=size=${SIZE}:rate=60" -frames:v "$N")
X265="log-level=none:bframes=0:aud=1:repeat-headers=1:keyint=600:info=0:scenecut=0"
for c in 420 444; do
    ffmpeg -hide_banner -loglevel error -y "${SRC[@]}" -pix_fmt "yuv${c}p" \
        -c:v libx265 -preset ultrafast -b:v 30M -x265-params "$X265" -f hevc "hevc-$c.es"
done
ffmpeg -hide_banner -loglevel error -y "${SRC[@]}" -pix_fmt yuv420p \
    -c:v libx264 -preset ultrafast -bf 0 -b:v 20M -g 600 -x264-params aud=1:repeat-headers=1 -f h264 h264-420.es
SVT_LOG=0 ffmpeg -hide_banner -loglevel error -y "${SRC[@]}" -pix_fmt yuv420p \
    -c:v libsvtav1 -preset 10 -crf 40 -g 600 -svtav1-params "pred-struct=1:lp=6" -f obu av1-420.obu

# PyroWave : l'outil `encode` de l'essai du client Mac (tests/pyrowave).
for f in pyrowave_common.mm pyrowave_encoder.mm pyrowave_decoder.mm pyrowave_bitstream.cpp; do
    clang++ -std=c++17 -O2 -fobjc-arc -c "$MAC/PyroWave/upstream/$f" -o "$f.o"
done
swiftc -O -I "$MAC/PyroWave/module" -framework Metal -framework IOSurface -lc++ ./*.o "$MAC/tests/pyrowave/common.swift" \
    "$MAC/KyberKit/PyroWaveDecoder.swift" "$MAC/KyberKit/VideoDecoder.swift" "$MAC/KyberKit/AV1.swift" \
    -o encode "$MAC/tests/pyrowave/encode/main.swift" 2>&1 | grep error: || true
mbps=$(( W * H * 60 * 12 / 10 / 1000000 ))
for c in 420 444; do
    ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=${SIZE}:rate=60" -frames:v 1 \
        -vf "scale=out_color_matrix=bt709:out_range=tv" -pix_fmt "yuv${c}p" -f rawvideo "pw-$c.yuv"
    ./encode "pw-$c.yuv" "pyrowave-$c.pw" "$W" "$H" "$c" "$mbps" > /dev/null
done

python3 "$HERE/samples-pack.py" "$WORK" "$OUT" "$W" "$H"
ls -la "$OUT"
