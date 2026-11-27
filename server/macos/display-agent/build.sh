#!/bin/bash
# Compile kybernow-display (écrans virtuels de la Station Mac, voir main.swift)
# dans build/, signé Developer ID si l'identité est là (KYBER_SIGN_IDENTITY).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
# Équipe et certificat : scripts/identite.sh.
# shellcheck source=../../../scripts/identite.sh
. "$HERE/../../../scripts/identite.sh"
IDENTITY="$KYBER_SIGN_IDENTITY"
mkdir -p "$HERE/build"
swiftc -O -target arm64-apple-macos15.0 -import-objc-header "$HERE/CGVirtualDisplayPrivate.h" \
    "$HERE/main.swift" "$HERE/audio.swift" "$HERE/apps.swift" -o "$HERE/build/kybernow-display"
"$HERE/build/kybernow-display" self-test >/dev/null || { echo "kybernow-display self-test a échoué" >&2; "$HERE/build/kybernow-display" self-test >&2; exit 1; }
if security find-identity -v -p codesigning | grep -qF "$IDENTITY"; then
    codesign -f -s "$IDENTITY" -i fr.leslignesclaires.kybernow.station.display "$HERE/build/kybernow-display" 2>/dev/null
fi
echo "$HERE/build/kybernow-display"
