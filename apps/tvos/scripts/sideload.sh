#!/bin/bash
# Installe KyberNow Link sur votre Apple TV, signée avec votre propre compte
# Apple, sans passer par l'App Store. Mode d'emploi : apps/tvos/SIDELOAD.md.
#
#   apps/tvos/scripts/sideload.sh --team ABCDE12345
#   apps/tvos/scripts/sideload.sh --team ABCDE12345 --full --device "Salon"
#
#   --team ID      identifiant d'équipe de votre compte Apple (Xcode › Réglages › Comptes)
#   --bundle ID    identifiant de l'app (défaut : kybernow.<équipe>)
#   --full         compte payant : garde iCloud, le Top Shelf et les profils tvOS
#                  (sans lui, l'app s'installe avec un compte gratuit, sans ces trois-là)
#   --device NOM   nom ou identifiant de l'Apple TV (défaut : la seule appairée)
#   --version X    version du SDK à télécharger si Vendor/ ne l'a pas (défaut : la dernière publiée)
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
REPO="MonsieurNours/kybernow"
TEAM="" BUNDLE="" FULL=0 DEVICE="" VERSION="latest"
while [ $# -gt 0 ]; do
    case "$1" in
        --team) TEAM="$2"; shift 2 ;;
        --bundle) BUNDLE="$2"; shift 2 ;;
        --full) FULL=1; shift ;;
        --device) DEVICE="$2"; shift 2 ;;
        --version) VERSION="$2"; shift 2 ;;
        -h|--help) sed -n 2,15p "$0"; exit 0 ;;
        *) echo "Option inconnue : $1 (voir --help)" >&2; exit 1 ;;
    esac
done
[ -n "$TEAM" ] || { echo "Il faut --team (Xcode › Réglages › Comptes › votre équipe)" >&2; exit 1; }
BUNDLE="${BUNDLE:-kybernow.$TEAM}"

command -v xcodebuild >/dev/null || { echo "Xcode est nécessaire (App Store du Mac)" >&2; exit 1; }
command -v xcodegen >/dev/null || { echo "XcodeGen est nécessaire : brew install xcodegen" >&2; exit 1; }

# 1. SDK de Kyber et cœur KyberNow pour tvOS : compilés sur place s'ils sont là, sinon ceux de la version publiée.
if [ ! -d "$HERE/Vendor/KyberSDK.xcframework" ] || [ ! -d "$HERE/Vendor/KyberNow.xcframework" ]; then
    echo "==> SDK de Kyber et cœur KyberNow pour l'Apple TV (téléchargement)"
    if [ "$VERSION" = latest ]; then
        URL="https://github.com/$REPO/releases/latest/download/KyberSDK-tvOS.zip"
    else
        URL="https://github.com/$REPO/releases/download/v$VERSION/KyberSDK-tvOS.zip"
    fi
    TMP="$(mktemp -d)"
    curl -fL --progress-bar "$URL" -o "$TMP/KyberSDK-tvOS.zip"
    mkdir -p "$HERE/Vendor"
    ditto -x -k "$TMP/KyberSDK-tvOS.zip" "$HERE/Vendor"
    rm -rf "$TMP"
    [ -d "$HERE/Vendor/KyberSDK.xcframework" ] || { echo "Archive du SDK inattendue (KyberSDK.xcframework absent)" >&2; exit 1; }
    [ -d "$HERE/Vendor/KyberNow.xcframework" ] || { echo "Archive du SDK inattendue (KyberNow.xcframework absent : version antérieure à la 0.3.1 ?)" >&2; exit 1; }
fi

