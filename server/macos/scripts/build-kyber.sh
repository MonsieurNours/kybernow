#!/bin/bash
# Compile le serveur de Kyber 0.29 pour un Mac à puce Apple (KyberNow Station
# pour Mac, docs/station-macos.md), avec les correctifs KyberNow : kycontroller,
# kyavserver et libtxproto (capture ScreenCaptureKit, encodage VideoToolbox),
# kynputserver et libkynput.
#
# Mêmes correctifs que pour Windows (server/kyber-patches, posés par
# server/kyber-patches/apply.sh), puis ceux propres au Mac
# (server/macos/kyber-patches, s'il y en a), dans l'ordre de leur nom.
#
# Sources : $KYBER_DIR (défaut : third_party/kyber-desktop du dépôt ou du
# worktree). Les sources sont remises à l'origine fichier par fichier, comme
# sous Windows : ne pas lancer ce script sur des sources qu'une autre
# compilation utilise en même temps (une publication en cours, par exemple).
#
# Sortie : $KYBER_DIR/rootfs-arm64-macos (bin/kycontroller, bin/kyavserver,
# bin/kynputserver, lib/*.dylib). Le script s'arrête à la première erreur.
#
# Prérequis (Homebrew) : meson ninja nasm cmake bison pkg-config ; Rust
# (scripts/kyber-toolchain.sh) avec la cible aarch64-apple-darwin ; cargo-c.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
. "$ROOT/scripts/kyber-toolchain.sh"

KYBER_DIR="$(cd "${KYBER_DIR:-$ROOT/third_party/kyber-desktop}" 2>/dev/null && pwd -P)" \
    || { echo "Sources de Kyber introuvables (KYBER_DIR, docs/build-notes.md)" >&2; exit 1; }
KYMEDIA="$KYBER_DIR/kysdk/kymedia"
KYCTL="$KYBER_DIR/kysdk/kyctl"
KYMUX="$KYBER_DIR/kysdk/kymux"
KYNPUT="$KYBER_DIR/kysdk/kynput"
# shellcheck disable=SC2034  # lu par apply.sh ; le moteur web se compile avec la Station Windows
KYWEB=""
OUT="$KYBER_DIR/rootfs-arm64-macos"

export PATH="/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
# Xcode 27 refuse les bibliothèques réduites par rust-objcopy (docs/build-notes.md).
export CARGO_PROFILE_RELEASE_STRIP=false
export RUSTUP_TOOLCHAIN="$KYBER_RUSTC"
kyber_toolchain_check rustc meson

STARTED=$(mktemp -t kyber-build-macos)
trap 'rm -f "$STARTED"' EXIT

# Correctifs communs (Windows et Mac).
HERE_PATCHES="$ROOT/server/kyber-patches"
(
    HERE="$HERE_PATCHES"
    # shellcheck source=../../kyber-patches/apply.sh
    . "$HERE_PATCHES/apply.sh"
    kyber_apply_server_patches
)

# Correctifs propres au Mac : même nommage que les correctifs communs.
MAC_PATCHES="$ROOT/server/macos/kyber-patches"
shopt -s nullglob
for p in "$MAC_PATCHES"/*.patch; do
    case "$(basename "$p")" in
        *-kycontroller-*) repo="$KYCTL" ;;
        *-kynet-*) repo="$KYMUX" ;;
        *-kynput-*) repo="$KYNPUT" ;;
        *) repo="$KYMEDIA" ;;
    esac
    for f in $(grep -h '^+++ b/' "$p" | sed 's|^+++ b/||; s|[[:space:]].*||'); do
        orig="$f"
        case "$f" in
            subprojects/txproto/*) dir="$repo/subprojects/txproto" f="${f#subprojects/txproto/}" ;;
            subprojects/libavconv/*) dir="$repo/subprojects/libavconv" f="${f#subprojects/libavconv/}" ;;
            *) dir="$repo" ;;
        esac
        # Un fichier déjà touché par un correctif commun reste tel quel.
        if ! grep -qsF "+++ b/${orig}" "$HERE_PATCHES"/*.patch && git -C "$dir" cat-file -e "HEAD:$f" 2>/dev/null; then
            git -C "$dir" checkout -q HEAD -- "$f"
        fi
    done
    echo "    $(basename "$p") (Mac)"
    (cd "$repo" && patch -p1 --forward --no-backup-if-mismatch -r - < "$p" | sed 's/^/        /') || {
        echo "Correctif $(basename "$p") inapplicable (hunk en échec ci-dessus)" >&2
        exit 1
    }
done
shopt -u nullglob

# Crates corrigées (server/macos/crates/<crate>-<version>-*.patch) : copie de la
# version du registre que verrouille kyctl, correctif posé, branchée par
# [patch.crates-io] dans kysdk/.cargo/config.toml (remis à l'origine d'abord).
# quinn-udp : sur macOS, réponse QUIC depuis l'adresse appelée par le client.
SDK="$KYBER_DIR/kysdk"
git -C "$SDK" checkout -q HEAD -- .cargo/config.toml
(cd "$KYCTL" && cargo fetch --quiet)
REGISTRY="${CARGO_HOME:-$HOME/.cargo}/registry/src"
shopt -s nullglob
for p in "$ROOT"/server/macos/crates/*.patch; do
    name="$(basename "$p" .patch)"
    crate="${name%%-[0-9]*}"
    version="${name#"$crate"-}"; version="${version%%-[!0-9.]*}"
    locked=$(awk -v c="$crate" '$0=="name = \""c"\"" {getline; gsub(/version = |"/, ""); print; exit}' "$KYCTL/Cargo.lock")
    [ "$locked" = "$version" ] || { echo "$crate : kyctl verrouille $locked, le correctif vise $version" >&2; exit 1; }
    src=$(find "$REGISTRY" -mindepth 2 -maxdepth 2 -type d -name "$crate-$version" | head -1)
    [ -n "$src" ] || { echo "$crate $version absent du registre de cargo" >&2; exit 1; }
    dst="$KYBER_DIR/target/kybernow-crates/$crate"
    rm -rf "$dst"; mkdir -p "$(dirname "$dst")"; cp -R "$src" "$dst"; chmod -R u+w "$dst"
    echo "    $(basename "$p") (crate)"
    (cd "$dst" && patch -p1 --forward --no-backup-if-mismatch -r - < "$p" | sed 's/^/        /') || {
        echo "Correctif $(basename "$p") inapplicable" >&2; exit 1
    }
    printf '%s = { path = "%s" }\n' "$crate" "$dst" >> "$SDK/.cargo/config.toml"
done
shopt -u nullglob

echo "==> Compilation (txproto, kyavserver, kynput, kycontroller)"
cd "$KYBER_DIR"
./build-macos.sh -a arm64 -o "$OUT" server

for f in bin/kycontroller bin/kyavserver bin/kynputserver; do
    [ -x "$OUT/$f" ] || { echo "$OUT/$f introuvable" >&2; exit 1; }
    [ "$OUT/$f" -nt "$STARTED" ] || { echo "$OUT/$f n'a pas été produit par ce build" >&2; exit 1; }
done
echo "==> Serveur Kyber pour Mac : $OUT"
