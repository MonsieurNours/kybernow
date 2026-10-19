#!/bin/bash
# Test hors ligne du son (Opus → tampon → Core Audio), voir main.swift.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
MAC="$HERE/../.."
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
swiftc -O -o "$WORK/audio" "$HERE/main.swift" "$MAC/KyberKit/AudioPlayer.swift" \
    -I "$MAC/Vendor/include" -L "$MAC/Vendor/Frameworks" -lopus.0 -lkyclient.0.1 \
    -Xlinker -rpath -Xlinker "$MAC/Vendor/Frameworks"
"$WORK/audio"
