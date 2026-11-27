#!/bin/bash
# Essais des outils de publication, sans rien construire (quelques secondes) :
# verrous (scripts/verrou.sh), remise à l'origine et état noté des sources de
# Kyber (scripts/arbres.sh), correctifs client non réécrits et remise à
# l'origine de tout ce que touchent les correctifs connus
# (scripts/kyber-client-patches.sh), entrées des étapes : chaque fichier lu
# par une recette est dans son empreinte (scripts/release-entrees.sh,
# scripts/tests/entrees-couvertes.py), liste des fichiers publiés et
# vérification des fichiers (scripts/release-verifie.py). Lancé par la CI
# (.github/workflows/outillage.yml) et par les vérifications préalables de
# scripts/release.sh. macOS (stat -f, ps -o lstart).
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
MSG="« qui » écrit d'un coup (pas de fichier intermédiaire laissé)" faux test -e "$T/pid.verrou/qui.$$"
# Reprise d'un verrou périmé sous garde : deux constructions qui le jugent
# périmé ensemble ne retirent pas, la seconde, le verrou neuf de la première.
mkdir "$T/garde.verrou"
printf 'pid=999999\ndebut=x\nmachine=%s\n' "$(hostname -s)" > "$T/garde.verrou/qui"
mkdir "$T/garde.verrou-reprise"
kyber_verrou_reprendre "$T/garde.verrou"
MSG="garde tenue par une autre reprise : le verrou n'est pas retiré" vrai test -f "$T/garde.verrou/qui"
rmdir "$T/garde.verrou-reprise"
kyber_verrou_reprendre "$T/garde.verrou"
MSG="garde libre : verrou périmé retiré, garde rendue" vrai test ! -e "$T/garde.verrou" -a ! -e "$T/garde.verrou-reprise"
mkdir "$T/neuf.verrou"
kyber_verrou_reprendre "$T/neuf.verrou"
MSG="verrou neuf sans « qui » (moins d'une minute) : jamais retiré" vrai test -d "$T/neuf.verrou"
# Nom de la machine : celui de macOS (scutil), stable ; l'ancien (hostname -s) reconnu.
MSG="nom de machine : verrou écrit sous « hostname -s » reconnu d'ici" vrai kyber_verrou_ici "$(hostname -s)"
MSG="nom de machine : verrou écrit sous le nom local reconnu d'ici" vrai kyber_verrou_ici "$(kyber_verrou_machine)"
MSG="nom de machine : une autre machine n'est pas d'ici" faux kyber_verrou_ici "kybernow-vm-essai"

# Rendu à la sortie, avec un trap existant plein de guillemets (piège du 25/11).
cat > "$T/sortie.sh" << FIN
set -euo pipefail
. "$ROOT/scripts/verrou.sh"
trap 'rm -rf "$T/x"; echo "trap d'"'"'avant"' EXIT
mkdir "$T/x"
kyber_verrou_sortie "$T/s1"
kyber_verrou_sortie "$T/s 2"
FIN
SORTIE="$(bash "$T/sortie.sh" 2>&1)"
MSG="verrous rendus à la sortie, trap d'avant gardé" vrai test "$SORTIE" = "trap d'avant"
MSG="… et les dossiers des verrous ont disparu" faux test -e "$T/s1.verrou" -o -e "$T/s 2.verrou" -o -e "$T/x"

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

