# Installateur KyberNow

[English](README.en.md) · Français

`KyberNow-Setup.exe` : un seul fichier qui installe le serveur Kyber 0.29, la console d'administration, l'icône de la zone de notification, le pilote d'écran virtuel SudoVDA et le pilote de manettes ViGEmBus.

## Architecture

```
KyberNow-Setup.exe = [lanceur][contenu tar.zst][pied 32 o « KNPAYLD1 »]
```

- **Lanceur** (`src/bin/setup.rs`) : sans WebView2, droits administrateur. Il dépose l'interface et `WebView2Loader.dll` dans `C:\Program Files\KyberNow\.setup` (effacé ensuite), puis lance l'interface. Pas dans `%TEMP%` : l'interface y tournerait en administrateur à côté de fichiers que tout programme de l'utilisateur peut déposer (DLL chargée à sa place, contournement de l'UAC). Avec `--uninstall`, il désinstalle (c'est aussi `uninstall.exe` une fois installé).
- **Interface** (`src/bin/ui.rs` + `ui/`) : fenêtre sans bordure (tao + wry/WebView2) qui sert l'app Svelte via le protocole `kn://`. Messages JSON : `window.ipc.postMessage` dans un sens, `window.__knReceive` dans l'autre (`src/events.rs`). Identité v2 (symbole « server », Archivo et Martian Mono embarquées), quatre étapes dans la colonne de gauche, dont la couture se remplit au fil du parcours : **Bienvenue** (l'analyse de l'ordinateur tourne déjà à côté, ports modifiables ; rien ne change avant « Installer »), **Votre compte**, **Options** (écran virtuel, manettes, son, démarrage avec l'ordinateur, performances), **Installation** puis « Terminé » (adresse, QR code de la console, `/ecran` dans un navigateur, redémarrage pour les manettes). Version affichée : celle de `Cargo.toml`.
- **Cœur** (`src/`) : `checks` (diagnostic ; carte graphique et encodeur, voir plus bas), `ports` (tirage au hasard, plages réservées, TCP + UDP), `install` (tâches), `uninstall`, `payload` (format du contenu).

## Ce que fait l'installation

1. Arrête et retire une version précédente ou Kyber officiel (désinstalleur NSIS lancé avec `_?=` pour l'attendre).
2. Extrait le contenu dans `C:\Program Files\KyberNow\` (`kyber\`, `admin\`, `tray\`, `drivers\`, `licenses\`). Dans `kyber\`, les fichiers de Kyber 0.29 recouverts par la surcouche corrigée (`kyavserver.exe`, txproto et leur runtime, `kycontroller.exe`, voir [docs/kyber-changes.md](../../docs/kyber-changes.md)). Aussitôt après : `kybernow.json`, `uninstall.exe` et l'inscription dans « Applications installées », pour qu'une installation interrompue plus loin se désinstalle normalement.
3. Génère le certificat TLS de Kyber (`cert-gen.exe`).
4. `kyber-admin init` : port, encodeur (voir « Choix de l'encodeur »), compte unique (Kyber Basic + console, mémorisé comme compte d'installation), secret JWT aléatoire à la place de la clé de développement de Kyber, écrit dans `C:\ProgramData\KyberNow\kyber-jwt.key` (`kyber_config.toml` n'en porte que le chemin). Juste avant, et de nouveau après : `kyber\kyber_config.toml`, le certificat et la clé de Kyber (`kybertest_cert.pem`, `kybertest_key.pem`) sont réservés à SYSTEM et aux Administrateurs (`src/acl.rs`, `secure_file`, descripteur `O:BAD:PAI(A;;FA;;;SY)(A;;FA;;;BA)`), aussi avec « Conserver mes réglages » : Program Files est lisible par tous et ces fichiers contiennent les empreintes des mots de passe des comptes. La console repose ces droits à chaque démarrage.
5. Crée le service `kyber` (chemin entre guillemets, redémarrage automatique en cas d'échec) et ouvre son démarrage et son arrêt aux utilisateurs interactifs (`sc sdset`, voir plus bas).
6. Pare-feu : ports du stream (TCP/UDP) et de la console (TCP), annonce Bonjour de la console (UDP 5353, pour que les apps trouvent l'ordinateur), sur les profils privé et domaine ; réseau « Public » passé en « Privé ».
7. SudoVDA : certificat dans Racine + Éditeurs approuvés, nœud de périphérique et pilote via `nefconc`.
8. ViGEmBus, s'il n'est pas déjà présent (`/quiet /norestart`).
8. bis. Son à distance (option) : haut-parleurs virtuels de Steam (`SteamStreamingSpeakers.inf` via `pnputil /add-driver /install`) si Steam est installé. La console y bascule la sortie de Windows pendant les sessions : le son quitte les haut-parleurs du PC et part au bon nombre de voies.
9. Service `kyber-admin` (console, mêmes droits que `kyber`), mode d'alimentation Performances élevées (option décochée d'office si l'ordinateur y est déjà ; recréé par `powercfg /duplicatescheme` s'il manque, comme sur beaucoup de portables ; si Windows le refuse, l'étape passe en avertissement et l'installation continue), raccourcis de la console et de l'icône dans le menu Démarrer.
10. Icône de la zone de notification : lancée à la fin de l'installation, et à chaque ouverture de session si « Démarrer avec Windows » est coché (valeur `KyberNow` de `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). Sans cette option, les deux services passent en démarrage manuel.
11. Démarrage vérifié : l'installateur attend que le service `kyber` soit en marche (`sc query`, état 4) puis que le serveur et la console acceptent une connexion sur leurs ports ; sinon une ligne « ATTENTION » le dit dans le journal.

## Réinstaller par-dessus KyberNow

L'analyse lit `C:\Program Files\KyberNow\kybernow.json` : si KyberNow est déjà installé, l'étape « Votre compte » commence par un choix.

- **Conserver mes réglages** (recommandé, proposé si `kyber\kyber_config.toml` et `C:\ProgramData\KyberNow\kyber-admin.toml` existent) : seuls les programmes, les pilotes et la surcouche de Kyber sont remplacés. `kyber_config.toml` n'est pas extrait du contenu (une copie part d'abord dans `C:\ProgramData\KyberNow\backups\kyber_config.toml.reinstallation-<date>`), le certificat de Kyber (`kybertest_cert.pem`, `kybertest_key.pem`) n'est pas régénéré, `kyber-admin init` n'est pas lancé : comptes Kyber, secret JWT, encodeur, mot de passe de la console, `devices.json` et `events.json` restent. Les ports sont ceux en service (`[kycontroller] port` de `kyber_config.toml`, `listen` de `kyber-admin.toml`, sinon `kybernow.json`), et le pare-feu est refait avec eux. L'écran « Votre compte » est sauté.
- **Repartir de zéro** : comme une première installation (nouveau compte, nouveaux ports tirés au hasard, nouveau certificat, `kyber_config.toml` d'origine), avec un avertissement : tous les appareils doivent être appairés de nouveau.

Une version de Kyber officielle (sans `kybernow.json`) est toujours retirée puis remplacée.

Pendant l'installation, la fenêtre ne se ferme pas : le bouton est grisé et Alt+F4 est ignoré.

Journal : `C:\Program Files\KyberNow\install.log` (et `%TEMP%\KyberNow-install.log`).

## Droits de `C:\ProgramData\KyberNow`

Par défaut, ProgramData laisse tout compte y créer des dossiers et en devenir propriétaire. Sur un ordinateur partagé, un compte standard pourrait donc déposer un fichier dans `update\` entre la vérification d'un paquet de mise à jour et son extraction par PowerShell, qui tourne en SYSTEM. Le dossier des données est réservé à SYSTEM et aux Administrateurs (contrôle total), sans héritage de ProgramData, et rien n'y est lisible par les autres comptes (il contient l'empreinte du mot de passe de la console, le certificat et le code d'initialisation).

- Posé par l'installateur (étape « files », avant que `kyber-admin init` écrive les réglages) et à chaque démarrage de la console (`server/admin/src/update/acl.rs`, même code que `src/acl.rs`) : parcours de haut en bas sans suivre aucun lien, chaque dossier verrouillé avant d'être lu (un autre compte ne peut plus rien y créer pendant la pose), propriétaire Administrateurs partout, droits explicites remplacés, jonctions, liens symboliques et liens physiques supprimés (le lien, pas sa cible). Poignées ouvertes en mode sauvegarde avec les privilèges de sauvegarde, de restauration et de prise de possession : un fichier qu'un autre compte aurait protégé n'arrête pas la pose. Descripteur du dossier : `O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)` (le contenu en hérite).
- `update\`, `lancement.ps1`, `relaunch-tray`, `kyber-jwt.key` et `local-token` sont créés ensuite par la console ou par `update.ps1` : ils en héritent.
- Un échec est écrit dans `install.log` ou dans `kyber-admin.log` sans bloquer l'installation ni la console.
- L'icône, qui tourne avec les droits de l'utilisateur, ne lit plus rien dans ce dossier : le port de la console vient de `C:\Program Files\KyberNow\kybernow.json` (lisible par tous, sans secret), et le code d'initialisation lui est transmis par sa partie élevée (voir « Réinitialiser le mot de passe de la console »).

Vérifier sur l'ordinateur : `icacls C:\ProgramData\KyberNow` doit lister `NT AUTHORITY\SYSTEM:(OI)(CI)(F)` et `BUILTIN\Administrateurs:(OI)(CI)(F)`, rien d'autre ; `icacls "C:\Program Files\KyberNow\kyber\kyber_config.toml"` (et les deux `kybertest_*.pem`) doit lister SYSTEM et Administrateurs en `(F)`, sans `BUILTIN\Utilisateurs`, et `type` depuis un compte standard doit être refusé.

## Désinstaller

« Applications installées » › KyberNow › Désinstaller lance `uninstall.exe --uninstall` (le lanceur seul, sans le contenu). Après confirmation, il demande s'il faut garder les réglages, puis :

- arrête et supprime les services `kyber` et `kyber-admin`, arrête l'icône et retire sa valeur Run (compte qui désinstalle), retire les règles de pare-feu ;
- SudoVDA : nœud de périphérique, paquet du magasin de pilotes (`pnputil /delete-driver oemNN.inf /uninstall /force`, `oemNN.inf` trouvé par son nom d'origine `sudovda.inf` dans `pnputil /enum-drivers`), certificat des magasins Racine et Éditeurs approuvés ;
- remet le mode d'alimentation d'avant KyberNow (et supprime le mode Performances élevées s'il l'avait recréé), et repasse en « Public » le réseau qu'il avait passé en « Privé », s'il porte toujours le même nom. Ces réglages d'avant sont notés dans `kybernow.json` (`restore`) à la première installation, et gardés par les suivantes et par les mises à jour ;
- garde `C:\ProgramData\KyberNow` si on l'a demandé (avec une copie de `kyber_config.toml` dans `backups\kyber_config.toml.desinstallation`, car les comptes des appareils y sont), sinon l'efface ;
- affiche la boîte de fin, puis, une fois fermée, efface `C:\Program Files\KyberNow` par une commande différée (le désinstalleur tourne depuis ce dossier).

ViGEmBus reste installé : d'autres logiciels peuvent s'en servir.

## Choix de l'encodeur

L'analyse liste les cartes graphiques de Windows (`Win32_VideoController`, eGPU branchés compris) et ne garde que les cartes PCI NVIDIA (`VEN_10DE`), AMD (`VEN_1002`, `VEN_1022`) et Intel (`VEN_8086`) en état de marche, avec le pilote du fabricant : l'adaptateur de base de Microsoft (`display.inf`, carte sans pilote), les cartes désactivées, les écrans virtuels (SudoVDA) et le bureau à distance sont écartés. Elle prend ensuite la plus performante : NVIDIA → `nvenc`, sinon AMD → `amf`, sinon Intel → `qsv`, sinon le processeur → `x264` (avertissement « Aucune carte graphique compatible »). Sur un portable hybride (Intel intégré + NVIDIA), c'est donc NVENC. Si Windows ne répond pas, NVML sert de repli pour les cartes NVIDIA. Le choix est écrit dans `[kyavserver] encoder` par `kyber-admin init` et se change ensuite dans la console (Système › Avancé). La mise à jour (`update.ps1`) ne remplace que les programmes, jamais `kyber_config.toml` : l'encodeur choisi est conservé. L'installateur aussi quand on réinstalle en choisissant « Conserver mes réglages » ; « Repartir de zéro » refait le choix.

Le contrôle « Pilote graphique » affiche la version du pilote de cette carte (pour NVIDIA, au format du fabricant : 32.0.15.6094 devient 560.94).

## Icône de la zone de notification (`server/tray`)

`kybernow-tray.exe` tourne dans la session de l'utilisateur, sans droits administrateur. Symbole « server » de l'identité v2 (`resources/kybernow.ico`, régénéré par `resources/make-ico.py` : le symbole seul jusqu'à 32 px, la tuile de l'app au-delà), avec une pastille : verte en marche, Spectre pendant une session, orange pendant un démarrage ou un arrêt ; grisé serveur arrêté.

- **Clic gauche : le panneau** (`src/panel.rs`, `src/panel.html`). Une fenêtre sans bordure (coins arrondis de Windows 11, ombre) posée contre l'icône, du côté de la barre des tâches, qui se ferme dès qu'elle perd le focus ou avec Échap. Vue WebView2 (wry, boucle tao), données dans `%LOCALAPPDATA%\KyberNow\tray-webview`. En haut, l'état : en marche (nom de l'ordinateur, dernière session), en direct (appareil, depuis quand, latence), démarrage, arrêté. Puis : « Mettre fin à la session » (`POST /api/local/end`), « Ouvrir la console », « Installer KyberNow x.y » si une version est publiée, « Redémarrer le serveur » (`kyber` et `kyber-admin`), « Arrêter / Démarrer le serveur » (service `kyber`, la console reste en marche), « Démarrer avec l'ordinateur », « Réinitialiser le mot de passe de la console… », et « Fermer l'icône », qui propose d'arrêter aussi le serveur.
- **Clic droit : le menu de Windows**, avec les mêmes actions. Sans WebView2, le menu s'ouvre aussi au clic gauche. `WebView2Loader.dll` est installé à côté de l'icône (`tray\`), par l'installateur et par la mise à jour.
- **Réinitialiser le mot de passe de la console.** L'icône se relance en administrateur (`--reset-console-password`, invite UAC) : elle arrête la console, lance `kyber-admin.exe reset-password`, la redémarre. La console écrit alors un code d'initialisation (`C:\ProgramData\KyberNow\setup-code.txt`, lisible en administrateur seulement) ; la partie élevée de l'icône l'y lit et en laisse une copie dans le profil du compte (`%LOCALAPPDATA%\KyberNow\setup-code.txt`), que l'icône ordinaire lit puis efface. Le panneau l'affiche et le copie, jusqu'à ce que la console dise avoir un mot de passe (`GET /api/session`), et la console s'ouvre sur la première visite. Avec un autre compte administrateur saisi dans l'invite UAC, la copie part dans le profil de cet autre compte : l'icône renvoie alors au fichier de ProgramData. Les appareils et leurs comptes ne changent pas.

« Ouvrir la console » : `https://localhost:<port>`, port lu dans `C:\Program Files\KyberNow\kybernow.json` (`console_port`, écrit par l'installateur ; la console ne change pas de port en dehors d'une installation). Une seconde ouverture (menu Démarrer) ouvre la console.

