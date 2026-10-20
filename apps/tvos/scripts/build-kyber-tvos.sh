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
#   rustup toolchain install nightly --profile minimal --component rust-src
#
# Sortie : Vendor/include (en-têtes, module.modulemap) et
# Vendor/KyberSDK.xcframework (une bibliothèque statique par plateforme).
# S'arrête à la première erreur (correctif inapplicable, compilation ratée)
# sans toucher à Vendor/. Versions vérifiées : scripts/kyber-toolchain.sh (une
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

[ -d "$KYSDK/kyctl" ] || { echo "SDK Kyber introuvable dans $KYSDK (voir docs/build-notes.md)" >&2; exit 1; }
[ -d "$OPUS_SRC" ] || { echo "Sources d'Opus introuvables : $OPUS_SRC" >&2; exit 1; }

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
rustup run nightly rustc --version >/dev/null 2>&1 || {
    echo "Rust nightly manquant : rustup toolchain install nightly --profile minimal --component rust-src" >&2
    exit 1
}
kyber_toolchain_check nightly cargo-c

# Sortie de cargo sans les lignes de progression ; avec pipefail, le code de
# retour reste celui de cargo.
cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating) ' || true; }
}

# Mêmes correctifs que l'app Mac (image et son vers l'app, presse-papiers,
# mémoire de réception UDP).
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
    (cd "$KYSDK/kynput" && cargo_quiet cargo +nightly cinstall -p kynput --target="$target" -Zbuild-std=std,panic_abort \
        --release --library-type=staticlib --prefix="$prefix")
    echo "==> libkyclient ($target)"
    (cd "$KYSDK/kyctl" && cargo_quiet cargo +nightly cinstall -p kyclient --target="$target" -Zbuild-std=std,panic_abort \
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
rm -rf "$OUT" && mkdir -p "$OUT/include/opus"
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
