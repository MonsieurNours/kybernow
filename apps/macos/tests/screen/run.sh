#!/bin/bash
# Test d'affichage réel (fenêtre de deux secondes), voir main.swift.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="$HERE/../../KyberKit"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=1920x1080:rate=60" -frames:v 120 -pix_fmt yuv420p \
    -c:v libx265 -preset ultrafast -x265-params "log-level=none:bframes=0:aud=1:repeat-headers=1:keyint=60:info=0" -f hevc "$WORK/s.hevc"
swiftc -O -o "$WORK/screen" "$HERE/main.swift" "$KIT/AV1.swift" "$KIT/MetalSurface.swift" "$KIT/VideoDecoder.swift" "$KIT/VideoRenderer.swift" "$KIT/LatencyTracker.swift" \
    -I "$KIT/../Vendor/include" -L "$KIT/../Vendor/Frameworks" -lkyclient.0.1 -Xlinker -rpath -Xlinker "$KIT/../Vendor/Frameworks" 2>&1 | grep error || true
"$WORK/screen" "$WORK/s.hevc"
