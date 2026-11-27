#!/bin/bash
# Compile le serveur de Kyber 0.29 pour Linux (KyberNow Station pour Linux,
# docs/station-linux.md), avec les correctifs KyberNow : kycontroller,
# kyavserver (moteurs txproto et libavconv, capture GStreamer/PipeWire, X11,
# DRM), kynputserver et libkynput.
#
# À lancer sur un Linux (x86_64 ou arm64 : la machine qui compile donne
# l'architecture ; la VM « KyberNow Linux » pour arm64, la même VM par
# Rosetta ou un PC pour x86_64).
#
# Mêmes correctifs que pour Windows et le Mac (server/kyber-patches), puis
# ceux propres à Linux (server/linux/kyber-patches), dans l'ordre de leur nom,
# posés par server/kyber-patches/apply.sh comme sur Mac.
#
# Sources : $KYBER_DIR (défaut : third_party/kyber-desktop-linux), une copie
# de kyber-desktop à part, comme pour le Mac (docs/build-notes.md). Les
# fichiers touchés par les correctifs sont remis à l'origine, fichier par
# fichier, avant chaque compilation ; une copie entière remise à l'origine
# (git checkout -- .) reste à la charge de l'appelant (release-station.sh).
# Une seule construction à la fois sur ces sources (scripts/verrou.sh).
# Un morceau de correctif qui ne s'applique qu'avec un décalage du contexte
# (« fuzz ») est refusé (STCOM-05 ; KYBERNOW_PATCH_STRICT=0 : signalé seulement).
#
# Sortie : $KYBER_DIR/rootfs-<arch>-linux (bin/kycontroller, bin/kyavserver,
# bin/kynputserver, lib/…), et l'empreinte des correctifs dans
# .kybernow-patches (kyber-stamp.sh), vérifiée par package.sh et run-station.sh.
#
# KYBER_PATCH_ONLY=1 : poser les correctifs, sans compiler.
#
# Prérequis (Ubuntu 24.04, Debian 13, Fedora 42 et après) : voir docs/station-linux.md,
# « Compiler ». Rust (scripts/kyber-toolchain.sh), cargo-c, meson 1.11.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
. "$ROOT/scripts/kyber-toolchain.sh"

KYBER_DIR="$(cd "${KYBER_DIR:-$ROOT/third_party/kyber-desktop-linux}" 2>/dev/null && pwd -P)" \
    || { echo "Sources de Kyber introuvables (KYBER_DIR, docs/station-linux.md)" >&2; exit 1; }
KYMEDIA="$KYBER_DIR/kysdk/kymedia"
KYCTL="$KYBER_DIR/kysdk/kyctl"
# shellcheck disable=SC2034  # lu par apply.sh
KYMUX="$KYBER_DIR/kysdk/kymux"
KYNPUT="$KYBER_DIR/kysdk/kynput"
# shellcheck disable=SC2034  # lu par apply.sh ; le moteur web se compile avec la Station Windows
KYWEB=""
ARCH="$(uname -m)"
TRIPLET="$(gcc -dumpmachine | sed 's/-pc-/-/; s/-unknown-/-/')"   # x86_64-linux-gnu, aarch64-linux-gnu
OUT="$KYBER_DIR/rootfs-$ARCH-linux"

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN="$KYBER_RUSTC"
kyber_toolchain_check rustc meson

STARTED=$(mktemp)
trap 'rm -f "$STARTED"' EXIT
# Une seule construction à la fois sur ces sources (STLIN-23).
. "$ROOT/scripts/verrou.sh"
kyber_verrou_sortie "$KYBER_DIR" "server/linux/scripts/build-kyber.sh" || exit 1
# shellcheck source=kyber-stamp.sh
. "$HERE/kyber-stamp.sh"    # lit aussi server/kyber-patches/apply.sh
# L'empreinte d'avant ne vaut plus pendant la construction.
rm -f "$OUT/.kybernow-patches"

# Correctifs communs (Windows, Mac et Linux), puis ceux de Linux
# (server/kyber-patches/apply.sh, qui lit les correctifs communs dans HERE).
# Le protocole du stylet des apps (apps/macos/kyber-patches), dont le
# correctif commun 0027 a besoin, est reposé avec tous les correctifs des apps.
(
    HERE="$ROOT/server/kyber-patches"
    kyber_apply_server_patches
    kyber_apply_own_patches "$ROOT/server/linux/kyber-patches" Linux
)

[ -n "${KYBER_PATCH_ONLY:-}" ] && { echo "==> Correctifs posés (KYBER_PATCH_ONLY)"; exit 0; }

