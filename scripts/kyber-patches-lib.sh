# shellcheck shell=bash
# Pose des correctifs de Kyber, en un seul endroit (audit TRANS-20) : remise à
# l'origine des fichiers d'un correctif, pose sans décalage du contexte,
# empreinte d'un wrap de meson, empreinte d'une liste de fichiers. Lu (source)
# par scripts/kyber-client-patches.sh (correctifs des apps) et par
# server/kyber-patches/apply.sh (correctifs des Stations Windows, Mac et
# Linux, et du moteur web). Rien ne s'exécute au chargement.
#
# Pose stricte : un morceau qui ne s'applique qu'avec un décalage du contexte
# (« fuzz ») peut tomber au mauvais endroit sans erreur. C'est un échec par
# défaut (STCOM-05) ; KYBERNOW_PATCH_STRICT=0 le ramène à un avertissement.
# Toute la série se pose sans décalage depuis le 27/11/2026.

# kyber_sha256 (et la chaîne d'outils) : scripts/kyber-toolchain.sh.
# shellcheck source=kyber-toolchain.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/kyber-toolchain.sh"

# kyber_patch_files <correctif>… : les fichiers qu'ils touchent, chemins de
# leur diff (« +++ b/<chemin> »).
kyber_patch_files() {
    grep -h '^+++ b/' "$@" | sed 's|^+++ b/||; s|[[:space:]].*||'
}

