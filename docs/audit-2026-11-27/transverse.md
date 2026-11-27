# Audit du 27/11/2026 — transverse (outillage, cohérence entre apps, documentation, code mort)

Audit statique en lecture seule de ce qui relie les apps et les Stations : `scripts/`, `tools/`, `.github/workflows`, les scripts de construction de chaque app et Station, les catalogues de traduction, les règles de `docs/link-ui.md`, les constantes partagées, la documentation et le code mort à l'échelle du dépôt. Rien n'a été compilé ni exécuté. Seuls des fichiers déjà construits ont été lus : `unzip`, `shasum`, en-tête PE, et `gh release view` en lecture seule.

Ce qui était réglé dans `docs/archives/audit-2026-10-20.md` n'est pas re-signalé. Le piège de `Vendor/kyclient.stamp` du Mac (fc2e71e) n'est pas re-signalé non plus. Les constats propres à une seule app et sans lien avec les autres restent aux audits de zone. Ici, ils ne sont cités que lorsqu'ils contredisent une règle commune.

Niveaux de confiance :
- **vérifié** : relu dans le code, et le scénario tient ;
- **plausible** : le mécanisme est relu, mais le déclenchement n'est pas prouvé.

Gravité : **critique** (déjà publié ou casse une publication), **haute**, **moyenne**, **faible**.

## Synthèse

