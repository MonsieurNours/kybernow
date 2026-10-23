#!/bin/bash
# Publie une version de KyberNow sur GitHub (github.com/MonsieurNours/kybernow).
#
#   scripts/release.sh X.Y.Z                  version stable
#   scripts/release.sh X.Y.Z --beta           version bêta (prerelease, canal beta)
#   scripts/release.sh X.Y.Z --dry-run        essai : tout sauf commit, notarisation,
#                                             release, tag et push (affichés)
#   scripts/release.sh X.Y.Z --check          vérifications préalables seules
#   … --min-installer A.B.C --requires-setup  exigences de la mise à jour (manifeste)
#
# Ordre, pour que les binaires publiés soient ceux du commit étiqueté :
#   1. vérifications préalables (branche main, arbre propre et à jour avec
#      origin, CHANGELOG.md et CHANGELOG.en.md, surcouche Kyber compilée avec
#      les correctifs du dépôt, gh connecté, profil notarytool, sign-manifest.sh) ;
#   2. alignement des versions (Admin, icône, installateur de la Station, apps
#      KyberNow Link) et commit « KyberNow X.Y.Z » ;
#   3. construction depuis ce commit : installateur de la Station, paquet de
#      mise à jour, app Mac (Developer ID), apps Apple TV et iPhone/iPad et leurs
#      SDK, app Windows et son installateur, app Android (APK signé), app des
#      téléviseurs LG (paquet webOS) ; l'arbre doit rester propre (sinon
#      les binaires ne correspondraient pas au commit) ;
#   4. notarisation de l'app Mac (Apple), agrafage, Gatekeeper ;
#   5. flux Sparkle signé EdDSA (sparkle:channel beta pour une bêta),
#      SHA256SUMS, manifeste signé (scripts/sign-manifest.sh, minisign) ;
#      flux cumulatif du site (site/updates/appcast.xml, stables et bêtas,
#      lu par l'app Mac) mis à jour et commité après le commit de version ;
#   6. release GitHub en brouillon, avec tous les fichiers ;
#   7. tag vX.Y.Z sur le commit de version, 8. push de main (commit de
#      version et flux du site) et du tag (atomique), 9. publication.
#
# Relance après un échec : relancer la même commande. Le commit « KyberNow
# X.Y.Z » déjà en tête (ou juste sous le commit du flux du site) est
# réutilisé (pas de second commit), le brouillon existant reçoit les
# nouveaux fichiers, le tag est posé s'il manque.
#
# Prérequis : third_party/ prêt (docs/build-notes.md), surcouche Kyber à jour
# (server/kyber-patches/build.sh), SDK client de l'app Mac (apps/macos/Vendor,
# build-kyclient.sh), gh connecté, clé Sparkle dans le Trousseau, certificat
# Developer ID et profil de notarisation « kybernow », clé minisign de
# sign-manifest.sh, clé de l'app Android (apps/android/scripts/release-apk.sh),
# Android Studio (SDK et NDK 27.2.12479018) et cibles Rust Android. Voir docs/releases.md.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="MonsieurNours/kybernow"
cd "$ROOT"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

usage() {
    echo "usage : scripts/release.sh X.Y.Z [--beta] [--dry-run | --check] [--min-installer A.B.C] [--requires-setup]" >&2
    exit "${1:-1}"
}

# État de l'arbre de travail, sans les schémas partagés des projets Xcode :
# Xcode ouvert les réécrit dans son format dès que xcodegen les régénère, et
# ils n'entrent pas dans les binaires.
tree_state() {
    git status --porcelain "$@" | grep -v '\.xcodeproj/xcshareddata/xcschemes/' || true
}

VERSION="" BETA=0 MODE=release MIN_INSTALLER="" REQUIRES_SETUP=0
while [ $# -gt 0 ]; do
    case "$1" in
        --beta) BETA=1 ;;
        --dry-run) MODE=dry-run ;;
        --check) MODE=check ;;
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

