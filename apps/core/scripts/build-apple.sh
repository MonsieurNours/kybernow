#!/bin/bash
# Compile le cœur client (kybernow-core, apps/core) pour les cibles Apple, avec
# les correctifs KyberNow de kyclient (apps/macos/kyber-patches), et laisse les
# artefacts dans apps/target/<cible>/release. La crate est une rlib : la
# bibliothèque statique et le xcframework des apps viennent de la crate du pont
# C (apps/apple/scripts/build-apple-mobile.sh). Ce script sert à vérifier que
# toute la chaîne de dépendances (kyclient avec kymux, kynput-rs, kyvlcplayer,
# rustls, ureq…) compile pour chaque cible, et à construire libkynput pour iOS.
#
#   apps/core/scripts/build-apple.sh                      toutes les cibles
#   apps/core/scripts/build-apple.sh --target <cible>     une seule
#   apps/core/scripts/build-apple.sh --check              cargo check seul
#
#   aarch64-apple-darwin      Rust stable (référence : le Mac qui compile)
#   aarch64-apple-ios         Rust stable, rang 2 (rustup target add)
#   aarch64-apple-ios-sim     Rust stable, rang 2 (rustup target add)
#   aarch64-apple-tvos        Rust nightly et -Zbuild-std (rang 3), comme
#   aarch64-apple-tvos-sim    apps/tvos/scripts/build-kyber-tvos.sh
#
# kynput-rs lit l'API C de libkynput (bindgen, pkg-config) : chaque cible a
# besoin d'un rootfs third_party/kyber-desktop/rootfs-<cible> avec kynput.pc.
# Celui du Mac vient de vendor-kyber.sh ; ceux de l'Apple TV de
# build-kyber-tvos.sh ; pour iOS (et une cible tvOS pas encore construite), ce
# script compile libkynput lui-même (cargo cinstall, sans SDL : sur iOS et
# tvOS, les manettes passent par GameController). Déploiement iOS 18 et tvOS 18.
#
# S'arrête à la première erreur (correctif inapplicable, compilation ratée).
# Versions vérifiées : scripts/kyber-toolchain.sh (une autre est signalée).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
# Chemin réel de third_party/ (un lien dans un worktree) : cargo enregistre les
# chemins absolus des sources, un autre chemin lui fait tout recompiler.
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYBER="$THIRD_PARTY/kyber-desktop"
KYSDK="$KYBER/kysdk"
DEPLOYMENT=18.0
. "$ROOT/scripts/kyber-toolchain.sh"

ALL_TARGETS=(aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-tvos aarch64-apple-tvos-sim)
TARGETS=()
COMMAND=build
while [ $# -gt 0 ]; do
    case "$1" in
        --target) TARGETS+=("${2:-}"); shift ;;
        --check) COMMAND=check ;;
        *) echo "Option inconnue : $1" >&2; exit 1 ;;
    esac
    shift
done
[ ${#TARGETS[@]} -gt 0 ] || TARGETS=("${ALL_TARGETS[@]}")
for target in "${TARGETS[@]}"; do
    case " ${ALL_TARGETS[*]} " in
        *" $target "*) ;;
        *) echo "Cible inconnue : $target (${ALL_TARGETS[*]})" >&2; exit 1 ;;
    esac
done

[ -d "$KYSDK/kyctl" ] || { echo "SDK Kyber introuvable dans $KYSDK (voir docs/build-notes.md)" >&2; exit 1; }

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
# bindgen (kynput-sys) : la libclang des outils en ligne de commande d'Xcode.
export LIBCLANG_PATH="${LIBCLANG_PATH:-/Library/Developer/CommandLineTools/usr/lib}"
export PKG_CONFIG_ALLOW_CROSS=1
# Le « strip » de cargo laisse une table de symboles que l'éditeur de liens
# d'Xcode refuse (« mis-aligned LINKEDIT string pool »).
export CARGO_PROFILE_RELEASE_STRIP=false

# Sortie de cargo sans les lignes de progression ; avec pipefail, le code de
# retour reste celui de cargo.
cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing) ' || true; }
}

# Chaîne et rootfs de chaque cible.
needs_nightly() { case "$1" in aarch64-apple-tvos*) return 0 ;; *) return 1 ;; esac; }
rootfs_of() {
    case "$1" in
        aarch64-apple-darwin) echo "$KYBER/rootfs-arm64-macos" ;;
        *) echo "$KYBER/rootfs-$1" ;;
    esac
}

kyber_toolchain_check rustc
for target in "${TARGETS[@]}"; do
    if needs_nightly "$target"; then
        rustup run nightly rustc --version >/dev/null 2>&1 || {
            echo "Rust nightly manquant : rustup toolchain install nightly --profile minimal --component rust-src" >&2
            exit 1
        }
        kyber_toolchain_check nightly
        break
    fi
