#!/bin/bash
# Essais des outils de publication, sans rien construire (quelques secondes) :
# verrous (scripts/verrou.sh), remise à l'origine et état noté des sources de
# Kyber (scripts/arbres.sh), correctifs client non réécrits
# (scripts/kyber-client-patches.sh), vérification des fichiers
# (scripts/release-verifie.py).
#
#   scripts/tests/release-outils.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
ECHECS=0
vrai() { if "$@"; then echo "  ok   ${MSG:-$*}"; else echo "  NON  ${MSG:-$*}"; ECHECS=$((ECHECS + 1)); fi; }
faux() { if "$@"; then echo "  NON  ${MSG:-$*}"; ECHECS=$((ECHECS + 1)); else echo "  ok   ${MSG:-$*}"; fi; }

echo "== verrous"
. "$ROOT/scripts/verrou.sh"
MSG="un verrou libre se prend" vrai kyber_verrou_prendre "$T/sources" essai
MSG="le même processus le reprend sans attendre (imbrication)" vrai kyber_verrou_prendre "$T/sources" essai
# Un autre processus vivant le tient : refus, avec son nom.
rm -rf "$T/autre.verrou"; mkdir "$T/autre.verrou"
sleep 30 & AUTRE=$!
printf 'pid=%s\ndebut=%s\nmachine=%s\ncommande=build-essai.sh\n' "$AUTRE" "$(kyber_verrou_debut "$AUTRE")" "$(hostname -s)" > "$T/autre.verrou/qui"
MSG="tenu par un processus vivant : refusé" faux kyber_verrou_prendre "$T/autre" essai 2> "$T/msg"
MSG="le refus nomme la commande qui le tient" vrai grep -q build-essai.sh "$T/msg"
kill "$AUTRE"; wait "$AUTRE" 2>/dev/null || true
MSG="processus terminé : verrou périmé, repris" vrai kyber_verrou_prendre "$T/autre" essai
kyber_verrou_rendre "$T/autre"
MSG="rendu : le dossier du verrou disparaît" faux test -d "$T/autre.verrou"
# Pid réutilisé par un autre processus (date de démarrage différente) : périmé.
mkdir "$T/pid.verrou"
printf 'pid=%s\ndebut=Thu Jan  1 00:00:00 1970\nmachine=%s\n' "$$" "$(hostname -s)" > "$T/pid.verrou/qui"
MSG="pid réutilisé (autre date de démarrage) : repris" vrai kyber_verrou_prendre "$T/pid" essai

echo "== sources de Kyber : origine et état noté"
. "$ROOT/scripts/arbres.sh"
KD="$T/arbre/third_party/kyber-desktop"
mkdir -p "$KD/kysdk/kyctl" "$T/arbre/server/kyber-patches" "$T/arbre/apps/macos/kyber-patches"
( cd "$KD/kysdk/kyctl" && git init -q && printf 'a\nb\n' > f.txt && git add f.txt && git -c user.name=t -c user.email=t@t commit -qm o )
# Un correctif qui modifie f.txt et ajoute n.rs.
cat > "$T/arbre/apps/macos/kyber-patches/0001-kyclient-essai.patch" << 'EOF'
--- a/f.txt
+++ b/f.txt
@@ -1,2 +1,2 @@
 a
-b
+c
--- /dev/null
+++ b/n.rs
@@ -0,0 +1 @@
+nouveau
EOF
(cd "$KD/kysdk/kyctl" && git apply "$T/arbre/apps/macos/kyber-patches/0001-kyclient-essai.patch")
echo "fichier à moi" > "$KD/kysdk/kyctl/garde.txt"
E1="$(arbre_sources_etat "$KD")"
arbre_sources_origine "$KD" "$T/arbre"
MSG="origine : fichier corrigé remis" vrai grep -qx b "$KD/kysdk/kyctl/f.txt"
MSG="origine : fichier ajouté par un correctif retiré" faux test -e "$KD/kysdk/kyctl/n.rs"
MSG="origine : autre fichier non suivi gardé" vrai test -e "$KD/kysdk/kyctl/garde.txt"
MSG="l'état change avec les sources" vrai test "$E1" != "$(arbre_sources_etat "$KD")"
E2="$(arbre_sources_etat "$KD")"
MSG="l'état est stable sans changement" vrai test "$E2" = "$(arbre_sources_etat "$KD")"

