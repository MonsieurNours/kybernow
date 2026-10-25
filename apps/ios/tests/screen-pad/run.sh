#!/bin/bash
# Règles de la manette à l'écran et du toucher (iPhone, iPad), sans simulateur :
# disposition, sticks et leurs tics, gâchettes, croix, toucher qui replie le clavier.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$HERE/../.."
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
swiftc -o "$WORK/screen-pad" "$HERE/main.swift" "$APP/KyberNowIOS/ScreenPadLogic.swift" "$APP/KyberKitIOS/TouchRules.swift"
"$WORK/screen-pad"
