#!/bin/bash
# Assemble dans DEST les seules sources dont l'app Linux a besoin : apps/core,
# apps/linux, les correctifs de Kyber, et le SDK de Kyber sans ses compilations
# (target, builddir, sous-projets de kymedia hors vlc-rs). Sert à flatpak.sh
# (source du Flatpak) et à compiler dans une machine virtuelle.
#
# Usage : apps/linux/scripts/sources.sh [--deps] DEST
#   --deps : seulement de quoi compiler libkynput et FFmpeg (module du
#            Flatpak qui ne change pas avec l'app, mis en cache).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DEPS=0
[ "${1:-}" = --deps ] && { DEPS=1; shift; }
DEST="${1:?usage : sources.sh [--deps] DEST}"
mkdir -p "$DEST/apps" "$DEST/third_party/kyber-desktop" "$DEST/brand/v2"
R=(rsync -a --delete --exclude target/ --exclude 'builddir*/' --exclude node_modules/ --exclude '.git/')
if [ "$DEPS" = 1 ]; then
    mkdir -p "$DEST/apps/linux/scripts"
    cp "$HERE/scripts/build.sh" "$DEST/apps/linux/scripts/"
else
    "${R[@]}" "$ROOT/apps/core" "$ROOT/apps/linux" "$DEST/apps/"
fi
mkdir -p "$DEST/apps/macos" "$DEST/apps/webos"
"${R[@]}" "$ROOT/apps/macos/kyber-patches" "$DEST/apps/macos/"
"${R[@]}" "$ROOT/apps/webos/kyber-patches" "$DEST/apps/webos/"
cp "$ROOT/apps/Cargo.lock" "$DEST/apps/"
# Espace de travail réduit au cœur et à l'app Linux.
sed -E 's/^members = .*/members = ["core", "linux"]/' "$ROOT/apps/Cargo.toml" > "$DEST/apps/Cargo.toml"
"${R[@]}" --exclude 'contrib/work/' --exclude 'contrib/build/' \
    --include 'kymedia/subprojects/vlc-rs/***' --exclude 'kymedia/subprojects/*' \
    "$ROOT/third_party/kyber-desktop/kysdk" "$DEST/third_party/kyber-desktop/"
cp "$ROOT/third_party/kyber-desktop/"COPYING* "$DEST/third_party/kyber-desktop/" 2>/dev/null || true
"${R[@]}" "$ROOT/brand/v2/app-icon" "$DEST/brand/v2/"
# Clé publique des versions signées (mises à jour, apps/core/src/update.rs).
install -Dm644 "$ROOT/server/admin/src/update/kybernow-release.pub" "$DEST/server/admin/src/update/kybernow-release.pub"
cp "$ROOT/LICENSE"* "$DEST/" 2>/dev/null || true
echo "sources dans $DEST ($(du -sh "$DEST" | cut -f1))"
