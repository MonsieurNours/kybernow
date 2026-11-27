#!/bin/bash
# Les vérifications sont des chaînes évaluées par check : pas de développement à l'écriture.
# shellcheck disable=SC2016,SC2034,SC2059
# Essais de la mise à jour A/B de KyberNow Link OS, sans carte ni root, sur
# Linux ou sur Mac (python3, xz et minisign suffisent) :
#
#   apps/linux/os/tests/os-update.sh
#
# 1. kybernow-os-image : une image factice (table de partitions MBR, deux
#    partitions au contenu connu) compressée en .xz, écrite dans deux fichiers
#    en un seul passage ; contenu, empreinte, taille, refus (emplacement trop
#    petit, .xz tronqué, octets ajoutés après le flux), manifeste, versions.
# 2. minisign : le commentaire de confiance que kybernow-os-update compare.
# 3. kybernow-os-update commit / status / rollback dans un faux système
#    (KYBERNOW_OS_TEST_ROOT, montages simulés) : essai réussi, essai raté,
#    commit pendant le démarrage qui a écrit l'image, retour refusé vers un
#    emplacement qui n'a pas démarré.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
F="$HERE/../stage-kybernow/01-link/files"
OSIMG="$F/kybernow-os-image"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
fails=0
ok() { echo "  ok   $1"; }
ko() { echo "  ÉCHEC $1" >&2; fails=$((fails + 1)); }
check() { if eval "$2"; then ok "$1"; else ko "$1"; fi; }
sha() { shasum -a 256 "$1" | cut -d' ' -f1; }

echo "== kybernow-os-image"
python3 - "$T" <<'EOF'
import os, sys
t = sys.argv[1]
SECTOR = 512
disk = bytearray(8 << 20)
def entry(i, start, count):
    o = 446 + 16 * i
    disk[o + 4] = 0x0c if i == 0 else 0x83
    disk[o + 8:o + 12] = start.to_bytes(4, "little")
    disk[o + 12:o + 16] = count.to_bytes(4, "little")
entry(0, 2048, 2048)    # p1 : 1 Mio à 1 Mio
entry(1, 4096, 6144)    # p2 : 3 Mio à 2 Mio
disk[510:512] = b"\x55\xaa"
# p1 : moitié de zéros (fichier creux), moitié de motif ; p2 : aléatoire.
disk[2048 * SECTOR + (512 << 10):4096 * SECTOR] = bytes(range(256)) * 2048
disk[4096 * SECTOR:(4096 + 6144) * SECTOR] = os.urandom(6144 * SECTOR)
open(f"{t}/disk.img", "wb").write(disk)
open(f"{t}/p1.want", "wb").write(disk[2048 * SECTOR:4096 * SECTOR])
open(f"{t}/p2.want", "wb").write(disk[4096 * SECTOR:(4096 + 6144) * SECTOR])
EOF
xz -T1 -3 -k "$T/disk.img"
: > "$T/boot.out"; : > "$T/root.out"
out="$("$OSIMG" write "$T/disk.img.xz" "$T/boot.out" "$T/root.out" $((4 << 20)) $((4 << 20)))"
check "empreinte et taille du fichier lu" '[ "$out" = "$(sha "$T/disk.img.xz") $(wc -c < "$T/disk.img.xz" | tr -d " ")" ]'
check "partition 1 identique" 'cmp -s "$T/boot.out" "$T/p1.want"'
check "partition 2 identique" 'cmp -s "$T/root.out" "$T/p2.want"'
set +e
"$OSIMG" write "$T/disk.img.xz" "$T/boot.out" "$T/root.out" $((4 << 20)) $((2 << 20)) > /dev/null 2>&1; rc=$?
set -e
check "emplacement trop petit refusé (6)" '[ "$rc" = 6 ]'
head -c 100000 "$T/disk.img.xz" > "$T/cut.xz"
check "image .xz tronquée refusée" '! "$OSIMG" write "$T/cut.xz" "$T/boot.out" "$T/root.out" $((4 << 20)) $((4 << 20)) > /dev/null 2>&1'
cp "$T/disk.img.xz" "$T/tail.xz"; printf 'X' >> "$T/tail.xz"
tail_out="$("$OSIMG" write "$T/tail.xz" "$T/boot.out" "$T/root.out" $((4 << 20)) $((4 << 20)) 2>/dev/null || true)"
check "un octet ajouté change l'empreinte" '[ -n "$tail_out" ] && [ "${tail_out%% *}" != "${out%% *}" ]'
head -c 4096 /dev/urandom | xz -T1 > "$T/nombr.xz"
check "image sans table de partitions refusée" '! "$OSIMG" write "$T/nombr.xz" "$T/boot.out" "$T/root.out" $((4 << 20)) $((4 << 20)) > /dev/null 2>&1'
printf '{"version":"0.12.0","channel":"stable","files":{"KyberNow-Link-OS.img.xz":{"sha256":"%s","size":%s}}}' "$(sha "$T/disk.img.xz")" "$(wc -c < "$T/disk.img.xz" | tr -d ' ')" > "$T/kybernow-release.json"
check "manifeste lu" '[ "$("$OSIMG" manifest "$T/kybernow-release.json" KyberNow-Link-OS.img.xz)" = "0.12.0 stable $(sha "$T/disk.img.xz") $(wc -c < "$T/disk.img.xz" | tr -d " ")" ]'
check "fichier absent du manifeste refusé" '! "$OSIMG" manifest "$T/kybernow-release.json" autre.xz 2>/dev/null'
check "0.12.0 plus récente que 0.11.9" '"$OSIMG" newer 0.12.0 0.11.9'
check "0.11.10 plus récente que 0.11.9" '"$OSIMG" newer 0.11.10 0.11.9'
check "0.11.9 pas plus récente que 0.11.9" '! "$OSIMG" newer 0.11.9 0.11.9'
check "0.9.0 pas plus récente que 0.11.0" '! "$OSIMG" newer 0.9.0 0.11.0'

