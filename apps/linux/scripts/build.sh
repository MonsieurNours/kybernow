#!/bin/bash
# Compile KyberNow Link pour Linux, sur un Linux (x86_64 ou aarch64) ou dans
# le SDK de freedesktop (flatpak-builder, voir flatpak.sh) :
#
#   1. applique les correctifs client de Kyber (apps/macos/kyber-patches) et
#      celui de webOS qui fait de kynput un client seul (cfg kybernow_webos :
#      sans l'injection d'entrées du serveur, libinput, libevdev, X11) ;
#   2. libkynput (statique, cargo cinstall) dans le préfixe ;
#   3. FFmpeg 7.1.1 réduit au décodage H.264, HEVC et AV1, VA-API et V4L2 M2M
#      compris (statique, LGPL 2.1) dans le préfixe ;
#   4. PyroWave (Vulkan, API C, bibliothèque partagée ; aussi avec --rpi, pour
#      le Pi 4 et le Pi 5 en Vulkan par Mesa v3dv : docs/pyrowave-client-pi.md) ;
#   5. l'app (cargo build -p kybernow-linux).
#
# Usage : apps/linux/scripts/build.sh [--check] [--debug] [--deps-only] [--media-only] [--rpi] [--kyber-pyrowave | --sans-pyrowave] [--prefix DIR] [--install DIR]
#   --kyber-pyrowave  le correctif kyclient de PyroWave (server/kyber-patches/0025,
#             `VideoCodec::PyroWave`, `fallback_codec`) et la fonctionnalité
#             `pyrowave` du cœur, qui vont ensemble (apps/core/README.md,
#             « PyroWave ») : l'app demande PyroWave à Kyber. **Par défaut**
#             (KYBER_PYROWAVE=1) ; il reste accepté pour les scripts d'avant.
#   --sans-pyrowave   sans eux : l'app décode PyroWave mais ne le demande jamais.
#   --deps-only  s'arrête après libkynput, FFmpeg et PyroWave (module du Flatpak mis en cache)
#   --media-only FFmpeg et PyroWave seuls, sans le SDK de Kyber (premier module du Flatpak :
#                ils ne sont plus refaits quand Kyber ou build.sh changent ailleurs, LINAPP-38)
#   --rpi     Raspberry Pi : le FFmpeg de Raspberry Pi (jc-kynesim/rpi-ffmpeg,
#             branche dev/7.1.1/rpi_import_1) à la place du 7.1.1 officiel, avec
#             le hwaccel HEVC V4L2 « request » (rpivid, Pi 4 et 5), les images
#             SAND et libudev ; préfixe third_party/linux-<arch>-rpi ; l'app est
#             compilée avec KYBERNOW_V4L2_REQUEST=1 (build.rs). Le décodeur H.264
#             V4L2 M2M (bcm2835-codec, Pi 3 et 4) est dans tous les FFmpeg Linux.
#   --prefix  où poser libkynput et FFmpeg (par défaut third_party/linux-<arch>)
#   --install où copier l'app (bin/kybernow), sinon elle reste dans apps/target
#
# Dépendances (Ubuntu/Debian) : build-essential pkg-config nasm clang libclang-dev cmake git libvulkan-dev
# libsdl2-dev libva-dev libdrm-dev libopus-dev libwayland-dev libegl-dev libgles-dev
# (et libudev-dev pour --rpi),
# Rust 1.89 ou après et cargo-c. Dans le SDK de freedesktop : extensions rust-stable et llvm.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
KYSDK="$ROOT/third_party/kyber-desktop/kysdk"
ARCH="$(uname -m)"
PREFIX=""
COMMAND=build
PROFILE=release
INSTALL=""
DEPS_ONLY=0
MEDIA_ONLY=0
RPI=0
KYBER_PYROWAVE="${KYBER_PYROWAVE:-1}"
while [ $# -gt 0 ]; do
    case "$1" in
        --check) COMMAND=check ;;
        --debug) PROFILE=debug ;;
        --deps-only) DEPS_ONLY=1 ;;
        --media-only) MEDIA_ONLY=1 ;;
        --rpi) RPI=1 ;;
        --kyber-pyrowave) KYBER_PYROWAVE=1 ;;
        --sans-pyrowave) KYBER_PYROWAVE=0 ;;
        --prefix) PREFIX="$2"; shift ;;
        --install) INSTALL="$2"; shift ;;
        *) echo "Option inconnue : $1" >&2; exit 1 ;;
    esac
    shift
