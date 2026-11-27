# shellcheck shell=bash
# Pose des correctifs serveur de KyberNow sur les sources de Kyber, commune aux
# trois Stations : build.sh (Windows), server/macos/scripts/build-kyber.sh
# (Mac) et server/linux/scripts/build-kyber.sh (Linux). Mêmes correctifs
# communs, même ordre ; puis, pour Mac et Linux, leurs correctifs propres.
# Aussi la liste de ce qui entre dans l'empreinte de chaque Station
# (kyber_station_hash), lue par stamp.sh (Windows), build-kyber.sh (Mac),
# kyber-stamp.sh (Linux) et scripts/release.sh : une seule liste (STCOM-11).
#
# Les primitives (remise à l'origine, pose sans décalage du contexte, wraps de
# meson) sont dans scripts/kyber-patches-lib.sh.
#
# À sourcer après avoir défini HERE (ce dossier), ROOT (racine du dépôt),
# KYMEDIA, KYCTL, KYMUX, KYNPUT et, pour le moteur web, KYWEB (vide ou absent :
# les correctifs *-kyberweb-* sont laissés de côté). Puis appeler
# kyber_apply_server_patches, et, pour Mac et Linux,
# kyber_apply_own_patches. kyber_station_inputs et kyber_station_hash ne
# demandent que la racine du dépôt.

# shellcheck source=../../scripts/kyber-patches-lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/kyber-patches-lib.sh"

# Le dépôt visé par un correctif serveur, d'après son nom : *-kycontroller-*,
# *-kyclient-* et *-kywebplayer-* visent kysdk/kyctl, *-kynet-* kysdk/kymux,
# *-kynput-* kysdk/kynput, *-kyberweb-* kyber-web, les autres kysdk/kymedia.
# *-kyctlweb-* ne vise que la copie de kyctl du moteur web (build.sh), jamais le
# kyctl partagé : ils touchent kyclient (lib.rs, enums.rs, host_service.rs),
# comme le correctif kyclient des apps (apps/macos/kyber-patches/0001-kyclient-…),
# qui ne s'y reposerait plus. Rien n'est rendu pour eux.
kyber_server_repo() {
    case "$(basename "$1")" in
        *-kyctlweb-*) ;;
        *-kycontroller-* | *-kyclient-* | *-kywebplayer-*) echo "$KYCTL" ;;
        *-kyberweb-*) echo "${KYWEB:-}" ;;
        *-kynet-*) echo "$KYMUX" ;;
        *-kynput-*) echo "$KYNPUT" ;;
        *) echo "$KYMEDIA" ;;
    esac
}

# Le dépôt visé par un correctif des apps (apps/macos, apps/webos).
kyber_app_repo() {
    case "$(basename "$1")" in
        *-kyclient-*) echo "$KYCTL" ;;
        *-kynput-*) echo "$KYNPUT" ;;
        *-kynet-*) echo "$KYMUX" ;;
    esac
}

