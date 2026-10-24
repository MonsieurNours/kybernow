#!/bin/bash
# Refait une image Raspberry Pi OS à deux partitions (boot, root) en image
# « KyberNow Link OS » à deux emplacements A/B et une partition de données, pour
# le tryboot du firmware (Pi 4, 400, CM4, 5, 500, CM5 ; un Pi 3 démarre
# l'emplacement A et ne se met pas à jour en A/B) :
#
#   sudo apps/linux/os/make-ab-image.sh BASE.img SORTIE.img
#
#   p1  BOOTA  FAT32  256 Mo   démarrage A, et autoboot.txt (lu par le firmware)
#   p2  rootA  ext4            système A (celui de l'image)
#   p3  BOOTB  FAT32  256 Mo   démarrage B (vide : la première mise à jour l'écrit)
#   p4  étendue
#   p5  rootB  ext4            système B (vide)
#   p6  data   ext4   256 Mo   ordinateurs, réglages, journal, mises à jour
#                              (agrandie au premier démarrage jusqu'au bout de la carte)
#
# L'identifiant de disque est fixe (0x4b79624e) : les PARTUUID le sont aussi
# (4b79624e-01 … -06), écrits dans cmdline.txt et fstab. autoboot.txt :
# [all] tryboot_a_b=1 boot_partition=1 / [tryboot] boot_partition=3 ; la mise à
# jour (kybernow-os-update) écrit bootB et rootB, essaie B, puis l'inscrit.
set -euo pipefail
BASE="$1"; OUT="$2"
[ "$(id -u)" = 0 ] || { echo "à lancer en root (losetup, mount)" >&2; exit 1; }
[ -f "$BASE" ] || { echo "image absente : $BASE" >&2; exit 1; }
DISK_ID=4b79624e
MiB=$((1024 * 1024))
BOOT_MB=256
DATA_MB=256

cleanup() {
    set +e
    [ -n "${MNT:-}" ] && { umount -q "$MNT/boot/firmware" 2>/dev/null; umount -q "$MNT" 2>/dev/null; umount -q "$MNT" 2>/dev/null; rmdir "$MNT" 2>/dev/null; }
    [ -n "${LB:-}" ] && losetup -d "$LB" 2>/dev/null
    [ -n "${LO:-}" ] && losetup -d "$LO" 2>/dev/null
}
trap cleanup EXIT

# La base : ses deux partitions.
LB="$(losetup --show --find --partscan "$BASE")"
sleep 1
[ -b "${LB}p1" ] && [ -b "${LB}p2" ] || { echo "la base n'a pas deux partitions ($LB)" >&2; exit 1; }
# rootA et rootB : ce que la base occupe (pas sa partition, que pi-gen laisse
# large), plus 15 % et 128 Mo pour les mises à jour du système ; le tout doit
# tenir sur une carte de 8 Go (environ 7 580 Mio utilisables).
SRC="$(mktemp -d)"
mount -o ro "${LB}p2" "$SRC"
USED_MB=$(du -sm -x "$SRC" | cut -f1)
umount "$SRC"; rmdir "$SRC"
ROOT_MB=$(( USED_MB * 115 / 100 + 128 ))
ROOT_MB=$(( (ROOT_MB + 7) / 8 * 8 ))

# La sortie : partitions alignées sur 8 Mo.
p1s=8; p1e=$((p1s + BOOT_MB))
p2s=$p1e; p2e=$((p2s + ROOT_MB))
p3s=$p2e; p3e=$((p3s + BOOT_MB))
p4s=$p3e
p5s=$((p4s + 8)); p5e=$((p5s + ROOT_MB))
p6s=$((p5e + 8)); p6e=$((p6s + DATA_MB))
p4e=$p6e
TOTAL=$((p6e + 8))
rm -f "$OUT"
truncate -s "$((TOTAL * MiB))" "$OUT"
parted --script "$OUT" mklabel msdos \
    unit MiB \
    mkpart primary fat32 $p1s $p1e \
    mkpart primary ext4 $p2s $p2e \
    mkpart primary fat32 $p3s $p3e \
    mkpart extended $p4s $p4e \
    mkpart logical ext4 $p5s $p5e \
    mkpart logical ext4 $p6s $p6e \
    set 1 boot on
# L'identifiant de disque (PARTUUID), fixe.
sfdisk --disk-id "$OUT" "0x$DISK_ID" >/dev/null
sfdisk -d "$OUT" | grep -qi "label-id: 0x$DISK_ID" || { echo "identifiant de disque non écrit" >&2; exit 1; }
LO="$(losetup --show --find --partscan "$OUT")"
sleep 1
for n in 1 2 3 5 6; do [ -b "${LO}p$n" ] || { echo "partition $n absente ($LO)" >&2; exit 1; }; done

echo "==> A/B : boot $BOOT_MB Mo × 2, root $ROOT_MB Mo × 2, données $DATA_MB Mo (image $TOTAL Mo)"
# bootA et rootA : copie de la base.
mkfs.vfat -F 32 -n BOOTA "${LO}p1" >/dev/null
mkfs.vfat -F 32 -n BOOTB "${LO}p3" >/dev/null
mkfs.ext4 -q -L rootA "${LO}p2"
mkfs.ext4 -q -L rootB "${LO}p5"
mkfs.ext4 -q -L data "${LO}p6"
MNT="$(mktemp -d)"
mount "${LO}p2" "$MNT"
SRC="$(mktemp -d)"
mount -o ro "${LB}p2" "$SRC"
rsync -aHAX --numeric-ids "$SRC/" "$MNT/"
umount "$SRC"
mkdir -p "$MNT/boot/firmware"
mount "${LO}p1" "$MNT/boot/firmware"
mount -o ro "${LB}p1" "$SRC"
rsync -rt "$SRC/" "$MNT/boot/firmware/"
umount "$SRC"; rmdir "$SRC"

# cmdline.txt : la racine A ; fstab : A, et les données.
sed -i -E "s#root=[^ ]+#root=PARTUUID=$DISK_ID-02#" "$MNT/boot/firmware/cmdline.txt"
grep -q "PARTUUID=$DISK_ID-02" "$MNT/boot/firmware/cmdline.txt"
cat > "$MNT/etc/fstab" <<EOF
proc                      /proc            proc    defaults          0  0
PARTUUID=$DISK_ID-01  /boot/firmware   vfat    defaults          0  2
PARTUUID=$DISK_ID-02  /                ext4    defaults,noatime  0  1
PARTUUID=$DISK_ID-06  /var/lib/kybernow ext4   defaults,noatime,nofail,x-systemd.growfs  0  2
EOF
mkdir -p "$MNT/var/lib/kybernow"
# autoboot.txt : A par défaut, B à l'essai.
cat > "$MNT/boot/firmware/autoboot.txt" <<EOF
[all]
tryboot_a_b=1
boot_partition=1
[tryboot]
boot_partition=3
EOF
# L'image sait sa disposition (lue par kybernow-os-update).
cat > "$MNT/etc/kybernow-os/layout" <<EOF
DISK_ID=$DISK_ID
BOOT_A=1
ROOT_A=2
BOOT_B=3
ROOT_B=5
DATA=6
EOF
sync
umount "$MNT/boot/firmware"
umount "$MNT"
rmdir "$MNT"; MNT=""
losetup -d "$LO"; LO=""
losetup -d "$LB"; LB=""
echo "    $OUT"
