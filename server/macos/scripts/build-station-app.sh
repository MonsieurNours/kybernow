#!/bin/bash
# KyberNow Station pour Mac (jalon 6 de docs/station-macos.md) : l'app, son
# installateur, son désinstalleur et le .dmg, signés Developer ID.
#
#   build-station-app.sh             construit tout dans server/macos/dist/
#   build-station-app.sh --notarize  et fait notariser l'app puis le .dmg par
#                                    Apple (profil « kybernow » de notarytool,
#                                    docs/releases.md), tickets agrafés
#
# dist/
#   KyberNow Station.app            l'app installée dans Applications : icône de la
#                                   barre des menus (exécutable principal), Kyber,
#                                   Admin, agent des écrans, fenêtres (kybernow-ui),
#                                   agent de console du prêt (kybernow-console),
#                                   sortie son et moteur du client web en ressources
#   Install KyberNow Station.app    « Installer KyberNow Station » en français :
#                                   kybernow-ui, l'app ci-dessus en ressources
#   Uninstall KyberNow Station.app  « Désinstaller KyberNow Station » : kybernow-ui
#                                   en mode désinstallation (KyberNowMode)
#   KyberNow-Station-Mac.dmg        les deux dernières
#   KyberNow-Station-Mac-update.zip l'app seule, pour la mise à jour depuis Admin
#                                   (applicateur : server/admin/src/update/mac.rs)
#
# Prérequis : server/macos/scripts/build-kyber.sh (Kyber compilé), node et npm,
# Rust, Xcode (swiftc, clang), l'identité Developer ID dans le trousseau, et
# third_party/payload/kyber-0.29.0 et third_party/kyber-web-out dans le dépôt
# principal (moteur du client web, comme sous Windows).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
MAIN="$(cd "$(git -C "$ROOT" rev-parse --git-common-dir)/.." && pwd)"
IDENTITY="${KYBER_SIGN_IDENTITY:-Developer ID Application: Mathieu Requenna (P6DB682S4K)}"
ENTITLEMENTS="$ROOT/server/macos/station.entitlements"
BUNDLE_ID="fr.leslignesclaires.kybernow.station"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/server/installer/Cargo.toml" | head -1)"
OUT="$ROOT/server/macos/dist"
WORK="$OUT/work"
NOTARIZE=0
[ "${1:-}" = "--notarize" ] && NOTARIZE=1

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
step() { printf '\n==> %s\n' "$*"; }

mkdir -p "$WORK"

# KYBER_FROM_APP=<app KyberNow Station déjà construite> : Kyber (exécutables,
# bibliothèques, réglages) repris de cette app, sans réempaqueter third_party
# (essais pendant qu'un autre travail y recompile Kyber).
if [ -n "${KYBER_FROM_APP:-}" ]; then
    step "Kyber repris de $KYBER_FROM_APP"
    KYAPP="$WORK/kyber-from-app.app"
    rm -rf "$KYAPP"
    mkdir -p "$KYAPP/Contents/MacOS" "$KYAPP/Contents/Resources"
    cp "$KYBER_FROM_APP"/Contents/MacOS/{kycontroller,kyavserver,kynputserver} "$KYAPP/Contents/MacOS/"
    cp -R "$KYBER_FROM_APP/Contents/Frameworks" "$KYAPP/Contents/Frameworks"
    cp "$KYBER_FROM_APP/Contents/Resources/kyber/kyber_config.toml" "$KYAPP/Contents/Resources/"
else
    step "Kyber (empaquetage et signature des bibliothèques)"
    "$HERE/run-kyber.sh" package >/dev/null
    KYAPP="$(cd "$ROOT/third_party/kyber-desktop" && pwd -P)/KyberNow-Station-Kyber.app"
fi

step "Admin"
(cd "$ROOT/server/web" && npm ci --silent && npm run build --silent)
(cd "$ROOT/server/admin" && touch src/assets.rs && cargo build --release --quiet)

step "Agent des écrans, icône de la barre des menus, sortie son, manettes"
AGENT="$("$ROOT/server/macos/display-agent/build.sh" | tail -1)"
MENUBAR="$("$ROOT/server/macos/menubar/build.sh" | tail -1)"
DRIVER="$("$ROOT/server/macos/audio-driver/build.sh" | tail -1)"
# Manettes virtuelles : app d'aide à part, pour le profil d'Apple qui porte
# le droit HID Virtual Device (docs/station-macos.md, « Manettes »).
GAMEPADS="$("$ROOT/server/macos/gamepads/build.sh" "$VERSION" | tail -1)"

step "Agent de console du prêt (server/tray, docs/pret.md)"
(cd "$ROOT/server/tray" && cargo build --release --quiet)
CONSOLE="$ROOT/server/tray/target/release/kybernow-tray"

