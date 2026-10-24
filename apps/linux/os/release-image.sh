#!/bin/bash
# Construit l'image « KyberNow Link OS » depuis le Mac, dans la VM Parallels
# « KyberNow Linux » (arm64, sudo sans mot de passe, pi-gen et ses outils),
# qui voit ce dépôt en /media/psf/kyber-apps ; le paquet .deb doit déjà être
# dans apps/linux/dist (release-flatpaks.sh le construit avant).
#
#   apps/linux/os/release-image.sh [--check] [DOSSIER]
#
# → apps/linux/dist/KyberNow-Link-OS.img.xz, copiée dans DOSSIER s'il est donné
# (et apps/linux/dist/KyberNow-Link-OS.img.xz.info : taille décompressée et
# empreintes, pour le dépôt JSON de Raspberry Pi Imager, imager-json.sh).
# Long : une bonne demi-heure (debootstrap, paquets, compression).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
VM="${KYBERNOW_LINUX_VM:-KyberNow Linux}"
HOST="${KYBERNOW_LINUX_SSH:-kybernow-vm}"
REPO="${KYBERNOW_LINUX_REPO:-/media/psf/kyber-apps}"
CHECK=0 DEST=""
for a in "$@"; do
    case "$a" in
        --check) CHECK=1 ;;
        -*) echo "option inconnue : $a" >&2; exit 2 ;;
        *) DEST="$a" ;;
    esac
done
vm() { ssh -o BatchMode=yes -o ConnectTimeout=5 "$HOST" "$@"; }
if ! vm true 2>/dev/null; then
    command -v prlctl >/dev/null || { echo "VM « $VM » injoignable par ssh ($HOST), et prlctl absent" >&2; exit 1; }
    state="$(prlctl status "$VM" 2>/dev/null | awk '{print $NF}')"
    case "$state" in
        running) ;;
        suspended | paused) prlctl resume "$VM" >/dev/null ;;
        stopped) prlctl start "$VM" >/dev/null ;;
        *) echo "VM « $VM » introuvable (prlctl status : ${state:-rien})" >&2; exit 1 ;;
    esac
    for _ in $(seq 60); do vm true 2>/dev/null && break; sleep 2; done
    vm true 2>/dev/null || { echo "VM « $VM » démarrée mais injoignable par ssh ($HOST)" >&2; exit 1; }
fi
vm "test -f '$REPO/apps/linux/os/build-image.sh'" || { echo "dépôt absent dans la VM : $REPO" >&2; exit 1; }
vm "command -v debootstrap >/dev/null && command -v parted >/dev/null && command -v xz >/dev/null && sudo -n true" || { echo "pi-gen : debootstrap, parted, xz ou sudo sans mot de passe absents de la VM (apps/linux/os/README.md)" >&2; exit 1; }
[ "$CHECK" = 1 ] && { echo "VM « $VM » prête pour l'image ($HOST, $REPO)"; exit 0; }

echo "==> image KyberNow Link OS (VM « $VM », pi-gen : long)"
rm -f "$ROOT/apps/linux/dist/KyberNow-Link-OS.img.xz" "$ROOT/apps/linux/dist/KyberNow-Link-OS.img.xz.info"
vm "cd ~ && '$REPO/apps/linux/os/build-image.sh' > ~/os-image.log 2>&1" \
    || { echo "échec ; journal dans la VM : ~/os-image.log" >&2; vm "tail -30 ~/os-image.log" >&2 || true; exit 1; }
f="$ROOT/apps/linux/dist/KyberNow-Link-OS.img.xz"
[ -s "$f" ] || { echo "$f absente après la construction" >&2; exit 1; }
echo "    $(du -h "$f" | cut -f1) $f"
# Les mesures de l'image, pour Raspberry Pi Imager : taille et SHA-256 du fichier,
# SHA-256 de l'image décompressée (lue en flux, sans l'écrire) et sa taille, lue
# dans l'index de xz (pas de course avec un tee).
vm "cd '$REPO/apps/linux/dist' && { echo download_size=\$(stat -c %s KyberNow-Link-OS.img.xz); echo download_sha256=\$(sha256sum KyberNow-Link-OS.img.xz | cut -d' ' -f1); echo extract_sha256=\$(xz -dc KyberNow-Link-OS.img.xz | sha256sum | cut -d' ' -f1); echo extract_size=\$(xz --robot -l KyberNow-Link-OS.img.xz | awk '/^totals/{print \$5}'); } > KyberNow-Link-OS.img.xz.info"
cat "$f.info"
[ -z "$DEST" ] || cp "$f" "$f.info" "$DEST/"