| ID | Gravité | Catégorie | Constat | Effort | Confiance |
|---|---|---|---|---|---|
| TRANS-01 | critique | bug | La 0.11.7 publiée contient encore `kybernow-micro.exe` et `kybernow-camera.exe` en mode console : l'empreinte de la surcouche Windows ignore `server/micro`, `server/camera` et `server/encoder-check` | S | vérifié |
| TRANS-02 | haute | outillage | Station Linux : l'empreinte de l'étape oublie le correctif du stylet, `LICENSE` et les sources de Kyber de la VM | S | vérifié |
| TRANS-03 | haute | outillage | Station Mac : `mac_kyber_hash` ignore `apps/macos/PyroWave/COMMIT` et `upstream` | S | vérifié |
| TRANS-04 | moyenne | outillage | Le nightly réellement utilisé (`+nightly`) n'entre pas dans l'empreinte des outils | S | vérifié |
| TRANS-05 | moyenne | outillage | `kyber-client-patches.sh` ne remet à l'origine que les fichiers des correctifs du moment | S | vérifié |
| TRANS-06 | moyenne | outillage | `apps/apple/scripts/build-apple.sh` compile kyclient sans poser les correctifs client | S | vérifié |
| TRANS-07 | moyenne | bug | Station Mac : un `kybernow-camera` périmé peut partir dans l'app (échec de `clang` masqué) | S | vérifié |
| TRANS-08 | moyenne | outillage | `x86-chroot.sh` fait `sudo rm -rf` sur un chroot qui peut avoir encore des montages bind | S | plausible |
| TRANS-09 | moyenne | outillage | `server/kyber-patches/build.sh` vide la surcouche avant de compiler les outils (pas atomique) | S | vérifié |
| TRANS-10 | moyenne | outillage | `verrou.sh` : deux processus peuvent prendre le même verrou périmé ; dérive de `hostname -s` | S | plausible |
| TRANS-11 | moyenne | outillage | Contrôle du certificat de l'APK qui passe à vide si `release-cert.sha256` manque | S | vérifié |
| TRANS-12 | moyenne | outillage | La CI ne teste ni le relais (145 tests), ni les outils de publication ; l'image du relais part sans tests | S | vérifié |
| TRANS-13 | faible | outillage | `relay-image.yml` : `${{ inputs.version }}` interpolé dans `run:` | S | vérifié |
| TRANS-14 | faible | outillage | `release.sh` : pas de `set -E` ni de piège `EXIT` ; attente des dépendances sans délai | S | vérifié / plausible |
| TRANS-15 | faible | outillage | Voies Linux parallèles : cache Flatpak amorcé deux fois, deux constructions Rosetta, Ctrl-C pendant que la VM écrit | S | plausible |
| TRANS-16 | faible | outillage | Empreintes incomplètes : recettes `construire_*`, `apps/tvos` pour iOS, chemins du job tvOS de la CI | S | vérifié |
| TRANS-17 | faible | outillage | Arbres de construction : les sources de Kyber copiées une fois ne sont jamais comparées au dépôt principal | M | vérifié |
| TRANS-18 | faible | outillage | `build-kyber-tvos.sh` efface `KyberNow.xcframework` ; `vendor-kyber.sh` réécrit `Vendor` sous l'app ouverte | S | vérifié |
| TRANS-19 | moyenne | factorisation | Commit PyroWave épinglé à 4 endroits ; marqueurs FFmpeg et PyroWave sans version | S | vérifié |
| TRANS-20 | faible | factorisation | Pose des correctifs écrite 5 fois, `cargo_quiet` 8 fois, identité et Team ID dans 8 scripts, versions d'outils recopiées | M | vérifié |
| TRANS-21 | faible | outillage | Constructions non reproductibles (pi-gen sur une branche, chaîne webOS `latest`, `curl \| tar` non reprenable) | S | vérifié / plausible |
| TRANS-22 | faible | doc | Commentaires et doc : « PyroWave éteint par défaut », alors que le code l'allume | S | vérifié |
| TRANS-23 | faible | outillage | `server/installer/build.sh` choisit WebView2Loader par tri alphabétique ; dit « macOS ou Linux » mais ne marche que sur macOS | S | vérifié |
| TRANS-24 | faible | outillage | Listes des fichiers publiés recopiées dans 4 scripts ; boucle morte dans `release-verifie.py` | S | vérifié |
| TRANS-25 | haute | i18n | App Windows : `t('bar.mouse_shortcut')` appelée, mais la clé a été retirée des catalogues | S | vérifié |
| TRANS-26 | haute | i18n | Android : 11 textes de la caméra présents dans les XML générés, absents de `strings.py` | S | vérifié |
| TRANS-27 | moyenne | outillage | Les vérificateurs i18n de l'app Windows et des installateurs ne contrôlent pas les clés employées | S | vérifié |
| TRANS-28 | faible | i18n | Admin : erreur VB-CABLE écrite en dur en français | S | vérifié |
| TRANS-29 | faible | code mort | Environ 70 clés de traduction mortes (web, cœur, Windows, Android, KyberKit, Station Mac) | S | vérifié |
| TRANS-30 | moyenne | cohérence | Client web : « Souris jeu », « Souris bureau », « Remettre le son » au lieu des textes de `link-ui.md` | S | vérifié |
| TRANS-31 | faible | cohérence | Libellés qui divergent entre apps (accueil, code, « refused/rejected », exemple de relais, glossaire) | S | vérifié |
| TRANS-32 | faible | i18n | Espace fine avant `: ; ! ?` appliquée dans Android seulement | M | vérifié |
| TRANS-33 | faible | doc | `docs/i18n.md` oublie les catalogues du relais et de la Station Mac, et deux fichiers sans paire | S | vérifié |
| TRANS-34 | haute | cohérence | Empreinte du certificat affichée de 4 façons ; Android et LG n'en montrent que 8 caractères | S | vérifié |
| TRANS-35 | moyenne | bug | `parse_address` recopié à l'identique entre Linux et LG, faux en IPv6 (`fe80::1` devient l'hôte `fe80:` sur le port 1) | S | vérifié |
| TRANS-36 | moyenne | bug | Adresse d'Admin : Android oublie les crochets IPv6, Mac et Apple TV peuvent les doubler ; `console::admin_url` n'est pas exposé | S | vérifié / plausible |
| TRANS-37 | moyenne | cohérence | Apple TV : la raison du HDR remplace le bandeau de mise à jour de la Station | S | vérifié |
| TRANS-38 | moyenne | cohérence | Statistiques et « Trajet d'une image » incomplets sur Linux, Android et le web | M | vérifié (agent) |
| TRANS-39 | faible | cohérence | Autres écarts à `link-ui.md` (erreur sous le nom, caméra refusée, menu Touches, caméra de l'Apple TV…) | S/M | vérifié / plausible |
| TRANS-40 | moyenne | factorisation | Débit plafonné quand la fenêtre est cachée : 20, 8 ou 2 Mbit/s selon l'app, aucun plafond sur Windows et Linux | S | vérifié |
| TRANS-41 | faible | factorisation | Bornes du débit réglé à la main différentes dans 6 endroits | S | vérifié |
| TRANS-42 | faible | factorisation | Constantes et règles du cœur recopiées dans les apps (profils, PyroWave, relais, invités, accueil, ports) | S | vérifié |
| TRANS-43 | moyenne | doc | `ROADMAP` : HDR, Station Mac, PlayStation et Apple Pencil encore « prévus » alors qu'ils sont livrés | S | vérifié |
| TRANS-44 | moyenne | doc | Le site dit encore « une Station pour Linux arrive bientôt » | M | vérifié |
| TRANS-45 | faible | doc | `apps/core/README.en.md` n'a pas la section « Accueil et compte » | S | vérifié |
| TRANS-46 | faible | doc | `docs/reprise-apres-perte.md` cite des messages de journal qui n'existent pas | S | vérifié |
| TRANS-47 | faible | doc | `link-ui.md` en retard sur les apps ; chemins de scripts ambigus dans `PROJECT.md` et d'autres docs | S | vérifié |
| TRANS-48 | moyenne | code mort | 179 Mo de vidéo du site suivis par git (environ 350 Mo dans l'historique) | M | vérifié |
| TRANS-49 | faible | code mort | Logo v1 (`brand/`) et captures de tests suivis que rien n'utilise | S | vérifié |
| TRANS-50 | faible | code mort | Dépendances Cargo déclarées et non utilisées (`ndk-sys`, `kynput-sys`) | S | vérifié / plausible |
| TRANS-51 | faible | code mort | Constantes et nettoyage morts (`regist::PORT`, `takion::PORT`, marqueur effacé par `arbres.sh`) | S | vérifié |
| TRANS-52 | faible | outillage | `tools/latency/run_client.sh` passe le mot de passe en argument (visible par `ps`) | S | vérifié |

Comptes par catégorie : bug 4, outillage 21, cohérence 6, i18n 4, factorisation 5, code mort 5, doc 7, soit 52 en tout. TRANS-01 et TRANS-07 relèvent aussi de l'outillage.

---

## A. Outillage

### TRANS-01 — La 0.11.7 publiée contient encore le micro et la caméra en mode console
- **Catégorie** : bug (outillage de publication). **Gravité** : critique. **Effort** : S. **Confiance** : vérifié sur les fichiers publiés.
- **Où** :
  - `scripts/release.sh:333` (`OVERLAY_WANT`) ;
  - `scripts/release.sh:681-683` (`construire_surcouche` ne relance `build.sh` que si l'empreinte diffère) ;
  - `server/kyber-patches/build.sh:382` (même empreinte) ;
  - `scripts/release.sh:574` (`entrees_surcouche` compte bien `server/micro`, `server/camera` et `server/encoder-check`).
- **Description** : l'empreinte de la surcouche ne hache que `server/kyber-patches/*.patch`, `build.sh`, `pyrowave.sh` et les correctifs client. Or `build.sh:362-369` compile aussi `kybernow-encoder-check.exe`, `kybernow-micro.exe` et `kybernow-camera.exe` dans la surcouche, et `apply.sh` et `scripts/kyber-toolchain.sh` influencent la construction. Quand seuls ces fichiers changent, la clé de l'étape change et l'étape est relancée. Mais l'empreinte de l'arbre est jugée à jour : `build.sh` est sauté et l'ancienne surcouche est réemballée.
- **Ce qui s'est passé** :
  - le commit e5c6a3c1 (27/11 à 00:34) passe `kybernow-micro` et `kybernow-camera` sans fenêtre (`server/micro/src/main.rs:20`, `server/camera/build.sh`) ;
  - dans `third_party/arbres/station-windows/third_party/payload/kyber-overlay/`, les deux exécutables datent du 26/11 à 20:08 et sont en sous-système 3 (console) ;
  - le `kybernow-update.zip` en cache (`.etapes/station`, 27/11 à 00:36) a le même SHA-256 que celui de la release v0.11.7 sur GitHub (`5e7aa473…`), et ses `kyber/kybernow-micro.exe` et `kyber/kybernow-camera.exe` sont en sous-système 3 ;
  - le CHANGELOG de la 0.11.7 annonce pourtant la correction.
- **Scénario** : un utilisateur de la 0.11.7 active « Micro » ou « Caméra » dans une app : un terminal noir s'ouvre toujours sur le bureau de la Station. Même chose pour `KyberNow-Station-Setup.exe`, construit dans le même arbre à la même heure.
- **Correctif** :
  1. Une seule fonction d'empreinte, partagée par `release.sh` et `build.sh`, qui ajoute `apply.sh`, `server/{micro,camera,encoder-check}` et `scripts/kyber-toolchain.sh`. Plus simple encore : toujours recompiler ces trois petits outils.
  2. Supprimer `kyber-overlay.stamp` dans l'arbre `station-windows`.
  3. Publier une 0.11.8. Prévoir aussi une vérification dans `release-verifie.py pe` : sous-système 2 attendu pour `kybernow-micro.exe` et `kybernow-camera.exe`.

### TRANS-02 — Station Linux : l'empreinte de l'étape oublie des entrées
- **Catégorie** : outillage. **Gravité** : haute. **Effort** : S. **Confiance** : vérifié.
- **Où** : `scripts/release.sh:589`, `entrees_station_linux() { chemins $(server_sauf macos) scripts/kyber-toolchain.sh; echo "$VM_TOOLS"; }`.
- **Description** : il manque :
  - `apps/macos/kyber-patches/*-kynput-stylet.patch`, posé par `server/linux/scripts/build-kyber.sh` ;
  - `LICENSE`, lu par `server/linux/scripts/package.sh` ;
  - l'état des sources de Kyber dans la VM et dans le chroot x86 (`release-station.sh`, `~/kn-station/kyber-desktop`).
- **Scénario** : on modifie le correctif du stylet, ou Kyber avance dans la VM. L'étape `station_linux` est « reprise » et publie les .deb et .rpm de la version précédente.
- **Correctif** : ajouter `apps/macos/kyber-patches` et `LICENSE`, plus `git rev-parse HEAD` des dépôts kysdk de la VM et du chroot, lus par ssh.

### TRANS-03 — Station Mac : PyroWave hors de l'empreinte de Kyber pour Mac
- **Catégorie** : outillage. **Gravité** : haute. **Effort** : S. **Confiance** : vérifié.
- **Où** : `scripts/release.sh:336` (`mac_kyber_hash`) et `server/macos/scripts/build-kyber.sh:170` (même liste). `server/macos/pyrowave/build.sh:17` compile `apps/macos/PyroWave/upstream`.
- **Scénario** : `vendor-pyrowave.sh <nouveau commit>` met à jour le décodeur des apps, mais Kyber pour Mac n'est pas recompilé. L'encodeur PyroWave de la Station Mac et le décodeur des apps ne viennent plus du même commit.
- **Correctif** : ajouter `apps/macos/PyroWave/COMMIT` aux deux listes (et les tirer d'une seule fonction).

### TRANS-04 — Le nightly réellement utilisé n'est pas haché
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `scripts/release.sh:512` hache `rustup run "nightly-$KYBER_NIGHTLY" rustc -V … || true`. Mais `apps/tvos/scripts/build-kyber-tvos.sh:101,104` et `apps/apple/scripts/build-apple-mobile.sh:123` appellent `cargo +nightly`, pas `+nightly-2026-10-19`.
- **Scénario** : un `rustup update` change le nightly. L'empreinte ne bouge pas (la toolchain datée n'est pas installée, donc la sortie est vide), et les étapes tvOS et iOS sont reprises d'un cache compilé avec un autre compilateur. À l'inverse, une construction peut casser sans qu'aucune empreinte ne l'explique.
- **Correctif** : épingler `+nightly-$KYBER_NIGHTLY` dans les deux scripts, et retirer le `|| true`.

### TRANS-05 — Remise à l'origine partielle des sources client
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `scripts/kyber-client-patches.sh:87-98`.
- **Description** : seuls les fichiers que touchent les correctifs passés en argument sont remis à l'origine. `0025-kyclient-pyrowave.patch` touche 4 fichiers qu'aucun autre correctif client ne touche : `kyclient-rs/src/lib.rs`, `kymux_backend/host_service.rs`, `kyctl-common/src/enums.rs` et `kyctl-common/src/kymux.rs`.
- **Scénario** : hors `release.sh` (qui a un arbre par variante), un build webOS (qui écarte 0025, `apps/webos/scripts/build.sh:147`) ou `--sans-pyrowave` lancé après un build avec PyroWave garde ces modifications. Le résultat dépend encore du build précédent, ce que l'en-tête du script dit justement éviter.
- **Correctif** : remettre à l'origine l'union des fichiers de tous les correctifs connus, ou faire `git checkout -- . && git clean -fd` sur kyctl, kymux et kynput, comme `server/linux/scripts/release-station.sh:94`.

### TRANS-06 — `build-apple.sh` ne pose pas les correctifs client
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié (mécanisme) ; la casse est plausible.
- **Où** : `apps/apple/scripts/build-apple.sh:68-74` (`cargo cinstall -p kybernow-apple`). C'est le seul script qui compile le cœur sans lire `kyber-client-patches.sh`, alors que `build-apple-mobile.sh:89-97` le fait.
- **Scénario** : lancé seul, après un build webOS ou un build de Station sur les mêmes sources, il compile kyclient sans 0025. La fonctionnalité `kyber-pyrowave`, active par défaut, ne compile plus. Dans `release.sh`, c'est `build-kyclient.sh`, lancé juste avant, qui masque le problème.
- **Correctif** : poser les correctifs comme `build-apple-mobile.sh`.

### TRANS-07 — Station Mac : une caméra périmée peut être embarquée
- **Catégorie** : bug (construction). **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** :
  - `server/camera/build.sh:53-57` : `clang … 2>&1 | grep -Ev … || true`, puis seulement `[ -x "$OUTPUT" ]` ;
  - `server/macos/scripts/build-station-app.sh:39,46,111` : `WORK="$OUT/work"`, jamais vidé ;
  - même motif dans `server/encoder-check/build.sh:32-34`.
- **Scénario** : `clang` échoue (en-tête manquant, FFmpeg changé). Le binaire de la construction précédente reste dans `work/`, passe le test `-x` et part dans l'app signée et notarisée.
- **Correctif** : `rm -f "$OUTPUT"` avant de compiler, ou tester `${PIPESTATUS[0]}`.

### TRANS-08 — `x86-chroot.sh` : `rm -rf` sur des montages
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S. **Confiance** : plausible.
- **Où** : `server/linux/scripts/x86-chroot.sh:47` (`unmount … || true`) et `:58` (`sudo rm -rf "$R"`). `/dev` (rbind), `/home/$U` et `/media/psf/kyber-apps` sont montés dessous, ce dernier en écriture (l.39).
- **Scénario** : un processus reste dans le chroot (gpg-agent lancé par apt). Le démontage échoue sans bruit, et le `rm -rf` de la relance efface `/dev` de la VM et le dépôt partagé du Mac.
- **Correctif** : `rm -rf --one-file-system`, et abandonner si `findmnt -R "$R"` liste encore un montage.

### TRANS-09 — Surcouche Windows vidée avant la fin de la construction
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/kyber-patches/build.sh:349` vide `$OVERLAY`, puis compile encoder-check, micro et caméra (l.362-369). Cela contredit l'en-tête (l.173-175, « sans toucher à la surcouche précédente »).
- **Scénario** : la compilation de la caméra échoue, et la surcouche reste à moitié remplie. Un `server/installer/build.sh` lancé à la main l'embarque (l.34).
- **Correctif** : construire dans `$OVERLAY.new`, puis `mv` à la fin.

### TRANS-10 — Course sur un verrou périmé
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S. **Confiance** : plausible.
- **Où** : `scripts/verrou.sh:63-68` (`kyber_verrou_perime` → `rm -rf "$d"` → `continue`), `:36-50`.
- **Scénario** :
  - A et B trouvent le même verrou périmé. A le supprime, le recrée et écrit `qui`. B, qui l'a déjà jugé périmé, fait son `rm -rf` et efface le verrou tout neuf de A : les deux constructions tournent sur les mêmes sources. Même chose pendant la minute où un verrou neuf n'a pas encore son `qui` (l.38-39) : il est jugé périmé seulement s'il a plus d'une minute, donc la fenêtre est étroite.
  - `hostname -s` change avec le réseau sur macOS : un verrou laissé sous l'ancien nom n'est jamais repris (« Tenu depuis une autre machine »), contrairement à ce que dit le message.
- **Correctif** : `mv "$d" "$d.perime.$$"` (atomique), puis `rm`. Utiliser `scutil --get LocalHostName` sur macOS.

### TRANS-11 — Contrôle du certificat de l'APK qui peut passer à vide
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `scripts/release.sh:984` : `grep -q "$(cat apps/android/scripts/release-cert.sha256 2>/dev/null || echo .)"`.
- **Scénario** : `release-cert.sha256` est supprimé ou renommé. Le motif devient `.`, qui accepte n'importe quel certificat, et un APK signé avec une autre clé part sans alerte. Les mises à jour Android sont alors refusées chez tout le monde.
- **Correctif** : vérification préalable de la présence du fichier ; `grep -qF` sans repli.

### TRANS-12 — Ce que la CI ne teste pas
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S/M. **Confiance** : vérifié.
- **Où** : `.github/workflows/ci.yml:54-70` (seulement Admin et l'installateur) ; `.github/workflows/relay-image.yml` (pas de `needs:` ni de tests) ; `scripts/tests/release-outils.sh` (cité nulle part).
- **Description** :
  - `server/relay` et son `tunnel` ne dépendent pas de `third_party`, ont 145 `#[test]` plus `tests/`, et sont exposés sur Internet. Ils ne sont jamais testés, ni en CI, ni par `release.sh`.
  - L'image Docker du relais est publiée à chaque release sans test préalable.
  - `scripts/tests/release-outils.sh` (verrous, arbres, correctifs, vérification des fichiers) n'est lancé par rien.
  - `tools/banc`, qui recompile `apps/core/src/adaptive.rs`, n'est jamais construit.
- **Scénario** : une régression du relais (comptes, invitations) passe en production par `relay-image.yml` et les relais se mettent à jour d'eux-mêmes la nuit (service « updater »).
- **Correctif** : un job `cargo test --locked --workspace` dans `server/relay`, avec `needs:` dans `relay-image.yml` ; lancer `scripts/tests/release-outils.sh` dans le job `scripts` ou dans `release.sh --check` ; le citer dans `docs/releases.md`.

### TRANS-13 — Injection possible dans `relay-image.yml`
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** : `.github/workflows/relay-image.yml:42-43` : `tag="v${{ inputs.version }}"` est interpolé dans `run:` avant la validation de la l.45.
- **Scénario** : quelqu'un qui a le droit de lancer le workflow à la main peut faire exécuter du shell avec `packages: write`. Le risque est faible tant que l'auteur est seul à pouvoir le lancer.
- **Correctif** : passer les entrées par `env:`.

### TRANS-14 — `release.sh` : piège d'erreur et attente sans délai
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié (piège) / plausible (blocage).
- **Où** : `scripts/release.sh:156` (piège ERR sans `set -E`) ; aucun piège `EXIT` pour les dossiers temporaires (`:363`, `:966`, `:1005`) ; boucle d'attente des dépendances sans délai (`:646-651`).
- **Scénario** :
  - Un échec dans une fonction (`arbre_preparer`) sort sans « Échec pendant… » ni indication de relance.
  - Disque plein (déjà vu sur ce Mac) : `etat` échoue, `set -e` tue la voie, et l'étape qui dépend d'elle (`notarisation`, qui attend `app_mac`) attend pour toujours.
- **Correctif** : `trap 'etat "$id" echec' ERR` dans `etape`, ou détecter que la voie porteuse est morte ; un piège `EXIT` pour les dossiers temporaires. Avec `set -E`, ajouter `trap - ERR` au début de chaque voie : `kyber_verrou_rendre` compare `$$`, qui vaut le pid du parent dans un sous-shell.

### TRANS-15 — Voies Linux parallèles dans la même VM
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : plausible.
- **Où** : `scripts/release.sh:819-820`, `:889` ; `apps/linux/scripts/release-flatpaks.sh:47`.
- **Description** :
  - `flatpak_aarch64` et `deb_pi` démarrent ensemble et amorcent tous deux `KYBERNOW_FLATPAK_WORK` par `cp -a`, même pour `--seulement deb`.
  - `station_linux` (chroot x86_64 par Rosetta) et `flatpak_x86_64` tournent en même temps sous Rosetta. C'est la famille de problèmes déjà rencontrée avec pi-gen (rustc qui rend une sortie vide).
  - Ctrl-C rend les verrous (`:889`) pendant que les commandes lancées par ssh écrivent encore dans l'arbre `linux`.
- **Correctif** : n'amorcer le cache que si une architecture Flatpak est demandée (dossier temporaire puis renommage) ; faire dépendre `flatpak_x86_64` de `station_linux` ; `vm pkill -f` avant de rendre les verrous.

### TRANS-16 — Autres trous dans les empreintes
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où et quoi** :
  - Les recettes elles-mêmes (`construire_<id>`, `fichiers_de`) ne sont pas hachées : modifier `construire_windows` n'invalide aucun cache. Correctif : `declare -f "construire_$id"` dans `empreinte_de`.
  - `entrees_ios` (`release.sh:582`) oublie `apps/tvos`, alors que `apps/ios/project.yml:52-56` compile `apps/tvos/KyberKitTV/*.swift`.
  - Le job tvOS de la CI (`ci.yml:131`) compare les chemins depuis la dernière release sans `scripts/kyber-client-patches.sh` ni les correctifs de la Station posés aussi côté client (`*-kynput-*`, `*-kynet-*`, `*-kycontroller-*`).

### TRANS-17 — Arbres de construction : sources de Kyber jamais rafraîchies
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : M. **Confiance** : vérifié.
- **Où** : `scripts/arbres.sh:136` copie une seule fois `kyber-desktop`, `linux-aarch64-rpi`, `sdl-webos` et `pyrowave` ; `:272` fait de même pour `local.properties`. `kyber_tetes` hache le commit de l'arbre, pas celui du dépôt principal.
- **Scénario** : on monte Kyber dans `third_party/` du dépôt principal. Les publications continuent avec l'ancienne version, sans alerte.
- **Correctif** : vérification préalable qui compare `HEAD` des kysdk du dépôt principal et de chaque arbre, et le signale.

### TRANS-18 — Effacements trop larges dans deux scripts Apple
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** :
  - `apps/tvos/scripts/build-kyber-tvos.sh:142` : `rm -rf "$OUT"` efface aussi `Vendor/KyberNow.xcframework` (produit par `build-apple-mobile.sh tvos`). Relancer le SDK seul casse le projet Xcode.
  - `apps/macos/scripts/vendor-kyber.sh:18` : `rm -rf Vendor` sans vérifier si l'app tourne et sans verrou. Réécrire les bibliothèques sous l'app ouverte fait planter VLC, piège documenté dans `build-notes.md`.
- **Correctif** : ne retirer que `include/` et `KyberSDK.xcframework` ; reprendre `app_closed` et `kyber_verrou_sortie` de `build-kyclient.sh`.

### TRANS-19 — PyroWave et FFmpeg : versions épinglées à plusieurs endroits, marqueurs sans version
- **Catégorie** : factorisation. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** :
  - Commit PyroWave dans `apps/macos/PyroWave/COMMIT` (lu par `apps/windows/scripts/build-pyrowave.sh:32`), et en dur dans `server/kyber-patches/pyrowave.sh:15`, `apps/linux/scripts/build.sh:215` et `server/macos/pyrowave/build.sh:3,43`.
  - `apps/linux/scripts/build.sh:217` : la bibliothèque n'est refaite que si le marqueur `PYROWAVE_PATCHES` manque.
  - `apps/linux/scripts/build.sh:150-152` : le marqueur FFmpeg ne contient que `FFMPEG_FEATURES`. `apps/webos/scripts/build.sh:185-197` : libavcodec et libopus ne sont refaites que si elles manquent.
  - `apps/webos/scripts/package.sh:45` a `ffmpeg-7.1.1` en dur : si la version change, la licence LGPL n'est plus copiée, sans bruit.
- **Scénario** : on change de commit PyroWave ou de version de FFmpeg. Mac et Windows suivent ; Linux et webOS gardent l'ancienne bibliothèque, et le paquet webOS perd la licence de FFmpeg.
- **Correctif** : lire `COMMIT` et `FFMPEG_VERSION` partout, et les mettre dans les marqueurs.

### TRANS-20 — Logique de construction recopiée
- **Catégorie** : factorisation. **Gravité** : faible. **Effort** : M. **Confiance** : vérifié.
- **Description** :
  - La pose des correctifs Kyber est écrite cinq fois avec des variantes : `kyber-client-patches.sh` (`git apply`), `server/linux/scripts/build-kyber.sh:66-102` et `server/macos/scripts/build-kyber.sh:67-104` (presque identiques, `patch --forward`), le repli de `apps/linux/scripts/build.sh:87-112` (`patch -R --dry-run`), et le moteur web dans `server/kyber-patches/build.sh:322`.
  - L'empreinte du wrap FFmpeg est copiée trois fois (Windows l.259, Mac l.152, Linux l.120).
  - `cargo_quiet` est copié dans huit scripts, avec des filtres différents.
  - Identité de signature et Team ID `P6DB682S4K` en dur : `apps/macos/scripts/sign-release.sh:16` (seul sans surcharge par `KYBER_SIGN_IDENTITY`, et son `mktemp -d` de la l.18 n'est jamais supprimé), `server/macos/camera-extension/build.sh:14`, `server/macos/gamepads/build.sh:25`, `server/macos/scripts/build-station-app.sh:89,104`.
  - Versions d'outils recopiées au lieu de lire `scripts/kyber-toolchain.sh` : cargo-c 0.10.15, rustc 1.89.0 et meson 1.11.2 dans `apps/linux/scripts/build.sh:138` et `server/linux/scripts/x86-chroot.sh:78-81`.
  - `server/linux/scripts/build-kyber.sh` ne prend pas de verrou, contrairement à ses équivalents Mac et Windows.
- **Correctif** : `scripts/kyber-patches-lib.sh` (pose et empreinte), `scripts/identite.sh`, `cargo_quiet` dans `kyber-toolchain.sh`.

### TRANS-21 — Constructions non reproductibles ou non reprenables
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié (pi-gen, `latest`) / plausible (reprise).
- **Où** :
  - `apps/linux/os/build-image.sh:57` clone pi-gen sur la branche `arm64` sans figer de commit, et ne le met jamais à jour ensuite.
  - `apps/webos/scripts/build.sh:33` télécharge la chaîne par `releases/latest`.
  - `:190` : `curl | tar` dans `third_party` ; une coupure laisse un `configure` présent et des sources incomplètes, que la reprise ne retélécharge pas.
  - `:65-66` : si `relocate-sdk.sh` échoue, il n'est plus jamais refait.
  - Dossiers temporaires sans `trap` dans `apps/android/scripts/build-native.sh:182`, `apps/webos/scripts/build.sh:171,194` et `server/camera/build.sh`.
  - `apps/linux/os/make-ab-image.sh:31-36` : le point de montage `SRC` (l.46, 93-101) manque dans `cleanup`.
- **Correctif** : figer les commits et les versions ; télécharger dans un fichier, extraire dans un dossier temporaire, puis `mv` ; poser un marqueur après `relocate` ; compléter les `trap`.

### TRANS-22 — « PyroWave éteint par défaut » : faux
- **Catégorie** : doc. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** : commentaires de `apps/tvos/scripts/build-kyber-tvos.sh:17-21` (code à la l.46 : `KYBER_PYROWAVE=1`), `apps/core/scripts/build-apple.sh:30-31` (code à la l.51), `apps/windows/scripts/build.sh:96-99` (code à la l.39), et `docs/pyrowave-client-apple.md:26`.
- **Correctif** : aligner les textes, et documenter `--sans-pyrowave`.

### TRANS-23 — Installateur de la Station : choix de WebView2Loader fragile
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/installer/build.sh:43,80` : `ls … webview2-com-sys-* | tail -1`.
- **Description** : c'est un tri alphabétique : avec une 0.9.x à côté d'une 0.39.x dans le registre de cargo, c'est la 0.9 qui est prise. `apps/windows/scripts/build-installer.sh:47-48` lit correctement le `Cargo.lock`. Par ailleurs, l'en-tête dit « depuis macOS ou Linux », mais `stat -f %z` (l.62, 68) et `tar --no-mac-metadata` (l.67) ne marchent que sur macOS.
- **Correctif** : lire la version dans `Cargo.lock` comme l'app Windows ; corriger l'en-tête.

### TRANS-24 — Listes des fichiers publiés recopiées
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** :
  - `scripts/release.sh:110` (`BUILT`/`ASSETS`) et `:1054` (`SHA256SUMS`) ;
  - `scripts/sign-manifest.sh:56` (`FILES`) ;
  - `scripts/release-suivi.py:23` (`CONSTRUCTIONS`, qui double `libelle()`) ;
  - `scripts/release-verifie.py:184-186` : une boucle qui ne contient que `pass`, et le manifeste n'est pas vérifié pour ses fichiers obligatoires ;
  - la docstring de `release-suivi.py` dit le journal « écrit par release.sh », alors qu'il faut un `tee` manuel.
- **Correctif** : une liste unique (fichier texte lu par les trois scripts) ; vérifier que les fichiers obligatoires figurent dans le manifeste.

### TRANS-52 — Mot de passe en argument dans `tools/latency`
- **Catégorie** : outillage. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** : `tools/latency/run_client.sh:42,53` (`--auth-password`, `curl -u`, `curl -k`).
- **Scénario** : le mot de passe est visible par `ps` pendant une mesure.
- **Correctif** : le lire dans une variable d'environnement, ou passer `curl --netrc-file`.

---

## B. Cohérence entre apps

### B.1 Catalogues de traduction

Les clés FR et EN sont **identiques dans tous les catalogues JSON**, avec les mêmes `{paramètres}`. Les catalogues Apple sont entièrement traduits, et Linux, webOS, le relais et la Station Mac sont bilingues de bout en bout. Restent les constats suivants.

### TRANS-25 — App Windows : clé appelée mais absente
- **Catégorie** : i18n. **Gravité** : haute. **Effort** : S. **Confiance** : vérifié.
- **Où** : `apps/windows/ui/src/Bar.svelte:1584` : `t('bar.mouse_shortcut')`, absente de `fr.json` et de `en.json` (retirée par cce02f8f).
- **Scénario** : barre d'une partie PlayStation. L'infobulle et l'`aria-label` du bouton souris affichent « Prendre la sourisbar.mouse_shortcut ».
- **Correctif** : remettre la clé (« (Ctrl+Alt+Maj+M) » / "(Ctrl+Alt+Shift+M)") ou la retirer de la ligne. Voir TRANS-27 pour que ça ne se reproduise pas.

### TRANS-26 — Android : textes de la caméra hors du générateur
- **Catégorie** : i18n. **Gravité** : haute. **Effort** : S. **Confiance** : vérifié.
- **Où** : `apps/android/app/src/main/res/values/strings.xml:799-811` (et `values-fr`) contient 11 clés `camera_*` : `camera`, `camera_starting`, `camera_turn_on`, `camera_turn_off`, `camera_indicator`, `camera_front`, `camera_back`, `camera_external`, `camera_resolution`, `camera_resolution_help` et `camera_stats`. Aucune ne figure dans `apps/android/scripts/strings.py`, la source des deux XML. Le commit 4ee3e688 a modifié les XML à la main.
- **Scénario** : le prochain `strings.py` efface ces lignes, et la compilation Kotlin échoue sur `R.string.camera_*` (`CameraSender.kt`, `Settings.kt:635`).
- **Correctif** : ajouter les 11 triplets à `strings.py`, et que la construction vérifie que les XML correspondent au script (mode `--check`).

### TRANS-27 — Les vérificateurs i18n ne contrôlent pas les clés employées
- **Catégorie** : outillage. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `apps/windows/ui/scripts/check-i18n.mjs`, `server/installer/ui/scripts/check-i18n.mjs` et `apps/windows/setup/ui/scripts/check-i18n.mjs` ne comparent que les clés FR et EN entre elles. Seul `server/web/scripts/i18n-check.mjs` vérifie que chaque `t('…')` littéral existe.
- **Correctif** : reprendre ce bloc dans les trois scripts. Ce contrôle aurait arrêté TRANS-25.

### TRANS-28 — Admin : erreur écrite en dur en français
- **Catégorie** : i18n. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/admin/src/micro.rs:152` : `ApiError(StatusCode::NOT_FOUND, "VB-CABLE n'est pas livré avec cette Station")`. C'est le seul `ApiError` littéral en français ; un Admin en anglais l'affiche tel quel.
- **Correctif** : passer par le catalogue d'Admin, comme les autres erreurs.

### TRANS-29 — Clés mortes
- **Catégorie** : code mort. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié. Les préfixes construits dynamiquement ont été recherchés ; pour Apple, c'est plausible au-delà des clés marquées `stale`.
- **Client web et Admin** (`server/web/src/lib/i18n/fr.json`, 22 clés) :
  - `common.list` et `common.langAutoHint` ;
  - `lend.page.intro`, `switch`, `switchHelp` et `current`, avec leurs variantes `macos.` et `linux.` ;
  - `lend.apps.steamHelp` et `lend.apps.none` ;
  - `lend.friends.empty` et `lend.friends.lastUsed` ;
  - `recording.on_computer`, `record_menu`, `stop_menu`, `codec`, `disk_full`, `folder`, `lent` et `failed`. Admin envoie déjà ces erreurs traduites (`server/admin/src/recording.rs:279-307`).
- **Cœur** (`apps/core/src/i18n/fr.json`, 6 clés) : `autotest.phrase`, `micro.permission_mac`, `micro.permission_web`, `micro.tile_on`, `micro.tile_off` et `micro.tile_waiting`.
- **App Windows** : `home.welcome_title`, `home.welcome_lead` et `camera.indicator` ; côté Rust, `errors.video_thread`.
- **Ailleurs** :
  - Admin Rust : `api.launchInvalid` ;
  - installateur de l'app Windows : `app.title` ;
  - Android : `ps_hdr_help`, `ps_step_wake` et `ps_touchpad` ;
  - Station Mac : `session.lent` et `error.title`.
- **Apple** :
  - « Faire jouer », « Faire regarder » et « Vous pouvez jouer. » sont `stale` dans Mac, iOS et tvOS ;
  - 25 erreurs de session de `KyberKit/Localizable.xcstrings` ne sont plus appelées depuis le passage au cœur (b412cb86).

### TRANS-30 — Client web : libellés de session hors du contrat
- **Catégorie** : cohérence. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `server/web/src/lib/i18n/fr.json:83-84` (« Souris jeu », « Souris bureau ») et `:69` (« Remettre le son »). `docs/link-ui.md` fige « Souris des jeux », « Souris du bureau » et « Rétablir le son », que les 7 autres apps reprennent.
- **Scénario** : le même bouton porte deux noms selon que l'invité est dans le navigateur ou dans une app.
- **Correctif** : reprendre les textes de `link-ui.md`. Le client web a aussi « Le code a six chiffres, et le prénom ne peut pas être vide. » (`fr.json:1147`), qui diffère de la doc, et des apostrophes typographiques ’ (`fr.json:228`, `:248`), contraires au reste.

### TRANS-31 — Libellés qui divergent pour la même action
- **Catégorie** : cohérence. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Retour à l'accueil** : « Revenir à l'accueil » (Mac, web, forme de `link-ui.md:72`) contre « Retour à l'accueil » (Windows, tvOS, Android, Linux `ui/session.rs:376`). En anglais, "Back home" sur Linux (`ui/device.rs:80`) et LG (`ui/session.rs:257`) contre "Back to home" ailleurs.
- **Code** : « Avec un code » (Windows, Linux), « Par un code » (Android) et « Par code » (LG `ui/pair.rs:45`).
- **Manette** : « Vibration d'essai » (Android, Linux, LG) contre « Tester la vibration » (tvOS) ; "Controller as a mouse" (tvOS) contre "Controller as mouse" ailleurs.
- **Consoles sur LG** : « Valider » donne "Accept" (`ui/consoles.rs:694`) ; « {n} compte(s) » (`ps.rs:426`) sans vrais pluriels, que Linux a.
- **Anglais** :
  - « Identifiant ou mot de passe refusé. » donne "…rejected." dans les apps Apple, "…refused." dans web, Windows, le cœur et `link-ui.md:357` ;
  - « Ce n'est pas un lien KyberNow. » a deux traductions (Mac et iOS contre tvOS) ;
  - "recognizes" en américain dans 6 apps, alors que le britannique domine.
- **Glossaire** (`docs/i18n.md`) : « décodage par la carte graphique » contre « Décodage matériel » dans Mac (`AppSettingsView.swift:158`), iOS (`:314`) et tvOS (`:213`).
- **Exemple de relais** : `relais.exemple.org:9443` est affiché tel quel en anglais (Windows `account.relay_example`, `Accounts.swift:93`, `AccountScreens.kt:339`, `apps/linux/src/ui/account.rs:189`, `server/web/src/pages/Devices.svelte:535`, installateurs).
- **LG, en anglais** : la carte 3 de l'accueil dit "ON YOUR NETWORK" au lieu de "ON THE NETWORK" (`apps/webos/src/ui/welcome.rs:170`, contre `link-ui.md:604`).
- **Correctif** : trancher chaque forme dans `link-ui.md`, puis l'appliquer ; `relay.example.org:9443` en anglais.

### TRANS-32 — Espace fine avant la ponctuation double
- **Catégorie** : i18n. **Gravité** : faible. **Effort** : M. **Confiance** : vérifié.
- **Description** : le contrat demande l'espace fine insécable avant `: ; ! ?`, mais seul `strings.py` (Android) l'impose. Le nombre de textes avec une espace simple, avec le nombre de textes déjà en espace fine entre parenthèses :

| Catalogue | Espace simple (espace fine) |
|---|---|
| Client web et Admin | 129 (47) |
| Admin Rust | 107 (2) |
| App Windows | 42 (42) |
| tray | 14 (25) |
| Installateur, interface | 19 (31) |
| Cœur | 54 (0) |
| Mac | 116 (0) |
| iOS | 134 (0) |
| tvOS | 117 (0) |
| Linux | 94 (0) |
| webOS | 47 (0) |

- **Scénario** : « Détails » peut se retrouver en fin de ligne et « : » au début de la suivante.
- **Correctif** : normaliser par un script, ou appliquer la règle dans `t()`, `tr()` et `fr()` à l'affichage.

### TRANS-33 — `docs/i18n.md` incomplet
- **Catégorie** : doc. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Catalogues absents de « Fichiers »** : `server/relay` (`t(lang, fr, en)` dans `pages.rs:66`, `v.tr` dans `accounts/`) et `server/macos/menubar/Strings.swift` (icône de la Station Mac, `--check-strings`).
- **Fichiers sans paire, mais justifiés et non cités dans la liste d'exceptions (`docs/i18n.md:89`)** : `docs/juridique/registre.md` (document interne, voir `docs/juridique/README.md:13`) et `apps/macos/PyroWave/upstream/README.md` (fichier tiers).
- **Correctif** : ajouter ces lignes, en FR et en EN.

### B.2 Règles d'interface (`docs/link-ui.md`)

### TRANS-34 — Empreinte du certificat : quatre affichages
- **Catégorie** : cohérence. **Gravité** : haute. **Effort** : S. **Confiance** : vérifié.
- **Où** : `link-ui.md:739` demande « les quatre premiers groupes ».

| App | Affichage |
|---|---|
| Mac, iPhone, iPad, Apple TV (`LinkUI.swift:81`), Windows (`api.ts:809`), web, Admin | `7C41 E09B AABB CCDD` |
| Linux (`app.rs:7761`) | `7c41 · e09b · aabb · ccdd` |
| LG (`webos/src/ui/settings.rs:308`) | `7C 41 E0 9B` |
| Android (`ui/add/AddHostScreen.kt:93`, `split(':').take(4)`) | `7C:41:E0:9B` |

- **Scénario** : après une réinstallation de la Station (le piège du TOFU), l'utilisateur doit comparer l'empreinte de l'app avec celle d'Admin. Sur Android et sur LG, il ne voit que 8 caractères sur 16, dans une autre forme. Sur Linux, elle est en minuscules.
- **Correctif** : une fonction `short_fingerprint` dans le cœur, exposée au pont C et au JNI, et supprimer les quatre copies.

### TRANS-35 — `parse_address` recopié, faux en IPv6
- **Catégorie** : bug. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `apps/linux/src/app.rs:7772` et `apps/webos/src/app.rs:4931`, identiques (`rsplit_once(':')`, puis on retire les crochets). Le cœur a son propre découpage (`share.rs:1342`), et Windows en a deux autres (`cli.rs:181`, `ui.rs:1504`).
- **Scénario** : `fe80::1` saisi sans port devient l'hôte `fe80:` sur le port 1, et l'app interroge cette adresse au lieu de refuser la saisie ou de prendre le port par défaut.
- **Correctif** : un seul `parse_host_port` dans le cœur, avec les cas IPv6 testés, utilisé par les quatre.

### TRANS-36 — Adresse d'Admin : crochets IPv6 oubliés ou doublés
- **Catégorie** : bug. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié (code) / plausible (déclenchement du repli).
- **Où** : `apps/android/.../Settings.kt:302-303` (`"https://${host.address}:${host.port}/"`) ; `apps/macos/.../SettingsView.swift:844` et `apps/tvos/.../SettingsView.swift:762` (crochets ajoutés à une adresse qui peut déjà en avoir). Le cœur a `console::admin_url` (`console.rs:353`), mais ni le pont C ni le JNI ne l'exposent.
- **Scénario** : Station jointe en IPv6. Le QR code et « Ouvrir Admin » donnent une URL invalide.
- **Correctif** : exposer `kn_admin_url` et `Native.adminUrl`, et supprimer les replis.

### TRANS-37 — Apple TV : la raison du HDR cache le bandeau de mise à jour
- **Catégorie** : cohérence. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `apps/tvos/KyberNowTV/HomeView.swift:383-405`. La raison du HDR (« HDR · {raison} ») passe avant `stationBanner`, `pairedBanner` et la phrase du profil. `link-ui.md` (l.45, 615, 775) prévoit le bandeau de la Station à cet endroit.
- **Scénario** : un téléviseur HDR face à un ordinateur qui n'a pas le HDR. Le bandeau « KyberNow Station {v} est disponible » ne s'affiche jamais à l'accueil de l'Apple TV.
- **Correctif** : ne montrer la raison que lorsque la puce HDR a le focus, comme son aide.

### TRANS-38 — Statistiques et « Trajet d'une image » incomplets
- **Catégorie** : cohérence. **Gravité** : moyenne. **Effort** : M. **Confiance** : vérifié par un sous-agent en lisant le code ; le cas Linux a été relu.
- **Où** :
  - **Linux** (`apps/linux/src/ui/session.rs:1436-1500`, l.113) : il manque le titre, « de la capture à votre écran », la croix, les repères « Latence » et « Débit », le bloc « Pertes · 30 s », et les lignes Réseau, Image, Mesures et Son.
  - **Android** (`SessionOverlay.kt:1219`, l.710-723) : six lignes en minuscules, sans titre ni lieux, et sans « — » quand une mesure manque.
  - **Client web** (`server/web/src/link/Session.svelte:749`) : trois blocs au lieu de six étapes.
- **Correctif** : reprendre le panneau de Windows. Plus durablement, que le cœur fournisse les lignes déjà libellées, comme il le fait pour les étapes de connexion.

### TRANS-39 — Autres écarts à `link-ui.md`
- **Catégorie** : cohérence. **Gravité** : faible. **Effort** : S/M. **Confiance** : vérifié, sauf mention contraire.
- **Windows et Linux** : pas d'erreur sous le nom à l'accueil (« Mot de passe refusé : mettez-le à jour… », « La Station a été réinstallée : vérifiez son empreinte… », l.41). Ces textes n'existent dans aucun de leurs catalogues ; leur affichage n'a pas été relu.
- **Windows** : une caméra refusée par Windows est annoncée « La caméra est utilisée par une autre app », sans lien vers `ms-settings:privacy-webcam` (`apps/windows/src/camera.rs:30-35,179`, l.1057).
- **Linux** :
  - le menu « Touches » a 10 entrées dans un autre ordre, au lieu de 4 (`app.rs:718-729`, l.98) ;
  - « Affichage des images » a deux choix de plus ;
  - « Garder la dernière image » est dans les réglages de l'app au lieu de ceux de l'ordinateur (`app.rs:6597`, l.543).
- **Apple TV** :
  - pas de réglage « Définition de la caméra » (`SettingsView.swift:529-558`, l.573) ;
  - « Gérer le compte » sans « Scannez ce code… » (`AccountViews.swift:381-400`, l.502) ;
  - définitions proposées : 4K, 1440p et 1080p, au lieu de « Au plus 4K » et « Sur mesure » (l.556). Linux propose de 2160p à 720p.
- **Android et LG** : « Ce lien de partage est illisible » et « partage indisponible » (réponse 503) absents (`MainActivity.kt:337`). Plausible.
- **Correctif** : corriger app par app, ou écrire l'exception dans `link-ui.md` quand elle est voulue.

### B.3 Constantes et logique hors du cœur

### TRANS-40 — Débit quand la fenêtre est cachée
- **Catégorie** : factorisation. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** :

| App | Débit plafonné |
|---|---|
| Mac (`apps/macos/KyberNow/SessionController.swift:330`, `occludedBitrate`) | 20 Mbit/s |
| iOS (`SessionController.swift:122`) | 8 Mbit/s |
| Android (`session/SessionActivity.kt:705`, `sessionSetBackground(handle, true, 2_000_000)`) | 2 Mbit/s |
| Windows, Linux | aucun plafond |

- **Scénario** : même usage (fenêtre réduite pendant une copie de fichiers), mais consommation et reprise différentes selon l'app.
- **Correctif** : une valeur dans le cœur (`session::background_bitrate(kind)`), appliquée par `sessionSetBackground` sans argument de débit, et appelée aussi par Windows et Linux.

### TRANS-41 — Bornes du débit réglé à la main
- **Catégorie** : factorisation. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** :

| Où | Bornes |
|---|---|
| Cœur (`profile.rs:324`) et Mac (`Model.swift:298-300`) | 5 à 4 294 Mbit/s |
| Windows (`Settings.svelte:387`) et Linux (`BITRATES`, `app.rs:2650`) | 500 au plus (Windows : 1 500 en PyroWave) |
| Apps Apple, PyroWave (`PyroWaveRate.max`) | 1 500 |
| Client web (`plan.ts:57-82`) | 10 à 150 (1 000 en PyroWave) |

  Le plafond de 95 Mbit/s de LG est justifié par sa prise réseau.
- **Correctif** : bornes exposées par le cœur, par plateforme et par format.

### TRANS-42 — Constantes et règles du cœur recopiées
- **Catégorie** : factorisation. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié. Les valeurs sont égales aujourd'hui ; le risque est qu'elles divergent.
- **Valeurs des profils** (60, 90, 300 ; +1/3 sans 4:4:4 ; +1/2 en H.264) : le cœur les calcule (`profile.rs:245`, `for_profile`, déjà exposé au JNI `exports.rs:860`). Elles sont recopiées dans `apps/macos/KyberNow/Model.swift:315-317` (partagé avec iOS et tvOS) et `apps/windows/ui/src/lib/api.ts:819`.
- **Formule PyroWave** (800 et 1 200 Mbit/s, puissance 0,75 bornée entre 0,4 et 3) : recopiée dans `PyroWaveSupport.swift:84-95`, depuis `profile.rs:25-26,319`.
- **Relais** :
  - `apps/linux/src/model.rs:655` et `apps/webos/src/model.rs:514` réécrivent une normalisation partielle (sans minuscules ni `https://`) au lieu de `relay::normalize`, et redéfinissent `DEFAULT_RELAY` ;
  - le cœur écrit lui-même `"share.kybernow.app"` en dur dans `welcome.rs:105`.
- **Délais et maxima** :
  - `RELAY_LINK_WAIT` (10 s, `share.rs:367`) dans `linux/app.rs:1067`, `webos/app.rs:807` et `ShareCore.swift:244` ;
  - `NOTICE_WAIT_SECS` et `BEFORE_NOTICE` "0.11.2" (`station_offer.rs:25,182`) dans `ConsoleClientCore.swift:241,247` et `StationOffer.kt:75` ;
  - `MAX_GUESTS` 4 dans `linux/app.rs:1198`, `webos/app.rs:928` et `Share.swift:488` ;
  - `MAX_SESSIONS` 4 (`focus.rs:35`, exposé par le pont C) dans `windows/ui/src/lib/api.ts:455`.
- **Accueil Android** : `core/Welcome.kt:56-80` recopie les 30 jours, les 24 heures et la logique de `welcome.rs`, et sert de repli en production quand l'appel natif échoue.
- **Port d'Admin 9443** en dur dans `Model.swift:846`, `Accounts.swift:385` et `windows/src/cli.rs:294`, sans constante dans le cœur. Les exemples de port de l'écran d'ajout diffèrent : 24811 sur Linux et LG, 37462 sur Mac.
- **Ports PlayStation** : écrits dans le cœur (`playstation/discovery.rs:29-37`, `endpoint.rs:16-20`), redéfinis dans Admin (`server/admin/src/consoles.rs:34-52`) et dans les tests du relais (`server/relay/tests/common/mod.rs:112`). Le relais et Admin doivent faire suivre exactement les ports que le cœur ouvre. La caisse commune `server/relay/tunnel`, déjà utilisée par les trois, peut les porter.
- **Correctif** : exposer ces valeurs par le cœur (et `kybernow-tunnel` pour les ports), et supprimer les copies.

---

## C. Documentation

Les paires FR/EN sont **toutes synchronisées** (même dernier commit, même nombre de titres), à une exception près (TRANS-45). Aucun lien relatif n'est cassé. Une trentaine d'affirmations ont été vérifiées sur pièces (scripts, options de `release.sh`, routes, variables `KYBERNOW_*`, numéros de correctifs) et sont justes.

### TRANS-43 — `ROADMAP` en retard
- **Catégorie** : doc. **Gravité** : moyenne. **Effort** : S. **Confiance** : vérifié.
- **Où** : `ROADMAP.md` et `ROADMAP.en.md`, modifiés pour la dernière fois le 23/11.
- **Description** :
  - « HDR » (première version et version suivante : Android, Windows, Linux) : livré en 0.9.8 (`CHANGELOG.md:125`).
  - « KyberNow Station pour Mac » : les jalons sont livrés en 0.9.8 (`:124`). Seules les « Demandes à Apple » restent.
  - « Consoles PlayStation » : « Première version, app Mac » et « Ensuite : les autres apps Link » sont livrées en 0.9.8 (`:126`, `:132`).
  - « iPhone et iPad : Apple Pencil » : livré en 0.11.0 (`:89`).
  - `ROADMAP.md:19` renvoie à `docs/archives/mesures.md` comme liste de travail, contre la règle de `docs/i18n.md:86`.
  - Cela contredit la règle de tête du fichier : « une ligne sort quand elle est faite ».
- **Correctif** : retirer ces lignes, ou les reformuler en essais restants (HDR jamais essayé sur un écran HDR Windows, Android ou Linux ; PS4) ; énoncer les mesures restantes dans la ligne même.

### TRANS-44 — Le site annonce encore la Station Linux pour bientôt
- **Catégorie** : doc. **Gravité** : moyenne. **Effort** : M. **Confiance** : vérifié.
- **Où** : `site/index.html:380` et `:602` (« Une Station pour Linux arrive bientôt »), `site/guide/index.html:397` (« ne se télécharge pas encore »), et les mêmes lignes dans `site/en/`. La 0.11.7 publie « KyberNow Station pour Linux, en bêta » (`CHANGELOG.md:16`), et `release.sh` joint les .deb, .rpm et .tar.xz.
- **Correctif** : accueil, FAQ, guide et carte de téléchargement, suivant `docs/ton-du-site.md`.

### TRANS-45 — README anglais du cœur incomplet
- **Catégorie** : doc. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** : `apps/core/README.md:357`, section « Accueil et compte KyberNow (`src/welcome.rs`) », ajoutée par 845cdfc0. Elle est absente de `apps/core/README.en.md` (26 titres contre 25).
- **Correctif** : la traduire.

### TRANS-46 — Messages de journal inexacts dans la doc de reprise
- **Catégorie** : doc. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** : `docs/reprise-apres-perte.md:113-117` (et `.en.md`) demande de chercher « Loss repaired by invalidation » ou « by a long-term reference ». Le correctif `server/kyber-patches/0060-kymedia-ffmpeg-reprise.patch:221` écrit `"Repair up to %d at %d: %s"`, au niveau VERBOSE. Pour VideoToolbox, seule une ligne est écrite au démarrage (`:461`).
- **Scénario** : pendant l'essai NVENC ou Station Mac à faire, le testeur ne trouve jamais la ligne attendue et conclut à tort que la réparation ne marche pas.
- **Correctif** : citer les vrais messages et le niveau de journal requis.

### TRANS-47 — `link-ui.md` en retard ; chemins ambigus
- **Catégorie** : doc. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **`link-ui.md`** :
  - l'infobulle du profil cite Auto sur Mac (`SessionChrome.swift:865`), pas dans la doc (l.91) ;
  - l'absence d'écran « Réglage du HDR » sur Windows et Linux, prévue par `hdr.md`, n'est pas dite à la l.183 ;
  - le Mac écrit « Retirer cet ordinateur… » avec des points de suspension.
- **Chemins** : `PROJECT.md:42` et `apps/linux/os/README.md:12` citent `scripts/deb.sh` et `scripts/apt-repo.sh` comme s'ils étaient à la racine (ils sont dans `apps/linux/scripts/`), et `docs/pyrowave-client-android.md:43` cite `scripts/strings.py` (dans `apps/android/scripts/`).
- **Voir aussi** TRANS-22 (PyroWave par défaut) et TRANS-33 (contrat i18n).

---

## D. Code mort à l'échelle du dépôt

### TRANS-48 — Vidéo du site dans git
- **Catégorie** : code mort (poids). **Gravité** : moyenne. **Effort** : M (L avec la purge de l'historique). **Confiance** : vérifié.
- **Où** : `site/assets/video/`, 179 Mo au commit courant : 69 segments HLS (de 3 à 7,6 Mo en 2160p) et `presentation-720p.mp4` (13,6 Mo). Deux versions dans l'historique (24/10), soit environ 350 Mo de blobs ; `.git` fait 347 Mo.
- **Scénario** : chaque clone, chaque worktree neuf et la CI la téléchargent, et chaque nouveau montage ajoute environ 170 Mo à jamais.
- **Correctif** : Git LFS pour `site/assets/video/**`, ou servir la vidéo depuis une release ou un CDN. Purger l'historique est possible, mais oblige à refaire les clones.

### TRANS-49 — Fichiers suivis que rien n'utilise
- **Catégorie** : code mort. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** :
  - `brand/` (logo v1 : SVG, `kybernow-app-icon-1024.png`, `preview.html`) : `brand/README.md:5` dit lui-même que ni les apps ni le site ne s'en servent.
  - 10 PNG dans `apps/webos/screenshots/` et `apps/android/app/screenshots/tablette-accueil.png` (environ 3,5 Mo), sorties de tests qu'aucun README ne cite, recommitées à chaque refonte.
- **Correctif** : déplacer `brand/` v1 dans `brand/archives/` ou le supprimer ; ignorer les captures non citées.

### TRANS-50 — Dépendances Cargo inutilisées
- **Catégorie** : code mort. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié par grep ; plausible pour `kynput-sys`, qu'un `-sys` peut garder pour l'édition de liens.
- **Où** :
  - `apps/android/jni/Cargo.toml:33` : `ndk-sys` (aucun `ndk_sys`, seul `ndk::` est employé) ;
  - `apps/android/jni/Cargo.toml:25`, `apps/linux/Cargo.toml:15` et `apps/webos/Cargo.toml:15` : `kynput-sys`, aucun `kynput_sys` dans les sources (le cœur l'apporte déjà).
- **Correctif** : retirer, puis confirmer à la construction (`unused_crate_dependencies`). Les quatre `package.json` sont entièrement utilisés.

### TRANS-51 — Constantes et nettoyage morts
- **Catégorie** : code mort. **Gravité** : faible. **Effort** : S. **Confiance** : vérifié.
- **Où** :
  - `apps/core/src/playstation/regist.rs:30` (`PORT`) et `takion.rs:27` (`PORT`) ne sont jamais lues. `session.rs:36` (`PORT`) ne sert qu'à l'en-tête `Host`. Les trois redoublent `endpoint::PORT_RP` et `PORT_TAKION`.
  - `scripts/arbres.sh:99` efface `kysdk/.kybernow-correctifs`, alors que le marqueur réel est `kyber-desktop/.kybernow-correctifs-apps` (`kyber-client-patches.sh:80`).
- **Correctif** : supprimer, et employer `endpoint::PORT_RP` dans `session.rs`.

---

## Les 10 priorités

1. **TRANS-01** : corriger l'empreinte de la surcouche, contrôler le sous-système PE, reconstruire et publier une 0.11.8. La 0.11.7 n'a pas le correctif qu'elle annonce.
2. **TRANS-02, TRANS-03, TRANS-04, TRANS-16** : compléter les empreintes (Station Linux, PyroWave Mac, nightly, recettes). C'est la même famille que TRANS-01 et que le Vendor Mac : une seule fonction d'entrées par produit, testée par `scripts/tests/release-outils.sh`.
3. **TRANS-26** : remettre les textes caméra dans `strings.py`, avant que la prochaine génération casse la construction Android.
4. **TRANS-25 et TRANS-27** : rétablir `bar.mouse_shortcut` et faire vérifier les clés employées par tous les vérificateurs i18n.
5. **TRANS-12** : relais testé en CI avant chaque image Docker, et outils de publication testés.
6. **TRANS-34, TRANS-35, TRANS-36** : empreinte du certificat, adresses et URL d'Admin calculées par le cœur. Elles touchent la sécurité perçue (TOFU) et l'IPv6.
7. **TRANS-07, TRANS-09, TRANS-05, TRANS-06** : plus de binaire périmé ni d'état hérité d'un build précédent (caméra Mac, surcouche non atomique, remise à l'origine, correctifs Apple).
8. **TRANS-43 et TRANS-44** : `ROADMAP` et site alignés sur ce qui est publié (Station Linux surtout).
9. **TRANS-40, TRANS-41, TRANS-42** : rapatrier dans le cœur débit en arrière-plan, bornes, profils et constantes de partage.
10. **TRANS-30, TRANS-37, TRANS-38** : textes de session du web, bandeau de l'Apple TV et statistiques alignés sur `link-ui.md`.

## Ce qui est sain

- **Scripts** : `set -euo pipefail` dans tous les scripts exécutables (les bibliothèques sourcées ne l'imposent pas, à dessein). Guillemets soignés (`${a[@]+"${a[@]}"}`). Aucun chemin `/Users/…` en dur. VM, hôte SSH, clés et dépôts passent par des variables `KYBERNOW_*` avec une valeur par défaut. Aucun script mort : tous sont appelés ou documentés, sauf `release-outils.sh`.
- **Publication** : les états, empreintes et caches sont écrits dans un fichier temporaire puis renommés, donc sans course entre voies. L'intégrité des caches est revérifiée (`shasum -c`). Les verrous gèrent l'imbrication et les processus terminés. La reprise après un échec (commit de version, flux, brouillon, tag) est cohérente. `release-apk.sh` épingle le certificat et lit le mot de passe dans le Trousseau.
- **Traductions** : parité FR/EN complète des clés et des paramètres dans tous les catalogues JSON ; catalogues Apple entièrement traduits ; aucune valeur anglaise vide ni restée en français ; typographie anglaise propre. Les actions clés concordent dans les 8 apps : Se connecter, Plein écran, Ajouter un ordinateur ou une console, Plus tard, Créer un compte, Mettre à jour, Rejoindre avec un code.
- **Cœur partagé** : la mise à jour des Stations (« Plus tard » de 7 jours, une vérification par jour), le réveil par le réseau, le retour de la Station en 90 s, les erreurs des comptes et du relais, les étapes de connexion et les phrases des profils viennent tous du cœur, sans réimplémentation. Les barres de session Mac et Windows ont le même ordre, les mêmes raccourcis et les mêmes délais.
- **Documentation** : paires FR/EN synchronisées, liens relatifs intacts, chemins, options et variables cités qui existent bien.
- **Dépôt** : aucun `.orig`, `.bak`, `.rej`, `.DS_Store` ni `dist/` suivi. `tools/latency` et `tools/banc` sont vivants et documentés. Les `package.json` sont entièrement utilisés. Aucune dépendance inutile dans le cœur, l'app Windows, le pont Apple, Admin, le relais, le tunnel, le tray, le micro ni l'installateur.
