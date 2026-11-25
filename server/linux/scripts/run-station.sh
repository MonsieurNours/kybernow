#!/bin/bash
# Station Linux d'essai (docs/station-linux.md) : Kyber et Admin en services
# systemd de l'utilisateur, dans sa session graphique (GNOME), comme la
# Station installée par le paquet, pour s'y connecter depuis les apps
# KyberNow Link (appairage, session).
#
#   run-station.sh start   compile Admin et l'agent des écrans, copie Kyber,
#                          initialise la Station au premier lancement, lance tout
#   run-station.sh stop    arrête Admin et Kyber
#   run-station.sh status  état et fin des journaux
#
# À lancer dans la machine Linux (la VM « KyberNow Linux » : par ssh, les
# services démarrent dans la session GNOME ouverte, celle de systemd --user).
# Prérequis : server/linux/scripts/build-kyber.sh (Kyber compilé dans
# $KYBER_DIR/rootfs-<arch>-linux), Rust, et l'interface d'Admin déjà compilée
# (server/web/dist : `npm ci && npm run build` sur le Mac suffit, le dossier
# est partagé).
#
# Dossier : $STATION_DIR (défaut : ~/.local/share/kybernow/station-essai)
#   admin/   données d'Admin (réglages, certificat, comptes)
#   kyber/   kyber_config.toml, certificat de Kyber, journaux
#   runtime/ copie de Kyber (programmes et bibliothèques)
#   bin/     kyber-admin et kybernow-display (une recompilation ne gêne pas
#            la Station en marche)
# Mot de passe (Admin et compte Kyber « kybernow ») : $KYBERNOW_PASSWORD au
# premier lancement, sinon tiré au hasard et écrit dans admin/mot-de-passe-essai.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
KYBER_DIR_SRC="$(cd "${KYBER_DIR:-$ROOT/third_party/kyber-desktop-linux}" 2>/dev/null && pwd -P || true)"
ARCH="$(uname -m)"
RT_SRC="$KYBER_DIR_SRC/rootfs-$ARCH-linux"
BASE="${STATION_DIR:-$HOME/.local/share/kybernow/station-essai}"
DATA="$BASE/admin"
KYBER="$BASE/kyber"
RT="$BASE/runtime"
BIN="$BASE/bin"
UNITS="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
TARGET="${CARGO_TARGET_DIR:-$HOME/.cache/kybernow-station-target}"

export PATH="$HOME/.cargo/bin:$PATH"
# Par ssh : le bus et le dossier de la session de l'utilisateur.
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=$XDG_RUNTIME_DIR/bus}"

stop() {
    systemctl --user stop kybernow-admin.service kybernow-kyber.service 2>/dev/null || true
}

build() {
    [ -f "$ROOT/server/web/dist/index.html" ] || { echo "Interface d'Admin absente : server/web, npm ci && npm run build" >&2; exit 1; }
    [ -x "$RT_SRC/bin/kycontroller" ] || { echo "Kyber absent : lancer d'abord server/linux/scripts/build-kyber.sh" >&2; exit 1; }
    echo "==> kyber-admin"
    (cd "$ROOT/server/admin" && touch src/assets.rs && CARGO_TARGET_DIR="$TARGET" cargo build --release --quiet)
    echo "==> Agent des écrans (kybernow-display)"
    (cd "$ROOT/server/linux/display-agent" && CARGO_TARGET_DIR="$TARGET" cargo build --release --quiet)
    mkdir -p "$BIN"
    cp "$TARGET/release/kyber-admin" "$TARGET/release/kybernow-display" "$BIN/"
    echo "==> Kyber"
    rm -rf "$RT" && mkdir -p "$RT"
    cp -a "$RT_SRC/bin" "$RT_SRC/lib" "$RT/"
}

