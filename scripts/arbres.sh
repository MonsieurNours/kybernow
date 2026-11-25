#!/bin/bash
# shellcheck shell=bash
# Arbres de construction : une copie du dépôt et des sources de Kyber par
# variante de construction, gardée d'une publication à l'autre (caches de
# cargo, de meson, d'Xcode), pour que les constructions ne se gênent plus en
# corrigeant chacune à sa façon les mêmes sources (docs/build-notes.md,
# « Arbres de construction »).
#
#   scripts/arbres.sh liste                      arbres, commit, verrous, taille
#   scripts/arbres.sh preparer <variante> [REV]  crée ou remet l'arbre au commit REV
#                                                (défaut HEAD), sources de Kyber
#                                                remises à l'origine si besoin
#   scripts/arbres.sh chemin <variante>          chemin de l'arbre
#
# Aussi lu (source) par scripts/release.sh.
#
# Un arbre est un worktree git détaché (third_party/arbres/<variante>) au
# commit construit, dont le dossier third_party/ est à lui : les entrées que la
# variante écrit (kyber-desktop, rootfs…) sont des copies APFS (cp -c, sans
# place sur le disque tant qu'elles ne changent pas), copiées une fois depuis
# le third_party/ du dépôt principal sans leurs dossiers de compilation (ils
# désignent les chemins d'origine), les autres des liens relatifs vers le
# third_party/ partagé (lus seulement ; relatifs pour la VM, qui voit le dépôt
# en /media/psf/kyber-apps).
#
# Sources de Kyber : remises à l'origine exacte (fichiers suivis, fichiers
# ajoutés par un correctif) au début d'une construction, sauf si elles sont
# encore dans l'état noté à la fin de la précédente avec les mêmes correctifs
# (.kybernow-arbre) : rien n'est réécrit, les caches restent valables. Les
# scripts de compilation posent ensuite leurs correctifs, comme à la main.

ARBRES_MAIN="$(cd "$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --git-common-dir)/.." && pwd -P)"
ARBRES_TP="$(cd "$ARBRES_MAIN/third_party" 2>/dev/null && pwd -P)" || ARBRES_TP="$ARBRES_MAIN/third_party"
ARBRES="${KYBERNOW_ARBRES:-$ARBRES_TP/arbres}"
# Le dépôt principal vu de la VM « KyberNow Linux » (dossier partagé de Parallels).
ARBRES_VM_MAIN="${KYBERNOW_LINUX_REPO_MAIN:-/media/psf/kyber-apps}"
# shellcheck source=verrou.sh
. "$(dirname "${BASH_SOURCE[0]}")/verrou.sh"

ARBRES_VARIANTES="station-windows station-mac apple windows-app android webos linux versions"

# Entrées de third_party/ propres à une variante (écrites par ses
# constructions) ; « a/b » : a est un dossier à l'arbre, b y est propre, le
# reste de a est lié.
arbre_possedes() {
    case "$1" in
        station-windows) echo "kyber-desktop kyber-web kyber-web-kysdk kyber-web-out rootfs-win64 rootfs-win64-kynput payload/kyber-overlay payload/kyber-overlay.stamp pyrowave toolchain-bin" ;;
        station-mac) echo "kyber-desktop-macos" ;;
        apple) echo "kyber-desktop" ;;
        windows-app) echo "kyber-desktop windows-client-x64 windows-client-arm64 pyrowave" ;;
        android) echo "kyber-desktop pyrowave" ;;
        webos) echo "kyber-desktop sdl-webos sdl-webos-dist" ;;
        linux) echo "kyber-desktop linux-aarch64 linux-aarch64-rpi pyrowave" ;;
        versions) echo "" ;;
        *) echo "variante inconnue : $1 ($ARBRES_VARIANTES)" >&2; return 1 ;;
    esac
}
# Entrées liées à celles d'un autre arbre (produites par lui) : « entrée:arbre ».
arbre_liens_arbres() {
    case "$1" in
        windows-app) echo "rootfs-win64:station-windows payload:station-windows" ;;
        station-mac) echo "kyber-web-out:station-windows" ;;
        *) echo "" ;;
    esac
}
# Sources de Kyber (dépôts git corrigés) de la variante.
arbre_sources_kyber() {
    case "$1" in
        station-windows) echo "kyber-desktop kyber-web" ;;
        station-mac) echo "kyber-desktop-macos" ;;
        versions) echo "" ;;
        *) echo "kyber-desktop" ;;
    esac
}

