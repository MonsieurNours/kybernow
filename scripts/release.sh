#!/bin/bash
# Listes de chemins et de processus coupées en mots exprès (sans espaces).
# shellcheck disable=SC2046,SC2207
# Publie une version de KyberNow sur GitHub (github.com/MonsieurNours/kybernow).
#
#   scripts/release.sh X.Y.Z                  version stable
#   scripts/release.sh X.Y.Z --beta           version bêta (prerelease, canal beta)
#   scripts/release.sh X.Y.Z --repetition     répétition : tout construire et tout
#                                             vérifier comme une publication, en
#                                             parallèle et avec reprise, sans rien
#                                             commiter ni publier (--dry-run : pareil)
#   scripts/release.sh X.Y.Z --check          vérifications préalables seules
#   … --min-installer A.B.C --requires-setup  exigences de la mise à jour (manifeste)
#   … --sans-notarisation                     répétition sans envoi à Apple
#
# Déroulé (docs/releases.md, « Publier une version ») :
#   1. vérifications préalables, rapides et toutes affichées d'un coup (git,
#      notes, disque, outils, accords d'Apple, VM « KyberNow Linux » sans
#      fenêtre et Rosetta, édition de liens de l'app Windows, copie du Flatpak,
#      dépendances de cargo, verrous des arbres) ; une seule en échec et rien
#      n'est modifié ;
#   2. alignement des versions dans l'arbre « versions » et commit « KyberNow
#      X.Y.Z » (en publication, main avance jusqu'à lui ; en répétition, le
#      commit n'est sur aucune branche) ;
#   3. constructions depuis ce commit, chacune dans son arbre de construction
#      (scripts/arbres.sh : worktree et sources de Kyber par variante, caches
#      gardés), en cinq voies parallèles : Windows/Android/webOS, Apple,
#      notarisation de l'app Mac, Flatpak (VM), .deb et KyberNow Link OS (VM).
#      Une étape dont les entrées (ses fichiers du commit, outils, sources de
#      Kyber) n'ont pas changé depuis sa dernière réussite est reprise telle
#      quelle (third_party/arbres/.etapes/) ; un échec arrête les autres voies
#      après leur étape en cours, rien n'est publié ;
#   4. vérification des fichiers (signatures, notarisation, Gatekeeper, APK,
#      versions lisibles, tailles, DLL des exécutables Windows) ;
#   5. flux Sparkle signé EdDSA, flux cumulatif du site, notes françaises, dépôt
#      de Raspberry Pi Imager (commités par-dessus le commit de version),
#      SHA256SUMS, manifeste signé (minisign), revérifiés ;
#   6. release GitHub en brouillon, 7. tag vX.Y.Z, 8. push de main et du tag
#      (atomique), 9. publication ;
#   10. résumé : durée de chaque étape, refaite ou reprise.
#
# Relance après un échec : la même commande. Le commit « KyberNow X.Y.Z » déjà
# en tête (ou sous le commit du flux du site) est réutilisé, ou HEAD si ses
# versions sont déjà alignées (une correction commitée par-dessus) ; seules
# les étapes dont les entrées ont changé sont refaites.
#
# Prérequis : docs/releases.md (« Ce que l'auteur prépare une fois ») et
# docs/build-notes.md (third_party/, outils, VM).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="MonsieurNours/kybernow"
cd "$ROOT"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
DEBUT_PUBLICATION=$(date +%s)

usage() {
    echo "usage : scripts/release.sh X.Y.Z [--beta] [--repetition | --check] [--min-installer A.B.C] [--requires-setup] [--sans-notarisation]" >&2
    exit "${1:-1}"
}

# État de l'arbre de travail, sans les schémas partagés des projets Xcode :
# Xcode ouvert les réécrit dans son format dès que xcodegen les régénère, et
# ils n'entrent pas dans les binaires.
tree_state() {
    git status --porcelain "$@" | grep -v '\.xcodeproj/xcshareddata/xcschemes/' || true
}

VERSION="" BETA=0 MODE=release MIN_INSTALLER="" REQUIRES_SETUP=0 NOTARISER=1
while [ $# -gt 0 ]; do
    case "$1" in
        --beta) BETA=1 ;;
        --repetition | --dry-run) MODE=repetition ;;
        --check) MODE=check ;;
        --sans-notarisation) NOTARISER=0 ;;
        --min-installer) [ $# -ge 2 ] || usage; MIN_INSTALLER="$2"; shift ;;
        --min-installer=*) MIN_INSTALLER="${1#*=}" ;;
        --requires-setup) REQUIRES_SETUP=1 ;;
        -h | --help) usage 0 ;;
        -*) echo "option inconnue : $1" >&2; usage ;;
        *) [ -z "$VERSION" ] || usage; VERSION="$1" ;;
    esac
    shift
done
[ -n "$VERSION" ] || usage
SEMVER='^[0-9]+\.[0-9]+\.[0-9]+$'
[[ "$VERSION" =~ $SEMVER ]] || { echo "version attendue : X.Y.Z (une bêta aussi : --beta)" >&2; exit 1; }
[ "$MODE" = release ] && [ "$NOTARISER" = 0 ] && { echo "--sans-notarisation : seulement en répétition" >&2; exit 1; }

TAG="v$VERSION"
SUFFIXE=""
[ "$MODE" = release ] || SUFFIXE="-repetition"
OUT="$ROOT/dist/$TAG$SUFFIXE"
LOGS="$ROOT/dist/.journaux-$TAG$SUFFIXE"
CHANNEL=stable
[ "$BETA" = 1 ] && CHANNEL=beta
# Archive de l'app Mac (flux Sparkle, site) ; KyberNow-macOS.zip, son nom
# jusqu'à la 0.4.2, est publié en copie pour les liens existants.
MAC_ZIP=KyberNow-Link-macOS.zip
# Flux cumulatif des mises à jour de l'app Mac, publié par le site
# (www.kybernow.app/updates/appcast.xml) : toutes les versions, bêtas comprises.
SITE_FEED="site/updates/appcast.xml"
FEED_SUBJECT="Flux des mises à jour : KyberNow $VERSION"
# Entrées des étapes, étapes et fichiers publiés : scripts/release-entrees.sh,
# scripts/release-etapes.txt et scripts/release-fichiers.txt (une seule liste,
# lue aussi par sign-manifest.sh, release-verifie.py et release-suivi.py).
. scripts/release-entrees.sh
# Fichiers construits (vérifiés avant publication), anciens noms compris
# (copies : docs/releases.md, « Anciens noms des installateurs »), puis tous
# ceux publiés.
COPIES=($(release_fichiers copie))
BUILT=($(release_fichiers construit copie))
ASSETS=("${BUILT[@]}" $(release_fichiers flux signature))
# Fichiers modifiés par l'alignement des versions (et rien d'autre dans le commit).
VERSION_FILES=(server/admin/Cargo.toml server/admin/Cargo.lock server/tray/Cargo.toml server/tray/Cargo.lock
    server/installer/Cargo.toml server/installer/Cargo.lock apps/macos/project.yml apps/macos/KyberNow.xcodeproj
    apps/tvos/project.yml apps/tvos/KyberNowTV.xcodeproj
    apps/ios/project.yml apps/ios/KyberNowIOS.xcodeproj
    apps/Cargo.toml apps/Cargo.lock apps/windows/tauri.conf.json
    apps/windows/setup/Cargo.toml apps/windows/setup/Cargo.lock server/micro/Cargo.toml server/micro/Cargo.lock
    apps/linux/flatpak/fr.leslignesclaires.kybernow.metainfo.xml)
STATION_LINUX=()
if [ "${KYBERNOW_STATION_LINUX:-1}" = 1 ]; then
    STATION_LINUX=($(release_fichiers station-linux))
fi

# Exigences de la mise à jour, transmises à package-update.sh et au manifeste.
UPDATE_FLAGS=()
[ -n "$MIN_INSTALLER" ] && UPDATE_FLAGS+=(--min-installer "$MIN_INSTALLER")
[ "$REQUIRES_SETUP" = 1 ] && UPDATE_FLAGS+=(--requires-setup)

version_num() { local a b c; IFS=. read -r a b c <<< "$1"; echo $((a * 1000000 + b * 1000 + c)); }
# Ce qui modifie le dépôt ou GitHub n'est fait qu'en publication réelle ; en
# répétition, la commande est affichée.
act() {
    if [ "$MODE" = release ]; then
        "$@"
    else
        printf '    [répétition, non lancé]'; printf ' %q' "$@"; echo
    fi
}
duree() { local s=$1; if [ "$s" -ge 60 ]; then printf '%d min %02d s' $((s / 60)) $((s % 60)); else printf '%d s' "$s"; fi; }

STEP="vérifications préalables"
step() { STEP="$1"; echo; echo "==> $1"; }
# set -E : une erreur dans une fonction (arbre_preparer…) passe aussi par
# on_error. Pas dans un sous-shell (substitution de commande, voie) : $$ y
# vaut le pid du script, et kyber_verrou_rendre rendrait ses verrous.
set -E
on_error() {
    [ "${BASHPID:-$$}" = "$$" ] || return 0
    echo >&2
    echo "Échec pendant « $STEP » (ligne $1)." >&2
    declare -F rendre_verrous > /dev/null && rendre_verrous
    if [ "$MODE" = release ]; then
        echo "Corrigez puis relancez : scripts/release.sh $VERSION$([ "$BETA" = 1 ] && echo " --beta")" \
            "(le commit de version, les étapes réussies et le brouillon sont repris)." >&2
    fi
}
trap 'on_error $LINENO' ERR
# Dossiers temporaires, retirés à la sortie quelle qu'elle soit.
TEMPS=()
nettoyer() { [ "${BASHPID:-$$}" = "$$" ] && rm -rf ${TEMPS[@]+"${TEMPS[@]}"}; return 0; }
trap nettoyer EXIT

. scripts/kyber-toolchain.sh
. scripts/arbres.sh
. apps/linux/scripts/vm.sh
# Caches des étapes (empreinte de leurs entrées, fichiers produits) et état de
# la publication en cours (voies, étapes).
CACHE="$ARBRES/.etapes"
RUN="$ARBRES/.publication"
NCPU="$(sysctl -n hw.ncpu 2>/dev/null || echo 8)"
# Deux voies compilent sur le Mac en même temps : chacune un peu plus de la
# moitié des cœurs pour cargo (meson, ninja et Xcode règlent les leurs).
JOBS_MAC=$(( NCPU * 2 / 3 > 4 ? NCPU * 2 / 3 : 4 ))
LLVM_OBJDUMP="$ROOT/third_party/toolchains/llvm-mingw-$KYBER_LLVM_MINGW-ucrt-macos-universal/bin/llvm-objdump"