done
[ -n "$PREFIX" ] || PREFIX="$ROOT/third_party/linux-$ARCH$([ "$RPI" = 1 ] && echo -rpi || true)"
[ "$MEDIA_ONLY" = 1 ] || [ -d "$KYSDK/kyctl" ] || { echo "SDK Kyber introuvable dans $KYSDK (voir docs/build-notes.md)" >&2; exit 1; }
mkdir -p "$PREFIX"
PREFIX="$(cd "$PREFIX" && pwd)"
JOBS="$(nproc 2>/dev/null || echo 4)"
export PATH="$HOME/.cargo/bin:$PATH"
# Versions (cargo-c, FFmpeg, commit de PyroWave) et cargo_quiet : en un seul
# endroit pour tout KyberNow (sources.sh le copie dans les sources du Flatpak).
. "$ROOT/scripts/kyber-toolchain.sh"
# Dans flatpak-builder avec --ccache (flatpak.sh), /run/ccache survit d'une construction à l'autre : les crates
# téléchargées y restent au lieu d'être reprises à chaque fois (LINAPP-38).
if [ -n "${FLATPAK_ID:-}" ] && [ -d /run/ccache ] && [ -w /run/ccache ]; then
    export CARGO_HOME=/run/ccache/kybernow-cargo
    mkdir -p "$CARGO_HOME"
fi

