#!/bin/bash
# Vérification rapide (quelques secondes) de l'édition de liens de l'app
# Windows : un petit programme lié aux mêmes bibliothèques statiques que
# kybernow.exe (FFmpeg et libopus du rootfs, dans l'ordre et avec les
# conditions d'apps/windows/build.rs). Un symbole manquant (l'encodeur
# PyroWave de la libavcodec du serveur sans sa bibliothèque d'import, par
# exemple) s'y voit tout de suite, avec son nom, au lieu d'arrêter cargo au
# bout de plusieurs minutes.
#
#   apps/windows/scripts/link-check.sh [--arch x64|arm64] [--rootfs DOSSIER]
#
# Par défaut, le rootfs que lit build.sh (third_party/rootfs-win64 pour x64,
# third_party/rootfs-winarm64 pour ARM64). Lu par scripts/release.sh.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable" >&2; exit 1; }
. "$ROOT/scripts/kyber-toolchain.sh"
ARCH=x64 ROOTFS=""
while [ $# -gt 0 ]; do
    case "$1" in
        --arch) ARCH="${2:?}"; shift ;;
        --rootfs) ROOTFS="${2:?}"; shift ;;
        *) echo "Option inconnue : $1" >&2; exit 2 ;;
    esac
    shift
done
case "$ARCH" in
    x64) CC=x86_64-w64-mingw32-gcc; ROOTFS="${ROOTFS:-$THIRD_PARTY/rootfs-win64}" ;;
    arm64)
        LLVM_MINGW="$THIRD_PARTY/toolchains/llvm-mingw-$KYBER_LLVM_MINGW-ucrt-macos-universal"
        CC="$LLVM_MINGW/bin/aarch64-w64-mingw32-clang"; ROOTFS="${ROOTFS:-$THIRD_PARTY/rootfs-winarm64}" ;;
    *) echo "Architecture inconnue : $ARCH" >&2; exit 2 ;;
esac
command -v "$CC" >/dev/null || [ -x "$CC" ] || { echo "$CC introuvable" >&2; exit 1; }
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# Les fonctions qu'appelle l'app : avcodec_find_decoder tire la liste de tous
# les codecs de la libavcodec (encodeurs du serveur compris).
cat > "$WORK/t.c" <<'EOF'
void *avcodec_find_decoder(int);
void *avcodec_alloc_context3(const void *);
void *av_frame_alloc(void);
void *swr_alloc(void);
void *opus_decoder_create(int, int, int *);
int main(void) {
    int e;
    return avcodec_find_decoder(27) != 0 && avcodec_alloc_context3(0) && av_frame_alloc() && swr_alloc() && opus_decoder_create(48000, 2, &e);
}
EOF
LIBS=()
for lib in avcodec swresample avutil opus x264 vpl z; do
    [ -f "$ROOTFS/lib/lib$lib.a" ] && LIBS+=("$ROOTFS/lib/lib$lib.a")
done
[ -f "$ROOTFS/lib/libavcodec.a" ] || { echo "$ROOTFS/lib/libavcodec.a absent" >&2; exit 1; }
[ -f "$ROOTFS/lib/libpyrowave-shared.dll.a" ] && LIBS+=("$ROOTFS/lib/libpyrowave-shared.dll.a")
[ -f "$ROOTFS/lib/libvpl.a" ] && LIBS+=(-lstdc++)
SYS=(-lmfuuid -lole32 -loleaut32 -lstrmiids -luser32 -lgdi32 -luuid -lbcrypt -lws2_32 -lsecur32 -ladvapi32
    -lshell32 -lcrypt32 -lpsapi -lshlwapi -ld3d11 -ldxgi -lmfplat -lmf -lvfw32)
if ! "$CC" -o "$WORK/t.exe" "$WORK/t.c" "${LIBS[@]}" "${SYS[@]}" > "$WORK/ld.log" 2>&1; then
    echo "App Windows $ARCH : édition de liens impossible avec $ROOTFS :" >&2
    grep -E "undefined reference|undefined symbol|cannot find|error" "$WORK/ld.log" | sed 's/^.*: //' | sort -u | head -15 >&2
    exit 1
fi
echo "app Windows $ARCH : édition de liens vérifiée ($(basename "$ROOTFS"), ${#LIBS[@]} bibliothèques)"
