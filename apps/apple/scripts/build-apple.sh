#!/bin/bash
# Compile le cœur KyberNow et son pont C pour le Mac (apps/core, apps/apple)
# et les range dans apps/macos/Vendor : voir docs/coeur-apple.md.
#
#   Vendor/KyberNow/include/kybernow/kybernow.h   en-tête (cbindgen, par cargo cinstall)
#   Vendor/KyberNow/include/module.modulemap      module Swift CKyberNow
#   Vendor/Frameworks/libkybernow.dylib           la bibliothèque, embarquée et signée
#                                                 par scripts/embed-kyber.sh comme les autres
#
# Le pont lie libkynput et libvlc du SDK Kyber (chaîne d'entrées, lecteur
# jamais appelé) : lancer d'abord apps/macos/scripts/vendor-kyber.sh et
# apps/macos/scripts/build-kyclient.sh. S'arrête à la première erreur sans
# toucher à Vendor/. Refuse de tourner pendant que l'app est ouverte (une
# bibliothèque réécrite sous l'app la fait planter, docs/build-notes.md).
# Versions vérifiées : scripts/kyber-toolchain.sh.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
MAC="$ROOT/apps/macos"
# Chemin réel de third_party/ (un lien dans un worktree) : cargo garde ainsi
# ses dossiers de build, liés au chemin absolu des sources.
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
ROOTFS="$THIRD_PARTY/kyber-desktop/rootfs-arm64-macos"
VENDOR="$MAC/Vendor"
PREFIX="$VENDOR/KyberNow"
TARGET=aarch64-apple-darwin
. "$ROOT/scripts/kyber-toolchain.sh"

[ -d "$VENDOR/Frameworks" ] || { echo "Lancez d'abord apps/macos/scripts/vendor-kyber.sh" >&2; exit 1; }
[ -f "$ROOTFS/lib/pkgconfig/kynput.pc" ] || { echo "libkynput introuvable dans $ROOTFS : lancez apps/macos/scripts/build-kyclient.sh" >&2; exit 1; }

# Vendor/ d'un autre checkout (worktree) que celui de l'app ouverte : KYBERNOW_IGNORE_RUNNING=1.
if [ "${KYBERNOW_IGNORE_RUNNING:-0}" != 1 ] && pgrep -x "KyberNow( Link)?" >/dev/null; then
    echo "KyberNow est ouvert : quittez l'app avant de remplacer ses bibliothèques (Vendor/)." >&2
    exit 1
fi

export PATH="$ROOTFS/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export PKG_CONFIG_LIBDIR="$ROOTFS/lib/pkgconfig"
export LIBCLANG_PATH="${LIBCLANG_PATH:-/Library/Developer/CommandLineTools/usr/lib}"
# Le « strip » de cargo laisse une table de symboles que l'éditeur de liens
# d'Xcode refuse (« mis-aligned LINKEDIT string pool »).
export CARGO_PROFILE_RELEASE_STRIP=false
kyber_toolchain_check rustc cargo-c

cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing) ' || true; }
}

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/kybernow-apple.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT

echo "==> kybernow-core + kybernow-apple ($TARGET)"
(cd "$ROOT/apps" && cargo_quiet cargo cinstall -p kybernow-apple --target "$TARGET" --release \
    --library-type cdylib --prefix="$STAGE")

LIB="$STAGE/lib/libkybernow.dylib"
[ -f "$LIB" ] || { echo "libkybernow.dylib absente de $STAGE/lib" >&2; exit 1; }
# Identifiant et dépendances en @rpath, comme les bibliothèques du SDK.
install_name_tool -id "@rpath/libkybernow.dylib" "$LIB"
for dep in $(otool -L "$LIB" | awk 'NR>1 {print $1}' | grep -E "rootfs-arm64-macos|/kyber-desktop/|^lib" || true); do
    install_name_tool -change "$dep" "@rpath/$(basename "$dep")" "$LIB"
done

rm -rf "$PREFIX"
mkdir -p "$PREFIX/include"
cp -R "$STAGE/include/kybernow" "$PREFIX/include/"
cat > "$PREFIX/include/module.modulemap" <<'EOF'
module CKyberNow {
    header "kybernow/kybernow.h"
    link "kybernow"
    export *
}
EOF
cp "$LIB" "$VENDOR/Frameworks/libkybernow.dylib"
echo "libkybernow.dylib ($(du -h "$LIB" | cut -f1)) et kybernow.h dans $VENDOR"
otool -L "$VENDOR/Frameworks/libkybernow.dylib" | awk 'NR>1 {print "   " $1}'
