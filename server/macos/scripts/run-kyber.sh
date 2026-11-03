#!/bin/bash
# Lance le serveur Kyber compilé par build-kyber.sh sur ce Mac, pour les essais
# de KyberNow Station pour Mac (jalon 1 de docs/station-macos.md), sans Admin.
#
#   run-kyber.sh start   empaquette, signe et lance (launchd, session de l'utilisateur)
#   run-kyber.sh stop    arrête
#   run-kyber.sh status  état et fin du journal
#   run-kyber.sh package empaquette et signe seulement (utilisé par run-station.sh)
#
# Le serveur tourne en LaunchAgent, comme le fera la Station : macOS demande
# alors l'autorisation d'enregistrer l'écran pour lui (« KyberNow Station
# (Kyber) »), pas pour le terminal qui l'a lancé. Signé Developer ID : une fois
# donnée, l'autorisation reste après chaque recompilation. Rien n'est installé
# dans ~/Library/LaunchAgents : le service disparaît à la fermeture de session.
#
# Compte d'essai de Kyber : kybernow / essai-kybernow (KYBER_TEST_PASSWORD pour
# un autre mot de passe). Dossier d'essai : $KYBER_RUN_DIR (défaut :
# ~/Library/Application Support/KyberNow/essai-kyber), avec kyber_config.toml et
# les journaux.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
KYBER_DIR="$(cd "${KYBER_DIR:-$ROOT/third_party/kyber-desktop}" && pwd -P)"
OUT="$KYBER_DIR/rootfs-arm64-macos"
APP="$KYBER_DIR/KyberNow-Station-Kyber.app"
RUN="${KYBER_RUN_DIR:-$HOME/Library/Application Support/KyberNow/essai-kyber}"
LABEL="fr.leslignesclaires.kybernow.station.kyber"
BUNDLE_ID="fr.leslignesclaires.kybernow.station.kyber"
IDENTITY="${KYBER_SIGN_IDENTITY:-Developer ID Application: Mathieu Requenna (P6DB682S4K)}"
PASSWORD="${KYBER_TEST_PASSWORD:-essai-kybernow}"
DOMAIN="gui/$(id -u)"

export PATH="/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export CARGO_PROFILE_RELEASE_STRIP=false

stop() {
    launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null && echo "Kyber arrêté" || echo "Kyber n'était pas lancé"
}

package() {
    [ -x "$OUT/bin/kycontroller" ] || { echo "Lancer d'abord server/macos/scripts/build-kyber.sh" >&2; exit 1; }
    rm -rf "$APP"
    # package_server de Kyber : binaires, Info.plist et dépendances dans Frameworks/.
    (cd "$KYBER_DIR" && KYBER_SERVER_SKIP_DMG=1 KYBER_SERVER_APP_DIRECTORY="$APP" \
        ./build-macos.sh -s -p -a arm64 -o "$OUT" server >/dev/null)
    [ -d "$APP" ] || { echo "Empaquetage raté ($APP absent)" >&2; exit 1; }
    # Hors des exécutables (sinon la signature du paquet échoue) : réglages et certificats.
    mv "$APP"/Contents/MacOS/*.pem "$APP"/Contents/MacOS/kyber_config.toml "$APP/Contents/Resources/"
    rm -f "$KYBER_DIR/KyberNow-Station-Kyber.app.zip"
    plutil -replace CFBundleIdentifier -string "$BUNDLE_ID" "$APP/Contents/Info.plist"
    plutil -replace CFBundleName -string "KyberNow Station (Kyber)" "$APP/Contents/Info.plist"
    plutil -replace CFBundleVersion -string "0.29.0" "$APP/Contents/Info.plist"
    # Bibliothèques d'abord, puis les exécutables, puis le paquet.
    find "$APP/Contents/Frameworks" -type f -name '*.dylib' -exec codesign -f -s "$IDENTITY" {} \;
    for exe in kyavserver kynputserver; do
        codesign -f -s "$IDENTITY" -i "$BUNDLE_ID.$exe" "$APP/Contents/MacOS/$exe"
    done
    codesign -f -s "$IDENTITY" "$APP"
    codesign --verify --strict "$APP"
}

config() {
    mkdir -p "$RUN"
    local hash
    hash=$(printf '%s' "$PASSWORD" | shasum -a 256 | cut -d' ' -f1)
    python3 - "$APP/Contents/Resources/kyber_config.toml" "$RUN/kyber_config.toml" "$hash" "$APP/Contents/Resources" <<'EOF'
import sys
src, dst, h, d = sys.argv[1:]
t = open(src).read()
t = t.replace('{ username = "Default user", password = "c2bb5d1edbff196e3a879ec0a59aeb3952822266909b5ba07b45072d20a05446" },',
              '{ username = "kybernow", password = "%s" },' % h)
t = t.replace('#tls_cert = "/path/to/server_cert.pem"', 'tls_cert = "%s/kybertest_cert.pem"' % d)
t = t.replace('#tls_key = "/path/to/server_key.pem"', 'tls_key = "%s/kybertest_key.pem"' % d)
open(dst, 'w').write(t)
EOF
    cat > "$RUN/$LABEL.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>$LABEL</string>
    <key>ProgramArguments</key><array><string>$APP/Contents/MacOS/kycontroller</string></array>
    <key>EnvironmentVariables</key><dict><key>KYBER_CONFIG</key><string>$RUN/kyber_config.toml</string></dict>
    <key>WorkingDirectory</key><string>$RUN</string>
    <key>StandardOutPath</key><string>$RUN/kyber.log</string>
    <key>StandardErrorPath</key><string>$RUN/kyber.log</string>
    <key>RunAtLoad</key><true/>
    <key>ProcessType</key><string>Interactive</string>
    <key>LimitLoadToSessionType</key><string>Aqua</string>
</dict>
</plist>
EOF
}

case "${1:-}" in
    start)
        stop >/dev/null
        package
        config
        : > "$RUN/kyber.log"
        launchctl bootstrap "$DOMAIN" "$RUN/$LABEL.plist"
        echo "Kyber lancé : port 8080, compte kybernow, journal $RUN/kyber.log"
        ;;
    package) package ;;
    stop) stop ;;
    status)
        launchctl print "$DOMAIN/$LABEL" 2>/dev/null | grep -E "state|pid" | head -3 || echo "Kyber n'est pas lancé"
        tail -20 "$RUN/kyber.log" 2>/dev/null || true
        ;;
    *) echo "usage : $0 start|stop|status|package" >&2; exit 2 ;;
esac
