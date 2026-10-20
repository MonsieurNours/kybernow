#!/bin/bash
# Phase de build Xcode : copie les bibliothèques Kyber et les plugins VLC
# dans KyberNow.app/Contents/Frameworks, signés avec l'identité de l'app.
#
# Les fichiers inchangés ne sont jamais réécrits dans l'app : on signe une
# copie de travail (seulement ce qui a changé dans Vendor/), puis on ne
# recopie dans l'app que ce qui diffère. Réécrire une bibliothèque pendant
# que l'app tourne fait charger à VLC une seconde copie de son cœur au
# démarrage d'une session, et il plante (config_GetInt sur NULL).
set -euo pipefail

SRC="$SRCROOT/Vendor/Frameworks"
STAGE="$DERIVED_FILE_DIR/kyber-frameworks"
DST="$TARGET_BUILD_DIR/$FRAMEWORKS_FOLDER_PATH"
STAMP="$STAGE/.signed"
mkdir -p "$STAGE" "$DST"

IDENTITY="${EXPANDED_CODE_SIGN_IDENTITY:--}"
[ -n "$IDENTITY" ] || IDENTITY="-"
# Version diffusée (Developer ID) : runtime renforcé et horodatage sécurisé,
# exigés par la notarisation. Développement : signature locale, plus rapide.
if [ "${ENABLE_HARDENED_RUNTIME:-NO}" = YES ]; then
    SIGN_FLAGS=(--options runtime --timestamp)
    MODE="$IDENTITY runtime"
else
    SIGN_FLAGS=(--timestamp=none)
    MODE="$IDENTITY"
fi

# 1. Copie de travail : seuls les fichiers plus récents dans Vendor/ sont
#    recopiés (--update ; une copie signée est datée d'une seconde après sa
#    source).
CHANGED=$(rsync -a --update --delete --exclude '.signed' --out-format='%n' "$SRC/" "$STAGE/" | grep '\.dylib$' || true)
if [ ! -f "$STAMP" ] || [ "$(cat "$STAMP")" != "$MODE" ]; then
    CHANGED=$(cd "$STAGE" && find . -maxdepth 2 -name '*.dylib' -type f | sed 's|^\./||')
fi
if [ -n "$CHANGED" ]; then
    echo "Signature de $(echo "$CHANGED" | wc -l | tr -d ' ') bibliothèque(s)"
    (cd "$STAGE" && echo "$CHANGED" | while read -r f; do
        [ -f "$f" ] && [ ! -L "$f" ] || continue
        ref=$(stat -f %m "$SRC/$f")
        codesign --force --sign "$IDENTITY" "${SIGN_FLAGS[@]}" "$f"
        # Une seconde après la source : le prochain rsync --update la garde.
        touch -t "$(date -r $((ref + 1)) +%Y%m%d%H%M.%S)" "$f"
    done)
    echo "$MODE" > "$STAMP"
fi

# 2. Dans l'app : seulement ce qui diffère (taille ou heure), en place
#    jamais pour un fichier identique.
# Les frameworks intégrés par Xcode (KyberKit, Sparkle) ne viennent pas de Vendor/ : on n'y touche pas.
rsync -a --delete --exclude '*.framework' --exclude '.signed' "$STAGE/" "$DST/"
