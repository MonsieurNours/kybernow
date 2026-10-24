#!/bin/bash
# Test hors ligne du décodeur Linux (decoder.c), 4:2:0 et 4:4:4, sans ordinateur
# distant. À lancer sous Linux après scripts/build.sh ; les flux sont générés
# par ffmpeg (x265) sur la machine ou fournis dans $SAMPLES.
#   apps/linux/tests/video/run.sh [--hw]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$HERE/../.."
ROOT="$APP/../.."
PREFIX="${PREFIX:-$ROOT/third_party/linux-$(uname -m)}"
HW=0; [ "${1:-}" = "--hw" ] && HW=1
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

EXTRA=()
[ "$(uname -m)" = aarch64 ] && EXTRA=(-lgcc)
cc -O2 -o "$WORK/check" "$HERE/check.c" "$APP/src/native/decoder.c" -I"$PREFIX/include" \
    "$PREFIX/lib/libavcodec.a" "$PREFIX/lib/libavutil.a" -lva -lva-drm -ldrm -lm -lpthread "${EXTRA[@]}"

S="${SAMPLES:-$WORK}"
if [ -z "${SAMPLES:-}" ]; then
    for f in yuv420p yuv444p; do
        ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=2560x1440:rate=60" -frames:v 120 -pix_fmt "$f" \
            -c:v libx265 -preset ultrafast -x265-params "log-level=none:bframes=0:aud=1:repeat-headers=1:keyint=60:info=0" -f hevc "$S/$f.hevc"
    done
    for c in 3366CC FF0000 FFFFFF 202020; do
        ffmpeg -hide_banner -loglevel error -y -f lavfi -i "color=c=0x${c}:size=256x256:rate=30" -frames:v 3 \
            -vf "scale=out_color_matrix=bt709:out_range=tv,format=yuv444p" -colorspace bt709 -color_primaries bt709 -color_trc bt709 \
            -c:v libx265 -crf 0 -x265-params "log-level=none:bframes=0:aud=1:info=0" -f hevc "$S/c_${c}_yuv444p.hevc"
    done
fi

fail=0
"$WORK/check" "$S/yuv420p.hevc" $HW || fail=1
"$WORK/check" "$S/yuv444p.hevc" $HW || fail=1
for c in 3366CC FF0000 FFFFFF 202020; do
    "$WORK/check" "$S/c_${c}_yuv444p.hevc" $HW "$c" || fail=1
done
exit $fail
