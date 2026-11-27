#!/bin/bash
# Empreinte de la surcouche Kyber de la Station Windows : tout ce qui entre dans
# third_party/payload/kyber-overlay. build.sh l'écrit à côté de la surcouche,
# scripts/release.sh la compare pour savoir si la surcouche de l'arbre est encore
# bonne : une seule liste, ici, pour que les deux ne divergent plus.
#
# Les correctifs (ceux du serveur, et ceux des apps qu'apply.sh repose après :
# 0027 s'appuie sur 0005), les scripts de construction, et les sources des aides
# compilées dans la surcouche (micro, caméra, essai de l'encodeur). Ces dernières
# manquaient : en 0.11.7, l'aide du micro et celle de la caméra corrigées pour ne
# plus ouvrir de fenêtre sont parties dans leur ancienne version (surcouche jugée
# à jour).
#
#   server/kyber-patches/stamp.sh     écrit l'empreinte sur la sortie
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
{
    cat server/kyber-patches/*.patch server/kyber-patches/build.sh server/kyber-patches/pyrowave.sh \
        apps/macos/kyber-patches/*.patch apps/webos/kyber-patches/*.patch
    # Fichiers suivis des aides, triés, avec leur nom (un fichier renommé change l'empreinte).
    git ls-files -z server/micro server/camera server/encoder-check | LC_ALL=C sort -z |
        while IFS= read -r -d '' f; do
            printf '%s\0' "$f"
            cat "$f"
        done
} | shasum -a 256 | cut -d' ' -f1
