# shellcheck shell=bash
# Correctifs KyberNow côté client (apps/macos/kyber-patches, puis ceux de
# server/kyber-patches nommés *-kyclient-*, puis ceux propres à une app), posés
# dans les sources de Kyber (third_party/kyber-desktop/kysdk) que partagent les
# builds des apps et de la Station. Lu (source) par build-kyclient.sh,
# build-kyber-tvos.sh, apps/core/scripts/build-apple.sh, build-apple-mobile.sh,
# apps/android/scripts/build-native.sh, apps/windows/scripts/build.sh et
# apps/webos/scripts/build.sh.
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
# du dépôt) chaque fichier que ces correctifs touchent, puis les pose dans
# l'ordre donné. S'arrête (code 1) au premier correctif qui ne se pose pas.
# Les correctifs de la Station pour ces dépôts (kycontroller, kywebplayer,
# kynet, kynput : pas ceux de kyclient, que l'appelant passe, ni ceux du moteur
# web) sont posés d'abord, comme server/kyber-patches/apply.sh : les apps se
# compilent sur le même état que la Station (le stylet de la Station touche
# kynput/Cargo.toml et ajoute des fichiers ; remettre l'un sans les autres
# cassait la compilation de kynput).
KYBER_CLIENT_PATCHES_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
kyber_station_patches() {
    local patch
    for patch in "$KYBER_CLIENT_PATCHES_ROOT"/server/kyber-patches/*.patch; do
        case "$(basename "$patch")" in
            *-kycontroller-* | *-kywebplayer-* | *-kynet-* | *-kynput-*) echo "$patch" ;;
        esac
    done
}

kyber_apply_client_patches() {
    local kysdk=$1 patch repo f
    shift
    local station=()
    while IFS= read -r patch; do station+=("$patch"); done < <(kyber_station_patches)
    set -- ${station[@]+"${station[@]}"} "$@"
    for patch in "$@"; do
        repo="$(kyber_client_repo "$kysdk" "$patch")" || return 1
        for f in $(grep -h '^+++ b/' "$patch" | sed 's|^+++ b/||; s|[[:space:]].*||'); do
            if git -C "$repo" cat-file -e "HEAD:$f" 2>/dev/null; then
                git -C "$repo" checkout -q HEAD -- "$f"
            else
                # Ajouté par un correctif : absent de l'origine.
                git -C "$repo" rm -q --cached --ignore-unmatch -- "$f" >/dev/null
                rm -f "$repo/$f"
            fi
        done
    done
    for patch in "$@"; do
        repo="$(kyber_client_repo "$kysdk" "$patch")" || return 1
        if ! git -C "$repo" apply "$patch"; then
            echo "Correctif $(basename "$patch") inapplicable sur l'origine de $(basename "$repo")" >&2
            return 1
        fi
        echo "==> $(basename "$patch") posé"
    done
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
    } | { if command -v shasum >/dev/null; then shasum -a 256; else sha256sum; fi; } | cut -d' ' -f1
}
# kyber_kynput_current <prefix> <empreinte> : vrai si la libkynput de ce rootfs en vient.
kyber_kynput_current() {
    [ -f "$1/lib/pkgconfig/kynput.pc" ] && [ "$(cat "$1/lib/pkgconfig/.kybernow-kynput" 2>/dev/null)" = "$2" ]
}
# kyber_kynput_mark <prefix> <empreinte> : note l'empreinte après l'installation.
kyber_kynput_mark() {
    echo "$2" > "$1/lib/pkgconfig/.kybernow-kynput"
}
