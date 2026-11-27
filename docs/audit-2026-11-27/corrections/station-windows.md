# Corrections de l'audit du 27/11 — Station Windows (et code d'Admin des Stations Mac et Linux)

Branche `claude/fix-station-windows-9ecece`. Zone : `server/admin`, `server/tray`, `server/installer`, `server/camera`, `server/micro`, `server/encoder-check`, `server/build-windows.sh`. Rapports : [station-windows.md](../station-windows.md), et les constats d'Admin de [stations-mac-linux.md](../stations-mac-linux.md).

## Constats corrigés

**Mises à jour et installateur**
- **STWIN-01** : marqueur de bascule `update\bascule.json` écrit (forcé sur le disque) avant le premier renommage, effacé quand tous les dossiers sont en place, réécrit pendant un retour arrière ; au démarrage, Admin finit la bascule dans le sens de la console qui tourne (`apply::recover`) ; l'installateur reprend les dossiers absents avant de chercher les réglages à garder. `update.ps1` écrit le même marqueur.
- **STWIN-02 + STWIN-43** : une seule liste, `server/admin/src/update/windows_common.rs` (incluse par l'installateur par `#[path]`) : processus (avec `kybernow-micro.exe` et `kybernow-camera.exe`), SDDL, relance, marqueur, verrou. `update.ps1` garde sa copie, comparée par un essai. Frame Server arrêté pendant la bascule ; bibliothèques de la caméra encore chargées renommées dans `.staging\verrouilles\` et effacées au redémarrage (remises si la mise à jour est annulée). L'installateur ne supprime plus les services avant d'avoir écrit les fichiers : il les supprime juste avant de les recréer.
- **STWIN-03** : relance automatique de `kyber-admin` (à la création du service, par l'installateur, et à chaque démarrage d'Admin, `failureflag` compris) ; crochet de panique qui écrit dans `kyber-admin.log` puis arrête Admin en échec (relancé par Windows, `Restart=on-failure` sous Linux, `KeepAlive` sur Mac) ; `service_main` sort en échec si Admin s'arrête sur une erreur.
- **STWIN-13** : secrets de Kyber recopiés dans `.staging` réservés à SYSTEM et aux Administrateurs tout de suite ; restes de `.staging` effacés au démarrage (après une heure, voir « Compatibilité »).
- **STWIN-23** : relance automatique de `kyber` (et de `kyber-admin`) reposée à chaque démarrage d'Admin.
- **STWIN-24** : « Installer » vérifié et réservé dans la même prise du verrou ; verrou entre processus `Global\KyberNowUpdate` pris par l'applicateur, `update.ps1` et l'installateur ; Admin ne reprend pas une bascule tant qu'il est tenu.
- **STWIN-25** (et **STCOM-04**) : paquet téléchargé en flux dans un fichier, délai à la mesure du paquet (10 min au moins, 32 Kio/s), aussi pour le paquet de retour arrière de Linux.
- **STWIN-27** : l'installateur retire `*.prev`, `*.bad`, `*.new` et `.staging`.
- **STWIN-28** : `update.ps1` refuse une version qui n'est pas plus récente, sauf `-Force`.
- **STWIN-29** : `kybernow.json` écrit d'un bloc (temporaire forcé sur le disque, puis renommage).
- **STWIN-11** : outils de Windows lancés par leur chemin complet sous `System32` (`sys::resolve`), `ping` et `cmd` de la désinstallation aussi.
- **STWIN-18 (7e point)** : chemins insérés dans le script PowerShell du raccourci avec apostrophes doublées.
- **Même défaut que WINAPP-02** : `cmd /C start` remplacé par `ShellExecuteW`, après contrôle de l'adresse (`sys::safe_web_url`), dans l'interface et le lanceur.
- **TRANS-23** : `WebView2Loader.dll` choisi d'après la version de `webview2-com-sys` du `Cargo.lock` ; en-tête de `build.sh` corrigé (macOS seulement).

**Sécurité de l'API**
- **STWIN-04** : l'appareil en session est reconnu par son identifiant **et** son compte (`Activity::is_live`, partage, enregistrement, prêt, signal de vie, fin, reprise de main, autorisation du relais).
- **STWIN-05** : régler le relais, rattacher et enrôler demandent aussi le jeton de l'installateur (`installer-token`, dans le dossier de données réservé aux administrateurs, en-tête `x-kyber-installer`) ; l'installateur l'envoie.
- **STWIN-14 (en partie)** : accepter un appareil et installer une mise à jour par les routes locales sont refusés pendant un prêt ; le rattachement passe par le jeton de l'installateur, que l'ami ne peut pas lire.
- **STWIN-06 (en partie) + STWIN-10** : essais de codes comptés par /64, partagés entre `share/join` et le prêt (un code de prêt révélé par `join` ne double plus le débit) ; plafond de 30 mauvais codes en 10 minutes, toutes adresses confondues ; mots de passe de comptes Kyber refusés plafonnés (20 par minute) sur `/api/stream/*`.
- **STWIN-07** : processus du prêt suivis par identifiant et instant de création ; à la fin, un processus n'est fermé que s'il est toujours le même et dans la session du prêt ; un nouveau processus qui reprend un identifiant est jugé.
- **STWIN-08** : jeux reconnus seulement dans les bibliothèques de Steam (`libraryfolders.vdf`) ; dossiers de Windows où un compte standard peut écrire (`Temp`, `Tasks`, `tracing`, `spool`…) exclus.
- **STWIN-15** : dossier des enregistrements refusé s'il est (ou passe par) un lien ou une jonction : écriture de Kyber, liste, lecture, effacement, ménage.
- **STWIN-16** : un compte KyberNow fait l'hôte du prêt avec le droit Administrer (Session seul ne suffit plus) ; les comptes Kyber appairés gardent l'accès.
- **STWIN-18** : changement de mot de passe limité comme la connexion ; code d'initialisation comparé à temps constant ; essais manqués comptés par /64 et oubliés après une heure ; appairage compté par /64, plafond total relevé à 20 ; `:` refusé dans les noms de journaux ; `kyber_config.toml` jamais remplacé si ses droits n'ont pas pu être posés.
- **STWIN-32** : adresse de la page d'Admin vérifiée sur son préfixe (plus de panique, `http://@hôte` refusé).

**Caméra**
- **STWIN-19** : image lue sous le verrou, file revérifiée : `Shutdown` ne démappe plus la vue pendant une lecture.
- **STWIN-21** : file d'événements prise sous le verrou (comme `GetEvent`), cadence lue et écrite sous le verrou, attente hors du verrou.
- **STWIN-20** : tampon de conversion, vue et handle libérés dans `~Pin` ; `malloc` vérifié ; allocateur arrêté avant le fil ; fil gardé (et attendu sans limite avant la destruction) s'il ne s'arrête pas en 5 s.
- **STWIN-12** : mémoire partagée lisible, sous Windows 11, par SYSTEM, Service local et Service réseau (Frame Server) ; sous Windows 10 par tous (le filtre est dans l'app qui filme) ; plus par les apps en bac à sable (AppContainer).
- **STWIN-50 (en partie)** : `camera/build.sh` et `encoder-check/build.sh` (macOS) effacent la sortie avant `clang`.

**Réglages et données**
- **STWIN-31** : module `store` : chaque lecture réussie garde une copie `.bak` ; un fichier illisible est mis de côté (`.invalide-<date>`) et la dernière copie lisible reprise ; pour `kyber-admin.toml`, `accounts.json`, `invites.json`, amis et état du prêt, dernières sessions de prêt, `devices.json`, `events.json`.

**Code d'Admin des Stations Mac et Linux**
- **STLIN-01 = STWIN-09** : fichiers du demandeur ouverts avec ses droits (`setfsuid`), `O_NOFOLLOW | O_NONBLOCK`, type et taille lus par `fstat` ; réserve en 0700, fichiers en 0600 ; manifeste, signature et paquet effacés si la vérification échoue.
- **STLIN-02 (côté service)** : seul le compte de la Station (le premier qui a demandé, noté dans `proprietaire`, root toujours accepté) pilote le service root. L'unité du socket reste à faire (autre session).
- **STLIN-03** : 10 s pour lire la demande ; `flock` sur `.lock`, « mise à jour déjà en cours » sinon.
- **STCOM-01** : SIGTERM et SIGHUP arrêtent Admin proprement (écrans, son, prêt rendus).
- **STCOM-02** : `secure_dir` (0700) et `secure_file` (0600, lien refusé) sur Mac et Linux ; clé TLS d'Admin écrite ainsi.
- **STLIN-20** : icônes des apps Linux dans un dossier propre au compte (`$XDG_RUNTIME_DIR`, nom aléatoire, création exclusive en 0700).
- **STMAC-23** : équipe de signature compilée (`P6DB682S4K`), vérifiée par une exigence `codesign -R`, plus celle de l'app installée.

**Textes**
- **STWIN-48** : textes définitifs de PyroWave (FR et EN, Windows, Mac, Linux).
- **TRANS-28** : erreur VB-CABLE traduite (`api.vbcableMissing`).

## Constats laissés

- **STWIN-06 (longueur et durée des codes)** : six chiffres, c'est le protocole du cœur et des apps (`code.len() != 6` côté Station et saisie dans les apps) ; les allonger demande une version coordonnée de toutes les apps (vague 2). La durée de 24 h est un choix documenté ; le plafond de toutes les adresses rend la recherche impraticable (au plus 30 mauvais codes en 10 minutes). Le `"lend": true` de `join` reste : le cœur s'en sert pour passer à `borrow`.
- **STWIN-14 (fichiers Steam, jeton renouvelé)** : les fichiers Steam mis de côté sont écrits par l'agent de l'icône dans la session (`tray/src/lend/store.rs`) ; les confier à Admin demande un nouvel échange agent ↔ Admin. Renouveler le jeton local au début du prêt sans le recopier couperait l'agent de console du prêt, qui l'utilise. Les routes sensibles sont fermées pendant le prêt à la place.
- **STWIN-03 (verrous tolérants)** : environ 590 `lock().unwrap()` ; le crochet de panique arrête Admin, qui est relancé : un verrou empoisonné ne laisse plus un Admin vivant et inutilisable.
- **STWIN-17** (relais de confiance totale, effort L, relais hors zone), **STWIN-22** (micro par défaut, M), **STWIN-26** (VB-CABLE et choix `microphone`, touche l'installateur et `kybernow.json`), **STWIN-30**, **STWIN-33** à **STWIN-36**, optimisations **STWIN-37** à **STWIN-42**, factorisations **STWIN-44** à **STWIN-46**, **STWIN-47**, **STWIN-49** (`docs/pret.md`, ligne de délai) : hors des priorités demandées, à reprendre en vague 2.
- **STWIN-50 (correctifs 0085 et fichiers qui s'accumulent)** : renuméroter les correctifs touche `server/kyber-patches` et la publication ; hors zone.
- **STLIN-02 (unité du socket)** : `server/linux/packaging`, autre session.

## Ce qui est vérifié, et comment

- `cargo test` d'Admin sur le Mac : 390 essais passés (dont les nouveaux : coupure après chacun des six renommages, marqueur d'`update.ps1`, listes d'`update.ps1` comparées à la liste commune, nettoyage de `.staging`, identifiant seul refusé, jeton de l'installateur, limites des codes par /64 et globales, plafond des mots de passe Kyber, hôte du prêt et droit Administrer, règles du prêt (`steamapps` et dossiers de Windows), appairage par /64, journaux avec `:`, copie `.bak` des données et des réglages, liens et FIFO du service root, propriétaire et verrou du service root, droits Unix, dossier d'enregistrement lié, adresse de la page d'Admin).
- `cargo test` de l'installateur (44, dont reprise d'une bascule et restes retirés, adresses ouvertes dans le navigateur), de l'icône (28) et du micro (7).
- `cargo check --target x86_64-pc-windows-gnu` : Admin et installateur (bibliothèque, lanceur et interface `--features ui`) compilent sans avertissement nouveau.
- `cargo check --target armv7-unknown-linux-gnueabi` (chaîne webOS comme racine système, faute de chaîne aarch64) : le code Linux d'Admin compile (`setfsuid`, `flock`, `XDG_RUNTIME_DIR`).
- Caméra : `KyberNowCameraSource.dll` et `KyberNowCameraFilter.dll` construites avec mingw ; `install.cpp` et `out_windows.c` compilés à blanc.
- `update.ps1` analysé par PowerShell 7 (`Parser::ParseFile`, sans erreur) ; le marqueur qu'il écrit relu par Admin dans un essai.
- Rien n'a été essayé sur un vrai Windows, Mac ou Linux.

### Compatibilité avec une Station 0.11.7

La 0.11.7 se met à jour avec son propre applicateur (sans marqueur ni verrou) : rien ne change pour elle. La nouvelle console démarrée par cet applicateur n'efface pas les restes de `.staging` de moins d'une heure (l'ancien applicateur peut encore y remettre les dossiers s'il revient en arrière). `GET /api/session` et `resultat.json` gardent leur format ; `update.ps1` accepte toujours `-Source` (consoles d'avant la 0.2.6).

### À essayer sur appareil

- Une mise à jour normale 0.11.7 → 0.11.8, puis une coupure simulée (arrêt brutal de la VM ou du PC) pendant la bascule d'une mise à jour 0.11.8 → suivante : la Station doit repartir dans une seule version et la page le dire.
- Une mise à jour pendant que Teams ou l'appli Caméra utilise « KyberNow Camera » (Windows 10 et 11) ; la caméra sous Windows 11 avec les nouveaux droits de la mémoire partagée (Frame Server doit lire l'image : si l'image reste grise, c'est la liste de lecture de `KN_SHARED_SDDL_WIN11` à compléter).
- L'installateur en réinstallation (« Conserver mes réglages ») et le rattachement pendant l'installation (jeton de l'installateur).
- Le prêt : jeux Steam hors du dossier de Steam, fin du prêt, et un compte KyberNow avec Session seul.
- Station Linux : mise à jour par le service root (droits du demandeur, 0700/0600) ; Station Mac : mise à jour signée et arrêt par launchd (SIGTERM).

## Notes de version

**FR**
- Une coupure de courant ou un gel pendant une mise à jour ne laisse plus votre Station à moitié installée : au redémarrage, elle se remet d'aplomb toute seule.
- Les mises à jour et la réinstallation passent même quand la caméra ou le micro de votre appareil sont en service, et ne laissent plus la Station sans services si quelque chose bloque.
- KyberNow Admin redémarre tout seul s'il s'arrête sur une erreur.
- Une mise à jour fonctionne aussi sur une connexion lente.
- Sécurité renforcée : les codes de partage et de prêt, les mots de passe des appareils et le prêt de l'ordinateur sont mieux protégés contre les essais à répétition, et un autre compte de l'ordinateur ne peut plus rattacher votre Station à son compte.
- Vos réglages et vos listes (appareils, comptes, amis) ne sont plus jamais effacés si un fichier est abîmé : la dernière version lisible revient.
- Caméra de l'appareil sous Windows : plus stable, et son image n'est plus lisible par les apps qui n'y ont pas droit.

**EN**
- A power cut or a freeze during an update no longer leaves your Station half installed: when it restarts, it puts itself right.
- Updates and reinstalling work even while your device's camera or microphone is in use, and no longer leave the Station without its services if something gets in the way.
- KyberNow Admin restarts on its own if it stops on an error.
- Updates also work on a slow connection.
- Stronger security: share and loan codes, device passwords and lending the computer are better protected against repeated guesses, and another account on the computer can no longer attach your Station to its own account.
- Your settings and lists (devices, accounts, friends) are never wiped when a file is damaged: the last readable version comes back.
- Your device's camera on Windows: more stable, and its picture can no longer be read by apps that are not allowed to.
