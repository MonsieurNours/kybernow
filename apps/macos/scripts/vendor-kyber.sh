#!/bin/bash
# Récupère le SDK client de Kyber 0.29 compilé pour macOS (voir docs/build-notes.md) :
# en-têtes C, bibliothèques (libkyclient, libkynput, VLC…) et plugins VLC.
#
# Source : third_party/kyber-desktop (build-macos.sh -p, qui corrige déjà les
# chemins en @rpath et range les plugins VLC dans Frameworks/plugins).
# Sortie : apps/macos/Vendor/ (ignoré par git).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
KYBER="$ROOT/third_party/kyber-desktop"
APP="$KYBER/Kyber-Client.app/Contents/Frameworks"
OUT="$HERE/Vendor"

[ -d "$APP" ] || { echo "Client Kyber introuvable : lancez build-macos.sh -p dans $KYBER" >&2; exit 1; }

rm -rf "$OUT" && mkdir -p "$OUT/include" "$OUT/Frameworks"
cp "$KYBER/rootfs-arm64-macos/include/kyclient/kyclient.h" "$KYBER/rootfs-arm64-macos/include/kynput/kynput.h" "$OUT/include/"
cat > "$OUT/include/module.modulemap" <<'EOF'
module CKyber {
    header "kyclient.h"
    header "kynput.h"
    link "kyclient.0.1"
    link "kynput.0.1"
    export *
}
EOF
rsync -a "$APP/" "$OUT/Frameworks/"
# Identifiants en @rpath partout (le paquet Kyber en laisse un absolu).
for lib in "$OUT"/Frameworks/*.dylib; do
    [ -L "$lib" ] && continue
    id=$(otool -D "$lib" | tail -1)
    case "$id" in @rpath/*) ;; *) install_name_tool -id "@rpath/$(basename "$id")" "$lib" 2>/dev/null ;; esac
    for dep in $(otool -L "$lib" | awk 'NR>1 {print $1}' | grep -E "rootfs-arm64-macos|/kyber-desktop/"); do
        install_name_tool -change "$dep" "@rpath/$(basename "$dep")" "$lib" 2>/dev/null
    done
done
echo "SDK Kyber copié dans $OUT ($(du -sh "$OUT/Frameworks" | cut -f1))"
