#!/bin/bash
# Essais de kybernow-station-setup (choix de l'encodeur, STLIN-14), sur
# n'importe quelle machine : /sys, /proc et /dev simulés (KYBERNOW_SETUP_ROOT).
#   server/linux/scripts/test-setup.sh
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SETUP="$HERE/../packaging/kybernow-station-setup"
ROOTDIR="$(mktemp -d)"
trap 'rm -rf "$ROOTDIR"' EXIT
fail=0
# cas <fabricant> <nœud de rendu 0|1> <pilote NVIDIA 0|1> <attendu>
cas() {
    rm -rf "${ROOTDIR:?}"/*
    mkdir -p "$ROOTDIR/sys/class/drm/card0/device" "$ROOTDIR/dev/dri" "$ROOTDIR/proc/driver/nvidia"
    echo "$1" > "$ROOTDIR/sys/class/drm/card0/device/vendor"
    [ "$2" = 1 ] && touch "$ROOTDIR/dev/dri/renderD128"
    [ "$3" = 1 ] && touch "$ROOTDIR/proc/driver/nvidia/version"
    local got
    got=$(HOME="$ROOTDIR" KYBERNOW_SETUP_ROOT="$ROOTDIR" bash "$SETUP" --encoder)
    if [ "$got" = "$4" ]; then echo "ok   $1 rendu=$2 pilote=$3 → $got"; else echo "ÉCHEC $1 rendu=$2 pilote=$3 → $got (attendu $4)"; fail=1; fi
}
cas 0x10de 1 1 nvenc    # NVIDIA, pilote propriétaire
cas 0x10de 1 0 vaapi    # NVIDIA sous nouveau ou NVK : pas de NVENC
cas 0x10de 0 0 x264
cas 0x1002 1 0 vaapi    # AMD
cas 0x8086 1 0 vaapi    # Intel
cas 0x8086 0 0 x264
exit $fail
