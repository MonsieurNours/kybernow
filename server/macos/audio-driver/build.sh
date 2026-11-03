#!/bin/bash
# Compile KyberNow.driver (sortie son virtuelle de la Station Mac, voir
# KyberNowAudio.c) dans build/, signé Developer ID si l'identité est là
# (KYBER_SIGN_IDENTITY). Dernière ligne : le chemin du paquet.
#
# Installation (administrateur ; coreaudiod charge les modules au démarrage) :
#   sudo cp -R build/KyberNow.driver /Library/Audio/Plug-Ins/HAL/
#   sudo killall coreaudiod
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
IDENTITY="${KYBER_SIGN_IDENTITY:-Developer ID Application: Mathieu Requenna (P6DB682S4K)}"
OUT="$HERE/build/KyberNow.driver"
rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS"
cp "$HERE/Info.plist" "$OUT/Contents/Info.plist"
clang -O2 -Wall -Wextra -Werror -target arm64-apple-macos15.0 -bundle \
    -framework CoreAudio -framework CoreFoundation \
    -o "$OUT/Contents/MacOS/KyberNow" "$HERE/KyberNowAudio.c"
if security find-identity -v -p codesigning | grep -qF "$IDENTITY"; then
    codesign -f -s "$IDENTITY" -o runtime --timestamp=none "$OUT" 2>/dev/null
fi
echo "$OUT"
