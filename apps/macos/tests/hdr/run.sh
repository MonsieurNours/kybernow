#!/bin/bash
# Essai croisé des courbes du HDR (docs/hdr.md) : les fonctions Metal de
# KyberKit (HDRShader.swift : PQ, primaires BT.2020 vers BT.709, courbe vers
# le SDR de la Station), exécutées sur le GPU sur une grille, contre celles du
# cœur (`rules::hdr`, par libkybernow de Vendor/, faite par
# apps/apple/scripts/build-apple.sh) : mêmes valeurs à 1e-4.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="$HERE/../../KyberKit"
VENDOR="$HERE/../../Vendor"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
if ! log="$(swiftc -O -o "$WORK/hdr" "$HERE/main.swift" "$KIT/HDRShader.swift" -framework Metal \
    -I "$VENDOR/KyberNow/include" -L "$VENDOR/Frameworks" -lkybernow -Xlinker -rpath -Xlinker "$VENDOR/Frameworks" 2>&1)"; then
    echo "$log" | grep -E "error" >&2 || echo "$log" >&2
    echo "Compilation de l'essai ratée." >&2
    exit 1
fi
"$WORK/hdr"