TAG="v$VERSION"
OUT="$ROOT/dist/$TAG"
CHANNEL=stable
[ "$BETA" = 1 ] && CHANNEL=beta
APP="apps/macos/build/Build/Products/Release/KyberNow Link.app"
# Archive de l'app Mac (flux Sparkle, site) ; KyberNow-macOS.zip, son nom
# jusqu'à la 0.4.2, est publié en copie pour les liens existants.
MAC_ZIP=KyberNow-Link-macOS.zip
LEGACY_MAC=(KyberNow-macOS.zip)
OVERLAY="$ROOT/third_party/payload/kyber-overlay"
OVERLAY_STAMP="$OVERLAY.stamp"   # écrit par server/kyber-patches/build.sh
# Flux cumulatif des mises à jour de l'app Mac, publié par le site
# (www.kybernow.app/updates/appcast.xml) : toutes les versions, bêtas comprises.
SITE_FEED="site/updates/appcast.xml"
FEED_SUBJECT="Flux des mises à jour : KyberNow $VERSION"
# Installateurs : KyberNow-Station-Setup.exe (la Station) et
# KyberNow-Link-Setup.exe (app Windows). Leurs anciens noms sont publiés à
# l'identique (copies) quelques versions encore : l'app Windows en 0.3.1 et
# avant cherche KyberNow-Client-Setup.exe dans le manifeste et la release,
# Admin en 0.3.1 et avant renvoie vers KyberNow-Server-Setup.exe, et les liens
# existants (…/latest/download/) mènent à KyberNow-Setup.exe (nom d'avant la
# 0.3.0), KyberNow-Server-Setup.exe ou KyberNow-Client-Setup.exe. Voir
# docs/releases.md, « Anciens noms des installateurs ».
LEGACY_STATION=(KyberNow-Server-Setup.exe KyberNow-Setup.exe)
LEGACY_LINK=(KyberNow-Client-Setup.exe)
ASSETS=(KyberNow-Station-Setup.exe kybernow-update.zip update.ps1 "$MAC_ZIP" "${LEGACY_MAC[@]}" appcast.xml KyberSDK-tvOS.zip KyberSDK-iOS.zip
    KyberNow-Link-Setup.exe "${LEGACY_STATION[@]}" "${LEGACY_LINK[@]}" KyberNow-Link-Android.apk KyberNow-Link-webOS.ipk
    SHA256SUMS kybernow-release.json kybernow-release.json.minisig)
# Fichiers modifiés par l'alignement des versions (et rien d'autre dans le commit).
VERSION_FILES=(server/admin/Cargo.toml server/admin/Cargo.lock server/tray/Cargo.toml server/tray/Cargo.lock
    server/installer/Cargo.toml server/installer/Cargo.lock apps/macos/project.yml apps/macos/KyberNow.xcodeproj
    apps/tvos/project.yml apps/tvos/KyberNowTV.xcodeproj
    apps/ios/project.yml apps/ios/KyberNowIOS.xcodeproj
    apps/Cargo.toml apps/Cargo.lock apps/windows/tauri.conf.json
    apps/windows/setup/Cargo.toml apps/windows/setup/Cargo.lock)

# Exigences de la mise à jour, transmises à package-update.sh et au manifeste.
UPDATE_FLAGS=()
[ -n "$MIN_INSTALLER" ] && UPDATE_FLAGS+=(--min-installer "$MIN_INSTALLER")
[ "$REQUIRES_SETUP" = 1 ] && UPDATE_FLAGS+=(--requires-setup)

version_num() { local a b c; IFS=. read -r a b c <<< "$1"; echo $((a * 1000000 + b * 1000 + c)); }
# Ce qui modifie le dépôt, Apple ou GitHub n'est fait qu'en publication réelle ;
# en essai, la commande est affichée.
act() {
    if [ "$MODE" = release ]; then
        "$@"
    else
        printf '    [essai, non lancé]'; printf ' %q' "$@"; echo
    fi
}

STEP="vérifications préalables"
step() { STEP="$1"; echo; echo "==> $1"; }
on_error() {
    echo >&2
    echo "Échec pendant « $STEP » (ligne $1)." >&2
    if [ "$MODE" = release ]; then
        echo "Corrigez puis relancez : scripts/release.sh $VERSION$([ "$BETA" = 1 ] && echo " --beta")" \
            "(le commit de version et le brouillon déjà créés sont réutilisés)." >&2
    fi
}
trap 'on_error $LINENO' ERR

# ---------------------------------------------------------------------------
step "Vérifications préalables ($([ "$MODE" = release ] && echo publication || echo "$MODE"), canal $CHANNEL)"
PROBLEMS=()
ok() { echo "  ok   $1"; }
problem() { PROBLEMS+=("$1"); echo "  NON  $1" >&2; }

if [ -n "$MIN_INSTALLER" ]; then
    if ! [[ "$MIN_INSTALLER" =~ $SEMVER ]]; then problem "--min-installer : version X.Y.Z attendue ($MIN_INSTALLER)"
    elif [ "$(version_num "$MIN_INSTALLER")" -gt "$(version_num "$VERSION")" ]; then
        problem "--min-installer $MIN_INSTALLER est plus récent que la version publiée $VERSION"
    else ok "installateur minimal : $MIN_INSTALLER"; fi
