#!/bin/bash
# Paquet de mise à jour d'un ordinateur déjà installé : surcouche Kyber,
# console, icône et update.ps1 (installé par la console en un clic, ou à la
# main). À lancer après build.sh. Sortie : dist/kybernow-update.zip et
# dist/update.ps1.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DIST="$HERE/dist"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

[ -d "$ROOT/third_party/payload/kyber-overlay" ] || { echo "Surcouche Kyber absente : lancez server/kyber-patches/build.sh" >&2; exit 1; }
mkdir -p "$STAGE"/{kyber,admin,tray} "$DIST"
cp "$ROOT/third_party/payload/kyber-overlay/"* "$STAGE/kyber/"
cp "$ROOT/server/dist/kyber-admin.exe" "$STAGE/admin/"
cp "$ROOT/server/tray/target/x86_64-pc-windows-gnu/release/kybernow-tray.exe" "$STAGE/tray/"
cp "$HERE/update.ps1" "$STAGE/"
# PowerShell 5 lit les scripts en ANSI : update.ps1 doit rester en ASCII.
if LC_ALL=C grep -q '[^ -~[:space:]]' "$HERE/update.ps1"; then echo "update.ps1 contient des caractères non ASCII" >&2; exit 1; fi
rm -f "$DIST/kybernow-update.zip"
(cd "$STAGE" && zip -qr "$DIST/kybernow-update.zip" kyber admin tray update.ps1)
cp "$HERE/update.ps1" "$DIST/update.ps1"
cp "$ROOT/server/dist/kyber-admin.exe" "$DIST/"
echo "Paquet : $DIST/kybernow-update.zip ($(du -h "$DIST/kybernow-update.zip" | cut -f1))"