# FFmpeg et Lua sont extraits une fois par meson, avec les fichiers de leur
# wrap de ce moment-là (0025, 0026 et les correctifs Linux y ajoutent les
# leurs ; Linux 0004 touche ceux de Lua) : extraits de nouveau quand ils
# changent (kyber_wrap_refresh, comme Windows et le Mac).
kyber_wrap_refresh "$KYMEDIA" ffmpeg builddir-linux || exit 1
kyber_wrap_refresh "$KYMEDIA" lua builddir-linux || exit 1

# Les scripts de Kyber nomment le dossier des bibliothèques de x86_64 en dur ;
# sur arm64, meson installe dans lib/aarch64-linux-gnu.
export PKG_CONFIG_PATH="$OUT/lib/pkgconfig:$OUT/lib/$TRIPLET/pkgconfig:$OUT/lib64/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export LD_LIBRARY_PATH="$OUT/lib:$OUT/lib/$TRIPLET:$OUT/lib64${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
mkdir -p "$OUT/bin" "$OUT/share"
# bindgen 0.69 (liaisons de libavconv) lit mal les structures avec un libclang
# 21 ou plus récent (Fedora 45 : clang 23) : celui de LLVM 20, 19 ou 18 s'il est
# installé (Fedora : clang20-devel ; Ubuntu 24.04 : libclang-18-dev).
if [ -z "${LIBCLANG_PATH:-}" ]; then
    for d in /usr/lib/llvm-20/lib /usr/lib64/llvm20/lib64 /usr/lib/llvm-19/lib /usr/lib64/llvm19/lib64 \
             /usr/lib/llvm-18/lib /usr/lib64/llvm18/lib64; do
        if ls "$d"/libclang.so* >/dev/null 2>&1; then export LIBCLANG_PATH="$d"; break; fi
    done
fi
[ -n "${LIBCLANG_PATH:-}" ] && echo "==> libclang : $LIBCLANG_PATH"

# Code relogeable partout : le GCC d'Ubuntu le fait par défaut (PIE), pas
# celui de Fedora ; sans lui, les objets C++ de libvpl (CMake) laissent des
# relocations en lecture seule et FFmpeg ne lie plus libavutil.so (« read-only
# segment has dynamic relocations », Fedora 45).
export CFLAGS="${CFLAGS:+$CFLAGS }-fPIC" CXXFLAGS="${CXXFLAGS:+$CXXFLAGS }-fPIC"

# PyroWave (libpyrowave-shared.so) avant FFmpeg, qui s'y lie pour son
# encodeur libpyrowave_vulkan (correctif Linux 0010, docs/pyrowave.md).
# shellcheck source=pyrowave-linux.sh
. "$HERE/pyrowave-linux.sh"
pyrowave_linux_build

echo "==> Compilation de kymedia (txproto, libavconv, FFmpeg, kyavserver)"
# Le lecteur (VLC) ne sert pas à la Station : seulement la partie serveur.
(cd "$KYMEDIA" && sed -e "s|x86_64-linux-gnu|$TRIPLET|g" \
        -e 's|"-Dbuild_server=enabled")|"-Dbuild_server=enabled" "-Dbuild_client=disabled")|' \
        build-linux.sh > build-linux-kybernow.sh \
    && chmod +x build-linux-kybernow.sh && ./build-linux-kybernow.sh -o "$OUT")
kyber_wrap_mark "$KYMEDIA" ffmpeg
kyber_wrap_mark "$KYMEDIA" lua

echo "==> Compilation de kynput (kynputserver, libkynput)"
(cd "$KYNPUT" && sed "s|x86_64-linux-gnu|$TRIPLET|g" build-linux.sh > build-linux-kybernow.sh \
    && chmod +x build-linux-kybernow.sh && ./build-linux-kybernow.sh -o "$OUT")

echo "==> Compilation de kycontroller"
(cd "$KYCTL" && cargo build -p kycontroller --features=auth-jwt,auth-oidc,cli,kyhub --release \
    && cp target/release/kycontroller target/release/jwt-gen "$OUT/bin/")

for f in bin/kycontroller bin/kyavserver bin/kynputserver; do
    [ -x "$OUT/$f" ] || { echo "$OUT/$f introuvable" >&2; exit 1; }
    [ "$OUT/$f" -nt "$STARTED" ] || { echo "$OUT/$f n'a pas été produit par ce build" >&2; exit 1; }
done
# Empreinte des correctifs de ce build (kyber-stamp.sh).
kyber_linux_patches_hash "$ROOT" > "$OUT/.kybernow-patches"
echo "==> Serveur Kyber pour Linux ($ARCH) : $OUT"