# arbre_kyber_origine : sans condition, même quand les sources sont dans
# l'état noté (la surcouche recompilée parce que son empreinte a changé sans
# que les correctifs changent : build.sh les trouvait à moitié posés).
(cd "$KD/kysdk/kyctl" && git apply "$T/arbre/apps/macos/kyber-patches/0001-kyclient-essai.patch")
ARBRES_AVANT="$ARBRES"; ARBRES="$T/arbres"
mkdir -p "$T/arbres/station-windows/third_party"
cp -R "$T/arbre/server" "$T/arbre/apps" "$T/arbres/station-windows/"
ln -s "$KD" "$T/arbres/station-windows/third_party/kyber-desktop"
arbre_kyber_noter station-windows
SORTIE="$(arbre_kyber_pret station-windows)"
MSG="sources dans l'état noté : arbre_kyber_pret n'y touche pas" vrai grep -q "rien à remettre" <<< "$SORTIE"
MSG="… le correctif reste posé" vrai grep -qx c "$KD/kysdk/kyctl/f.txt"
arbre_kyber_origine station-windows
MSG="arbre_kyber_origine : remises à l'origine quand même" vrai grep -qx b "$KD/kysdk/kyctl/f.txt"
MSG="… fichier ajouté retiré, état noté effacé" vrai test ! -e "$KD/kysdk/kyctl/n.rs" -a ! -e "$KD/.kybernow-arbre"
ARBRES="$ARBRES_AVANT"

echo "== empreinte de la surcouche Windows (server/kyber-patches/stamp.sh)"
# Calculée dans la copie où elle tourne (l'arbre, au commit de version), et
# sensible aux sources et aux Cargo.toml des aides micro, caméra, encodeur.
git clone -q --no-checkout --shared "$ROOT" "$T/copie"
git -C "$T/copie" checkout -q HEAD -- server/kyber-patches server/micro server/camera server/encoder-check apps/macos/kyber-patches apps/webos/kyber-patches
S0="$("$T/copie/server/kyber-patches/stamp.sh")"
MSG="stable d'un calcul à l'autre" vrai test "$S0" = "$("$T/copie/server/kyber-patches/stamp.sh")"
AIDES=(server/kyber-patches server/micro server/camera server/encoder-check apps/macos/kyber-patches apps/webos/kyber-patches)
if git -C "$ROOT" diff --quiet HEAD -- "${AIDES[@]}"; then
    MSG="même valeur que dans le dépôt au même commit" vrai test "$S0" = "$("$ROOT/server/kyber-patches/stamp.sh")"
fi
sed -i '' -E '1,10s/^version = "(.*)"/version = "\1-essai"/' "$T/copie/server/micro/Cargo.toml"
S1="$("$T/copie/server/kyber-patches/stamp.sh")"
MSG="version de l'aide micro changée (commit de version) : empreinte changée" vrai test "$S1" != "$S0"
MSG="… et calculée dans sa copie, pas dans le dépôt d'où l'on lance" vrai test "$S1" != "$("$ROOT/server/kyber-patches/stamp.sh")"
git -C "$T/copie" checkout -q HEAD -- server/micro
echo "// essai" >> "$(git -C "$T/copie" ls-files -- server/camera | grep -E '\.(c|m|mm|rs|cpp)$' | head -1 | sed "s|^|$T/copie/|")"
MSG="source de l'aide caméra changée : empreinte changée" vrai test "$("$T/copie/server/kyber-patches/stamp.sh")" != "$S0"

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
# Un correctif connu (ici d'une autre app) posé par un build précédent, que
# ce build ne pose pas : ses fichiers reviennent quand même à l'origine.
( cd "$KD/kysdk/kyctl" && printf 'g\n' > g.txt && git add g.txt && git -c user.name=t -c user.email=t@t commit -qm g )
mkdir -p "$T/arbre/apps/webos/kyber-patches"
cat > "$T/arbre/apps/webos/kyber-patches/0001-kyclient-autre.patch" << 'EOF'
--- a/g.txt
+++ b/g.txt
@@ -1 +1 @@
-g
+h
EOF
(cd "$KD/kysdk/kyctl" && git apply "$T/arbre/apps/webos/kyber-patches/0001-kyclient-autre.patch")
kyber_apply_client_patches "$KD/kysdk" "$P" > /dev/null
MSG="correctif connu non demandé : son fichier remis à l'origine" vrai test "$(cat "$KD/kysdk/kyctl/g.txt")" = g
MSG="… le correctif demandé reste posé" vrai test "$(cat "$KD/kysdk/kyctl/f.txt")" = "$(printf 'a\nc')"
MSG="correctifs connus : apps et Stations, seulement kyctl, kymux et kynput" vrai test "$(kyber_known_client_patches | wc -l | tr -d ' ')" = 2

