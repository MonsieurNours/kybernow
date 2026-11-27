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

**Deuxième passe (demande de la coordination : rien de côté, hors factorisation de vague 2)**
- **STLIN-02 (message)** : un socket qui refuse (`EACCES`) donne un message clair côté Admin.
- **STLIN-05** : sous Linux, Admin dit avant d'accorder un prêt que le clavier et la souris ne peuvent pas être bloqués (`input_locked`, `input_warning` de la vue du prêt ; ligne dans le fil au début du prêt).
- **STLIN-07** : canal et version contrôlés avant de copier le paquet ; rien ne reste d'une demande refusée (dossier retiré, sauf une version déjà gardée).
- **STLIN-08** : `apt-get` attend le verrou de dpkg (`DPkg::Lock::Timeout=300`).
- **STCOM-03** : aide du micro attendue après délai (pas de zombie), sortie d'erreur lue au fil de l'eau.
- **STCOM-12** : message neutre `display.noVirtualDisplayAgent` (plus « pas encore d'écran virtuel sur Mac » sous Linux).
- **WEB-14** : `/api/session` donne `stream_user` (session d'Admin ouverte) ; `/api/web/ticket` reste pour les clients d'avant.
- **STWIN-22** : registre partagé du micro par défaut (`micro-defaut.txt`, verrou `Global\KyberNowMicroDefault`) : la dernière aide le rend ; `--check` le rend après une aide tuée, lancé par Admin à son démarrage.
- **STWIN-26** : choix `microphone` noté dans `kybernow.json` ; VB-CABLE installé après une mise à jour seulement s'il a été choisi, une seule fois, marque posée seulement sur succès (anciennes marques par version reprises puis effacées).
- **STWIN-30** : icône lancée à l'ouverture de session par `HKLM\…\Run` (tous les comptes) ; ancienne valeur `HKCU` retirée par l'installateur, la désinstallation, l'icône et `update.ps1`.
- **STWIN-33** : signe de vie du prêt sur un fil à lui ; délais courts pour la partie jouée (3 s) et les icônes (2 s).
- **STWIN-34** : réinitialisation du mot de passe : le code va dans le profil du compte qui l'a demandé (chemin passé à la partie élevée, nom contrôlé) ; un échec déjà affiché n'est plus dit « erreur système 1 ».
- **STWIN-35** : fenêtre de l'installateur refermable après une panique de l'installation (garde `Drop`, verrou tolérant).
- **STWIN-36** : lanceur du prêt refusé hors prêt ; identifiant d'appairage encodé par l'icône ; icône ratée redemandée ; caméra : filtre de Windows 10 reconnu sur un PC passé à Windows 11 (et mémoire partagée laissée lisible par les apps dans ce cas), `mfsensorgroup.dll` chargée une fois ; micro : événement fermé sur erreur, tampon jamais attendu dans le fil du son (Windows et Linux).
- **STWIN-37** : activité des cartes relevée chaque seconde pendant un flux ou quand une page la regarde, toutes les 10 s sinon.
- **STWIN-38** : `/api/status` et `/api/local/state` sans appel bloquant sur le fil asynchrone ; nom de l'ordinateur gardé 30 s ; réglages écrits hors de leur verrou (une copie plus ancienne n'écrase jamais une plus récente) ; `events.json` écrit sans forcer le disque.
- **STWIN-39** : port de Kyber gardé en mémoire tant que `kyber_config.toml` ne change pas ; puce du Mac lue une fois.
- **STWIN-40** : PlayStation recherchées en fond seulement après une console vue ; états des agents réécrits quand ils changent (ou toutes les 2 s) ; veille du lanceur seulement avec une app ouverte ; `kybernow.json` de l'icône relu quand il change.
- **STWIN-41** : liste des versions demandée avec son ETag ; manifeste vérifié gardé pour la même version.
- **STWIN-42** : `npm ci` et recompilation d'Admin seulement si `package-lock.json` ou `web/dist` ont changé ; paquet extrait et copié sans forcer chaque fichier, volume forcé d'un coup avant la bascule (fichier par fichier s'il refuse).
- **STWIN-47** : `/api/lend/catalog` retirée ; `Role::from_rights` réservé aux essais.
- **STWIN-49** : `docs/pret.md` : 35 s pour `ready` (FR et EN).
- **STWIN-50 (fichiers qui s'accumulent)** : copies des réinstallations bornées (installateur et Admin) ; marques `micro-vbcable-<version>` effacées ; `.staging` vidé au démarrage (après une heure).
- **STWIN-17 (court terme)** : `docs/relais.md` (FR et EN) dit ce que le relais voit et signe avec « Admin à distance » et les comptes.

## Constats laissés

- **STWIN-06 (longueur des codes)** : six chiffres, c'est le protocole du cœur et de toutes les apps (`code.len() != 6` côté Station, saisie et `"lend": true` dans les apps). Les allonger demande une version coordonnée des apps (cœur, Windows, Android, Apple, web) : hors zone. La durée de 24 h reste (documentée) ; le plafond de 30 mauvais codes en 10 minutes, toutes adresses confondues, rend la recherche impraticable.
- **STWIN-14 (fichiers Steam de l'hôte)** : `config.vdf.kybernow` et `local.vdf.kybernow` sont mis de côté par l'agent de console, qui tourne sous le compte de l'hôte, comme les apps de l'ami : rien de ce que l'agent lit ne peut être caché à l'ami dans cette session. Les confier à Admin (SYSTEM) demande un échange agent ↔ Admin qui sache rendre les fichiers même quand Admin ne répond plus à la fin du prêt (aujourd'hui la remise en état marche hors ligne) : c'est un changement de conception du prêt, à décider avec l'auteur. Renouveler le jeton local sans le recopier couperait l'agent ; les routes sensibles sont fermées pendant le prêt à la place.
- **STWIN-17 (fond)** : TLS de bout en bout jusqu'à la Station pour « Admin à distance », ou seconde preuve locale : relais et apps, hors zone (effort L).
- **STWIN-03 (verrous tolérants)** : environ 590 `lock().unwrap()` ; le crochet de panique arrête Admin, qui est relancé : un verrou empoisonné ne laisse plus un Admin vivant et inutilisable.
- **STWIN-44 à 46** : factorisation, vague 2 (demande de la coordination).
- **STWIN-50 (correctifs numérotés 0085)** : `server/kyber-patches`, hors zone (session des correctifs).
- **STLIN-02 (unité du socket)** : `server/linux/packaging`, autre session.
- **TRANS-21** : rien dans `server/camera/build.sh` (constat sur `apps/linux/os` et `apps/webos`).

## Ce qui est vérifié, et comment

- `cargo test` d'Admin sur le Mac : 394 essais passés (deuxième passe comprise : canal refusé sans copie, compte de streaming dans `/api/session`, aide du micro tuée et attendue, choix du micro lu dans `kybernow.json`, hôte du prêt) (dont les nouveaux : coupure après chacun des six renommages, marqueur d'`update.ps1`, listes d'`update.ps1` comparées à la liste commune, nettoyage de `.staging`, identifiant seul refusé, jeton de l'installateur, limites des codes par /64 et globales, plafond des mots de passe Kyber, hôte du prêt et droit Administrer, règles du prêt (`steamapps` et dossiers de Windows), appairage par /64, journaux avec `:`, copie `.bak` des données et des réglages, liens et FIFO du service root, propriétaire et verrou du service root, droits Unix, dossier d'enregistrement lié, adresse de la page d'Admin).
- `cargo test` de l'installateur (44, dont reprise d'une bascule et restes retirés, adresses ouvertes dans le navigateur), de l'icône (28) et du micro (9, dont le registre du micro par défaut).
- `cargo check --target x86_64-pc-windows-gnu` de l'icône et du micro : sans erreur.
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
- Station Linux : mise à jour par le service root (droits du demandeur, 0700/0600, `apt-get` pendant unattended-upgrades) ; avertissement du prêt sans règle udev des entrées. Station Mac : mise à jour signée et arrêt par launchd (SIGTERM).
- Deux appareils qui envoient leur micro en même temps, puis une session coupée brutalement : le micro par défaut de Windows doit revenir.
- « Démarrer avec l'ordinateur » depuis l'icône (HKLM), et la réinitialisation du mot de passe avec les identifiants d'un autre administrateur.

## Notes de version

**FR**
- Une coupure de courant ou un gel pendant une mise à jour ne laisse plus votre Station à moitié installée : au redémarrage, elle se remet d'aplomb toute seule.
- Les mises à jour et la réinstallation passent même quand la caméra ou le micro de votre appareil sont en service, et ne laissent plus la Station sans services si quelque chose bloque.
- KyberNow Admin redémarre tout seul s'il s'arrête sur une erreur.
- Une mise à jour fonctionne aussi sur une connexion lente.
- Sécurité renforcée : les codes de partage et de prêt, les mots de passe des appareils et le prêt de l'ordinateur sont mieux protégés contre les essais à répétition, et un autre compte de l'ordinateur ne peut plus rattacher votre Station à son compte.
- Vos réglages et vos listes (appareils, comptes, amis) ne sont plus jamais effacés si un fichier est abîmé : la dernière version lisible revient.
- Caméra de l'appareil sous Windows : plus stable, et son image n'est plus lisible par les apps qui n'y ont pas droit.
- Le micro de vos appareils rend toujours le micro d'origine de l'ordinateur, même à plusieurs ou après une coupure.
- VB-CABLE n'est plus réinstallé si vous ne l'avez pas choisi.
- L'icône KyberNow Station démarre avec la session de chaque compte de l'ordinateur.
- KyberNow Admin travaille moins quand personne ne joue.

**EN**
- A power cut or a freeze during an update no longer leaves your Station half installed: when it restarts, it puts itself right.
- Updates and reinstalling work even while your device's camera or microphone is in use, and no longer leave the Station without its services if something gets in the way.
- KyberNow Admin restarts on its own if it stops on an error.
- Updates also work on a slow connection.
- Stronger security: share and loan codes, device passwords and lending the computer are better protected against repeated guesses, and another account on the computer can no longer attach your Station to its own account.
- Your settings and lists (devices, accounts, friends) are never wiped when a file is damaged: the last readable version comes back.
- Your device's camera on Windows: more stable, and its picture can no longer be read by apps that are not allowed to.
- Your devices' microphone always gives the computer's own microphone back, even with several devices or after a cut.
- VB-CABLE is no longer reinstalled if you did not choose it.
- The KyberNow Station icon starts with the session of every account on the computer.
- KyberNow Admin does less work when nobody is playing.
