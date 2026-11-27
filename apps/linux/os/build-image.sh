#!/bin/bash
# Construit l'image « KyberNow Link OS » pour Raspberry Pi (arm64), sur un Linux
# arm64 avec sudo (la VM « KyberNow Linux », ou un Pi) :
#
#   apps/linux/os/build-image.sh [--deb FICHIER] [--work DOSSIER] [--no-compress] [--ab-only]
#   apps/linux/os/build-image.sh --clean [--work DOSSIER]
#
#   --ab-only : sans relancer pi-gen, refait la disposition A/B depuis la
#               dernière image de base construite (mise au point de make-ab-image.sh).
#   --clean   : retire seulement ce qu'une construction laisse dans le dossier de
#               travail (voir « Place sur le disque »), puis s'arrête.
#
# Place sur le disque : pi-gen garde une copie du système par étape (stages 0,
# 1 et 2, notre stage, l'export de l'image) : une quinzaine de Go. Une fois
# l'image vérifiée, et au début de chaque construction (une précédente a pu
# échouer avant), tout est retiré sauf le système du stage 2, que la
# construction suivante reprend (environ 3 Go, LINAPP-38), et la dernière image
# de base (deploy/, pour --ab-only). Aussi pi-gen/work, l'ancien dossier de
# travail de pi-gen, que plus rien n'emploie. Un dossier où un système de
# fichiers est encore monté (pi-gen arrêté en cours de route) n'est jamais
# effacé : le script s'arrête et dit lequel démonter.
#
# 1. pi-gen (étiquette PIGEN_TAG, dans $WORK/pi-gen) construit Raspberry Pi OS Lite
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
WORK="${KYBERNOW_OS_WORK:-$HOME/kybernow-os}"
DEB="${KYBERNOW_OS_DEB:-$LINUX/dist/kybernow-link_$(sed -n 's/^version = "\(.*\)"/\1/p' "$APPS/Cargo.toml" | head -1)_arm64.deb}"
COMPRESS=1
AB_ONLY=0
CLEAN=0
while [ $# -gt 0 ]; do
    case "$1" in
        --deb) DEB="$2"; shift ;;
        --work) WORK="$2"; shift ;;
        --no-compress) COMPRESS=0 ;;
        --ab-only) AB_ONLY=1 ;;
        --clean) CLEAN=1 ;;
        *) echo "option inconnue : $1" >&2; exit 2 ;;
    esac
    shift
done
[ "$(uname -m)" = aarch64 ] || { echo "à lancer sur un Linux arm64 (VM ou Pi) : $(uname -m)" >&2; exit 1; }
sudo -n true 2>/dev/null || { echo "sudo sans mot de passe requis (pi-gen monte des systèmes de fichiers)" >&2; exit 1; }
PIGEN="$WORK/pi-gen"
PIGEN_WORK="$WORK/pi-gen-work"
mkdir -p "$WORK"
# Une seule construction (ou un seul nettoyage) à la fois dans ce dossier.
# shellcheck source=../../../scripts/verrou.sh
. "$APPS/../scripts/verrou.sh"
kyber_verrou_sortie "$WORK" "apps/linux/os/build-image.sh" || exit 1