# ---------------------------------------------------------------------------
step "Vérifications préalables ($([ "$MODE" = release ] && echo publication || echo "$MODE"), canal $CHANNEL)"
PROBLEMS=()
ok() { echo "  ok   $1"; }
note() { echo "  note $1"; }
problem() { PROBLEMS+=("$1"); echo "  NON  $1" >&2; }
# En répétition, ce qui n'empêche que la publication (git, notes, GitHub) est signalé sans arrêter.
release_only() { if [ "$MODE" = repetition ]; then note "$1 (bloquerait une publication)"; else problem "$1"; fi; }

if [ -n "$MIN_INSTALLER" ]; then
    if ! [[ "$MIN_INSTALLER" =~ $SEMVER ]]; then problem "--min-installer : version X.Y.Z attendue ($MIN_INSTALLER)"
    elif [ "$(version_num "$MIN_INSTALLER")" -gt "$(version_num "$VERSION")" ]; then
        problem "--min-installer $MIN_INSTALLER est plus récent que la version publiée $VERSION"
    else ok "installateur minimal : $MIN_INSTALLER"; fi
fi
[ "$REQUIRES_SETUP" = 1 ] && ok "mise à jour réservée à l'installateur (requires_setup)"

# Une seule publication ou répétition à la fois, et aucun arbre pris ailleurs.
if kyber_verrou_prendre "$ARBRES/publication" "scripts/release.sh $VERSION ($MODE)" 2> "$ARBRES.verrou-msg"; then
    ok "aucune autre publication en cours"
else
    problem "une autre publication ou répétition est en cours : $(tr '\n' ' ' < "$ARBRES.verrou-msg")"
fi
rm -f "$ARBRES.verrou-msg"
for v in $ARBRES_VARIANTES; do
    if ! arbre_verrou "$v" "scripts/release.sh $VERSION" 2> "$ARBRES.verrou-msg"; then
        problem "arbre « $v » pris par une autre construction : $(tr '\n' ' ' < "$ARBRES.verrou-msg")"
    fi
    rm -f "$ARBRES.verrou-msg"
done
rendre_verrous() {
    local v
    for v in $ARBRES_VARIANTES; do kyber_verrou_rendre "$(arbre_dir "$v")"; done
    kyber_verrou_rendre "$ARBRES/publication"
}

# Git : main, propre, à jour.
BRANCH="$(git symbolic-ref -q --short HEAD || echo "(tête détachée)")"
if [ "$BRANCH" = main ]; then ok "branche main"; else release_only "branche « $BRANCH » : on ne publie que depuis main"; fi
DIRTY="$(tree_state --untracked-files=normal)"
if [ -z "$DIRTY" ]; then
    ok "arbre de travail propre"
else
    release_only "arbre de travail modifié (commitez d'abord, CHANGELOG compris ; la répétition construit HEAD, sans ces modifications) :"
    printf '%s\n' "$DIRTY" | head -15 | sed 's/^/         /' >&2
fi
if [ "$MODE" = release ]; then
    if git fetch -q origin main 2>/dev/null; then
        if git merge-base --is-ancestor origin/main HEAD; then ok "à jour avec origin/main"
        else problem "HEAD n'inclut pas origin/main : git pull (le push serait refusé)"; fi
    else
        problem "origin inaccessible (git fetch origin main)"
    fi
fi

# Relance : le commit de version est déjà en tête, ou juste sous celui du
# flux du site (échec après l'étape 5) ; ou HEAD a déjà ses versions alignées
# (une correction commitée par-dessus le commit de version).
RELAUNCH=0
VERSION_REF=HEAD
if [ "$(git log -1 --format=%s)" = "$FEED_SUBJECT" ] && [ "$(git log -1 --format=%s HEAD~1 2> /dev/null)" = "KyberNow $VERSION" ]; then
    VERSION_REF=HEAD~1
fi
aligned() { git show "$1:server/admin/Cargo.toml" | grep -qxF "version = \"$VERSION\"" && git show "$1:apps/macos/project.yml" | grep -qF "MARKETING_VERSION: \"$VERSION\""; }
if [ "$(git log -1 --format=%s "$VERSION_REF")" = "KyberNow $VERSION" ]; then
    if aligned "$VERSION_REF"; then
        RELAUNCH=1; ok "commit « KyberNow $VERSION » déjà fait$([ "$VERSION_REF" != HEAD ] && echo ", flux du site commité par-dessus") : relance, pas de nouveau commit"
    else
        problem "le commit « KyberNow $VERSION » est là mais server/admin/Cargo.toml n'a pas cette version"
    fi
elif aligned HEAD; then
    RELAUNCH=1; ok "versions déjà alignées sur $VERSION dans HEAD ($(git log -1 --format='%h « %s »' | cut -c1-70)) : HEAD est le commit construit"
fi
if git rev-parse -q --verify "refs/tags/$TAG" > /dev/null; then
    if [ "$RELAUNCH" = 1 ] && [ "$(git rev-list -n 1 "$TAG")" = "$(git rev-parse "$VERSION_REF")" ]; then
        ok "tag $TAG déjà posé sur le commit de version (relance)"
    else
        release_only "le tag $TAG existe déjà sur $(git rev-list -n 1 "$TAG" | cut -c1-7) (version publiée, ou reste d'un essai raté : git tag -d $TAG)"
    fi
fi

# Notes de version, en français et en anglais.
for f in CHANGELOG.md CHANGELOG.en.md; do
    if grep -qxF "## $VERSION" "$f"; then ok "$f : section ## $VERSION"
    else release_only "$f : ajoutez une section « ## $VERSION »"; fi
done

# Place sur le disque du Mac (arbres, caches, image de KyberNow Link OS).
FREE_GB=$(df -Pk "$ROOT" | awk 'NR==2 {print int($4/1048576)}')
if [ "$FREE_GB" -lt 40 ]; then problem "disque du Mac : $FREE_GB Go libres, 40 au moins (docs/build-notes.md, « Place sur le disque »)"
elif [ "$FREE_GB" -lt 80 ]; then note "disque du Mac : $FREE_GB Go libres (moins de 80)"
else ok "disque du Mac : $FREE_GB Go libres"; fi

# Outils.
for tool in xcodegen xcodebuild npm cargo zstd x86_64-w64-mingw32-gcc zip ditto python3 xmllint rsync minisign meson cmake; do
    command -v "$tool" > /dev/null || problem "$tool introuvable"
done
# Rust nightly (Apple TV, iPhone/iPad : cargo +nightly) : son empreinte entre
# dans celle des outils ; une autre date que celle vérifiée est signalée.
if NIGHTLY_V="$(rustup run nightly rustc -V 2>&1)"; then
    if [[ "$NIGHTLY_V" == *" $KYBER_NIGHTLY)" ]]; then ok "Rust nightly du $KYBER_NIGHTLY"
    else note "Rust nightly : $NIGHTLY_V, versions vérifiées : $KYBER_NIGHTLY (scripts/kyber-toolchain.sh)"; fi
else
    problem "Rust nightly introuvable (rustup toolchain install nightly-$KYBER_NIGHTLY, puis rustup default ou lien « nightly ») : $NIGHTLY_V"
fi

# Outils de publication : verrous, arbres, correctifs, entrées des étapes et
# vérification des fichiers (quelques secondes, rien n'est construit).
if CHECK="$(scripts/tests/release-outils.sh 2>&1)"; then ok "essais des outils de publication (scripts/tests/release-outils.sh)"
else problem "scripts/tests/release-outils.sh en échec : $(grep -E '^ +NON' <<< "$CHECK" | head -5 | tr '\n' ' ')"; fi
[ -x "$LLVM_OBJDUMP" ] || problem "llvm-objdump de llvm-mingw $KYBER_LLVM_MINGW introuvable (third_party/toolchains, docs/build-notes.md)"

# GitHub : seulement pour publier.
RELEASE_STATE=none
if [ "$MODE" = release ] || [ "$MODE" = check ]; then
    if gh auth status -h github.com > /dev/null 2>&1; then
        ok "gh connecté à github.com"
        if INFO="$(gh release view "$TAG" -R "$REPO" --json isDraft --jq .isDraft 2>&1)"; then
            if [ "$INFO" = true ]; then RELEASE_STATE=draft; ok "brouillon $TAG existant : réutilisé"
            else RELEASE_STATE=published; problem "$TAG est déjà publiée sur GitHub"; fi
        elif ! grep -qi "not found" <<< "$INFO"; then
            problem "lecture des releases de $REPO impossible : $INFO"
        fi
        # Image du relais, signée par le workflow relay-image.yml si sa clé cosign
        # est dans les secrets du dépôt : sans elle, elle part non signée (note).
        if SECRETS="$(gh secret list -R "$REPO" 2>/dev/null)"; then
            if grep -q '^COSIGN_PRIVATE_KEY' <<< "$SECRETS" && grep -q '^COSIGN_PASSWORD' <<< "$SECRETS"; then ok "image du relais : clé cosign dans les secrets du dépôt (signée à la publication)"
            else note "image du relais : pas de clé cosign dans les secrets du dépôt, elle sera publiée sans signature (docs/releases.md, « Signature de l'image du relais »)"; fi
        else
            note "image du relais : secrets du dépôt illisibles par gh, signature cosign non vérifiée"
        fi
    else
        problem "gh non connecté : gh auth login (jeton et droits : docs/releases.md)"
    fi
fi

# Notarisation : profil lisible et accords d'Apple en règle (une requête
# sans effet : un accord expiré répond 403, comme l'envoi le ferait).
if [ "$NOTARISER" = 1 ]; then
    if NOTA="$(xcrun notarytool history --keychain-profile kybernow 2>&1)"; then
        ok "notarisation : profil « kybernow » lisible, accords d'Apple en règle"
    elif grep -qiE "403|agreement|contrat|accord" <<< "$NOTA"; then
        problem "notarisation refusée par Apple (403) : un accord est à accepter sur developer.apple.com (Account › Agreements) et App Store Connect"
    else
        problem "profil de notarisation « kybernow » illisible : Mac verrouillé, ou xcrun notarytool store-credentials (docs/releases.md) — $(tail -1 <<< "$NOTA")"
    fi
