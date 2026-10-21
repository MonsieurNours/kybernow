#!/bin/bash
# Reconstruit le flux cumulatif des mises à jour de l'app Mac,
# site/updates/appcast.xml (www.kybernow.app/updates/appcast.xml), à partir
# des releases publiées sur GitHub, stables et bêtas.
#
#   scripts/rebuild-appcast.sh            écrit site/updates/appcast.xml
#   scripts/rebuild-appcast.sh -o FICHIER écrit ailleurs (pour comparer)
#
# Pour chaque release publiée (brouillons exclus) : son appcast.xml, signé
# par release.sh au moment de la publication, est repris tel quel (lien vers
# la release, signature EdDSA, notes). Une release qui a une app Mac mais pas
# d'appcast.xml est signée à nouveau par generate_appcast (clé Sparkle du
# Trousseau). Les prereleases sont marquées sparkle:channel beta.
#
# À lancer une fois pour amorcer le flux, ou pour le réparer ; ensuite,
# release.sh y ajoute chaque version. Le script ne commite rien.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="MonsieurNours/kybernow"
cd "$ROOT"

OUT="site/updates/appcast.xml"
case "${1:-}" in
    -o) [ $# -ge 2 ] || { echo "usage : scripts/rebuild-appcast.sh [-o FICHIER]" >&2; exit 1; }; OUT="$2" ;;
    -h | --help) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    "") ;;
    *) echo "usage : scripts/rebuild-appcast.sh [-o FICHIER]" >&2; exit 1 ;;
esac

gh auth status -h github.com > /dev/null 2>&1 || { echo "gh non connecté : gh auth login" >&2; exit 1; }
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

MERGE=()
while read -r TAG PRERELEASE; do
    CHANNEL=stable
    [ "$PRERELEASE" = true ] && CHANNEL=beta
    mkdir -p "$TMP/$TAG"
    if gh release download "$TAG" -R "$REPO" -p appcast.xml -D "$TMP/$TAG" > /dev/null 2>&1; then
        echo "  $TAG ($CHANNEL) : appcast.xml de la release"
    elif gh release download "$TAG" -R "$REPO" -p KyberNow-macOS.zip -D "$TMP/$TAG/archives" > /dev/null 2>&1; then
        # Release sans flux : generate_appcast le refait (clé Sparkle du Trousseau).
        SPARKLE=$(ls -d apps/macos/build/SourcePackages/artifacts/*/*/bin 2> /dev/null | head -1)
        [ -n "$SPARKLE" ] || { echo "$TAG : outils Sparkle absents, compilez l'app Mac une fois (xcodebuild)" >&2; exit 1; }
        gh release view "$TAG" -R "$REPO" --json body --jq .body > "$TMP/$TAG/archives/KyberNow-macOS.md"
        FLAGS=()
        [ "$CHANNEL" = beta ] && FLAGS=(--channel beta)
        "$SPARKLE/generate_appcast" --download-url-prefix "https://github.com/$REPO/releases/download/$TAG/" \
            ${FLAGS[@]+"${FLAGS[@]}"} --embed-release-notes -o "$TMP/$TAG/appcast.xml" "$TMP/$TAG/archives" > /dev/null
        echo "  $TAG ($CHANNEL) : pas d'appcast.xml, flux refait par generate_appcast"
    else
        echo "  $TAG : pas d'app Mac, ignorée"
        continue
    fi
    MERGE+=("--$CHANNEL" "$TMP/$TAG/appcast.xml")
done < <(gh release list -R "$REPO" --limit 500 --json tagName,isDraft,isPrerelease \
    --jq '.[] | select(.isDraft | not) | "\(.tagName) \(.isPrerelease)"')

[ ${#MERGE[@]} -gt 0 ] || { echo "aucune release avec l'app Mac" >&2; exit 1; }
mkdir -p "$(dirname "$OUT")"
echo "Flux cumulatif $OUT :"
python3 scripts/merge-appcast.py -o "$OUT" "${MERGE[@]}"
xmllint --noout "$OUT"
echo "Fait. À commiter avec le site (le workflow Pages publie site/ au push sur main)."
