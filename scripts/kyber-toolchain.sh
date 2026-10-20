# Versions des outils avec lesquelles les binaires Kyber de KyberNow sont
# compilés et vérifiés (voir docs/build-notes.md). Lu par
# server/kyber-patches/build.sh, apps/macos/scripts/build-kyclient.sh et
# apps/tvos/scripts/build-kyber-tvos.sh : une version différente n'arrête pas
# la compilation, elle est signalée. Après une mise à jour vérifiée, changer la
# valeur ici et dans docs/build-notes.md.

KYBER_RUSTC=1.89.0            # Rust stable : serveur Windows et client macOS
KYBER_NIGHTLY=2026-10-19      # date du Rust nightly : client Apple TV (-Zbuild-std)
KYBER_CARGO_C=0.10.15         # cargo-c (cargo cinstall)
KYBER_MINGW=14.0.0            # mingw-w64 (Homebrew), en-têtes et UCRT
KYBER_MINGW_GCC=16.2.0        # x86_64-w64-mingw32-gcc
KYBER_MESON=1.11.2
KYBER_WASM_BINDGEN=0.2.100    # wasm-bindgen-cli : celle du Cargo.lock de kyber-web
KYBER_BINARYEN=133            # wasm-opt (Homebrew binaryen) : moteur web

# kyber_toolchain_check outil… : avertit pour chaque outil dont la version
# diffère (rustc, nightly, cargo-c, mingw, meson, wasm-bindgen, binaryen). À
# appeler une fois le PATH
# de la compilation posé.
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