arbre_dir() { echo "$ARBRES/$1"; }
# Chemin d'un fichier de l'arbre vu de la VM.
arbre_vm() { echo "$ARBRES_VM_MAIN${1#"$ARBRES_MAIN"}"; }

relpath() { python3 -c 'import os,sys; print(os.path.relpath(sys.argv[1], sys.argv[2]))' "$1" "$2"; }

# Copie APFS d'une entrée de third_party/ dans un arbre, sans ce qui désigne
# les chemins d'origine (dossiers de compilation de cargo, meson, CMake) ni les
# empreintes qui feraient croire à une compilation déjà faite ici ; chemins des
# fichiers .pc, .la et CMake réécrits vers la copie.
arbre_copier() {
    local src=$1 dst=$2 vsrc vdst
    rm -rf "$dst.tmp"
    cp -c -R "$src" "$dst.tmp" 2>/dev/null || cp -R "$src" "$dst.tmp"
    if [ -d "$dst.tmp" ]; then
        find "$dst.tmp" -maxdepth 4 \( -name target -o -name 'builddir*' -o -name kynput-build \) -type d -prune -exec rm -rf {} + 2>/dev/null || true
        case "$(basename "$src")" in
            pyrowave) find "$dst.tmp" -maxdepth 1 -type d -name 'build-*' -exec rm -rf {} + ;;
            linux-*) rm -rf "$dst.tmp/build/kynput" ;;
            sdl-webos) rm -rf "$dst.tmp/build" ;;
            kyber-desktop | kyber-desktop-macos)
                rm -rf "$dst.tmp/Kyber-Client.app.zip" "$dst.tmp/KyberNow-Station-Kyber.app"
                find "$dst.tmp" -maxdepth 3 -name .kybernow-kynput -delete
                rm -f "$dst.tmp"/rootfs-*/.kybernow-patches "$dst.tmp/.kybernow-arbre" "$dst.tmp/kysdk/.kybernow-correctifs" ;;
        esac
        find "$dst.tmp" -maxdepth 1 -name '*.verrou' -exec rm -rf {} +
        vsrc="$(arbre_vm "$src")" vdst="$(arbre_vm "$dst")"
        find "$dst.tmp" -type f \( -name '*.pc' -o -name '*.la' -o -name '*.cmake' -o -name '*-config' \) -print0 2>/dev/null \
            | xargs -0 grep -lF -e "$src" -e "$vsrc" 2>/dev/null \
            | while IFS= read -r f; do
                sed -i '' -e "s|$src|$dst|g" -e "s|$vsrc|$vdst|g" "$f"
            done || true
    else
        rm -f "$dst.tmp"
        cp -c "$src" "$dst.tmp" 2>/dev/null || cp "$src" "$dst.tmp"
        case "$(basename "$src")" in *.stamp) : > "$dst.tmp" ;; esac
    fi
    mv "$dst.tmp" "$dst"
}

