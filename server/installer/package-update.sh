#!/bin/bash
# Paquet de mise à jour d'un ordinateur déjà installé : surcouche Kyber,
# console, icône et update.ps1 (installé par la console en un clic, ou à la
# main). À lancer après build.sh. Sortie : dist/kybernow-update.zip,
# dist/update.ps1 et dist/SHA256SUMS (pour essayer la mise à jour à la main
# depuis le Mac, voir docs/releases.md ; release.sh recalcule les siennes).
#
#   package-update.sh [--min-installer A.B.C] [--requires-setup]
#
# --requires-setup : cette mise à jour exige l'installateur (pilotes, services
# à recréer…) : la console refuse de l'installer en un clic et renvoie vers
# KyberNow-Setup.exe. --min-installer : version minimale de KyberNow déjà
# installée pour accepter le paquet. release.sh passe les mêmes options à
# scripts/sign-manifest.sh, dont le manifeste signé fait foi ; le paquet en
# garde une copie (requirements.json) pour la mise à jour à la main.
set -euo pipefail
MIN_INSTALLER="" REQUIRES_SETUP=false
while [ $# -gt 0 ]; do
    case "$1" in
        --min-installer) MIN_INSTALLER="${2:?--min-installer attend une version X.Y.Z}"; shift ;;
        --min-installer=*) MIN_INSTALLER="${1#*=}" ;;
        --requires-setup) REQUIRES_SETUP=true ;;
        *) echo "usage : package-update.sh [--min-installer A.B.C] [--requires-setup]" >&2; exit 1 ;;
    esac
    shift
done
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DIST="$HERE/dist"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

[ -d "$ROOT/third_party/payload/kyber-overlay" ] || { echo "Surcouche Kyber absente : lancez server/kyber-patches/build.sh" >&2; exit 1; }
mkdir -p "$STAGE"/{kyber,admin,tray} "$DIST"
# -R : la surcouche a des sous-dossiers (webclient/, moteur du client web).
cp -R "$ROOT/third_party/payload/kyber-overlay/." "$STAGE/kyber/"
cp "$ROOT/server/dist/kyber-admin.exe" "$STAGE/admin/"
cp "$ROOT/server/tray/target/x86_64-pc-windows-gnu/release/kybernow-tray.exe" "$STAGE/tray/"
cp "$(ls -d "$HOME"/.cargo/registry/src/index.crates.io-*/webview2-com-sys-*/x64/WebView2Loader.dll | tail -1)" "$STAGE/tray/"
cp "$HERE/update.ps1" "$STAGE/"
# Version du paquet, lue par update.ps1 (kybernow.json, Applications installées).
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/server/admin/Cargo.toml" | head -1)"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Version de server/admin/Cargo.toml illisible : $VERSION" >&2; exit 1; }
printf '%s\r\n' "$VERSION" > "$STAGE/version.txt"
# Sans --min-installer : 0.2.0, première console qui se met à jour, même
# défaut que scripts/sign-manifest.sh.
MIN_INSTALLER="${MIN_INSTALLER:-0.2.0}"
[[ "$MIN_INSTALLER" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "--min-installer : version X.Y.Z attendue ($MIN_INSTALLER)" >&2; exit 1; }
num() { local a b c; IFS=. read -r a b c <<< "$1"; echo $((a * 1000000 + b * 1000 + c)); }
[ "$(num "$MIN_INSTALLER")" -le "$(num "$VERSION")" ] || { echo "--min-installer $MIN_INSTALLER est plus récent que le paquet $VERSION" >&2; exit 1; }
# Mêmes champs que kybernow-release.json (scripts/sign-manifest.sh), qui fait
# foi ; cette copie sert au chemin à la main (update.ps1), sans manifeste.
printf '{"version": "%s", "min_installer_version": "%s", "requires_setup": %s}\r\n' \
    "$VERSION" "$MIN_INSTALLER" "$REQUIRES_SETUP" > "$STAGE/requirements.json"
# PowerShell 5 lit les scripts en ANSI : update.ps1 doit rester en ASCII.
if LC_ALL=C grep -q '[^ -~[:space:]]' "$HERE/update.ps1"; then echo "update.ps1 contient des caractères non ASCII" >&2; exit 1; fi
rm -f "$DIST/kybernow-update.zip"
(cd "$STAGE" && zip -qr "$DIST/kybernow-update.zip" kyber admin tray update.ps1 version.txt requirements.json)
cp "$HERE/update.ps1" "$DIST/update.ps1"
(cd "$DIST" && shasum -a 256 kybernow-update.zip update.ps1 > SHA256SUMS)
cp "$ROOT/server/dist/kyber-admin.exe" "$DIST/"
echo "Paquet $VERSION : $DIST/kybernow-update.zip ($(du -h "$DIST/kybernow-update.zip" | cut -f1)), installateur minimal $MIN_INSTALLER, installateur exigé : $REQUIRES_SETUP"
