#!/bin/bash
# Construit le paquet .deb de KyberNow Link pour Raspberry Pi OS (Debian 13
# « trixie », arm64), sur un Linux arm64 (la VM « KyberNow Linux », ou un Pi) :
#
#   apps/linux/scripts/deb.sh [--check] [--no-build] [--sans-pyrowave] [--prefix DIR]
#
#   PyroWave est demandé à Kyber par défaut (build.sh) ; --sans-pyrowave, passé
#             à build.sh, s'en passe (le Pi 5 le décode alors sans le demander
#             jamais) ; --kyber-pyrowave reste accepté (c'est le défaut)
#   --prefix  préfixe des dépendances passé à build.sh (par défaut
#             third_party/linux-<arch>-rpi) ; avec --no-build, où prendre
#             libpyrowave-shared.so.1
#
# → apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb (nom publié avec chaque
#   version) et apps/linux/dist/kybernow-link_<version>_arm64.deb (le même, au
#   nom que veut un dépôt apt : scripts/apt-repo.sh).
#
# L'app est compilée par build.sh --rpi (FFmpeg de Raspberry Pi, décodeur HEVC
# V4L2 request) ; les dépendances du paquet sont lues sur le binaire
# (dpkg-shlibdeps : libsdl2, libopus, libva, libdrm, libudev…), pour
# les noms de paquets de Debian. PyroWave (/usr/lib/libpyrowave-shared.so.1,
# chargé au lancement) ouvre Vulkan lui-même (volk) : libvulkan1 et le pilote
# Mesa du Pi (mesa-vulkan-drivers, v3dv) sont donc recommandés, pas lus sur le
# binaire (docs/pyrowave-client-pi.md). Le paquet s'installe par
# `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`.
#
# Réglages : KYBERNOW_DEB_MAINTAINER (« Nom <adresse> »), KYBERNOW_DEB_VERSION
# (sinon la version du workspace, apps/Cargo.toml).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$APPS/.." && pwd)"
CHECK=0 BUILD=1 BUILD_ARGS=() PREFIX="$ROOT/third_party/linux-$(uname -m)-rpi"
while [ $# -gt 0 ]; do
    case "$1" in
        --check) CHECK=1 ;;
        --no-build) BUILD=0 ;;
        --prefix) BUILD_ARGS+=(--prefix "$2"); PREFIX="$2"; shift ;;
        --kyber-pyrowave) BUILD_ARGS+=(--kyber-pyrowave) ;;
        --sans-pyrowave) BUILD_ARGS+=(--sans-pyrowave) ;;
        *) echo "option inconnue : $1" >&2; exit 2 ;;
    esac
    shift
done
for t in dpkg-deb dpkg-shlibdeps fakeroot; do
    command -v "$t" >/dev/null || { echo "$t absent (apt install dpkg-dev fakeroot)" >&2; exit 1; }
done
DEB_ARCH="$(dpkg --print-architecture)"
[ "$DEB_ARCH" = arm64 ] || echo "attention : architecture $DEB_ARCH (le paquet publié est arm64)" >&2
VERSION="${KYBERNOW_DEB_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$APPS/Cargo.toml" | head -1)}"
[ -n "$VERSION" ] || { echo "version introuvable dans $APPS/Cargo.toml" >&2; exit 1; }
MAINTAINER="${KYBERNOW_DEB_MAINTAINER:-Les Lignes Claires <https://www.leslignesclaires.fr>}"
[ "$CHECK" = 1 ] && { echo "deb.sh : prêt (version $VERSION, $DEB_ARCH)"; exit 0; }

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/usr/bin"
if [ "$BUILD" = 1 ]; then
    "$HERE/scripts/build.sh" --rpi ${BUILD_ARGS[@]+"${BUILD_ARGS[@]}"} --install "$STAGE/usr"
else
    bin="${CARGO_TARGET_DIR:-$APPS/target}/release/kybernow"
    [ -f "$bin" ] || { echo "$bin absent : lancer sans --no-build" >&2; exit 1; }
    install -Dm755 "$bin" "$STAGE/usr/bin/kybernow"
    # PyroWave du préfixe, comme build.sh --install.
    [ ! -f "$PREFIX/lib/libpyrowave-shared.so.1" ] || install -Dm755 "$PREFIX/lib/libpyrowave-shared.so.1" "$STAGE/usr/lib/libpyrowave-shared.so.1"
