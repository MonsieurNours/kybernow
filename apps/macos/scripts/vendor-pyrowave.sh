#!/bin/bash
# Reprend le port Metal de PyroWave (https://github.com/Themaister/pyrowave,
# dossier metal/, licence MIT) dans PyroWave/upstream, au commit épinglé de
# PyroWave/COMMIT, ou à celui donné en argument (qui devient le nouveau
# commit épinglé). Les sources sont compilées dans KyberKit (project.yml) :
# aucune bibliothèque du Vendor n'est réécrite, l'app peut rester ouverte.
#
#   scripts/vendor-pyrowave.sh            # même commit (vérifie la copie)
#   scripts/vendor-pyrowave.sh <commit>   # change de commit
#
# Le port n'est presque plus maintenu en amont : changer de commit, c'est
# relire le diff de metal/ et repasser tests/pyrowave/run.sh.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
COMMIT="${1:-$(cat "$HERE/PyroWave/COMMIT")}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
git -C "$WORK" init -q
git -C "$WORK" remote add origin https://github.com/Themaister/pyrowave
git -C "$WORK" fetch -q --depth 1 origin "$COMMIT"
git -C "$WORK" checkout -q FETCH_HEAD
DEST="$HERE/PyroWave/upstream"
rm -rf "$DEST"
mkdir -p "$DEST"
cp "$WORK"/metal/*.mm "$WORK"/metal/*.cpp "$WORK"/metal/*.hpp "$WORK"/metal/*.h \
   "$WORK"/metal/README.md "$WORK"/metal/CMakeLists.txt "$WORK"/metal/pyrowave.exports "$DEST/"
cp -R "$WORK/metal/shaders" "$DEST/"
cp "$WORK/LICENSE" "$DEST/LICENSE"
git -C "$WORK" rev-parse HEAD > "$HERE/PyroWave/COMMIT"
echo "PyroWave (port Metal) $(cat "$HERE/PyroWave/COMMIT") → $DEST"
