#!/bin/bash
# Essai de la liste des correctifs des apps Apple (scripts/client-patches.sh) :
# la pose et l'empreinte de Vendor/kyclient.stamp voient les mêmes fichiers,
# correctifs de la Station compris (MAC-03), PyroWave selon KYBER_PYROWAVE.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
MAC="$HERE/../.."
REPO="$(cd "$MAC/../.." && pwd)"
FAKE="$(mktemp -d)"
trap 'rm -rf "$FAKE"' EXIT
mkdir -p "$FAKE/apps/macos/kyber-patches" "$FAKE/server/kyber-patches"
touch "$FAKE/apps/macos/kyber-patches/0001-kyclient-a.patch" \
      "$FAKE/server/kyber-patches/0022-kyclient-hdr.patch" \
      "$FAKE/server/kyber-patches/0025-kyclient-pyrowave.patch" \
      "$FAKE/server/kyber-patches/0027-kynput-stylet.patch" \
      "$FAKE/server/kyber-patches/0050-kynet-retour.patch" \
      "$FAKE/server/kyber-patches/0060-kyavserver-x.patch"
. "$REPO/scripts/kyber-client-patches.sh"
. "$MAC/scripts/client-patches.sh"
KYBER_CLIENT_PATCHES_ROOT="$FAKE"

fail=0
check() { if [ "$2" != "$3" ]; then echo "ÉCHEC : $1"; echo "  attendu : $3"; echo "  obtenu  : $2"; fail=1; fi; }
names() { while IFS= read -r f; do basename "$f"; done | tr '\n' ' '; }

check "liste posée, PyroWave allumé" "$(KYBER_PYROWAVE=1 mac_client_patches "$FAKE" | names)" \
    "0001-kyclient-a.patch 0022-kyclient-hdr.patch 0025-kyclient-pyrowave.patch "
check "liste posée, PyroWave coupé" "$(KYBER_PYROWAVE=0 mac_client_patches "$FAKE" | names)" \
    "0001-kyclient-a.patch 0022-kyclient-hdr.patch "
check "empreinte : correctifs de la Station compris" "$(KYBER_PYROWAVE=1 mac_client_patch_files "$FAKE" | names)" \
    "0027-kynput-stylet.patch 0050-kynet-retour.patch 0001-kyclient-a.patch 0022-kyclient-hdr.patch 0025-kyclient-pyrowave.patch "

# L'empreinte réelle change quand un correctif kynput de la Station change.
before="$("$MAC/scripts/build-kyclient.sh" --stamp)"
kynput="$(ls "$REPO"/server/kyber-patches/*-kynput-*.patch | head -1)"
cp "$kynput" "$FAKE/sauve"
trap 'cp "$FAKE/sauve" "$kynput"; rm -rf "$FAKE"' EXIT
echo "# essai" >> "$kynput"
after="$("$MAC/scripts/build-kyclient.sh" --stamp)"
cp "$FAKE/sauve" "$kynput"
[ "$before" != "$after" ] || { echo "ÉCHEC : l'empreinte ignore $(basename "$kynput")"; fail=1; }

[ "$fail" = 0 ] && echo "correctifs : tout passe"
exit "$fail"
