#!/bin/bash
# Compile l'icône de la barre des menus de KyberNow Station (exécutable
# principal de « KyberNow Station.app ») dans build/. La signature se fait
# avec le paquet entier (server/macos/scripts/build-station-app.sh).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$HERE/build"
swiftc -O -target arm64-apple-macos15.0 \
    "$HERE/main.swift" "$HERE/Station.swift" "$HERE/Permissions.swift" "$HERE/Icon.swift" "$HERE/Strings.swift" "$HERE/Notify.swift" \
    -o "$HERE/build/KyberNow Station"
"$HERE/build/KyberNow Station" --check-strings
echo "$HERE/build/KyberNow Station"
