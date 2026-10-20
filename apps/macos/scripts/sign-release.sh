#!/bin/bash
# Signature finale de la version diffusée, après xcodebuild (Release) :
#   apps/macos/scripts/sign-release.sh apps/macos/build/Build/Products/Release/KyberNow.app
#
# Xcode copie Sparkle (paquet Swift) dans l'app après les phases du projet,
# avec la signature de Sparkle : la notarisation exige notre Developer ID, le
# runtime renforcé et un horodatage sur chacun de ses outils. On les
# re-signe du plus profond au framework (ordre de la doc de Sparkle), puis
# l'app elle-même, dont le sceau doit couvrir ces nouvelles signatures.
#
# L'app garde les droits que lui a donnés Xcode : avec le profil Developer ID
# (iCloud), Xcode y ajoute l'identifiant d'app et l'équipe, que le fichier
# .entitlements ne contient pas ; le profil reste dans Contents/.
set -euo pipefail
APP="${1:?usage : sign-release.sh KyberNow.app}"
IDENTITY="Developer ID Application: Mathieu Requenna (P6DB682S4K)"
FLAGS=(--force --sign "$IDENTITY" --options runtime --timestamp)
ENTITLEMENTS="$(mktemp -d)/KyberNow.entitlements"
codesign -d --entitlements "$ENTITLEMENTS" --xml "$APP" 2>/dev/null

SPARKLE="$APP/Contents/Frameworks/Sparkle.framework/Versions/B"
codesign "${FLAGS[@]}" "$SPARKLE/XPCServices/Installer.xpc"
codesign "${FLAGS[@]}" --preserve-metadata=entitlements "$SPARKLE/XPCServices/Downloader.xpc"
codesign "${FLAGS[@]}" "$SPARKLE/Autoupdate"
codesign "${FLAGS[@]}" "$SPARKLE/Updater.app"
codesign "${FLAGS[@]}" "$APP/Contents/Frameworks/Sparkle.framework"
codesign "${FLAGS[@]}" --entitlements "$ENTITLEMENTS" "$APP"
codesign -v --deep --strict "$APP"
echo "Signée : $IDENTITY"
