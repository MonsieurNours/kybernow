#!/bin/bash
# Recompile les bibliothèques client de Kyber avec les correctifs KyberNow
# (kyber-patches/) et les remplace dans Vendor/ (lancer
# scripts/vendor-kyber.sh une première fois). Détail : docs/kyber-changes.md.
#
# 0001-kyclient : les paquets audio et vidéo vont à l'app au lieu de VLC.
# 0002-kynput   : paquets de presse-papiers lisibles et fabriqués depuis l'app.
# 0003-kynet    : grande mémoire de réception UDP (rafales d'une image 4K).
#
# S'arrête à la première erreur (correctif inapplicable, compilation ratée)
# sans toucher à Vendor/. Refuse de tourner pendant que l'app est ouverte.
# Versions vérifiées : scripts/kyber-toolchain.sh (une autre est signalée).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# Chemin réel de third_party/ (un lien dans un worktree) : meson et cargo
# gardent ainsi leurs dossiers de build, liés au chemin absolu des sources.
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYSDK="$THIRD_PARTY/kyber-desktop/kysdk"
ROOTFS="$THIRD_PARTY/kyber-desktop/rootfs-arm64-macos"
OUT="$HERE/Vendor"
. "$ROOT/scripts/kyber-toolchain.sh"

[ -d "$OUT/Frameworks" ] || { echo "Lancez d'abord scripts/vendor-kyber.sh" >&2; exit 1; }

# Remplacer les bibliothèques sous l'app ouverte fait planter VLC à la
# session suivante (docs/build-notes.md).
app_closed() {
    if pgrep -x KyberNow >/dev/null; then
        echo "KyberNow est ouvert : quittez l'app avant de remplacer ses bibliothèques (Vendor/)." >&2
        exit 1
    fi
}
app_closed

# Sortie de cargo sans les lignes de progression ; avec pipefail, le code de
# retour reste celui de cargo.
cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating) ' || true; }
}

# Chaque correctif vise le dépôt nommé après son numéro.
for patch in "$HERE"/kyber-patches/*.patch; do
    case "$(basename "$patch")" in
        *-kyclient-*) repo="$KYSDK/kyctl" ;;
        *-kynput-*) repo="$KYSDK/kynput" ;;
        *-kynet-*) repo="$KYSDK/kymux" ;;
        *) echo "Dépôt inconnu pour $(basename "$patch")" >&2; exit 1 ;;
    esac
    if git -C "$repo" apply --check "$patch" 2>/dev/null; then
        git -C "$repo" apply "$patch" && echo "==> $(basename "$patch") appliqué"
    elif git -C "$repo" apply --reverse --check "$patch" 2>/dev/null; then
        echo "==> $(basename "$patch") déjà appliqué"
    else
        echo "Correctif $(basename "$patch") inapplicable :" >&2
        git -C "$repo" apply --check "$patch" >&2 || true
        exit 1
    fi
done

export PATH="$ROOTFS/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export PKG_CONFIG_LIBDIR="$ROOTFS/lib/pkgconfig"
# Le « strip » de cargo laisse une table de symboles que l'éditeur de liens
# d'Xcode refuse (« mis-aligned LINKEDIT string pool »).
export CARGO_PROFILE_RELEASE_STRIP=false
kyber_toolchain_check rustc cargo-c

# cargo cinstall garde la date de la bibliothèque déjà compilée : on retire
# celles d'un build précédent pour ne copier que ce que ce build a installé.
rm -f "$ROOTFS"/lib/libkynput.0.1.0.dylib "$ROOTFS"/lib/libkyclient.0.1.0.dylib

echo "==> libkynput"
(cd "$KYSDK/kynput" && cargo_quiet cargo cinstall -p kynput --target aarch64-apple-darwin --release \
    --prefix="$ROOTFS" --features sdl2)
echo "==> libkyclient"
(cd "$KYSDK/kyctl" && cargo_quiet cargo cinstall -p kyclient --target aarch64-apple-darwin --release \
    --prefix="$ROOTFS" --features=kymux,rtp)

app_closed
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