step "Fenêtres (installation, mise à jour, désinstallation, autorisations)"
(cd "$ROOT/server/installer/ui" && npm ci --silent && npm run build --silent)
(cd "$ROOT/server/installer" && cargo build --release --quiet --features ui --bin kybernow-ui)
UI="$ROOT/server/installer/target/release/kybernow-ui"

step "Icône de l'app"
ICNS="$WORK/AppIcon.icns"
SVG="$ROOT/brand/v2/app-icon/kybernow-station-1024.svg"
if [ ! -f "$ICNS" ] || [ "$SVG" -nt "$ICNS" ]; then
    swift "$HERE/svg-to-icns.swift" "$SVG" "$ICNS"
fi

# Info.plist d'un paquet : exécutable, identifiant, nom, puis lignes en plus.
info_plist() {
    local exe="$1" id="$2" name="$3" extra="${4:-}"
    cat <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>fr</string>
    <key>CFBundleExecutable</key><string>$exe</string>
    <key>CFBundleIdentifier</key><string>$id</string>
    <key>CFBundleName</key><string>$name</string>
    <key>CFBundleDisplayName</key><string>$name</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSMinimumSystemVersion</key><string>15.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>Les Lignes Claires · AGPL-3.0</string>
$extra</dict>
</plist>
EOF
}

# Nom affiché dans la langue du Mac (InfoPlist.strings).
localized() {
    local bundle="$1" fr="$2" en="$3" more_fr="${4:-}" more_en="${5:-}"
    mkdir -p "$bundle/Contents/Resources/fr.lproj" "$bundle/Contents/Resources/en.lproj"
    printf '"CFBundleDisplayName" = "%s";\n"CFBundleName" = "%s";\n%s' "$fr" "$fr" "$more_fr" > "$bundle/Contents/Resources/fr.lproj/InfoPlist.strings"
    printf '"CFBundleDisplayName" = "%s";\n"CFBundleName" = "%s";\n%s' "$en" "$en" "$more_en" > "$bundle/Contents/Resources/en.lproj/InfoPlist.strings"
}

sign() {
    codesign -f -s "$IDENTITY" --timestamp -o runtime "$@"
}

step "KyberNow Station.app"
APP="$OUT/KyberNow Station.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Frameworks" "$APP/Contents/Resources/kyber" "$APP/Contents/Resources/licenses"
cp "$MENUBAR" "$APP/Contents/MacOS/KyberNow Station"
cp "$KYAPP"/Contents/MacOS/{kycontroller,kyavserver,kynputserver} "$APP/Contents/MacOS/"
cp -R "$KYAPP/Contents/Frameworks/." "$APP/Contents/Frameworks/"
cp "$KYAPP/Contents/Resources/kyber_config.toml" "$APP/Contents/Resources/kyber/"
cp "$ROOT/server/admin/target/release/kyber-admin" "$AGENT" "$UI" "$APP/Contents/MacOS/"
cp "$CONSOLE" "$APP/Contents/MacOS/kybernow-console"
cp -R "$DRIVER" "$APP/Contents/Resources/KyberNow.driver"
# Déjà signée (avec son profil s'il existe) : copiée telle quelle.
mkdir -p "$APP/Contents/Helpers"
ditto "$GAMEPADS" "$APP/Contents/Helpers/KyberNow Manettes.app"
cp "$ICNS" "$APP/Contents/Resources/AppIcon.icns"
# Moteur du client web (invités du navigateur, /link) : la version officielle
# de Kyber 0.29, puis notre moteur par-dessus, comme la surcouche Windows.
WEB_OFFICIAL="$MAIN/third_party/payload/kyber-0.29.0/webclient"
WEB_ENGINE="$MAIN/third_party/kyber-web-out"
if [ -d "$WEB_OFFICIAL" ] && [ -f "$WEB_ENGINE/kyclient_wasm_bg.wasm" ]; then
    mkdir -p "$APP/Contents/Resources/webclient"
    cp -R "$WEB_OFFICIAL/." "$APP/Contents/Resources/webclient/"
    cp "$WEB_ENGINE/kyclient_wasm.js" "$WEB_ENGINE/kyclient_wasm_bg.wasm" "$APP/Contents/Resources/webclient/"
    cp "$MAIN/third_party/payload/kyber-0.29.0/license.txt" "$APP/Contents/Resources/licenses/Kyber.txt" 2>/dev/null || true
else
    echo "    moteur web absent ($WEB_OFFICIAL, $WEB_ENGINE) : pas d'invité du navigateur" >&2
fi
cp "$ROOT/third_party/kyber-desktop/COPYING.AGPLv3" "$APP/Contents/Resources/licenses/AGPL-3.0.txt"
cp "$ROOT/server/installer/resources/THIRD-PARTY.txt" "$APP/Contents/Resources/licenses/"
info_plist "KyberNow Station" "$BUNDLE_ID" "KyberNow Station" "    <key>LSUIElement</key><true/>
    <key>NSMicrophoneUsageDescription</key><string>KyberNow Station lit le son de la session sur la sortie KyberNow pour l'envoyer à vos appareils. Aucun vrai micro n'est écouté.</string>
