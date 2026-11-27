#!/bin/bash
# Test d'affichage réel (deux passages de 2,5 s, avec et sans la garde « une image en vol »), voir main.swift.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="$HERE/../../KyberKit"
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
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=1920x1080:rate=60" -frames:v 120 -pix_fmt yuv420p \
    -c:v libx265 -preset ultrafast -x265-params "log-level=none:bframes=0:aud=1:repeat-headers=1:keyint=60:info=0" -f hevc "$WORK/s.hevc"
# Le port Metal de PyroWave (VideoDecoder passe le format pwv1 à PyroWaveDecoder).
PW="$HERE/../../PyroWave"
for f in pyrowave_common.mm pyrowave_decoder.mm pyrowave_bitstream.cpp; do
    clang++ -std=c++17 -O2 -fobjc-arc -c "$PW/upstream/$f" -o "$WORK/$f.o"
done
compile -O -o "$WORK/screen" "$HERE/main.swift" "$KIT/AV1.swift" "$KIT/MetalSurface.swift" "$KIT/VideoDecoder.swift" "$KIT/VideoRenderer.swift" "$KIT/LatencyReporting.swift" \
    "$KIT/PyroWaveDecoder.swift" "$KIT/PyroWaveConfig.swift" "$KIT/HDRShader.swift" "$HERE/../common/hdr-stub.swift" "$KIT/HDRVideoLayer.swift" "$KIT/Upscaler.swift" "$KIT/DecoderCapabilities.swift" "$WORK"/pyrowave_*.o -lc++ -framework IOSurface \
    -I "$KIT/../Vendor/include" -I "$KIT/../Vendor/KyberNow/include" -I "$PW/module" -L "$KIT/../Vendor/Frameworks" -lkyclient.0.1 -lkybernow -Xlinker -rpath -Xlinker "$KIT/../Vendor/Frameworks"
# Essai d'affichage réel (fenêtres à l'écran) : KYBERNOW_COMPILE_ONLY=1 s'arrête après la compilation.
[ "${KYBERNOW_COMPILE_ONLY:-0}" = 1 ] && { echo "screen : compilé"; exit 0; }
"$WORK/screen" "$WORK/s.hevc"