# Liens et copies de third_party/ d'un arbre (refaits à chaque préparation :
# une nouvelle entrée du third_party/ partagé y apparaît).
arbre_third_party() {
    local v=$1 dir tp e name own owned links target sub
    dir="$(arbre_dir "$v")"
    tp="$dir/third_party"
    owned=" $(arbre_possedes "$v") "
    links=" $(arbre_liens_arbres "$v") "
    [ -L "$tp" ] && rm -f "$tp"
    mkdir -p "$tp"
    for e in "$ARBRES_TP"/*; do
        name="$(basename "$e")"
        case "$name" in arbres | *.log | *.orig | *.verrou | *.tmp) continue ;; esac
        target=""
        for l in $links; do [ "${l%%:*}" = "$name" ] && target="$(arbre_dir "${l#*:}")/third_party/$name"; done
        if [ -n "$target" ]; then
            [ -L "$tp/$name" ] || rm -rf "${tp:?}/$name"
            ln -sfn "$(relpath "$target" "$tp")" "$tp/$name"
        elif [[ "$owned" == *" $name "* ]]; then
            [ -L "$tp/$name" ] && rm -f "$tp/$name"
            [ -e "$tp/$name" ] || { echo "    $v : copie de third_party/$name" >&2; arbre_copier "$e" "$tp/$name"; }
        elif [[ "$owned" == *" $name/"* ]]; then
            # Dossier à l'arbre : ses entrées propres copiées, les autres liées.
            [ -L "$tp/$name" ] && rm -f "$tp/$name"
            mkdir -p "$tp/$name"
            for sub in "$e"/*; do
                own="$name/$(basename "$sub")"
                if [[ "$owned" == *" $own "* ]]; then
                    [ -e "$tp/$own" ] || [ -L "$tp/$own" ] || arbre_copier "$sub" "$tp/$own"
                else
                    ln -sfn "$(relpath "$sub" "$tp/$name")" "$tp/$own"
                fi
            done
        else
            [ -e "$tp/$name" ] && [ ! -L "$tp/$name" ] && continue
            ln -sfn "$(relpath "$e" "$tp")" "$tp/$name"
        fi
    done
}

# Correctifs connus de Kyber (tous : la Station, les apps, le Mac) : leur
# empreinte décide d'une remise à l'origine, leurs fichiers ajoutés sont ceux
# qu'une remise à l'origine retire.
arbre_correctifs() {
    local root=$1
    ls "$root"/server/kyber-patches/*.patch "$root"/apps/macos/kyber-patches/*.patch \
        "$root"/apps/webos/kyber-patches/*.patch "$root"/server/macos/kyber-patches/*.patch \
        "$root"/server/macos/crates/*.patch 2>/dev/null
}
arbre_correctifs_empreinte() {
    local root=$1 p
    for p in $(arbre_correctifs "$root"); do echo "${p#"$root"/}"; cat "$p"; done | shasum -a 256 | cut -d' ' -f1
}

# Dépôts git corrigés d'un dossier de sources de Kyber.
arbre_depots() {
    local kd=$1 r
    case "$(basename "$kd")" in
        kyber-web) echo "$kd"; return ;;
    esac
    for r in kysdk kysdk/kyctl kysdk/kymux kysdk/kynput kysdk/kymedia kysdk/kymedia/subprojects/txproto kysdk/kymedia/subprojects/libavconv; do
        [ -e "$kd/$r/.git" ] && echo "$kd/$r"
    done
}

# État exact des sources : commit, modifications, fichiers non suivis (hors ignorés).
arbre_sources_etat() {
    local kd=$1 r
    for r in $(arbre_depots "$kd"); do
        echo "== ${r#"$kd"}"
        git -C "$r" rev-parse HEAD
        git -C "$r" diff HEAD --binary --ignore-submodules=all
        (cd "$r" && git ls-files -o --exclude-standard -z -- . ':!.kybernow-*' | sort -z | while IFS= read -r -d '' f; do
            [ -L "$f" ] && { echo "lien $f -> $(readlink "$f")"; continue; }
            [ -f "$f" ] && { echo "fichier $f"; shasum -a 256 < "$f"; }
        done)
    done | shasum -a 256 | cut -d' ' -f1
}

# Remise à l'origine : fichiers suivis tels que dans le commit, et fichiers
# ajoutés par un correctif connu retirés (pas d'autre nettoyage : les
# sous-projets extraits par meson ne sont pas suivis).
arbre_sources_origine() {
    local kd=$1 root=$2 r ajoutes f rel chemin
    ajoutes="$(for p in $(arbre_correctifs "$root"); do
        awk '/^--- \/dev\/null/ {n=1; next} n && /^\+\+\+ b\// {sub(/^\+\+\+ b\//, ""); sub(/[ \t].*/, ""); print} {n=0}' "$p"
    done | sort -u)"
    # kyber-web : son sous-module kysdk remplacé par un lien (server/kyber-patches/build.sh).
    if [ "$(basename "$kd")" = kyber-web ] && [ -L "$kd/kysdk" ]; then rm -f "$kd/kysdk"; mkdir -p "$kd/kysdk"; fi
    for r in $(arbre_depots "$kd"); do
        git -C "$r" checkout -q -- . 2>/dev/null || git -C "$r" checkout -q HEAD -- .
        rel="${r#"$kd"/}"
        while IFS= read -r f; do
            [ -n "$f" ] || continue
            for chemin in "$f" "${rel#kysdk/kymedia/}/$f"; do
                if grep -qxF "$chemin" <<< "$ajoutes"; then rm -f "$r/$f"; break; fi
            done
        done < <(cd "$r" && git ls-files -o --exclude-standard -- . ':!.kybernow-*')
    done
}

