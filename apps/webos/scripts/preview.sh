#!/bin/bash
# Captures des écrans de l'app webOS sur l'ordinateur de développement, avec
# des données de démonstration, sans téléviseur ni bibliothèques de Kyber
# (l'interface seule : --no-default-features).
#
#   apps/webos/scripts/preview.sh [dossier] [fr|en]    (dossier : apps/target/webos-preview)
#
# Les captures de apps/webos/screenshots en sont une sélection.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$HERE/../target/webos-preview}"
LANG_UI="${2:-fr}"
cd "$HERE/.."
cargo run -q -p kybernow-webos --no-default-features --release -- --preview "$OUT" "$LANG_UI"