# kyber_patch_reset <dépôt> <correctif> [correctif…] : remet à l'origine (HEAD
# de son dépôt git) chaque fichier que touche le correctif ; un fichier qu'il
# ajoute est retiré. Les fichiers des sous-projets txproto et libavconv de
# kymedia, qui ont leur propre dépôt, sont remis dans celui-ci. Les correctifs
# donnés après le premier sont ceux déjà posés sur ces sources : un fichier
# qu'ils touchent reste tel quel.
kyber_patch_reset() {
    local repo=$1 patch=$2 f sub dir rel kept=""
    shift 2
    # La liste des fichiers déjà touchés, lue une fois. Pas de
    # « kyber_patch_files … | grep -q » : grep -q s'arrête au premier fichier
    # trouvé, kyber_patch_files reçoit SIGPIPE, et sous « set -o pipefail » le
    # test échoue au hasard ; le fichier était alors remis à l'origine, ce qui
    # effaçait ce qu'un correctif commun y avait mis (Station Linux, 0.11.8 :
    # avconv/video.rs perdu avant le correctif 0013, décalages et échecs
    # changeant d'une construction à l'autre).
    [ $# -gt 0 ] && kept="$(kyber_patch_files "$@")"
    while IFS= read -r f; do
        if [ -n "$kept" ] && grep -qxF -- "$f" <<< "$kept"; then continue; fi
        case "$f" in
            subprojects/txproto/* | subprojects/libavconv/*)
                sub=${f#subprojects/}; sub=${sub%%/*}
                dir="$repo/subprojects/$sub" rel=${f#subprojects/"$sub"/} ;;
            *) dir=$repo rel=$f ;;
        esac
        if git -C "$dir" cat-file -e "HEAD:$rel" 2>/dev/null; then
            git -C "$dir" checkout -q HEAD -- "$rel" || return 1
        else
            # Ajouté par le correctif : absent de l'origine.
            git -C "$dir" rm -q --cached --ignore-unmatch -- "$rel" >/dev/null 2>&1 || true
            rm -f "$dir/$rel"
        fi
    done < <(kyber_patch_files "$patch")
}

# kyber_patch_posed <correctif> <dossier> : vrai s'il y est déjà posé en entier.
kyber_patch_posed() {
    (cd "$2" && patch -p1 -R --dry-run -s -f < "$1") >/dev/null 2>&1
}

# kyber_patch_put <correctif> <dossier> : pose le correctif (patch -p1) dans le
# dossier, sans décalage du contexte (voir plus haut). Essai à blanc d'abord :
# un correctif refusé ne laisse rien à moitié posé. La sortie de patch est
# affichée en retrait. Code 1 si le correctif ne se pose pas.
kyber_patch_put() {
    local p=$1 dir=$2 name out
    local fuzz=(-F0)
    name=$(basename "$p")
    if ! (cd "$dir" && patch -p1 -F0 --forward --dry-run -s < "$p") >/dev/null 2>&1; then
        if (cd "$dir" && patch -p1 --forward --dry-run -s < "$p") >/dev/null 2>&1; then
            echo "Correctif $name : ne se pose qu'avec un décalage du contexte (fuzz), à régénérer" >&2
            [ "${KYBERNOW_PATCH_STRICT:-1}" = 0 ] || return 1
            echo "    (KYBERNOW_PATCH_STRICT=0 : posé quand même)" >&2
            fuzz=()
        else
            (cd "$dir" && patch -p1 --forward --dry-run < "$p") 2>&1 | sed 's/^/        /' >&2
            echo "Correctif $name inapplicable dans $dir (morceau en échec ci-dessus)" >&2
            return 1
        fi
    fi
    if ! out="$(cd "$dir" && patch -p1 ${fuzz[@]+"${fuzz[@]}"} --forward --no-backup-if-mismatch -r - < "$p" 2>&1)"; then
        printf '%s\n' "$out" | sed 's/^/        /' >&2
        echo "Correctif $name inapplicable dans $dir" >&2
        return 1
    fi
    printf '%s\n' "$out" | sed 's/^/        /'
}

# kyber_files_hash <racine> <chemin>… : empreinte de fichiers du dépôt, dans
# l'ordre donné, chacun avec son nom (un fichier renommé change l'empreinte).
# Un dossier compte pour ses fichiers suivis par git (sans .git : tous ses
# fichiers), triés ; un chemin absent est sauté.
kyber_files_hash() {
    local root=$1 p
    shift
    (
        cd "$root" || exit 1
        for p in "$@"; do
            if [ -d "$p" ]; then
                if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
                    git ls-files -z -- "$p"
                else
                    find "$p" -type f -print0
                fi | LC_ALL=C sort -z
            elif [ -f "$p" ]; then
                printf '%s\0' "$p"
            fi
        done | while IFS= read -r -d '' p; do
            [ -f "$p" ] || continue     # suivi mais retiré de la copie de travail
            printf '%s\0' "$p"
            cat "$p"
        done
    ) | kyber_sha256
}

# kyber_wrap_hash <kymedia> <nom> : empreinte d'un wrap de meson (le fichier
# <nom>.wrap et ses packagefiles/<nom>), que meson n'applique qu'à
# l'extraction.
kyber_wrap_hash() {
    local km=$1 name=$2 f
    {
        cat "$km/subprojects/$name.wrap"
        find "$km/subprojects/packagefiles/$name" -type f 2>/dev/null | LC_ALL=C sort | while IFS= read -r f; do cat "$f"; done
    } | kyber_sha256
}

# kyber_wrap_refresh <kymedia> <nom> <dossier de compilation> : un sous-projet
# est extrait une fois par meson, avec les fichiers du wrap de ce moment-là
# (0025 et 0026 y ajoutent leurs encodeurs à FFmpeg, Linux 0004 touche Lua).
# Quand le wrap a changé depuis l'extraction, le sous-projet et sa compilation
# sont retirés : meson l'extrait de nouveau. Après la compilation,
# kyber_wrap_mark note le wrap employé.
kyber_wrap_refresh() {
    local km=$1 name=$2 builddir=$3 sub
    sub=$(sed -n 's/^directory *= *//p' "$km/subprojects/$name.wrap" | head -1)
    [ -n "$sub" ] || { echo "$name.wrap sans « directory »" >&2; return 1; }
    [ -d "$km/subprojects/$sub" ] || return 0
    [ "$(cat "$km/subprojects/$sub/.kybernow-wrap" 2>/dev/null)" = "$(kyber_wrap_hash "$km" "$name")" ] && return 0
    echo "==> $sub : wrap changé, nouvelle extraction"
    rm -rf "${km:?}/subprojects/$sub" "${km:?}/$builddir/subprojects/$sub"
}
kyber_wrap_mark() {
    local km=$1 name=$2 sub
    sub=$(sed -n 's/^directory *= *//p' "$km/subprojects/$name.wrap" | head -1)
    [ -d "$km/subprojects/$sub" ] || return 0
    kyber_wrap_hash "$km" "$name" > "$km/subprojects/$sub/.kybernow-wrap"
}
