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
# Mêmes correctifs que pour Windows et le Mac (server/kyber-patches, posés par
# server/kyber-patches/apply.sh), puis ceux propres à Linux
# (server/linux/kyber-patches), dans l'ordre de leur nom.
#
# Sources : $KYBER_DIR (défaut : third_party/kyber-desktop-linux), une copie
# de kyber-desktop à part, comme pour le Mac (docs/build-notes.md). Les
# fichiers touchés par les correctifs sont remis à l'origine, fichier par
# fichier, avant chaque compilation ; une copie entière remise à l'origine
# (git checkout -- .) reste à la charge de l'appelant (release-station.sh).
# Une seule construction à la fois sur ces sources (scripts/verrou.sh).
# Un morceau de correctif qui ne s'applique qu'avec un décalage du contexte
# (« fuzz ») est signalé (STCOM-05 ; KYBERNOW_PATCH_STRICT=1 : un échec).
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
. "$HERE/kyber-stamp.sh"
# L'empreinte d'avant ne vaut plus pendant la construction.
rm -f "$OUT/.kybernow-patches"

# Correctifs communs (Windows, Mac et Linux).
HERE_PATCHES="$ROOT/server/kyber-patches"
(
    HERE="$HERE_PATCHES"
    # shellcheck source=../../kyber-patches/apply.sh
    . "$HERE_PATCHES/apply.sh"
    kyber_apply_server_patches
)

# Le protocole du stylet des apps (apps/macos/kyber-patches), dont le
# correctif commun 0027 a besoin, est reposé par kyber_apply_server_patches
# avec tous les correctifs des apps (STLIN-25 : l'ancienne boucle ne faisait
# plus rien).

# Un correctif qui ne se pose qu'avec un décalage du contexte (« fuzz »)
# est signalé : il peut tomber au mauvais endroit sans échec
# (STCOM-05). KYBERNOW_PATCH_STRICT=1 en fait un échec. Pas encore d'échec
# par défaut : le correctif Mac 0009 n'a pas été rafraîchi (session des
# correctifs Kyber).
kyber_patch_fuzz_warn() {
    # $1 : le correctif, $2 : le dossier. Essai à blanc sans décalage permis
    # (-F0) : le patch d'Apple ne dit pas quand il en prend un.
    local p=$1 dir=$2
    (cd "$dir" && patch -p1 -F0 --forward --dry-run -s < "$p" >/dev/null 2>&1) && return 0
    echo "    Attention : $(basename "$p") ne se pose qu'avec un décalage du contexte (fuzz) : à rafraîchir" >&2
    [ -z "${KYBERNOW_PATCH_STRICT:-}" ]
}

# Correctifs propres à Linux : même nommage que les correctifs communs.
LINUX_PATCHES="$ROOT/server/linux/kyber-patches"
linux_repo() {
    case "$(basename "$1")" in
        *-kycontroller-*) echo "$KYCTL" ;;
        *-kynet-*) echo "$KYMUX" ;;
        *-kynput-*) echo "$KYNPUT" ;;
        *) echo "$KYMEDIA" ;;
    esac
}
shopt -s nullglob
for p in "$LINUX_PATCHES"/*.patch; do
    repo="$(linux_repo "$p")"
    for f in $(grep -h '^+++ b/' "$p" | sed 's|^+++ b/||; s|[[:space:]].*||'); do
        orig="$f"
        case "$f" in
            subprojects/txproto/*) dir="$repo/subprojects/txproto" f="${f#subprojects/txproto/}" ;;
            subprojects/libavconv/*) dir="$repo/subprojects/libavconv" f="${f#subprojects/libavconv/}" ;;
            *) dir="$repo" ;;
        esac
        # Un fichier déjà touché par un correctif commun reste tel quel.
        if ! grep -qsF "+++ b/${orig}" "$HERE_PATCHES"/*.patch; then
            if git -C "$dir" cat-file -e "HEAD:$f" 2>/dev/null; then
                git -C "$dir" checkout -q HEAD -- "$f"
            else
                rm -f "$dir/$f"
            fi
        fi
    done
done
for p in "$LINUX_PATCHES"/*.patch; do
    repo="$(linux_repo "$p")"
    echo "    $(basename "$p") (Linux)"
    kyber_patch_fuzz_warn "$p" "$repo" || exit 1
    (cd "$repo" && patch -p1 --forward --no-backup-if-mismatch -r - < "$p" | sed 's/^/        /') || {
        echo "Correctif $(basename "$p") inapplicable (hunk en échec ci-dessus)" >&2
        exit 1
    }
done
shopt -u nullglob

[ -n "${KYBER_PATCH_ONLY:-}" ] && { echo "==> Correctifs posés (KYBER_PATCH_ONLY)"; exit 0; }

# FFmpeg est extrait une fois par meson, avec les correctifs du wrap de ce
# moment-là : quand le wrap change (0025, 0026 et les correctifs Linux y
# ajoutent les leurs), on l'extrait de nouveau.
FFMPEG_STAMP="$KYMEDIA/subprojects/FFmpeg-n8.1/.kybernow-wrap"
FFMPEG_WRAP=$(cat "$KYMEDIA/subprojects/ffmpeg.wrap" $(find "$KYMEDIA/subprojects/packagefiles/ffmpeg" -type f | sort) | sha256sum | cut -d' ' -f1)
if [ -d "$KYMEDIA/subprojects/FFmpeg-n8.1" ] && [ "$(cat "$FFMPEG_STAMP" 2>/dev/null)" != "$FFMPEG_WRAP" ]; then
    echo "==> FFmpeg : wrap changé, nouvelle extraction"
    rm -rf "$KYMEDIA/subprojects/FFmpeg-n8.1" "$KYMEDIA/builddir-linux/subprojects/FFmpeg-n8.1"
fi

# Lua, de même (correctif Linux 0004 dans ses fichiers de construction).
LUA_STAMP="$KYMEDIA/subprojects/lua-5.4.6/.kybernow-wrap"
LUA_WRAP=$(cat "$KYMEDIA/subprojects/lua.wrap" $(find "$KYMEDIA/subprojects/packagefiles/lua" -type f | sort) | sha256sum | cut -d' ' -f1)
if [ -d "$KYMEDIA/subprojects/lua-5.4.6" ] && [ "$(cat "$LUA_STAMP" 2>/dev/null)" != "$LUA_WRAP" ]; then
    echo "==> Lua : fichiers de construction changés, nouvelle extraction"
    rm -rf "$KYMEDIA/subprojects/lua-5.4.6" "$KYMEDIA/builddir-linux/subprojects/lua-5.4.6"
fi

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
echo "$FFMPEG_WRAP" > "$FFMPEG_STAMP"
[ -d "$KYMEDIA/subprojects/lua-5.4.6" ] && echo "$LUA_WRAP" > "$LUA_STAMP"

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