# Première fois : la même initialisation que le paquet
# (packaging/kybernow-station-setup), dans le dossier d'essai.
first_setup() {
    [ -f "$DATA/kyber-admin.toml" ] && return
    mkdir -p "$BASE"
    cp "$KYBER_DIR_SRC/kyber_config.toml" "$BASE/kyber_config.toml.orig"
    STATION_DIR="$BASE" KYBER_TEMPLATE="$BASE/kyber_config.toml.orig" KYBERNOW_ADMIN="$BIN/kyber-admin" \
        "$ROOT/server/linux/packaging/kybernow-station-setup"
    [ -f "$DATA/mot-de-passe" ] && mv "$DATA/mot-de-passe" "$DATA/mot-de-passe-essai"
    return 0
}

units() {
    mkdir -p "$UNITS"
    local libs="$RT/lib:$RT/lib/$(gcc -dumpmachine 2>/dev/null | sed 's/-pc-/-/; s/-unknown-/-/'):$RT/lib64"
    cat > "$UNITS/kybernow-kyber.service" <<EOF
[Unit]
Description=KyberNow Station (Kyber)
PartOf=graphical-session.target
After=graphical-session.target pipewire.service

[Service]
Environment=KYBER_CONFIG=$KYBER/kyber_config.toml
Environment=PATH=$RT/bin:/usr/local/bin:/usr/bin:/bin
Environment=LD_LIBRARY_PATH=$libs
WorkingDirectory=$KYBER
ExecStart=$RT/bin/kycontroller
StandardOutput=append:$KYBER/log/kyber.log
StandardError=append:$KYBER/log/kyber.log
Restart=on-failure
RestartSec=2

[Install]
WantedBy=graphical-session.target
EOF
    cat > "$UNITS/kybernow-admin.service" <<EOF
[Unit]
Description=KyberNow Station (Admin)
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Environment=KYBERNOW_DISPLAY_AGENT=$BIN/kybernow-display
Environment=KYBERNOW_KYBER_UNIT=kybernow-kyber.service
WorkingDirectory=$DATA
ExecStart=$BIN/kyber-admin --data-dir $DATA --kyber-dir $KYBER
StandardOutput=append:$DATA/admin.log
StandardError=append:$DATA/admin.log
Restart=on-failure
RestartSec=2

[Install]
WantedBy=graphical-session.target
EOF
    # KDE Plasma : le droit de l'agent au protocole d'écrans de KWin, pour
    # cette copie de l'agent (comme le fichier que pose le paquet).
    local apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
    mkdir -p "$apps"
    sed "s|^Exec=.*|Exec=$BIN/kybernow-display|" "$ROOT/server/linux/packaging/fr.leslignesclaires.kybernow.station.display.desktop" \
        > "$apps/fr.leslignesclaires.kybernow.station.display.desktop"
    systemctl --user daemon-reload
}

# Entrées : /dev/uinput pour l'utilisateur de la session (règle udev
# `uaccess`, comme le paquet). Demande les droits d'administrateur une fois.
uinput() {
    [ -w /dev/uinput ] && return
    local rule=/etc/udev/rules.d/60-kybernow-uinput.rules
    if sudo -n true 2>/dev/null; then
        sudo install -m 644 "$ROOT/server/linux/packaging/60-kybernow-uinput.rules" "$rule"
        sudo modprobe uinput || true
        sudo udevadm control --reload && sudo udevadm trigger --name-match=uinput
    else
        echo "    /dev/uinput n'est pas accessible : sudo install -m 644 $ROOT/server/linux/packaging/60-kybernow-uinput.rules $rule && sudo udevadm trigger --name-match=uinput" >&2
    fi
}

case "${1:-}" in
    start)
        stop
        build
        first_setup
        units
        uinput
        systemctl --user start kybernow-kyber.service kybernow-admin.service
        ip=$(hostname -I 2>/dev/null | awk '{print $1}')
        echo "Station d'essai lancée : Admin sur https://${ip:-localhost}:9443, Kyber sur le port 8080"
        ;;
    stop) stop ;;
    status)
        systemctl --user --no-pager status kybernow-kyber.service kybernow-admin.service 2>/dev/null | grep -E "●|Active:" || true
        echo "--- Admin"; tail -15 "$DATA/admin.log" 2>/dev/null || true
        echo "--- Kyber"; tail -15 "$KYBER/log/kyber.log" 2>/dev/null || true
        ;;
    *) echo "usage : $0 start|stop|status" >&2; exit 2 ;;
esac
