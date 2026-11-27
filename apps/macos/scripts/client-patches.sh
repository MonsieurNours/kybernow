# shellcheck shell=bash
# Correctifs de Kyber des apps Apple, une seule liste (MAC-03, MAC-31) : lue
# par build-kyclient.sh (pose et empreinte de Vendor/kyclient.stamp), par
# apps/apple/scripts/build-apple.sh et build-apple-mobile.sh (le cœur compile
# contre ces sources). Suppose scripts/kyber-client-patches.sh déjà lu.
#
#   kyber-patches/*.patch                      (ce dossier sert à toutes les apps)
#   server/kyber-patches/*-kyclient-*.patch    (côté client des correctifs de la
#                                               Station ; 0025-kyclient-pyrowave
#                                               seulement avec KYBER_PYROWAVE=1)

# mac_client_patches <racine du dépôt> : les correctifs que l'appelant passe à
# kyber_apply_client_patches, dans l'ordre.
mac_client_patches() {
    local root=$1 patch
    for patch in "$root"/apps/macos/kyber-patches/*.patch; do echo "$patch"; done
    for patch in "$root"/server/kyber-patches/*-kyclient-*.patch; do
        case "$(basename "$patch")" in
            *-kyclient-pyrowave.patch) [ "${KYBER_PYROWAVE:-1}" = 1 ] || continue ;;
        esac
        echo "$patch"
    done
}

# mac_client_patch_files <racine> : tout ce qui est posé, correctifs de la
# Station compris (kynput, kynet, kycontroller, kywebplayer, que
# kyber_apply_client_patches pose d'abord) : la base de l'empreinte.
mac_client_patch_files() {
    kyber_station_patches
    mac_client_patches "$1"
}
