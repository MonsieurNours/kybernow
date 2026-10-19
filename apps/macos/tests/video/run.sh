#!/bin/bash
# Test hors ligne du décodage (VideoToolbox) et du rendu (Metal) de KyberKit,
# sans ordinateur distant : flux HEVC/H.264 générés par ffmpeg (x265, x264).
#   - decode : nombre d'images, temps de décodage, format de sortie lisible ;
#   - color  : couleur rendue au centre d'images unies (BT.709, plage vidéo),
#              à ±4 près de la couleur attendue (en 4:2:0, la conversion de ffmpeg arrondit).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="$HERE/../../KyberKit"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

for f in yuv420p yuv444p; do
    ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=2560x1440:rate=60" -frames:v 120 -pix_fmt "$f" \
        -c:v libx265 -preset ultrafast -x265-params "log-level=none:bframes=0:aud=1:repeat-headers=1:keyint=60:info=0" -f hevc "$f.hevc"
done
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=2560x1440:rate=60" -frames:v 120 -pix_fmt yuv420p \
    -c:v libx264 -preset ultrafast -bf 0 -x264-params aud=1 -f h264 h264.h264
for c in 3366CC FF0000 FFFFFF 202020; do
    for f in yuv420p yuv444p; do
        ffmpeg -hide_banner -loglevel error -y -f lavfi -i "color=c=0x${c}:size=256x256:rate=30" -frames:v 3 \
            -vf "scale=out_color_matrix=bt709:out_range=tv,format=${f}" -colorspace bt709 -color_primaries bt709 -color_trc bt709 \
            -c:v libx265 -crf 0 -x265-params "log-level=none:bframes=0:aud=1:info=0" -f hevc "c_${c}_${f}.hevc"
    done
done

SOURCES=("$HERE/common.swift" "$KIT/VideoDecoder.swift" "$KIT/DecoderCapabilities.swift" "$KIT/VideoRenderer.swift")
swiftc -O -o decode "$HERE/decode/main.swift" "${SOURCES[@]}"
swiftc -O -o color "$HERE/color/main.swift" "${SOURCES[@]}"
./decode
./color