# Les correctifs des apps (apps/macos, apps/webos) : les scripts des apps les
# posent dans les mêmes kyctl, kymux et kynput.
kyber_app_patches() {
    local p
    for p in "$ROOT"/apps/macos/kyber-patches/*.patch "$ROOT"/apps/webos/kyber-patches/*.patch; do
        [ -f "$p" ] && echo "$p"
    done
}

kyber_apply_server_patches() {
    # L'ordre de pose est celui des noms, octet par octet, quelle que soit la
    # langue du système : un même numéro est une famille (un correctif par
    # dépôt ou par moteur), et 0085-kycontroller-micro-droits se pose sur
    # 0085-kycontroller-flux-montant (« f » avant « m »).
    local LC_ALL=C
    local p repo
    echo "==> Correctifs"
    # On part toujours des sources d'origine : les fichiers de tous les
    # correctifs qui visent ces dépôts sont remis à l'origine, ceux des
    # correctifs communs et ceux des apps, qui touchent parfois les mêmes
    # (kyclient : 0001 des apps et les *-kyclient-* communs ; kynput/Cargo.toml :
    # 0013 et le correctif webOS). Seuls ces fichiers-là : kyctl et kymux
    # portent aussi ce que les apps y ajoutent. Ensuite, les correctifs communs
    # dans l'ordre, puis toujours ceux de apps/macos : le correctif commun du
    # stylet (0027-kynput) s'appuie sur celui des apps (0005-kynput, Pen). Ceux
    # de webOS ne sont pas reposés : les scripts des apps reposent tous les
    # leurs (scripts/kyber-client-patches.sh). Avant le 27/11, les correctifs
    # des apps posés en entier étaient retirés puis reposés, ceux à moitié
    # posés arrêtaient tout : une seconde construction sur les mêmes sources
    # trouvait 0001 « à moitié posé » (recouvert par les *-kyclient-*).
    for p in "$HERE"/*.patch; do
        repo=$(kyber_server_repo "$p")
        [ -n "$repo" ] || continue
        kyber_patch_reset "$repo" "$p" || exit 1
    done
    while IFS= read -r p; do
        repo=$(kyber_app_repo "$p")
        [ -n "$repo" ] || { echo "Dépôt inconnu pour le correctif d'app $(basename "$p")" >&2; exit 1; }
        kyber_patch_reset "$repo" "$p" || exit 1
    done < <(kyber_app_patches)
    for p in "$HERE"/*.patch; do
        repo=$(kyber_server_repo "$p")
        [ -n "$repo" ] || continue
        echo "    $(basename "$p")"
        kyber_patch_put "$p" "$repo" || exit 1
    done
    for p in "$ROOT"/apps/macos/kyber-patches/*.patch; do
        repo=$(kyber_app_repo "$p")
        git -C "$repo" apply "$p" || { echo "Correctif d'app $(basename "$p") inapplicable après les correctifs communs" >&2; exit 1; }
        echo "    $(basename "$p") (app)"
    done
}

# kyber_apply_own_patches <dossier> <libellé> : les correctifs propres à une
# Station (server/macos/kyber-patches, server/linux/kyber-patches), même
# nommage que les communs, posés après eux dans l'ordre de leur nom. Leurs
# fichiers sont remis à l'origine une fois pour toutes, avant le premier (deux
# correctifs propres peuvent toucher le même fichier : Mac 0003 et 0004,
# mod.rs) ; un fichier déjà touché par un correctif commun ou d'app (remis à
# l'origine puis corrigé par kyber_apply_server_patches) reste tel quel.
kyber_apply_own_patches() {
    local LC_ALL=C
    local dir=$1 label=$2 p
    local own=()
    for p in "$dir"/*.patch; do [ -f "$p" ] && own+=("$p"); done
    [ ${#own[@]} -gt 0 ] || return 0
    local common=("$HERE"/*.patch)
    while IFS= read -r p; do common+=("$p"); done < <(kyber_app_patches)
    for p in "${own[@]}"; do
        kyber_patch_reset "$(kyber_server_repo "$p")" "$p" "${common[@]}" || exit 1
    done
    for p in "${own[@]}"; do
        echo "    $(basename "$p") ($label)"
        kyber_patch_put "$p" "$(kyber_server_repo "$p")" || exit 1
    done
}

# kyber_station_inputs <windows|mac|linux> : ce qui entre dans l'empreinte du
# Kyber d'une Station, chemins relatifs à la racine du dépôt, dans l'ordre :
# correctifs communs et ceux des apps (kyber_apply_server_patches repose
# toujours ceux de apps/macos, le 0027 commun en dépend ; ceux de webOS quand
# ils étaient posés), les scripts qui les posent, la chaîne d'outils, le commit
# de PyroWave (apps/macos/PyroWave/COMMIT : l'encodeur de chaque Station en
# vient), puis ce qui est propre à la Station. Les fichiers de cette liste sont
# aussi dans les entrées de son étape (scripts/release-entrees.sh).
kyber_station_inputs() {
    local LC_ALL=C
    case "$1" in
        windows | mac | linux) ;;
        *) echo "kyber_station_inputs : Station inconnue « $1 » (windows, mac ou linux)" >&2; return 1 ;;
    esac
    printf '%s\n' server/kyber-patches/*.patch server/kyber-patches/apply.sh scripts/kyber-patches-lib.sh \
        scripts/kyber-toolchain.sh apps/macos/kyber-patches/*.patch apps/webos/kyber-patches/*.patch \
        apps/macos/PyroWave/COMMIT
    case "$1" in
        # Surcouche de la Station Windows (third_party/payload/kyber-overlay) :
        # aussi le moteur web, PyroWave pour Windows et les aides compilées dans
        # la surcouche (micro, caméra, essai de l'encodeur ; en 0.11.7, celles du
        # micro et de la caméra corrigées sont parties dans leur ancienne
        # version, faute d'y être).
        windows) printf '%s\n' server/kyber-patches/build.sh server/kyber-patches/pyrowave.sh \
            server/micro server/camera server/encoder-check ;;
        # Kyber pour Mac : ses correctifs, ses crates corrigées, l'encodeur
        # PyroWave Metal (server/macos/pyrowave) et le port Metal qu'il compile
        # (apps/macos/PyroWave/upstream, audit TRANS-03).
        mac) printf '%s\n' server/macos/kyber-patches/*.patch server/macos/crates/*.patch \
            server/macos/scripts/build-kyber.sh server/macos/pyrowave apps/macos/PyroWave ;;
        linux) printf '%s\n' server/kyber-patches/pyrowave.sh server/linux/kyber-patches/*.patch \
            server/linux/scripts/build-kyber.sh server/linux/scripts/pyrowave-linux.sh ;;
    esac
}

# kyber_station_hash <racine du dépôt> <windows|mac|linux> : l'empreinte.
kyber_station_hash() {
    local list=() p
    kyber_station_inputs "$2" > /dev/null || return 1
    while IFS= read -r p; do list+=("$p"); done < <(cd "$1" && kyber_station_inputs "$2")
    kyber_files_hash "$1" "${list[@]}"
}
