#!/bin/bash
# Compile le cœur KyberNow et son pont C (apps/core, apps/apple) pour l'Apple
# TV ou l'iPhone et l'iPad, appareil et simulateur, en bibliothèques
# statiques réunies dans un xcframework : voir docs/coeur-apple.md.
#
#   apps/apple/scripts/build-apple-mobile.sh tvos    → apps/tvos/Vendor
#   apps/apple/scripts/build-apple-mobile.sh ios     → apps/ios/Vendor
#
#   Vendor/KyberNow.xcframework                   libkybernow.a par plateforme
#   Vendor/KyberNow/include/kybernow/kybernow.h   en-tête (cbindgen)
#   Vendor/KyberNow/include/module.modulemap      module Swift CKyberNow
#
# Le pont appelle l'API C de libkynput (chaîne d'entrées) et libopus, et
# kyclient référence VLC (jamais appelé). Apple TV : les trois viennent du SDK
# de l'Apple TV (Vendor/KyberSDK.xcframework, apps/tvos/scripts/build-kyber-tvos.sh).
# iOS : libkynput vient du rootfs que apps/core/scripts/build-apple.sh
# construit, libopus est compilée ici (cmake), les fonctions vides de VLC
# aussi, et tout va dans la même bibliothèque ; l'en-tête de libkynput va dans
# Vendor/include (module CKyber).
#
# Cibles de rang 3 pour tvOS : Rust nightly et -Zbuild-std ; iOS : stable.
# S'arrête à la première erreur sans toucher à Vendor/. Déploiement 18.0.
set -euo pipefail

PLATFORM="${1:-}"
case "$PLATFORM" in
    tvos) TARGETS=(aarch64-apple-tvos aarch64-apple-tvos-sim) ;;
    ios) TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim) ;;
    *) echo "Usage : $0 tvos|ios" >&2; exit 1 ;;
esac

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APPS="$ROOT/apps"
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
KYBER="$THIRD_PARTY/kyber-desktop"
OPUS_SRC="$KYBER/kysdk/kymedia/subprojects/opus-1.6.1"
VENDOR="$APPS/$PLATFORM/Vendor"
DEPLOYMENT=18.0
. "$ROOT/scripts/kyber-toolchain.sh"

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export LIBCLANG_PATH="${LIBCLANG_PATH:-/Library/Developer/CommandLineTools/usr/lib}"
export PKG_CONFIG_ALLOW_CROSS=1
# Le « strip » de cargo laisse une table de symboles que l'éditeur de liens
# d'Xcode refuse (« mis-aligned LINKEDIT string pool »).
export CARGO_PROFILE_RELEASE_STRIP=false

cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing) ' || true; }
}

needs_nightly() { case "$1" in aarch64-apple-tvos*) return 0 ;; *) return 1 ;; esac; }

kyber_toolchain_check rustc cargo-c
if [ "$PLATFORM" = tvos ]; then
    rustup run nightly rustc --version >/dev/null 2>&1 || {
        echo "Rust nightly manquant : rustup toolchain install nightly --profile minimal --component rust-src" >&2
        exit 1
    }
    kyber_toolchain_check nightly
fi

for target in "${TARGETS[@]}"; do
    [ -f "$KYBER/rootfs-$target/lib/pkgconfig/kynput.pc" ] || {
        echo "libkynput ($target) introuvable : lancez apps/core/scripts/build-apple.sh --target $target" >&2
        exit 1
    }
done

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/kybernow-mobile.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT

