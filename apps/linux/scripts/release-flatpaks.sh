#!/bin/bash
# Construit les deux Flatpak de KyberNow Link (x86_64 et aarch64) et le paquet
# .deb du Raspberry Pi (arm64, scripts/deb.sh) depuis le Mac, dans la VM
# Parallels « KyberNow Linux » (Ubuntu ARM64, Rosetta pour x86_64), qui voit ce
# dépôt en /media/psf/kyber-apps : les paquets sont ceux de l'arbre courant (le
# commit de version quand release.sh l'appelle).
#
#   apps/linux/scripts/release-flatpaks.sh [--check] [DOSSIER]
#
# → apps/linux/dist/KyberNow-Link-Linux-{x86_64,aarch64}.flatpak et
# apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb, copiés dans DOSSIER s'il
# est donné. --check : la VM répond et peut construire.
# Réglages : KYBERNOW_LINUX_VM (nom Parallels), KYBERNOW_LINUX_SSH (hôte ssh),
# KYBERNOW_LINUX_REPO (chemin du dépôt dans la VM).
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

# La VM : démarrée si elle dort, puis attendue au ssh (deux minutes au plus).
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
# Le dépôt partagé doit être celui-ci, et x86_64 passe par Rosetta (binfmt).
vm "test -f '$REPO/apps/linux/scripts/flatpak.sh'" || { echo "dépôt absent dans la VM : $REPO (dossier partagé de Parallels)" >&2; exit 1; }
vm "command -v flatpak-builder >/dev/null" || { echo "flatpak-builder absent de la VM" >&2; exit 1; }
vm "command -v dpkg-deb >/dev/null && command -v dpkg-shlibdeps >/dev/null && command -v fakeroot >/dev/null" || { echo "dpkg-dev ou fakeroot absents de la VM (paquet .deb du Raspberry Pi)" >&2; exit 1; }
if ! vm "test -e /proc/sys/fs/binfmt_misc/rosetta"; then
    # Un apt install relance systemd-binfmt, qui peut effacer Rosetta : le service la remet.
    vm "sudo -n systemctl restart kybernow-rosetta 2>/dev/null; test -e /proc/sys/fs/binfmt_misc/rosetta" \
        || { echo "Rosetta absente du binfmt de la VM (x86_64 impossible) : sudo systemctl restart kybernow-rosetta" >&2; exit 1; }
fi
[ "$CHECK" = 1 ] && { echo "VM « $VM » prête ($HOST, $REPO)"; exit 0; }

for arch in aarch64 x86_64; do
    echo "==> Flatpak $arch (VM « $VM »)"
    rm -f "$ROOT/apps/linux/dist/KyberNow-Link-Linux-$arch.flatpak"
    vm "cd ~ && '$REPO/apps/linux/scripts/flatpak.sh' $arch > ~/flatpak-$arch.log 2>&1" \
        || { echo "échec ; journal dans la VM : ~/flatpak-$arch.log" >&2; vm "tail -20 ~/flatpak-$arch.log" >&2 || true; exit 1; }
    f="$ROOT/apps/linux/dist/KyberNow-Link-Linux-$arch.flatpak"
    [ -s "$f" ] || { echo "$f absent après la construction" >&2; exit 1; }
    echo "    $(du -h "$f" | cut -f1) $f"
    [ -z "$DEST" ] || cp "$f" "$DEST/"
done

# Le paquet .deb du Raspberry Pi (arm64, FFmpeg de Raspberry Pi), dans la même VM.
echo "==> .deb Raspberry Pi arm64 (VM « $VM »)"
rm -f "$ROOT/apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb"
vm "cd ~ && '$REPO/apps/linux/scripts/deb.sh' > ~/deb.log 2>&1" \
    || { echo "échec ; journal dans la VM : ~/deb.log" >&2; vm "tail -20 ~/deb.log" >&2 || true; exit 1; }
f="$ROOT/apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb"
[ -s "$f" ] || { echo "$f absent après la construction" >&2; exit 1; }
echo "    $(du -h "$f" | cut -f1) $f"
[ -z "$DEST" ] || cp "$f" "$DEST/"