fi
[ "$REQUIRES_SETUP" = 1 ] && ok "mise à jour réservée à l'installateur (requires_setup)"

# Git : main, propre, à jour.
BRANCH="$(git symbolic-ref -q --short HEAD || echo "(tête détachée)")"
if [ "$BRANCH" = main ]; then ok "branche main"; else problem "branche « $BRANCH » : on ne publie que depuis main"; fi
DIRTY="$(tree_state --untracked-files=normal)"
if [ -z "$DIRTY" ]; then
    ok "arbre de travail propre"
else
    problem "arbre de travail modifié (commitez d'abord, CHANGELOG compris) :"
    printf '%s\n' "$DIRTY" | head -15 | sed 's/^/         /' >&2
fi
if git fetch -q origin main 2>/dev/null; then
    if git merge-base --is-ancestor origin/main HEAD; then ok "à jour avec origin/main"
    else problem "HEAD n'inclut pas origin/main : git pull (le push serait refusé)"; fi
else
    problem "origin inaccessible (git fetch origin main)"
fi

# Relance : le commit de version est déjà en tête, ou juste sous celui du
# flux du site (échec après l'étape 5).
RELAUNCH=0
VERSION_REF=HEAD
if [ "$(git log -1 --format=%s)" = "$FEED_SUBJECT" ] && [ "$(git log -1 --format=%s HEAD~1 2> /dev/null)" = "KyberNow $VERSION" ]; then
    VERSION_REF=HEAD~1
fi
if [ "$(git log -1 --format=%s "$VERSION_REF")" = "KyberNow $VERSION" ]; then
    if grep -qxF "version = \"$VERSION\"" server/admin/Cargo.toml; then
        RELAUNCH=1; ok "commit « KyberNow $VERSION » déjà fait$([ "$VERSION_REF" != HEAD ] && echo ", flux du site commité par-dessus") : relance, pas de nouveau commit"
    else
        problem "le commit « KyberNow $VERSION » est là mais server/admin/Cargo.toml n'a pas cette version"
    fi
fi
if git rev-parse -q --verify "refs/tags/$TAG" > /dev/null; then
    if [ "$RELAUNCH" = 1 ] && [ "$(git rev-list -n 1 "$TAG")" = "$(git rev-parse "$VERSION_REF")" ]; then
        ok "tag $TAG déjà posé sur le commit de version (relance)"
    else
        problem "le tag $TAG existe déjà sur $(git rev-list -n 1 "$TAG" | cut -c1-7) (version publiée, ou reste d'un essai raté : git tag -d $TAG)"
    fi
fi

# Notes de version, en français et en anglais.
for f in CHANGELOG.md CHANGELOG.en.md; do
    if grep -qxF "## $VERSION" "$f"; then ok "$f : section ## $VERSION"
    else problem "$f : ajoutez une section « ## $VERSION »"; fi
done

