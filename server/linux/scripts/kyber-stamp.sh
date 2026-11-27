# shellcheck shell=bash
# Empreinte des correctifs de Kyber pour la Station Linux (STLIN-23), lue
# (source) par build-kyber.sh, qui l'écrit dans $OUT/.kybernow-patches après
# la compilation, et par package.sh et run-station.sh, qui refusent un Kyber
# construit avec d'autres correctifs. Comme sur Mac
# (server/macos/scripts/build-kyber.sh) : correctifs communs, ceux des apps
# (apply.sh les repose toujours, le 0027 commun en dépend), ceux de Linux et
# les scripts qui les posent.

# kyber_linux_patches_hash <racine du dépôt> : l'empreinte (sha256, hex).
kyber_linux_patches_hash() {
    local root=$1 sum
    if command -v sha256sum >/dev/null 2>&1; then sum=(sha256sum); else sum=(shasum -a 256); fi
    (cd "$root" && cat server/kyber-patches/*.patch server/kyber-patches/apply.sh apps/macos/kyber-patches/*.patch \
        server/linux/kyber-patches/*.patch server/linux/scripts/build-kyber.sh server/linux/scripts/pyrowave-linux.sh \
        2>/dev/null | "${sum[@]}" | cut -d' ' -f1)
}

# kyber_linux_check_stamp <racine du dépôt> <rootfs de Kyber> : vrai si Kyber
# y a été construit avec les correctifs d'aujourd'hui ; sinon, dit quoi faire.
kyber_linux_check_stamp() {
    local root=$1 out=$2 want have
    want=$(kyber_linux_patches_hash "$root")
    have=$(cat "$out/.kybernow-patches" 2>/dev/null || true)
    [ "$have" = "$want" ] && return 0
    if [ -z "$have" ]; then
        echo "Kyber dans $out : empreinte des correctifs absente (construit avant le 27/11 ?) : relancer server/linux/scripts/build-kyber.sh" >&2
    else
        echo "Kyber dans $out construit avec d'autres correctifs : relancer server/linux/scripts/build-kyber.sh" >&2
    fi
    [ -n "${KYBERNOW_KYBER_STAMP_IGNORE:-}" ] && { echo "    (KYBERNOW_KYBER_STAMP_IGNORE : on continue)" >&2; return 0; }
    return 1
}
