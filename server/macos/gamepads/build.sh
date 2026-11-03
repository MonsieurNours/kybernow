#!/bin/bash
# Assemble « KyberNow Manettes.app » (manettes virtuelles de la Station Mac,
# voir main.swift) dans build/ : l'app d'aide que kynputserver lance depuis
# Contents/Helpers de KyberNow Station.app.
#
#   build.sh [version]
#
# Le droit d'Apple com.apple.developer.hid.virtual.device ne vaut qu'avec un
# profil Developer ID qui l'autorise pour l'App ID
# fr.leslignesclaires.kybernow.station.gamepads : $KYBERNOW_GAMEPADS_PROFILE,
# sinon le premier profil valable des dossiers de profils de Xcode. Sans
# profil, l'aide est signée sans le droit : elle tourne, macOS refuse ses
# manettes, et Admin annonce les manettes indisponibles.
#
# Dernière ligne : chemin de l'app.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
IDENTITY="${KYBER_SIGN_IDENTITY:-Developer ID Application: Mathieu Requenna (P6DB682S4K)}"
TEAM="P6DB682S4K"
BUNDLE_ID="fr.leslignesclaires.kybernow.station.gamepads"
VERSION="${1:-0.0.0}"
APP="$HERE/build/KyberNow Manettes.app"

mkdir -p "$HERE/build"
swiftc -O -target arm64-apple-macos15.0 "$HERE/main.swift" -o "$HERE/build/kybernow-gamepads"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$HERE/build/kybernow-gamepads" "$APP/Contents/MacOS/"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>fr</string>
    <key>CFBundleExecutable</key><string>kybernow-gamepads</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleName</key><string>KyberNow Manettes</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSMinimumSystemVersion</key><string>15.0</string>
    <key>LSBackgroundOnly</key><true/>
    <key>NSHumanReadableCopyright</key><string>Les Lignes Claires · AGPL-3.0</string>
</dict>
</plist>
PLIST

# Profil Developer ID valable : bon App ID, le droit, pour tous les Mac
# (Developer ID), pas expiré.
profile_ok() {
    local plist
    plist="$(security cms -D -i "$1" 2>/dev/null)" || return 1
    local pb=/usr/libexec/PlistBuddy tmp
    tmp="$(mktemp -t kybernow-profile)"
    printf '%s' "$plist" > "$tmp"
    local ok=1
    [ "$($pb -c 'Print :Entitlements:com.apple.application-identifier' "$tmp" 2>/dev/null)" = "$TEAM.$BUNDLE_ID" ] &&
        [ "$($pb -c 'Print :Entitlements:com.apple.developer.hid.virtual.device' "$tmp" 2>/dev/null)" = "true" ] &&
        [ "$($pb -c 'Print :ProvisionsAllDevices' "$tmp" 2>/dev/null)" = "true" ] &&
        [ "$(date -j -f '%a %b %d %T %Z %Y' "$($pb -c 'Print :ExpirationDate' "$tmp")" +%s 2>/dev/null || echo 0)" -gt "$(date +%s)" ] &&
        ok=0
    rm -f "$tmp"
    return $ok
}
PROFILE="${KYBERNOW_GAMEPADS_PROFILE:-}"
if [ -z "$PROFILE" ]; then
    for p in "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"/*.provisionprofile \
             "$HOME/Library/MobileDevice/Provisioning Profiles"/*.provisionprofile; do
        [ -f "$p" ] && profile_ok "$p" && { PROFILE="$p"; break; }
    done
elif ! profile_ok "$PROFILE"; then
    echo "Profil $PROFILE refusé : il faut l'App ID $TEAM.$BUNDLE_ID, le droit HID Virtual Device et un profil Developer ID valable" >&2
    exit 1
fi

if [ -n "$PROFILE" ]; then
    cp "$PROFILE" "$APP/Contents/embedded.provisionprofile"
    codesign -f -s "$IDENTITY" --timestamp -o runtime --entitlements "$HERE/gamepads.entitlements" "$APP" >&2
    echo "    manettes : profil $(basename "$PROFILE"), droit HID Virtual Device posé" >&2
else
    codesign -f -s "$IDENTITY" --timestamp -o runtime "$APP" >&2
    echo "    manettes : aucun profil avec le droit HID Virtual Device (Apple ne l'a pas encore accordé) ; macOS refusera les manettes" >&2
fi
echo "$APP"