# Surcouche Kyber compilée avec les correctifs du dépôt.
patches_hash() { cat server/kyber-patches/*.patch server/kyber-patches/build.sh | shasum -a 256 | cut -d' ' -f1; }
if [ ! -d "$OVERLAY" ]; then
    problem "surcouche Kyber absente ($OVERLAY) : server/kyber-patches/build.sh"
elif [ -f "$OVERLAY_STAMP" ]; then
    if [ "$(cat "$OVERLAY_STAMP")" = "$(patches_hash)" ]; then ok "surcouche Kyber compilée avec les correctifs actuels (empreinte)"
    else problem "surcouche Kyber compilée avec d'autres correctifs (empreinte différente) : server/kyber-patches/build.sh"; fi
else
    STALE=()
    for f in server/kyber-patches/*.patch server/kyber-patches/build.sh; do
        [ -n "$(find "$OVERLAY" -type f ! -newer "$f" -print -quit)" ] && STALE+=("$(basename "$f")")
    done
    if [ ${#STALE[@]} -eq 0 ]; then
        ok "surcouche Kyber plus récente que les correctifs (pas d'empreinte : relancez build.sh pour en avoir une)"
    else
        problem "surcouche Kyber plus ancienne que ${#STALE[@]} fichier(s) de server/kyber-patches (${STALE[0]}…), et sans empreinte : relancez server/kyber-patches/build.sh (il écrit $(basename "$OVERLAY_STAMP"))"
    fi
fi

# SDK client de Kyber des apps (hors git, compilés à part).
if [ -d apps/macos/Vendor ]; then ok "SDK Kyber de l'app Mac (apps/macos/Vendor)"
else problem "apps/macos/Vendor absent : apps/macos/scripts/build-kyclient.sh"; fi
if [ "$MODE" != release ]; then
    # L'essai ne recompile pas le SDK Apple TV (build-kyber-tvos.sh applique
    # les correctifs dans third_party/, partagé) : il reprend celui en place.
    if [ -d apps/tvos/Vendor/KyberSDK.xcframework ]; then ok "SDK Kyber de l'app Apple TV en place (repris par l'essai)"
    else problem "apps/tvos/Vendor absent : l'essai ne le compile pas (apps/tvos/scripts/build-kyber-tvos.sh)"; fi
fi

# App Windows : bibliothèques natives (hors git) des deux architectures.
. scripts/kyber-toolchain.sh
for f in third_party/rootfs-win64/lib/libavcodec.a third_party/rootfs-win64-kynput/bin/kynput.dll \
    third_party/rootfs-winarm64/lib/libavcodec.a third_party/rootfs-winarm64/bin/kynput.dll \
    "third_party/toolchains/llvm-mingw-$KYBER_LLVM_MINGW-ucrt-macos-universal/bin/aarch64-w64-mingw32-clang"; do
    if [ -e "$f" ]; then ok "app Windows : $f"
    else problem "app Windows : $f absent (server/kyber-patches/build.sh, apps/windows/scripts/build-arm64-deps.sh, docs/build-notes.md)"; fi
done

# App Android : clé de publication (Trousseau) et outils (JDK, SDK, apksigner).
if CHECK="$(apps/android/scripts/release-apk.sh --check 2>&1)"; then ok "app Android : ${CHECK##*$'\n'}"
else problem "app Android : ${CHECK##*$'\n'}"; fi

# App des téléviseurs LG : sources de Kyber (kysdk) ; la chaîne webOS est
# téléchargée par apps/webos/scripts/build.sh si elle manque.
if [ -d third_party/kyber-desktop/kysdk/kyctl ]; then ok "app webOS : sources de Kyber"
else problem "app webOS : third_party/kyber-desktop/kysdk absent (docs/build-notes.md)"; fi
if rustup target list --installed 2>/dev/null | grep -qx armv7-unknown-linux-gnueabi; then ok "app webOS : cible Rust armv7-unknown-linux-gnueabi"
else problem "app webOS : rustup target add armv7-unknown-linux-gnueabi"; fi

# Flux cumulatif du site : amorcé une fois par scripts/rebuild-appcast.sh.
if [ -f "$SITE_FEED" ]; then ok "flux cumulatif du site ($SITE_FEED)"
else problem "$SITE_FEED absent : scripts/rebuild-appcast.sh (une fois, à commiter)"; fi

# Outils et comptes.
for tool in xcodegen xcodebuild npm cargo zstd x86_64-w64-mingw32-gcc zip ditto python3 xmllint; do
    command -v "$tool" > /dev/null || problem "$tool introuvable"
done
RELEASE_STATE=none
if gh auth status -h github.com > /dev/null 2>&1; then
    ok "gh connecté à github.com"
    if INFO="$(gh release view "$TAG" -R "$REPO" --json isDraft --jq .isDraft 2>&1)"; then
        if [ "$INFO" = true ]; then RELEASE_STATE=draft; ok "brouillon $TAG existant : réutilisé"
        else RELEASE_STATE=published; problem "$TAG est déjà publiée sur GitHub"; fi
    elif ! grep -qi "not found" <<< "$INFO"; then
        problem "lecture des releases de $REPO impossible : $INFO"
    fi
else
    problem "gh non connecté : gh auth login (jeton limité au dépôt, voir docs/releases.md)"
fi
if xcrun notarytool history --keychain-profile kybernow > /dev/null 2>&1; then
    ok "profil de notarisation « kybernow »"
else
    problem "profil de notarisation « kybernow » illisible : Mac verrouillé, ou xcrun notarytool store-credentials (docs/releases.md)"
fi
if [ -x scripts/sign-manifest.sh ]; then
    ok "scripts/sign-manifest.sh (manifeste signé minisign)"
else
    problem "scripts/sign-manifest.sh absent : sans manifeste signé, Admin refuse la mise à jour (phase 2 « signature et updater » à fusionner)"
fi
# Aucune KyberNow ne doit tourner : le cœur Rust de l'app Mac
# (apps/macos/Vendor/Frameworks/libkybernow.dylib) et les bibliothèques de
# l'app Release sont réécrits (docs/build-notes.md, « App ouverte »).
if pgrep -x "KyberNow( Link)?" > /dev/null; then problem "quittez KyberNow Link (⌘Q) : le cœur Rust de l'app Mac est recompilé"; fi

if [ ${#PROBLEMS[@]} -gt 0 ]; then
    echo >&2
    echo "${#PROBLEMS[@]} vérification(s) en échec :" >&2
    printf '  - %s\n' "${PROBLEMS[@]}" >&2
    case "$MODE" in
        release) echo "Rien n'a été modifié." >&2; exit 1 ;;
        dry-run) echo "Une publication s'arrêterait ici ; l'essai continue pour montrer la suite." >&2 ;;
    esac
else
    echo; echo "Toutes les vérifications préalables passent."
fi
if [ "$MODE" = check ]; then
    cat << EOF

Publication prévue ($CHANNEL) :
  2. versions alignées sur $VERSION, $([ "$RELAUNCH" = 1 ] && echo "commit déjà fait (relance)" || echo "commit « KyberNow $VERSION »")
  3. installateur de la Station, paquet de mise à jour${UPDATE_FLAGS[*]+ (${UPDATE_FLAGS[*]})}, app Mac,
     apps Apple TV et iPhone/iPad et leurs SDK, app Windows et son installateur, app Android,
     app des téléviseurs LG (KyberNow-Link-webOS.ipk)
     (anciens noms des installateurs publiés en copie : ${LEGACY_STATION[*]} ${LEGACY_LINK[*]})
  4. notarisation de l'app Mac
  5. appcast.xml$([ "$BETA" = 1 ] && echo " (sparkle:channel beta)"), SHA256SUMS, kybernow-release.json signé (canal $CHANNEL),
     $SITE_FEED mis à jour, commit « $FEED_SUBJECT »
  6. release $TAG en brouillon$([ "$BETA" = 1 ] && echo ", prerelease")$([ "$RELEASE_STATE" = draft ] && echo " (existante, fichiers remplacés)")
  7-9. tag $TAG sur le commit de version, push de main et du tag, publication
EOF
    exit $((${#PROBLEMS[@]} > 0))
fi

# Écran allumé jusqu'à la fin : le mot de passe de notarytool n'est lisible
# que Mac déverrouillé.
caffeinate -dimsu -w $$ &

# ---------------------------------------------------------------------------
step "Versions"
if [ "$MODE" = dry-run ]; then
    # L'essai aligne les versions pour construire comme la publication, puis
    # remet les fichiers tels qu'il les a trouvés (modifications comprises).
    BACKUP="$(mktemp -d)"
    tar -cf "$BACKUP/versions.tar" "${VERSION_FILES[@]}"
    trap 'tar -xf "$BACKUP/versions.tar" && rm -rf "$BACKUP" && echo "(essai : fichiers de version remis en état)"' EXIT
fi
if [ "$RELAUNCH" = 1 ]; then
    echo "    déjà alignées (commit « KyberNow $VERSION »)"
else
    for f in server/admin/Cargo.toml server/tray/Cargo.toml server/installer/Cargo.toml apps/windows/setup/Cargo.toml; do
        sed -i '' -E "1,10s/^version = \".*\"/version = \"$VERSION\"/" "$f"
        (cd "$(dirname "$f")" && cargo update --workspace --offline --quiet)
    done
    BUILD=$(date +%Y%m%d%H%M)
    # Clients en Rust (cœur et app Windows) : version de l'espace de travail.
    sed -i '' -E "s/^version = \".*\"/version = \"$VERSION\"/" apps/Cargo.toml
    sed -i '' -E "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" apps/windows/tauri.conf.json
    (cd apps && cargo update --workspace --offline --quiet)
    for d in apps/macos apps/tvos apps/ios; do
        sed -i '' -E "s/MARKETING_VERSION: \".*\"/MARKETING_VERSION: \"$VERSION\"/; s/CURRENT_PROJECT_VERSION: \".*\"/CURRENT_PROJECT_VERSION: \"$BUILD\"/" "$d/project.yml"
        (cd "$d" && xcodegen generate --quiet)
    done
    git --no-pager diff --stat -- "${VERSION_FILES[@]}"
    if [ "$MODE" = release ]; then
        git add -- "${VERSION_FILES[@]}"
        git commit -q -m "KyberNow $VERSION"
        echo "    commit $(git rev-parse --short HEAD) « KyberNow $VERSION »"
    else
        act git commit -m "KyberNow $VERSION"
    fi
fi
# Commit étiqueté : celui de la version (le commit du flux du site, s'il est
# déjà là, ne touche qu'à site/ : les binaires sont les mêmes).
COMMIT="$(git rev-parse "$VERSION_REF")"
# Tout ce qui est construit doit venir de ce commit : la construction ne doit
# toucher à aucun fichier suivi.
TREE_BEFORE="$(tree_state)"

# ---------------------------------------------------------------------------
step "KyberNow Station : installateur et paquet de mise à jour"
# Sortie de cargo sans les lignes de progression ; avec pipefail, le code de
# retour reste celui de build.sh.
server/installer/build.sh 2>&1 > /dev/null \
    | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) ' || true; }
server/installer/package-update.sh ${UPDATE_FLAGS[@]+"${UPDATE_FLAGS[@]}"}

step "Cœur Rust de l'app Mac (apps/core, apps/apple)"
# libkybernow.dylib et son en-tête dans apps/macos/Vendor (docs/coeur-apple.md).
apps/apple/scripts/build-apple.sh 2>&1 > /dev/null \
    | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) ' || true; }

step "App Mac"
(cd apps/macos && xcodegen generate --quiet && xcodebuild -project KyberNow.xcodeproj -scheme KyberNow -configuration Release \
    -derivedDataPath build build 2>&1 | grep -E "error:|BUILD (SUCC|FAIL)")
apps/macos/scripts/sign-release.sh "$APP"

rm -rf "$OUT" && mkdir -p "$OUT"

step "App Apple TV (SDK de Kyber pour tvOS, compilation de vérification)"
if [ "$MODE" = release ]; then
    apps/tvos/scripts/build-kyber-tvos.sh > /dev/null
else
    echo "    [essai] SDK repris de apps/tvos/Vendor (build-kyber-tvos.sh non lancé)"
fi
# Le cœur Rust pour tvOS (apps/tvos/Vendor/KyberNow.xcframework), publié avec
# le SDK dans KyberSDK-tvOS.zip : la CI et le sideload le reprennent.
apps/apple/scripts/build-apple-mobile.sh tvos 2>&1 > /dev/null \
    | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) ' || true; }
(cd apps/tvos && xcodegen generate --quiet && xcodebuild -project KyberNowTV.xcodeproj -scheme KyberNowTV -configuration Release \
    -destination 'generic/platform=tvOS' -derivedDataPath build CODE_SIGNING_ALLOWED=NO build 2>&1 | grep -E "error:|BUILD (SUCC|FAIL)")
(cd apps/tvos/Vendor && ditto -c -k . "$OUT/KyberSDK-tvOS.zip")

step "App iPhone et iPad (cœur Rust et SDK, compilation de vérification)"
# libkynput pour iOS (rootfs par cible), puis le cœur, libopus et les en-têtes
# dans apps/ios/Vendor, publiés dans KyberSDK-iOS.zip pour apps/ios/scripts/sideload.sh.
for t in aarch64-apple-ios aarch64-apple-ios-sim; do
    apps/core/scripts/build-apple.sh --target "$t" 2>&1 > /dev/null \
    | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) ' || true; }
done
apps/apple/scripts/build-apple-mobile.sh ios 2>&1 > /dev/null \
    | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) ' || true; }
(cd apps/ios && xcodegen generate --quiet && xcodebuild -project KyberNowIOS.xcodeproj -scheme KyberNowIOS -configuration Release \
    -destination 'generic/platform=iOS' -derivedDataPath build CODE_SIGNING_ALLOWED=NO build 2>&1 | grep -E "error:|BUILD (SUCC|FAIL)")
(cd apps/ios/Vendor && ditto -c -k . "$OUT/KyberSDK-iOS.zip")

step "App Windows (KyberNow Link, x64 et ARM64) et son installateur"
# Comme pour la Station : avec pipefail, un échec de build.sh arrête tout
# (sinon l'exécutable d'un build précédent partirait dans la version).
for arch in x64 arm64; do
    apps/windows/scripts/build.sh --arch "$arch" 2>&1 > /dev/null \
        | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) ' || true; }
done
# Un seul installateur pour les deux : il pose la version du processeur.
apps/windows/scripts/build-installer.sh 2>&1 > /dev/null \
    | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) ' || true; }
cp apps/windows/dist/KyberNow-Link-Setup.exe "$OUT/"
for f in "${LEGACY_LINK[@]}"; do cp "$OUT/KyberNow-Link-Setup.exe" "$OUT/$f"; done

step "App Android (KyberNow Link, APK signé)"
# Bibliothèque native des trois architectures, R8, signature vérifiée par apksigner.
apps/android/scripts/release-apk.sh "$OUT/KyberNow-Link-Android.apk" 2>&1 \
    | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) ' || true; }

step "App des téléviseurs LG (KyberNow Link, paquet webOS)"
# Binaire armv7 et paquet .ipk (installé en mode développeur : ares-install ou
# webOS Dev Manager ; l'app signale la nouvelle version, sans s'installer).
apps/webos/scripts/build.sh 2>&1 \
    | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating|Finished) |^warning|^ *(-->|\||=|[0-9]+ \|)' || true; }
cp "apps/webos/dist/fr.leslignesclaires.kybernow_${VERSION}_arm.ipk" "$OUT/KyberNow-Link-webOS.ipk"

TREE_AFTER="$(tree_state)"
if [ "$TREE_AFTER" != "$TREE_BEFORE" ]; then
    echo "La construction a modifié des fichiers suivis : les binaires ne correspondraient pas au commit." >&2
    diff <(printf '%s\n' "$TREE_BEFORE") <(printf '%s\n' "$TREE_AFTER") >&2 || true
    [ "$MODE" = release ] && false
fi

# ---------------------------------------------------------------------------
step "Notarisation de l'app (Apple, quelques minutes)"
# Profil enregistré une fois : xcrun notarytool store-credentials kybernow --apple-id … --team-id P6DB682S4K
if [ "$MODE" = release ]; then
    ditto -c -k --keepParent "$APP" "$OUT/notarisation.zip"
    xcrun notarytool submit "$OUT/notarisation.zip" --keychain-profile kybernow --wait --timeout 30m
    rm "$OUT/notarisation.zip"
    xcrun stapler staple "$APP"
    spctl --assess --type execute -vv "$APP"
else
    act xcrun notarytool submit notarisation.zip --keychain-profile kybernow --wait --timeout 30m
    act xcrun stapler staple "$APP"
fi

ditto -c -k --keepParent "$APP" "$OUT/$MAC_ZIP"
for f in "${LEGACY_MAC[@]}"; do cp "$OUT/$MAC_ZIP" "$OUT/$f"; done
cp server/installer/dist/KyberNow-Station-Setup.exe server/installer/dist/kybernow-update.zip server/installer/dist/update.ps1 "$OUT/"
for f in "${LEGACY_STATION[@]}"; do cp "$OUT/KyberNow-Station-Setup.exe" "$OUT/$f"; done

step "Flux de mises à jour de l'app (Sparkle, canal $CHANNEL)"
SPARKLE=$(ls -d apps/macos/build/SourcePackages/artifacts/*/*/bin | head -1)
# Notes en deux langues (docs/i18n.md) : la release GitHub porte la section
# française, une ligne <!-- en -->, puis l'anglaise ; Admin et l'app Windows
# prennent la partie de leur langue.
section() { awk -v v="## $VERSION" '$0==v{f=1;next} /^## /{f=0} f' "$1"; }
NOTES_FR="$(section CHANGELOG.md)"
NOTES_EN="$(section CHANGELOG.en.md)"
printf '%s\n\n<!-- en -->\n\n%s\n' "$NOTES_FR" "$NOTES_EN" > "$OUT/notes.md"
# Flux Sparkle : l'anglais est la description par défaut (toute langue autre
# que le français la reçoit) ; le français est un lien par langue (xml:lang),
# servi par le site dans un dossier par version (les entrées plus anciennes du
# flux cumulatif gardent leurs propres notes).
NOTES_DIR="site/updates/notes/$VERSION"
FEED="$(mktemp -d)"
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

