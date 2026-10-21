#!/bin/bash
# Installateur par utilisateur de l'app Windows, depuis le dossier construit
# par build.sh : apps/windows/dist/KyberNow-Windows-<arch>.exe (NSIS de
# Homebrew, `brew install makensis`).
#
#   apps/windows/scripts/build-installer.sh              x64
#   apps/windows/scripts/build-installer.sh --arch arm64
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ARCH=x64
while [ $# -gt 0 ]; do
    case "$1" in
        --arch) ARCH="${2:-}"; shift ;;
        *) echo "Option inconnue : $1" >&2; exit 1 ;;
    esac
    shift
done
case "$ARCH" in x64|arm64) ;; *) echo "Architecture inconnue : $ARCH" >&2; exit 1 ;; esac

SRC="$HERE/dist/$ARCH"
[ -f "$SRC/kybernow.exe" ] || { echo "$SRC/kybernow.exe absent : lancez d'abord scripts/build.sh --arch $ARCH" >&2; exit 1; }
command -v makensis >/dev/null || { echo "makensis absent : brew install makensis" >&2; exit 1; }
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$HERE/../Cargo.toml" | head -1)
OUT="$HERE/dist/KyberNow-Windows-$ARCH.exe"

echo "==> Installateur $ARCH ($VERSION)"
LC_ALL=en_US.UTF-8 makensis -V2 \
    -DVERSION="$VERSION" -DARCH="$ARCH" -DSRC="$SRC" -DRES="$HERE/resources" -DOUT="$OUT" \
    "$HERE/installer/kybernow.nsi"
ls -la "$OUT"
