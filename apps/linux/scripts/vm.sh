# shellcheck shell=bash
# La VM Parallels « KyberNow Linux » vue du Mac, pour release-flatpaks.sh,
# os/release-image.sh et scripts/release.sh (source).
#
# Réglages : KYBERNOW_LINUX_VM (nom Parallels), KYBERNOW_LINUX_SSH (hôte ssh),
# KYBERNOW_LINUX_REPO (cet arbre vu de la VM ; par défaut, déduit de son
# chemin dans le dépôt principal, que la VM voit en /media/psf/kyber-apps :
# un worktree ou un arbre de construction de release.sh s'y trouve aussi).
#
# La VM tourne sans fenêtre (vue au démarrage « headless ») : jamais au
# premier plan, elle ne prend ni le clavier ni la souris du Mac. Démarrée au
# besoin, elle reste allumée.

VM_NAME="${KYBERNOW_LINUX_VM:-KyberNow Linux}"
VM_HOST="${KYBERNOW_LINUX_SSH:-kybernow-vm}"
VM_MAIN_IN_VM="${KYBERNOW_LINUX_REPO_MAIN:-/media/psf/kyber-apps}"

# vm_repo <racine de l'arbre sur le Mac> : le même dossier vu de la VM.
vm_repo() {
    local root main
    [ -n "${KYBERNOW_LINUX_REPO:-}" ] && { echo "$KYBERNOW_LINUX_REPO"; return; }
    root="$(cd "$1" && pwd -P)"
    main="$(cd "$(git -C "$root" rev-parse --git-common-dir 2>/dev/null)/.." 2>/dev/null && pwd -P)" || main="$root"
    case "$root" in
        "$main" | "$main"/*) echo "$VM_MAIN_IN_VM${root#"$main"}" ;;
        *) echo "$VM_MAIN_IN_VM" ;;
    esac
}

vm() { ssh -o BatchMode=yes -o ConnectTimeout=5 -o ServerAliveInterval=30 "$VM_HOST" "$@"; }

# Vue au démarrage de la VM (headless, window, same…).
vm_vue() {
    command -v prlctl >/dev/null || return 0
    prlctl list -i "$VM_NAME" 2>/dev/null | awk -F': ' '/Startup view:/ {print $2; exit}'
}

# vm_sans_fenetre : vue au démarrage « headless » (sans effet sur une VM déjà
# ouverte dans une fenêtre : vaut au prochain démarrage).
vm_sans_fenetre() {
    command -v prlctl >/dev/null || return 0
    [ "$(vm_vue)" = headless ] && return 0
    prlctl set "$VM_NAME" --startup-view headless >/dev/null 2>&1 \
        || echo "attention : VM « $VM_NAME » : vue au démarrage non réglée sur headless (prlctl set --startup-view headless)" >&2
}

# vm_demarrer : la VM répond au ssh (démarrée sans fenêtre si elle dort,
# attendue deux minutes au plus).
vm_demarrer() {
    local state
    vm true 2>/dev/null && return 0
    command -v prlctl >/dev/null || { echo "VM « $VM_NAME » injoignable par ssh ($VM_HOST), et prlctl absent" >&2; return 1; }
    vm_sans_fenetre
    state="$(prlctl status "$VM_NAME" 2>/dev/null | awk '{print $NF}')"
    case "$state" in
        running) ;;
        suspended | paused) prlctl resume "$VM_NAME" >/dev/null ;;
        stopped) prlctl start "$VM_NAME" >/dev/null ;;
        *) echo "VM « $VM_NAME » introuvable (prlctl status : ${state:-rien})" >&2; return 1 ;;
    esac
    for _ in $(seq 60); do vm true 2>/dev/null && return 0; sleep 2; done
    echo "VM « $VM_NAME » démarrée mais injoignable par ssh ($VM_HOST)" >&2
    return 1
}

# vm_rosetta : x86_64 par Rosetta (binfmt), remise par le service s'il l'a perdue.
vm_rosetta() {
    vm "test -e /proc/sys/fs/binfmt_misc/rosetta" && return 0
    # Un apt install relance systemd-binfmt, qui peut effacer Rosetta : le service la remet.
    vm "sudo -n systemctl restart kybernow-rosetta 2>/dev/null; test -e /proc/sys/fs/binfmt_misc/rosetta" \
        || { echo "Rosetta absente du binfmt de la VM (x86_64 impossible) : sudo systemctl restart kybernow-rosetta" >&2; return 1; }
}

# vm_lancer <journal dans la VM> <commande> : lance la commande dans la VM
# (bash, pipefail), sa sortie ici et dans ~/<journal> de la VM.
vm_lancer() {
    local log=$1
    shift
    vm "bash -o pipefail -c 'cd ~ && { $* ; } 2>&1 | tee ~/$log'"
}
