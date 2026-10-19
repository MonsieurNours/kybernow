#!/bin/bash
# Recompile les bibliothèques client de Kyber avec les correctifs KyberNow
# (kyber-patches/) et les remplace dans Vendor/ (lancer
# scripts/vendor-kyber.sh une première fois). Détail : docs/kyber-changes.md.
#
# 0001-kyclient : les paquets audio et vidéo vont à l'app au lieu de VLC.
# 0002-kynput   : paquets de presse-papiers lisibles et fabriqués depuis l'app.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
KYSDK="$ROOT/third_party/kyber-desktop/kysdk"
ROOTFS="$ROOT/third_party/kyber-desktop/rootfs-arm64-macos"
OUT="$HERE/Vendor"

[ -d "$OUT/Frameworks" ] || { echo "Lancez d'abord scripts/vendor-kyber.sh" >&2; exit 1; }

# Chaque correctif vise le dépôt nommé après son numéro.
for patch in "$HERE"/kyber-patches/*.patch; do
    case "$(basename "$patch")" in
        *-kyclient-*) repo="$KYSDK/kyctl" ;;
        *-kynput-*) repo="$KYSDK/kynput" ;;
        *) echo "Dépôt inconnu pour $(basename "$patch")" >&2; exit 1 ;;
    esac
    if git -C "$repo" apply --check "$patch" 2>/dev/null; then
        git -C "$repo" apply "$patch" && echo "==> $(basename "$patch") appliqué"
    elif git -C "$repo" apply --reverse --check "$patch" 2>/dev/null; then
        echo "==> $(basename "$patch") déjà appliqué"
    else
        echo "Correctif $(basename "$patch") inapplicable" >&2; exit 1
    fi
done

export PATH="$ROOTFS/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export PKG_CONFIG_LIBDIR="$ROOTFS/lib/pkgconfig"
# Le « strip » de cargo laisse une table de symboles que l'éditeur de liens
# d'Xcode refuse (« mis-aligned LINKEDIT string pool »).
export CARGO_PROFILE_RELEASE_STRIP=false

echo "==> libkynput"
(cd "$KYSDK/kynput" && cargo cinstall -p kynput --target aarch64-apple-darwin --release \
    --prefix="$ROOTFS" --features sdl2 2>&1 | grep -E "^error|Finished" || true)
echo "==> libkyclient"
(cd "$KYSDK/kyctl" && cargo cinstall -p kyclient --target aarch64-apple-darwin --release \
    --prefix="$ROOTFS" --features=kymux,rtp 2>&1 | grep -E "^error|Finished" || true)

cp "$ROOTFS/include/kyclient/kyclient.h" "$ROOTFS/include/kynput/kynput.h" "$OUT/include/"
for name in kynput kyclient; do
    LIB="$OUT/Frameworks/lib$name.0.1.0.dylib"
    cp "$ROOTFS/lib/lib$name.0.1.0.dylib" "$LIB"
    install_name_tool -id "@rpath/lib$name.0.1.dylib" "$LIB" 2>/dev/null
    for dep in $(otool -L "$LIB" | awk 'NR>1 {print $1}' | grep -E "rootfs-arm64-macos|/kyber-desktop/"); do
        install_name_tool -change "$dep" "@rpath/$(basename "$dep")" "$LIB" 2>/dev/null
    done
done
echo "libkynput et libkyclient à jour dans $OUT"
