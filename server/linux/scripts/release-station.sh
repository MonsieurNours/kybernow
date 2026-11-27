#!/bin/bash
# Construit les paquets de KyberNow Station pour Linux publiés avec chaque
# version (docs/station-linux.md, docs/releases.md) : un .deb et un .rpm par
# architecture, sous les noms fixes que cherche Admin pour ses mises à jour :
#
#   KyberNow-Station-Linux-arm64.deb    KyberNow-Station-Linux-arm64.rpm
#   KyberNow-Station-Linux-x86_64.deb   KyberNow-Station-Linux-x86_64.rpm
#   KyberNow-Station-Linux-aarch64.tar.xz, KyberNow-Station-Linux-x86_64.tar.xz
#     (systèmes immuables, installées dans le dossier de l'utilisateur)
#
# Chaque architecture se construit sur un Linux de cette architecture, joint
# par ssh, avec ce dépôt au même commit (build-kyber.sh puis package.sh) :
#   arm64  : la VM « KyberNow Linux » (KYBERNOW_STATION_ARM64_SSH, défaut
#            kybernow-vm ; dépôt en dossier partagé, KYBERNOW_LINUX_REPO, sinon
#            cet arbre vu de la VM : vm_repo de apps/linux/scripts/vm.sh) ;
#   x86_64 : un PC ou une VM x86_64 (KYBERNOW_STATION_X86_SSH, dépôt dans
#            KYBERNOW_STATION_X86_REPO, défaut ~/kyber-apps) ; sans elle, la
#            même VM arm64, dans son Ubuntu x86_64 par Rosetta
#            (server/linux/scripts/x86-chroot.sh, sources de Kyber x86_64 dans
#            ~/kn-amd64-home/kyber-desktop).
# Sources de Kyber sur chaque machine : KYBERNOW_STATION_KYBER_DIR (défaut
# ~/kn-station/kyber-desktop).
#
#   server/linux/scripts/release-station.sh [--check] [DOSSIER]
#
# → server/linux/dist/release/KyberNow-Station-Linux-*, copiés dans DOSSIER
# s'il est donné. --check : les deux machines répondent, au bon commit.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
ARM_HOST="${KYBERNOW_STATION_ARM64_SSH:-kybernow-vm}"
# Ce dépôt vu de la VM (arbre de construction de release.sh compris), comme
# release-flatpaks.sh : apps/linux/scripts/vm.sh, vm_repo.
. "$ROOT/apps/linux/scripts/vm.sh"
ARM_REPO="$(vm_repo "$ROOT")"
X86_HOST="${KYBERNOW_STATION_X86_SSH:-}"
X86_REPO="${KYBERNOW_STATION_X86_REPO:-~/kyber-apps}"
# Sans machine x86_64 : le chroot x86_64 de la VM arm64 (Rosetta).
X86_CHROOT=0
if [ -z "$X86_HOST" ]; then X86_CHROOT=1; X86_HOST="$ARM_HOST" X86_REPO="$ARM_REPO"; fi
KYBER="${KYBERNOW_STATION_KYBER_DIR:-~/kn-station/kyber-desktop}"
OUT="$ROOT/server/linux/dist/release"
CHECK=0 DEST=""
for a in "$@"; do
    case "$a" in
        --check) CHECK=1 ;;
        -*) echo "option inconnue : $a" >&2; exit 2 ;;
        *) DEST="$a" ;;
    esac
done

