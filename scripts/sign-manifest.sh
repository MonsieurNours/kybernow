#!/bin/bash
# Manifeste signé d'une version de KyberNow (kybernow-release.json et sa
# signature minisign kybernow-release.json.minisig), à publier avec la version :
#   scripts/sign-manifest.sh <dossier dist> <version> <canal> [--min-installer X.Y.Z] [--requires-setup]
#
# Le manifeste donne la version, le canal (stable ou beta), la date, la
# version minimale déjà installée pour une mise à jour depuis Admin, s'il faut
# repasser par KyberNow-Station-Setup.exe, et l'empreinte SHA-256 et la taille
# de chaque fichier publié présent dans le dossier. KyberNow Admin (service
# kyber-admin) refuse tout paquet dont le manifeste n'est pas signé par la clé
# publique compilée en lui : server/admin/src/update/kybernow-release.pub.
#
# Les installateurs y figurent sous leurs nouveaux noms (KyberNow-Station-Setup.exe,
# KyberNow-Link-Setup.exe) et sous leurs anciens noms, copies identiques
# publiées quelques versions encore : l'app Windows en 0.3.1 et avant cherche
# KyberNow-Client-Setup.exe ici (docs/releases.md, « Anciens noms des
# installateurs »).
#
# Clé privée : ~/Library/Application Support/KyberNow/minisign.key, chiffrée
# par un mot de passe aléatoire gardé dans le Trousseau (élément
# « kybernow-minisign »). Absente, elle est créée ici (minisign -G) et la clé
# publique est écrite dans le dépôt, à commiter. Sauvegarde hors ligne et
# remise sur un autre Mac : docs/releases.md.
#
# Essai sans toucher à la vraie clé : KYBERNOW_MINISIGN_KEY=<chemin.key> et
# KYBERNOW_MINISIGN_ITEM=<élément du Trousseau> (la clé publique est alors
# écrite à côté de la clé privée, pas dans le dépôt).
set -euo pipefail

usage() { echo "usage : scripts/sign-manifest.sh <dossier dist> <version> <stable|beta> [--min-installer X.Y.Z] [--requires-setup]" >&2; exit 1; }
[ $# -ge 3 ] || usage
DIST="$1" VERSION="$2" CHANNEL="$3"
shift 3
MIN_INSTALLER="0.2.0"
REQUIRES_SETUP=false
while [ $# -gt 0 ]; do
    case "$1" in
        --min-installer) MIN_INSTALLER="${2:-}"; shift 2 ;;
        --requires-setup) REQUIRES_SETUP=true; shift ;;
        *) usage ;;
    esac
done
version_ok() { [[ "$1" =~ ^[0-9]{1,9}\.[0-9]{1,9}\.[0-9]{1,9}$ ]]; }
version_ok "$VERSION" || { echo "version attendue : X.Y.Z (reçu : $VERSION)" >&2; exit 1; }
version_ok "$MIN_INSTALLER" || { echo "--min-installer attend X.Y.Z (reçu : $MIN_INSTALLER)" >&2; exit 1; }
[[ "$CHANNEL" == stable || "$CHANNEL" == beta ]] || { echo "canal attendu : stable ou beta (reçu : $CHANNEL)" >&2; exit 1; }
[ -d "$DIST" ] || { echo "dossier introuvable : $DIST" >&2; exit 1; }

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_PUB="$ROOT/server/admin/src/update/kybernow-release.pub"
KEY="${KYBERNOW_MINISIGN_KEY:-$HOME/Library/Application Support/KyberNow/minisign.key}"
ITEM="${KYBERNOW_MINISIGN_ITEM:-kybernow-minisign}"
if [ -n "${KYBERNOW_MINISIGN_KEY:-}" ]; then PUB="${KEY%.key}.pub"; else PUB="$REPO_PUB"; fi
MANIFEST="kybernow-release.json"
FILES=(KyberNow-Station-Setup.exe kybernow-update.zip update.ps1 KyberNow-Link-macOS.zip KyberNow-macOS.zip appcast.xml KyberSDK-tvOS.zip KyberSDK-iOS.zip
    KyberNow-Link-Setup.exe KyberNow-Link-Android.apk KyberNow-Link-webOS.ipk
    KyberNow-Link-Linux-x86_64.flatpak KyberNow-Link-Linux-aarch64.flatpak KyberNow-Link-RaspberryPi-arm64.deb KyberNow-Link-OS.img.xz
    KyberNow-Server-Setup.exe KyberNow-Setup.exe KyberNow-Client-Setup.exe)

if ! command -v minisign >/dev/null; then
    echo "==> minisign absent : installation (Homebrew)"
    brew install minisign
fi

