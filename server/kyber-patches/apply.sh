# shellcheck shell=bash
# Pose des correctifs serveur de KyberNow sur les sources de Kyber, commune à
# build.sh (Windows) et à server/macos/scripts/build-kyber.sh (Mac) : mêmes
# correctifs, même ordre, pour les deux systèmes.
#
# À sourcer après avoir défini HERE (ce dossier), ROOT (racine du dépôt),
# KYMEDIA, KYCTL, KYMUX, KYNPUT et, pour le moteur web, KYWEB (vide ou absent :
# les correctifs *-kyberweb-* sont laissés de côté). Puis appeler
# kyber_apply_server_patches.

kyber_apply_server_patches() {
    echo "==> Correctifs"
    # Les correctifs se suivent et touchent parfois les mêmes fichiers : on part
    # toujours des sources d'origine, puis on les applique tous, dans l'ordre.
    # Ceux nommés *-kycontroller-*, *-kyclient-* et *-kywebplayer-* visent kysdk/kyctl,
    # *-kynet-* kysdk/kymux, *-kynput-* kysdk/kynput, *-kyberweb-* kyber-web, les
    # autres kysdk/kymedia. Seuls les fichiers des correctifs sont remis à
    # l'origine : kyctl et kymux portent aussi les correctifs des apps.
    # *-kyctlweb-* ne vise que la copie de kyctl du moteur web (plus bas), jamais
    # le kyctl partagé : ils touchent kyclient (lib.rs, enums.rs, host_service.rs),
    # comme le correctif kyclient des apps (apps/macos/kyber-patches/0001-kyclient-…),
    # qui ne s'y reposerait plus. Ici, repo_of ne rend rien pour eux.
    repo_of() {
            case "$(basename "$1")" in
            *-kyctlweb-*) ;;
            *-kycontroller-* | *-kyclient-* | *-kywebplayer-*) echo "$KYCTL" ;;
            *-kyberweb-*) echo "${KYWEB:-}" ;;
            *-kynet-*) echo "$KYMUX" ;;
            *-kynput-*) echo "$KYNPUT" ;;
            *) echo "$KYMEDIA" ;;
        esac
    }
    # Piège : les correctifs des apps (apps/macos, apps/webos) posés par leurs
    # scripts dans kyctl, kymux et kynput touchent parfois les mêmes fichiers
    # (kynput/Cargo.toml : 0013 et le correctif webOS). Remis à l'origine, ces
    # fichiers perdraient leur part, et le script de l'app trouverait ensuite son
    # correctif à moitié posé, ni applicable ni retirable. Ceux qui sont posés en
    # entier sont donc retirés d'abord (du dernier au premier), puis reposés après
    # ceux du serveur.
    client_repo_of() {
            case "$(basename "$1")" in
            *-kyclient-*) echo "$KYCTL" ;;
            *-kynput-*) echo "$KYNPUT" ;;
            *-kynet-*) echo "$KYMUX" ;;
        esac
    }
    CLIENT_APPLIED=()
    for p in "$ROOT"/apps/macos/kyber-patches/*.patch "$ROOT"/apps/webos/kyber-patches/*.patch; do
        repo=$(client_repo_of "$p")
        [ -n "$repo" ] && git -C "$repo" apply --reverse --check "$p" 2>/dev/null && CLIENT_APPLIED=("$p" ${CLIENT_APPLIED[@]+"${CLIENT_APPLIED[@]}"})
    done
    for p in ${CLIENT_APPLIED[@]+"${CLIENT_APPLIED[@]}"}; do
        git -C "$(client_repo_of "$p")" apply --reverse "$p"
    done
    for p in "$HERE"/*.patch; do
        repo=$(repo_of "$p")
        [ -n "$repo" ] || continue
        for f in $(grep -h '^+++ b/' "$p" | sed 's|^+++ b/||; s|[[:space:]].*||'); do
            case "$f" in
                subprojects/txproto/*) dir="$repo/subprojects/txproto" f="${f#subprojects/txproto/}" ;;
                subprojects/libavconv/*) dir="$repo/subprojects/libavconv" f="${f#subprojects/libavconv/}" ;;
                *) dir="$repo" ;;
            esac
            if git -C "$dir" cat-file -e "HEAD:$f" 2>/dev/null; then
                git -C "$dir" checkout -q HEAD -- "$f"
            else
                rm -f "$dir/$f"     # ajouté par le correctif : absent de l'origine
            fi
        done
    done
    for p in "$HERE"/*.patch; do
        repo=$(repo_of "$p")
        [ -n "$repo" ] || continue
        echo "    $(basename "$p")"
        (cd "$repo" && patch -p1 --forward --no-backup-if-mismatch -r - < "$p" | sed 's/^/        /') || {
            echo "Correctif $(basename "$p") inapplicable (hunk en échec ci-dessus)" >&2
            exit 1
        }
    done
    REAPPLY=()
    for p in ${CLIENT_APPLIED[@]+"${CLIENT_APPLIED[@]}"}; do REAPPLY=("$p" ${REAPPLY[@]+"${REAPPLY[@]}"}); done
    for p in ${REAPPLY[@]+"${REAPPLY[@]}"}; do
        repo=$(client_repo_of "$p")
        if git -C "$repo" apply --check "$p" 2>/dev/null; then
            git -C "$repo" apply "$p" && echo "    $(basename "$p") (app) reposé"
        elif ! git -C "$repo" apply --reverse --check "$p" 2>/dev/null; then
            echo "Correctif d'app $(basename "$p") à moitié posé : remettre $repo à l'origine (git -C $repo checkout -- .), puis relancer" >&2
            exit 1
        fi
    done
}
