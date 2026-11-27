#!/bin/bash
# Compile le SDK client de Kyber 0.29 pour l'Apple TV (appareil et
# simulateur), avec les correctifs KyberNow de l'app Mac
# (apps/macos/kyber-patches, appliqués par apps/macos/scripts/build-kyclient.sh
# ou ici), et range le tout dans Vendor/ (ignoré par git).
#
#   libkynput   entrées (sans SDL : les manettes passent par GameController)
#   libkyclient connexion, réseau QUIC, paquets d'image et de son vers l'app
#   libopus     décodage du son
#   VLC         jamais utilisé (l'app décode l'image et le son) : des
#               fonctions vides satisfont l'éditeur de liens
#
# Les cibles Rust de tvOS sont de rang 3 : Rust nightly et la bibliothèque
# standard recompilée (-Zbuild-std), comme le fait build-ios.sh de Kyber.
# Le nightly est celui de scripts/kyber-toolchain.sh (KYBER_NIGHTLY), épinglé :
# la toolchain datée si elle est installée, sinon « nightly » seulement s'il
# est de cette date ; un autre arrête le script (l'empreinte de release.sh
# ne verrait pas le changement de compilateur).
#   rustup toolchain install nightly-<KYBER_NIGHTLY> --profile minimal --component rust-src
#
# Le correctif kyclient de PyroWave (0025, `VideoCodec::PyroWave`,
# `fallback_codec`) n'est posé qu'avec --kyber-pyrowave (comme
# build-native.sh --kyber-pyrowave d'Android), tant qu'il n'est pas allumé
# pour toutes les apps : third_party/ est partagé. Il va avec
# `build-apple-mobile.sh tvos --kyber-pyrowave` (fonctionnalité `pyrowave` du cœur).
#
# Sortie : Vendor/include (en-têtes, module.modulemap) et
# Vendor/KyberSDK.xcframework (une bibliothèque statique par plateforme).
# S'arrête à la première erreur (correctif inapplicable, compilation ratée)
# sans toucher à Vendor/ ; n'y remplace que ces deux entrées (le cœur,
# Vendor/KyberNow.xcframework, reste). Versions vérifiées : scripts/kyber-toolchain.sh (une
# autre est signalée).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# Chemin réel de third_party/ (un lien dans un worktree) : meson et cargo
# gardent ainsi leurs dossiers de build, liés au chemin absolu des sources.
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYBER="$THIRD_PARTY/kyber-desktop"
KYSDK="$KYBER/kysdk"
OPUS_SRC="$KYSDK/kymedia/subprojects/opus-1.6.1"
OUT="$HERE/Vendor"
WORK="$HERE/build/sdk"
DEPLOYMENT=18.0
. "$ROOT/scripts/kyber-toolchain.sh"
# Une seule construction à la fois dans ce Vendor/ (scripts/verrou.sh).
. "$ROOT/scripts/verrou.sh"
kyber_verrou_sortie "$OUT" "build-kyber-tvos.sh" || exit 1

KYBER_PYROWAVE="${KYBER_PYROWAVE:-1}"
while [ $# -gt 0 ]; do
    case "$1" in
        --kyber-pyrowave) KYBER_PYROWAVE=1 ;;
        --sans-pyrowave) KYBER_PYROWAVE=0 ;;
        *) echo "Option inconnue : $1 (--kyber-pyrowave)" >&2; exit 1 ;;
    esac
    shift
done

[ -d "$KYSDK/kyctl" ] || { echo "SDK Kyber introuvable dans $KYSDK (voir docs/build-notes.md)" >&2; exit 1; }
[ -d "$OPUS_SRC" ] || { echo "Sources d'Opus introuvables : $OPUS_SRC" >&2; exit 1; }

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
# Le nightly épinglé : `nightly-<date>` installé, ou `nightly` de la même date.
if rustup run "nightly-$KYBER_NIGHTLY" rustc --version >/dev/null 2>&1; then
    NIGHTLY="nightly-$KYBER_NIGHTLY"
elif [ "$(rustup run nightly rustc --version 2>/dev/null | sed -n 's/.* \([0-9-]*\))$/\1/p')" = "$KYBER_NIGHTLY" ]; then
    NIGHTLY=nightly
else
    echo "Rust nightly du $KYBER_NIGHTLY manquant : rustup toolchain install nightly-$KYBER_NIGHTLY --profile minimal --component rust-src" >&2
    exit 1
fi
kyber_toolchain_check cargo-c

# Mêmes correctifs que l'app Mac (image et son vers l'app, presse-papiers,
# mémoire de réception UDP).
. "$ROOT/scripts/kyber-client-patches.sh"
kyber_apply_app_patches "$KYSDK" || exit 1

export TVOS_DEPLOYMENT_TARGET=$DEPLOYMENT
export PKG_CONFIG_ALLOW_CROSS=1
# Le « strip » de cargo laisse une table de symboles que l'éditeur de liens
# d'Xcode refuse (« mis-aligned LINKEDIT string pool »).
export CARGO_PROFILE_RELEASE_STRIP=false