echo "== correctifs client : non réécrits quand déjà posés"
. "$ROOT/scripts/kyber-client-patches.sh"
KYBER_CLIENT_PATCHES_ROOT="$T/arbre"    # pas de correctifs de la Station ici
mkdir -p "$KD/kysdk/kymux" "$KD/kysdk/kynput"
for r in kymux kynput; do (cd "$KD/kysdk/$r" && git init -q && touch x && git add x && git -c user.name=t -c user.email=t@t commit -qm o); done
P="$T/arbre/apps/macos/kyber-patches/0001-kyclient-essai.patch"
kyber_apply_client_patches "$KD/kysdk" "$P" > /dev/null
D1="$(stat -f %m "$KD/kysdk/kyctl/f.txt")"
sleep 1.1
SORTIE="$(kyber_apply_client_patches "$KD/kysdk" "$P")"
MSG="deuxième pose : rien à faire" vrai grep -q "déjà posés" <<< "$SORTIE"
MSG="deuxième pose : fichier pas réécrit (date inchangée)" vrai test "$D1" = "$(stat -f %m "$KD/kysdk/kyctl/f.txt")"
echo "x" >> "$KD/kysdk/kyctl/f.txt"
SORTIE="$(kyber_apply_client_patches "$KD/kysdk" "$P")"
MSG="sources touchées depuis : correctif reposé" vrai grep -q "posé" <<< "$SORTIE"
MSG="… et le fichier est celui du correctif" vrai test "$(cat "$KD/kysdk/kyctl/f.txt")" = "$(printf 'a\nc')"

echo "== vérification des fichiers"
mkdir -p "$T/dist"
head -c 2000000 /dev/zero > "$T/dist/gros.bin"
printf 'x' > "$T/dist/petit.bin"
python3 -I "$ROOT/scripts/release-verifie.py" fichiers "$T/dist" 1.2.3 gros.bin > /dev/null && R=0 || R=$?
MSG="fichier de bonne taille accepté" vrai test "$R" = 0
python3 -I "$ROOT/scripts/release-verifie.py" fichiers "$T/dist" 1.2.3 petit.bin absent.bin > /dev/null && R=0 || R=$?
MSG="fichier tronqué et fichier absent refusés (2 problèmes)" vrai test "$R" = 2
(cd "$T/dist" && shasum -a 256 gros.bin > SHA256SUMS)
python3 - "$T/dist" << 'EOF'
import json, os, sys, hashlib
d = sys.argv[1]
h = hashlib.sha256(open(os.path.join(d, "gros.bin"), "rb").read()).hexdigest()
json.dump({"version": "1.2.3", "channel": "stable", "files": {"gros.bin": {"sha256": h, "size": 2000000}}}, open(os.path.join(d, "kybernow-release.json"), "w"))
EOF
python3 -I "$ROOT/scripts/release-verifie.py" manifeste "$T/dist" 1.2.3 stable > /dev/null && R=0 || R=$?
MSG="manifeste cohérent accepté" vrai test "$R" = 0
printf 'y' >> "$T/dist/gros.bin"
python3 -I "$ROOT/scripts/release-verifie.py" manifeste "$T/dist" 1.2.3 stable > /dev/null && R=0 || R=$?
MSG="fichier modifié après le manifeste : refusé" vrai test "$R" -gt 0

echo
if [ "$ECHECS" = 0 ]; then echo "Tous les essais passent."; else echo "$ECHECS essai(s) en échec."; exit 1; fi
