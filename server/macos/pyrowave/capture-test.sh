#!/bin/bash
# Essai de l'encodeur PyroWave de la Station Mac sur une capture réelle
# (docs/pyrowave.md, « Essai sur le Mac ») : capture-test.m (ScreenCaptureKit
# → encodeur FFmpeg libpyrowave_metal de la Station, compilé dans le rootfs de
# build-kyber.sh) puis décodage par le décodeur de l'app Mac
# (apps/macos/tests/pyrowave/bench) : 1440p et 4K, 4:2:0 (NV12 de la capture,
# comme en session, et BGRA converti par l'encodeur) et 4:4:4, SDR et HDR10,
# et l'image réduite d'un invité (4K → 1080p).
#
#   KYBER_DIR=<sources de Kyber> capture-test.sh [écran]
#
# Il faut l'autorisation « Enregistrement de l'écran » pour le terminal qui
# le lance. Faire bouger l'image (une vidéo, une fenêtre qu'on déplace) donne
# des images différentes ; sinon la même image est encodée à 60 i/s.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
KYBER_DIR="$(cd "${KYBER_DIR:-$ROOT/third_party/kyber-desktop-macos}" && pwd -P)"
ROOTFS="$KYBER_DIR/rootfs-arm64-macos"
DISPLAY_INDEX="${1:-0}"
APP="$ROOT/apps/macos"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# L'outil, lié au FFmpeg statique de la Station.
export PKG_CONFIG_LIBDIR="$ROOTFS/lib/pkgconfig"
# shellcheck disable=SC2046
clang -O2 -fobjc-arc -mmacosx-version-min=15.0 "$HERE/capture-test.m" \
    $(pkg-config --cflags --static libavcodec libavutil) \
    $(pkg-config --libs --static libavcodec libavutil) \
    -framework ScreenCaptureKit -framework CoreMedia -framework CoreGraphics \
    -o "$WORK/capture-test"

# Le banc de décodage de l'app Mac (comme apps/macos/tests/pyrowave/run.sh).
PW="$APP/PyroWave"
for f in pyrowave_common.mm pyrowave_decoder.mm pyrowave_encoder.mm pyrowave_bitstream.cpp; do
    clang++ -std=c++17 -O2 -fobjc-arc -c "$PW/upstream/$f" -o "$WORK/$f.o"
done
swiftc -O -I "$PW/module" -framework Metal -framework IOSurface -lc++ "$WORK"/*.o \
    "$APP/tests/pyrowave/common.swift" "$APP/KyberKit/PyroWaveDecoder.swift" \
    "$APP/KyberKit/VideoDecoder.swift" "$APP/KyberKit/AV1.swift" \
    -o "$WORK/bench" "$APP/tests/pyrowave/bench/main.swift" 2>&1 | grep error: || true

status=0
run() { # nom, options de capture-test
    local name=$1; shift
    echo "=== $name"
    if "$WORK/capture-test" -display "$DISPLAY_INDEX" -o "$WORK/$name.pyrowave" -src "$WORK/$name.yuv" "$@"; then
        "$WORK/bench" "$WORK/$name.pyrowave" $([ -s "$WORK/$name.yuv" ] && echo "$WORK/$name.yuv") || status=1
    else
        status=1
    fi
    rm -f "$WORK/$name.yuv"
}
for size in 2560x1440 3840x2160; do
    run "$size-420-sdr-nv12" -size "$size" -nv12
    run "$size-420-sdr" -size "$size"
    run "$size-444-sdr" -size "$size" -444
    run "$size-420-hdr" -size "$size" -hdr
done
run "invite-1080p" -size 3840x2160 -scale 1920x1080
run "invite-1080p-hdr" -size 3840x2160 -scale 1920x1080 -hdr
exit $status