echo "== entrées des étapes : ce que lit chaque recette est dans son empreinte"
. "$ROOT/scripts/kyber-toolchain.sh"
. "$ROOT/scripts/release-entrees.sh"
(cd "$ROOT" && for id in $(release_etapes); do echo "$id"; release_chemins_couverts "$id" | sed "s/^/$id	/"; done) > "$T/chemins.tsv"
couvertes() { python3 -I "$ROOT/scripts/tests/entrees-couvertes.py" "$ROOT" "$ROOT/scripts/release.sh" "$1" "$ROOT/scripts/tests/entrees-exceptions.tsv"; }
couvertes_muet() { couvertes "$1" > /dev/null; }
couvertes "$T/chemins.tsv" > "$T/couv" && R=0 || R=$?
sed 's/^/    /' "$T/couv"
MSG="tous les fichiers lus par les recettes sont dans leur empreinte" vrai test "$R" = 0
MSG="chaque étape a un libellé" vrai test -z "$(for id in $(release_etapes); do [ -n "$(release_libelle "$id")" ] || echo "$id"; done)"
# L'essai voit bien un manque : chemins retirés exprès.
for manque in "station_mac	apps/macos/PyroWave" "ios	apps/tvos" "station_linux	apps/macos/kyber-patches" "station	scripts/verrou.sh"; do
    grep -vxF "$manque" "$T/chemins.tsv" > "$T/manque.tsv"
    MSG="sans « ${manque/	/ : } », le manque est vu" faux couvertes_muet "$T/manque.tsv"
done
printf 'essai\nessai\tapps/core\n' > "$T/inconnue.tsv"
MSG="étape sans recette construire_<id> : signalée" faux couvertes_muet "$T/inconnue.tsv"

echo "== fichiers publiés : une seule liste"
LISTE_SH="$(release_fichiers construit copie flux station-linux | sort)"
LISTE_PY="$(python3 -I "$ROOT/scripts/release-verifie.py" liste construit copie flux station-linux | sort)"
MSG="release.sh et release-verifie.py (sign-manifest.sh) lisent la même liste" vrai test "$LISTE_SH" = "$LISTE_PY"
MSG="chaque ancien nom est la copie d'un fichier construit" vrai test -z "$(for f in $(release_fichiers copie); do release_fichiers construit | grep -qxF "$(release_copie_de "$f")" || echo "$f"; done)"
MSG="aucun nom en double" vrai test -z "$(release_fichiers construit copie flux station-linux signature | sort | uniq -d)"

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
# Un fichier publié (de la liste) présent dans le dossier mais oublié par le manifeste.
head -c 2000000 /dev/zero > "$T/dist/KyberNow-Link-Android.apk"
(cd "$T/dist" && shasum -a 256 gros.bin KyberNow-Link-Android.apk > SHA256SUMS)
python3 - "$T/dist" << 'EOF'
import json, os, sys, hashlib
d = sys.argv[1]
h = hashlib.sha256(open(os.path.join(d, "gros.bin"), "rb").read()).hexdigest()
json.dump({"version": "1.2.3", "channel": "stable", "files": {"gros.bin": {"sha256": h, "size": os.path.getsize(os.path.join(d, "gros.bin"))}}}, open(os.path.join(d, "kybernow-release.json"), "w"))
EOF
python3 -I "$ROOT/scripts/release-verifie.py" manifeste "$T/dist" 1.2.3 stable > "$T/msg" && R=0 || R=$?
MSG="fichier publié absent du manifeste : refusé" vrai grep -q "manifeste : KyberNow-Link-Android.apk manque" "$T/msg"

echo
if [ "$ECHECS" = 0 ]; then echo "Tous les essais passent."; else echo "$ECHECS essai(s) en échec."; exit 1; fi