" > "$APP/Contents/Info.plist"
localized "$APP" "KyberNow Station" "KyberNow Station" \
    "\"NSMicrophoneUsageDescription\" = \"KyberNow Station lit le son de la session sur la sortie KyberNow pour l'envoyer à vos appareils. Aucun vrai micro n'est écouté.\";
" "\"NSMicrophoneUsageDescription\" = \"KyberNow Station reads the session's sound on the KyberNow output to send it to your devices. No real microphone is listened to.\";
"
# Signature de l'intérieur vers l'extérieur : bibliothèques, sortie son,
# exécutables (même droits : le son), puis le paquet.
find "$APP/Contents/Frameworks" -type f -name '*.dylib' -print0 | while IFS= read -r -d '' lib; do sign "$lib"; done
sign "$APP/Contents/Resources/KyberNow.driver"
for exe in kycontroller kyavserver kynputserver kyber-admin kybernow-display kybernow-ui kybernow-console; do
    sign --entitlements "$ENTITLEMENTS" -i "$BUNDLE_ID.$exe" "$APP/Contents/MacOS/$exe"
done
sign --entitlements "$ENTITLEMENTS" "$APP"
codesign --verify --deep --strict "$APP"
if [ "$NOTARIZE" = 1 ]; then
    step "Notarisation de l'app (Apple, quelques minutes)"
    ditto -c -k --keepParent "$APP" "$WORK/notarisation-app.zip"
    xcrun notarytool submit "$WORK/notarisation-app.zip" --keychain-profile kybernow --wait --timeout 30m
    rm "$WORK/notarisation-app.zip"
    xcrun stapler staple "$APP"
fi

step "KyberNow-Station-Mac-update.zip"
# Paquet de mise à jour : l'app seule (Admin la vérifie contre le manifeste
# signé, l'applicateur revérifie sa signature avant de la mettre en place).
rm -f "$OUT/KyberNow-Station-Mac-update.zip"
ditto -c -k --keepParent "$APP" "$OUT/KyberNow-Station-Mac-update.zip"

# Installateur et désinstalleur : la même interface (kybernow-ui).
window_app() {
    local path="$1" id="$2" fr="$3" en="$4" extra="${5:-}"
    rm -rf "$path"
    mkdir -p "$path/Contents/MacOS" "$path/Contents/Resources"
    cp "$UI" "$path/Contents/MacOS/kybernow-ui"
    cp "$ICNS" "$path/Contents/Resources/AppIcon.icns"
    info_plist kybernow-ui "$id" "$en" "    <key>LSHasLocalizedDisplayName</key><true/>
$extra" > "$path/Contents/Info.plist"
    localized "$path" "$fr" "$en"
}

step "Install KyberNow Station.app"
INSTALLER="$OUT/Install KyberNow Station.app"
window_app "$INSTALLER" "$BUNDLE_ID.installer" "Installer KyberNow Station" "Install KyberNow Station"
ditto "$APP" "$INSTALLER/Contents/Resources/KyberNow Station.app"
sign "$INSTALLER/Contents/MacOS/kybernow-ui"
sign "$INSTALLER"
codesign --verify --deep --strict "$INSTALLER"

step "Uninstall KyberNow Station.app"
UNINSTALLER="$OUT/Uninstall KyberNow Station.app"
window_app "$UNINSTALLER" "$BUNDLE_ID.uninstaller" "Désinstaller KyberNow Station" "Uninstall KyberNow Station" \
    "    <key>KyberNowMode</key><string>uninstall</string>
"
sign "$UNINSTALLER/Contents/MacOS/kybernow-ui"
sign "$UNINSTALLER"
codesign --verify --deep --strict "$UNINSTALLER"

step "KyberNow-Station-Mac.dmg"
DMG="$OUT/KyberNow-Station-Mac.dmg"
STAGE="$WORK/dmg"
rm -rf "$STAGE" "$DMG"
mkdir -p "$STAGE"
ditto "$INSTALLER" "$STAGE/Install KyberNow Station.app"
ditto "$UNINSTALLER" "$STAGE/Uninstall KyberNow Station.app"
hdiutil create -quiet -volname "KyberNow Station" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
codesign -f -s "$IDENTITY" --timestamp "$DMG"

if [ "$NOTARIZE" = 1 ]; then
    step "Notarisation du .dmg (Apple, quelques minutes)"
    xcrun notarytool submit "$DMG" --keychain-profile kybernow --wait --timeout 30m
    xcrun stapler staple "$DMG"
    spctl --assess --type open --context context:primary-signature -vv "$DMG"
fi

step "Fait"
du -sh "$APP" "$DMG" "$OUT/KyberNow-Station-Mac-update.zip" | sed 's|'"$OUT"'/||'
echo "$DMG"