step "Flux cumulatif du site ($SITE_FEED)"
# Le flux de cette version rejoint ceux déjà publiés (même version : la
# nouvelle entrée remplace l'ancienne, cas d'une relance). Les entrées
# gardent leurs liens vers leur release et leurs signatures.
BASE=()
[ -f "$SITE_FEED" ] && BASE=(--base "$SITE_FEED")
python3 scripts/merge-appcast.py -o "$OUT/appcast-site.xml" ${BASE[@]+"${BASE[@]}"} "--$CHANNEL" "$OUT/appcast.xml"
xmllint --noout "$OUT/appcast-site.xml"
if cmp -s "$OUT/appcast-site.xml" "$SITE_FEED" && cmp -s "$OUT/notes/${MAC_ZIP%.zip}.fr.html" "$NOTES_DIR/${MAC_ZIP%.zip}.fr.html"; then
    echo "    $SITE_FEED déjà à jour"
elif [ "$MODE" = release ]; then
    mkdir -p "$(dirname "$SITE_FEED")" "$NOTES_DIR"
    cp "$OUT/appcast-site.xml" "$SITE_FEED"
    cp "$OUT/notes/${MAC_ZIP%.zip}.fr.html" "$NOTES_DIR/"
    git add -- "$SITE_FEED" "$NOTES_DIR"
    # Relance : le commit du flux pas encore poussé est remplacé, pas doublé.
    if [ "$(git log -1 --format=%s)" = "$FEED_SUBJECT" ] && ! git merge-base --is-ancestor HEAD origin/main; then
        git commit -q --amend --no-edit
    else
        git commit -q -m "$FEED_SUBJECT"
    fi
    echo "    commit $(git rev-parse --short HEAD) « $FEED_SUBJECT » (publié par le workflow Pages au push)"
