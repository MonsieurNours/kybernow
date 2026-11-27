# Vague 2 de l'audit du 27/11 — scripts de construction et de publication

Branche `claude/v2-scripts-150860`. Zone : `scripts/`, `apps/*/scripts`, `server/*/scripts`, `server/kyber-patches/{apply,build,pyrowave,stamp}.sh`, `server/camera/build.sh`, `.github/workflows`, `docs/releases*.md`, `docs/build-notes*.md`. Aussi, parce qu'ils portent l'identité de signature recopiée (TRANS-20) et ne sont la zone d'aucune autre session : les `build.sh` de `server/macos/{camera-extension,gamepads,audio-driver,display-agent}`. Aucun `.patch` touché.

## Ce qui est en un seul endroit maintenant

| Fichier | Ce qu'il porte |
|---|---|
| `scripts/kyber-toolchain.sh` | versions des outils, de FFmpeg (7.1.1, avec son SHA-256) et d'Opus (1.6.1) des apps ; `kyber_pyrowave_commit` (lit `apps/macos/PyroWave/COMMIT`) ; `cargo_quiet` ; `kyber_sha256` |
| `scripts/kyber-patches-lib.sh` (nouveau) | `kyber_patch_reset` (remise à l'origine, sous-projets txproto et libavconv compris), `kyber_patch_put` (pose stricte), `kyber_patch_posed`, `kyber_wrap_refresh`/`kyber_wrap_mark` (wraps de meson), `kyber_files_hash` |
| `scripts/kyber-client-patches.sh` | `kyber_client_patches`, `kyber_client_patch_files`, `kyber_apply_app_patches` : la liste et la pose des correctifs des apps, avec ou sans git |
| `server/kyber-patches/apply.sh` | pose des correctifs des Stations (`kyber_apply_server_patches`, `kyber_apply_own_patches`) ; empreinte des trois Stations (`kyber_station_inputs`, `kyber_station_hash`) |
| `scripts/identite.sh` (nouveau) | `KYBER_TEAM_ID`, `KYBER_SIGN_IDENTITY` |

## Fait

| ID | Correction |
|---|---|
| TRANS-19 | Commit de PyroWave lu dans `apps/macos/PyroWave/COMMIT` par `kyber_pyrowave_commit` : `server/kyber-patches/pyrowave.sh` (Stations Windows et Linux, app Android), `apps/windows/scripts/build-pyrowave.sh` (qui reprend `pyrowave_fetch` au lieu d'une copie), `apps/linux/scripts/build.sh` ; plus aucun commit en dur dans les scripts de ma zone. Marqueurs versionnés : FFmpeg de l'app Linux (version, ou commit du FFmpeg du Pi, et options), PyroWave de l'app Linux (commit et correction v3dv), FFmpeg et Opus de webOS, Opus d'Android ; une autre version les fait refaire. Archive de FFmpeg vérifiée (SHA-256) aussi sous Linux. `apps/webos/scripts/package.sh` : licence de FFmpeg prise à la version de `kyber-toolchain.sh`, son absence arrête l'empaquetage. |
| TRANS-20 | Pose : une seule pose par `patch` (`kyber_patch_put`), employée par les correctifs communs, ceux du Mac et de Linux, les crates du Mac, le moteur web et les sources sans git ; la remise à l'origine (`kyber_patch_reset`) sert aux apps et aux Stations. `cargo_quiet` : huit copies → une. Empreinte du wrap FFmpeg : trois copies → `kyber_wrap_refresh` (Lua de Linux aussi). Identité et Team ID : `scripts/identite.sh`, lu par `sign-release.sh` (qui accepte maintenant `KYBER_SIGN_IDENTITY` et retire son dossier temporaire), `build-station-app.sh`, `run-kyber.sh` et les quatre `build.sh` de `server/macos` ; les scripts de sideload remplacent l'équipe du projet quelle qu'elle soit. Versions : cargo-c de l'app Linux, Rust, cargo-c et meson de `x86-chroot.sh`, FFmpeg et Opus lus dans `kyber-toolchain.sh`. |
| MAC-31 | Une seule liste des correctifs des apps (`kyber_client_patches [correctifs propres à l'app]`, 0025 selon `KYBER_PYROWAVE`, défaut 1) pour les dix scripts : Mac (`build-kyclient.sh`), `apps/apple` (deux), tvOS, cœur (`build-apple.sh`, `ci-essais.sh`), Android, Windows, webOS (avec ses correctifs, sans 0025), Linux (avec ceux de webOS). `apps/macos/scripts/client-patches.sh` retiré ; son essai passe dans `scripts/tests/release-outils.sh`. |
| STCOM-11 | Les deux `build-kyber.sh` posent par `apply.sh` (`kyber_apply_server_patches` puis `kyber_apply_own_patches`) ; leurs copies (remise à l'origine, boucle de correctifs, `kyber_patch_fuzz_warn`, wraps) sont retirées. L'empreinte de Kyber vient de `kyber_station_hash` pour les trois Stations : `stamp.sh` (surcouche Windows), `build-kyber.sh` (Mac), `kyber-stamp.sh` (Linux) et `release.sh` (`mac_kyber_hash`) ; la chaîne d'outils y est. |
| TRANS-03 (reste) | La liste propre de `build-kyber.sh` (Mac) n'existe plus : `apps/macos/PyroWave` (COMMIT et port Metal) est dans l'empreinte de Kyber pour Mac ; le marqueur à part `.kybernow-pyrowave` de `release.sh` est retiré. |
| KYB-48 | Le bloc mort du stylet était déjà retiré (vague 1, STLIN-25) ; le commentaire qui restait est fondu dans celui de la pose. |
| TRANS-51 (scripts) | `scripts/arbres.sh` efface le vrai marqueur, `kyber-desktop/.kybernow-correctifs-apps`. |
| LINAPP-43 | Pose sans git (Flatpak) dans `kyber-client-patches.sh`, stricte, avec un marqueur : une seconde pose ne reprend plus un correctif recouvert par un suivant. `sources.sh` copie les scripts communs et `COMMIT` ; `flatpak.sh` prend le SDK de Kyber à l'origine par défaut (`KYBERNOW_KYBER_HEAD=1`). |
| LINAPP-44 | Commit de PyroWave de l'app Linux lu dans `COMMIT` (voir TRANS-19). |
| TRANS-21 (reste) | Android : dossier temporaire d'Opus retiré même sur une erreur (`trap`). `server/camera/build.sh` : de même pour ses objets Windows. Le reste (pi-gen figé, chaîne webOS figée, téléchargements reprenables, marqueur de `relocate`, `make-ab-image.sh`) était fait en vague 1 : vérifié présent. |
| TRANS-22 (scripts) | Commentaires « PyroWave seulement avec --kyber-pyrowave » alignés sur le code (allumé par défaut) dans Android, `build-apple-mobile.sh`, `build-kyclient.sh`, tvOS, Windows. |
| CI | Ligne de tête de `ci.yml` et IOS-29 (`screen-pad`, `session-rules`) : déjà faits en vague 1, vérifiés (`actionlint`). |
| (trouvé) | `apply.sh` ne se reprenait pas : une seconde construction sur les mêmes sources (Stations Mac et Linux, surcouche relancée sans `arbre_kyber_origine`) trouvait 0001 « à moitié posé », recouvert par les `*-kyclient-*` (vérifié avec la version d'avant). Le retrait et la repose des correctifs des apps sont remplacés par la remise à l'origine des fichiers de tous les correctifs communs et des apps, puis la pose des communs et de ceux de `apps/macos` ; les scripts des apps reposent tous les leurs (depuis le 25/11). Même résultat qu'avant sur des sources d'origine. |
| (trouvé) | `kyber_toolchain_check` arrêtait le script quand un outil manquait (`set -e` et `pipefail`) au lieu de le signaler. |
| (trouvé) | `build-kyclient.sh --stamp` exigeait `third_party/` : l'essai de l'empreinte du Vendor ne passait pas dans un worktree ni en CI. |
| (trouvé) | `kyber_apply_client_patches` lisait un correctif donné en chemin relatif depuis le dépôt visé. |
| (trouvé) | shellcheck : `server/linux/scripts/package.sh` (SC2046) et `release-station.sh` (SC2088, `~` distant voulu). |

## Laissé, et pourquoi

| Quoi | Pourquoi |
|---|---|
| MAC-31, second volet : renommer `apps/macos/kyber-patches` en `apps/kyber-patches` | Le dossier contient des `.patch` que la session des correctifs Kyber modifie en ce moment, et une quarantaine de chemins le citent dans des zones d'autres sessions (docs, `apps/core/README`, CI). À faire juste après la fusion de la phase 2a, en un commit : `git mv`, puis `release-entrees.sh`, `kyber-client-patches.sh`, `apply.sh`, `sources.sh`, `ci.yml`, `coeur.yml` et les docs. |
| `server/macos/pyrowave/build.sh` l. 3 et 43 : « a32b05e » dans un commentaire et la `Description` du `.pc` | Zone de la session des correctifs Kyber. Rien n'y compile un autre commit (le port Metal vendu est la source) ; `kyber_pyrowave_commit` le lirait. |
| `apps/apple/tests/pyrowave/run.sh:29` : `TEAM=P6DB682S4K` | Essais de l'app Apple (phase 2b) : `. scripts/identite.sh` et `$KYBER_TEAM_ID`. |
| `apps/linux/tests/pyrowave/run.sh:34` : SC2046 | Essais de l'app Linux (phase 2b). C'est le seul avertissement de shellcheck du dépôt : le job « Scripts » de la CI reste rouge tant qu'il est là. |
| Docs hors zone qui citent l'ancien état | `apps/macos/README.md` et `.en.md` (l. 390 : `scripts/client-patches.sh`), `docs/pyrowave.md` et `.en.md` (l. 43), `docs/pyrowave-client-android.md` et `.en.md` (l. 9 : commit « épinglé dans `server/kyber-patches/pyrowave.sh` ») : session des docs. |
| TRANS-04 (reste) | `cargo +nightly` des scripts Apple : la session Mac a choisi de ne pas épingler `+nightly-<date>` (autre compilateur, absent de ce Mac) ; `build-apple-mobile.sh` vérifie la date et l'empreinte de `release.sh` suit le nightly lancé. Rien à changer. |
| TRANS-23 | `server/installer/build.sh` : hors de ma zone (installateur). |

## Vérifié, et comment

- `scripts/tests/release-outils.sh` : 105 essais, tous passent. Nouveaux : pose stricte (exact, inapplicable sans rien laisser, décalage refusé par défaut puis posé avec `KYBERNOW_PATCH_STRICT=0`, sous-projet remis dans son dépôt, fichier déjà corrigé gardé), wraps de meson, `kyber_pyrowave_commit`, liste des apps (PyroWave, webOS, correctifs de la Station en tête, `Vendor/kyclient.stamp` sensible à un correctif kynput de la Station), pose sans git qui se reprend, pose des Stations qui se reprend, empreintes des trois Stations (stables, distinctes ; `release.sh`, `stamp.sh`, `kyber-stamp.sh` égales à `kyber_station_hash` ; COMMIT, port Metal, chaîne d'outils et correctif Linux font changer celles qu'il faut, et pas les autres), commit de PyroWave dans l'empreinte de la surcouche. Les entrées des étapes restent complètes (`entrees-couvertes.py`) : trois exceptions motivées pour `station_mac` (la liste de la Station Linux dans `apply.sh`).
- Correctifs des apps, dans mon arbre `apple` (sources d'origine) : `kyber_apply_app_patches` pour chaque variante — Mac, iOS, tvOS, Android et Windows (35 correctifs, 0025 compris), sans PyroWave (34), webOS (35, sans 0025, avec le sien), Linux (36) — enchaînées dans deux ordres différents sans échec ; une seconde pose identique ne réécrit rien (« déjà posés »).
- Flatpak : `KYBERNOW_KYBER_HEAD=1 apps/linux/scripts/sources.sh` depuis l'arbre, puis la pose de `build.sh` sur la copie sans git : 36 correctifs, puis 0 à la seconde pose ; le commit de PyroWave se lit dans la copie.
- Station Linux : `KYBER_PATCH_ONLY=1 server/linux/scripts/build-kyber.sh` sur une copie d'origine des sources (dans mon dossier d'arbres) : 59 communs, 6 des apps, 12 de Linux, sans décalage ; trois passages de suite, sources identiques après le premier et le second. La version d'avant échoue au second passage.
- Surcouche Windows : voir plus bas.
- Station Mac : voir plus bas.
- `bash -n` et `shellcheck -S warning -s bash` sur tous les `*.sh` du dépôt : un seul avertissement, hors zone (`apps/linux/tests/pyrowave/run.sh`) ; `actionlint` sans remarque sur les cinq workflows.
- **Pas vérifié** : `release.sh` de bout en bout (publication 0.11.8 en cours) ; la Station Linux compilée dans la VM et le Flatpak dans flatpak-builder (pas de VM) ; les signatures (pas de trousseau) ; les apps compilées (seule la pose de leurs correctifs l'est).

**À la fusion** : les empreintes de la surcouche Windows, de Kyber pour Mac, de Kyber pour Linux et du Vendor de l'app Mac changent (liste commune, chaîne d'outils, scripts communs) : chacun est recompilé une fois, et FFmpeg est extrait de nouveau une fois (empreinte du wrap triée en `LC_ALL=C`). Les FFmpeg et Opus de webOS et l'Opus d'Android sont refaits une fois (marqueurs nouveaux), comme le FFmpeg et le PyroWave de l'app Linux.

## Notes de version

FR :
- Chaque version est construite de façon plus sûre : chaque modification de Kyber se pose exactement là où elle doit, et chaque composant est reconstruit dès qu'une de ses sources change.

EN:
- Every release is built more safely: each change to Kyber lands exactly where it should, and each component is rebuilt as soon as one of its sources changes.
