#!/bin/bash
# Assemble dans DEST les seules sources dont l'app Linux a besoin : apps/core,
# apps/linux, les correctifs de Kyber, et le SDK de Kyber sans ses compilations
# (target, builddir, sous-projets de kymedia hors vlc-rs). Sert à flatpak.sh
# (source du Flatpak) et à compiler dans une machine virtuelle.
#
# Usage : apps/linux/scripts/sources.sh [--deps] DEST
#   --deps : seulement de quoi compiler libkynput et FFmpeg (module du
#            Flatpak qui ne change pas avec l'app, mis en cache).
#   --media : build.sh, les correctifs de PyroWave et les scripts communs
#            qu'il lit seuls (FFmpeg et PyroWave, premier module du Flatpak :
#            ni Kyber ni l'app).
# KYBERNOW_KYBER_HEAD=1 : le SDK de Kyber pris dans ses dépôts git (HEAD), sans
#            les correctifs déjà posés dans third_party/ (voir plus bas).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DEPS=0
MEDIA=0
[ "${1:-}" = --deps ] && { DEPS=1; shift; }
[ "${1:-}" = --media ] && { MEDIA=1; shift; }
DEST="${1:?usage : sources.sh [--deps] DEST}"
# Ce que build.sh lit dans tous les cas : versions et commit de PyroWave
# (scripts/kyber-toolchain.sh, apps/macos/PyroWave/COMMIT), pose des correctifs
# (scripts/kyber-client-patches.sh et kyber-patches-lib.sh).
mkdir -p "$DEST/scripts" "$DEST/apps/macos/PyroWave"
cp "$ROOT/scripts/kyber-toolchain.sh" "$ROOT/scripts/kyber-patches-lib.sh" "$ROOT/scripts/kyber-client-patches.sh" "$DEST/scripts/"
cp "$ROOT/apps/macos/PyroWave/COMMIT" "$DEST/apps/macos/PyroWave/"
if [ "$MEDIA" = 1 ]; then
    mkdir -p "$DEST/apps/linux/scripts" "$DEST/apps/linux/pyrowave"
    cp "$HERE/scripts/build.sh" "$DEST/apps/linux/scripts/"
    rm -f "$DEST/apps/linux/pyrowave/"* && cp "$HERE/pyrowave/"* "$DEST/apps/linux/pyrowave/"
    exit 0
fi
mkdir -p "$DEST/apps" "$DEST/third_party/kyber-desktop" "$DEST/brand/v2"
# Sans dist/ : apps/linux/dist garde les paquets et l'image de KyberNow Link OS
# (plusieurs Go), que flatpak-builder recopiait et relisait à chaque construction.
R=(rsync -a --delete --exclude target/ --exclude 'builddir*/' --exclude node_modules/ --exclude '.git/' --exclude /dist/)
if [ "$DEPS" = 1 ]; then
    mkdir -p "$DEST/apps/linux/scripts" "$DEST/apps/linux/pyrowave"
    cp "$HERE/scripts/build.sh" "$DEST/apps/linux/scripts/"
    cp "$HERE/pyrowave/"* "$DEST/apps/linux/pyrowave/"
else
    "${R[@]}" "$ROOT/apps/core" "$ROOT/apps/linux" "$DEST/apps/"
    # Le cœur dépend du tunnel du relais (KyberNow Share), par chemin.
    mkdir -p "$DEST/server/relay"
    "${R[@]}" "$ROOT/server/relay/tunnel" "$DEST/server/relay/"
fi
mkdir -p "$DEST/apps/macos" "$DEST/apps/webos"
"${R[@]}" "$ROOT/apps/macos/kyber-patches" "$DEST/apps/macos/"
"${R[@]}" "$ROOT/apps/webos/kyber-patches" "$DEST/apps/webos/"
# Côté client des correctifs de la Station (*-kyclient-*, comme 0022 : HDR),
# appliqués par build.sh après ceux de apps/macos, et ceux de la Station pour
# kyctl, kymux et kynput (*-kycontroller-*, *-kywebplayer-*, *-kynet-*,
# *-kynput-*), posés avant eux : le canal de retour du client (0051-kyclient)
# s'appuie sur celui de kymux (0050-kynet).
mkdir -p "$DEST/server/kyber-patches"
rm -f "$DEST"/server/kyber-patches/*.patch
for p in "$ROOT"/server/kyber-patches/*.patch; do
    case "$(basename "$p")" in
        *-kyclient-* | *-kycontroller-* | *-kywebplayer-* | *-kynet-* | *-kynput-*) cp "$p" "$DEST/server/kyber-patches/" ;;
    esac
done
cp "$ROOT/apps/Cargo.lock" "$DEST/apps/"
# Espace de travail réduit au cœur et à l'app Linux.
sed -E 's/^members = .*/members = ["core", "linux"]/' "$ROOT/apps/Cargo.toml" > "$DEST/apps/Cargo.toml"
if [ "${KYBERNOW_KYBER_HEAD:-0}" = 1 ]; then
    # Le SDK de Kyber tel qu'il est dans ses dépôts git (HEAD de kysdk et de ses
    # sous-modules), sans les correctifs posés dans third_party/ : ceux de cette
    # branche s'y appliquent ensuite (build.sh). Utile quand le third_party/
    # partagé porte les correctifs d'un autre travail. Lecture seule.
    SDK="$ROOT/third_party/kyber-desktop/kysdk"
    rm -rf "$DEST/third_party/kyber-desktop/kysdk" && mkdir -p "$DEST/third_party/kyber-desktop/kysdk"
    git -C "$SDK" archive HEAD | tar -x -C "$DEST/third_party/kyber-desktop/kysdk"
    git -C "$SDK" submodule status --recursive | awk '{print $2}' | while read -r sub; do
        case "$sub" in kymedia/subprojects/vlc-rs) ;; kymedia/subprojects/*) continue ;; esac
        mkdir -p "$DEST/third_party/kyber-desktop/kysdk/$sub"
        git -C "$SDK/$sub" archive HEAD | tar -x -C "$DEST/third_party/kyber-desktop/kysdk/$sub"
    done
else
    "${R[@]}" --exclude 'contrib/work/' --exclude 'contrib/build/' \
        --include 'kymedia/subprojects/vlc-rs/***' --exclude 'kymedia/subprojects/*' \
        "$ROOT/third_party/kyber-desktop/kysdk" "$DEST/third_party/kyber-desktop/"
fi
cp "$ROOT/third_party/kyber-desktop/"COPYING* "$DEST/third_party/kyber-desktop/" 2>/dev/null || true
"${R[@]}" "$ROOT/brand/v2/app-icon" "$DEST/brand/v2/"
# Clé publique des versions signées (mises à jour, apps/core/src/update.rs).
# (mkdir + cp plutôt qu'install -D, absent de macOS : release.sh vérifie cette copie sur le Mac.)
mkdir -p "$DEST/server/admin/src/update"
cp "$ROOT/server/admin/src/update/kybernow-release.pub" "$DEST/server/admin/src/update/kybernow-release.pub"
# La licence du dépôt (AGPL-3.0), à la racine : le Flatpak la porte toujours, son absence est une erreur.
[ -f "$ROOT/LICENSE" ] || { echo "LICENSE absent de $ROOT : le Flatpak doit porter la licence" >&2; exit 1; }
cp "$ROOT/LICENSE" "$DEST/LICENSE"
echo "sources dans $DEST ($(du -sh "$DEST" | cut -f1))"