HEAD="$(git -C "$ROOT" rev-parse HEAD)"
remote() { local host="$1"; shift; ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" "$@"; }

# Machine prête : joignable, de la bonne architecture, dépôt au même commit
# (un dossier partagé sans .git est pris tel quel : c'est cet arbre).
ready() {
    local host="$1" repo="$2" arch="$3" got
    [ -n "$host" ] || { echo "machine $arch non réglée (KYBERNOW_STATION_X86_SSH)"; return 1; }
    got="$(remote "$host" uname -m 2>/dev/null)" || { echo "$host injoignable par ssh"; return 1; }
    [ "$got" = "$arch" ] || { echo "$host est en $got, pas en $arch"; return 1; }
    remote "$host" "test -x $repo/server/linux/scripts/package.sh" || { echo "dépôt absent sur $host : $repo"; return 1; }
    if remote "$host" "test -d $repo/.git"; then
        got="$(remote "$host" "git -C $repo rev-parse HEAD")"
        [ "$got" = "$HEAD" ] || { echo "$host : dépôt en ${got:0:9}, pas en ${HEAD:0:9} (git fetch && git checkout)"; return 1; }
    fi
    remote "$host" "test -d $KYBER" || { echo "sources de Kyber absentes sur $host : $KYBER"; return 1; }
}

msg="$(ready "$ARM_HOST" "$ARM_REPO" aarch64)" || { echo "Station Linux arm64 : $msg" >&2; exit 1; }
if [ "$X86_CHROOT" = 1 ]; then
    remote "$ARM_HOST" "$ARM_REPO/server/linux/scripts/x86-chroot.sh run 'test \"\$(uname -m)\" = x86_64 && command -v cargo-cbuild >/dev/null && test -d ~/kyber-desktop'" >/dev/null 2>&1 \
        || { echo "Station Linux x86_64 : chroot x86_64 de $ARM_HOST pas prêt (x86-chroot.sh setup, sources de Kyber dans ~/kn-amd64-home/kyber-desktop)" >&2; exit 1; }
    X86_WHERE="$ARM_HOST, Ubuntu x86_64 par Rosetta"
else
    msg="$(ready "$X86_HOST" "$X86_REPO" x86_64)" || { echo "Station Linux x86_64 : $msg" >&2; exit 1; }
    X86_WHERE="$X86_HOST"
fi
[ "$CHECK" = 1 ] && { echo "Station Linux : arm64 sur $ARM_HOST, x86_64 sur $X86_WHERE"; exit 0; }

# Interface d'Admin, commune aux deux architectures : construite ici, sur le
# Mac, comme pour la Station Mac (build-station-app.sh) ; les machines la
# lisent dans le dépôt partagé (package.sh, server/web/dist).
(cd "$ROOT/server/web" && npm ci --silent && npm run build --silent)

rm -rf "$OUT"
mkdir -p "$OUT"
for arch in aarch64 x86_64; do
    if [ "$arch" = aarch64 ]; then host="$ARM_HOST" repo="$ARM_REPO" name=arm64; else host="$X86_HOST" repo="$X86_REPO" name=x86_64; fi
    # shellcheck disable=SC2088  # ~ de la machine qui construit, développé là-bas
    dist="~/kn-station/release-$name"
    # Sources de Kyber de la machine remises à l'origine dans kyctl, kymux et
    # kynput, comme les arbres de release.sh : après les correctifs des apps
    # et ceux de la Station qui touchent les mêmes fichiers, un correctif d'app
    # n'est plus retirable et build-kyber.sh s'arrêtait (« à moitié posé »).
    reset="for r in kyctl kymux kynput; do git -C $KYBER/kysdk/\$r checkout -q -- . && git -C $KYBER/kysdk/\$r clean -fdq || exit 1; done"
    build="rm -rf $dist && mkdir -p ~/kn-station && ( $reset ) && cd $repo && KYBER_DIR=$KYBER server/linux/scripts/build-kyber.sh > ~/kn-station/release-$name-kyber.log 2>&1 \
        && KYBER_DIR=$KYBER DIST=$dist server/linux/scripts/package.sh > ~/kn-station/release-$name.log 2>&1"
    if [ "$arch" = x86_64 ] && [ "$X86_CHROOT" = 1 ]; then
        # Dans le chroot, ~ est ~/kn-amd64-home de la VM (sources de Kyber x86_64 à part).
        echo "==> Station Linux $name ($X86_WHERE)"
        build="${build//$KYBER/~/kyber-desktop}"
        remote "$host" "$repo/server/linux/scripts/x86-chroot.sh run '$build'" \
            || { echo "échec ; journaux sur $host : ~/kn-amd64-home/kn-station/release-$name*.log" >&2; remote "$host" "tail -20 ~/kn-amd64-home/kn-station/release-$name.log" >&2 || true; exit 1; }
        # shellcheck disable=SC2088  # ~ de la VM, développé là-bas
        dist="~/kn-amd64-home/kn-station/release-$name"
    else
        echo "==> Station Linux $name ($host)"
        remote "$host" "$build" \
            || { echo "échec ; journaux sur $host : ~/kn-station/release-$name*.log" >&2; remote "$host" "tail -20 ~/kn-station/release-$name.log" >&2 || true; exit 1; }
    fi
    # « || true » : sous pipefail, un paquet absent arrêtait le script avant
    # le message ci-dessous (STLIN-22).
    deb="$(remote "$host" "ls $dist/*.deb 2>/dev/null" | head -1 || true)"
    rpm="$(remote "$host" "ls $dist/*.rpm 2>/dev/null" | head -1 || true)"
    [ -n "$deb" ] && [ -n "$rpm" ] || { echo "$host : .deb ou .rpm absent de $dist (rpmbuild ?)" >&2; exit 1; }
    scp -q "$host:$deb" "$OUT/KyberNow-Station-Linux-$name.deb"
    scp -q "$host:$rpm" "$OUT/KyberNow-Station-Linux-$name.rpm"
    scp -q "$host:$dist/KyberNow-Station-Linux-$arch.tar.xz" "$OUT/"
    for f in "$OUT"/KyberNow-Station-Linux-$name.* "$OUT/KyberNow-Station-Linux-$arch.tar.xz"; do
        echo "    $(du -h "$f" | cut -f1) $f"
        [ -z "$DEST" ] || cp "$f" "$DEST/"
    done
done
