#!/bin/bash
# Assemble l'extension caméra de KyberNow Station (docs/webcam.md, « Station
# Mac ») dans build/ : fr.leslignesclaires.kybernow.station.camera.systemextension,
# que build-station-app.sh met dans Contents/Library/SystemExtensions (avec le
# profil de l'app qui autorise « System Extension »). Signée ici (Developer ID,
# exécution renforcée, bac à sable).
#
#   build.sh [version]
#
# Dernière ligne : chemin de l'extension.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
# Équipe et certificat : scripts/identite.sh.
# shellcheck source=../../../scripts/identite.sh
. "$HERE/../../../scripts/identite.sh"
IDENTITY="$KYBER_SIGN_IDENTITY"
TEAM="$KYBER_TEAM_ID"
BUNDLE_ID="fr.leslignesclaires.kybernow.station.camera"
VERSION="${1:-0.0.0}"
EXT="$HERE/build/$BUNDLE_ID.systemextension"

mkdir -p "$HERE/build"
swiftc -O -target arm64-apple-macos15.0 "$HERE/main.swift" -o "$HERE/build/$BUNDLE_ID"
rm -rf "$EXT"
mkdir -p "$EXT/Contents/MacOS"
cp "$HERE/build/$BUNDLE_ID" "$EXT/Contents/MacOS/"
cat > "$EXT/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>fr</string>
    <key>CFBundleExecutable</key><string>$BUNDLE_ID</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleName</key><string>KyberNow Camera</string>
    <key>CFBundleDisplayName</key><string>KyberNow Camera</string>
    <key>CFBundlePackageType</key><string>SYSX</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSMinimumSystemVersion</key><string>15.0</string>
    <key>NSHumanReadableCopyright</key><string>Les Lignes Claires · AGPL-3.0</string>
    <key>NSSystemExtensionUsageDescription</key><string>La caméra de votre appareil, vue sur ce Mac comme une webcam ordinaire (KyberNow Camera).</string>
    <key>CMIOExtension</key>
    <dict>
        <key>CMIOExtensionMachServiceName</key><string>$TEAM.$BUNDLE_ID</string>
    </dict>
</dict>
</plist>
PLIST
codesign -f -s "$IDENTITY" --timestamp -o runtime --entitlements "$HERE/camera.entitlements" "$EXT" >&2
echo "$EXT"
