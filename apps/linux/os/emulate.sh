#!/bin/bash
# Démarre l'image KyberNow Link OS dans QEMU (émulation, sans Raspberry Pi), sur
# un Linux arm64 avec sudo sans mot de passe : la VM « KyberNow Linux ».
#
#   apps/linux/os/emulate.sh [IMAGE.img.xz | IMAGE.img] [--work DOSSIER] [--size 8G] [--fresh]
#   apps/linux/os/emulate.sh --stop
#
# Ce que ça vérifie : le système tel que pi-gen l'a construit (comptes, services,
# Plymouth, le service Link sur KMS/DRM en utilisateur kybernow, le premier
# démarrage : partition de données agrandie, écrans à la manette ou au clavier,
# nmcli, kybernow-os-update status). Ce que ça ne vérifie pas : le firmware du
# Pi (config.txt, autoboot.txt, tryboot), le décodeur V4L2, le son, le
# Bluetooth, le CEC — voir « À vérifier sur une carte » dans le README.
#
# Comment : l'image (.xz décompressée) est copiée dans DOSSIER/kn.img et agrandie
# (--size), puis préparée pour QEMU, par montage : le noyau de la VM remplace
# celui du Pi (QEMU « virt » ne démarre pas un noyau Raspberry Pi : ses modules
# sont copiés dans le rootfs, vmlinuz et initrd sont donnés à QEMU), ssh est
# activé avec une clé créée ici et sudo ouvert au compte kybernow (pour l'essai
# seulement), et la carte virtuelle ne prenant que du XRGB à l'affichage, une
# petite bibliothèque (gbmshim.c) remplace l'ARGB demandé par SDL. Plymouth est
# coupé (plymouth.enable=0) : l'initrd est celui de la VM, son thème Ubuntu
# s'afficherait à la place du nôtre et son plymouthd garderait l'écran. QEMU tourne
# sans fenêtre : captures et touches par QMP (emulate/qmp.py), journal série
# dans DOSSIER/serial.log, ssh sur localhost:2222.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
LINUX="$(cd "$HERE/.." && pwd)"
WORK="${KYBERNOW_QEMU:-$HOME/kybernow-os/qemu}"
IMG="$LINUX/dist/KyberNow-Link-OS.img.xz"
SIZE=8G
FRESH=0
STOP=0
while [ $# -gt 0 ]; do
    case "$1" in
        --work) WORK="$2"; shift ;;
        --size) SIZE="$2"; shift ;;
        --fresh) FRESH=1 ;;
        --stop) STOP=1 ;;
        -*) echo "option inconnue : $1" >&2; exit 2 ;;
        *) IMG="$1" ;;
    esac
    shift
done
export KYBERNOW_QEMU="$WORK"
QMP="python3 $HERE/emulate/qmp.py"

if [ "$STOP" = 1 ]; then
    [ -S "$WORK/qmp.sock" ] && $QMP quit >/dev/null 2>&1 || true
    pkill -f "qemu-system-aarch64 .*$WORK/kn.img" 2>/dev/null || true
    echo "QEMU arrêté"
    exit 0
fi
for t in qemu-system-aarch64 gcc python3 xz losetup; do
    command -v "$t" >/dev/null || { echo "$t manque (apt install qemu-system-arm gcc python3 xz-utils)" >&2; exit 1; }
done
[ -f "$IMG" ] || { echo "image absente : $IMG (apps/linux/os/build-image.sh)" >&2; exit 1; }
[ "$(uname -m)" = aarch64 ] || echo "attention : hôte $(uname -m), l'émulation sera très lente" >&2
mkdir -p "$WORK/shots"
[ -S "$WORK/qmp.sock" ] && { $QMP quit >/dev/null 2>&1 || true; sleep 1; }

# 1. L'image à plat, agrandie : le premier démarrage agrandit la partition de données.
if [ "$FRESH" = 1 ] || [ ! -f "$WORK/kn.img" ] || [ "$IMG" -nt "$WORK/kn.img" ]; then
    echo "==> $WORK/kn.img depuis $IMG"
    case "$IMG" in *.xz) xz -dc "$IMG" > "$WORK/kn.img" ;; *) cp "$IMG" "$WORK/kn.img" ;; esac
    truncate -s "$SIZE" "$WORK/kn.img"
    PREPARE=1
else
    PREPARE=0
fi