else
    note "répétition sans notarisation (--sans-notarisation) : agrafes et Gatekeeper non vérifiés"
fi
if [ -x scripts/sign-manifest.sh ]; then ok "scripts/sign-manifest.sh (manifeste signé minisign)"
else problem "scripts/sign-manifest.sh absent : sans manifeste signé, Admin refuse la mise à jour"; fi
if [ -f "$SITE_FEED" ]; then ok "flux cumulatif du site ($SITE_FEED)"
else problem "$SITE_FEED absent : scripts/rebuild-appcast.sh (une fois, à commiter)"; fi

# App Android : clé de publication (Trousseau) et outils (JDK, SDK, apksigner).
if CHECK="$(apps/android/scripts/release-apk.sh --check 2>&1)"; then ok "app Android : ${CHECK##*$'\n'}"
else problem "app Android : ${CHECK##*$'\n'}"; fi
# App des téléviseurs LG : cible Rust.
if rustup target list --installed 2>/dev/null | grep -qx armv7-unknown-linux-gnueabi; then ok "app webOS : cible Rust armv7-unknown-linux-gnueabi"
else problem "app webOS : rustup target add armv7-unknown-linux-gnueabi"; fi

# Sources de Kyber des arbres : copiées une fois depuis le third_party/ du
# dépôt principal ; si Kyber y a avancé depuis, l'arbre construirait l'ancien.
for v in $ARBRES_VARIANTES; do
    for e in $(arbre_sources_kyber "$v"); do
        [ -d "$(arbre_dir "$v")/third_party/$e" ] && [ -d "$ARBRES_TP/$e" ] || continue
        if [ "$(arbre_kyber_commits "$ARBRES_TP/$e")" != "$(arbre_kyber_commits "$(arbre_dir "$v")/third_party/$e")" ]; then
            note "arbre « $v » : sources de $e à un autre commit que third_party/$e du dépôt principal (scripts/arbres.sh recopier $v, pour construire la nouvelle version)"
        fi
    done
done

# Sources de Kyber et dépendances lues par les arbres (third_party/ partagé).
for f in third_party/kyber-desktop/kysdk/kyctl third_party/kyber-desktop-macos/kysdk/kyctl third_party/kyber-web/Cargo.toml \
    third_party/payload/kyber-0.29.0/SDL2.dll third_party/payload/vbcable/VBCABLE_Setup_x64.exe third_party/rootfs-winarm64/lib/libavcodec.a \
    "third_party/toolchains/llvm-mingw-$KYBER_LLVM_MINGW-ucrt-macos-universal/bin/aarch64-w64-mingw32-clang"; do
    if [ -e "$f" ]; then ok "${f#third_party/}"
    else problem "$f absent (docs/build-notes.md)"; fi
done

# App Windows : édition de liens contre la libavcodec que l'app liera (celle
# de la Station Windows de l'arbre, sinon du third_party/ partagé) ; ARM64
# contre la sienne. Quelques secondes ; un symbole manquant est nommé.
WIN_ROOTFS="$(arbre_dir station-windows)/third_party/rootfs-win64"
[ -f "$WIN_ROOTFS/lib/libavcodec.a" ] || WIN_ROOTFS="$ROOT/third_party/rootfs-win64"
for arch in x64 arm64; do
    r=()
    [ "$arch" = x64 ] && r=(--rootfs "$WIN_ROOTFS")
    if CHECK="$(apps/windows/scripts/link-check.sh --arch "$arch" ${r[@]+"${r[@]}"} 2>&1)"; then ok "$CHECK"
    else problem "$(tr '\n' ' ' <<< "$CHECK")"; fi
done

# Ce que les arbres recompileront d'eux-mêmes (rien à faire ; dit pour la durée).
STATION_STAMP="$(arbre_dir station-windows)/third_party/payload/kyber-overlay.stamp"
# Empreinte de la surcouche : calculée dans l'arbre, au commit construit (le commit
# de version change les Cargo.toml des aides, qu'elle compte) ; ici, sur main, pour
# la note seulement.
OVERLAY_WANT="$(server/kyber-patches/stamp.sh)"
overlay_voulue() { "$(arbre_dir station-windows)/server/kyber-patches/stamp.sh"; }
if [ "$(cat "$STATION_STAMP" 2>/dev/null)" = "$OVERLAY_WANT" ]; then ok "surcouche Kyber de l'arbre station-windows : correctifs actuels (empreinte)"
else note "surcouche Kyber (Station Windows) à compiler dans son arbre : correctifs changés ou arbre neuf (automatique, long)"; fi
# Empreinte de Kyber pour Mac : la fonction qu'emploie build-kyber.sh
# (kyber_station_hash d'apply.sh, STCOM-11), calculée dans le dépôt donné ;
# elle compte aussi apps/macos/PyroWave, d'où vient l'encodeur PyroWave de la
# Station Mac (le même commit que le décodeur des apps, TRANS-03).
mac_kyber_hash() { (. "$1/server/kyber-patches/apply.sh" && kyber_station_hash "$1" mac); }
mac_kyber_a_jour() {
    local mk="$1/third_party/kyber-desktop-macos/rootfs-arm64-macos"
    [ -x "$mk/bin/kybernow-pyrowave-check" ] && [ "$(cat "$mk/.kybernow-patches" 2>/dev/null)" = "$(mac_kyber_hash "$2")" ]
}
if mac_kyber_a_jour "$(arbre_dir station-mac)" "$ROOT"; then
    ok "Station Mac : Kyber pour Mac de l'arbre compilé avec les correctifs actuels (empreinte, PyroWave)"
else
    note "Station Mac : Kyber pour Mac à compiler dans son arbre (correctifs changés, PyroWave absent ou arbre neuf : automatique)"
fi

# VM « KyberNow Linux » : sans fenêtre, joignable, Rosetta, place pour pi-gen.
vm_sans_fenetre
if [ "$(vm_vue)" = headless ]; then ok "VM « $VM_NAME » : démarrage sans fenêtre (headless)"
else problem "VM « $VM_NAME » : vue au démarrage « $(vm_vue) », pas headless (prlctl set \"$VM_NAME\" --startup-view headless)"; fi
if CHECK="$(apps/linux/scripts/release-flatpaks.sh --check 2>&1)"; then ok "app Linux : ${CHECK##*$'\n'}"
else problem "app Linux : ${CHECK##*$'\n'} (apps/linux/README.md, « Compiler »)"; fi
if CHECK="$(apps/linux/os/release-image.sh --check 2>&1)"; then ok "KyberNow Link OS : ${CHECK##*$'\n'}"
else problem "KyberNow Link OS : ${CHECK##*$'\n'} (apps/linux/os/README.md)"; fi
VM_TOOLS="$(vm "uname -m; flatpak --version; flatpak-builder --version; ~/.cargo/bin/rustc -V 2>/dev/null || rustc -V" 2>/dev/null || echo "VM injoignable")"
VM_KYBER_STATION=""
if [ "${KYBERNOW_STATION_LINUX:-1}" = 1 ]; then
    if CHECK="$(server/linux/scripts/release-station.sh --check 2>&1)"; then ok "${CHECK##*$'\n'}"
    else problem "${CHECK##*$'\n'} (docs/station-linux.md, « Compiler »)"; fi
    # Sources de Kyber de la Station Linux, dans la VM et dans son chroot x86_64
    # (release-station.sh) : leurs commits entrent dans l'empreinte de l'étape.
    # shellcheck disable=SC2088  # le ~ est celui de la VM
    VM_KYBER_DIRS="${KYBERNOW_STATION_KYBER_DIR:-~/kn-station/kyber-desktop} ~/kn-amd64-home/kyber-desktop"
    VM_KYBER_STATION="$(vm "for d in $VM_KYBER_DIRS; do for r in kysdk kysdk/kyctl kysdk/kymux kysdk/kynput kysdk/kymedia; do echo \"\$d/\$r \$(git -C \$d/\$r rev-parse HEAD 2>/dev/null || echo absent)\"; done; done" 2>/dev/null || echo "VM injoignable")"
else
    ok "Station Linux : pas dans cette version (KYBERNOW_STATION_LINUX=0)"
fi

