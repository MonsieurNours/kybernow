#!/bin/bash
# Construit l'image « KyberNow Link OS » pour Raspberry Pi (arm64), sur un Linux
# arm64 avec sudo (la VM « KyberNow Linux », ou un Pi) :
#
#   apps/linux/os/build-image.sh [--deb FICHIER] [--work DOSSIER] [--no-compress] [--ab-only]
#
#   --ab-only : sans relancer pi-gen, refait la disposition A/B depuis la
#               dernière image de base construite (mise au point de make-ab-image.sh).
#
# 1. pi-gen (branche arm64, dans $WORK/pi-gen) construit Raspberry Pi OS Lite
#    (stage0 à stage2) puis notre stage, `stage-kybernow` : le paquet .deb de
#    KyberNow Link (apps/linux/scripts/deb.sh), les services (Link en plein
#    écran sur KMS/DRM, premier démarrage, mise à jour), Plymouth avec le symbole
#    éteint, Bluetooth, réseau, son, CEC, sudoers ; pas de bureau.
# 2. make-ab-image.sh refait l'image en deux emplacements A/B (tryboot du
#    firmware) et une partition de données.
#
# → apps/linux/dist/KyberNow-Link-OS.img.xz (KyberNow-Link-OS.img avec --no-compress).
#
# Variables : KYBERNOW_OS_WORK (dossier de travail, hors dossier partagé :
# pi-gen monte des systèmes de fichiers), KYBERNOW_OS_DEB (le .deb),
# KYBERNOW_OS_LOCALE (fr_FR.UTF-8 par défaut : le premier démarrage choisit la langue).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
LINUX="$(cd "$HERE/.." && pwd)"
APPS="$(cd "$LINUX/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
WORK="${KYBERNOW_OS_WORK:-$HOME/kybernow-os}"
DEB="${KYBERNOW_OS_DEB:-$LINUX/dist/kybernow-link_$(sed -n 's/^version = "\(.*\)"/\1/p' "$APPS/Cargo.toml" | head -1)_arm64.deb}"
COMPRESS=1
AB_ONLY=0
while [ $# -gt 0 ]; do
    case "$1" in
        --deb) DEB="$2"; shift ;;
        --work) WORK="$2"; shift ;;
        --no-compress) COMPRESS=0 ;;
        --ab-only) AB_ONLY=1 ;;
        *) echo "option inconnue : $1" >&2; exit 2 ;;
    esac
    shift
done
[ "$(uname -m)" = aarch64 ] || { echo "à lancer sur un Linux arm64 (VM ou Pi) : $(uname -m)" >&2; exit 1; }
[ -f "$DEB" ] || { echo "paquet absent : $DEB (apps/linux/scripts/deb.sh)" >&2; exit 1; }
sudo -n true 2>/dev/null || { echo "sudo sans mot de passe requis (pi-gen monte des systèmes de fichiers)" >&2; exit 1; }
for t in debootstrap quilt parted zerofree bsdtar pigz xz arch-test; do command -v "$t" >/dev/null || { echo "$t absent (apt install $(sed -n 's/^.*:\(.*\)$/\1/p' <<< "$t" 2>/dev/null || echo "$t"))" >&2; exit 1; }; done
VERSION="$(dpkg-deb -f "$DEB" Version)"
mkdir -p "$WORK"

# 1. pi-gen.
PIGEN="$WORK/pi-gen"
if [ "$AB_ONLY" = 1 ]; then
    BASE="$(ls -t "$WORK"/deploy/*KyberNow-Link-OS*.img 2>/dev/null | head -1)"
    [ -n "$BASE" ] || { echo "aucune image de base dans $WORK/deploy" >&2; exit 1; }
    echo "==> disposition A/B seule, depuis $BASE"
else
if [ ! -d "$PIGEN/.git" ]; then
    echo "==> pi-gen (arm64)"
    git clone -q --branch arm64 --depth 1 https://github.com/RPi-Distro/pi-gen.git "$PIGEN"
fi
# Notre stage, copié à côté des autres (pi-gen veut les stages dans son dossier).
rm -rf "$PIGEN/stage-kybernow"
cp -R "$HERE/stage-kybernow" "$PIGEN/stage-kybernow"
cp "$DEB" "$PIGEN/stage-kybernow/01-link/files/kybernow-link.deb"
# La version de l'OS (celle du paquet), lue par 00-run.sh : pi-gen ne passe pas
# les variables de config inconnues aux stages.
echo "$VERSION" > "$PIGEN/stage-kybernow/01-link/files/version"
chmod +x "$PIGEN/stage-kybernow"/*/*.sh "$PIGEN/stage-kybernow/prerun.sh"
# Les stages de bureau ne servent pas, et seul le nôtre exporte une image.
touch "$PIGEN/stage3/SKIP" "$PIGEN/stage4/SKIP" "$PIGEN/stage5/SKIP" "$PIGEN/stage2/SKIP_IMAGES" "$PIGEN/stage4/SKIP_IMAGES" "$PIGEN/stage5/SKIP_IMAGES"
cat > "$PIGEN/config" <<EOF
IMG_NAME=KyberNow-Link-OS
RELEASE=trixie
DEPLOY_DIR=$WORK/deploy
DEPLOY_COMPRESSION=none
TARGET_HOSTNAME=kybernow-link
FIRST_USER_NAME=kybernow
FIRST_USER_PASS=$(head -c 24 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24)
DISABLE_FIRST_BOOT_USER_RENAME=1
ENABLE_SSH=0
ENABLE_CLOUD_INIT=0
LOCALE_DEFAULT=${KYBERNOW_OS_LOCALE:-fr_FR.UTF-8}
KEYBOARD_KEYMAP=fr
KEYBOARD_LAYOUT="French"
TIMEZONE_DEFAULT=Europe/Paris
WPA_COUNTRY=FR
STAGE_LIST="stage0 stage1 stage2 stage-kybernow"
EOF
# Le mot de passe du compte : pi-gen en exige un pour ne pas renommer le compte
# au premier démarrage ; tiré au hasard et jamais écrit ailleurs, puis le compte
# est verrouillé dans le chroot (pas de session interactive ; SSH s'active dans
# Réglages de l'appareil › Avancé, par clé seulement).
echo "==> pi-gen : Raspberry Pi OS Lite + KyberNow Link $VERSION (long : debootstrap, paquets)"
(cd "$PIGEN" && sudo -n ./build.sh > "$WORK/pi-gen.log" 2>&1) || { tail -30 "$WORK/pi-gen.log" >&2; echo "pi-gen en échec (journal : $WORK/pi-gen.log)" >&2; exit 1; }
BASE="$(ls -t "$WORK"/deploy/*KyberNow-Link-OS*.img 2>/dev/null | head -1)"
[ -n "$BASE" ] || { echo "aucune image dans $WORK/deploy" >&2; exit 1; }
echo "    image de base : $BASE ($(du -h "$BASE" | cut -f1))"
fi

# 2. A/B.
OUT="$LINUX/dist/KyberNow-Link-OS.img"
mkdir -p "$LINUX/dist"
sudo -n "$HERE/make-ab-image.sh" "$BASE" "$OUT"
sudo -n chown "$(id -u):$(id -g)" "$OUT"
if [ "$COMPRESS" = 1 ]; then
    echo "==> xz"
    rm -f "$OUT.xz"
    xz -T0 -6 --no-warn "$OUT"
    echo "$OUT.xz ($(du -h "$OUT.xz" | cut -f1))"
else
    echo "$OUT ($(du -h "$OUT" | cut -f1))"
fi
