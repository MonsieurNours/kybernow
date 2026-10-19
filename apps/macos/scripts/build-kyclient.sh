#!/bin/bash
# Recompile libkyclient avec les correctifs KyberNow (kyber-patches/) et la
# remplace dans Vendor/ (lancer scripts/vendor-kyber.sh une première fois).
#
# 0001 : les paquets vidéo vont à l'app (décodage VideoToolbox, rendu Metal)
#        au lieu de VLC.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
KYSDK="$ROOT/third_party/kyber-desktop/kysdk"
KYCTL="$KYSDK/kyctl"
ROOTFS="$ROOT/third_party/kyber-desktop/rootfs-arm64-macos"
OUT="$HERE/Vendor"

[ -d "$OUT/Frameworks" ] || { echo "Lancez d'abord scripts/vendor-kyber.sh" >&2; exit 1; }

for patch in "$HERE"/kyber-patches/*.patch; do
    if git -C "$KYCTL" apply --check "$patch" 2>/dev/null; then
        git -C "$KYCTL" apply "$patch" && echo "==> $(basename "$patch") appliqué"
    elif git -C "$KYCTL" apply --reverse --check "$patch" 2>/dev/null; then
        echo "==> $(basename "$patch") déjà appliqué"
    else
        echo "Correctif $(basename "$patch") inapplicable" >&2; exit 1
    fi
done

echo "==> libkyclient"
export PATH="$ROOTFS/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export PKG_CONFIG_LIBDIR="$ROOTFS/lib/pkgconfig"
# Le « strip » de cargo laisse une table de symboles que l'éditeur de liens
# d'Xcode refuse (« mis-aligned LINKEDIT string pool »).
export CARGO_PROFILE_RELEASE_STRIP=false
(cd "$KYSDK/kyctl" && cargo cinstall -p kyclient --target aarch64-apple-darwin --release \
    --prefix="$ROOTFS" --features=kymux,rtp 2>&1 | grep -E "^(error|warning: unused)|Finished" || true)

cp "$ROOTFS/include/kyclient/kyclient.h" "$OUT/include/"
LIB="$OUT/Frameworks/libkyclient.0.1.0.dylib"
cp "$ROOTFS/lib/libkyclient.0.1.0.dylib" "$LIB"
install_name_tool -id "@rpath/libkyclient.0.1.dylib" "$LIB" 2>/dev/null
for dep in $(otool -L "$LIB" | awk 'NR>1 {print $1}' | grep -E "rootfs-arm64-macos|/kyber-desktop/"); do
    install_name_tool -change "$dep" "@rpath/$(basename "$dep")" "$LIB" 2>/dev/null
done
echo "libkyclient à jour dans $OUT"