# Mot de passe de la clé, lu dans le Trousseau (jamais sur la ligne de commande).
password() { security find-generic-password -s "$ITEM" -w 2>/dev/null; }

if [ ! -f "$KEY" ]; then
    if [ -z "${KYBERNOW_MINISIGN_KEY:-}" ] && [ -f "$REPO_PUB" ]; then
        echo "Clé privée absente ($KEY), mais une clé publique est déjà dans le dépôt." >&2
        echo "Remettez la sauvegarde (docs/releases.md, « Clé de signature des mises à jour ») ;" >&2
        echo "une nouvelle clé ferait refuser les mises à jour par toutes les Stations installées (Admin)." >&2
        exit 1
    fi
    echo "==> Clé de signature absente : création ($KEY)"
    mkdir -p "$(dirname "$KEY")"
    if ! password >/dev/null; then
        # Ajouté par « security -i » : le mot de passe ne passe pas dans la liste des processus.
        printf 'add-generic-password -a kybernow -s %s -l %s -w %s\n' "$ITEM" "$ITEM" "$(openssl rand -hex 32)" | security -i >/dev/null
    fi
    PW="$(password)" || { echo "mot de passe introuvable dans le Trousseau ($ITEM)" >&2; exit 1; }
    mkdir -p "$(dirname "$PUB")"
    printf '%s\n%s\n' "$PW" "$PW" | minisign -G -f -p "$PUB" -s "$KEY" > /dev/null 2> >(grep -v "^Password" >&2)
    chmod 600 "$KEY"
    # Copie de la clé publique à côté de la clé privée (sauvegarde).
    [ "$PUB" = "${KEY%.key}.pub" ] || cp "$PUB" "${KEY%.key}.pub"
    echo "    clé publique : $PUB"
    [ "$PUB" != "$REPO_PUB" ] || echo "    à commiter, puis à sauvegarder hors ligne avec la clé privée et son mot de passe (docs/releases.md)"
fi
[ -f "$PUB" ] || { echo "clé publique introuvable : $PUB" >&2; exit 1; }
PW="$(password)" || { echo "mot de passe de la clé introuvable dans le Trousseau (élément $ITEM) : Mac verrouillé, ou voir docs/releases.md" >&2; exit 1; }

echo "==> Manifeste $VERSION ($CHANNEL)"
ENTRIES=()
for f in "${FILES[@]}"; do
    [ -f "$DIST/$f" ] || continue
    SHA="$(shasum -a 256 "$DIST/$f" | cut -d' ' -f1)"
    SIZE="$(stat -f%z "$DIST/$f")"
    ENTRIES+=("$(printf '    "%s": { "sha256": "%s", "size": %s }' "$f" "$SHA" "$SIZE")")
    printf '    %-26s %s  %s octets\n' "$f" "$SHA" "$SIZE"
done
[ ${#ENTRIES[@]} -gt 0 ] || { echo "aucun fichier publiable dans $DIST (${FILES[*]})" >&2; exit 1; }
[ -f "$DIST/kybernow-update.zip" ] || echo "    attention : kybernow-update.zip absent, Admin ne pourra pas installer cette version" >&2
{
    printf '{\n'
    printf '  "version": "%s",\n' "$VERSION"
    printf '  "channel": "%s",\n' "$CHANNEL"
    printf '  "published": "%s",\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf '  "min_installer_version": "%s",\n' "$MIN_INSTALLER"
    printf '  "requires_setup": %s,\n' "$REQUIRES_SETUP"
    printf '  "files": {\n'
    for i in "${!ENTRIES[@]}"; do
        [ "$i" -eq $((${#ENTRIES[@]} - 1)) ] && printf '%s\n' "${ENTRIES[$i]}" || printf '%s,\n' "${ENTRIES[$i]}"
    done
    printf '  }\n'
    printf '}\n'
} > "$DIST/$MANIFEST"

echo "==> Signature (minisign, commentaire de confiance « kybernow $VERSION $CHANNEL »)"
rm -f "$DIST/$MANIFEST.minisig"
printf '%s\n' "$PW" | minisign -S -s "$KEY" -m "$DIST/$MANIFEST" -x "$DIST/$MANIFEST.minisig" \
    -c "signature de $MANIFEST (KyberNow $VERSION)" -t "kybernow $VERSION $CHANNEL" > /dev/null 2> >(grep -v "^Password" >&2)
# Contre la clé publique que compile Admin : une clé qui ne correspond
# pas au dépôt est vue ici, pas sur les ordinateurs.
minisign -V -q -p "$PUB" -m "$DIST/$MANIFEST" -x "$DIST/$MANIFEST.minisig" \
    || { echo "la signature ne se vérifie pas avec $PUB" >&2; exit 1; }
echo "Signé : $DIST/$MANIFEST et $MANIFEST.minisig"