# Copie des sources du Flatpak (apps/linux/scripts/sources.sh, sans scripts/
# ni historique git) : ce que build.sh y lit est là, ses correctifs se posent
# sur les sources de Kyber d'origine, et l'espace de travail réduit se résout.
FLATPAK_SRC="$(mktemp -d)"; TEMPS+=("$FLATPAK_SRC")
if CHECK="$(KYBERNOW_KYBER_HEAD=1 apps/linux/scripts/sources.sh "$FLATPAK_SRC/src" 2>&1)"; then
    MISSING=()
    for f in apps/linux/scripts/build.sh apps/Cargo.toml apps/Cargo.lock apps/core/Cargo.toml server/relay/tunnel \
        server/admin/src/update/kybernow-release.pub third_party/kyber-desktop/kysdk/kyctl/kyclient third_party/kyber-desktop/kysdk/kynput/kynput \
        third_party/kyber-desktop/kysdk/kymux/kymux third_party/kyber-desktop/kysdk/kymedia/subprojects/vlc-rs/src third_party/kyber-desktop/COPYING.AGPLv3; do
        [ -e "$FLATPAK_SRC/src/$f" ] || MISSING+=("$f")
    done
    PATCH_FAIL=()
    # Dans l'ordre de build.sh : ceux de la Station pour kyctl, kymux et kynput,
    # puis ceux des apps et le côté client de la Station.
    FLATPAK_STATION=()
    for p in "$FLATPAK_SRC"/src/server/kyber-patches/*.patch; do
        case "$(basename "$p")" in *-kycontroller-* | *-kywebplayer-* | *-kynet-* | *-kynput-*) FLATPAK_STATION+=("$p") ;; esac
    done
    for p in ${FLATPAK_STATION[@]+"${FLATPAK_STATION[@]}"} "$FLATPAK_SRC"/src/apps/macos/kyber-patches/*.patch "$FLATPAK_SRC"/src/server/kyber-patches/*-kyclient-*.patch "$FLATPAK_SRC"/src/apps/webos/kyber-patches/*.patch; do
        case "$(basename "$p")" in
            *-kyclient-* | *-kycontroller-* | *-kywebplayer-*) depot=kyctl ;; *-kynput-*) depot=kynput ;; *-kynet-*) depot=kymux ;; *) PATCH_FAIL+=("$(basename "$p") (dépôt ?)"); continue ;;
        esac
        (cd "$FLATPAK_SRC/src/third_party/kyber-desktop/kysdk/$depot" && patch -p1 -s -f -F0 < "$p" > /dev/null 2>&1) || PATCH_FAIL+=("$(basename "$p")")
    done
    if [ ${#MISSING[@]} -gt 0 ]; then problem "copie du Flatpak : manque ${MISSING[*]}"
    elif [ ${#PATCH_FAIL[@]} -gt 0 ]; then problem "copie du Flatpak : correctifs inapplicables sur les sources d'origine : ${PATCH_FAIL[*]}"
    elif ! CHECK="$(cd "$FLATPAK_SRC/src/apps" && cargo metadata --offline --format-version 1 > /dev/null 2>&1 || cargo metadata --format-version 1 2>&1 > /dev/null)"; then
        problem "copie du Flatpak : l'espace de travail ne se résout pas : $(tail -3 <<< "$CHECK" | tr '\n' ' ')"
    else ok "copie du Flatpak : sources, correctifs posés sur l'origine, espace de travail résolu"; fi
else
    problem "copie du Flatpak : sources.sh en échec : $(tail -2 <<< "$CHECK" | tr '\n' ' ')"
fi
rm -rf "$FLATPAK_SRC"

# Dépendances de cargo téléchargées (après une fusion, un Cargo.lock nouveau).
FETCH_FAIL=()
for ws in apps server/admin server/tray server/installer server/relay apps/windows/setup; do
    [ -f "$ws/Cargo.lock" ] || continue
    (cd "$ws" && cargo fetch --locked --quiet > /dev/null 2>&1) || FETCH_FAIL+=("$ws")
done
if [ ${#FETCH_FAIL[@]} -eq 0 ]; then ok "dépendances de cargo téléchargées (cargo fetch --locked)"
else problem "cargo fetch --locked en échec dans ${FETCH_FAIL[*]} (Cargo.lock à jour ? réseau ?)"; fi

if [ ${#PROBLEMS[@]} -gt 0 ]; then
    echo >&2
    echo "${#PROBLEMS[@]} vérification(s) en échec :" >&2
    printf '  - %s\n' "${PROBLEMS[@]}" >&2
    echo "Rien n'a été modifié." >&2
    rendre_verrous
    exit 1
fi
echo; echo "Toutes les vérifications préalables passent ($(duree $(( $(date +%s) - DEBUT_PUBLICATION )))."
if [ "$MODE" = check ]; then
    cat << EOF

Publication prévue ($CHANNEL) :
  2. versions alignées sur $VERSION, $([ "$RELAUNCH" = 1 ] && echo "commit déjà fait (relance)" || echo "commit « KyberNow $VERSION »")
  3. constructions en cinq voies, dans les arbres de third_party/arbres (étapes inchangées reprises) :
     Station Windows (surcouche si besoin), app Windows, Android, webOS ; app Mac, Apple TV, iPhone/iPad,
     Station Mac ; notarisation de l'app Mac ; Flatpak x86_64 et aarch64 ; .deb du Raspberry Pi et KyberNow Link OS
  4. vérification des fichiers ; 5. appcast.xml$([ "$BETA" = 1 ] && echo " (sparkle:channel beta)"), SHA256SUMS,
     kybernow-release.json signé (canal $CHANNEL), $SITE_FEED, commit « $FEED_SUBJECT »
  6. release $TAG en brouillon$([ "$BETA" = 1 ] && echo ", prerelease")$([ "$RELEASE_STATE" = draft ] && echo " (existante, fichiers remplacés)")
  7-9. tag $TAG sur le commit de version, push de main et du tag, publication
EOF
    rendre_verrous
    exit 0
fi
PRECHECK_S=$(( $(date +%s) - DEBUT_PUBLICATION ))

# Écran allumé jusqu'à la fin : le mot de passe de notarytool n'est lisible
# que Mac déverrouillé.
caffeinate -dimsu -w $$ &

# Fichiers lus sur l'entrée standard qui ne sont pas des fichiers de version.
hors_versions() {
    local f v keep
    while read -r f; do
        keep=0
        for v in "${VERSION_FILES[@]}"; do
            [ "$f" = "$v" ] && keep=1
            [ "${f#"$v"/}" != "$f" ] && keep=1
        done
        [ "$keep" = 1 ] || echo "$f"
    done
}

# ---------------------------------------------------------------------------
step "Versions"
T0=$(date +%s)
VT="$(arbre_dir versions)"
if [ "$RELAUNCH" = 1 ]; then
    COMMIT="$(git rev-parse "$VERSION_REF")"
    echo "    déjà alignées : $(git log -1 --format='%h « %s »' "$COMMIT")"
elif [ "$MODE" = repetition ] && [ -e "$VT/.git" ] && [ "$(git -C "$VT" log -1 --format=%s)" = "KyberNow $VERSION" ] \
    && [ "$(git -C "$VT" rev-parse HEAD~1)" = "$(git rev-parse HEAD)" ]; then
    # Répétition relancée sur le même HEAD : le commit de version de la
    # précédente est repris (même numéro de build des apps Apple : leurs
    # étapes peuvent être reprises).
    COMMIT="$(git -C "$VT" rev-parse HEAD)"
    echo "    commit de version de la répétition précédente repris : $(git log -1 --format='%h « %s »' "$COMMIT")"
else
    # Dans l'arbre « versions » (l'arbre de travail courant n'est pas touché) :
    # en publication, main avance ensuite jusqu'à ce commit ; en répétition, il
    # n'est sur aucune branche.
    arbre_preparer versions "$(git rev-parse HEAD)" > /dev/null
    BUILD=$(date +%Y%m%d%H%M)
    (
        cd "$VT"
        for f in server/admin/Cargo.toml server/tray/Cargo.toml server/installer/Cargo.toml apps/windows/setup/Cargo.toml server/micro/Cargo.toml; do
            sed -i '' -E "1,10s/^version = \".*\"/version = \"$VERSION\"/" "$f"
            (cd "$(dirname "$f")" && cargo update --workspace --offline --quiet)
        done
        # Clients en Rust (cœur et app Windows) : version de l'espace de travail.
        sed -i '' -E "s/^version = \".*\"/version = \"$VERSION\"/" apps/Cargo.toml
        sed -i '' -E "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" apps/windows/tauri.conf.json
        # Fiche AppStream du Flatpak : la version et sa date.
        sed -i '' -E "s|<release version=\"[^\"]*\" date=\"[^\"]*\"/>|<release version=\"$VERSION\" date=\"$(date +%Y-%m-%d)\"/>|" \
            apps/linux/flatpak/fr.leslignesclaires.kybernow.metainfo.xml
        (cd apps && cargo update --workspace --offline --quiet)
        for d in apps/macos apps/tvos apps/ios; do
            sed -i '' -E "s/MARKETING_VERSION: \".*\"/MARKETING_VERSION: \"$VERSION\"/; s/CURRENT_PROJECT_VERSION: \".*\"/CURRENT_PROJECT_VERSION: \"$BUILD\"/" "$d/project.yml"
            (cd "$d" && xcodegen generate --quiet)
        done
        git --no-pager diff --stat -- "${VERSION_FILES[@]}"
        EXTRA="$(git status --porcelain | grep -v '\.xcodeproj/xcshareddata/xcschemes/' | awk '{print $2}' | hors_versions)"
        [ -z "$EXTRA" ] || { echo "l'alignement des versions a modifié d'autres fichiers : $EXTRA" >&2; exit 1; }
        git add -- "${VERSION_FILES[@]}"
        git commit -q -m "KyberNow $VERSION"
    )
    COMMIT="$(git -C "$VT" rev-parse HEAD)"
    if [ "$MODE" = release ]; then
        git merge -q --ff-only "$COMMIT"
        echo "    commit $(git rev-parse --short HEAD) « KyberNow $VERSION » (main avancé)"
    else
        echo "    commit $(git rev-parse --short "$COMMIT") « KyberNow $VERSION » (répétition : sur aucune branche, arbre de travail intact)"
    fi
fi
VERSIONS_S=$(( $(date +%s) - T0 ))

# ---------------------------------------------------------------------------
step "Arbres de construction (third_party/arbres, au commit $(git rev-parse --short "$COMMIT"))"
T0=$(date +%s)
for v in station-windows windows-app android webos apple station-mac linux; do
    arbre_preparer "$v" "$COMMIT"
done
ARBRES_S=$(( $(date +%s) - T0 ))
rm -rf "$RUN" "$LOGS" "$OUT"
mkdir -p "$RUN/etat" "$LOGS" "$OUT" "$CACHE"

# Versions des outils : entrée de toutes les étapes.
TOOLS_ID="$({
    # Le nightly que lancent les scripts (cargo +nightly : apps/tvos,
    # apps/apple, apps/core), pas celui de la date vérifiée (souvent absent).
    rustc -V; rustup run nightly rustc -V; cargo -V; cargo cinstall --version 2>/dev/null | head -1
    xcodebuild -version | tr '\n' ' '; echo; xcodegen --version 2>/dev/null; x86_64-w64-mingw32-gcc -dumpversion; node -v; npm -v
    meson --version; cmake --version | head -1; java -version 2>&1 | head -1; ls "${ANDROID_HOME:-$HOME/Library/Android/sdk}/ndk" 2>/dev/null | tr '\n' ' '
    echo; sw_vers -productVersion
} | shasum -a 256 | cut -d' ' -f1)"

# ---------------------------------------------------------------------------
# Étapes. Chacune : ses entrées (scripts/release-entrees.sh : chemins du
# commit, sources de Kyber, dépendances de third_party/ ; plus les outils et sa
# recette), sa construction (construire_<id>, qui dépose ses fichiers dans $E),
# ses dépendances.
ETAPES=()
for id in $(release_etapes); do
    [ "$id" = station_linux ] && [ "${KYBERNOW_STATION_LINUX:-1}" != 1 ] && continue
    ETAPES+=("$id")
done
libelle() { release_libelle "$1"; }
deps_de() {
    case "$1" in
        station | windows | station_mac) echo surcouche ;;
        notarisation) echo app_mac ;;
        link_os) echo deb_pi ;;
        # Le Flatpak x86_64 (Rosetta) après pi-gen : pendant pi-gen, rustc
        # sous Rosetta a rendu une sortie vide (répétition du 25/11). Après la
        # Station Linux aussi, compilée elle aussi par Rosetta (chroot x86_64).
        flatpak_x86_64) echo link_os; [ "${KYBERNOW_STATION_LINUX:-1}" = 1 ] && echo station_linux ;;
    esac
    return 0
}
# Voie de chaque étape (voie_<nom> plus bas) : une voie morte sans avoir fini
# ses étapes (disque plein…) les met en échec, au lieu de laisser attendre
# pour toujours celles des autres voies qui en dépendent.
voie_de() {
    case "$1" in
        surcouche | station | windows | android | webos) echo autres ;;
        app_mac | tvos | ios | station_mac) echo apple ;;
        notarisation) echo notarisation ;;
        flatpak_aarch64 | flatpak_x86_64) echo linux ;;
        deb_pi | link_os | station_linux) echo linux_os ;;
    esac
}

# Entrées : chemins du commit (objets git : un seul fichier changé change
# l'empreinte), outils.
export RELEASE_REV="$COMMIT"
chemins() { echo "commit-chemins :"; git ls-tree "$COMMIT" -- "$@"; echo "outils $TOOLS_ID"; }
# Sources de Kyber d'un arbre : commit d'origine de chaque dépôt.
kyber_tetes() { local r; for r in $(arbre_depots "$(arbre_dir "$1")/third_party/$2"); do echo "${r##*/third_party/} $(git -C "$r" rev-parse HEAD)"; done; }
# Une dépendance lue dans third_party/ : noms, tailles et dates de ses fichiers.
tp_id() {
    local d="$ROOT/third_party/$1" f t
    f="$RUN/tp-$(echo "$1" | tr '/' '_')"
    if [ ! -f "$f" ]; then
        t="$f.$RANDOM$RANDOM"
        echo "$1 $( (cd "$d" 2>/dev/null && find -L . -type f -print0 | xargs -0 stat -f '%N %z %m' 2>/dev/null) | shasum -a 256 | cut -d' ' -f1)" > "$t"
        mv "$t" "$f"
    fi
    cat "$f"
}
empreinte_de() {
    local f="$RUN/empreinte-$1" t
    if [ ! -f "$f" ]; then
        t="$f.$RANDOM$RANDOM"
        { echo "$1"; entrees_de "$1"; } | shasum -a 256 | cut -d' ' -f1 > "$t"
        mv "$t" "$f"
    fi
    cat "$f"
}
# Tout ce qui entre dans l'empreinte d'une étape, recette comprise (modifier
# construire_<id> la refait).
# Profils d'approvisionnement d'Apple installés (noms et empreintes), et ceux
# que désignent KYBERNOW_GAMEPADS_PROFILE et KYBERNOW_STATION_PROFILE.
profils_apple() {
    local f
    for f in "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"/*.provisionprofile \
             "$HOME/Library/MobileDevice/Provisioning Profiles"/*.provisionprofile \
             ${KYBERNOW_GAMEPADS_PROFILE:+"$KYBERNOW_GAMEPADS_PROFILE"} ${KYBERNOW_STATION_PROFILE:+"$KYBERNOW_STATION_PROFILE"}; do
        [ -f "$f" ] && echo "profil $(basename "$f") $(shasum -a 256 < "$f" | cut -d' ' -f1)"
    done
    return 0
}
entrees_de() {
    local id=$1 lc k t d
    lc=($(release_chemins "$id"))
    [ ${#lc[@]} -eq 0 ] || chemins "${lc[@]}"
    for k in $(release_kyber "$id"); do kyber_tetes "${k%%:*}" "${k#*:}"; done
    for t in $(release_tp "$id"); do tp_id "$t"; done
    for d in $(release_empreintes "$id"); do echo "empreinte $d $(empreinte_de "$d")"; done
    echo "recette :"; declare -f "construire_$id"; echo "fichiers : $(fichiers_de "$id")"
    case "$id" in
        station) echo "maj $UPDATE_FLAGS_STR" ;;
        notarisation) echo "notariser $NOTARISER" ;;
        surcouche) declare -f overlay_voulue ;;
        # Les profils d'Apple installés : build-station-app.sh y choisit celui
        # des manettes (droit HID Virtual Device) et celui de la caméra
        # (extension système) ; un profil Developer ID ajouté (droit des
        # manettes accordé le 27/11/2026) doit refaire la Station Mac.
        station_mac) echo "notariser $NOTARISER"; declare -f mac_kyber_hash mac_kyber_a_jour; profils_apple ;;
        flatpak_aarch64 | flatpak_x86_64 | deb_pi) echo "$VM_TOOLS" ;;
        station_linux) echo "$VM_TOOLS"; echo "$VM_KYBER_STATION" ;;
    esac
}

# Fichiers produits par chaque étape (dans $E, puis dans son cache).
fichiers_de() {
    case "$1" in
        surcouche) echo "surcouche.stamp" ;;
        station) echo "KyberNow-Station-Setup.exe kybernow-update.zip update.ps1" ;;
        windows) echo "KyberNow-Link-Setup.exe" ;;
        android) echo "KyberNow-Link-Android.apk" ;;
        webos) echo "KyberNow-Link-webOS.ipk" ;;
        app_mac) echo "app-mac-signee.zip" ;;
        notarisation) echo "$MAC_ZIP" ;;
        tvos) echo "KyberSDK-tvOS.zip" ;;
        ios) echo "KyberSDK-iOS.zip" ;;
        station_mac) echo "KyberNow-Station-Mac.dmg KyberNow-Station-Mac-update.zip" ;;
        flatpak_aarch64) echo "KyberNow-Link-Linux-aarch64.flatpak" ;;
        flatpak_x86_64) echo "KyberNow-Link-Linux-x86_64.flatpak" ;;
        deb_pi) echo "KyberNow-Link-RaspberryPi-arm64.deb kybernow-link_${VERSION}_arm64.deb" ;;
        link_os) echo "KyberNow-Link-OS.img.xz KyberNow-Link-OS.img.xz.info" ;;
        station_linux) echo "${STATION_LINUX[*]}" ;;
    esac
}

# Cache valable : même empreinte, fichiers intacts (et, pour la surcouche,
# celle de l'arbre encore compilée avec ces correctifs).
cache_valide() {
    local id=$1 key=$2 c="$CACHE/$1"
    [ "$(cat "$c/empreinte" 2>/dev/null)" = "$key" ] || return 1
    (cd "$c/fichiers" 2>/dev/null && shasum -a 256 -s -c ../sommes) || return 1
    if [ "$id" = surcouche ]; then
        [ "$(cat "$(arbre_dir station-windows)/third_party/payload/kyber-overlay.stamp" 2>/dev/null)" = "$(overlay_voulue)" ] || return 1
    fi
}
cache_noter() {
    local id=$1 key=$2 c="$CACHE/$1" f
    for f in $(fichiers_de "$id"); do
        [ -s "$CACHE/$id.nouveau/$f" ] || { echo "$f absent après la construction" >> "$LOGS/$id.log"; return 1; }
    done
    (cd "$CACHE/$id.nouveau" && shasum -a 256 $(fichiers_de "$id")) > "$CACHE/$id.sommes"
    rm -rf "$c"; mkdir -p "$c"
    mv "$CACHE/$id.nouveau" "$c/fichiers"
    mv "$CACHE/$id.sommes" "$c/sommes"
    echo "$key" > "$c/empreinte"
}

etat() { echo "$2 ${3:-$(date +%s)} $(date +%s)" > "$RUN/etat/$1.tmp" && mv "$RUN/etat/$1.tmp" "$RUN/etat/$1"; }
etat_de() { cut -d' ' -f1 "$RUN/etat/$1" 2>/dev/null || echo attente; }
arret() { [ -f "$RUN/arret" ]; }

# etape <id> : attend ses dépendances, reprend le cache s'il est valable,
# sinon construit (sortie dans $LOGS/<id>.log) ; un échec arrête les voies.
# Rend toujours 0 (l'issue est dans $RUN/etat/<id>) : appelée dans un « || »,
# set -e serait ignoré jusque dans la construction (piège de bash), et une
# commande en échec n'arrêterait plus rien.
etape() {
    local id=$1 d st key debut
    arret && { etat "$id" annulee; return 0; }
    for d in $(deps_de "$id"); do
        while :; do
            st="$(etat_de "$d")"
            case "$st" in faite | reprise) break ;; echec | annulee) etat "$id" annulee; return 0 ;; esac
            sleep 2
        done
    done
    arret && { etat "$id" annulee; return 0; }
    key="$(empreinte_de "$id")"
    if cache_valide "$id" "$key"; then
        etat "$id" reprise
        return 0
    fi
    debut=$(date +%s)
    etat "$id" encours "$debut"
    rm -rf "$CACHE/$id.nouveau"
    mkdir -p "$CACHE/$id.nouveau"
    # Sous-shell lancé en arrière-plan puis attendu : set -e y vaut (il serait
    # ignoré dans un « if ( … ) »).
    ( set -euo pipefail; E="$CACHE/$id.nouveau"; "construire_$id" ) > "$LOGS/$id.log" 2>&1 < /dev/null &
    if wait $! && cache_noter "$id" "$key"; then
        etat "$id" faite "$debut"
        return 0
    fi
    etat "$id" echec "$debut"
    touch "$RUN/arret"
    return 0
}

# --- Constructions -----------------------------------------------------------
export KYBERNOW_IGNORE_RUNNING=1   # les arbres ont leur propre Vendor/, pas celui de l'app ouverte
UPDATE_FLAGS_STR="${UPDATE_FLAGS[*]+"${UPDATE_FLAGS[*]}"}"
note_kyber() { arbre_kyber_noter "$1" > /dev/null; }

construire_surcouche() {
    cd "$(arbre_dir station-windows)"
    local want
    want="$(overlay_voulue)"
    if [ "$(cat third_party/payload/kyber-overlay.stamp 2>/dev/null)" != "$want" ]; then
        # build.sh pose tous les correctifs : il part des sources d'origine.
        arbre_kyber_origine station-windows
        server/kyber-patches/build.sh
    fi
    [ "$(cat third_party/payload/kyber-overlay.stamp)" = "$want" ]
    python3 -I "$ROOT/scripts/release-verifie.py" pe "$LLVM_OBJDUMP" third_party/payload/kyber-overlay third_party/payload/kyber-0.29.0
    cp third_party/payload/kyber-overlay.stamp "$E/surcouche.stamp"
    note_kyber station-windows
}
construire_station() {
    cd "$(arbre_dir station-windows)"
    server/installer/build.sh
    # shellcheck disable=SC2086
    server/installer/package-update.sh $UPDATE_FLAGS_STR
    cp server/installer/dist/KyberNow-Station-Setup.exe server/installer/dist/kybernow-update.zip server/installer/dist/update.ps1 "$E/"
}
construire_windows() {
    cd "$(arbre_dir windows-app)"
    apps/windows/scripts/link-check.sh --arch x64
    apps/windows/scripts/link-check.sh --arch arm64
    for arch in x64 arm64; do
        apps/windows/scripts/build.sh --arch "$arch"
        python3 -I "$ROOT/scripts/release-verifie.py" pe "$LLVM_OBJDUMP" "apps/windows/dist/$arch"
    done
    # Un seul installateur pour les deux : il pose la version du processeur.
    apps/windows/scripts/build-installer.sh
    cp apps/windows/dist/KyberNow-Link-Setup.exe "$E/"
    note_kyber windows-app
}
construire_android() {
    cd "$(arbre_dir android)"
    apps/android/scripts/release-apk.sh "$E/KyberNow-Link-Android.apk"
    note_kyber android
}
construire_webos() {
    cd "$(arbre_dir webos)"
    apps/webos/scripts/build.sh
    cp "apps/webos/dist/fr.leslignesclaires.kybernow_${VERSION}_arm.ipk" "$E/KyberNow-Link-webOS.ipk"
    note_kyber webos
}
APP_REL="apps/macos/build/Build/Products/Release/KyberNow Link.app"
construire_app_mac() {
    cd "$(arbre_dir apple)"
    [ -d apps/macos/Vendor/Frameworks ] || apps/macos/scripts/vendor-kyber.sh
    if [ "$(apps/macos/scripts/build-kyclient.sh --stamp)" != "$(cat apps/macos/Vendor/kyclient.stamp 2>/dev/null)" ] \
        || otool -L apps/macos/Vendor/Frameworks/libkynput.0.1.0.dylib | grep -qi sdl; then
        apps/macos/scripts/build-kyclient.sh
    fi
    otool -L apps/macos/Vendor/Frameworks/libkynput.0.1.0.dylib | grep -qi sdl && { echo "libkynput lie encore SDL" >&2; exit 1; }
    apps/apple/scripts/build-apple.sh
    (cd apps/macos && xcodegen generate --quiet && xcodebuild -project KyberNow.xcodeproj -scheme KyberNow -configuration Release \
        -derivedDataPath build build)
    apps/macos/scripts/sign-release.sh "$APP_REL"
    codesign --verify --deep --strict "$APP_REL"
    plutil -extract CFBundleShortVersionString raw "$APP_REL/Contents/Info.plist" | grep -qxF "$VERSION"
    ditto -c -k --keepParent "$APP_REL" "$E/app-mac-signee.zip"
    note_kyber apple
}
construire_notarisation() {
    local w="$E/.travail" app
    mkdir -p "$w"
    ditto -x -k "$CACHE/app_mac/fichiers/app-mac-signee.zip" "$w"
    app="$w/KyberNow Link.app"
    if [ "$NOTARISER" = 1 ]; then
        # Profil enregistré une fois : xcrun notarytool store-credentials kybernow --apple-id … --team-id <KYBER_TEAM_ID de scripts/identite.sh>
        xcrun notarytool submit "$CACHE/app_mac/fichiers/app-mac-signee.zip" --keychain-profile kybernow --wait --timeout 30m | tee "$w/notarytool.txt"
        grep -q "status: Accepted" "$w/notarytool.txt" || { echo "notarisation refusée (xcrun notarytool log <id> --keychain-profile kybernow)" >&2; exit 1; }
        xcrun stapler staple "$app"
        xcrun stapler validate "$app"
        spctl --assess --type execute -vv "$app"
    fi
    ditto -c -k --keepParent "$app" "$E/$MAC_ZIP"
    rm -rf "$w"
}
construire_tvos() {
    cd "$(arbre_dir apple)"
    apps/tvos/scripts/build-kyber-tvos.sh
    # Le cœur Rust pour tvOS (apps/tvos/Vendor/KyberNow.xcframework), publié avec
    # le SDK dans KyberSDK-tvOS.zip : la CI et le sideload le reprennent.
    apps/apple/scripts/build-apple-mobile.sh tvos
    (cd apps/tvos && xcodegen generate --quiet && xcodebuild -project KyberNowTV.xcodeproj -scheme KyberNowTV -configuration Release \
        -destination 'generic/platform=tvOS' -derivedDataPath build CODE_SIGNING_ALLOWED=NO build)
    (cd apps/tvos/Vendor && ditto -c -k . "$E/KyberSDK-tvOS.zip")
    note_kyber apple
}
construire_ios() {
    cd "$(arbre_dir apple)"
    # libkynput pour iOS (rootfs par cible), puis le cœur, libopus et les en-têtes
    # dans apps/ios/Vendor, publiés dans KyberSDK-iOS.zip pour apps/ios/scripts/sideload.sh.
    for t in aarch64-apple-ios aarch64-apple-ios-sim; do
        apps/core/scripts/build-apple.sh --target "$t"
    done
    apps/apple/scripts/build-apple-mobile.sh ios
    (cd apps/ios && xcodegen generate --quiet && xcodebuild -project KyberNowIOS.xcodeproj -scheme KyberNowIOS -configuration Release \
        -destination 'generic/platform=iOS' -derivedDataPath build CODE_SIGNING_ALLOWED=NO build)
    (cd apps/ios/Vendor && ditto -c -k . "$E/KyberSDK-iOS.zip")
    note_kyber apple
}
construire_station_mac() {
    local t
    t="$(arbre_dir station-mac)"
    cd "$t"
    mac_kyber_a_jour "$t" "$t" || server/macos/scripts/build-kyber.sh
    mac_kyber_a_jour "$t" "$t"
    if [ "$NOTARISER" = 1 ]; then server/macos/scripts/build-station-app.sh --notarize; else server/macos/scripts/build-station-app.sh; fi
    cp server/macos/dist/KyberNow-Station-Mac.dmg server/macos/dist/KyberNow-Station-Mac-update.zip "$E/"
    note_kyber station-mac
}
# Flatpak et .deb dans la VM, depuis l'arbre « linux » : sources de Kyber du
# Flatpak prises dans leurs dépôts git (d'origine), dossier de travail de
# flatpak-builder propre à la publication.
# shellcheck disable=SC2088  # le ~ est celui de la VM, développé là-bas
export KYBERNOW_FLATPAK_WORK='~/.cache/kybernow-flatpak-publication'
export KYBERNOW_KYBER_HEAD=1
construire_flatpak_aarch64() { cd "$(arbre_dir linux)"; apps/linux/scripts/release-flatpaks.sh --seulement aarch64 "$E"; }
construire_flatpak_x86_64() { cd "$(arbre_dir linux)"; apps/linux/scripts/release-flatpaks.sh --seulement x86_64 "$E"; }
construire_deb_pi() {
    cd "$(arbre_dir linux)"
    apps/linux/scripts/release-flatpaks.sh --seulement deb "$E"
    cp "apps/linux/dist/kybernow-link_${VERSION}_arm64.deb" "$E/"
    note_kyber linux
}
construire_link_os() {
    cd "$(arbre_dir linux)"
    # Le .deb de cette version (repris du cache s'il n'a pas été refait).
    mkdir -p apps/linux/dist
    cp "$CACHE/deb_pi/fichiers/kybernow-link_${VERSION}_arm64.deb" apps/linux/dist/
    apps/linux/os/release-image.sh "$E"
}
construire_station_linux() { cd "$(arbre_dir linux)"; server/linux/scripts/release-station.sh "$E"; }

# --- Voies -------------------------------------------------------------------
# Chaque voie, un sous-shell : sans le piège d'erreur du script (set -E le lui
# transmettrait, et on_error y rendrait les verrous du script : même $$).
voie_autres() { trap - ERR; export CARGO_BUILD_JOBS=$JOBS_MAC; etape surcouche; etape station; etape windows; etape android; etape webos; }
voie_apple() { trap - ERR; export CARGO_BUILD_JOBS=$JOBS_MAC; etape app_mac; etape tvos; etape ios; etape station_mac; }
voie_notarisation() { trap - ERR; etape notarisation; }
voie_linux() { trap - ERR; etape flatpak_aarch64; etape flatpak_x86_64; }
voie_linux_os() { trap - ERR; etape deb_pi; etape link_os; if [ "${KYBERNOW_STATION_LINUX:-1}" = 1 ]; then etape station_linux; fi; }

# --- Affichage ---------------------------------------------------------------
# shellcheck disable=SC2034  # lus par indirection (${!sym}, ${!nom})
SYM_attente="·" SYM_encours="▶" SYM_faite="✓" SYM_reprise="=" SYM_echec="✗" SYM_annulee="–"
# shellcheck disable=SC2034
NOM_attente="en attente" NOM_encours="en cours" NOM_faite="refaite" NOM_reprise="reprise" NOM_echec="ÉCHEC" NOM_annulee="annulée"
ligne_etape() {
    local id=$1 st d0 d1 s now
    now=$(date +%s)
    read -r st d0 d1 < "$RUN/etat/$id" 2>/dev/null || { st=attente d0=$now d1=$now; }
    case "$st" in encours) s=$((now - d0)) ;; faite | echec) s=$((d1 - d0)) ;; *) s="" ;; esac
    local sym="SYM_$st" nom="NOM_$st"
    printf '  %s %-27s %-10s %s' "${!sym}" "$(libelle "$id")" "${!nom}" "$([ -n "$s" ] && duree "$s")"
}
fin_journal() { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished|Installing) |replacing existing signature' "$LOGS/$1.log" 2>/dev/null | tail -"${2:-30}"; }
TTY=0
[ -t 1 ] && TTY=1
VU=()
LIGNES=0
DERNIER_POINT=$(date +%s)
afficher() {
    local id st evts="" now i=0
    now=$(date +%s)
    for id in "${ETAPES[@]}"; do
        i=$((i + 1))
        st="$(etat_de "$id")"
        [ "${VU[$i]:-}" = "$st" ] && continue
        VU[$i]="$st"
        case "$st" in
            faite | reprise | annulee) evts+="[$(date +%H:%M:%S)] $(ligne_etape "$id")"$'\n' ;;
            encours) [ "$TTY" = 1 ] || evts+="[$(date +%H:%M:%S)] $(ligne_etape "$id")"$'\n' ;;
            echec) evts+="[$(date +%H:%M:%S)] $(ligne_etape "$id") — fin de $LOGS/$id.log :"$'\n'"$(fin_journal "$id" 25 | sed 's/^/      /')"$'\n' ;;
        esac
    done
    if [ "$TTY" = 1 ]; then
        [ "$LIGNES" -gt 0 ] && printf '\033[%dA\033[J' "$LIGNES"
        printf '%s' "$evts"
        LIGNES=0
        for id in "${ETAPES[@]}"; do ligne_etape "$id"; printf '\033[K\n'; LIGNES=$((LIGNES + 1)); done
    else
        printf '%s' "$evts"
        if [ $((now - DERNIER_POINT)) -ge 300 ]; then
            DERNIER_POINT=$now
            local cours=""
            for id in "${ETAPES[@]}"; do [ "$(etat_de "$id")" = encours ] && cours+=" $(libelle "$id") ($(duree $((now - $(cut -d' ' -f2 "$RUN/etat/$id")))))," ; done
            [ -n "$cours" ] && echo "[$(date +%H:%M:%S)] en cours :${cours%,}"
        fi
    fi
}

# ---------------------------------------------------------------------------
step "Constructions (5 voies ; journaux : ${LOGS#"$ROOT"/})"
T0=$(date +%s)
PIDS=() VOIES=(autres apple notarisation linux linux_os)
for voie in "${VOIES[@]}"; do "voie_$voie" & PIDS+=($!); done
# Interruption : les voies et tout ce qu'elles ont lancé (les constructions
# dans la VM, lancées par ssh, y finissent leur commande en cours).
descendants() { local p; for p in $(pgrep -P "$1" 2>/dev/null); do descendants "$p"; echo "$p"; done; }
arreter_voies() {
    local p tous=()
    touch "$RUN/arret" 2>/dev/null || true
    for p in "${PIDS[@]}"; do tous+=($(descendants "$p") "$p"); done
    kill ${tous[@]+"${tous[@]}"} 2>/dev/null || true
}
# Les commandes lancées dans la VM par ssh y continuent : arrêtées aussi avant
# de rendre les verrous (elles écrivent dans l'arbre « linux »).
arreter_vm() {
    local a
    a="$(arbre_vm "$(arbre_dir linux)")"
    vm "pkill -f -- '$a' ; pkill -f -- kybernow-flatpak-publication ; true" > /dev/null 2>&1 &
    a=$!
    ( sleep 15; kill "$a" 2>/dev/null ) > /dev/null 2>&1 &
    wait "$a" 2>/dev/null || true
}
trap 'arreter_voies; arreter_vm; echo; echo "Interrompu." >&2; rendre_verrous; exit 130' INT TERM
# Une voie morte (set -e, disque plein…) avant d'avoir fini ses étapes : elles
# passent en échec, et les autres voies s'arrêtent.
voie_morte() {
    local i=$1 id st morte=0
    for id in "${ETAPES[@]}"; do
        [ "$(voie_de "$id")" = "${VOIES[$i]}" ] || continue
        st="$(etat_de "$id")"
        case "$st" in faite | reprise | echec | annulee) ;; *) etat "$id" echec 2>/dev/null || true; morte=1 ;; esac
    done
    [ "$morte" = 1 ] && { echo "voie « ${VOIES[$i]} » arrêtée avant la fin de ses étapes" >> "$LOGS/voies.log" 2>/dev/null || true; arreter_voies; }
    return 0
}
MORTES=()
while :; do
    afficher
    vivantes=0
    for i in "${!PIDS[@]}"; do
        if kill -0 "${PIDS[$i]}" 2>/dev/null; then vivantes=$((vivantes + 1))
        elif [ -z "${MORTES[$i]:-}" ]; then MORTES[$i]=1; voie_morte "$i"; fi
    done
    [ "$vivantes" -eq 0 ] && break
    sleep 1
done
wait "${PIDS[@]}" 2>/dev/null || true
afficher
CONSTRUCTIONS_S=$(( $(date +%s) - T0 ))

# Résumé : durée de chaque étape, refaite ou reprise.
resume() {
    local id st d0 d1 refaites=0 reprises=0
    {
        echo
        echo "Résumé ($MODE $VERSION, commit $(git rev-parse --short "$COMMIT")) :"
        printf '  %-27s %s\n' "Vérifications préalables" "$(duree "$PRECHECK_S")"
        printf '  %-27s %s\n' "Versions" "$(duree "$VERSIONS_S")"
        printf '  %-27s %s\n' "Arbres de construction" "$(duree "$ARBRES_S")"
        for id in "${ETAPES[@]}"; do
            read -r st d0 d1 < "$RUN/etat/$id" 2>/dev/null || st=attente
            case "$st" in faite) refaites=$((refaites + 1)) ;; reprise) reprises=$((reprises + 1)) ;; esac
            ligne_etape "$id"; echo
        done
        printf '  %-27s %s (en parallèle)\n' "Constructions" "$(duree "$CONSTRUCTIONS_S")"
        [ -n "${FIN_S:-}" ] && printf '  %-27s %s\n' "Vérifications et flux" "$(duree "$FIN_S")"
        printf '  %-27s %s (%d étapes refaites, %d reprises)\n' "Total" "$(duree $(( $(date +%s) - DEBUT_PUBLICATION )))" "$refaites" "$reprises"
    } | tee "$LOGS/resume.txt"
}

ECHECS=() INCOMPLETES=()
for id in "${ETAPES[@]}"; do
    case "$(etat_de "$id")" in
        echec) ECHECS+=("$id") ;;
        faite | reprise) ;;
        *) INCOMPLETES+=("$(libelle "$id")") ;;
    esac
done
if [ ${#ECHECS[@]} -gt 0 ] || [ ${#INCOMPLETES[@]} -gt 0 ] || [ -f "$RUN/arret" ]; then
    [ ${#INCOMPLETES[@]} -eq 0 ] || echo "Étapes non terminées : ${INCOMPLETES[*]}" >&2
    for id in ${ECHECS[@]+"${ECHECS[@]}"}; do
        echo >&2
        echo "Échec de « $(libelle "$id") ». Fin de $LOGS/$id.log :" >&2
        fin_journal "$id" 40 >&2
    done
    resume
    echo >&2
    echo "Rien n'a été publié. Corrigez, commitez, puis relancez la même commande : les étapes réussies sont reprises." >&2
    rendre_verrous
    exit 1
fi

# Les constructions ne doivent avoir modifié aucun fichier suivi des arbres :
# les binaires ne correspondraient plus au commit.
SALES=()
for v in station-windows windows-app android webos apple station-mac linux; do
    s="$(arbre_propre "$v")"
    [ -z "$s" ] || { SALES+=("$v"); echo "Arbre $v modifié par la construction :" >&2; printf '%s\n' "$s" | head -10 | sed 's/^/    /' >&2; }
done
[ ${#SALES[@]} -eq 0 ] || { echo "Les binaires ne correspondraient pas au commit ($(printf '%s ' ${SALES[@]+"${SALES[@]}"}))." >&2; resume; rendre_verrous; exit 1; }

# ---------------------------------------------------------------------------
step "Fichiers de la version (${OUT#"$ROOT"/})"
T0=$(date +%s)
for id in "${ETAPES[@]}"; do
    [ "$id" = surcouche ] || [ "$id" = app_mac ] || cp -c "$CACHE/$id"/fichiers/* "$OUT/" 2>/dev/null || cp "$CACHE/$id"/fichiers/* "$OUT/"
done
rm -f "$OUT/kybernow-link_${VERSION}_arm64.deb"
for f in "${COPIES[@]}"; do cp "$OUT/$(release_copie_de "$f")" "$OUT/$f"; done

step "Vérification des fichiers"
VERIF=0
python3 -I scripts/release-verifie.py fichiers "$OUT" "$VERSION" "${BUILT[@]}" KyberNow-Link-OS.img.xz.info ${STATION_LINUX[@]+"${STATION_LINUX[@]}"} || VERIF=1
V="$(mktemp -d)"; TEMPS+=("$V")
ditto -x -k "$OUT/$MAC_ZIP" "$V/app"
ditto -x -k "$OUT/KyberNow-Station-Mac-update.zip" "$V/station"
verifie() { local quoi=$1; shift; if "$@" > "$V/sortie" 2>&1; then echo "  ok   $quoi"; else echo "  NON  $quoi : $(tail -2 "$V/sortie" | tr '\n' ' ')"; VERIF=1; fi; }
verifie "app Mac : signature Developer ID" codesign --verify --deep --strict "$V/app/KyberNow Link.app"
verifie "Station Mac (mise à jour) : signature" codesign --verify --deep --strict "$V/station/KyberNow Station.app"
verifie "Station Mac (.dmg) : signature" codesign --verify "$OUT/KyberNow-Station-Mac.dmg"
if [ "$NOTARISER" = 1 ]; then
    verifie "app Mac : ticket de notarisation agrafé" xcrun stapler validate "$V/app/KyberNow Link.app"
    verifie "app Mac : Gatekeeper" spctl --assess --type execute "$V/app/KyberNow Link.app"
    verifie "Station Mac (mise à jour) : ticket agrafé" xcrun stapler validate "$V/station/KyberNow Station.app"
    verifie "Station Mac (.dmg) : ticket agrafé" xcrun stapler validate "$OUT/KyberNow-Station-Mac.dmg"
    verifie "Station Mac (.dmg) : Gatekeeper" spctl --assess --type open --context context:primary-signature "$OUT/KyberNow-Station-Mac.dmg"
fi
SDK_ANDROID="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
APKSIGNER="$(ls -d "$SDK_ANDROID"/build-tools/*/apksigner 2> /dev/null | sort -V | tail -1)"
AAPT2="$(ls -d "$SDK_ANDROID"/build-tools/*/aapt2 2> /dev/null | sort -V | tail -1)"
# L'empreinte attendue du certificat : sans elle, rien ne passe (un motif
# vide accepterait n'importe quelle clé).
APK_CERT="$(tr -d '[:space:]' < apps/android/scripts/release-cert.sha256 2>/dev/null || true)"
if [[ "$APK_CERT" =~ ^[0-9a-f]{64}$ ]]; then
    verifie "APK : signature (apksigner, clé de publication ${APK_CERT:0:16}…)" \
        sh -c "'$APKSIGNER' verify --print-certs '$OUT/KyberNow-Link-Android.apk' | grep -qiF 'SHA-256 digest: $APK_CERT'"