# 2. Le noyau de la VM, la bibliothèque de substitution, la clé ssh.
KVER="$(uname -r)"
sudo -n cp "/boot/vmlinuz-$KVER" "$WORK/vmlinuz"; sudo -n cp "/boot/initrd.img-$KVER" "$WORK/initrd.img"; sudo -n chown "$(id -u)" "$WORK/vmlinuz" "$WORK/initrd.img"
gcc -shared -fPIC -O2 -o "$WORK/gbmshim.so" "$HERE/emulate/gbmshim.c" -ldl
[ -f "$WORK/id_ed25519" ] || ssh-keygen -q -t ed25519 -N "" -C "kybernow-os-emulation" -f "$WORK/id_ed25519"

# 3. Dans le rootfs (partition 2, l'emplacement A) : modules, ssh, sudo, substitution.
if [ "$PREPARE" = 1 ]; then
    echo "==> préparation du rootfs pour QEMU"
    LOOP="$(sudo -n losetup -fP --show "$WORK/kn.img")"
    R="$WORK/root"; mkdir -p "$R"
    sudo -n mount "${LOOP}p2" "$R"
    trap 'sudo -n umount "$R" 2>/dev/null; sudo -n losetup -d "$LOOP" 2>/dev/null' EXIT
    sudo -n rm -rf "$R/lib/modules"/*
    sudo -n cp -a "/lib/modules/$KVER" "$R/lib/modules/"
    sudo -n install -m 644 "$WORK/gbmshim.so" "$R/usr/local/lib/gbmshim.so"
    sudo -n install -d "$R/etc/systemd/system/kybernow-link.service.d"
    printf '# Émulation QEMU seulement (emulate.sh) : XRGB à l'"'"'affichage.\n[Service]\nEnvironment=LD_PRELOAD=/usr/local/lib/gbmshim.so\n' | sudo -n tee "$R/etc/systemd/system/kybernow-link.service.d/emulation.conf" >/dev/null
    sudo -n ln -sf /lib/systemd/system/ssh.service "$R/etc/systemd/system/multi-user.target.wants/ssh.service"
    sudo -n install -d -m 700 -o 1000 -g 1000 "$R/home/kybernow/.ssh"
    sudo -n install -m 600 -o 1000 -g 1000 "$WORK/id_ed25519.pub" "$R/home/kybernow/.ssh/authorized_keys"
    sudo -n sed -i 's/^kybernow:![^:]*:/kybernow:*:/' "$R/etc/shadow"
    printf 'kybernow ALL=(ALL) NOPASSWD: ALL\n' | sudo -n tee "$R/etc/sudoers.d/emulation" >/dev/null
    sudo -n chmod 440 "$R/etc/sudoers.d/emulation"
    sudo -n umount "$R"; sudo -n losetup -d "$LOOP"; trap - EXIT
fi

# 4. QEMU, sans fenêtre : QMP pour les captures et les touches, ssh sur 2222.
echo "==> QEMU (série : $WORK/serial.log)"
: > "$WORK/serial.log"
qemu-system-aarch64 -M virt -accel tcg,thread=multi -cpu cortex-a72 -smp 4 -m 2048 \
    -kernel "$WORK/vmlinuz" -initrd "$WORK/initrd.img" \
    -append "root=PARTUUID=4b79624e-02 rw rootwait console=ttyAMA0 plymouth.enable=0 vt.global_cursor_default=0" \
    -drive "file=$WORK/kn.img,format=raw,if=virtio" \
    -device virtio-gpu-pci -device virtio-keyboard-pci -device virtio-mouse-pci -device virtio-rng-pci \
    -netdev user,id=n0,hostfwd=tcp::2222-:22 -device virtio-net-pci,netdev=n0 \
    -display none -serial "file:$WORK/serial.log" -qmp "unix:$WORK/qmp.sock,server,nowait" \
    -daemonize -pidfile "$WORK/qemu.pid"
cat <<MSG
QEMU démarre (une à deux minutes jusqu'à l'app). Ensuite :
  ssh -p 2222 -i $WORK/id_ed25519 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null kybernow@localhost
  KYBERNOW_QEMU=$WORK python3 $HERE/emulate/qmp.py shot accueil     # → $WORK/shots/accueil.png
  KYBERNOW_QEMU=$WORK python3 $HERE/emulate/qmp.py key ret down     # touches
  $0 --stop
MSG
