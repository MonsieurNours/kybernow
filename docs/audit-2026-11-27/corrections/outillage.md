# Corrections de l'audit du 27/11 — outillage (transverse, partie A)

Branche `claude/fix-outillage-73093a`. Zone : `scripts/`, `tools/`, `.github/workflows` (hors workflow des essais du cœur), `docs/releases*.md`, `docs/build-notes*.md`, plus `server/linux/scripts/x86-chroot.sh` pour TRANS-08 (demandé par la coordinatrice).

## Constats corrigés

| ID | Correction |
|---|---|
| TRANS-02 | Station Linux : son empreinte compte `apps/macos/kyber-patches`, `apps/webos/kyber-patches`, `LICENSE`/`LICENSE.md`, `apps/linux/scripts/vm.sh` et les commits des sources de Kyber de la VM et du chroot x86_64 (`VM_KYBER_STATION`, lus par ssh pendant les vérifications). |
| TRANS-03 | Station Mac : `apps/macos/PyroWave` (COMMIT et `upstream`) dans les chemins de l'étape ; Kyber pour Mac recompilé quand l'arbre git de `apps/macos/PyroWave` change (marqueur `.kybernow-pyrowave` posé par `release.sh` à côté de `.kybernow-patches`). |
| TRANS-04 | L'empreinte des outils compte `rustup run nightly rustc -V` (celui que lancent les scripts), sans `|| true` ; vérification préalable : nightly absent = échec, autre date = note. |
| TRANS-05 | `kyber_apply_client_patches` remet à l'origine les fichiers de tous les correctifs connus de kyctl, kymux et kynput (apps et Stations), pas seulement ceux du build. |
| TRANS-10 | Verrou périmé retiré sous une garde (`<chemin>.verrou-reprise`) et rejugé sous elle ; `qui` écrit à côté puis renommé ; nom de machine `scutil --get LocalHostName` sur macOS, l'ancien `hostname -s` toujours reconnu. |
| TRANS-11 | Le contrôle du certificat de l'APK exige une empreinte SHA-256 lisible dans `release-cert.sha256` (sinon échec), et cherche `SHA-256 digest: <empreinte>` (`grep -F`). |
| TRANS-12 | `relay-image.yml` : job d'essais du relais (`cargo test --locked --workspace`) avant l'image (`needs`). Nouveau `.github/workflows/outillage.yml` : `scripts/tests/release-outils.sh` (macOS), essais du relais, compilation de `tools/banc`. `release-outils.sh` est aussi une vérification préalable de `release.sh`, et cité dans `docs/releases.md`. |
| TRANS-13 | `relay-image.yml` : entrées et champs de l'événement passés par `env:`, version validée par une expression régulière, dans un job `version` à part. |
| TRANS-14 | `set -E` ; `on_error` ne fait rien dans un sous-shell (il y rendait les verrous du script) ; `trap - ERR` au début de chaque voie ; piège `EXIT` pour les dossiers temporaires ; une voie morte avant d'avoir fini met ses étapes en échec et arrête les autres (fin de l'attente sans fin). |
| TRANS-15 | Le Flatpak x86_64 attend aussi la Station Linux (toutes deux sous Rosetta) ; Ctrl-C arrête dans la VM les commandes lancées depuis l'arbre `linux` (`pkill -f`, 15 s au plus) avant de rendre les verrous. |
| TRANS-16 | Recette (`declare -f construire_<id>`) et liste des fichiers produits dans l'empreinte ; `apps/tvos` dans les entrées de l'app iPhone/iPad ; chemins du job tvOS de la CI complétés (correctifs `*-kynput-*`, `*-kynet-*`, `*-kycontroller-*`, `*-kywebplayer-*`, webOS, `kyber-client-patches.sh`, `kyber-toolchain.sh`, `apps/Cargo.*`). |
| TRANS-17 | Vérification préalable (note) quand les commits des sources de Kyber d'un arbre diffèrent du `third_party/` principal ; `scripts/arbres.sh recopier <variante>` ; `local.properties` recopié dès qu'il change. |
| TRANS-24 | `scripts/release-fichiers.txt` lu par `release.sh` (construits, copies des anciens noms, `SHA256SUMS`, release), `sign-manifest.sh` et `release-verifie.py` ; `scripts/release-etapes.txt` lu par `release.sh` et `release-suivi.py` ; `release-verifie.py manifeste` exige que tout fichier publié présent soit dans `SHA256SUMS` et dans le manifeste (la boucle `pass` est remplacée) ; docstring de `release-suivi.py` corrigée (journal par `tee`). |
| TRANS-08 | `x86-chroot.sh setup` : abandon si `findmnt` voit encore un montage sous le chroot, puis `rm -rf --one-file-system`. |
| TRANS-52 | `tools/latency/run_client.sh` : identifiants de curl dans un fichier de configuration `chmod 600`, retiré à la sortie. |
| (trouvé) | `tools/banc` ne compilait plus (champ `feedback` ajouté à `Second` dans le cœur) : réparé ; la CI le compile désormais. |

