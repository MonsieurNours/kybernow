# shellcheck shell=bash
# Versions des outils et des dépendances avec lesquelles KyberNow est compilé
# et vérifié (voir docs/build-notes.md), en un seul endroit (audit TRANS-19,
# TRANS-20). Lu (source) par les scripts de construction des Stations
# (server/kyber-patches, server/macos/scripts, server/linux/scripts), des apps
# (apps/*/scripts) et par scripts/release.sh : une version d'outil différente
# n'arrête pas la compilation, elle est signalée. Après une mise à jour
# vérifiée, changer la valeur ici et dans docs/build-notes.md. Rien ne
# s'exécute au chargement.

KYBER_RUSTC=1.89.0            # Rust stable : serveur Windows, client macOS, cœur client pour iOS
KYBER_NIGHTLY=2026-10-19      # date du Rust nightly : client Apple TV et cœur client pour tvOS (-Zbuild-std)
KYBER_CARGO_C=0.10.15         # cargo-c (cargo cinstall)
KYBER_MINGW=14.0.0            # mingw-w64 (Homebrew), en-têtes et UCRT
KYBER_MINGW_GCC=16.2.0        # x86_64-w64-mingw32-gcc
KYBER_MESON=1.11.2
# shellcheck disable=SC2034  # lu par scripts/release.sh, apps/windows/scripts/build.sh et build-arm64-deps.sh
KYBER_LLVM_MINGW=20261006     # llvm-mingw (ucrt, macOS) : client Windows ARM64, dans third_party/toolchains
KYBER_WASM_BINDGEN=0.2.100    # wasm-bindgen-cli : celle du Cargo.lock de kyber-web
KYBER_BINARYEN=133            # wasm-opt (Homebrew binaryen) : moteur web
# shellcheck disable=SC2034  # lus par les scripts des apps et apps/linux/flatpak
{
KYBER_FFMPEG=7.1.1            # FFmpeg des apps Linux (décodage) et webOS (E-AC-3) ; les Stations ont celui de Kyber (wrap n8.1)
KYBER_FFMPEG_SHA256=733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1   # ffmpeg-7.1.1.tar.xz
KYBER_OPUS=1.6.1              # libopus (la version de Kyber 0.29) des apps Android et webOS ; les autres prennent celle des sources de Kyber
}

KYBER_TOOLCHAIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# kyber_pyrowave_commit : le commit de PyroWave de tout KyberNow, celui du port
# Metal vendu dans apps/macos/PyroWave (apps/macos/scripts/vendor-pyrowave.sh
# le change) : l'encodeur des Stations Windows, Mac et Linux et le décodeur de
# chaque app en viennent. Code 1 si le fichier manque ou ne nomme pas un commit.
kyber_pyrowave_commit() {
    local c
    c=$(tr -d ' \n' < "$KYBER_TOOLCHAIN_ROOT/apps/macos/PyroWave/COMMIT" 2>/dev/null) || true
    [[ "$c" =~ ^[0-9a-f]{40}$ ]] || { echo "apps/macos/PyroWave/COMMIT absent ou illisible" >&2; return 1; }
    echo "$c"
}

# kyber_sha256 : empreinte SHA-256 (hex) de l'entrée standard, sur macOS
# (shasum) comme sous Linux (sha256sum).
kyber_sha256() {
    if command -v shasum >/dev/null 2>&1; then shasum -a 256; else sha256sum; fi | cut -d' ' -f1
}

# cargo_quiet <commande>… : sortie de cargo sans les lignes de progression ;
# avec pipefail, le code de retour reste celui de cargo.
cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing|Installed|Replacing) ' || true; }
}

# kyber_toolchain_check outil… : avertit pour chaque outil dont la version
# diffère (rustc, nightly, cargo-c, mingw, meson, wasm-bindgen, binaryen). À
# appeler une fois le PATH de la compilation posé.
kyber_toolchain_check() {
    local tool have want inc
    for tool in "$@"; do
        case "$tool" in
            rustc)
                want=$KYBER_RUSTC
                have=$(rustc --version 2>/dev/null | awk '{print $2}') ;;
            nightly)
                want=$KYBER_NIGHTLY
                have=$(rustup run nightly rustc --version 2>/dev/null | sed -n 's/.* \([0-9-]*\))$/\1/p') ;;
            cargo-c)
                want=$KYBER_CARGO_C
                have=$(cargo cinstall --version 2>/dev/null | sed -n 's/^cargo-c \([0-9.]*\).*/\1/p') ;;
            mingw)
                want="$KYBER_MINGW (GCC $KYBER_MINGW_GCC)"
                inc=/opt/homebrew/opt/mingw-w64/toolchain-x86_64/x86_64-w64-mingw32/include/_mingw_mac.h
                have="$(awk '/define __MINGW64_VERSION_(MAJOR|MINOR|BUGFIX)/ {v = v (v ? "." : "") $3} END {print v}' "$inc" 2>/dev/null)"
                have="$have (GCC $(x86_64-w64-mingw32-gcc -dumpversion 2>/dev/null))" ;;
            meson)
                want=$KYBER_MESON
                have=$(meson --version 2>/dev/null) ;;
            wasm-bindgen)
                want=$KYBER_WASM_BINDGEN
                have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}') ;;
            binaryen)
                want=$KYBER_BINARYEN
                have=$(wasm-opt --version 2>/dev/null | awk '{print $3}') ;;
            *)
                echo "kyber_toolchain_check : outil inconnu « $tool »" >&2
                return 1 ;;
        esac
        if [ "$have" != "$want" ]; then
            echo "Attention : $tool ${have:-introuvable}, versions vérifiées : $want (docs/build-notes.md)" >&2
        fi
    done
}
