#!/bin/bash
# Paquets de KyberNow Station pour Linux (docs/station-linux.md) : un .deb
# (Ubuntu 24.04 et après, Debian 13) et un .rpm (Fedora 42 et après), tirés
# du même assemblage, pour l'architecture de la machine qui les fait
# (x86_64 ou arm64).
#
# À lancer sur un Linux, après server/linux/scripts/build-kyber.sh (Kyber
# dans $KYBER_DIR/rootfs-<arch>-linux) et avec l'interface d'Admin compilée
# (server/web/dist). Compile Admin, l'agent des écrans et l'icône (outils de
# compilation de GTK 3 : libgtk-3-dev, ou gtk3-devel sous Fedora).
#
# Contenu :
#   /usr/lib/kybernow-station/bin/       kyber-admin, kybernow-display, kybernow-tray, kybernow-station-setup
#   /usr/lib/kybernow-station/runtime/   Kyber (bin, lib, share/kyber_config.toml)
#   /usr/lib/systemd/user/               kybernow-admin.service, kybernow-kyber.service, kybernow-tray.service
#   /usr/lib/systemd/system/             kybernow-station-update.socket et kybernow-station-update@.service
#                                        (service root des mises à jour, demandées par Admin)
#   /usr/lib/udev/rules.d/               60-kybernow-uinput.rules (uinput, uhid, hidraw des DualSense)
#   /usr/lib/modules-load.d/             kybernow-station.conf (uinput, uhid au démarrage)
#   /usr/share/applications/             fr.leslignesclaires.kybernow.station.display.desktop (KDE)
#
# Sortie : server/linux/dist/kybernow-station_<version>_<arch>.deb,
# kybernow-station-<version>.<arch>.rpm si rpmbuild est là, et l'archive des
# systèmes immuables KyberNow-Station-Linux-<arch>.tar.xz
# (server/linux/immutable/make-archive.sh).
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

echo "==> kyber-admin, kybernow-display et kybernow-tray ($ARCH)"
(cd "$ROOT/server/admin" && touch src/assets.rs && CARGO_TARGET_DIR="$TARGET" cargo build --release --quiet)
(cd "$ROOT/server/linux/display-agent" && CARGO_TARGET_DIR="$TARGET" cargo build --release --quiet)
(cd "$ROOT/server/tray" && CARGO_TARGET_DIR="$TARGET" cargo build --release --quiet)

echo "==> Assemblage"
TREE="$WORK/tree"
LIB="$TREE/usr/lib/kybernow-station"
install -d "$LIB/bin" "$LIB/runtime/share" "$TREE/usr/lib/systemd/user" "$TREE/usr/lib/systemd/system" "$TREE/usr/lib/udev/rules.d" "$TREE/usr/lib/modules-load.d" \
    "$TREE/usr/share/doc/kybernow-station" "$TREE/usr/share/applications"
install -m 755 "$TARGET/release/kyber-admin" "$TARGET/release/kybernow-display" "$TARGET/release/kybernow-tray" \
    "$ROOT/server/linux/packaging/kybernow-station-setup" "$LIB/bin/"
# L'outil par lequel Admin essaie l'encodeur (docs/profil-auto.md), contre le
# FFmpeg du runtime de Kyber (en-têtes encore là : retirés plus bas).
"$ROOT/server/encoder-check/build.sh" linux "$RT_SRC" "$LIB/bin/kybernow-encoder-check"
cp -a "$RT_SRC/bin" "$RT_SRC/lib" "$LIB/runtime/"
# La caméra de l'appareil (docs/webcam.md) : l'outil que kycontroller lance
# depuis son dossier (correctif 0085), v4l2loopback ou PipeWire.
"$ROOT/server/camera/build.sh" linux "$RT_SRC" "$LIB/runtime/bin/kybernow-camera"
# Fedora range les bibliothèques dans lib64, Debian et Ubuntu dans lib/<triplet>.
[ -d "$RT_SRC/lib64" ] && cp -a "$RT_SRC/lib64" "$LIB/runtime/"
# Ce qui ne sert qu'à compiler (en-têtes, .a, pkg-config) reste dehors.
find "$LIB/runtime/lib" $([ -d "$LIB/runtime/lib64" ] && echo "$LIB/runtime/lib64") \( -name '*.a' -o -name '*.la' -o -name pkgconfig -o -name cmake \) -prune -exec rm -rf {} +
rm -rf "$LIB/runtime/include"
install -m 644 "$KYBER_DIR/kyber_config.toml" "$LIB/runtime/share/kyber_config.toml"
install -m 644 "$ROOT/server/linux/packaging/kybernow-admin.service" "$ROOT/server/linux/packaging/kybernow-kyber.service" \
    "$ROOT/server/linux/packaging/kybernow-tray.service" "$TREE/usr/lib/systemd/user/"