- **Droits des services.** L'installateur applique aux deux services le SDDL `SERVICE_SDDL` (`src/install.rs`) : celui de Windows par défaut, plus `RP` (démarrer) et `WP` (arrêter) pour `IU` (utilisateurs interactifs). `DT` (suspendre) ne sert pas et n'est plus donné (0.2.6). Même chaîne dans `update.ps1`. La modification de la configuration (`DC`) reste réservée aux administrateurs : donner `DC` permettrait de changer l'exécutable d'un service qui tourne en SYSTEM.
- **Démarrer avec l'ordinateur.** La case suit le type de démarrage du service `kyber`. La basculer change celui des deux services (automatique ou manuel) et la valeur Run de l'utilisateur. Changer le type de démarrage demande l'élévation : l'icône se relance alors en administrateur (`--start-with-computer on|off`), avec une invite UAC ponctuelle.
- **Session en cours.** La console répond à `GET /api/local/state` et `POST /api/local/end` sans compte, mais seulement depuis l'ordinateur lui-même (`127.0.0.1`) et avec le **jeton local** : tiré par la console à son démarrage et à chaque ouverture de session Windows, écrit dans `C:\ProgramData\KyberNow\local-token` (réservé à SYSTEM et aux Administrateurs), copié par `kyber-admin display-agent local-token` dans le profil du compte de la session (`%LOCALAPPDATA%\KyberNow\local-token`, 64 caractères hexadécimaux, lisible par ce compte et les administrateurs), relu par l'icône à chaque appel et envoyé dans l'en-tête `x-kyber-local` (voir [server/README.md](../README.md), « Routes locales »). Sans lui, `403` : l'icône affiche « La console ne répond pas » jusqu'à la copie suivante (quelques secondes après une ouverture de session ou un démarrage de la console). L'état donne le service `kyber`, la session d'un appareil (écran ou son virtuels ouverts par l'app), l'identifiant Kyber, l'appareil connecté et sa latence, la dernière session, le nom et l'adresse de l'ordinateur, et les demandes d'accès sans leur code : le panneau propose « Vérifier dans la console » (qui ouvre Accès, où le code se compare et s'accepte) et « Refuser ».
- **Désinstallation.** Le processus est arrêté et la valeur Run retirée (pour l'utilisateur qui désinstalle).

Compiler seule : `cd server/tray && cargo build --release --target x86_64-pc-windows-gnu` (avec l'éditeur de liens de `build.sh`).

## Construire (depuis le Mac)

`build.sh` produit `dist/KyberNow-Setup.exe` ; `package-update.sh` produit ensuite le paquet de mise à jour d'un ordinateur déjà installé (`dist/kybernow-update.zip`, avec `update.ps1` et `version.txt`, plus `dist/SHA256SUMS`). Publication : [docs/releases.md](../../docs/releases.md). `update.ps1` se teste sans Windows avec `pwsh -NoProfile -File tests/update-bench.ps1`.

```bash
server/installer/build.sh      # → server/installer/dist/KyberNow-Setup.exe
```

Prérequis : `brew install mingw-w64 zstd`, `rustup target add x86_64-pc-windows-gnu`, node, et les contenus extraits dans `third_party/payload/` (voir l'en-tête du script).

## Développer l'interface

```bash
npm --prefix server/installer/ui run dev     # navigateur : backend simulé (ui/src/lib/backend.ts)
```

Hors Windows, `kybernow-ui` fonctionne aussi avec un déroulé d'installation simulé.
