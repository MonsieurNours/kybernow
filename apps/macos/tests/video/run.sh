#!/bin/bash
# Test hors ligne du décodage (VideoToolbox) et du rendu (Metal) de KyberKit,
# sans ordinateur distant : flux HEVC/H.264 générés par ffmpeg (x265, x264).
#   - decode : nombre d'images, temps de décodage, format de sortie lisible ;
#   - recovery : perte d'une image dans un flux à rafraîchissement progressif,
#              reprise sur l'image clé suivante ;
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

# Rafraîchissement progressif, puis une vraie image clé (IDR) à 200 (test de
# reprise). x265 ignore une image clé forcée en rafraîchissement progressif :
# on met bout à bout deux encodages, le second commence par une IDR.
IR="log-level=none:bframes=0:aud=1:keyint=100000:min-keyint=100000:intra-refresh=1:repeat-headers=1:info=0:scenecut=0"
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=1280x720:rate=60" -frames:v 200 -pix_fmt yuv420p \
    -c:v libx265 -preset ultrafast -x265-params "$IR" -f hevc ir-a.hevc
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=1280x720:rate=60" -vf "trim=start_frame=200" -frames:v 100 -pix_fmt yuv420p \
    -c:v libx265 -preset ultrafast -x265-params "$IR" -f hevc ir-b.hevc
cat ir-a.hevc ir-b.hevc > ir.hevc

# AV1 en faible délai (SVT-AV1), comme NVENC en temps réel.
AV1P="pred-struct=1:lp=6"
SVT_LOG=0 ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=2560x1440:rate=60" -frames:v 120 -pix_fmt yuv420p \
    -c:v libsvtav1 -preset 10 -g 60 -svtav1-params "$AV1P" -f obu av1.obu
for c in 3366CC 202020; do
    SVT_LOG=0 ffmpeg -hide_banner -loglevel error -y -f lavfi -i "color=c=0x${c}:size=256x256:rate=30" -frames:v 3 \
        -vf "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p" -colorspace bt709 -color_primaries bt709 -color_trc bt709 \
        -c:v libsvtav1 -preset 10 -crf 1 -svtav1-params "$AV1P" -f obu "c_${c}.obu"
done

SOURCES=("$HERE/common.swift" "$KIT/AV1.swift" "$KIT/VideoDecoder.swift" "$KIT/DecoderCapabilities.swift" "$KIT/VideoRenderer.swift" "$KIT/LatencyTracker.swift")
LINK=(-I "$KIT/../Vendor/include" -L "$KIT/../Vendor/Frameworks" -lkyclient.0.1 -Xlinker -rpath -Xlinker "$KIT/../Vendor/Frameworks")
swiftc -O -o decode "$HERE/decode/main.swift" "${SOURCES[@]}" "${LINK[@]}" 2>&1 | grep error || true
swiftc -O -o color "$HERE/color/main.swift" "${SOURCES[@]}" "${LINK[@]}" 2>&1 | grep error || true
swiftc -O -o recovery "$HERE/recovery/main.swift" "${SOURCES[@]}" "${LINK[@]}" 2>&1 | grep error || true
swiftc -O -o av1 "$HERE/av1/main.swift" "${SOURCES[@]}" "${LINK[@]}" 2>&1 | grep error || true
./decode
./color
./recovery ir.hevc
./av1 av1.obu c_3366CC.obu c_202020.obu