install -m 644 "$ROOT/server/linux/packaging/kybernow-station-update.socket" "$ROOT/server/linux/packaging/kybernow-station-update@.service" \
    "$TREE/usr/lib/systemd/system/"
install -m 644 "$ROOT/server/linux/packaging/60-kybernow-uinput.rules" "$TREE/usr/lib/udev/rules.d/"
install -m 644 "$ROOT/server/linux/packaging/kybernow-station-modules.conf" "$TREE/usr/lib/modules-load.d/kybernow-station.conf"
# Caméra : v4l2loopback (s'il est installé) chargé au démarrage avec le nom
# « KyberNow Camera », et ouvert à la session de l'utilisateur.
install -d "$TREE/usr/lib/modprobe.d"
install -m 644 "$ROOT/server/camera/linux/modules-load-kybernow-camera.conf" "$TREE/usr/lib/modules-load.d/kybernow-camera.conf"
install -m 644 "$ROOT/server/camera/linux/modprobe-kybernow-camera.conf" "$TREE/usr/lib/modprobe.d/kybernow-camera.conf"
install -m 644 "$ROOT/server/camera/linux/70-kybernow-camera.rules" "$TREE/usr/lib/udev/rules.d/"
# Pare-feu : firewalld (Fedora, dont la zone par défaut ferme Bonjour) et le
# profil de ufw (Ubuntu), ouverts à l'installation comme sous Windows.
install -d "$TREE/usr/lib/firewalld/services" "$TREE/etc/ufw/applications.d"
install -m 644 "$ROOT/server/linux/packaging/kybernow-station.firewalld.xml" "$TREE/usr/lib/firewalld/services/kybernow-station.xml"
install -m 644 "$ROOT/server/linux/packaging/kybernow-station.ufw" "$TREE/etc/ufw/applications.d/kybernow-station"
# KDE Plasma : le droit de l'agent au protocole d'écrans de KWin.
install -m 644 "$ROOT/server/linux/packaging/fr.leslignesclaires.kybernow.station.display.desktop" "$TREE/usr/share/applications/"
install -m 644 "$ROOT/LICENSE" "$TREE/usr/share/doc/kybernow-station/copyright" 2>/dev/null || true
SIZE_KB=$(du -sk "$TREE" | cut -f1)

# Après une installation comme après une mise à jour (par Admin : le service
# root relance ensuite les services de l'utilisateur lui-même).
POSTINST='#!/bin/sh
set -e
udevadm control --reload 2>/dev/null || true
modprobe uinput 2>/dev/null || true
modprobe uhid 2>/dev/null || true
udevadm trigger --name-match=uinput 2>/dev/null || true
udevadm trigger --name-match=uhid 2>/dev/null || true
systemctl daemon-reload 2>/dev/null || true
systemctl enable --now kybernow-station-update.socket 2>/dev/null || true
systemctl --global enable kybernow-admin.service kybernow-kyber.service kybernow-tray.service 2>/dev/null || true
# Pare-feu actif : les ports de la Station ouverts (zone par défaut de
# firewalld, ufw), comme sous Windows ; un pare-feu arrêté reste tel quel.
if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
    firewall-cmd --reload >/dev/null 2>&1 || true
    firewall-cmd --permanent --add-service=kybernow-station >/dev/null 2>&1 || true
    firewall-cmd --add-service=kybernow-station >/dev/null 2>&1 || true
fi
if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q "Status: active"; then
    ufw allow "KyberNow Station" >/dev/null 2>&1 || true
fi
echo "KyberNow Station démarrera à votre prochaine ouverture de session (ou maintenant : systemctl --user start kybernow-admin kybernow-kyber kybernow-tray)."
'
# Seulement à la désinstallation : une mise à jour (dpkg « upgrade », rpm
# « 1 ») garde les services, sinon rpm les couperait après le %post de la
# nouvelle version.
PRERM='#!/bin/sh
set -e
case "$1" in
    remove | purge | 0)
        systemctl disable --now kybernow-station-update.socket 2>/dev/null || true
        systemctl --global disable kybernow-admin.service kybernow-kyber.service kybernow-tray.service 2>/dev/null || true
        if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
            firewall-cmd --permanent --remove-service=kybernow-station >/dev/null 2>&1 || true
            firewall-cmd --remove-service=kybernow-station >/dev/null 2>&1 || true
        fi
        if command -v ufw >/dev/null 2>&1; then
            ufw delete allow "KyberNow Station" >/dev/null 2>&1 || true
        fi
        ;;
