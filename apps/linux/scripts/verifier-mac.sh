#!/bin/bash
# Vérifie le Rust de l'app entière (partie appareil comprise) sur le Mac, sans
# FFmpeg, SDL ni Wayland (build.rs ne compile rien de natif avec
# KYBERNOW_CHECK_ONLY=1), et fait tourner ses essais qui n'appellent pas de
# code natif : les symboles natifs sont laissés non résolus à l'édition des
# liens (macOS), un essai qui en appelle un s'arrête net. Chaque essai tourne
# donc dans son propre processus.
#
#   apps/linux/scripts/verifier-mac.sh            cargo check (app et essais)
#   apps/linux/scripts/verifier-mac.sh --essais   puis les essais de src/device
#
# Il faut les sources de Kyber avec les correctifs des apps posés (comme
# build.sh : scripts/kyber-client-patches.sh, plus apps/webos/kyber-patches),
# dans le third_party/ de l'arbre qui construit (scripts/arbres.sh), et un
# kynput.h à jour pour bindgen (KYNPUT_INCLUDE, par défaut celui de l'app Mac,
# apps/macos/Vendor/include, fait par build-kyclient.sh).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)"
APPS="$ROOT/apps"
KYNPUT_INCLUDE="${KYNPUT_INCLUDE:-$APPS/macos/Vendor/include}"
[ -f "$KYNPUT_INCLUDE/kynput.h" ] || { echo "kynput.h absent de $KYNPUT_INCLUDE (KYNPUT_INCLUDE)" >&2; exit 1; }
[ -f "$ROOT/third_party/kyber-desktop/kysdk/kyctl/kyclient/Cargo.toml" ] || { echo "sources de Kyber absentes de $ROOT/third_party" >&2; exit 1; }

PC="$(mktemp -d)"
trap 'rm -rf "$PC"' EXIT
# libvlc : kyvlcplayer la nomme, l'app ne l'appelle jamais ; kynput : l'en-tête seul.
printf 'Name: libvlc\nDescription: absente (vérification)\nVersion: 3.0.0\nLibs:\nCflags:\n' > "$PC/libvlc.pc"
printf 'Name: kynput\nDescription: en-tête seul (vérification)\nVersion: 0.1.0\nLibs:\nCflags: -I%s\n' "$KYNPUT_INCLUDE" > "$PC/kynput.pc"

export PKG_CONFIG_PATH="$PC" KYBERNOW_CHECK_ONLY=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$APPS/target/verifier-mac}"
cd "$APPS"
RUSTFLAGS="--cfg kybernow_webos" cargo check -p kybernow-linux --features pyrowave --tests

if [ "${1:-}" = "--essais" ]; then
    # Le binaire d'essai : celui que cargo nomme (« Executable … »).
    BIN="$(RUSTFLAGS="--cfg kybernow_webos -C link-arg=-Wl,-undefined,dynamic_lookup" cargo test -p kybernow-linux --features pyrowave --no-run 2>&1 | sed -n 's/.*Executable .*(\(.*\))$/\1/p' | tail -1)"
    [ -x "$BIN" ] || { echo "binaire d'essai introuvable" >&2; exit 1; }
    ok=0; ko=0
    while read -r t; do
        if "$BIN" --exact "$t" --test-threads=1 >/dev/null 2>&1; then
            ok=$((ok + 1))
        else
            ko=$((ko + 1)); echo "échec (ou code natif) : $t"
        fi
    done < <("$BIN" --list 2>/dev/null | sed -n 's/: test$//p' | grep '^device::')
    echo "essais de src/device : $ok passent, $ko non (code natif : la carte graphique, SDL, X11)"
fi
