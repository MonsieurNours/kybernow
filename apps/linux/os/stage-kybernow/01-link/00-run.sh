#!/bin/bash -e
# Côté hôte : les fichiers de KyberNow Link OS dans le rootfs (le paquet,
# les services, Plymouth, le son, Bluetooth, sudoers, config.txt, cmdline.txt).
: "${ROOTFS_DIR:?}"
F="$(dirname "$0")/files"

install -m 644 "$F/kybernow-link.deb" "${ROOTFS_DIR}/tmp/kybernow-link.deb"

# Services.
install -m 644 "$F/systemd/kybernow-link.service" "${ROOTFS_DIR}/etc/systemd/system/kybernow-link.service"
install -m 644 "$F/systemd/kybernow-os-firstboot.service" "${ROOTFS_DIR}/etc/systemd/system/kybernow-os-firstboot.service"
install -m 755 "$F/kybernow-os-firstboot" "${ROOTFS_DIR}/usr/local/sbin/kybernow-os-firstboot"
install -m 755 "$F/kybernow-os-update" "${ROOTFS_DIR}/usr/local/sbin/kybernow-os-update"
install -m 755 "$F/kybernow-os-ctl" "${ROOTFS_DIR}/usr/local/sbin/kybernow-os-ctl"

# L'OS se reconnaît (l'app lit ce dossier : mode OS, version, disposition A/B).
install -d -m 755 "${ROOTFS_DIR}/etc/kybernow-os"
cat > "${ROOTFS_DIR}/etc/kybernow-os/release" <<EOF
NAME="KyberNow Link OS"
VERSION="$(cat "$F/version" 2>/dev/null || echo 0)"
BUILT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
BASE="Raspberry Pi OS Lite (${RELEASE:-trixie}, arm64)"
EOF

# Droits de l'app (mise à jour A/B, réseau, Bluetooth, SSH, redémarrage).
install -m 440 "$F/etc/sudoers-kybernow" "${ROOTFS_DIR}/etc/sudoers.d/kybernow"
# NetworkManager depuis l'app (pas de session interactive) : règle polkit.
install -d -m 755 "${ROOTFS_DIR}/etc/polkit-1/rules.d"
install -m 644 "$F/etc/polkit-kybernow.rules" "${ROOTFS_DIR}/etc/polkit-1/rules.d/50-kybernow.rules"
# Son : la sortie HDMI de la puce, par défaut.
install -m 644 "$F/etc/asound.conf" "${ROOTFS_DIR}/etc/asound.conf"
# Bluetooth : manettes acceptées sans code, rappariage libre.
install -d -m 755 "${ROOTFS_DIR}/etc/bluetooth/main.conf.d"
install -m 644 "$F/etc/bluetooth-kybernow.conf" "${ROOTFS_DIR}/etc/bluetooth/main.conf.d/kybernow.conf"
# Plymouth : l'image fixe du démarrage (le symbole éteint sur l'encre), celle de l'app.
install -d -m 755 "${ROOTFS_DIR}/usr/share/plymouth/themes/kybernow"
install -m 644 "$F/plymouth/kybernow.plymouth" "${ROOTFS_DIR}/usr/share/plymouth/themes/kybernow/kybernow.plymouth"
install -m 644 "$F/plymouth/kybernow.script" "${ROOTFS_DIR}/usr/share/plymouth/themes/kybernow/kybernow.script"

# config.txt : pilote KMS, pas d'arc-en-ciel, 4K à 60 Hz sur Pi 4, son HDMI ; cmdline.txt : silence et Plymouth.
CFG="${ROOTFS_DIR}/boot/firmware/config.txt"
grep -q "^disable_splash=1" "$CFG" || cat >> "$CFG" <<'EOF'

# KyberNow Link OS
disable_splash=1
disable_overscan=1
dtoverlay=vc4-kms-v3d
dtparam=audio=off
[pi4]
hdmi_enable_4kp60=1
[all]
EOF
CMD="${ROOTFS_DIR}/boot/firmware/cmdline.txt"
if ! grep -q "plymouth" "$CMD"; then
    sed -i -E 's/[[:space:]]*$//' "$CMD"
    sed -i -E 's/$/ quiet splash plymouth.ignore-serial-consoles logo.nologo vt.global_cursor_default=0 consoleblank=0 loglevel=3/' "$CMD"
    # Le mode texte ne sert plus.
    sed -i -E 's/ console=tty1//' "$CMD"
fi
