#!/bin/bash
# Paquets de KyberNow Station pour Linux (docs/station-linux.md) : un .deb
# (Ubuntu 24.04 et après, Debian 13) et un .rpm (Fedora 42 et après), tirés
# du même assemblage, pour l'architecture de la machine qui les fait
# (x86_64 ou arm64).
#
# À lancer sur un Linux, après server/linux/scripts/build-kyber.sh (Kyber
# dans $KYBER_DIR/rootfs-<arch>-linux) et avec l'interface d'Admin compilée
# (server/web/dist). Compile Admin et l'agent des écrans.
#
# Contenu :
#   /usr/lib/kybernow-station/bin/       kyber-admin, kybernow-display, kybernow-station-setup
#   /usr/lib/kybernow-station/runtime/   Kyber (bin, lib, share/kyber_config.toml)
#   /usr/lib/systemd/user/               kybernow-admin.service, kybernow-kyber.service
#   /usr/lib/udev/rules.d/               60-kybernow-uinput.rules
#
# Sortie : server/linux/dist/kybernow-station_<version>_<arch>.deb, et
# kybernow-station-<version>.<arch>.rpm si rpmbuild est là.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
KYBER_DIR="$(cd "${KYBER_DIR:-$ROOT/third_party/kyber-desktop-linux}" && pwd -P)"
ARCH="$(uname -m)"
RT_SRC="$KYBER_DIR/rootfs-$ARCH-linux"
TARGET="${CARGO_TARGET_DIR:-$HOME/.cache/kybernow-station-target}"
VERSION="${VERSION:-$(awk -F'"' '/^version/ {print $2; exit}' "$ROOT/server/admin/Cargo.toml")}"
DIST="${DIST:-$ROOT/server/linux/dist}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export PATH="$HOME/.cargo/bin:$PATH"

[ -x "$RT_SRC/bin/kycontroller" ] || { echo "Kyber absent : lancer d'abord server/linux/scripts/build-kyber.sh" >&2; exit 1; }
[ -f "$ROOT/server/web/dist/index.html" ] || { echo "Interface d'Admin absente : server/web, npm ci && npm run build" >&2; exit 1; }

echo "==> kyber-admin et kybernow-display ($ARCH)"
(cd "$ROOT/server/admin" && touch src/assets.rs && CARGO_TARGET_DIR="$TARGET" cargo build --release --quiet)
(cd "$ROOT/server/linux/display-agent" && CARGO_TARGET_DIR="$TARGET" cargo build --release --quiet)

echo "==> Assemblage"
TREE="$WORK/tree"
LIB="$TREE/usr/lib/kybernow-station"
install -d "$LIB/bin" "$LIB/runtime/share" "$TREE/usr/lib/systemd/user" "$TREE/usr/lib/udev/rules.d" \
    "$TREE/usr/share/doc/kybernow-station"
install -m 755 "$TARGET/release/kyber-admin" "$TARGET/release/kybernow-display" \
    "$ROOT/server/linux/packaging/kybernow-station-setup" "$LIB/bin/"
cp -a "$RT_SRC/bin" "$RT_SRC/lib" "$LIB/runtime/"
# Ce qui ne sert qu'à compiler (en-têtes, .a, pkg-config) reste dehors.
find "$LIB/runtime/lib" \( -name '*.a' -o -name '*.la' -o -name pkgconfig -o -name cmake \) -prune -exec rm -rf {} +
rm -rf "$LIB/runtime/include"
install -m 644 "$KYBER_DIR/kyber_config.toml" "$LIB/runtime/share/kyber_config.toml"
install -m 644 "$ROOT/server/linux/packaging/kybernow-admin.service" "$ROOT/server/linux/packaging/kybernow-kyber.service" \
    "$TREE/usr/lib/systemd/user/"
install -m 644 "$ROOT/server/linux/packaging/60-kybernow-uinput.rules" "$TREE/usr/lib/udev/rules.d/"
install -m 644 "$ROOT/LICENSE" "$TREE/usr/share/doc/kybernow-station/copyright" 2>/dev/null || true
SIZE_KB=$(du -sk "$TREE" | cut -f1)