else
    echo "  NON  APK : apps/android/scripts/release-cert.sha256 absent ou illisible (empreinte SHA-256 du certificat de publication attendue)"
    VERIF=1
fi
verifie "APK : version $VERSION" sh -c "'$AAPT2' dump badging '$OUT/KyberNow-Link-Android.apk' | grep -q \"versionName='$VERSION'\""
rm -rf "$V"
[ "$VERIF" = 0 ] || { echo "Fichiers de la version en défaut : rien n'est publié." >&2; resume; rendre_verrous; exit 1; }

# ---------------------------------------------------------------------------
step "Flux de mises à jour de l'app (Sparkle, canal $CHANNEL)"
SPARKLE=$(ls -d "$(arbre_dir apple)"/apps/macos/build/SourcePackages/artifacts/*/*/bin apps/macos/build/SourcePackages/artifacts/*/*/bin 2>/dev/null | head -1 || true)
[ -x "$SPARKLE/generate_appcast" ] || { echo "generate_appcast de Sparkle introuvable (build de l'app Mac)" >&2; false; }
# Notes en deux langues (docs/i18n.md) : la release GitHub porte la section
# française, une ligne <!-- en -->, puis l'anglaise ; Admin et l'app Windows
# prennent la partie de leur langue.
section() { awk -v v="## $VERSION" '$0==v{f=1;next} /^## /{f=0} f' "$1"; }
NOTES_FR="$(git show "$COMMIT:CHANGELOG.md" | section /dev/stdin)"
NOTES_EN="$(git show "$COMMIT:CHANGELOG.en.md" | section /dev/stdin)"
printf '%s\n\n<!-- en -->\n\n%s\n' "$NOTES_FR" "$NOTES_EN" > "$OUT/notes.md"
# Flux Sparkle : l'anglais est la description par défaut (toute langue autre
# que le français la reçoit) ; le français est un lien par langue (xml:lang),
# servi par le site dans un dossier par version (les entrées plus anciennes du
# flux cumulatif gardent leurs propres notes).
NOTES_DIR="site/updates/notes/$VERSION"
FEED="$(mktemp -d)"; TEMPS+=("$FEED")
cp "$OUT/$MAC_ZIP" "$FEED/"
printf '%s\n' "$NOTES_EN" > "$FEED/${MAC_ZIP%.zip}.md"
printf '%s\n' "$NOTES_FR" | python3 scripts/notes-html.py > "$FEED/${MAC_ZIP%.zip}.fr.html"
CHANNEL_FLAG=()
[ "$BETA" = 1 ] && CHANNEL_FLAG=(--channel beta)
"$SPARKLE/generate_appcast" --download-url-prefix "https://github.com/$REPO/releases/download/$TAG/" \
    --release-notes-url-prefix "https://www.kybernow.app/updates/notes/$VERSION/" \
    ${CHANNEL_FLAG[@]+"${CHANNEL_FLAG[@]}"} --embed-release-notes -o "$OUT/appcast.xml" "$FEED"
