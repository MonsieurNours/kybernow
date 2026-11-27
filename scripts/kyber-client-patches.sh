# shellcheck shell=bash
# Correctifs KyberNow côté client (apps/macos/kyber-patches, puis ceux de
# server/kyber-patches nommés *-kyclient-*, puis ceux propres à une app), posés
# dans les sources de Kyber (third_party/kyber-desktop/kysdk) que partagent les
# builds des apps et de la Station. Une seule liste pour toutes les apps
# (kyber_client_patches, audit MAC-31), lue (source) par
# apps/macos/scripts/build-kyclient.sh, apps/apple/scripts/build-apple.sh et
# build-apple-mobile.sh, apps/tvos/scripts/build-kyber-tvos.sh,
# apps/core/scripts/build-apple.sh et ci-essais.sh,
# apps/android/scripts/build-native.sh, apps/windows/scripts/build.sh,
# apps/linux/scripts/build.sh et apps/webos/scripts/build.sh.
#
#   kyber_client_patches [correctif…]        la liste posée par une app, dans
#                                            l'ordre ; 0025-kyclient-pyrowave
#                                            seulement avec KYBER_PYROWAVE=1
#                                            (défaut) ; puis ceux donnés en plus
#   kyber_client_patch_files [correctif…]    la même, précédée des correctifs de
#                                            la Station posés d'abord : la base
#                                            d'une empreinte
#   kyber_apply_app_patches <kysdk> [correctif…]   pose la liste
#
# Pourquoi tout reposer : les mêmes fichiers reçoivent aussi les correctifs de
# la Station (server/kyber-patches/apply.sh) et ceux d'autres apps. Un
# correctif déjà posé puis recouvert par un suivant n'est ni applicable ni
# retirable tel quel (« 0022 inapplicable ») : le build s'arrêtait selon ce
# qu'un autre build avait laissé. Comme apply.sh pour la Station, on remet
# d'abord à l'origine les fichiers que ces correctifs touchent, puis on les pose
# tous dans l'ordre : le résultat ne dépend plus de l'état trouvé.
#
# Un build de la Station lancé ensuite reprend tout de son côté (apply.sh remet
# ses fichiers à l'origine et repose les siens, puis ceux des apps).

# Le dépôt visé par un correctif, d'après son nom.
kyber_client_repo() {
    local kysdk=$1 patch=$2
    case "$(basename "$patch")" in
        *-kyclient-* | *-kycontroller-* | *-kywebplayer-*) echo "$kysdk/kyctl" ;;
        *-kynput-*) echo "$kysdk/kynput" ;;
        *-kynet-*) echo "$kysdk/kymux" ;;
        *) echo "Dépôt inconnu pour $(basename "$patch")" >&2; return 1 ;;
    esac
}

