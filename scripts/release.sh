#!/bin/bash
# Publie une version de KyberNow sur GitHub (github.com/MonsieurNours/kybernow) :
#   scripts/release.sh 0.2.0
#
# 1. Aligne le numéro de version (console, icône, installateur, app Mac, app Apple TV).
# 2. Construit l'ordinateur (installateur, paquet de mise à jour) et l'app Mac,
#    signée Developer ID, notarisée par Apple et agrafée ; compile l'app Apple TV
#    et joint son SDK de Kyber (KyberSDK-tvOS.zip, pour apps/tvos/scripts/sideload.sh).
# 3. Signe le flux de mises à jour de l'app (Sparkle, clé EdDSA du Trousseau).
# 4. Calcule les empreintes (SHA256SUMS), commite, étiquette vX, pousse, et
#    crée la version GitHub avec les notes de CHANGELOG.md.
#
# Ce que téléchargent les mises à jour :
#   - console (Mises à jour > Installer) : kybernow-update.zip, vérifié par SHA256SUMS ;
#   - app Mac (Sparkle) : appcast.xml puis KyberNow-macOS.zip, signé EdDSA ;
#   - à la main : update.ps1 (irm …/releases/latest/download/update.ps1 | iex).
#
# Prérequis : third_party/ prêt (voir docs/build-notes.md), gh connecté,
# clé Sparkle dans le Trousseau (generate_keys), certificat Developer ID et
# profil de notarisation « kybernow » (docs/releases.md), Kyber corrigé déjà compilé
# (server/kyber-patches/build.sh si les correctifs ont changé).
set -euo pipefail
VERSION="${1:?usage : scripts/release.sh X.Y.Z}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version attendue : X.Y.Z" >&2; exit 1; }
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="MonsieurNours/kybernow"
TAG="v$VERSION"
OUT="$ROOT/dist/$TAG"
cd "$ROOT"

grep -q "^## $VERSION\$" CHANGELOG.md || { echo "Ajoutez une section « ## $VERSION » à CHANGELOG.md" >&2; exit 1; }
if gh release view "$TAG" -R "$REPO" >/dev/null 2>&1; then echo "$TAG est déjà publiée" >&2; exit 1; fi
if pgrep -x KyberNow >/dev/null; then echo "Quittez KyberNow avant de construire l'app (⌘Q)" >&2; exit 1; fi

echo "==> Version $VERSION"
for f in server/admin/Cargo.toml server/tray/Cargo.toml server/installer/Cargo.toml; do
    sed -i '' -E "1,10s/^version = \".*\"/version = \"$VERSION\"/" "$f"
done
BUILD=$(date +%Y%m%d%H%M)
for f in apps/macos/project.yml apps/tvos/project.yml; do
    sed -i '' -E "s/MARKETING_VERSION: \".*\"/MARKETING_VERSION: \"$VERSION\"/; s/CURRENT_PROJECT_VERSION: \".*\"/CURRENT_PROJECT_VERSION: \"$BUILD\"/" "$f"
done

echo "==> Ordinateur : installateur et paquet de mise à jour"
server/installer/build.sh > /dev/null
server/installer/package-update.sh

echo "==> App Mac"
(cd apps/macos && xcodegen generate > /dev/null && xcodebuild -project KyberNow.xcodeproj -scheme KyberNow -configuration Release \
    -derivedDataPath build build 2>&1 | grep -E "error:|BUILD (SUCC|FAIL)")
APP="apps/macos/build/Build/Products/Release/KyberNow.app"
apps/macos/scripts/sign-release.sh "$APP"

rm -rf "$OUT" && mkdir -p "$OUT"

echo "==> App Apple TV (SDK de Kyber pour tvOS, compilation de vérification)"
apps/tvos/scripts/build-kyber-tvos.sh > /dev/null
(cd apps/tvos && xcodegen generate > /dev/null && xcodebuild -project KyberNowTV.xcodeproj -scheme KyberNowTV -configuration Release \
    -destination 'generic/platform=tvOS' -derivedDataPath build CODE_SIGNING_ALLOWED=NO build 2>&1 | grep -E "error:|BUILD (SUCC|FAIL)")
(cd apps/tvos/Vendor && ditto -c -k . "$OUT/KyberSDK-tvOS.zip")

echo "==> Notarisation de l'app (Apple, quelques minutes)"
# Profil enregistré une fois : xcrun notarytool store-credentials kybernow --apple-id … --team-id P6DB682S4K
ditto -c -k --keepParent "$APP" "$OUT/notarisation.zip"
xcrun notarytool submit "$OUT/notarisation.zip" --keychain-profile kybernow --wait --timeout 30m
rm "$OUT/notarisation.zip"
xcrun stapler staple "$APP"
spctl --assess --type execute -vv "$APP"

ditto -c -k --keepParent "$APP" "$OUT/KyberNow-macOS.zip"
cp server/installer/dist/KyberNow-Setup.exe server/installer/dist/kybernow-update.zip server/installer/dist/update.ps1 "$OUT/"

echo "==> Flux de mises à jour de l'app (Sparkle)"
SPARKLE=$(ls -d apps/macos/build/SourcePackages/artifacts/*/*/bin | head -1)
NOTES="$(awk -v v="## $VERSION" '$0==v{f=1;next} /^## /{f=0} f' CHANGELOG.md)"
FEED="$(mktemp -d)"
cp "$OUT/KyberNow-macOS.zip" "$FEED/"
printf '%s\n' "$NOTES" > "$FEED/KyberNow-macOS.md"
"$SPARKLE/generate_appcast" --download-url-prefix "https://github.com/$REPO/releases/download/$TAG/" \
    --embed-release-notes -o "$OUT/appcast.xml" "$FEED"
rm -rf "$FEED"

(cd "$OUT" && shasum -a 256 KyberNow-Setup.exe kybernow-update.zip update.ps1 KyberNow-macOS.zip appcast.xml KyberSDK-tvOS.zip > SHA256SUMS)
cat "$OUT/SHA256SUMS"

echo "==> Git et GitHub"
git add server/admin/Cargo.toml server/admin/Cargo.lock server/tray/Cargo.toml server/tray/Cargo.lock \
    server/installer/Cargo.toml server/installer/Cargo.lock apps/macos/project.yml apps/macos/KyberNow.xcodeproj \
    apps/tvos/project.yml apps/tvos/KyberNowTV.xcodeproj CHANGELOG.md
git commit -q -m "KyberNow $VERSION" || true
git tag -a "$TAG" -m "KyberNow $VERSION"
git push -q origin HEAD "$TAG"
printf '%s\n' "$NOTES" | gh release create "$TAG" -R "$REPO" --title "KyberNow $VERSION" --notes-file - \
    "$OUT/KyberNow-Setup.exe" "$OUT/kybernow-update.zip" "$OUT/update.ps1" "$OUT/KyberNow-macOS.zip" \
    "$OUT/appcast.xml" "$OUT/KyberSDK-tvOS.zip" "$OUT/SHA256SUMS"
echo "Publiée : https://github.com/$REPO/releases/tag/$TAG"
