#!/bin/bash
# Assemble le dépôt apt de KyberNow Link (Raspberry Pi OS et Debian arm64)
# depuis les .deb de apps/linux/dist, sur un Linux avec dpkg-dev et apt-utils :
#
#   apps/linux/scripts/apt-repo.sh [DOSSIER]      (par défaut apps/linux/dist/apt)
#
# Arborescence classique : pool/main/k/kybernow-link/*.deb et
# dists/stable/main/binary-arm64/Packages(.gz), dists/stable/Release ; signé
# (InRelease, Release.gpg) si KYBERNOW_APT_GPG_KEY donne l'identifiant d'une
# clé GPG présente dans le trousseau, et la clé publique exportée dans
# kybernow.gpg (format binaire, pour /usr/share/keyrings). Sans clé, le dépôt
# n'est pas signé : apt le refuse sans `[trusted=yes]`, à réserver aux essais.
#
# Le dossier se sert tel quel par un site statique ; release.sh ne l'appelle
# pas et aucun dépôt n'est publié. Côté Pi, pour un dépôt servi à
# https://www.kybernow.app/apt :
#   curl -fsSL https://www.kybernow.app/apt/kybernow.gpg | sudo tee /usr/share/keyrings/kybernow.gpg >/dev/null
#   echo "deb [signed-by=/usr/share/keyrings/kybernow.gpg] https://www.kybernow.app/apt stable main" | sudo tee /etc/apt/sources.list.d/kybernow.list
#   sudo apt update && sudo apt install kybernow-link
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$HERE/dist/apt}"
for t in dpkg-scanpackages apt-ftparchive; do
    command -v "$t" >/dev/null || { echo "$t absent (apt install dpkg-dev apt-utils)" >&2; exit 1; }
done
shopt -s nullglob
debs=("$HERE"/dist/kybernow-link_*_arm64.deb)
[ ${#debs[@]} -gt 0 ] || { echo "aucun kybernow-link_*_arm64.deb dans $HERE/dist (scripts/deb.sh)" >&2; exit 1; }
mkdir -p "$OUT/pool/main/k/kybernow-link" "$OUT/dists/stable/main/binary-arm64"
cp "${debs[@]}" "$OUT/pool/main/k/kybernow-link/"
cd "$OUT"
dpkg-scanpackages --arch arm64 pool/ > dists/stable/main/binary-arm64/Packages
gzip -9n -k -f dists/stable/main/binary-arm64/Packages
apt-ftparchive -o APT::FTPArchive::Release::Origin=KyberNow \
    -o APT::FTPArchive::Release::Label="KyberNow Link" \
    -o APT::FTPArchive::Release::Suite=stable \
    -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Architectures=arm64 \
    -o APT::FTPArchive::Release::Components=main \
    -o APT::FTPArchive::Release::Description="KyberNow Link pour Raspberry Pi OS et Debian (arm64)" \
    release dists/stable > dists/stable/Release
if [ -n "${KYBERNOW_APT_GPG_KEY:-}" ]; then
    gpg --batch --yes --default-key "$KYBERNOW_APT_GPG_KEY" --clearsign -o dists/stable/InRelease dists/stable/Release
    gpg --batch --yes --default-key "$KYBERNOW_APT_GPG_KEY" --detach-sign --armor -o dists/stable/Release.gpg dists/stable/Release
    gpg --batch --yes --export "$KYBERNOW_APT_GPG_KEY" > kybernow.gpg
    echo "dépôt signé par $KYBERNOW_APT_GPG_KEY ; clé publique : $OUT/kybernow.gpg"
else
    rm -f dists/stable/InRelease dists/stable/Release.gpg
    echo "dépôt NON signé (KYBERNOW_APT_GPG_KEY absent) : apt exige [trusted=yes]" >&2
fi
echo "$OUT : $(ls pool/main/k/kybernow-link | tr '\n' ' ')"