build_platform() {
    local target=$1 sdk=$2
    local prefix="$KYBER/rootfs-$target"
    mkdir -p "$prefix"
    export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"

    # cargo cinstall garde la date de la bibliothèque déjà compilée : on retire
    # celles d'un build précédent pour ne lier que ce que ce build a installé.
    rm -f "$prefix/lib/libkyclient.a" "$prefix/lib/libkynput.a"

    echo "==> libkynput ($target)"
    (cd "$KYSDK/kynput" && cargo_quiet cargo "+$NIGHTLY" cinstall -p kynput --target="$target" -Zbuild-std=std,panic_abort \
        --release --library-type=staticlib --prefix="$prefix")
    echo "==> libkyclient ($target)"
    (cd "$KYSDK/kyctl" && cargo_quiet cargo "+$NIGHTLY" cinstall -p kyclient --target="$target" -Zbuild-std=std,panic_abort \
        --release --library-type=staticlib --prefix="$prefix" --features=kymux,rtp)
    [ -f "$prefix/lib/libkyclient.a" ] && [ -f "$prefix/lib/libkynput.a" ] || { echo "Échec de la compilation Rust ($target)" >&2; exit 1; }

    echo "==> libopus ($target)"
    local opus="$WORK/opus-$sdk"
    cmake -S "$OPUS_SRC" -B "$opus" -G Ninja -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_SYSTEM_NAME=tvOS -DCMAKE_OSX_SYSROOT="$sdk" -DCMAKE_OSX_ARCHITECTURES=arm64 \
        -DCMAKE_OSX_DEPLOYMENT_TARGET=$DEPLOYMENT -DCMAKE_INSTALL_PREFIX="$prefix" \
        -DOPUS_BUILD_PROGRAMS=OFF -DOPUS_BUILD_TESTING=OFF -DOPUS_BUILD_SHARED_LIBRARY=OFF \
        -DOPUS_DRED=OFF -DOPUS_OSCE=OFF >/dev/null
    cmake --build "$opus" --target install >/dev/null

    # Fonctions de libvlc appelées par kyvlcplayer : jamais atteintes (l'image
    # et le son vont à l'app), elles rendent 0 / NULL.
    echo "==> fonctions vides pour VLC ($target)"
    local stubs="$WORK/vlc-stubs-$sdk.c"
    {
        echo "// Généré par build-kyber-tvos.sh : VLC n'est jamais appelé sur l'Apple TV."
        nm -u "$prefix/lib/libkyclient.a" 2>/dev/null | awk '/^_libvlc_/ {print substr($1, 2)}' | sort -u \
            | while read -r fn; do echo "void *$fn(void) { return 0; }"; done
    } > "$stubs"
    local triple="arm64-apple-tvos$DEPLOYMENT"
    [ "$sdk" = appletvsimulator ] && triple="arm64-apple-tvos$DEPLOYMENT-simulator"
    xcrun --sdk "$sdk" clang -target "$triple" -O2 -c "$stubs" -o "$WORK/vlc-stubs-$sdk.o"
    xcrun --sdk "$sdk" ar rcs "$WORK/libvlcstubs-$sdk.a" "$WORK/vlc-stubs-$sdk.o"

    # Une seule bibliothèque par plateforme.
    mkdir -p "$WORK/$sdk"
    xcrun libtool -static -no_warning_for_no_symbols -o "$WORK/$sdk/libKyberSDK.a" \
        "$prefix/lib/libkyclient.a" "$prefix/lib/libkynput.a" "$prefix/lib/libopus.a" "$WORK/libvlcstubs-$sdk.a"
}

mkdir -p "$WORK"
build_platform aarch64-apple-tvos appletvos
build_platform aarch64-apple-tvos-sim appletvsimulator

echo "==> Vendor/"
# Seulement ce que ce script produit : Vendor/KyberNow.xcframework (le cœur,
# build-apple-mobile.sh tvos) reste en place, sinon le projet Xcode ne se
# construit plus jusqu'à la prochaine construction du cœur.
rm -rf "$OUT/include" "$OUT/KyberSDK.xcframework" && mkdir -p "$OUT/include/opus"
DEVICE="$KYBER/rootfs-aarch64-apple-tvos"
cp "$DEVICE/include/kyclient/kyclient.h" "$DEVICE/include/kynput/kynput.h" "$OUT/include/"
cp "$DEVICE"/include/opus/opus*.h "$OUT/include/opus/"
cat > "$OUT/include/module.modulemap" <<'EOF'
module CKyber {
    header "kyclient.h"
    header "kynput.h"
    export *
}
module COpus {
    header "opus/opus_multistream.h"
    export *
}
EOF
xcodebuild -create-xcframework \
    -library "$WORK/appletvos/libKyberSDK.a" \
    -library "$WORK/appletvsimulator/libKyberSDK.a" \
    -output "$OUT/KyberSDK.xcframework" >/dev/null
echo "SDK Kyber pour l'Apple TV prêt dans $OUT ($(du -sh "$OUT" | cut -f1))"