mkdir -p "$OUT/notes" && cp "$FEED/${MAC_ZIP%.zip}.fr.html" "$OUT/notes/"
rm -rf "$FEED"
xmllint --noout "$OUT/appcast.xml"
python3 -I scripts/release-verifie.py fichiers "$OUT" "$VERSION" appcast.xml

step "Flux cumulatif du site ($SITE_FEED)"
# Le flux de cette version rejoint ceux déjà publiés (même version : la
# nouvelle entrée remplace l'ancienne, cas d'une relance). Les entrées
# gardent leurs liens vers leur release et leurs signatures.
BASE=()
[ -f "$SITE_FEED" ] && BASE=(--base "$SITE_FEED")
python3 scripts/merge-appcast.py -o "$OUT/appcast-site.xml" ${BASE[@]+"${BASE[@]}"} "--$CHANNEL" "$OUT/appcast.xml"
xmllint --noout "$OUT/appcast-site.xml"
# Le dépôt JSON de Raspberry Pi Imager (KyberNow Link OS de cette version), sur le site aussi.
IMAGER_JSON="site/updates/rpi-imager.json"
apps/linux/os/imager-json.sh "$VERSION" "$OUT/KyberNow-Link-OS.img.xz.info" "$OUT/rpi-imager.json" >/dev/null
if cmp -s "$OUT/appcast-site.xml" "$SITE_FEED" && cmp -s "$OUT/notes/${MAC_ZIP%.zip}.fr.html" "$NOTES_DIR/${MAC_ZIP%.zip}.fr.html" && cmp -s "$OUT/rpi-imager.json" "$IMAGER_JSON"; then
    echo "    $SITE_FEED déjà à jour"
