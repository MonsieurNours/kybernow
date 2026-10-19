#!/bin/bash
# Phase de build Xcode : copie les bibliothèques Kyber et les plugins VLC
# dans KyberNow.app/Contents/Frameworks, puis les signe avec l'identité de l'app.
set -euo pipefail

SRC="$SRCROOT/Vendor/Frameworks"
DST="$TARGET_BUILD_DIR/$FRAMEWORKS_FOLDER_PATH"
mkdir -p "$DST"
rsync -a --delete-excluded --exclude 'KyberKit.framework' "$SRC/" "$DST/"

IDENTITY="${EXPANDED_CODE_SIGN_IDENTITY:--}"
[ -n "$IDENTITY" ] || IDENTITY="-"
find "$DST" -maxdepth 2 -name '*.dylib' -type f -print0 | xargs -0 -n 64 codesign --force --sign "$IDENTITY" --timestamp=none
