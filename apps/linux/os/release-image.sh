#!/bin/bash
# Construit l'image « KyberNow Link OS » depuis le Mac, dans la VM Parallels
# « KyberNow Linux » (arm64, sudo sans mot de passe, pi-gen et ses outils),
# qui voit ce dépôt en /media/psf/kyber-apps (réglages : apps/linux/scripts/vm.sh) ;
# le paquet .deb doit déjà être dans apps/linux/dist (release-flatpaks.sh le
# construit avant).
#
#   apps/linux/os/release-image.sh [--check] [DOSSIER]
#
# → apps/linux/dist/KyberNow-Link-OS.img.xz, copiée dans DOSSIER s'il est donné
# (et apps/linux/dist/KyberNow-Link-OS.img.xz.info : taille décompressée et
# empreintes, pour le dépôt JSON de Raspberry Pi Imager, imager-json.sh).
# Long : une bonne demi-heure (debootstrap, paquets, compression).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$ROOT/apps/linux/scripts/vm.sh"
REPO="$(vm_repo "$ROOT")"
CHECK=0 DEST=""
for a in "$@"; do
    case "$a" in
        --check) CHECK=1 ;;
        -*) echo "option inconnue : $a" >&2; exit 2 ;;
        *) DEST="$a" ;;
    esac
done
vm_demarrer
vm "test -f '$REPO/apps/linux/os/build-image.sh'" || { echo "dépôt absent dans la VM : $REPO" >&2; exit 1; }
vm "command -v debootstrap >/dev/null && command -v parted >/dev/null && command -v xz >/dev/null && sudo -n true" || { echo "pi-gen : debootstrap, parted, xz ou sudo sans mot de passe absents de la VM (apps/linux/os/README.md)" >&2; exit 1; }
# pi-gen remplit le disque de la VM (son dossier work/) : 20 Go libres au moins.
free_gb="$(vm "df -Pk ~ | awk 'NR==2 {print int(\$4/1048576)}'")"
[ "${free_gb:-0}" -ge 20 ] || { echo "VM : ${free_gb:-?} Go libres, 20 au moins pour pi-gen (vider ~/kybernow-os/pi-gen/work : sudo rm -rf ~/kybernow-os/pi-gen/work)" >&2; exit 1; }
[ "$CHECK" = 1 ] && { echo "VM « $VM_NAME » prête pour l'image ($VM_HOST, $REPO, $free_gb Go libres)"; exit 0; }

echo "==> image KyberNow Link OS (VM « $VM_NAME », pi-gen : long)"
rm -f "$ROOT/apps/linux/dist/KyberNow-Link-OS.img.xz" "$ROOT/apps/linux/dist/KyberNow-Link-OS.img.xz.info"
vm_lancer os-image.log "\"$REPO/apps/linux/os/build-image.sh\"" \
    || { echo "échec ; journal dans la VM : ~/os-image.log" >&2; exit 1; }
f="$ROOT/apps/linux/dist/KyberNow-Link-OS.img.xz"
[ -s "$f" ] || { echo "$f absente après la construction" >&2; exit 1; }
echo "    $(du -h "$f" | cut -f1) $f"
# Les mesures de l'image, pour Raspberry Pi Imager : taille et SHA-256 du fichier,
# SHA-256 de l'image décompressée (lue en flux, sans l'écrire) et sa taille, lue
# dans l'index de xz (pas de course avec un tee).
vm "cd '$REPO/apps/linux/dist' && { echo download_size=\$(stat -c %s KyberNow-Link-OS.img.xz); echo download_sha256=\$(sha256sum KyberNow-Link-OS.img.xz | cut -d' ' -f1); echo extract_sha256=\$(xz -dc KyberNow-Link-OS.img.xz | sha256sum | cut -d' ' -f1); echo extract_size=\$(xz --robot -l KyberNow-Link-OS.img.xz | awk '/^totals/{print \$5}'); } > KyberNow-Link-OS.img.xz.info"
cat "$f.info"
[ -z "$DEST" ] || cp "$f" "$f.info" "$DEST/"
