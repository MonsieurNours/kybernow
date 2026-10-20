#!/bin/sh
# Régénère les SVG puis la charte. Argument : dossier assets de @phosphor-icons/core.
set -e
here=$(dirname "$0")
node "$here/generate.mjs" "${PISTE:-spectre}"
node "$here/build-charte.mjs" "$1" "$2"