# vider <dossier>… : retire ces dossiers (root y écrit : sudo -n), jamais en
# traversant un système de fichiers monté (rm --one-file-system) ; s'il en reste
# un monté dessous (pi-gen arrêté en cours de route : /proc, /dev du chroot),
# rien n'est retiré et le script s'arrête.
vider() {
    local d m
    for d in "$@"; do
        [ -e "$d" ] || continue
        m="$(findmnt -rn -o TARGET | awk -v d="$d" '$0 == d || index($0, d "/") == 1' | head -1)"
        if [ -n "$m" ]; then
            echo "$m est encore monté (pi-gen interrompu ?) : sudo umount -R \"$m\", puis relancer" >&2
            return 1
        fi
        sudo -n rm -rf --one-file-system "$d"
    done
}
# Ce qu'une construction laisse et que la suivante ne reprend pas : tout pi-gen-work
# sauf le système du stage 2 (et sa clé), et pi-gen/work.
vider_restes() {
    local d
    vider "$PIGEN/work" || return 1
    [ -d "$PIGEN_WORK" ] || return 0
    for d in "$PIGEN_WORK"/*; do
        case "$(basename "$d")" in stage2) ;; *) vider "$d" || return 1 ;; esac
    done
    [ ! -d "$PIGEN_WORK/stage2" ] || for d in "$PIGEN_WORK/stage2"/*; do
        [ "$(basename "$d")" = rootfs ] || vider "$d" || return 1
    done
}
if [ "$CLEAN" = 1 ]; then
    vider_restes
    echo "==> dossier de travail vidé ($(du -sh "$WORK" 2>/dev/null | cut -f1) gardés : système du stage 2, dernière image de base)"
    exit 0
fi
[ -f "$DEB" ] || { echo "paquet absent : $DEB (apps/linux/scripts/deb.sh)" >&2; exit 1; }
# commande:paquet (le paquet d'Ubuntu qui la donne).
for t in debootstrap:debootstrap quilt:quilt parted:parted zerofree:zerofree bsdtar:libarchive-tools pigz:pigz xz:xz-utils arch-test:arch-test; do
    command -v "${t%%:*}" >/dev/null || { echo "${t%%:*} absent (apt install ${t#*:})" >&2; exit 1; }
done
VERSION="$(dpkg-deb -f "$DEB" Version)"

# 1. pi-gen.
if [ "$AB_ONLY" = 1 ]; then
    BASE="$(ls -t "$WORK"/deploy/*KyberNow-Link-OS*.img 2>/dev/null | head -1)"
    [ -n "$BASE" ] || { echo "aucune image de base dans $WORK/deploy" >&2; exit 1; }
    echo "==> disposition A/B seule, depuis $BASE"
else
# pi-gen figé sur une publication de Raspberry Pi OS (trixie, arm64) : la même
# base d'une construction à l'autre ; changer PIGEN_TAG pour suivre une version
# plus récente (audit TRANS-21). Un clone d'avant, sur la branche, est remis à
# cette étiquette.
PIGEN_TAG="${KYBERNOW_PIGEN_TAG:-2026-10-06-raspios-trixie-arm64}"
if [ ! -d "$PIGEN/.git" ]; then
    echo "==> pi-gen ($PIGEN_TAG)"
    git clone -q --branch "$PIGEN_TAG" --depth 1 https://github.com/RPi-Distro/pi-gen.git "$PIGEN"
elif [ "$(git -C "$PIGEN" describe --tags --exact-match 2>/dev/null || true)" != "$PIGEN_TAG" ]; then
    echo "==> pi-gen remis à $PIGEN_TAG"
    git -C "$PIGEN" fetch -q --depth 1 origin "refs/tags/$PIGEN_TAG:refs/tags/$PIGEN_TAG"
    git -C "$PIGEN" checkout -q -f "$PIGEN_TAG"
fi
# Notre stage, copié à côté des autres (pi-gen veut les stages dans son dossier).
rm -rf "$PIGEN/stage-kybernow"
cp -R "$HERE/stage-kybernow" "$PIGEN/stage-kybernow"
cp "$DEB" "$PIGEN/stage-kybernow/01-link/files/kybernow-link.deb"
# La version de l'OS (celle du paquet), lue par 00-run.sh : pi-gen ne passe pas
# les variables de config inconnues aux stages.
echo "$VERSION" > "$PIGEN/stage-kybernow/01-link/files/version"
# La clé publique de publication (celle des apps et de la Station) : la mise à
# jour de l'OS vérifie le manifeste signé avec elle, côté root.
cp "$APPS/../server/admin/src/update/kybernow-release.pub" "$PIGEN/stage-kybernow/01-link/files/release.pub"
chmod +x "$PIGEN/stage-kybernow"/*/*.sh "$PIGEN/stage-kybernow/prerun.sh"
# Raspberry Pi OS Lite (stages 0 à 2 : debootstrap, paquets) ne change pas
# d'une version de KyberNow à l'autre : son rootfs est gardé dans PIGEN_WORK et
# repris tant que l'étiquette de pi-gen et la langue sont les mêmes (SKIP et
# CONTINUE de pi-gen) ; seul notre stage est refait, depuis une copie fraîche
# du rootfs du stage 2. KYBERNOW_OS_FRESH=1 repart de zéro (LINAPP-38).
BASE_KEY="$PIGEN_TAG ${KYBERNOW_OS_LOCALE:-fr_FR.UTF-8}"
rm -f "$PIGEN/stage0/SKIP" "$PIGEN/stage1/SKIP" "$PIGEN/stage2/SKIP"
PIGEN_CONTINUE=0
if [ "${KYBERNOW_OS_FRESH:-0}" != 1 ] && [ "$(cat "$PIGEN_WORK/.kybernow-base" 2>/dev/null || true)" = "$BASE_KEY" ] && [ -d "$PIGEN_WORK/stage2/rootfs" ]; then
    echo "==> Raspberry Pi OS Lite repris ($BASE_KEY) ; notre stage seul"
    touch "$PIGEN/stage0/SKIP" "$PIGEN/stage1/SKIP" "$PIGEN/stage2/SKIP"
    PIGEN_CONTINUE=1
    vider_restes || exit 1
else
    vider "$PIGEN/work" "$PIGEN_WORK" || exit 1
fi
# Les stages de bureau ne servent pas, et seul le nôtre exporte une image.
touch "$PIGEN/stage3/SKIP" "$PIGEN/stage4/SKIP" "$PIGEN/stage5/SKIP" "$PIGEN/stage2/SKIP_IMAGES" "$PIGEN/stage4/SKIP_IMAGES" "$PIGEN/stage5/SKIP_IMAGES"
cat > "$PIGEN/config" <<EOF
IMG_NAME=KyberNow-Link-OS
RELEASE=trixie
DEPLOY_DIR=$WORK/deploy
WORK_DIR=$PIGEN_WORK
CONTINUE=$PIGEN_CONTINUE
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
# Le journal est écrit par l'utilisateur, pas par root : voulu.
# shellcheck disable=SC2024
(cd "$PIGEN" && sudo -n ./build.sh > "$WORK/pi-gen.log" 2>&1) || { tail -30 "$WORK/pi-gen.log" >&2; echo "pi-gen en échec (journal : $WORK/pi-gen.log)" >&2; exit 1; }
BASE="$(ls -t "$WORK"/deploy/*KyberNow-Link-OS*.img 2>/dev/null | head -1)"
[ -n "$BASE" ] || { echo "aucune image dans $WORK/deploy" >&2; exit 1; }
echo "    image de base : $BASE ($(du -h "$BASE" | cut -f1))"
# Stages 0 à 2 réussis avec cette étiquette et cette langue : repris la prochaine fois.
echo "$BASE_KEY" | sudo -n tee "$PIGEN_WORK/.kybernow-base" >/dev/null
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
    # Vérifiée avant de vider le dossier de travail : relue en entier.
    xz -t "$OUT.xz" || { echo "$OUT.xz illisible : dossier de travail gardé" >&2; exit 1; }
    echo "$OUT.xz ($(du -h "$OUT.xz" | cut -f1))"
else
    parted -s "$OUT" print >/dev/null || { echo "$OUT illisible : dossier de travail gardé" >&2; exit 1; }
    echo "$OUT ($(du -h "$OUT" | cut -f1))"
fi

# Image écrite et vérifiée : les copies du système de cette construction sont
# retirées (une quinzaine de Go) ; le stage 2 reste, pour la prochaine.
if vider_restes; then
    echo "==> dossier de travail vidé ($(du -sh "$WORK" | cut -f1) gardés)"
else
    echo "Attention : dossier de travail pas vidé (voir plus haut) ; l'image est bonne" >&2
fi
