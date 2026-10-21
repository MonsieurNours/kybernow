#!/bin/bash
# Installe KyberNow sur votre iPhone ou votre iPad, signée avec votre propre
# compte Apple, en attendant TestFlight et l'App Store. Mode d'emploi :
# apps/ios/SIDELOAD.md.
#
#   apps/ios/scripts/sideload.sh --team ABCDE12345
#   apps/ios/scripts/sideload.sh --team ABCDE12345 --full --device "iPad de Léa"
#
#   --team ID      identifiant d'équipe de votre compte Apple (Xcode › Réglages › Comptes)
#   --bundle ID    identifiant de l'app (défaut : kybernow.<équipe>)
#   --full         compte payant : garde iCloud et le Trousseau partagé (ajout des
#                  ordinateurs « Depuis votre Mac ») ; sans lui, l'app s'installe
#                  avec un compte gratuit, sans ces deux-là
#   --device NOM   nom ou identifiant de l'appareil (défaut : le seul branché)
#   --version X    version du SDK à télécharger (défaut : la dernière publiée)
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
        -h|--help) sed -n 2,16p "$0"; exit 0 ;;
        *) echo "Option inconnue : $1 (voir --help)" >&2; exit 1 ;;
    esac
done
[ -n "$TEAM" ] || { echo "Il faut --team (Xcode › Réglages › Comptes › votre équipe)" >&2; exit 1; }
BUNDLE="${BUNDLE:-kybernow.$TEAM}"

command -v xcodebuild >/dev/null || { echo "Xcode est nécessaire (App Store du Mac)" >&2; exit 1; }
command -v xcodegen >/dev/null || { echo "XcodeGen est nécessaire : brew install xcodegen" >&2; exit 1; }

# 1. Le cœur KyberNow, libkynput, libopus et leurs en-têtes pour iOS : compilés
#    sur place s'ils sont là (README.md), sinon ceux de la version publiée.
if [ ! -d "$HERE/Vendor/KyberNow.xcframework" ]; then
    echo "==> SDK KyberNow pour iOS (téléchargement)"
    if [ "$VERSION" = latest ]; then
        URL="https://github.com/$REPO/releases/latest/download/KyberSDK-iOS.zip"
    else
        URL="https://github.com/$REPO/releases/download/v$VERSION/KyberSDK-iOS.zip"
    fi
    TMP="$(mktemp -d)"
    if ! curl -fL --progress-bar "$URL" -o "$TMP/KyberSDK-iOS.zip"; then
        echo "KyberSDK-iOS.zip introuvable pour cette version : il est joint aux versions publiées à partir de la 0.3.2" >&2
        echo "(--version X.Y.Z pour une autre version, ou compilez le SDK vous-même : apps/ios/README.md)" >&2
        rm -rf "$TMP"
        exit 1
    fi
    mkdir -p "$HERE/Vendor"
    ditto -x -k "$TMP/KyberSDK-iOS.zip" "$HERE/Vendor"
    rm -rf "$TMP"
    [ -d "$HERE/Vendor/KyberNow.xcframework" ] || { echo "Archive du SDK inattendue" >&2; exit 1; }
fi

# 2. Projet à votre nom : équipe, identifiants, droits selon le compte.
echo "==> Projet pour l'équipe $TEAM ($BUNDLE)"
SPEC="$HERE/project.sideload.yml"
mkdir -p "$HERE/build/sideload"
python3 - "$HERE/project.yml" "$SPEC" "$TEAM" "$BUNDLE" "$FULL" <<'PY'
import re, sys
src, dst, team, bundle, full = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5] == "1"
s = open(src).read()
s = s.replace("name: KyberNowIOS\n", "name: KyberNowIOSSideload\n", 1)
s = s.replace("DEVELOPMENT_TEAM: P6DB682S4K", f"DEVELOPMENT_TEAM: {team}")
s = s.replace("fr.leslignesclaires.KyberKit", f"{bundle}.kyberkit")
s = s.replace("fr.leslignesclaires.kybernow", bundle)
# Info.plist et droits écrits à part : le projet de l'app reste intact.
s = s.replace("      - path: KyberNowIOS\n", "      - path: KyberNowIOS\n        excludes: [Info.plist, KyberNowIOS.entitlements]\n", 1)
s = s.replace("path: KyberNowIOS/Info.plist", "path: build/sideload/KyberNowIOS-Info.plist")
s = s.replace("path: KyberNowIOS/KyberNowIOS.entitlements", "path: build/sideload/KyberNowIOS.entitlements")
# Signature automatique partout : Xcode crée le profil pour votre équipe
# (le projet d'origine signe le Debug avec un profil propre à l'auteur).
s = re.sub(r"\n      configs:\n(?:        .*\n)+", "\n", s)
if not full:
    # Compte gratuit : ni iCloud, ni Trousseau partagé.
    s = re.sub(r"\n    entitlements:\n(?:      .*\n)+", "\n", s)
open(dst, "w").write(s)
PY
(cd "$HERE" && xcodegen generate --spec project.sideload.yml >/dev/null)

# 3. L'iPhone ou l'iPad branché (ou appairé en Wi-Fi) et vu par Xcode.
LIST="$(mktemp)"
xcrun devicectl list devices --json-output "$LIST" >/dev/null 2>&1 || true
ID="$(python3 - "$LIST" "$DEVICE" <<'PY'
import json, sys
try:
    devices = json.load(open(sys.argv[1]))["result"]["devices"]
except Exception:
    devices = []
want = sys.argv[2].lower()
phones = [d for d in devices if d.get("hardwareProperties", {}).get("platform") == "iOS"
          and d.get("hardwareProperties", {}).get("reality") != "simulated"]
if want:
    phones = [d for d in phones if want in (d.get("deviceProperties", {}).get("name", "").lower(), d.get("identifier", "").lower(),
                                             d.get("hardwareProperties", {}).get("udid", "").lower())]
if len(phones) == 1:
    print(phones[0].get("hardwareProperties", {}).get("udid") or phones[0]["identifier"])
PY
)"
rm -f "$LIST"
[ -n "$ID" ] || { echo "iPhone ou iPad introuvable (ou plusieurs) : branchez-le, déverrouillez-le, acceptez « Se fier à cet ordinateur », ou précisez --device" >&2; exit 1; }

# 4. Compilation signée par Xcode (profil créé tout seul), puis installation.
echo "==> Compilation et signature"
DERIVED="$HERE/build/sideload"
(cd "$HERE" && xcodebuild -project KyberNowIOSSideload.xcodeproj -scheme KyberNowIOS -configuration Release \
    -destination "id=$ID" -derivedDataPath "$DERIVED" -allowProvisioningUpdates build 2>&1 \
    | grep -E "error:|BUILD (SUCCEEDED|FAILED)" || true)
APP="$DERIVED/Build/Products/Release-iphoneos/KyberNow.app"
[ -d "$APP" ] || { echo "La compilation a échoué (relancez avec Xcode ouvert sur KyberNowIOSSideload.xcodeproj pour voir l'erreur)" >&2; exit 1; }

echo "==> Installation sur l'appareil"
xcrun devicectl device install app --device "$ID" "$APP"
echo "KyberNow est installée. Ouvrez-la depuis l'écran d'accueil."
[ "$FULL" = 1 ] || echo "Compte gratuit : au premier lancement, autorisez le développeur dans Réglages › Général › VPN et gestion de l'appareil ; l'app cesse de s'ouvrir au bout de 7 jours, relancez ce script pour la renouveler."
