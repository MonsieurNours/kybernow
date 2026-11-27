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
# Dans flatpak-builder avec --ccache (flatpak.sh), /run/ccache survit d'une construction à l'autre : les crates
# téléchargées y restent au lieu d'être reprises à chaque fois (LINAPP-38).
if [ -n "${FLATPAK_ID:-}" ] && [ -d /run/ccache ] && [ -w /run/ccache ]; then
    export CARGO_HOME=/run/ccache/kybernow-cargo
    mkdir -p "$CARGO_HOME"
fi

cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Installing) ' || true; }
}

if [ "$MEDIA_ONLY" = 0 ]; then
# 1. Correctifs. Sources de git (arbre de construction, copie de travail) :
# remis à l'origine puis tous reposés dans l'ordre, Station d'abord
# (scripts/kyber-client-patches.sh), comme les autres apps ; un correctif
# posé puis recouvert par un suivant (0022 puis 0025) n'était ni reconnu ni
# applicable tel quel (« 0022 inapplicable »). Sans git (sources du Flatpak,
# déjà prises à l'origine) : un à un, ceux déjà posés passés.
PATCHES=()
for patch in "$APPS"/macos/kyber-patches/*.patch "$ROOT"/server/kyber-patches/*-kyclient-*.patch "$APPS"/webos/kyber-patches/*.patch; do
    case "$(basename "$patch")" in
        *-kyclient-pyrowave.patch) [ "$KYBER_PYROWAVE" = 1 ] || continue ;;
    esac
    PATCHES+=("$patch")
done
if [ -f "$ROOT/scripts/kyber-client-patches.sh" ] && git -C "$KYSDK/kyctl" rev-parse -q --verify HEAD >/dev/null 2>&1; then
    . "$ROOT/scripts/kyber-client-patches.sh"
    kyber_apply_client_patches "$KYSDK" "${PATCHES[@]}" || exit 1
else
    # Ceux de la Station pour kyctl, kymux et kynput d'abord, comme
    # kyber_station_patches : le canal de retour du client (0051-kyclient)
    # s'appuie sur celui de kymux (0050-kynet).
    STATION=()
    for patch in "$ROOT"/server/kyber-patches/*.patch; do
        case "$(basename "$patch")" in
            *-kycontroller-* | *-kywebplayer-* | *-kynet-* | *-kynput-*) STATION+=("$patch") ;;
        esac
    done
    for patch in ${STATION[@]+"${STATION[@]}"} "${PATCHES[@]}"; do
        case "$(basename "$patch")" in
            *-kyclient-* | *-kycontroller-* | *-kywebplayer-*) repo="$KYSDK/kyctl" ;;
            *-kynput-*) repo="$KYSDK/kynput" ;;
            *-kynet-*) repo="$KYSDK/kymux" ;;
            *) echo "Dépôt inconnu pour $(basename "$patch")" >&2; exit 1 ;;
        esac
        # Déjà appliqué : rien à faire.
        if (cd "$repo" && patch -p1 -R --dry-run -s -f < "$patch" >/dev/null 2>&1); then
            continue
        fi
        if (cd "$repo" && patch -p1 --dry-run -s -f -F0 < "$patch" >/dev/null 2>&1); then
            (cd "$repo" && patch -p1 -s -f -F0 < "$patch") && echo "==> $(basename "$patch") appliqué"
        else
            echo "Correctif $(basename "$patch") inapplicable" >&2
            exit 1
        fi
    done
fi

# kynput en client seul, comme sur webOS et Android.
export RUSTFLAGS="${RUSTFLAGS:-} --cfg kybernow_webos"

# 2. libkynput, refaite quand les sources de kynput changent
# (scripts/kyber-client-patches.sh, kyber_kynput_stamp). Dans la copie du
# Flatpak (apps/linux/scripts/sources.sh), ni ce fichier ni l'historique git de
# kynput : son dossier de construction part de zéro, libkynput y est faite
# quand elle manque, comme avant.
KYNPUT_STAMP=""
if [ -f "$ROOT/scripts/kyber-client-patches.sh" ] && git -C "$KYSDK/kynput" rev-parse -q --verify HEAD >/dev/null 2>&1; then
    . "$ROOT/scripts/kyber-client-patches.sh"
    KYNPUT_STAMP="$(kyber_kynput_stamp "$KYSDK")"
fi
kynput_current() {
    if [ -n "$KYNPUT_STAMP" ]; then kyber_kynput_current "$PREFIX" "$KYNPUT_STAMP"; else [ -f "$PREFIX/lib/pkgconfig/kynput.pc" ]; fi
}
if ! kynput_current; then
    command -v cargo-cinstall >/dev/null || cargo install cargo-c --version 0.10.15 --locked
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
#    FFMPEG_FEATURES nomme ces ajouts : un FFmpeg construit sans eux est refait.
FFMPEG_VERSION=7.1.1
FFMPEG_FEATURES=camera-1
[ -f "$PREFIX/lib/libavcodec.a" ] && [ "$(cat "$PREFIX/lib/.kybernow-ffmpeg" 2>/dev/null)" != "$FFMPEG_FEATURES" ] && rm -f "$PREFIX/lib/libavcodec.a"
RPI_FFMPEG_BRANCH="dev/7.1.1/rpi_import_1"
# La branche du FFmpeg de Raspberry Pi bouge : figée sur un commit (audit TRANS-21).
RPI_FFMPEG_COMMIT=6dbf87aefd7f491210abe1e043a1c228fa1439a0
# Des sources téléchargées : l'archive dans un fichier, extraite dans un dossier
# à part, renommé seulement une fois complète, puis un marqueur. Une coupure ne
# laisse jamais un `configure` sur des sources incomplètes (audit TRANS-21).
# Un dossier donné par FFMPEG_SRC est celui du développeur : jamais effacé,
# jamais rempli (audit LINAPP-28).
fetch_sources() { # dossier marqueur url compression
    local dir="$1" mark="$2" url="$3" z="$4" part="$1.part" arc="$1.archive"
    [ -f "$dir/$mark" ] && return 0
    rm -rf "$part" "$dir" && mkdir -p "$part"
    curl -fsSL --retry 5 --retry-all-errors --retry-delay 10 -o "$arc" "$url"
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
        fetch_sources "$src" .kybernow-complet "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz" J
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
    echo "$FFMPEG_FEATURES" > "$PREFIX/lib/.kybernow-ffmpeg"
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
#    ailleurs) ; PYROWAVE_PATCHES nomme cette correction, une bibliothèque qui
#    ne l'a pas est refaite.
PYROWAVE_COMMIT=a32b05e03d70826b19d4d11ff74fe166ed74371b
PYROWAVE_PATCHES=v3dv-dequant-1
if [ ! -f "$PREFIX/lib/libpyrowave-shared.so.1" ] || [ ! -f "$PREFIX/lib/.pyrowave-$PYROWAVE_PATCHES" ]; then
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
    rm -f "$PREFIX"/lib/.pyrowave-* && touch "$PREFIX/lib/.pyrowave-$PYROWAVE_PATCHES"
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