**Une seule fonction d'entrées par produit** : `scripts/release-entrees.sh` (`release_chemins`, `release_kyber`, `release_tp`, `release_empreintes`), lue par `release.sh` (`entrees_de`, qui remplace les quinze `entrees_<id>`) et par l'essai. `scripts/tests/entrees-couvertes.py` relit chaque `construire_<id>`, puis, sur trois niveaux, les scripts du dépôt qu'il lance et les `project.yml` de xcodegen : un fichier suivi cité hors de l'empreinte fait échouer l'essai. Exceptions motivées : `scripts/tests/entrees-exceptions.tsv` (deux).

## Constats laissés

| ID | Raison |
|---|---|
| TRANS-03 (reste) | `server/macos/scripts/build-kyber.sh:170` garde sa propre liste (zone Stations Mac/Linux, STCOM-11) ; `release.sh` ne la change pas pour que l'empreinte écrite par le script reste comparable. À tirer d'une fonction commune en vague 2. |
| TRANS-04 (reste) | Épingler `cargo +nightly-$KYBER_NIGHTLY` dans `apps/tvos/scripts/build-kyber-tvos.sh`, `apps/apple/scripts/build-apple-mobile.sh` et `apps/core/scripts/build-apple.sh` : scripts des apps. L'empreinte suit déjà le nightly réellement lancé. |
| TRANS-15 (reste) | N'amorcer le cache Flatpak que pour une architecture demandée : `apps/linux/scripts/release-flatpaks.sh` (zone app Linux). |
| TRANS-21 | Tout est dans des scripts d'app ou de Station (pi-gen non figé, chaîne webOS `latest`, `curl | tar`, marqueur de `relocate`, `trap` de `build-native.sh`, `webos/build.sh`, `camera/build.sh`, `make-ab-image.sh`). Rien dans ma zone. |
| TRANS-52 (reste) | Le client de Kyber ne lit son mot de passe qu'en `--auth-password` (pas de variable d'environnement) : il reste visible par `ps` ; commentaire dans le script. |
| TRANS-02 (reste) | **Le dépôt n'a aucun fichier `LICENSE`** : `server/linux/scripts/package.sh:90` (`install … || true`) et `apps/linux/scripts/sources.sh:74` le sautent sans bruit, la Station Linux part sans `copyright`. Décision de l'auteur (fichier de licence à la racine). |
| TRANS-19, 20 | Factorisation des scripts d'app : vague 2. |
| TRANS-06, 07, 09, 18, 22, 23 | Scripts de construction des apps et des Stations : autres sessions. |

## Ce qui est vérifié, et comment

- `scripts/tests/release-outils.sh` : 43 essais, tous passent (verrous : garde de reprise, verrou neuf jamais retiré, noms de machine ; correctif connu non demandé remis à l'origine ; toutes les recettes couvertes ; quatre manques volontaires vus, dont PyroWave de la Station Mac, `apps/tvos` pour iOS, correctifs des apps pour la Station Linux ; étape sans recette ; même liste de fichiers pour `release.sh` et `release-verifie.py` ; fichier publié oublié par le manifeste refusé).
- `bash -n` de tous les scripts, `shellcheck -S warning -s bash` sans avertissement (`scripts/`, `tools/latency`, `x86-chroot.sh`), `actionlint` sans remarque sur les trois workflows.
- `entrees_de` et `deps_de` extraits de `release.sh` et lancés sur un arbre vide : quinze empreintes, stables d'un appel à l'autre, changées quand une recette change ; dépendances du Flatpak x86_64 : `link_os`, `station_linux`.
- `cargo test --locked --workspace` de `server/relay` : 22 essais sur 23 ; `un_bout_qui_ne_lit_pas_ses_reponses_est_coupe` a échoué une fois dans la suite entière et passe seul (sensible au minutage) : à regarder côté relais, la nouvelle CI le fera tourner. `cargo build --locked` de `tools/banc` : passe.
- Échappement du fichier de configuration de curl (guillemets, barres obliques inverses) essayé à part.
- **Pas vérifié** : `release.sh` de bout en bout (interdit pendant la publication 0.11.8), les workflows sur GitHub, les parties par ssh (VM, chroot x86_64), `x86-chroot.sh` dans la VM.

**À la fusion** : toutes les empreintes d'étapes changent (recette et nouvelles entrées) : la première publication après la fusion refait chaque étape une fois, et Kyber pour Mac est recompilé une fois (marqueur `.kybernow-pyrowave` absent). Rien d'autre à faire.

## Notes de version

FR :
- Chaque version est mieux vérifiée avant de vous parvenir : les mises à jour se reconstruisent dès que ce qui les compose change, et le relais passe ses essais avant chaque nouvelle image.

EN:
- Every release is checked more thoroughly before it reaches you: updates are rebuilt as soon as anything they're made of changes, and the relay passes its tests before each new image.
