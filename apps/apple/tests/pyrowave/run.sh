#!/bin/bash
# Essai hors ligne de PyroWave sur une Apple TV, un iPhone ou un iPad
# (docs/pyrowave-client-apple.md) : construit l'app d'essai PyroWaveBench
# (identifiant fr.leslignesclaires.kybernow.pyrowavebench, à côté de
# KyberNow Link sans la remplacer), l'installe, la lance et affiche son
# résultat (code de sortie 0 : tout bon).
#
#   run.sh tv  <appareil> [1920x1080,3840x2160]   Apple TV
#   run.sh ios <appareil> [1920x1080]             iPhone, iPad
#
# <appareil> : nom ou identifiant de `xcrun devicectl list devices`.
# L'appareil doit être éveillé (tvOS refuse de lancer une app pendant la veille).
#
# Signature : Apple TV avec le profil d'équipe générique de tvOS (« tvOS
# Team Provisioning Profile: * ») s'il est installé et couvre l'appareil ;
# sinon (et pour l'iPhone, l'iPad) signature automatique d'Xcode
# (-allowProvisioningUpdates), qui enregistre l'identifiant de l'app d'essai
# sur le compte développeur la première fois.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PLATFORM="${1:-}"; DEVICE="${2:-}"; SIZES="${3:-}"
case "$PLATFORM" in
    tv) SCHEME=PyroWaveBenchTV; SDK=appletvos; DEST="generic/platform=tvOS"; PROFILE_PLATFORM=tvOS ;;
    ios) SCHEME=PyroWaveBenchIOS; SDK=iphoneos; DEST="generic/platform=iOS"; PROFILE_PLATFORM=iOS ;;
    *) echo "Usage : $0 tv|ios <appareil> [définitions]" >&2; exit 2 ;;
esac
[ -n "$DEVICE" ] || { echo "Appareil manquant (xcrun devicectl list devices)" >&2; exit 2; }
BUNDLE=fr.leslignesclaires.kybernow.pyrowavebench
TEAM=P6DB682S4K
WORK="${TMPDIR:-/tmp}/pyrowavebench"
APP="$WORK/Build/Products/Release-$SDK/PyroWaveBench.app"

UDID=$(xcrun devicectl list devices 2>/dev/null | awk -v d="$DEVICE" 'index($0, d) { for (i = 1; i <= NF; i++) if ($i == "(UDID)") print $(i-1) }' | head -1)
[ -n "$UDID" ] || { echo "Appareil introuvable : $DEVICE" >&2; exit 2; }

(cd "$HERE" && xcodegen generate -q)

# Profil générique de tvOS qui couvre l'appareil.
PROFILE=""
for f in "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"/*.mobileprovision; do
    [ -f "$f" ] || continue
    plist=$(security cms -D -i "$f" 2>/dev/null) || continue
    [ "$(plutil -extract Entitlements.application-identifier raw -o - - <<<"$plist" 2>/dev/null)" = "$TEAM.*" ] || continue
    [ "$(plutil -extract Platform.0 raw -o - - <<<"$plist" 2>/dev/null)" = "$PROFILE_PLATFORM" ] || continue
    grep -q "$UDID" <<<"$plist" && PROFILE="$f" && break
done

echo "==> PyroWaveBench ($SCHEME) pour $DEVICE ($UDID)"
if [ -n "$PROFILE" ]; then
    xcodebuild -project "$HERE/PyroWaveBench.xcodeproj" -scheme "$SCHEME" -configuration Release -sdk "$SDK" -destination "$DEST" \
        -derivedDataPath "$WORK" CODE_SIGNING_ALLOWED=NO build 2>&1 | grep -E "error:|BUILD FAILED" && exit 1
    cert=$(security cms -D -i "$PROFILE" | plutil -extract DeveloperCertificates.0 raw -o - - | base64 -d | shasum | cut -d' ' -f1 | tr a-f A-F)
    cp "$PROFILE" "$APP/embedded.mobileprovision"
    cat > "$WORK/bench.entitlements" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>application-identifier</key><string>$TEAM.$BUNDLE</string>
<key>com.apple.developer.team-identifier</key><string>$TEAM</string>
<key>get-task-allow</key><true/>
</dict></plist>
EOF
    codesign --force --sign "$cert" --entitlements "$WORK/bench.entitlements" --timestamp=none "$APP"
else
    xcodebuild -project "$HERE/PyroWaveBench.xcodeproj" -scheme "$SCHEME" -configuration Release -sdk "$SDK" -destination "$DEST" \
        -derivedDataPath "$WORK" -allowProvisioningUpdates CODE_SIGN_STYLE=Automatic build 2>&1 | grep -E "error:|BUILD FAILED" && exit 1
fi

echo "==> installation"
xcrun devicectl device install app --device "$UDID" "$APP" >/dev/null
echo "==> lancement (une à quelques minutes : chaque cas est encodé puis décodé sur l'appareil)"
args=()
[ -n "$SIZES" ] && args=(-sizes "$SIZES")
xcrun devicectl device process launch --console --terminate-existing --device "$UDID" "$BUNDLE" ${args[@]+"${args[@]}"}