elif [ "$MODE" = release ]; then
    mkdir -p "$(dirname "$SITE_FEED")" "$NOTES_DIR"
    cp "$OUT/appcast-site.xml" "$SITE_FEED"
    cp "$OUT/notes/${MAC_ZIP%.zip}.fr.html" "$NOTES_DIR/"
    cp "$OUT/rpi-imager.json" "$IMAGER_JSON"
    git add -- "$SITE_FEED" "$NOTES_DIR" "$IMAGER_JSON"
    # Relance : le commit du flux pas encore poussé est remplacé, pas doublé.
    if [ "$(git log -1 --format=%s)" = "$FEED_SUBJECT" ] && ! git merge-base --is-ancestor HEAD origin/main; then
        git commit -q --amend --no-edit
    else
        git commit -q -m "$FEED_SUBJECT"
    fi
    echo "    commit $(git rev-parse --short HEAD) « $FEED_SUBJECT » (publié par le workflow Pages au push)"
else
    echo "    [répétition] $SITE_FEED serait remplacé par ${OUT#"$ROOT"/}/appcast-site.xml, et $IMAGER_JSON par rpi-imager.json :"
    git --no-pager diff --no-index --stat -- "$SITE_FEED" "$OUT/appcast-site.xml" || true
    git --no-pager diff --no-index --stat -- "$IMAGER_JSON" "$OUT/rpi-imager.json" || true
    act git commit -m "$FEED_SUBJECT" -- "$SITE_FEED" "$NOTES_DIR" "$IMAGER_JSON"
