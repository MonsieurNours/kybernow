#!/bin/bash
# Le dépôt JSON de Raspberry Pi Imager (format V4, Imager 2.0) qui liste
# « KyberNow Link OS » : à publier sur le site (site/updates/rpi-imager.json,
# https://www.kybernow.app/updates/rpi-imager.json), et à coller dans
# Imager › Réglages › Dépôt personnalisé. L'image elle-même est celle de la
# version GitHub (releases/download/<tag>/KyberNow-Link-OS.img.xz).
#
#   apps/linux/os/imager-json.sh VERSION INFO SORTIE.json
#
# INFO : le fichier KyberNow-Link-OS.img.xz.info écrit par release-image.sh
# (download_size, download_sha256, extract_size, extract_sha256).
set -euo pipefail
VERSION="${1:?version}"; INFO="${2:?fichier .info}"; OUT="${3:?sortie .json}"
[ -f "$INFO" ] || { echo "info absent : $INFO" >&2; exit 1; }
# shellcheck source=/dev/null
. "$INFO"
: "${download_size:?}" "${download_sha256:?}" "${extract_size:?}" "${extract_sha256:?}"
DATE="$(date -u +%Y-%m-%d)"
URL="https://github.com/MonsieurNours/kybernow/releases/download/v$VERSION/KyberNow-Link-OS.img.xz"
mkdir -p "$(dirname "$OUT")"
cat > "$OUT" <<EOF
{
  "os_list": [
    {
      "name": "KyberNow Link OS $VERSION",
      "description": "Votre ordinateur sur votre Raspberry Pi : KyberNow Link seul, au téléviseur à la manette ou au bureau. Pi 3, 4 et 5 (64 bits). · Your computer on your Raspberry Pi: KyberNow Link alone, on a TV with a controller or at a desk.",
      "url": "$URL",
      "icon": "https://www.kybernow.app/apple-touch-icon.png",
      "website": "https://www.kybernow.app",
      "release_date": "$DATE",
      "image_download_size": $download_size,
      "image_download_sha256": "$download_sha256",
      "extract_size": $extract_size,
      "extract_sha256": "$extract_sha256",
      "init_format": "none",
      "devices": ["pi5-64bit", "pi4-64bit", "pi3-64bit"]
    }
  ]
}
EOF
echo "$OUT"
