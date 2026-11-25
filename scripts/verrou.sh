# shellcheck shell=bash
# Verrous des constructions : un dossier de sources de Kyber, un Vendor/ ou un
# arbre de construction (scripts/arbres.sh) n'est touché que par une
# construction à la fois. Lu (source) par scripts/release.sh, scripts/arbres.sh
# et par les scripts de compilation (via scripts/kyber-client-patches.sh,
# server/kyber-patches/build.sh, server/macos/scripts/build-kyber.sh).
#
# Un verrou est un dossier « <chemin>.verrou » créé atomiquement (mkdir), qui
# contient « qui » : le processus qui le tient (pid, date de démarrage du
# processus, machine), la commande et l'heure. Il est rendu à la sortie de ce
# processus (kyber_verrou_rendre, ou tout seul : un verrou dont le processus
# n'existe plus est périmé et repris, avec un message).
#
# Imbrication : un script lancé par celui qui tient déjà le verrou (release.sh
# qui lance build-kyclient.sh, par exemple) le reconnaît par la variable
# KYBERNOW_VERROUS (chemins tenus, séparés par « : », héritée par les
# processus fils) et ne le reprend pas.
#
# KYBERNOW_VERROU_ATTENTE=<secondes> : attendre au plus ce temps que le verrou
# se libère (défaut 0 : refus immédiat, avec le nom de celui qui le tient).

# Date de démarrage d'un processus (contre la réutilisation d'un pid).
kyber_verrou_debut() {
    LC_ALL=C ps -o lstart= -p "$1" 2>/dev/null | sed 's/^ *//; s/ *$//'
}

# kyber_verrou_tenu <chemin> : vrai si ce processus ou un parent le tient déjà.
kyber_verrou_tenu() {
    case ":${KYBERNOW_VERROUS:-}:" in *":$1:"*) return 0 ;; esac
    return 1
}

# kyber_verrou_perime <dossier du verrou> : vrai si son processus n'existe plus
# (même machine), ou si son fichier « qui » manque depuis plus d'une minute
# (création interrompue).
kyber_verrou_perime() {
    local d=$1 pid debut machine
    if [ ! -f "$d/qui" ]; then
        [ -n "$(find "$d" -maxdepth 0 -mmin +1 2>/dev/null)" ]
        return
    fi
    pid=$(sed -n 's/^pid=//p' "$d/qui")
    debut=$(sed -n 's/^debut=//p' "$d/qui")
    machine=$(sed -n 's/^machine=//p' "$d/qui")
    # Tenu depuis une autre machine (la VM) : jamais repris d'ici.
    [ "$machine" = "$(hostname -s)" ] || return 1
    [ -n "$pid" ] || return 0
    kill -0 "$pid" 2>/dev/null || return 0
    [ "$(kyber_verrou_debut "$pid")" = "$debut" ] || return 0
    return 1
}

# kyber_verrou_prendre <chemin> [description] : prend le verrou de <chemin>
# pour le processus courant ($$ : le script, pas un sous-shell) ; refuse (code
# 1, message sur la sortie d'erreur) s'il est tenu ailleurs.
kyber_verrou_prendre() {
    local cible=$1 quoi=${2:-${0##*/}} d attente fin
    kyber_verrou_tenu "$cible" && return 0
    d="$cible.verrou"
    mkdir -p "$(dirname "$d")"
    attente=${KYBERNOW_VERROU_ATTENTE:-0}
    fin=$(( $(date +%s) + attente ))
    while ! mkdir "$d" 2>/dev/null; do
        if kyber_verrou_perime "$d"; then
            # Laissé par un processus terminé (le cas normal quand son script
            # n'a pas pu le rendre lui-même) : repris sans bruit.
            rm -rf "$d"
            continue
        fi
        if [ "$(date +%s)" -ge "$fin" ]; then
            echo "Déjà en cours de construction : $cible" >&2
            sed 's/^/    /' "$d/qui" >&2 2>/dev/null || echo "    (verrou $d sans « qui »)" >&2
            echo "    Attendre la fin de cette construction (ou KYBERNOW_VERROU_ATTENTE=<secondes>) ;" \
                "si ce processus n'existe plus, le verrou est repris tout seul." >&2
            return 1
        fi
        sleep 2
    done
    {
        echo "pid=$$"
        echo "debut=$(kyber_verrou_debut $$)"
        echo "machine=$(hostname -s)"
        echo "commande=$quoi"
        echo "dossier=$(pwd)"
        echo "depuis=$(date '+%Y-%m-%d %H:%M:%S')"
    } > "$d/qui"
    export KYBERNOW_VERROUS="${KYBERNOW_VERROUS:+$KYBERNOW_VERROUS:}$cible"
}

# kyber_verrou_rendre <chemin> : rend un verrou pris par ce processus.
kyber_verrou_rendre() {
    local cible=$1 d="$1.verrou"
    [ -f "$d/qui" ] && [ "$(sed -n 's/^pid=//p' "$d/qui")" = "$$" ] && rm -rf "$d"
    KYBERNOW_VERROUS="$(printf '%s' "${KYBERNOW_VERROUS:-}" | tr ':' '\n' | { grep -vxF "$cible" || true; } | paste -sd: -)"
    export KYBERNOW_VERROUS
}

# kyber_verrou_sortie <chemin> [description] : prend le verrou et le rend à la
# sortie du script (ajouté au trap EXIT existant ; un trap posé ensuite par le
# script le remplace, et le verrou est alors repris comme périmé).
kyber_verrou_sortie() {
    local cible=$1 avant
    kyber_verrou_tenu "$cible" && return 0
    kyber_verrou_prendre "$cible" "${2:-${0##*/}}" || return 1
    avant="$(trap -p EXIT | sed "s/^trap -- '//; s/' EXIT\$//")"
    # shellcheck disable=SC2064
    trap "${avant:+$avant; }kyber_verrou_rendre '$cible'" EXIT
}