fi
strip --strip-unneeded "$STAGE/usr/bin/kybernow" 2>/dev/null || true
PYROWAVE_LIB=""
if [ -f "$STAGE/usr/lib/libpyrowave-shared.so.1" ]; then
    PYROWAVE_LIB=usr/lib/libpyrowave-shared.so.1
    chmod 644 "$STAGE/$PYROWAVE_LIB"
    strip --strip-unneeded "$STAGE/$PYROWAVE_LIB" 2>/dev/null || true
else
    echo "attention : paquet sans PyroWave (libpyrowave-shared.so.1 absente)" >&2
fi

# Lanceur, fiche AppStream, icônes, licences (les mêmes que le Flatpak).
install -Dm644 "$HERE/flatpak/fr.leslignesclaires.kybernow.desktop" "$STAGE/usr/share/applications/fr.leslignesclaires.kybernow.desktop"
install -Dm644 "$HERE/flatpak/fr.leslignesclaires.kybernow.metainfo.xml" "$STAGE/usr/share/metainfo/fr.leslignesclaires.kybernow.metainfo.xml"
install -Dm644 "$ROOT/brand/v2/app-icon/kybernow-link-1024.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/fr.leslignesclaires.kybernow.svg"
install -Dm644 "$HERE/flatpak/icon-256.png" "$STAGE/usr/share/icons/hicolor/256x256/apps/fr.leslignesclaires.kybernow.png"
install -Dm644 "$ROOT/third_party/kyber-desktop/COPYING.AGPLv3" "$STAGE/usr/share/doc/kybernow-link/COPYING.AGPLv3"
install -Dm644 "$HERE/flatpak/LICENSES.md" "$STAGE/usr/share/doc/kybernow-link/LICENSES.md"
cat > "$STAGE/usr/share/doc/kybernow-link/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: KyberNow Link
Source: https://github.com/MonsieurNours/kybernow

Files: *
Copyright: Les Lignes Claires
License: AGPL-3.0
 Voir COPYING.AGPLv3 et LICENSES.md dans ce dossier (Kyber, FFmpeg, SDL, polices).
EOF
gzip -9n -c "$ROOT/CHANGELOG.md" > "$STAGE/usr/share/doc/kybernow-link/changelog.gz"

# Dépendances, lues sur le binaire.
mkdir -p "$STAGE/DEBIAN" "$STAGE/debian"
printf 'Source: kybernow-link\nPackage: kybernow-link\nArchitecture: any\n' > "$STAGE/debian/control"
DEPENDS="$(cd "$STAGE" && dpkg-shlibdeps -O -e usr/bin/kybernow ${PYROWAVE_LIB:+-e $PYROWAVE_LIB} 2>/dev/null | sed -n 's/^shlibs:Depends=//p')"
rm -rf "$STAGE/debian"
[ -n "$DEPENDS" ] || { echo "dpkg-shlibdeps n'a rien trouvé" >&2; exit 1; }
SIZE_KB="$(du -sk "$STAGE/usr" | cut -f1)"
cat > "$STAGE/DEBIAN/control" <<EOF
Package: kybernow-link
Version: $VERSION
Section: games
Priority: optional
Architecture: $DEB_ARCH
Installed-Size: $SIZE_KB
Depends: $DEPENDS
Recommends: pipewire, bluez${PYROWAVE_LIB:+, libvulkan1, mesa-vulkan-drivers}
Maintainer: $MAINTAINER
Homepage: https://www.kybernow.app
Description: KyberNow Link, votre ordinateur sur tous vos appareils
 L'app KyberNow Link affiche l'image de l'ordinateur Windows équipé de
 KyberNow Station (jeu et bureau à distance, basse latence, Kyber/QUIC).
 Sur le Raspberry Pi, le HEVC est décodé par la puce (rpivid, Pi 4 et 5) et
 le H.264 par bcm2835-codec (Pi 3 et 4), et PyroWave en Vulkan (Mesa v3dv)
 sur les cartes qui le décodent ; interface Bureau dans une fenêtre, ou Salon
 sur un téléviseur, à la manette.
EOF
mkdir -p "$HERE/dist"
OUT="$HERE/dist/kybernow-link_${VERSION}_${DEB_ARCH}.deb"
fakeroot dpkg-deb --build --root-owner-group -Zxz "$STAGE" "$OUT" >/dev/null
cp "$OUT" "$HERE/dist/KyberNow-Link-RaspberryPi-$DEB_ARCH.deb"
echo "$OUT ($(du -h "$OUT" | cut -f1))"
echo "$HERE/dist/KyberNow-Link-RaspberryPi-$DEB_ARCH.deb"
dpkg-deb --info "$OUT" | sed -n '/Depends/p;/Recommends/p'
[ -z "$PYROWAVE_LIB" ] || dpkg-deb -c "$OUT" | grep libpyrowave-shared