if [ "$MEDIA_ONLY" = 0 ]; then
# 1. Correctifs, comme les autres apps (scripts/kyber-client-patches.sh), plus
# celui de webOS (kynput en client seul). Sources de git (arbre de
# construction, copie de travail) : remis à l'origine puis tous reposés dans
# l'ordre, Station d'abord. Sans git (sources du Flatpak, sources.sh) : un à
# un, ceux déjà posés passés.
. "$ROOT/scripts/kyber-client-patches.sh"
kyber_apply_app_patches "$KYSDK" "$APPS"/webos/kyber-patches/*.patch || exit 1

# kynput en client seul, comme sur webOS et Android.
export RUSTFLAGS="${RUSTFLAGS:-} --cfg kybernow_webos"

# 2. libkynput, refaite quand les sources de kynput changent
# (scripts/kyber-client-patches.sh, kyber_kynput_stamp). Dans la copie du
# Flatpak (apps/linux/scripts/sources.sh), pas d'historique git de kynput :
# son dossier de construction part de zéro, libkynput y est faite quand elle
# manque.
KYNPUT_STAMP=""
if git -C "$KYSDK/kynput" rev-parse -q --verify HEAD >/dev/null 2>&1; then
    KYNPUT_STAMP="$(kyber_kynput_stamp "$KYSDK")"
fi
kynput_current() {
    if [ -n "$KYNPUT_STAMP" ]; then kyber_kynput_current "$PREFIX" "$KYNPUT_STAMP"; else [ -f "$PREFIX/lib/pkgconfig/kynput.pc" ]; fi
}
if ! kynput_current; then
    command -v cargo-cinstall >/dev/null || cargo install cargo-c --version "$KYBER_CARGO_C" --locked
    echo "==> libkynput dans $PREFIX"
    (cd "$KYSDK/kynput" && cargo_quiet cargo cinstall -p kynput --release --library-type=staticlib --prefix="$PREFIX" --libdir="$PREFIX/lib" --target-dir="$PREFIX/build/kynput")
    [ -f "$PREFIX/lib/pkgconfig/kynput.pc" ] || { echo "libkynput non installée dans $PREFIX" >&2; exit 1; }
    [ -z "$KYNPUT_STAMP" ] || kyber_kynput_mark "$PREFIX" "$KYNPUT_STAMP"
fi
fi # MEDIA_ONLY

# 3. FFmpeg réduit au décodage (VA-API ; V4L2 M2M partout sur Linux ; avec --rpi,
#    le FFmpeg de Raspberry Pi et son hwaccel HEVC V4L2 request), plus ce que
#    demande la caméra de l'appareil (docs/webcam.md) : les encodeurs H.264
#    matériels (VA-API, V4L2 M2M) et le décodeur MJPEG des webcams USB.
#    Le marqueur nomme la version (KYBER_FFMPEG, scripts/kyber-toolchain.sh ; le
#    commit du FFmpeg de Raspberry Pi avec --rpi) et ces ajouts
#    (FFMPEG_FEATURES) : un FFmpeg construit autrement est refait (TRANS-19).
FFMPEG_VERSION=$KYBER_FFMPEG
FFMPEG_FEATURES=camera-1
RPI_FFMPEG_BRANCH="dev/7.1.1/rpi_import_1"
# La branche du FFmpeg de Raspberry Pi bouge : figée sur un commit (audit TRANS-21).
RPI_FFMPEG_COMMIT=6dbf87aefd7f491210abe1e043a1c228fa1439a0
if [ "$RPI" = 1 ]; then FFMPEG_MARK="rpi-$RPI_FFMPEG_COMMIT $FFMPEG_FEATURES"; else FFMPEG_MARK="$FFMPEG_VERSION $FFMPEG_FEATURES"; fi
[ -f "$PREFIX/lib/libavcodec.a" ] && [ "$(cat "$PREFIX/lib/.kybernow-ffmpeg" 2>/dev/null)" != "$FFMPEG_MARK" ] && rm -f "$PREFIX/lib/libavcodec.a"
# Des sources téléchargées : l'archive dans un fichier, extraite dans un dossier
# à part, renommé seulement une fois complète, puis un marqueur. Une coupure ne
# laisse jamais un `configure` sur des sources incomplètes (audit TRANS-21).
# Un dossier donné par FFMPEG_SRC est celui du développeur : jamais effacé,
# jamais rempli (audit LINAPP-28).
fetch_sources() { # dossier marqueur url compression [sha256 de l'archive]
    local dir="$1" mark="$2" url="$3" z="$4" sum="${5:-}" part="$1.part" arc="$1.archive"
    [ -f "$dir/$mark" ] && return 0
    rm -rf "$part" "$dir" && mkdir -p "$part"
    curl -fsSL --retry 5 --retry-all-errors --retry-delay 10 -o "$arc" "$url"
    if [ -n "$sum" ] && [ "$(kyber_sha256 < "$arc")" != "$sum" ]; then
        rm -f "$arc"
        echo "$url : empreinte SHA-256 inattendue (scripts/kyber-toolchain.sh)" >&2
        exit 1
    fi
    tar "x${z}f" "$arc" -C "$part" --strip-components=1
    rm -f "$arc"
    touch "$part/$mark"
    mv "$part" "$dir"
}
if [ ! -f "$PREFIX/lib/libavcodec.a" ]; then
    if [ -n "${FFMPEG_SRC:-}" ]; then
        src="$FFMPEG_SRC"
        [ -f "$src/configure" ] || { echo "FFMPEG_SRC=$src n'a pas de configure (dossier laissé tel quel)" >&2; exit 1; }
    elif [ "$RPI" = 1 ]; then
        src="$PREFIX/build/rpi-ffmpeg"
        mkdir -p "$PREFIX/build"
        [ -f "$src/.kybernow-$RPI_FFMPEG_COMMIT" ] || echo "==> sources du FFmpeg de Raspberry Pi ($RPI_FFMPEG_BRANCH, $RPI_FFMPEG_COMMIT)"
        fetch_sources "$src" ".kybernow-$RPI_FFMPEG_COMMIT" "https://github.com/jc-kynesim/rpi-ffmpeg/archive/$RPI_FFMPEG_COMMIT.tar.gz" z
    else
        src="$PREFIX/build/ffmpeg-$FFMPEG_VERSION"
        mkdir -p "$PREFIX/build"
        [ -f "$src/.kybernow-complet" ] || echo "==> sources de FFmpeg $FFMPEG_VERSION"
        # Avec réessais : le site de FFmpeg coupe parfois la connexion en plein téléchargement.
        fetch_sources "$src" .kybernow-complet "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz" J "$KYBER_FFMPEG_SHA256"
    fi
    if [ "$RPI" = 1 ]; then
        extra=(--enable-v4l2-request --enable-libudev --enable-sand --enable-hwaccel=hevc_v4l2request --extra-version=kybernow-rpi)
        what="décodeurs H.264, HEVC, AV1, VA-API, V4L2 M2M et HEVC V4L2 request du Pi"
    else
        extra=()
        what="décodeurs H.264, HEVC, AV1, VA-API, V4L2 M2M"
    fi
    echo "==> libavcodec ($what) dans $PREFIX"
    build="$PREFIX/build/ffmpeg-build"
    rm -rf "$build" && mkdir -p "$build"
    if ! (cd "$build" && "$src/configure" --prefix="$PREFIX" --libdir="$PREFIX/lib" \
        --disable-everything --disable-programs --disable-doc --disable-network --disable-autodetect \
        --enable-static --disable-shared --enable-pic --disable-debug \
        --enable-avcodec --enable-avutil --disable-avformat --disable-avfilter --disable-avdevice --disable-swresample --disable-swscale \
        --enable-vaapi --enable-libdrm --enable-v4l2-m2m \
        --enable-decoder=h264 --enable-decoder=hevc --enable-decoder=av1 --enable-decoder=h264_v4l2m2m \
        --enable-hwaccel=h264_vaapi --enable-hwaccel=hevc_vaapi --enable-hwaccel=av1_vaapi \
        --enable-parser=h264 --enable-parser=hevc --enable-parser=av1 \
        --enable-encoder=h264_vaapi --enable-encoder=h264_v4l2m2m --enable-decoder=mjpeg "${extra[@]}" > "$build/configure.log" 2>&1); then
        tail -30 "$build/configure.log" >&2
        [ -f "$build/ffbuild/config.log" ] && tail -40 "$build/ffbuild/config.log" >&2
        echo "configure de FFmpeg en échec (journal : $build/configure.log)" >&2
        exit 1
    fi
    (cd "$build" && make -j"$JOBS" >/dev/null && make install >/dev/null)
    [ -f "$PREFIX/lib/libavcodec.a" ] || { echo "libavcodec non construite" >&2; exit 1; }
    echo "$FFMPEG_MARK" > "$PREFIX/lib/.kybernow-ffmpeg"
fi

# 4. PyroWave (docs/pyrowave-client-linux.md) : l'API C de l'amont, chemin
#    Vulkan, au commit de la Station (apps/macos/PyroWave/COMMIT), avec la
#    partie de Granite que prend checkout_granite.sh ; bibliothèque partagée
#    (libpyrowave-shared.so.1), chargée par l'app au lancement (dlopen) : sans
#    elle, PyroWave est indisponible et rien d'autre ne change. Aussi pour le
#    Raspberry Pi (--rpi : le .deb et KyberNow Link OS), où le pilote Vulkan
#    est Mesa v3dv (docs/pyrowave-client-pi.md) ; c'est l'app qui décide, au
#    lancement, si la carte sait le décoder. Le déquantificateur y est corrigé
#    (pyrowave/v3dv-dequant.py : image en r16f, que v3dv exige ; sans effet
#    ailleurs) ; PYROWAVE_PATCHES nomme cette correction. Le marqueur nomme le
#    commit et la correction : une bibliothèque d'un autre commit, ou sans la
#    correction, est refaite (TRANS-19).
PYROWAVE_COMMIT=$(kyber_pyrowave_commit) || exit 1
PYROWAVE_PATCHES=v3dv-dequant-1
PYROWAVE_MARK=".pyrowave-${PYROWAVE_COMMIT:0:12}-$PYROWAVE_PATCHES"
if [ ! -f "$PREFIX/lib/libpyrowave-shared.so.1" ] || [ ! -f "$PREFIX/lib/$PYROWAVE_MARK" ]; then
    if [ -n "${PYROWAVE_SRC:-}" ]; then
        # Le clone du développeur : jamais effacé (audit LINAPP-28).
        src="$PYROWAVE_SRC"
        [ -f "$src/pyrowave.h" ] && [ -d "$src/Granite" ] || { echo "PYROWAVE_SRC=$src n'a pas pyrowave.h et Granite (dossier laissé tel quel)" >&2; exit 1; }
    else
        src="$PREFIX/build/pyrowave"
    fi
    if [ -z "${PYROWAVE_SRC:-}" ] && [ ! -f "$src/.kybernow-$PYROWAVE_COMMIT" ]; then
        echo "==> sources de PyroWave ($PYROWAVE_COMMIT) et de Granite"
        rm -rf "$src.part" "$src" && mkdir -p "$src.part"
        (cd "$src.part" && git init -q && git remote add origin https://github.com/Themaister/pyrowave \
            && git fetch -q --depth 1 origin "$PYROWAVE_COMMIT" && git checkout -q FETCH_HEAD \
            && ./checkout_granite.sh >/dev/null 2>&1) || { echo "sources de PyroWave non obtenues" >&2; exit 1; }
        touch "$src.part/.kybernow-$PYROWAVE_COMMIT"
        mv "$src.part" "$src"
    fi
    python3 "$HERE/pyrowave/v3dv-dequant.py" "$src" || exit 1
    echo "==> PyroWave dans $PREFIX"
    build="$PREFIX/build/pyrowave-build"
    rm -rf "$build" && mkdir -p "$build"
    if ! (cd "$build" && cmake -S "$src" -B . -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$PREFIX" \
        -DCMAKE_INSTALL_LIBDIR=lib > "$build/cmake.log" 2>&1 \
        && cmake --build . --target pyrowave-shared -j"$JOBS" >> "$build/cmake.log" 2>&1); then
        tail -40 "$build/cmake.log" >&2
        echo "PyroWave non construit (journal : $build/cmake.log)" >&2
        exit 1
    fi
    install -Dm644 "$src/pyrowave.h" "$PREFIX/include/pyrowave/pyrowave.h"
    # Les en-têtes de Vulkan de Granite (ceux avec lesquels PyroWave est compilé).
    cp -r "$src/Granite/third_party/khronos/vulkan-headers/include/vulkan" "$src/Granite/third_party/khronos/vulkan-headers/include/vk_video" "$PREFIX/include/"
    install -Dm755 "$(readlink -f "$build/libpyrowave-shared.so.1")" "$PREFIX/lib/libpyrowave-shared.so.1"
    [ -f "$PREFIX/lib/libpyrowave-shared.so.1" ] || { echo "libpyrowave-shared.so.1 absente" >&2; exit 1; }
    rm -f "$PREFIX"/lib/.pyrowave-* && touch "$PREFIX/lib/$PYROWAVE_MARK"
fi
[ "$MEDIA_ONLY" = 1 ] && { echo "==> FFmpeg et PyroWave prêts dans $PREFIX"; exit 0; }

# VLC : jamais appelé (fonctions vides, build.rs).
if [ ! -f "$PREFIX/lib/pkgconfig/libvlc.pc" ]; then
    cat > "$PREFIX/lib/pkgconfig/libvlc.pc" <<'PC'
Name: libvlc
Description: VLC absent (fonctions vides)
Version: 4.0.0
Libs:
Cflags:
PC
fi

[ "$DEPS_ONLY" = 1 ] && { echo "==> dépendances prêtes dans $PREFIX"; exit 0; }

# 5. L'app.
export KYBERNOW_PREFIX="$PREFIX"
export KYBERNOW_V4L2_REQUEST="$RPI"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export CARGO_PROFILE_RELEASE_STRIP=false
flags=()
[ "$PROFILE" = release ] && flags+=(--release)
[ "$KYBER_PYROWAVE" = 1 ] && flags+=(--features pyrowave)
echo "==> cargo $COMMAND kybernow-linux ($ARCH, $PROFILE$([ "$KYBER_PYROWAVE" = 1 ] && echo ', PyroWave demandé à Kyber' || true))"
started=$(date +%s)
(cd "$APPS" && cargo_quiet cargo "$COMMAND" -p kybernow-linux "${flags[@]}")
[ "$COMMAND" = build ] || { echo "    vérifié ($(( $(date +%s) - started )) s)"; exit 0; }
bin="${CARGO_TARGET_DIR:-$APPS/target}/$PROFILE/kybernow"
[ -f "$bin" ] || { echo "$bin absent" >&2; exit 1; }
echo "    $bin ($(du -h "$bin" | cut -f1), $(( $(date +%s) - started )) s)"
if [ -n "$INSTALL" ]; then
    install -Dm755 "$bin" "$INSTALL/bin/kybernow"
    echo "    → $INSTALL/bin/kybernow"
    # PyroWave à côté de l'app : <préfixe>/lib, où l'app le cherche d'abord.
    if [ -f "$PREFIX/lib/libpyrowave-shared.so.1" ]; then
        install -Dm755 "$PREFIX/lib/libpyrowave-shared.so.1" "$INSTALL/lib/libpyrowave-shared.so.1"
        echo "    → $INSTALL/lib/libpyrowave-shared.so.1"
    fi
fi