else
    echo "    [essai] $SITE_FEED serait remplacé par ${OUT#"$ROOT"/}/appcast-site.xml :"
    git --no-pager diff --no-index --stat -- "$SITE_FEED" "$OUT/appcast-site.xml" || true
    act git commit -m "$FEED_SUBJECT" -- "$SITE_FEED" "$NOTES_DIR"
fi

step "Empreintes et manifeste signé"
# Nouveaux et anciens noms des installateurs (même empreinte), comme le manifeste.
(cd "$OUT" && shasum -a 256 KyberNow-Station-Setup.exe kybernow-update.zip update.ps1 "$MAC_ZIP" "${LEGACY_MAC[@]}" appcast.xml KyberSDK-tvOS.zip KyberSDK-iOS.zip \
    KyberNow-Link-Setup.exe "${LEGACY_STATION[@]}" "${LEGACY_LINK[@]}" KyberNow-Link-Android.apk KyberNow-Link-webOS.ipk > SHA256SUMS)
cat "$OUT/SHA256SUMS"
if [ -x scripts/sign-manifest.sh ]; then
    scripts/sign-manifest.sh "$OUT" "$VERSION" "$CHANNEL" ${UPDATE_FLAGS[@]+"${UPDATE_FLAGS[@]}"}
    for f in kybernow-release.json kybernow-release.json.minisig; do
        [ -s "$OUT/$f" ] || { echo "sign-manifest.sh n'a pas produit $f" >&2; false; }
    done
