#!/bin/bash -e
# Dans le chroot : le paquet, le compte, les services, Plymouth.
export DEBIAN_FRONTEND=noninteractive
apt-get install -y --no-install-recommends /tmp/kybernow-link.deb
rm -f /tmp/kybernow-link.deb

# Le compte qui fait tourner Link : pas de mot de passe (créé par pi-gen,
# FIRST_USER_NAME=kybernow), accès à l'écran, aux entrées, au son, aux manettes.
usermod -a -G video,render,input,audio,plugdev,netdev,bluetooth kybernow 2>/dev/null || usermod -a -G video,render,input,audio,plugdev,netdev kybernow
passwd -l kybernow >/dev/null
install -d -m 755 -o kybernow -g kybernow /var/lib/kybernow

# L'image fixe du démarrage : la première image de l'allumage de l'app, aux pixels près.
kybernow --splash /usr/share/plymouth/themes/kybernow/mark.png 1920 1080
plymouth-set-default-theme kybernow
update-initramfs -u -k all || true

# Services : Link en plein écran sur KMS/DRM, le premier démarrage (partition
# de données agrandie) ; plus de console texte ni de sortie de Plymouth à
# l'arrivée d'un getty (c'est l'app qui la lui demande).
systemctl enable kybernow-os-firstboot.service kybernow-link.service
systemctl disable getty@tty1.service
systemctl mask plymouth-quit.service plymouth-quit-wait.service
# L'assistant de Raspberry Pi OS qui demande un nom d'utilisateur au premier
# démarrage (userconfig) n'a pas lieu d'être : le compte est fixé.
systemctl mask userconfig.service 2>/dev/null || true
rm -f /etc/systemd/system/multi-user.target.wants/userconfig.service
systemctl enable NetworkManager.service bluetooth.service
# NetworkManager gère le réseau : l'attente de systemd-networkd échouerait.
systemctl disable systemd-networkd-wait-online.service 2>/dev/null || true
systemctl disable ssh.service 2>/dev/null || true
# Bluetooth : pas de discovery automatique au démarrage, l'app la lance.
systemctl disable hciuart.service 2>/dev/null || true; systemctl enable hciuart.service 2>/dev/null || true