# 2. Projet à votre nom : équipe, identifiants, droits selon le compte.
echo "==> Projet pour l'équipe $TEAM ($BUNDLE)"
SPEC="$HERE/project.sideload.yml"
mkdir -p "$HERE/build/sideload"
python3 - "$HERE/project.yml" "$SPEC" "$TEAM" "$BUNDLE" "$FULL" <<'EOF'
import re, sys
src, dst, team, bundle, full = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5] == "1"
s = open(src).read()
s = s.replace("name: KyberNowTV\n", "name: KyberNowTVSideload\n", 1)
s = s.replace("DEVELOPMENT_TEAM: P6DB682S4K", f"DEVELOPMENT_TEAM: {team}")
s = s.replace("group.fr.leslignesclaires.kybernow", f"group.{bundle}")
s = s.replace("fr.leslignesclaires.KyberKit", f"{bundle}.kyberkit")
s = s.replace("fr.leslignesclaires.kybernow", bundle)
# Info.plist et droits écrits à part : le projet de l'app reste intact.
s = s.replace("      - path: KyberNowTV\n", "      - path: KyberNowTV\n        excludes: [Info.plist, KyberNowTV.entitlements]\n", 1)
s = s.replace("      - path: TopShelf\n", "      - path: TopShelf\n        excludes: [Info.plist, TopShelf.entitlements]\n", 1)
s = s.replace("path: KyberNowTV/Info.plist", "path: build/sideload/KyberNowTV-Info.plist")
s = s.replace("path: TopShelf/Info.plist", "path: build/sideload/TopShelf-Info.plist")
s = s.replace("path: KyberNowTV/KyberNowTV.entitlements", "path: build/sideload/KyberNowTV.entitlements")
s = s.replace("path: TopShelf/TopShelf.entitlements", "path: build/sideload/TopShelf.entitlements")
if not full:
    # Compte gratuit : ni iCloud, ni groupe d'apps, ni profils tvOS.
    s = re.sub(r"\n    entitlements:\n(?:      .*\n)+", "\n", s)
open(dst, "w").write(s)
EOF
(cd "$HERE" && xcodegen generate --spec project.sideload.yml >/dev/null)

# 3. L'Apple TV appairée avec Xcode.
LIST="$(mktemp)"
xcrun devicectl list devices --json-output "$LIST" >/dev/null 2>&1 || true
ID="$(python3 - "$LIST" "$DEVICE" <<'EOF'
import json, sys
try:
    devices = json.load(open(sys.argv[1]))["result"]["devices"]
except Exception:
    devices = []
want = sys.argv[2].lower()
tvs = [d for d in devices if d.get("hardwareProperties", {}).get("platform") == "tvOS"]
if want:
    tvs = [d for d in tvs if want in (d.get("deviceProperties", {}).get("name", "").lower(), d.get("identifier", "").lower(),
                                       d.get("hardwareProperties", {}).get("udid", "").lower())]
if len(tvs) == 1:
    print(tvs[0].get("hardwareProperties", {}).get("udid") or tvs[0]["identifier"])
EOF
)"
rm -f "$LIST"
[ -n "$ID" ] || { echo "Apple TV introuvable (ou plusieurs) : appairez-la dans Xcode › Window › Devices and Simulators, ou précisez --device" >&2; exit 1; }

# 4. Compilation signée par Xcode (profil créé tout seul), puis installation.
echo "==> Compilation et signature"
DERIVED="$HERE/build/sideload"
(cd "$HERE" && xcodebuild -project KyberNowTVSideload.xcodeproj -scheme KyberNowTV -configuration Release \
    -destination "id=$ID" -derivedDataPath "$DERIVED" -allowProvisioningUpdates build 2>&1 \
    | grep -E "error:|BUILD (SUCCEEDED|FAILED)" || true)
APP="$DERIVED/Build/Products/Release-appletvos/KyberNow.app"
[ -d "$APP" ] || { echo "La compilation a échoué (relancez avec Xcode ouvert sur KyberNowTVSideload.xcodeproj pour voir l'erreur)" >&2; exit 1; }

echo "==> Installation sur l'Apple TV"
xcrun devicectl device install app --device "$ID" "$APP"
echo "KyberNow est installée. Ouvrez-la depuis l'accueil de l'Apple TV (réveillez-la d'abord si elle est en veille)."
[ "$FULL" = 1 ] || echo "Compte gratuit : l'app cesse de s'ouvrir au bout de 7 jours ; relancez ce script pour la renouveler."
