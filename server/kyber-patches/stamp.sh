#!/bin/bash
# Empreinte de la surcouche Kyber de la Station Windows : tout ce qui entre dans
# third_party/payload/kyber-overlay. build.sh l'écrit à côté de la surcouche,
# scripts/release.sh la compare pour savoir si la surcouche de l'arbre est encore
# bonne. La liste est celle de kyber_station_inputs (apply.sh), commune aux
# trois Stations : correctifs, scripts qui les posent, chaîne d'outils, commit
# de PyroWave, et les sources des aides compilées dans la surcouche (micro,
# caméra, essai de l'encodeur).
#
#   server/kyber-patches/stamp.sh     écrit l'empreinte sur la sortie
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=apply.sh
. "$ROOT/server/kyber-patches/apply.sh"
kyber_station_hash "$ROOT" windows