# kyber_apply_client_patches <kysdk> <correctif>... : remet à l'origine (HEAD
# du dépôt) chaque fichier que touche un correctif connu de kyctl, kymux ou
# kynput (kyber_known_client_patches : ceux du moment et les autres, pour
# qu'un build sans 0025, webOS ou --sans-pyrowave, ne garde pas les fichiers
# que seul 0025 touche après un build avec), puis pose ceux-ci dans l'ordre
# donné. S'arrête (code 1) au premier correctif qui ne se pose pas.
# Les correctifs de la Station pour ces dépôts (kycontroller, kywebplayer,
# kynet, kynput : pas ceux de kyclient, que l'appelant passe, ni ceux du moteur
# web) sont posés d'abord, comme server/kyber-patches/apply.sh : les apps se
# compilent sur le même état que la Station (le stylet de la Station touche
# kynput/Cargo.toml et ajoute des fichiers ; remettre l'un sans les autres
# cassait la compilation de kynput).
KYBER_CLIENT_PATCHES_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=kyber-patches-lib.sh
. "$KYBER_CLIENT_PATCHES_ROOT/scripts/kyber-patches-lib.sh"
kyber_station_patches() {
    local patch
    for patch in "$KYBER_CLIENT_PATCHES_ROOT"/server/kyber-patches/*.patch; do
        case "$(basename "$patch")" in
            *-kycontroller-* | *-kywebplayer-* | *-kynet-* | *-kynput-*) echo "$patch" ;;
        esac
    done
}

# Tous les correctifs connus qui visent kyctl, kymux ou kynput : ceux des apps
# et ceux des Stations (Windows, Mac, Linux), qu'un build les pose ou non.
kyber_known_client_patches() {
    local patch
    for patch in "$KYBER_CLIENT_PATCHES_ROOT"/apps/*/kyber-patches/*.patch "$KYBER_CLIENT_PATCHES_ROOT"/server/*/kyber-patches/*.patch \
        "$KYBER_CLIENT_PATCHES_ROOT"/server/kyber-patches/*.patch; do
        [ -f "$patch" ] || continue
        case "$(basename "$patch")" in
            *-kyclient-* | *-kycontroller-* | *-kywebplayer-* | *-kynput-* | *-kynet-*) echo "$patch" ;;
        esac
    done | awk '!vu[$0]++'
}

# État des dépôts que touchent ces correctifs (commit, modifications,
# fichiers non suivis) : s'il est celui noté après la dernière pose des mêmes
# correctifs, rien n'est réécrit (les dates des fichiers ne bougent pas, cargo
# et meson ne recompilent pas tout).
kyber_client_state() {
    local kysdk=$1 r
    for r in kyctl kymux kynput; do
        echo "== $r"
        git -C "$kysdk/$r" rev-parse HEAD
        git -C "$kysdk/$r" diff HEAD --binary
        (cd "$kysdk/$r" && git ls-files -o --exclude-standard -z | sort -z | while IFS= read -r -d '' f; do
            echo "$f"; [ -f "$f" ] && cat "$f"
        done)
    done | kyber_sha256
}

# kyber_client_patches [correctif…] : les correctifs qu'une app passe à
# kyber_apply_client_patches, dans l'ordre (ceux de la Station, que celle-ci
# pose d'abord, n'y sont pas).
kyber_client_patches() {
    local patch
    for patch in "$KYBER_CLIENT_PATCHES_ROOT"/apps/macos/kyber-patches/*.patch; do echo "$patch"; done
    for patch in "$KYBER_CLIENT_PATCHES_ROOT"/server/kyber-patches/*-kyclient-*.patch; do
        case "$(basename "$patch")" in
            *-kyclient-pyrowave.patch) [ "${KYBER_PYROWAVE:-1}" = 1 ] || continue ;;
        esac
        echo "$patch"
    done
    [ $# -eq 0 ] || printf '%s\n' "$@"
}

# kyber_client_patch_files [correctif…] : tout ce qui est posé, correctifs de
# la Station compris (kynput, kynet, kycontroller, kywebplayer, que
# kyber_apply_client_patches pose d'abord) : la base d'une empreinte
# (apps/macos/scripts/build-kyclient.sh, Vendor/kyclient.stamp).
kyber_client_patch_files() {
    kyber_station_patches
    kyber_client_patches "$@"
}

# kyber_apply_app_patches <kysdk> [correctif…] : pose la liste de
# kyber_client_patches (KYBER_PYROWAVE et les correctifs propres à l'app
# compris). Code 1 au premier correctif qui ne se pose pas.
kyber_apply_app_patches() {
    local kysdk=$1 patch list=()
    shift
    while IFS= read -r patch; do list+=("$patch"); done < <(kyber_client_patches "$@")
    kyber_apply_client_patches "$kysdk" "${list[@]}"
}

# Sources sans git (copie du Flatpak, apps/linux/scripts/sources.sh, prise
# dans third_party/ telle quelle) : ni remise à l'origine ni état noté
# possibles. Chaque correctif, Station d'abord, est passé s'il est déjà posé en
# entier, sinon posé (kyber_patch_put : sans décalage du contexte).
kyber_apply_client_patches_sans_git() {
    local kysdk=$1 patch repo
    shift
    for patch in "$@"; do
        repo="$(kyber_client_repo "$kysdk" "$patch")" || return 1
        kyber_patch_posed "$patch" "$repo" && continue
        echo "==> $(basename "$patch")"
        kyber_patch_put "$patch" "$repo" || return 1
    done
}

kyber_apply_client_patches() {
    local kysdk=$1 patch repo want mark
    shift
    # Une seule construction à la fois sur ces sources (scripts/verrou.sh) :
    # rendu à la sortie du script.
    if [ -f "$KYBER_CLIENT_PATCHES_ROOT/scripts/verrou.sh" ]; then
        # shellcheck source=verrou.sh
        . "$KYBER_CLIENT_PATCHES_ROOT/scripts/verrou.sh"
        kyber_verrou_sortie "$(cd "$kysdk/.." && pwd -P)" || return 1
    fi
    # Chemins absolus : git -C et patch les liraient depuis le dépôt visé.
    local station=() given=()
    for patch in "$@"; do case "$patch" in /*) given+=("$patch") ;; *) given+=("$PWD/$patch") ;; esac; done
    while IFS= read -r patch; do station+=("$patch"); done < <(kyber_station_patches)
    set -- ${station[@]+"${station[@]}"} ${given[@]+"${given[@]}"}
    if ! git -C "$kysdk/kyctl" rev-parse -q --verify HEAD >/dev/null 2>&1; then
        kyber_apply_client_patches_sans_git "$kysdk" "$@"
        return
    fi
    mark="$kysdk/../.kybernow-correctifs-apps"
    want="$(for patch in "$@"; do basename "$patch"; cat "$patch"; done | kyber_sha256)"
    if [ "$(cat "$mark" 2>/dev/null)" = "$want $(kyber_client_state "$kysdk")" ]; then
        echo "==> correctifs déjà posés ($# correctifs, sources inchangées depuis)"
        return 0
    fi
    rm -f "$mark"
    local known=()
    while IFS= read -r patch; do known+=("$patch"); done < <(kyber_known_client_patches)
    for patch in ${known[@]+"${known[@]}"} "$@"; do
        repo="$(kyber_client_repo "$kysdk" "$patch")" || return 1
        kyber_patch_reset "$repo" "$patch" || return 1
    done
    for patch in "$@"; do
        repo="$(kyber_client_repo "$kysdk" "$patch")" || return 1
        if ! git -C "$repo" apply "$patch"; then
            echo "Correctif $(basename "$patch") inapplicable sur l'origine de $(basename "$repo")" >&2
            return 1
        fi
        echo "==> $(basename "$patch") posé"
    done
    echo "$want $(kyber_client_state "$kysdk")" > "$mark"
}

# libkynput installée dans un rootfs : refaite quand ses sources changent
# (correctifs posés ou retirés, autre version de kynput), pas seulement quand
# elle manque. Sans cela, une libkynput d'avant un correctif (le stylet, 0005)
# restait en place et kynput-rs ne compilait plus contre elle.
# kyber_kynput_stamp <kysdk> : empreinte des sources de kynput telles qu'elles sont.
kyber_kynput_stamp() {
    local repo="$1/kynput"
    {
        git -C "$repo" rev-parse HEAD
        git -C "$repo" diff HEAD
        (cd "$repo" && git ls-files -o --exclude-standard -z | xargs -0 cat 2>/dev/null) || true
    } | kyber_sha256
}
# kyber_kynput_current <prefix> <empreinte> : vrai si la libkynput de ce rootfs en vient.
kyber_kynput_current() {
    [ -f "$1/lib/pkgconfig/kynput.pc" ] && [ "$(cat "$1/lib/pkgconfig/.kybernow-kynput" 2>/dev/null)" = "$2" ]
}
# kyber_kynput_mark <prefix> <empreinte> : note l'empreinte après l'installation.
kyber_kynput_mark() {
    echo "$2" > "$1/lib/pkgconfig/.kybernow-kynput"
}
