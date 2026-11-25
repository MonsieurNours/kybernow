#!/bin/bash
# Construit les deux Flatpak de KyberNow Link (x86_64 et aarch64) et le paquet
# .deb du Raspberry Pi (arm64, scripts/deb.sh) depuis le Mac, dans la VM
# Parallels « KyberNow Linux » (Ubuntu ARM64, Rosetta pour x86_64), qui voit ce
# dépôt en /media/psf/kyber-apps (et ses worktrees et arbres de construction
# dessous) : les paquets sont ceux de l'arbre courant (le commit de version
# quand release.sh l'appelle, depuis son arbre de construction « linux »).
#
#   apps/linux/scripts/release-flatpaks.sh [--check] [--seulement aarch64,x86_64,deb] [DOSSIER]
#
# → apps/linux/dist/KyberNow-Link-Linux-{x86_64,aarch64}.flatpak et
# apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb (avec
# kybernow-link_<version>_arm64.deb), copiés dans DOSSIER s'il est donné.
# --seulement : une partie (release.sh en fait des étapes à part).
# --check : la VM répond (démarrée sans fenêtre au besoin) et peut construire.
# Réglages : apps/linux/scripts/vm.sh ; KYBERNOW_FLATPAK_WORK (dossier de
# travail de flatpak-builder dans la VM) et KYBERNOW_KYBER_HEAD (sources de
# Kyber prises dans leurs dépôts git, sources.sh) sont transmis à la VM.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$ROOT/apps/linux/scripts/vm.sh"
REPO="$(vm_repo "$ROOT")"
CHECK=0 DEST="" PARTS="aarch64,x86_64,deb"
while [ $# -gt 0 ]; do
    case "$1" in
        --check) CHECK=1 ;;
        --seulement) PARTS="${2:?--seulement aarch64,x86_64,deb}"; shift ;;
        -*) echo "option inconnue : $1" >&2; exit 2 ;;
        *) DEST="$1" ;;
    esac
    shift
done

vm_demarrer
# Le dépôt partagé doit être celui-ci, et x86_64 passe par Rosetta (binfmt).
vm "test -f '$REPO/apps/linux/scripts/flatpak.sh'" || { echo "dépôt absent dans la VM : $REPO (dossier partagé de Parallels)" >&2; exit 1; }
vm "command -v flatpak-builder >/dev/null" || { echo "flatpak-builder absent de la VM" >&2; exit 1; }
vm "command -v dpkg-deb >/dev/null && command -v dpkg-shlibdeps >/dev/null && command -v fakeroot >/dev/null" || { echo "dpkg-dev ou fakeroot absents de la VM (paquet .deb du Raspberry Pi)" >&2; exit 1; }
vm_rosetta
[ "$CHECK" = 1 ] && { echo "VM « $VM_NAME » prête ($VM_HOST, $REPO, vue au démarrage : $(vm_vue))"; exit 0; }

ENV=""
[ -n "${KYBERNOW_FLATPAK_WORK:-}" ] && ENV+="KYBERNOW_FLATPAK_WORK=$KYBERNOW_FLATPAK_WORK "
[ -n "${KYBERNOW_KYBER_HEAD:-}" ] && ENV+="KYBERNOW_KYBER_HEAD=$KYBERNOW_KYBER_HEAD "

for arch in aarch64 x86_64; do
    [[ ",$PARTS," == *",$arch,"* ]] || continue
    echo "==> Flatpak $arch (VM « $VM_NAME »)"
    rm -f "$ROOT/apps/linux/dist/KyberNow-Link-Linux-$arch.flatpak"
    vm_lancer "flatpak-$arch.log" "$ENV\"$REPO/apps/linux/scripts/flatpak.sh\" $arch" \
        || { echo "échec ; journal dans la VM : ~/flatpak-$arch.log" >&2; exit 1; }
    f="$ROOT/apps/linux/dist/KyberNow-Link-Linux-$arch.flatpak"
    [ -s "$f" ] || { echo "$f absent après la construction" >&2; exit 1; }
    echo "    $(du -h "$f" | cut -f1) $f"
    [ -z "$DEST" ] || cp "$f" "$DEST/"
done

# Le paquet .deb du Raspberry Pi (arm64, FFmpeg de Raspberry Pi), dans la même VM.
if [[ ",$PARTS," == *",deb,"* ]]; then
    echo "==> .deb Raspberry Pi arm64 (VM « $VM_NAME »)"
    rm -f "$ROOT/apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb"
    vm_lancer deb.log "\"$REPO/apps/linux/scripts/deb.sh\"" \
        || { echo "échec ; journal dans la VM : ~/deb.log" >&2; exit 1; }
    f="$ROOT/apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb"
    [ -s "$f" ] || { echo "$f absent après la construction" >&2; exit 1; }
    echo "    $(du -h "$f" | cut -f1) $f"
    [ -z "$DEST" ] || cp "$f" "$DEST/"
fi
