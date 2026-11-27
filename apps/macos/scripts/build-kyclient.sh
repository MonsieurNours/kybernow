#!/bin/bash
# Recompile les bibliothèques client de Kyber avec les correctifs KyberNow
# (kyber-patches/) et les remplace dans Vendor/ (lancer
# scripts/vendor-kyber.sh une première fois). Détail : docs/kyber-changes.md.
#
# 0001-kyclient : les paquets audio et vidéo vont à l'app au lieu de VLC.
# 0002-kynput   : paquets de presse-papiers lisibles et fabriqués depuis l'app.
# 0003-kynet    : grande mémoire de réception UDP (rafales d'une image 4K).
# 0004-kynput   : client Windows sans le presse-papiers de kynput (cfg, sans
#                 effet ici).
# Puis ceux de server/kyber-patches nommés *-kyclient-* (côté client d'un
# correctif de la Station, posés aussi par son build.sh, car kycontroller lit
# le même kyctl-common) :
# 0022-kyclient : HDR par écran (VideoSessionConfig.hdr, requête de démarrage).
# 0025-kyclient-pyrowave : seulement avec --kyber-pyrowave (ou
#                 KYBER_PYROWAVE=1), comme build-apple.sh : sans la
#                 fonctionnalité pyrowave, le cœur ne compile pas avec lui.
#
# libkynput est construite sans son lecteur de manettes SDL : les manettes
# sont lues une seule fois pour toutes les sessions de l'app par le cœur
# (apps/core/src/gamepads, plusieurs ordinateurs à la fois), qui lie SDL
# lui-même (libkybernow.dylib, apps/apple/scripts/build-apple.sh).
#
# Empreinte : à la fin, Vendor/kyclient.stamp reçoit l'empreinte des correctifs
# posés (kyber_client_patch_files de scripts/kyber-client-patches.sh, ceux de
# la Station compris), de ce script et des scripts qu'il lit (chaîne d'outils,
# pose des correctifs) ;
# scripts/release.sh la compare à `build-kyclient.sh --stamp` (l'empreinte
# attendue, sans rien compiler) et recompile si elles diffèrent.
#
# S'arrête à la première erreur (correctif inapplicable, compilation ratée)
# sans toucher à Vendor/. Refuse de tourner pendant que l'app est ouverte
# (dans un worktree git : KYBERNOW_IGNORE_RUNNING=1, voir app_closed).
# Versions vérifiées : scripts/kyber-toolchain.sh (une autre est signalée).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
OUT="$HERE/Vendor"
. "$ROOT/scripts/kyber-toolchain.sh"

# Empreinte de ce qui fait les bibliothèques client : correctifs, ce script,
# versions des outils.
KYBER_PYROWAVE="${KYBER_PYROWAVE:-1}"
STAMP_ONLY=0
for arg in "$@"; do
    case "$arg" in
        --stamp) STAMP_ONLY=1 ;;
        --kyber-pyrowave) KYBER_PYROWAVE=1 ;;
        --sans-pyrowave) KYBER_PYROWAVE=0 ;;
        *) echo "Option inconnue : $arg" >&2; exit 1 ;;
    esac
done
# Liste des correctifs : la même pour la pose et pour l'empreinte, correctifs
# de la Station compris (un 0027-kynput changé recompile libkynput).
. "$ROOT/scripts/kyber-client-patches.sh"
export KYBER_PYROWAVE
stamp() {
    local files=()
    while IFS= read -r f; do files+=("$f"); done < <(kyber_client_patch_files)
    { for f in "${files[@]}"; do basename "$f"; cat "$f"; done
      cat "$HERE/scripts/build-kyclient.sh" "$ROOT/scripts/kyber-toolchain.sh" "$ROOT/scripts/kyber-client-patches.sh" \
          "$ROOT/scripts/kyber-patches-lib.sh"; echo "pyrowave=$KYBER_PYROWAVE"; } | kyber_sha256
}
if [ "$STAMP_ONLY" = 1 ]; then
    stamp
    exit 0
fi

# L'empreinte seule se calcule sans third_party/ (essais de l'outillage, CI).
# Chemin réel de third_party/ (un lien dans un worktree) : meson et cargo
# gardent ainsi leurs dossiers de build, liés au chemin absolu des sources.
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYSDK="$THIRD_PARTY/kyber-desktop/kysdk"
ROOTFS="$THIRD_PARTY/kyber-desktop/rootfs-arm64-macos"

[ -d "$OUT/Frameworks" ] || { echo "Lancez d'abord scripts/vendor-kyber.sh" >&2; exit 1; }

# Remplacer les bibliothèques sous l'app ouverte fait planter VLC à la
# session suivante (docs/build-notes.md). Dans un worktree git, dont le
# Vendor/ n'est pas celui du dépôt principal, KYBERNOW_IGNORE_RUNNING=1 laisse
# tourner le script app ouverte ; sans effet dans le dépôt principal.
other_checkout() {
    local dir common
    dir="$(git -C "$ROOT" rev-parse --path-format=absolute --git-dir 2>/dev/null)" || return 1
    common="$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || return 1
    [ "$dir" != "$common" ]
}
app_closed() {
    if [ "${KYBERNOW_IGNORE_RUNNING:-0}" = 1 ] && other_checkout; then
        return
    fi
    if pgrep -x "KyberNow( Link)?" >/dev/null; then
        echo "KyberNow est ouvert : quittez l'app avant de remplacer ses bibliothèques (Vendor/ ; dans un worktree : KYBERNOW_IGNORE_RUNNING=1)." >&2
        exit 1
    fi
}
app_closed
# Une seule construction à la fois dans ce Vendor/ (scripts/verrou.sh).
. "$ROOT/scripts/verrou.sh"
kyber_verrou_sortie "$OUT" "build-kyclient.sh" || exit 1

# Chaque correctif vise le dépôt nommé après son numéro ; ses fichiers sont
# remis à l'origine puis tous reposés dans l'ordre (scripts/kyber-client-patches.sh).
kyber_apply_app_patches "$KYSDK" || exit 1

export PATH="$ROOTFS/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export PKG_CONFIG_LIBDIR="$ROOTFS/lib/pkgconfig"
# Le « strip » de cargo laisse une table de symboles que l'éditeur de liens
# d'Xcode refuse (« mis-aligned LINKEDIT string pool »).
export CARGO_PROFILE_RELEASE_STRIP=false
kyber_toolchain_check rustc cargo-c

# cargo cinstall garde la date de la bibliothèque déjà compilée : on retire
# celles d'un build précédent pour ne copier que ce que ce build a installé.
rm -f "$ROOTFS"/lib/libkynput.0.1.0.dylib "$ROOTFS"/lib/libkyclient.0.1.0.dylib

echo "==> libkynput (sans lecteur de manettes SDL)"
(cd "$KYSDK/kynput" && cargo_quiet cargo cinstall -p kynput --target aarch64-apple-darwin --release \
    --prefix="$ROOTFS")
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
stamp > "$OUT/kyclient.stamp"
echo "libkynput et libkyclient à jour dans $OUT"