POSTINST='#!/bin/sh
set -e
udevadm control --reload 2>/dev/null || true
modprobe uinput 2>/dev/null || true
udevadm trigger --name-match=uinput 2>/dev/null || true
systemctl --global enable kybernow-admin.service kybernow-kyber.service 2>/dev/null || true
echo "KyberNow Station démarrera à votre prochaine ouverture de session (ou maintenant : systemctl --user start kybernow-admin kybernow-kyber)."
'
PRERM='#!/bin/sh
set -e
systemctl --global disable kybernow-admin.service kybernow-kyber.service 2>/dev/null || true
'
# Programmes que la Station lance, et bibliothèques du système qu'elle lit.
DEB_DEPENDS="pipewire, pipewire-bin, wireplumber, pipewire-pulse | pulseaudio, pulseaudio-utils, gstreamer1.0-pipewire, gstreamer1.0-plugins-base, libgstreamer1.0-0, libgstreamer-plugins-base1.0-0, libva2, libva-drm2, libdrm2, libpulse0, libevdev2, libudev1, libxcb1, openssl, python3, systemd"
RPM_REQUIRES="pipewire, pipewire-utils, wireplumber, pipewire-pulseaudio, pulseaudio-utils, pipewire-gstreamer, gstreamer1-plugins-base, libva, libdrm, pulseaudio-libs, libevdev, systemd-libs, libxcb, openssl, python3, systemd"

mkdir -p "$DIST"

echo "==> .deb"
DEB_ARCH=$(dpkg --print-architecture 2>/dev/null || { [ "$ARCH" = x86_64 ] && echo amd64 || echo arm64; })
DEB_VERSION="${VERSION//-/\~}"
cp -a "$TREE" "$WORK/deb"
install -d "$WORK/deb/DEBIAN"
cat > "$WORK/deb/DEBIAN/control" <<EOF
Package: kybernow-station
Version: $DEB_VERSION
Architecture: $DEB_ARCH
Maintainer: Les Lignes Claires <info@leslignesclaires.fr>
Installed-Size: $SIZE_KB
Depends: $DEB_DEPENDS
Section: net
Priority: optional
Homepage: https://kybernow.app
Description: KyberNow Station : votre ordinateur sur tous vos appareils
 Fait de cet ordinateur sous Linux une Station KyberNow : son écran, son son
 et ses manettes sur vos autres appareils, avec les apps KyberNow Link.
EOF
printf '%s' "$POSTINST" > "$WORK/deb/DEBIAN/postinst"
printf '%s' "$PRERM" > "$WORK/deb/DEBIAN/prerm"
chmod 755 "$WORK/deb/DEBIAN/postinst" "$WORK/deb/DEBIAN/prerm"
DEB="$DIST/kybernow-station_${DEB_VERSION}_${DEB_ARCH}.deb"
dpkg-deb --root-owner-group -Zxz --build "$WORK/deb" "$DEB" >/dev/null
echo "    $DEB"

if command -v rpmbuild >/dev/null; then
    echo "==> .rpm"
    RPM_VERSION="${VERSION%%-*}"
    RPM_RELEASE=1
    [ "$VERSION" != "$RPM_VERSION" ] && RPM_RELEASE="0.${VERSION#*-}"
    mkdir -p "$WORK/rpm/SPECS" "$WORK/rpm/BUILDROOT"
    cat > "$WORK/rpm/SPECS/kybernow-station.spec" <<EOF
Name: kybernow-station
Version: $RPM_VERSION
Release: ${RPM_RELEASE//-/.}
Summary: KyberNow Station : votre ordinateur sur tous vos appareils
License: AGPL-3.0-or-later
URL: https://kybernow.app
Requires: $RPM_REQUIRES
AutoReqProv: no
%define _build_id_links none
%define __strip /bin/true

%description
Fait de cet ordinateur sous Linux une Station KyberNow : son écran, son son
et ses manettes sur vos autres appareils, avec les apps KyberNow Link.

%install
cp -a $TREE/. %{buildroot}/

%post
$(printf '%s' "$POSTINST" | sed 1d)

%preun
$(printf '%s' "$PRERM" | sed 1d)

%files
/usr/lib/kybernow-station
/usr/lib/systemd/user/kybernow-admin.service
/usr/lib/systemd/user/kybernow-kyber.service
/usr/lib/udev/rules.d/60-kybernow-uinput.rules
/usr/share/doc/kybernow-station
EOF
    rpmbuild --quiet --define "_topdir $WORK/rpm" -bb "$WORK/rpm/SPECS/kybernow-station.spec"
    cp "$WORK"/rpm/RPMS/*/*.rpm "$DIST/"
    echo "    $(ls "$DIST"/kybernow-station-*.rpm)"
else
    echo "    rpmbuild absent : pas de .rpm (dnf install rpm-build, ou apt install rpm)"
fi