fi

step "Empreintes et manifeste signé"
# Nouveaux et anciens noms des installateurs (même empreinte), comme le manifeste.
(cd "$OUT" && shasum -a 256 "${BUILT[@]}" $(release_fichiers flux) ${STATION_LINUX[@]+"${STATION_LINUX[@]}"} > SHA256SUMS)
scripts/sign-manifest.sh "$OUT" "$VERSION" "$CHANNEL" ${UPDATE_FLAGS[@]+"${UPDATE_FLAGS[@]}"}
for f in kybernow-release.json kybernow-release.json.minisig; do
    [ -s "$OUT/$f" ] || { echo "sign-manifest.sh n'a pas produit $f" >&2; false; }
done
python3 -I scripts/release-verifie.py manifeste "$OUT" "$VERSION" "$CHANNEL"
minisign -Vm "$OUT/kybernow-release.json" -p server/admin/src/update/kybernow-release.pub > /dev/null
echo "  ok   signature du manifeste (clé publique d'Admin)"
FIN_S=$(( $(date +%s) - T0 ))

# ---------------------------------------------------------------------------
step "Release GitHub en brouillon"
FILES=()
for f in "${ASSETS[@]}" ${STATION_LINUX[@]+"${STATION_LINUX[@]}"}; do FILES+=("${OUT#"$ROOT"/}/$f"); done
PRERELEASE=false
[ "$BETA" = 1 ] && PRERELEASE=true
if [ "$RELEASE_STATE" = draft ]; then
    act gh release upload "$TAG" -R "$REPO" --clobber "${FILES[@]}"
    act gh release edit "$TAG" -R "$REPO" --title "KyberNow $VERSION" --notes-file "${OUT#"$ROOT"/}/notes.md" --prerelease="$PRERELEASE"
else
    # Pas de tag encore : GitHub le crée à la publication s'il n'existe pas ;
    # ici il existera (étape suivante), --target n'est qu'une valeur requise.
    act gh release create "$TAG" -R "$REPO" --draft --target main --title "KyberNow $VERSION" \
        --notes-file "${OUT#"$ROOT"/}/notes.md" --prerelease="$PRERELEASE" "${FILES[@]}"
fi

step "Tag $TAG et push"
if git rev-parse -q --verify "refs/tags/$TAG" > /dev/null; then
    echo "    tag $TAG déjà posé sur $(git rev-parse --short "$(git rev-list -n 1 "$TAG")")"
else
    act git tag -a "$TAG" -m "KyberNow $VERSION" "$COMMIT"
fi
# main : le commit de version et, par-dessus, celui du flux du site.
act git push --atomic origin "HEAD:refs/heads/main" "refs/tags/$TAG"

step "Publication"
LATEST=true
[ "$BETA" = 1 ] && LATEST=false
act gh release edit "$TAG" -R "$REPO" --draft=false --latest="$LATEST"
resume
rendre_verrous
if [ "$MODE" = release ]; then
    gh release view "$TAG" -R "$REPO" --json isDraft,isPrerelease,url --jq '"\(.url) (brouillon : \(.isDraft), prerelease : \(.isPrerelease))"'
    echo "Publiée : https://github.com/$REPO/releases/tag/$TAG"
else
    echo
    echo "Répétition terminée : fichiers dans ${OUT#"$ROOT"/}$([ "$NOTARISER" = 1 ] && echo " (notarisés)"), rien n'a été commité, poussé ni publié."
fi