done
for target in "${TARGETS[@]}"; do
    needs_nightly "$target" && continue
    if ! rustup target list --installed 2>/dev/null | grep -qx "$target"; then
        echo "Cible Rust $target absente : rustup target add $target" >&2
        exit 1
    fi
done

# Mêmes correctifs que les apps (image et son vers l'app, presse-papiers,
# mémoire de réception UDP) : le cœur compile contre cette API de kyclient.
for patch in "$ROOT"/apps/macos/kyber-patches/*.patch; do
    case "$(basename "$patch")" in
        *-kyclient-*) repo="$KYSDK/kyctl" ;;
        *-kynput-*) repo="$KYSDK/kynput" ;;
        *-kynet-*) repo="$KYSDK/kymux" ;;
        *) echo "Dépôt inconnu pour $(basename "$patch")" >&2; exit 1 ;;
    esac
    if git -C "$repo" apply --check "$patch" 2>/dev/null; then
        git -C "$repo" apply "$patch" && echo "==> $(basename "$patch") appliqué"
    elif ! git -C "$repo" apply --reverse --check "$patch" 2>/dev/null; then
        echo "Correctif $(basename "$patch") inapplicable :" >&2
        git -C "$repo" apply --check "$patch" >&2 || true
        exit 1
    fi
done

# libkynput (API C de kynput, sans SDL) dans le rootfs de la cible, pour
# kynput-sys. Le Mac ne passe pas ici : son rootfs vient de vendor-kyber.sh,
# avec le libvlc que kyvlcplayer lie sur macOS.
ensure_kynput() {
    local target=$1 prefix=$2
    [ -f "$prefix/lib/pkgconfig/kynput.pc" ] && return 0
    kyber_toolchain_check cargo-c
    echo "==> libkynput ($target) dans $prefix"
    mkdir -p "$prefix"
    unset IPHONEOS_DEPLOYMENT_TARGET TVOS_DEPLOYMENT_TARGET
    case "$target" in
        aarch64-apple-ios*) export IPHONEOS_DEPLOYMENT_TARGET=$DEPLOYMENT ;;
        aarch64-apple-tvos*) export TVOS_DEPLOYMENT_TARGET=$DEPLOYMENT ;;
    esac
    if needs_nightly "$target"; then
        (cd "$KYSDK/kynput" && cargo_quiet cargo +nightly cinstall -p kynput --target="$target" -Zbuild-std=std,panic_abort \
            --release --library-type=staticlib --prefix="$prefix")
    else
        (cd "$KYSDK/kynput" && cargo_quiet cargo cinstall -p kynput --target="$target" \
            --release --library-type=staticlib --prefix="$prefix")
    fi
    [ -f "$prefix/lib/pkgconfig/kynput.pc" ] || { echo "libkynput ($target) non installée dans $prefix" >&2; exit 1; }
}

build_target() {
    local target=$1 prefix started
    prefix="$(rootfs_of "$target")"
    if [ "$target" = aarch64-apple-darwin ]; then
        [ -f "$prefix/lib/pkgconfig/kynput.pc" ] && [ -f "$prefix/lib/pkgconfig/libvlc.pc" ] || {
            echo "$prefix sans kynput.pc et libvlc.pc : lancez d'abord apps/macos/scripts/vendor-kyber.sh" >&2
            exit 1
        }
    else
        ensure_kynput "$target" "$prefix"
    fi
    export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
    # Une seule cible de déploiement dans l'environnement : le clang de
    # bindgen refuse d'en voir deux (« conflicting deployment targets »).
    unset IPHONEOS_DEPLOYMENT_TARGET TVOS_DEPLOYMENT_TARGET
    case "$target" in
        aarch64-apple-ios*) export IPHONEOS_DEPLOYMENT_TARGET=$DEPLOYMENT ;;
        aarch64-apple-tvos*) export TVOS_DEPLOYMENT_TARGET=$DEPLOYMENT ;;
    esac

    local cargo=(cargo "$COMMAND" -p kybernow-core --release --target "$target")
    # shellcheck disable=SC2054  # la virgule est celle de -Zbuild-std=std,panic_abort
    needs_nightly "$target" && cargo=(cargo +nightly "$COMMAND" -p kybernow-core --release --target "$target" -Zbuild-std=std,panic_abort)
    echo "==> cargo $COMMAND ($target)"
    started=$(date +%s)
    (cd "$APPS" && cargo_quiet "${cargo[@]}")
    if [ "$COMMAND" = build ]; then
        local rlib="$APPS/target/$target/release/libkybernow_core.rlib"
        [ -f "$rlib" ] || { echo "$rlib absent" >&2; exit 1; }
        echo "    $rlib ($(du -h "$rlib" | cut -f1), $(( $(date +%s) - started )) s)"
    else
        echo "    $target vérifiée ($(( $(date +%s) - started )) s)"
    fi
}

for target in "${TARGETS[@]}"; do
    build_target "$target"
done
echo "kybernow-core : ${TARGETS[*]}"
