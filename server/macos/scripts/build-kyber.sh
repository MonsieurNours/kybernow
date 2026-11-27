#!/bin/bash
# Compile le serveur de Kyber 0.29 pour un Mac à puce Apple (KyberNow Station
# pour Mac, docs/station-macos.md), avec les correctifs KyberNow : kycontroller,
# kyavserver et libtxproto (capture ScreenCaptureKit, encodage VideoToolbox),
# kynputserver et libkynput.
#
# Mêmes correctifs que pour Windows (server/kyber-patches), puis ceux propres au
# Mac (server/macos/kyber-patches), dans l'ordre de leur nom, posés par
# server/kyber-patches/apply.sh comme sous Linux. Un morceau qui ne s'applique
# qu'avec un décalage du contexte (« fuzz ») est refusé (STCOM-05 ;
# KYBERNOW_PATCH_STRICT=0 : signalé seulement).
#
# Sources : $KYBER_DIR (défaut : third_party/kyber-desktop-macos, une copie de
# kyber-desktop à part : les correctifs Mac et son rootfs-arm64-macos ne se
# mêlent pas à ceux de la Station Windows et des apps, docs/build-notes.md). Les sources sont remises à l'origine fichier par fichier, comme
# sous Windows : ne pas lancer ce script sur des sources qu'une autre
# compilation utilise en même temps (une publication en cours, par exemple).
#
# Sortie : $KYBER_DIR/rootfs-arm64-macos (bin/kycontroller, bin/kyavserver,
# bin/kynputserver, lib/*.dylib ; bin/kybernow-pyrowave-check, la
# vérification PyroWave d'Admin). Le script s'arrête à la première erreur.
#
# KYBER_PATCH_ONLY=1 : poser les correctifs, sans compiler (pour écrire un
# correctif sur l'état d'après les autres). Une copie privée des sources
# (par exemple `cp -cR third_party/kyber-desktop-macos <dossier>` sur APFS,
# sans son rootfs-arm64-macos ni kysdk/kymedia/builddir-macos, qui pointent
# vers l'original) se compile avec KYBER_DIR=<dossier>.
#
# Prérequis (Homebrew) : meson ninja nasm cmake bison pkg-config ; Rust
# (scripts/kyber-toolchain.sh) avec la cible aarch64-apple-darwin ; cargo-c.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
. "$ROOT/scripts/kyber-toolchain.sh"

KYBER_DIR="$(cd "${KYBER_DIR:-$ROOT/third_party/kyber-desktop-macos}" 2>/dev/null && pwd -P)" \
    || { echo "Sources de Kyber introuvables (KYBER_DIR, docs/build-notes.md)" >&2; exit 1; }
KYMEDIA="$KYBER_DIR/kysdk/kymedia"
KYCTL="$KYBER_DIR/kysdk/kyctl"
# shellcheck disable=SC2034  # lu par apply.sh
KYMUX="$KYBER_DIR/kysdk/kymux"
# shellcheck disable=SC2034  # lu par apply.sh
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
# Une seule construction à la fois sur ces sources (scripts/verrou.sh).
. "$ROOT/scripts/verrou.sh"
kyber_verrou_sortie "$KYBER_DIR" "server/macos/scripts/build-kyber.sh" || exit 1

# Correctifs communs (Windows, Mac et Linux), puis ceux du Mac
# (server/kyber-patches/apply.sh, qui lit les correctifs communs dans HERE).
# shellcheck source=../../kyber-patches/apply.sh
. "$ROOT/server/kyber-patches/apply.sh"
(
    HERE="$ROOT/server/kyber-patches"
    kyber_apply_server_patches
    kyber_apply_own_patches "$ROOT/server/macos/kyber-patches" Mac
)

# Crates corrigées (server/macos/crates/<crate>-<version>-*.patch) : copie de la
# version du registre que verrouille kyctl, correctif posé, branchée par
# [patch.crates-io] dans kysdk/.cargo/config.toml (remis à l'origine d'abord).
# quinn-udp : sur macOS, réponse QUIC depuis l'adresse appelée par le client.
SDK="$KYBER_DIR/kysdk"
git -C "$SDK" checkout -q HEAD -- .cargo/config.toml
# Verrou d'origine : celui d'un build précédent désigne la copie corrigée, et
# cargo le résoudrait de nouveau sans elle (version plus récente).
git -C "$KYCTL" checkout -q HEAD -- Cargo.lock
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
    kyber_patch_put "$p" "$dst" || exit 1
    printf '%s = { path = "%s" }\n' "$crate" "$dst" >> "$SDK/.cargo/config.toml"
done
shopt -u nullglob

[ -n "${KYBER_PATCH_ONLY:-}" ] && { echo "==> Correctifs posés (KYBER_PATCH_ONLY)"; exit 0; }

# PyroWave (docs/pyrowave.md, « La Station Mac ») : le port Metal et la
# conversion de KyberNow en bibliothèque statique, dans le rootfs avant
# FFmpeg (son configure la trouve par pkg-config : encodeur
# libpyrowave_metal, correctif Mac 0016), et l'outil de vérification d'Admin.
echo "==> PyroWave Metal"
"$ROOT/server/macos/pyrowave/build.sh" "$OUT"

# FFmpeg est extrait une fois par meson, avec les fichiers du wrap de ce
# moment-là (0025 et le correctif Mac 0016 y ajoutent les encodeurs PyroWave) :
# extrait de nouveau quand ils changent (kyber_wrap_refresh, comme Windows et Linux).
kyber_wrap_refresh "$KYMEDIA" ffmpeg builddir-macos || exit 1

echo "==> Compilation (txproto, kyavserver, kynput, kycontroller)"
cd "$KYBER_DIR"
./build-macos.sh -a arm64 -o "$OUT" server
kyber_wrap_mark "$KYMEDIA" ffmpeg

for f in bin/kycontroller bin/kyavserver bin/kynputserver bin/kybernow-pyrowave-check; do
    [ -x "$OUT/$f" ] || { echo "$OUT/$f introuvable" >&2; exit 1; }
    [ "$OUT/$f" -nt "$STARTED" ] || { echo "$OUT/$f n'a pas été produit par ce build" >&2; exit 1; }
done
# Empreinte de ce build (kyber_station_hash d'apply.sh : correctifs, scripts,
# chaîne d'outils, PyroWave), comparée par scripts/release.sh, qui la calcule
# avec la même fonction.
kyber_station_hash "$ROOT" mac > "$OUT/.kybernow-patches"
echo "==> Serveur Kyber pour Mac : $OUT"