LIBS=()
for target in "${TARGETS[@]}"; do
    prefix="$KYBER/rootfs-$target"
    export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
    # Une seule cible de déploiement dans l'environnement : le clang de
    # bindgen refuse d'en voir deux.
    unset IPHONEOS_DEPLOYMENT_TARGET TVOS_DEPLOYMENT_TARGET
    case "$target" in
        aarch64-apple-ios*) export IPHONEOS_DEPLOYMENT_TARGET=$DEPLOYMENT ;;
        aarch64-apple-tvos*) export TVOS_DEPLOYMENT_TARGET=$DEPLOYMENT ;;
    esac
    out="$STAGE/$target"
    echo "==> kybernow-core + kybernow-apple ($target)"
    if needs_nightly "$target"; then
        (cd "$APPS" && cargo_quiet cargo +nightly cinstall -p kybernow-apple --target "$target" -Zbuild-std=std,panic_abort \
            --release --library-type staticlib --prefix="$out")
    else
        (cd "$APPS" && cargo_quiet cargo cinstall -p kybernow-apple --target "$target" \
            --release --library-type staticlib --prefix="$out")
    fi
    lib="$out/lib/libkybernow.a"
    [ -f "$lib" ] || { echo "libkybernow.a absente pour $target" >&2; exit 1; }
    # iOS : pas de SDK Kyber à part. libkynput, libopus et les fonctions vides
    # de VLC vont avec le cœur, dans la même bibliothèque.
    if [ "$PLATFORM" = ios ]; then
        case "$target" in
            *-sim) sdk=iphonesimulator; triple="arm64-apple-ios$DEPLOYMENT-simulator" ;;
            *) sdk=iphoneos; triple="arm64-apple-ios$DEPLOYMENT" ;;
        esac
        if [ ! -f "$prefix/lib/libopus.a" ]; then
            echo "==> libopus ($target)"
            cmake -S "$OPUS_SRC" -B "$STAGE/opus-$target" -G Ninja -DCMAKE_BUILD_TYPE=Release \
                -DCMAKE_SYSTEM_NAME=iOS -DCMAKE_OSX_SYSROOT="$sdk" -DCMAKE_OSX_ARCHITECTURES=arm64 \
                -DCMAKE_OSX_DEPLOYMENT_TARGET=$DEPLOYMENT -DCMAKE_INSTALL_PREFIX="$prefix" \
                -DOPUS_BUILD_PROGRAMS=OFF -DOPUS_BUILD_TESTING=OFF -DOPUS_BUILD_SHARED_LIBRARY=OFF \
                -DOPUS_DRED=OFF -DOPUS_OSCE=OFF >/dev/null
            cmake --build "$STAGE/opus-$target" --target install >/dev/null
            [ -f "$prefix/lib/libopus.a" ] || { echo "libopus ($target) non construite" >&2; exit 1; }
        fi
        # Fonctions de libvlc que kyvlcplayer référence : jamais appelées (l'image
        # et le son vont à l'app), elles rendent 0 / NULL.
        stubs="$STAGE/vlc-stubs-$target.c"
        {
            echo "// Généré par build-apple-mobile.sh : VLC n'est jamais appelé sur iOS."
            nm -u "$lib" 2>/dev/null | awk '/^_libvlc_/ {print substr($1, 2)}' | sort -u \
                | while read -r fn; do echo "void *$fn(void) { return 0; }"; done
        } > "$stubs"
        xcrun --sdk "$sdk" clang -target "$triple" -O2 -c "$stubs" -o "$STAGE/vlc-stubs-$target.o"
        xcrun libtool -static -no_warning_for_no_symbols -o "$out/lib/libkybernow-all.a" \
            "$lib" "$prefix/lib/libkynput.a" "$prefix/lib/libopus.a" "$STAGE/vlc-stubs-$target.o"
        mv "$out/lib/libkybernow-all.a" "$lib"
    fi
    LIBS+=(-library "$lib" -headers "$out/include")
done

NEW="$STAGE/vendor"
mkdir -p "$NEW/KyberNow/include"
xcodebuild -create-xcframework "${LIBS[@]}" -output "$NEW/KyberNow.xcframework" >/dev/null
cp -R "$STAGE/${TARGETS[0]}/include/kybernow" "$NEW/KyberNow/include/"
# Bibliothèque statique : liée par la dépendance au xcframework, pas par le module.
cat > "$NEW/KyberNow/include/module.modulemap" <<'EOF'
module CKyberNow {
    header "kybernow/kybernow.h"
    export *
}
EOF

mkdir -p "$VENDOR"
rm -rf "$VENDOR/KyberNow.xcframework" "$VENDOR/KyberNow"
mv "$NEW/KyberNow.xcframework" "$NEW/KyberNow" "$VENDOR/"
# iOS : l'en-tête de libkynput (mise en page de la vue, constantes des
# manettes) pour le module CKyber, que l'Apple TV reçoit de son SDK.
if [ "$PLATFORM" = ios ]; then
    rm -rf "$VENDOR/include"
    mkdir -p "$VENDOR/include"
    cp "$KYBER/rootfs-${TARGETS[0]}/include/kynput/kynput.h" "$VENDOR/include/"
    cat > "$VENDOR/include/module.modulemap" <<'EOF'
module CKyber {
    header "kynput.h"
    export *
}
EOF
fi
echo "KyberNow.xcframework et kybernow.h dans $VENDOR"
for target in "${TARGETS[@]}"; do
    echo "   $target : $(du -h "$STAGE/$target/lib/libkybernow.a" | cut -f1)"
done