echo "== minisign (commentaire de confiance)"
if command -v minisign >/dev/null; then
    minisign -G -W -p "$T/k.pub" -s "$T/k.sec" > /dev/null 2>&1
    minisign -S -s "$T/k.sec" -m "$T/kybernow-release.json" -t "kybernow 0.12.0 stable" > /dev/null 2>&1
    c="$(minisign -V -Q -p "$T/k.pub" -m "$T/kybernow-release.json" -x "$T/kybernow-release.json.minisig" 2>/dev/null)"
    check "commentaire rendu par -Q" '[ "$c" = "kybernow 0.12.0 stable" ]'
    printf ' ' >> "$T/kybernow-release.json"
    check "manifeste modifié refusé" '! minisign -V -Q -p "$T/k.pub" -m "$T/kybernow-release.json" -x "$T/kybernow-release.json.minisig" > /dev/null 2>&1'
else
    echo "  (minisign absent : passé)"
fi

echo "== kybernow-os-update (faux système)"
R="$T/root"
mkdir -p "$R/etc/kybernow-os" "$R/var/lib/kybernow" "$R/proc/device-tree/chosen/bootloader" "$R/proc/sys/kernel/random" "$R/boot/firmware" "$R/run" "$T/bin"
printf 'DISK_ID=4b79624e\nBOOT_A=1\nROOT_A=2\nBOOT_B=3\nROOT_B=5\nDATA=6\n' > "$R/etc/kybernow-os/layout"
printf 'NAME="KyberNow Link OS"\nVERSION="0.11.9"\n' > "$R/etc/kybernow-os/release"
printf 'raspberrypi,4-model-b\0brcm,bcm2711\0' > "$R/proc/device-tree/compatible"
echo "boot-1" > "$R/proc/sys/kernel/random/boot_id"
# Montages simulés : l'autre racine porte une version, rien d'autre.
cat > "$T/bin/mount" <<EOF
#!/bin/bash
d="\${@: -1}"; mkdir -p "\$d/etc/kybernow-os"; echo 'VERSION="0.12.0"' > "\$d/etc/kybernow-os/release"
EOF
printf '#!/bin/bash\nrm -rf "${@: -1}"/*\n' > "$T/bin/umount"
printf '#!/bin/bash\nexit 0\n' > "$T/bin/flock"
chmod +x "$T/bin/"*
u() { PATH="$T/bin:$PATH" KYBERNOW_OS_TEST_ROOT="$R" KYBERNOW_OS_TEST_ROOT_SRC=/dev/mmcblk0p2 KYBERNOW_OS_TEST_DISK=/dev/mmcblk0 SUDO_USER="" "$F/kybernow-os-update" "$@"; }
tryboot() { printf "\x00\x00\x00\x0$1" > "$R/proc/device-tree/chosen/bootloader/tryboot"; }
st() { cat "$R/var/lib/kybernow/.os-state/$1" 2>/dev/null || true; }
u prepare
check "prepare : dossier d'état" '[ -d "$R/var/lib/kybernow/.os-state" ] && [ -d "$R/var/lib/kybernow/update" ]'
check "status sans essai" 'u status | grep -q "\"current\":\"A\",\"version\":\"0.11.9\",\"other\":\"B\",\"other_version\":\"0.12.0\""'
# Essai réussi : démarré à l'essai (tryboot=1), autre démarrage que celui de l'écriture.
echo 0.12.0 > "$R/var/lib/kybernow/.os-state/pending"; echo boot-0 > "$R/var/lib/kybernow/.os-state/written"
tryboot 1
u commit > /dev/null
check "essai réussi : inscrit par défaut" 'grep -q "^boot_partition=1$" "$R/boot/firmware/autoboot.txt" && [ -z "$(st pending)" ] && [ -z "$(st failed)" ]'
# Commit pendant le démarrage qui vient d'écrire l'image : rien ne change.
echo 0.12.0 > "$R/var/lib/kybernow/.os-state/pending"; echo boot-1 > "$R/var/lib/kybernow/.os-state/written"
tryboot 0
u commit > /dev/null
check "même démarrage que l'écriture : essai gardé" '[ "$(st pending)" = 0.12.0 ] && [ -z "$(st failed)" ]'
# Essai raté : le firmware est revenu au défaut (tryboot=0), un essai attendait.
echo boot-0 > "$R/var/lib/kybernow/.os-state/written"
u commit > /dev/null
check "essai raté : noté, plus en attente" '[ "$(st failed)" = "B 0.12.0" ] && [ -z "$(st pending)" ]'
check "essai raté : autoboot sur l'emplacement en cours" '[ "$(grep -c "^boot_partition=1$" "$R/boot/firmware/autoboot.txt")" = 2 ]'
check "status : la version ratée, pas de version précédente" 'u status | grep -q "\"other_version\":\"\".*\"failed\":\"0.12.0\""'
check "retour vers l'emplacement raté refusé" '! u rollback --no-reboot > /dev/null 2>&1'
check "apply sans image refusé" '! u apply --no-reboot > /dev/null 2>&1'

echo
if [ "$fails" = 0 ]; then echo "tous les essais passent"; else echo "$fails essai(s) en échec" >&2; exit 1; fi
