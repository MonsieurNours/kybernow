#!/bin/bash
# Signature finale de la version diffusée, après xcodebuild (Release) :
#   apps/macos/scripts/sign-release.sh apps/macos/build/Build/Products/Release/KyberNow.app
#
# Xcode copie Sparkle (paquet Swift) dans l'app après les phases du projet,
# avec la signature de Sparkle : la notarisation exige notre Developer ID, le
# runtime renforcé et un horodatage sur chacun de ses outils. On les
# re-signe du plus profond au framework (ordre de la doc de Sparkle), puis
# l'app elle-même, dont le sceau doit couvrir ces nouvelles signatures.
set -euo pipefail
APP="${1:?usage : sign-release.sh KyberNow.app}"
HERE="$(cd "$(dirname "$0")/.." && pwd)"
IDENTITY="Developer ID Application: Mathieu Requenna (P6DB682S4K)"
FLAGS=(--force --sign "$IDENTITY" --options runtime --timestamp)

SPARKLE="$APP/Contents/Frameworks/Sparkle.framework/Versions/B"
codesign "${FLAGS[@]}" "$SPARKLE/XPCServices/Installer.xpc"
codesign "${FLAGS[@]}" --preserve-metadata=entitlements "$SPARKLE/XPCServices/Downloader.xpc"
codesign "${FLAGS[@]}" "$SPARKLE/Autoupdate"
codesign "${FLAGS[@]}" "$SPARKLE/Updater.app"
codesign "${FLAGS[@]}" "$APP/Contents/Frameworks/Sparkle.framework"
codesign "${FLAGS[@]}" --entitlements "$HERE/KyberNow/KyberNow.entitlements" "$APP"
codesign -v --deep --strict "$APP"
echo "Signée : $IDENTITY"