# arbre_kyber_pret <variante> : sources de Kyber de l'arbre à l'origine, sauf
# si elles sont dans l'état noté avec les mêmes correctifs.
arbre_kyber_pret() {
    local v=$1 dir e kd want have
    dir="$(arbre_dir "$v")"
    want="$(arbre_correctifs_empreinte "$dir")"
    for e in $(arbre_sources_kyber "$v"); do
        kd="$dir/third_party/$e"
        [ -d "$kd" ] || continue
        have="$(cat "$kd/.kybernow-arbre" 2>/dev/null || true)"
        if [ -n "$have" ] && [ "$have" = "$want $(arbre_sources_etat "$kd")" ]; then
            echo "    $v : sources de $e dans l'état noté (rien à remettre)"
        else
            echo "    $v : sources de $e remises à l'origine"
            rm -f "$kd/.kybernow-arbre"
            arbre_sources_origine "$kd" "$dir"
        fi
    done
}

# arbre_kyber_noter <variante> : note l'état des sources après une construction réussie.
arbre_kyber_noter() {
    local v=$1 dir e kd want
    dir="$(arbre_dir "$v")"
    want="$(arbre_correctifs_empreinte "$dir")"
    for e in $(arbre_sources_kyber "$v"); do
        kd="$dir/third_party/$e"
        [ -d "$kd" ] && echo "$want $(arbre_sources_etat "$kd")" > "$kd/.kybernow-arbre"
    done
    return 0
}

# arbre_preparer <variante> <commit> : l'arbre au commit (créé au besoin),
# third_party/ refait, sources de Kyber prêtes. L'appelant tient le verrou de
# l'arbre (arbre_verrou).
arbre_preparer() {
    local v=$1 rev=$2 dir
    arbre_possedes "$v" > /dev/null || return 1
    dir="$(arbre_dir "$v")"
    mkdir -p "$ARBRES"
    if [ ! -e "$dir/.git" ]; then
        echo "    $v : nouvel arbre ($dir)" >&2
        git -C "$ARBRES_MAIN" worktree prune
        git -C "$ARBRES_MAIN" worktree add -q --detach --force "$dir" "$rev"
    else
        git -C "$dir" checkout -q --detach --force "$rev"
        git -C "$dir" clean -fdq
    fi
    arbre_third_party "$v"
    # Fichiers locaux (ignorés par git) dont une construction a besoin.
    if [ -f "$ARBRES_MAIN/apps/android/local.properties" ] && [ ! -f "$dir/apps/android/local.properties" ]; then
        cp "$ARBRES_MAIN/apps/android/local.properties" "$dir/apps/android/"
    fi
    arbre_kyber_pret "$v"
}

# arbre_verrou <variante> : verrou de l'arbre pour ce processus.
arbre_verrou() { kyber_verrou_prendre "$(arbre_dir "$1")" "${2:-${0##*/}} ($1)"; }

# arbre_propre <variante> : vide si la construction n'a modifié aucun fichier
# suivi de l'arbre (sinon la liste) ; les schémas partagés d'Xcode exceptés.
arbre_propre() {
    git -C "$(arbre_dir "$1")" status --porcelain --untracked-files=no | grep -v '\.xcodeproj/xcshareddata/xcschemes/' || true
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    set -euo pipefail
    case "${1:-}" in
        liste)
            for v in $ARBRES_VARIANTES; do
                d="$(arbre_dir "$v")"
                if [ -e "$d/.git" ]; then
                    printf '%-16s %s  %s' "$v" "$(git -C "$d" rev-parse --short HEAD)" "$(git -C "$d" log -1 --format=%s | cut -c1-50)"
                    [ -d "$d.verrou" ] && printf '  [verrou : %s]' "$(sed -n 's/^commande=//p' "$d.verrou/qui" 2>/dev/null)"
                    echo
                else
                    printf '%-16s (pas encore créé)\n' "$v"
                fi
            done ;;
        chemin) arbre_dir "${2:?variante}" ;;
        preparer)
            v="${2:?variante}"
            arbre_verrou "$v" "arbres.sh preparer"
            arbre_preparer "$v" "$(git rev-parse "${3:-HEAD}^{commit}")"
            kyber_verrou_rendre "$(arbre_dir "$v")"
            arbre_dir "$v" ;;
        *) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
    esac
fi