else
    echo "    [essai] scripts/sign-manifest.sh absent : manifeste non produit"
fi

# ---------------------------------------------------------------------------
step "Release GitHub en brouillon"
FILES=()
for f in "${ASSETS[@]}"; do FILES+=("dist/$TAG/$f"); done
PRERELEASE=false
[ "$BETA" = 1 ] && PRERELEASE=true
if [ "$RELEASE_STATE" = draft ]; then
    act gh release upload "$TAG" -R "$REPO" --clobber "${FILES[@]}"
    act gh release edit "$TAG" -R "$REPO" --title "KyberNow $VERSION" --notes-file "dist/$TAG/notes.md" --prerelease="$PRERELEASE"
else
    # Pas de tag encore : GitHub le crée à la publication s'il n'existe pas ;
    # ici il existera (étape suivante), --target n'est qu'une valeur requise.
    act gh release create "$TAG" -R "$REPO" --draft --target main --title "KyberNow $VERSION" \
        --notes-file "dist/$TAG/notes.md" --prerelease="$PRERELEASE" "${FILES[@]}"
fi

step "Tag $TAG et push"
if git rev-parse -q --verify "refs/tags/$TAG" > /dev/null; then
    echo "    tag $TAG déjà posé sur $(git rev-parse --short "$COMMIT")"
else
    act git tag -a "$TAG" -m "KyberNow $VERSION" "$COMMIT"
fi
# main : le commit de version et, par-dessus, celui du flux du site.
act git push --atomic origin "HEAD:refs/heads/main" "refs/tags/$TAG"

step "Publication"
LATEST=true
[ "$BETA" = 1 ] && LATEST=false
act gh release edit "$TAG" -R "$REPO" --draft=false --latest="$LATEST"
if [ "$MODE" = release ]; then
    gh release view "$TAG" -R "$REPO" --json isDraft,isPrerelease,url --jq '"\(.url) (brouillon : \(.isDraft), prerelease : \(.isPrerelease))"'
    echo "Publiée : https://github.com/$REPO/releases/tag/$TAG"
else
    echo
    echo "Essai terminé : fichiers dans ${OUT#"$ROOT"/} (app non notarisée), rien n'a été commité, poussé ni publié."
    [ ${#PROBLEMS[@]} -eq 0 ] || { echo "${#PROBLEMS[@]} vérification(s) préalable(s) en échec (voir le début)." >&2; exit 1; }
fi