esac
'
# Programmes que la Station lance, et bibliothèques du système qu'elle lit ;
# l'icône : GTK 3, AppIndicator (ouvert à l'exécution, Ayatana ou l'ancien) et
# WebKitGTK 4.1 (écran du prêt, wry).
DEB_DEPENDS="pipewire, pipewire-bin, wireplumber, pipewire-pulse | pulseaudio, pulseaudio-utils, gstreamer1.0-pipewire, gstreamer1.0-plugins-base, libgstreamer1.0-0, libgstreamer-plugins-base1.0-0, libva2, libva-drm2, libdrm2, libpulse0, libevdev2, libudev1, libxcb1, openssl, python3, systemd, libgtk-3-0t64 | libgtk-3-0, libayatana-appindicator3-1 | libappindicator3-1, libwebkit2gtk-4.1-0, util-linux, ethtool, gstreamer1.0-plugins-good, hwdata"
RPM_REQUIRES="pipewire, pipewire-utils, wireplumber, pipewire-pulseaudio, pulseaudio-utils, pipewire-gstreamer, gstreamer1-plugins-base, libva, libdrm, pulseaudio-libs, libevdev, systemd-libs, libxcb, openssl, python3, systemd, gtk3, (libayatana-appindicator-gtk3 or libappindicator-gtk3), webkit2gtk4.1, util-linux, ethtool, gstreamer1-plugins-good, hwdata"
# Recommandés : écrans de KDE, l'extension AppIndicator de GNOME (Ubuntu l'a
# d'office), boîtes de dialogue et notifications de l'icône, Wi-Fi (iw),
# fenêtres X11 (xprop), icônes des apps en SVG (rsvg-convert).
# Les aides d'un bureau ne s'installent que sur ce bureau : l'extension de
# GNOME dépend de gnome-shell, et dnf comme apt installent les recommandés
# (essai du 26/11 : tout GNOME et GDM arrivaient sur une Fedora KDE). Le .rpm
# les pose sous condition ; Debian n'a pas de condition, le .deb les suggère
# (Plasma a déjà kscreen-doctor, Ubuntu déjà l'extension).
DEB_RECOMMENDS="zenity | kdialog, libnotify-bin, iw, x11-utils, librsvg2-bin"
DEB_SUGGESTS="libkf6screen-bin | libkf5screen-bin, gnome-shell-extension-appindicator, v4l2loopback-dkms"
RPM_RECOMMENDS="(libkscreen if plasma-workspace), (gnome-shell-extension-appindicator if gnome-shell), (zenity or kdialog), libnotify, iw, xprop, librsvg2-tools"

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
Recommends: $DEB_RECOMMENDS
Suggests: $DEB_SUGGESTS
Section: net
Priority: optional
Homepage: https://kybernow.app
Description: KyberNow Station : votre ordinateur sur tous vos appareils
 Fait de cet ordinateur sous Linux une Station KyberNow : son écran, son son
 et ses manettes sur vos autres appareils, avec les apps KyberNow Link.
EOF
echo /etc/ufw/applications.d/kybernow-station > "$WORK/deb/DEBIAN/conffiles"
printf '%s' "$POSTINST" > "$WORK/deb/DEBIAN/postinst"
printf '%s' "$PRERM" > "$WORK/deb/DEBIAN/prerm"
chmod 755 "$WORK/deb/DEBIAN/postinst" "$WORK/deb/DEBIAN/prerm"
DEB="$DIST/kybernow-station_${DEB_VERSION}_${DEB_ARCH}.deb"
dpkg-deb --root-owner-group -Zxz --build "$WORK/deb" "$DEB" >/dev/null
echo "    $DEB"

echo "==> Archive des systèmes immuables (Bazzite, Fedora Atomic, SteamOS)"
if [ -x "$ROOT/server/linux/immutable/make-archive.sh" ]; then
    "$ROOT/server/linux/immutable/make-archive.sh" "$TREE" "$DIST"
else
    echo "    server/linux/immutable/make-archive.sh absent : pas d'archive"
fi

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
Recommends: $RPM_RECOMMENDS
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
/usr/lib/systemd/user/kybernow-tray.service
/usr/lib/systemd/system/kybernow-station-update.socket
/usr/lib/systemd/system/kybernow-station-update@.service
/usr/lib/udev/rules.d/60-kybernow-uinput.rules
/usr/lib/modules-load.d/kybernow-station.conf
/usr/lib/modules-load.d/kybernow-camera.conf
/usr/lib/modprobe.d/kybernow-camera.conf
/usr/lib/udev/rules.d/70-kybernow-camera.rules
/usr/share/applications/fr.leslignesclaires.kybernow.station.display.desktop
/usr/lib/firewalld/services/kybernow-station.xml
%config(noreplace) /etc/ufw/applications.d/kybernow-station
/usr/share/doc/kybernow-station
EOF
    rpmbuild --quiet --define "_topdir $WORK/rpm" -bb "$WORK/rpm/SPECS/kybernow-station.spec"
    cp "$WORK"/rpm/RPMS/*/*.rpm "$DIST/"
    echo "    $(ls "$DIST"/kybernow-station-*.rpm)"
else
    echo "    rpmbuild absent : pas de .rpm (dnf install rpm-build, ou apt install rpm)"
fi
