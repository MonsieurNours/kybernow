#!/bin/bash
# Essai hors ligne du décodeur PyroWave de l'app Linux (src/native/pyrowave.c),
# sans ordinateur distant (docs/pyrowave-client-linux.md) : une mire de ffmpeg
# (testsrc2) encodée par l'encodeur Vulkan de l'amont (encode.c, l'API C de
# libpyrowave-shared, comme la Station), puis décodée comme des paquets média
# (check.c). À lancer sous Linux après scripts/build.sh. Sans carte graphique
# (machine virtuelle : lavapipe), KYBERNOW_PYROWAVE_CPU=1 laisse le rendu
# logiciel servir, pour vérifier le décodage, pas pour mesurer.
#   apps/linux/tests/pyrowave/run.sh [LARGEUR HAUTEUR] [IMAGES]
# Sans encodeur Vulkan qui convienne (lavapipe : sous-groupes de 4), des flux
# encodés ailleurs : SAMPLES=DOSSIER (samples-mac.sh, l'encodeur Metal de l'amont
# sur le Mac) ; chaque NOM.pyrowave avec sa source NOM.yuv (« hdr » dans le nom :
# source 8 bits étiquetée HDR).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$HERE/../.."
ROOT="$APP/../.."
PREFIX="${PREFIX:-$ROOT/third_party/linux-$(uname -m)}"
W="${1:-2560}"; H="${2:-1440}"; N="${3:-120}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export LD_LIBRARY_PATH="$PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
EXTRA=()
[ "$(uname -m)" = aarch64 ] && EXTRA=(-lgcc)
cc -O2 -o "$WORK/encode" "$HERE/encode.c" -I"$PREFIX/include" -L"$PREFIX/lib" -l:libpyrowave-shared.so.1
cc -O2 -o "$WORK/check" "$HERE/check.c" "$APP/src/native/pyrowave.c" "$APP/src/native/decoder.c" -I"$PREFIX/include" \
    -DKN_PYROWAVE_DIR="\"$PREFIX/lib\"" "$PREFIX/lib/libavcodec.a" "$PREFIX/lib/libavutil.a" -lva -lva-drm -ldrm -ldl -lm -lpthread "${EXTRA[@]}"

fail=0
if [ -n "${SAMPLES:-}" ]; then
    for p in "$SAMPLES"/*.pyrowave; do
        name="$(basename "$p" .pyrowave)"
        echo "== $name"
        # Un flux HDR se compare à la mire 8 bits d'origine.
        extra=()
        [[ "$name" == *hdr ]] && extra=(src8)
        "$WORK/check" "$p" "$SAMPLES/${name%-hdr}.yuv" "${extra[@]}" || fail=1
    done
    exit $fail
fi
for f in yuv420p:420 yuv444p:444 yuv420p10le:420p10; do
    pix="${f%%:*}"; mode="${f##*:}"
    # SDR plage réduite (comme une capture BT.709), HDR10 en 10 bits (la même mire, étiquetée HDR : le chemin I010).
    ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=${W}x${H}:rate=60" -frames:v "$N" \
        -vf "scale=out_color_matrix=bt709:out_range=tv,format=$pix" -f rawvideo "$WORK/$pix.yuv"
    "$WORK/encode" "$WORK/$pix.yuv" "$WORK/$pix.pw" "$W" "$H" "$mode" 1.2 1
    "$WORK/check" "$WORK/$pix.pw" "$WORK/$pix.yuv" || fail=1
done
exit $fail
