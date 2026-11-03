#!/bin/bash
# Compile kybernow-display (écrans virtuels de la Station Mac, voir main.swift)
# dans build/, signé Developer ID si l'identité est là (KYBER_SIGN_IDENTITY).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
IDENTITY="${KYBER_SIGN_IDENTITY:-Developer ID Application: Mathieu Requenna (P6DB682S4K)}"
mkdir -p "$HERE/build"
swiftc -O -target arm64-apple-macos15.0 -import-objc-header "$HERE/CGVirtualDisplayPrivate.h" \
    "$HERE/main.swift" "$HERE/audio.swift" "$HERE/apps.swift" -o "$HERE/build/kybernow-display"
if security find-identity -v -p codesigning | grep -qF "$IDENTITY"; then
    codesign -f -s "$IDENTITY" -i fr.leslignesclaires.kybernow.station.display "$HERE/build/kybernow-display" 2>/dev/null
fi
echo "$HERE/build/kybernow-display"
