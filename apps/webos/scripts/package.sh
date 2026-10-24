#!/bin/bash
# Paquet .ipk de l'app webOS, après scripts/build.sh : le binaire (sans les
# informations de débogage), la SDL2 de webOS dans lib/, les images et
# appinfo.json. Sortie : apps/webos/dist/fr.leslignesclaires.kybernow_<version>_arm.ipk
#
#   apps/webos/scripts/package.sh [release|debug]
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
PROFILE="${1:-release}"
TARGET=armv7-unknown-linux-gnueabi
THIRD_PARTY="$(cd "$ROOT/third_party" && pwd -P)"
# La chaîne de scripts/build.sh (qui la passe par WEBOS_TOOLCHAIN) ; lancé
# seul, la même selon la machine : Linux x86_64 ou Mac Apple Silicon.
if [ -z "${WEBOS_TOOLCHAIN:-}" ]; then
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64) WEBOS_TOOLCHAIN="$THIRD_PARTY/webos-toolchain/arm-webos-linux-gnueabi_sdk-buildroot" ;;
        Darwin-arm64) WEBOS_TOOLCHAIN="$THIRD_PARTY/webos-toolchain-mac/arm-webos-linux-gnueabi_sdk-buildroot" ;;
        *) echo "Chaîne webOS : posez WEBOS_TOOLCHAIN (github.com/webosbrew/native-toolchain)" >&2; exit 1 ;;
    esac
fi
SDL_PREFIX="${SDL_PREFIX:-$THIRD_PARTY/sdl-webos-dist}"
BIN="$APPS/target/$TARGET/$PROFILE/kybernow-webos"
VERSION="$(awk -F'"' '/^version *=/ {print $2; exit}' "$APPS/Cargo.toml")"
APP_ID=fr.leslignesclaires.kybernow

[ -f "$BIN" ] || { echo "$BIN absent : lancez scripts/build.sh" >&2; exit 1; }
[ -x "$WEBOS_TOOLCHAIN/bin/arm-webos-linux-gnueabi-strip" ] || { echo "Chaîne webOS absente de $WEBOS_TOOLCHAIN : lancez scripts/build.sh" >&2; exit 1; }
[ -f "$SDL_PREFIX/lib/libSDL2-2.0.so.0" ] || { echo "SDL2 de webOS absente de $SDL_PREFIX : lancez scripts/build.sh" >&2; exit 1; }

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/kybernow-ipk.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/bin" "$STAGE/lib" "$STAGE/licenses"
"$WEBOS_TOOLCHAIN/bin/arm-webos-linux-gnueabi-strip" --strip-debug -o "$STAGE/bin/kybernow-webos" "$BIN"
chmod 755 "$STAGE/bin/kybernow-webos"
cp -L "$SDL_PREFIX/lib/libSDL2-2.0.so.0" "$STAGE/lib/libSDL2-2.0.so.0"
"$WEBOS_TOOLCHAIN/bin/arm-webos-linux-gnueabi-strip" --strip-unneeded "$STAGE/lib/libSDL2-2.0.so.0"
cp "$HERE/webos/icon.png" "$HERE/webos/icon-large.png" "$HERE/webos/splash.png" "$STAGE/"
sed "s/@VERSION@/$VERSION/" "$HERE/webos/appinfo.json" > "$STAGE/appinfo.json"
cp "$ROOT/brand/v2/fonts/OFL-Archivo.txt" "$ROOT/brand/v2/fonts/OFL-MartianMono.txt" "$STAGE/licenses/"
[ -f "$THIRD_PARTY/sdl-webos/LICENSE.txt" ] && cp "$THIRD_PARTY/sdl-webos/LICENSE.txt" "$STAGE/licenses/SDL2.txt"
[ -f "$THIRD_PARTY/ffmpeg-7.1.1/COPYING.LGPLv2.1" ] && cp "$THIRD_PARTY/ffmpeg-7.1.1/COPYING.LGPLv2.1" "$STAGE/licenses/FFmpeg-LGPL-2.1.txt"
[ -f "$ROOT/LICENSE" ] && cp "$ROOT/LICENSE" "$STAGE/licenses/KyberNow-AGPL-3.0.txt"

mkdir -p "$HERE/dist"
python3 "$HERE/scripts/ipk.py" "$STAGE" "$VERSION" "$HERE/dist/${APP_ID}_${VERSION}_arm.ipk"
