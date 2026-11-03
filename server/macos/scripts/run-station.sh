#!/bin/bash
# Station Mac d'essai (jalon 2 de docs/station-macos.md) : Admin et Kyber en
# LaunchAgents de la session de l'utilisateur, comme la future Station, pour
# s'y connecter depuis les apps KyberNow Link (Bonjour, appairage, session).
#
#   run-station.sh start   compile Admin et son interface, empaquette Kyber,
#                          initialise la Station au premier lancement, lance tout
#   run-station.sh stop    arrête Admin et Kyber
#   run-station.sh status  état et fin des journaux
#
# Prérequis : server/macos/scripts/build-kyber.sh (Kyber compilé), node et npm.
# Dossier : $STATION_DIR (défaut : ~/Library/Application Support/KyberNow/station-essai)
#   admin/   données d'Admin (réglages, certificat, comptes)
#   kyber/   kyber_config.toml, certificat de Kyber, journaux, plist du LaunchAgent
#   bin/     copie de kyber-admin (une recompilation ne gêne pas la Station en marche)
# Mot de passe (Admin et compte Kyber « kybernow ») : $KYBERNOW_PASSWORD au
# premier lancement, sinon tiré au hasard et écrit dans admin/mot-de-passe-essai
# (lisible par vous seul). Rien n'est installé dans ~/Library/LaunchAgents : les
# services disparaissent à la fermeture de session.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
KYBER_DIR_SRC="$(cd "${KYBER_DIR:-$ROOT/third_party/kyber-desktop}" && pwd -P)"
APP="$KYBER_DIR_SRC/KyberNow-Station-Kyber.app"
BASE="${STATION_DIR:-$HOME/Library/Application Support/KyberNow/station-essai}"
DATA="$BASE/admin"
KYBER="$BASE/kyber"
BIN="$BASE/bin"
KYBER_LABEL="fr.leslignesclaires.kybernow.station.kyber"
ADMIN_LABEL="fr.leslignesclaires.kybernow.station.admin"
KYBER_PLIST="$KYBER/$KYBER_LABEL.plist"
ADMIN_PLIST="$DATA/$ADMIN_LABEL.plist"
DOMAIN="gui/$(id -u)"

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

stop() {
    local label
    for label in "$ADMIN_LABEL" "$KYBER_LABEL"; do
        launchctl bootout "$DOMAIN/$label" 2>/dev/null && echo "$label arrêté" || true
    done
}

build_admin() {
    echo "==> Interface d'Admin"
    (cd "$ROOT/server/web" && npm ci --silent && npm run build --silent)
    echo "==> kyber-admin"
    (cd "$ROOT/server/admin" && touch src/assets.rs && cargo build --release --quiet)
    mkdir -p "$BIN"
    cp "$ROOT/server/admin/target/release/kyber-admin" "$BIN/kyber-admin"
}

# Première fois : réglages de Kyber, certificat propre à cette Station (pas le
# certificat d'essai public de Kyber), comptes et clé JWT posés par `init`.
first_setup() {
    [ -f "$DATA/kyber-admin.toml" ] && return
    echo "==> Première initialisation de la Station d'essai"
    mkdir -p "$DATA" "$KYBER/log"
    chmod 700 "$BASE"
    cp "$APP/Contents/Resources/kyber_config.toml" "$KYBER/kyber_config.toml"
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 3650 \
        -subj "/CN=KyberNow Station" -keyout "$KYBER/kyber_key.pem" -out "$KYBER/kyber_cert.pem" 2>/dev/null
    chmod 600 "$KYBER/kyber_key.pem"
    python3 - "$KYBER/kyber_config.toml" "$KYBER" <<'EOF'
import sys
path, d = sys.argv[1:]
t = open(path).read()
t = t.replace('#tls_cert = "/path/to/server_cert.pem"', 'tls_cert = "%s/kyber_cert.pem"' % d)
t = t.replace('#tls_key = "/path/to/server_key.pem"', 'tls_key = "%s/kyber_key.pem"' % d)
open(path, 'w').write(t)
EOF
    local password="${KYBERNOW_PASSWORD:-}"
    if [ -z "$password" ]; then
        password=$(openssl rand -base64 12 | tr -d '/+=' | cut -c1-12)
        (umask 077 && printf '%s\n' "$password" > "$DATA/mot-de-passe-essai")
        echo "    mot de passe tiré au hasard : $DATA/mot-de-passe-essai"
    fi
    KYBERNOW_PASSWORD="$password" "$BIN/kyber-admin" init --data-dir "$DATA" --kyber-dir "$KYBER" \
        --user kybernow --kyber-port 8080 --encoder videotoolbox
}

plists() {
    cat > "$KYBER_PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>$KYBER_LABEL</string>
    <key>ProgramArguments</key><array><string>$APP/Contents/MacOS/kycontroller</string></array>
    <key>EnvironmentVariables</key><dict><key>KYBER_CONFIG</key><string>$KYBER/kyber_config.toml</string></dict>
    <key>WorkingDirectory</key><string>$KYBER</string>
    <key>StandardOutPath</key><string>$KYBER/log/kyber.log</string>
    <key>StandardErrorPath</key><string>$KYBER/log/kyber.log</string>
    <key>RunAtLoad</key><true/>
    <key>ProcessType</key><string>Interactive</string>
    <key>LimitLoadToSessionType</key><string>Aqua</string>
</dict>
</plist>
EOF
    cat > "$ADMIN_PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>$ADMIN_LABEL</string>
    <key>ProgramArguments</key><array>
        <string>$BIN/kyber-admin</string>
        <string>--data-dir</string><string>$DATA</string>
        <string>--kyber-dir</string><string>$KYBER</string>
    </array>
    <key>EnvironmentVariables</key><dict><key>KYBERNOW_KYBER_PLIST</key><string>$KYBER_PLIST</string></dict>
    <key>WorkingDirectory</key><string>$DATA</string>
    <key>StandardOutPath</key><string>$DATA/admin.log</string>
    <key>StandardErrorPath</key><string>$DATA/admin.log</string>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
    <key>LimitLoadToSessionType</key><string>Aqua</string>
</dict>
</plist>
EOF
}

case "${1:-}" in
    start)
        stop >/dev/null
        build_admin
        echo "==> Kyber"
        "$HERE/run-kyber.sh" package >/dev/null 2>&1 || { echo "Empaquetage de Kyber raté (relancer server/macos/scripts/run-kyber.sh package)" >&2; exit 1; }
        first_setup
        plists
        launchctl bootstrap "$DOMAIN" "$KYBER_PLIST"
        launchctl bootstrap "$DOMAIN" "$ADMIN_PLIST"
        echo "Station d'essai lancée : Admin sur https://$(ipconfig getifaddr en0 2>/dev/null || echo localhost):9443, Kyber sur le port 8080"
        ;;
    stop) stop ;;
    status)
        for label in "$KYBER_LABEL" "$ADMIN_LABEL"; do
            printf '%s : ' "$label"
            launchctl print "$DOMAIN/$label" 2>/dev/null | awk '/^\tstate = /{print $3, $4; exit}' || true
            launchctl print "$DOMAIN/$label" >/dev/null 2>&1 || echo "pas lancé"
        done
        echo "--- Admin"; tail -10 "$DATA/admin.log" 2>/dev/null || true
        echo "--- Kyber"; tail -10 "$KYBER/log/kyber.log" 2>/dev/null || true
        ;;
    *) echo "usage : $0 start|stop|status" >&2; exit 2 ;;
esac
