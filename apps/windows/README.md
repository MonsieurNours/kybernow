# KyberNow pour Windows

[English](README.en.md) · Français

Le client de streaming de KyberNow pour Windows 10 (22H2) et 11 : votre ordinateur (serveur Kyber 0.29 + console KyberNow) en plein écran sur un PC, avec l'image, le son, le clavier, la souris et les manettes. Cible : un portable ou un ordinateur peu puissant qui se connecte à un plus puissant. Décisions et jalons : ligne « Client Windows » de [PROJECT.md](../../PROJECT.md) ; maquettes : https://claude.ai/artifact/JkUTEMQmxKjcYksaXRHgBf.

**Jalon 2 (l'app)** : l'app s'ouvre sur son accueil (Tauri 2, interface Svelte dans WebView2) : vos ordinateurs, leur ajout, leurs réglages, le passage vers la session et la barre de session. La session elle-même (jalon 1) reste une fenêtre Win32 en Direct3D 11 ; la ligne de commande demeure pour les essais.

C'est le premier client écrit sur le **cœur client en Rust** ([`apps/core`](../core), crate `kybernow-core`), qui pourra servir à Android et Linux ; les apps Apple restent en Swift.

## Ce que fait l'app

### Hors session (interface Svelte, `ui/`)

- **Accueil** : l'ordinateur choisi en plein cadre, son image (aperçu de son écran demandé à la console, une fois par minute au plus, arrivée en balayage à 45°) ou sinon sa **signature** (deux lueurs dont la teinte vient de l'empreinte de sa console), désaturée hors ligne ; son nom, son système, sa carte graphique, son adresse ; l'écran virtuel qui l'attend (« 1920 × 1200 · 60 Hz, comme cet écran ») ; **Se connecter** et le profil (Jeu, Bureau, Max) ; à droite la latence de la dernière session (médiane, 95e centile, durée) sur la règle d'une image ; en bas vos ordinateurs, ceux annoncés sur le réseau (ils s'allument en apparaissant) et l'ajout. En ligne ou non : la console est interrogée toutes les 8 s.
- **Ajouter un ordinateur** : le trouver **sur le réseau** (annonce Bonjour `_kybernow._tcp` de la console, empreinte comprise), **avec le lien** de la console (`kybernow://…`, collé) ou **par adresse** (l'empreinte de la console est alors affichée, les quatre premiers groupes à comparer à ceux de la console) ; puis y avoir accès **avec un code** à six chiffres à accepter dans la console (Accès › Appareils : l'appareil reçoit son propre compte, sans rien taper) ou **avec un compte** et son mot de passe, vérifiés auprès de la console. Pas de QR code à la caméra.
- **Réglages par ordinateur** (enregistrés à chaque changement) : profils, un réglage qui s'en écarte (format, couleurs, débit, transport) les rend « Personnalisé » ; écran de l'ordinateur (comme cet écran, au plus 4K, ou **sur mesure** : définition et fréquence au choix, jusqu'à 7680 × 4320, avec des valeurs courantes), format (ceux que la carte décode), 4:4:4, affichage synchronisé ou immédiat, débit (débit réel affiché) ; souris Auto, Bureau ou Jeu, manette vue par l'ordinateur (Xbox 360 ou fidèle), presse-papiers, son (Auto, stéréo, 5.1, 7.1, aucun), transport, écran virtuel ; carte **Ce PC** (carte graphique, ce qu'elle décode, écran, son) et **sur batterie** (alléger : 30 Hz au plus). Renommer, retirer.
- **Passage** : l'accueil coupé en deux le long de la couture à 45°, un tube néon allumé pendant la préparation (capsule : écran virtuel, connexion, premières images, Annuler), puis, à la première image montrée (pas dès la connexion), la fenêtre de la session se montre en plein écran et l'accueil se cache, toujours ; sans image au bout de 12 s, le passage reste sur « Premières images » avec Annuler et dit que l'ordinateur n'envoie pas d'image (son journal : console, Système › Journaux) (s'il restait devant, il montrait le passage ouvert sur du noir). Si Windows refuse le premier plan à la session (lancement par la barre des tâches, une notification, un lien ou un outil à distance), elle se rattache un instant au fil de premier plan pour le prendre, sinon elle clignote dans la barre des tâches ; une session vers ce PC lui-même est refusée aussi par adresse (une adresse de ses cartes réseau) ; à la fin, il revient, les moitiés se referment et la dernière session s'y inscrit.
- **Barre de session** : fenêtre transparente au-dessus de la session, montrée quand le pointeur touche le haut de l'écran, cachée quand il la quitte (le clavier revient à la session) : symbole allumé, latence et sa courbe, définition, images par seconde, décodeur, profil, souris, plein écran, Quitter.
- **Mots de passe** dans le gestionnaire d'identifiants de Windows (`KyberNow:<id>`), jamais dans un fichier.
- **Pas sur lui-même** : se connecter à ce PC est refusé (l'écran virtuel éteindrait les écrans où l'app s'affiche).
- **Compte d'appareil pas encore connu de Kyber** : la console crée le compte d'un appareil accepté par code, mais Kyber ne le lit qu'à son redémarrage, que la console reporte pendant une session ou un encodage. L'app le dit (« redémarrez le serveur dans la console ») au lieu d'un simple refus.

### Intégration à Windows (jalon 3)

- **Une seule app** : une deuxième ouverture (raccourci, lien, barre des tâches, notification) passe sa demande à l'app déjà ouverte, qui revient devant, puis se ferme (mutex `Local\KyberNow.App`, tube nommé local à l'utilisateur).
- **Lien `kybernow://`** (lien et QR code de la console) : inscrit pour l'utilisateur (HKCU) par l'installateur et à chaque lancement ; il ouvre l'ajout avec l'ordinateur et son empreinte déjà vérifiés.
- **Barre des tâches** : au clic droit de l'icône (et dans le menu Démarrer pour l'app épinglée), vos ordinateurs, un clic lance la session (`kybernow.exe --connect <id>`), et « Ajouter un ordinateur ». La liste suit les ajouts, retraits et changements de nom.
- **Notifications de Windows**, seulement si l'app n'est pas au premier plan : session terminée (par un autre appareil, la console ou une erreur), avec « Reprendre la main » qui relance la session ; nouvelle version prête. Identité des notifications inscrite dans HKCU (`AppUserModelId\LesLignesClaires.KyberNow`, icône `kybernow.png`).
- **Réglages de l'app** (roue de la barre de titre) : version, versions proposées (stables, ou aussi les bêtas), recherche et installation d'une mise à jour, ses nouveautés. Une pastille sur la roue et un bandeau dans la barre de titre disent qu'une version attend.

### Installation et mises à jour

- **Installateur** `KyberNow-Client-Setup.exe` (`setup/`), dans le style de celui de l'ordinateur (`KyberNow-Server-Setup.exe`) : fenêtre sans bordure en Rust, WebView2 et Svelte, colonne au symbole « now » dont la couture s'allume avec les étapes (Bienvenue, Installation, C'est prêt). Un seul fichier pour **x64 et ARM64** : il porte les deux versions de l'app et pose celle du processeur (il tourne émulé sur un PC ARM et y installe la version ARM64 ; une app x64 émulée le note dans son journal). Pour l'utilisateur seul, sans droits d'administrateur, dans `%LOCALAPPDATA%\Programs\KyberNow`.
  - **Bienvenue** : ce qu'il voit du PC sans rien changer (Windows, processeur, carte graphique, écran, réseau, WebView2, version déjà installée, app ouverte), option « Raccourci sur le Bureau ». Une version installée fait de l'installation une mise à jour (« Mettre à jour »).
  - **Installation** : ferme l'app si elle est ouverte ; pose les fichiers à côté (`KyberNow.new`) puis les échange d'un coup avec l'ancienne version (une mise à jour qui échoue la laisse intacte ; si l'ancien dossier est verrouillé, copie par-dessus) ; installe WebView2 s'il manque (installateur officiel de Microsoft, pour l'utilisateur) ; lien `kybernow://`, notifications et raccourcis posés par l'app elle-même (`kybernow.exe --setup-shell [--desktop]`) ; « Applications installées » (HKCU).
  - **C'est prêt** : « Ouvrir KyberNow », et les raccourcis de la session.
  - `/S` : silencieux ; `/S /UPDATE` : mise à jour lancée par l'app (attend sa fermeture, la relance avec `--updated`, et la rouvre avec un message si l'installation échoue). L'installateur ne garde jamais le dossier de l'app comme dossier courant (Windows refuserait de le renommer) ; si le dossier reste verrouillé, les fichiers sont échangés un par un, avec retour arrière. Pendant l'installation il tient `Local\KyberNow.Setup` : l'app rouverte entre-temps attend la fin (2 minutes au plus). Programmes de Windows appelés par leur chemin complet (`System32`), pas depuis le dossier de l'installateur. Le pied se lit aussi après une future signature Authenticode (table des certificats de l'en-tête PE). ARM64 : Windows 11 (Windows 10 sur ARM n'émule pas le x64 du lanceur). Journaux : `%LOCALAPPDATA%\KyberNow\setup.log` (lanceur) et `setup-ui.log` (interface).
  - Pas de signature Authenticode pendant la bêta : Windows (SmartScreen) avertit au premier lancement.
- **Désinstallation** (« Applications installées », `uninstall.exe --uninstall`, le lanceur sans son contenu) : ferme l'app, vide la liste de la barre des tâches, propose d'effacer aussi les ordinateurs, réglages et mots de passe (`kybernow.exe --forget-all`), retire raccourcis et inscriptions, puis le dossier. Les options internes de l'installateur écrivent dans `kybernow-setup.log`, pas dans le journal de l'app.
- **Mises à jour**, comme la console (`server/admin/src/update`) : l'app cherche au lancement puis toutes les 6 heures la dernière version publiée sur GitHub, sur son canal ; rien n'est proposé sans le manifeste signé de la version (`kybernow-release.json`, minisign, même clé que la console) qui doit annoncer la même version et donner l'installateur. L'installateur téléchargé (`KyberNow-Client-Setup.exe`, le même pour les deux architectures) doit avoir exactement la taille et l'empreinte SHA-256 du manifeste. « Installer et relancer » le lance en silence (`/S /UPDATE`) et ferme l'app ; il attend sa fermeture, remplace ses fichiers (pas les données) et la relance (`--updated`, « KyberNow X est installée »). Pas pendant une session.

### Session

- **Session** (cœur, comme `SessionController.swift`) : préparation par la console (`POST /api/stream/prepare` : écran virtuel à la définition et à la fréquence de l'écran de ce PC, sortie son virtuelle au nombre de voies de la sortie de ce PC), connexion à Kyber avec le même compte, certificat de Kyber exigé tel que la console le donne (`kyber_fingerprint`), choix de l'écran (attend 6 s au plus que Kyber n'annonce plus que l'écran virtuel), signal de vie toutes les 5 s avec la latence (la console peut y demander la fin de la session), premier plan de l'ordinateur interrogé chaque seconde (souris Auto), fin propre (`POST /api/stream/end`, latence de la session). Reprise : reconnexion automatique de Kyber, écran redemandé à la console si Kyber a redémarré, image relancée (3 fois au plus) après une erreur de la chaîne d'image, image clé redemandée dès qu'une image de référence manque (toutes les 300 ms tant qu'elle n'est pas venue). Si le paquet qui annonce le format du flux se perd (transport rapide, avec pertes), le décodeur du format demandé pour la session s'ouvre d'office à la première image : avant, toutes les images étaient jetées sans bruit et l'écran restait noir. Le journal note le format annoncé, les paramètres, les premières données et la première image clé. Comme sur Mac, une image clé ne se demande qu'après une perte, jamais avant les premières données (l'encodeur commence par une), et un flux sans image 4 s après son démarrage est relancé (ce qui relance aussi le service vidéo de Kyber sur l'ordinateur), trois fois au plus avant d'arrêter la session avec la raison.
- **Console** : compte Basic, en-tête `x-kyber-admin`, certificat épinglé par son empreinte (`sha256:aa:bb:…`, SHA-256 du certificat). Sans empreinte donnée (`--fingerprint`), la première vue est affichée et retenue dans `%APPDATA%\KyberNow\consoles.txt` ; un certificat différent ensuite arrête la connexion.
- **Profils** Jeu, Bureau et Max, ramenés à ce que la carte graphique décode, et **débit** selon les pixels par seconde : mêmes règles et mêmes valeurs que l'app Mac (testées dans `apps/core`). Une option de format, de couleurs ou de débit rend le profil « Personnalisé ».
- **Image** : FFmpeg (celui de Kyber, lié en statique) décode par la carte graphique (**D3D11VA**, sans copie hors de la carte ; l'app donne à FFmpeg l'appareil Direct3D et ses interfaces vidéo, que FFmpeg 8.1 ne va pas chercher lui-même : sans elles il plantait à la création du décodeur ; sans interfaces vidéo, comme dans une machine virtuelle, décodage par le processeur) ; au lancement, l'app décode pour de bon une petite image de chaque format (H.264, HEVC, HEVC 4:4:4, AV1, dans `src/probe/`) pour savoir ce que la carte sait faire. Si la carte ne décode pas un format, ou le décode dans un format de texture qu'on ne sait pas montrer, l'app décode par le **processeur** (fils par tranches seulement : les fils par image ajoutent une image de retard chacun). Rendu Direct3D 11 : un shader convertit le YUV (NV12, AYUV 4:4:4 ou trois plans) en RVB selon l'espace et la plage de couleurs du flux, à la taille de la fenêtre en gardant les proportions ; chaîne d'échange « flip », une image en attente au plus ; chaque paquet est décodé mais seule l'image la plus récente est montrée (si d'autres attendent, celle-ci est sautée : le retard ne s'accumule pas derrière la synchro verticale). Synchro verticale par défaut ; `--no-vsync` présente chaque image dès qu'elle est décodée (déchirure permise quand Windows le permet, utile avec un écran G-SYNC ou FreeSync).
- **Son** : libopus (masquage des pertes compris), WASAPI en mode partagé piloté par événement ; Windows convertit au format de la sortie. Réserve de 30 ms, plus de 100 ms en retard : le plus ancien est jeté. Voies remises dans l'ordre de Windows en 5.1 et 7.1.
- **Clavier** : les codes de touche de Windows tels quels (jeu 1, `0xE000` pour les touches étendues, comme le client de Kyber), sans les répétitions (l'ordinateur répète lui-même) ; Alt et F10 ne déclenchent pas de menu local. Les raccourcis de Windows (touche Windows, Alt+Tab) restent à ce PC pour l'instant (jalon 4).
- **Souris** : position absolue ramenée aux pixels de l'ordinateur, boutons (dont les deux latéraux), molette. Souris des jeux (relative, déplacements bruts par Raw Input, pointeur caché et retenu dans la fenêtre) : automatique quand la console voit un jeu en plein écran et que l'ordinateur cache son pointeur, ou forcée par Ctrl+Alt+Maj+M.
- **Pointeur** : celui de l'ordinateur (forme et point chaud) devient le pointeur de Windows au-dessus de l'image ; une forme entièrement transparente garde la précédente.
- **Manettes et presse-papiers** : par la configuration client de kynput sous Windows (SDL pour les manettes, vues par l'ordinateur comme des Xbox 360 ; presse-papiers texte dans les deux sens).
- **Paquets refusés** : la file d'envoi de kynput tient 64 paquets et refuse sans retour quand elle est pleine ; touches et boutons refusés sont réessayés toutes les 3 ms, dans l'ordre, pendant 200 ms au plus (comme `PacketQueue.swift`), positions, déplacements et molette partent une fois. Le total des refus va au journal.
- **Rien ne reste enfoncé** : quand la fenêtre perd le clavier, l'app relâche sur l'ordinateur les touches et boutons qu'elle y tient ; en fin de session, tout est relâché.
- **Plantages** : notés dans le journal (paniques avec la pile, exceptions natives avec le module et le décalage) ; le journal du lancement précédent est gardé (`kybernow.prev.log`). La sonde de la carte graphique tourne dans un processus à part (`kybernow --probe-json`, journal `kybernow-probe.log`) : s'il plante, l'app décode par le processeur. Les exécutables non allégés de chaque version donnent la ligne en cause (`llvm-addr2line -e kybernow.exe 0x140000000+décalage`).
- **Mesures** : le titre de la fenêtre donne la latence de bout en bout (capture sur l'ordinateur → présentation ici, sur l'horloge de Kyber recalée), la définition, le décodeur et les images par seconde ; la médiane et le 95e centile de la session vont au journal et à la console.

| Raccourci | Effet |
|---|---|
| Ctrl+Alt+Maj+D | Se déconnecter (aussi : fermer la fenêtre) |
| Ctrl+Alt+Maj+F | Plein écran ou fenêtre |
| Ctrl+Alt+Maj+M | Souris des jeux ou du bureau |

## Utilisation

`kybernow.exe` sans argument ouvre l'app. En ligne de commande, une session directe (essais, dépannage) :

```
kybernow <adresse>[:port] [--user <compte>] [--profile jeu|bureau|max] [options]
```

- `<adresse>:port` : l'ordinateur et le port de sa console (Accès, dans la console) ; 9443 par défaut.
- Mot de passe : `--password`, la variable `KYBERNOW_PASSWORD`, sinon demandé.
- `--codec h264|hevc|av1`, `--444` / `--420`, `--mbps <débit>`, `--size 1920x1080@60` (sinon l'écran principal de ce PC), `--window`, `--no-vsync`, `--no-hw` (décodage par le processeur), `--no-audio` (son laissé sur l'ordinateur), `--physical` (écrans de l'ordinateur tels quels).
- `--probe` : affiche ce que la carte graphique décode, l'écran, la sortie son et le système, puis quitte. Ne demande pas d'ordinateur.

## Organisation du code

| Dossier | Contenu |
|---|---|
| `apps/Cargo.toml` | Espace de travail Rust des clients (`core`, `windows`) ; renvoie les crates de Kyber vers `third_party/kyber-desktop/kysdk` |
| `apps/core/src/console.rs` | Client de la console (`info`, `prepare`, `heartbeat`, `foreground`, `end`), certificat épinglé |
| `apps/core/src/session.rs` | Déroulé d'une session sur un fil dédié ; ce que la plateforme fournit : `Platform` |
| `apps/core/src/media.rs` | Paquets d'image et de son livrés par Kyber (correctif 0001), en-tête Opus |
| `apps/core/src/profile.rs` | Profils, format, débit selon les pixels par seconde |
| `apps/core/src/latency.rs` | Latence de bout en bout, horloge de Kyber |
| `apps/windows/src/main.rs` | App ou ligne de commande |
| `apps/windows/src/ui.rs` | App Tauri : fenêtres, commandes de l'interface, sondage des ordinateurs, ce PC |
| `apps/windows/src/store.rs` | Ordinateurs et réglages (`computers.json`), mots de passe (gestionnaire d'identifiants) |
| `apps/windows/src/discovery.rs` | Ordinateurs annoncés sur le réseau (mDNS) |
| `apps/windows/src/host.rs` | Une session sur son fil : fenêtre, image, son, entrées, nouvelles pour l'app |
| `apps/windows/src/plan.rs` | Réglages d'une session (profil, retouches, écran, batterie) |
| `apps/windows/src/cli.rs` | Ligne de commande |
| `apps/windows/ui/` | Interface Svelte : accueil, ajout, réglages, passage, barre ; `src/lib/mock.ts` imite l'app dans un navigateur (`npm run dev`, développement seulement) |
| `apps/windows/src/window.rs` | Fenêtre Win32, clavier, souris, pointeur, raccourcis, plein écran |
| `apps/windows/src/video.rs` | Fil de l'image : paquets → décodeur → rendu, images clés |
| `apps/windows/src/decoder.rs` | FFmpeg (D3D11VA ou processeur), sonde des capacités |
| `apps/windows/src/render.rs` | Direct3D 11 : textures, shader YUV → RVB, chaîne d'échange |
| `apps/windows/src/audio.rs` | libopus et WASAPI |
| `apps/windows/src/input.rs` | Chaîne kynput : clavier, souris, pointeur de l'ordinateur |
| `apps/windows/src/system.rs` | Journal, écran, identité de l'appareil, consoles connues |

Le cœur ne dépend d'aucun système : une autre plateforme fournit sa fenêtre, ses décodeurs, son son et ses entrées par `Platform`.

## Construire

Tout se compile depuis le Mac :

```bash
apps/windows/scripts/build.sh                # x64 (x86_64-pc-windows-gnu, MinGW de Homebrew, comme le serveur)
apps/windows/scripts/build.sh --arch arm64   # ARM64, PC Snapdragon (aarch64-pc-windows-gnullvm, llvm-mingw)
```

Puis l'installateur, une fois les deux dossiers construits : `apps/windows/scripts/build-installer.sh` écrit `apps/windows/dist/KyberNow-Client-Setup.exe` (`[lanceur][x64.tar.zst][arm64.tar.zst][pied de 64 o « KNCLNT01 »]`, voir `setup/src/payload.rs`). Le projet `setup/` a son propre `Cargo.lock`, aux versions de l'installateur du serveur (`wry`, `tao`, `zstd`), et se construit pour x64 seulement. Son interface (`setup/ui`) se regarde dans un navigateur avec une simulation : `npm --prefix apps/windows/setup/ui run dev`, et `?maj`, `?ouverte`, `?arm`, `?echec` dans l'adresse. `scripts/release.sh` construit tout et joint l'installateur à chaque version.

Le dossier à copier sur le PC est `apps/windows/dist/x64/` (ou `arm64/`), aussi en archive `dist/KyberNow-<version>-windows-<arch>.zip` : `kybernow.exe`, `kynput.dll` et `SDL2.dll` (manettes), plus `libunwind.dll` en ARM64. Le reste vient de Windows 10 et 11 (runtime C universel, Direct3D 11, `d3dcompiler_47.dll`).

- **Prérequis x64** : `third_party/` en place, et `server/kyber-patches/build.sh` passé une fois (il produit `third_party/rootfs-win64`, avec FFmpeg et libopus, et `third_party/rootfs-win64-kynput`, avec `kynput.dll`).
- **Prérequis ARM64** : llvm-mingw dans `third_party/toolchains`, la cible Rust `aarch64-pc-windows-gnullvm`, puis `apps/windows/scripts/build-arm64-deps.sh` une fois : libopus, un FFmpeg réduit au décodage (H.264, HEVC, AV1, D3D11VA), SDL2 (sources vérifiées de kynput) et `kynput.dll`, dans `third_party/rootfs-winarm64`. Versions des outils : [docs/build-notes.md](../../docs/build-notes.md).
- **Correctifs de Kyber** : les mêmes que pour les apps Apple (`apps/macos/kyber-patches`), appliqués par le script s'ils ne le sont pas.
- **VLC** : kyclient le lie toujours sous Windows, mais l'app décode elle-même : le script remplace chaque fonction de `libvlc.dll` par une fonction vide (comme sur l'Apple TV) et refuse un exécutable qui dépendrait encore de `libvlc.dll`.
- `--debug` : sans optimisation.
- **Tests du cœur** (sur le Mac) :

```bash
cd apps && PKG_CONFIG_PATH=$PWD/../third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib cargo test -p kybernow-core
```

## Tester

L'interface se travaille dans un navigateur sur le Mac, avec une imitation de l'app : `npm --prefix apps/windows/ui run dev` (ou la configuration `windows-ui` de `.claude/launch.json`).

Copier `dist/x64` sur le PC (ou `dist/arm64` sur un PC Snapdragon, ou dans Windows 11 ARM en machine virtuelle sur le Mac : elle dit si l'app démarre et se connecte, pas la latence ni le décodage par la carte graphique), puis lancer `kybernow.exe` (l'app), ou dans un terminal :

```
kybernow.exe --probe
kybernow.exe 192.168.1.100:9443 --user <compte> --profile bureau
```

L'interface demande le runtime WebView2 d'Edge, présent sur Windows 11 et sur la plupart des Windows 10 (l'installateur du jalon 3 le posera au besoin).

Au premier contact, l'empreinte de la console s'affiche : la comparer à celle de la console (Accès). Le journal est dans `%LOCALAPPDATA%\KyberNow\kybernow.log`.

## Données

| Fichier | Contenu |
|---|---|
| `%APPDATA%\KyberNow\computers.json` | Ordinateurs de l'app, leurs réglages et leur dernière session |
| Gestionnaire d'identifiants, `KyberNow:<id>` | Mot de passe de chaque ordinateur |
| `%APPDATA%\KyberNow\device-id` | Identifiant de cet appareil pour la console (tiré une fois) |
| `%APPDATA%\KyberNow\consoles.txt` | Empreintes des consoles retenues par la ligne de commande (`hôte:port sha256:…`) |
| `%LOCALAPPDATA%\KyberNow\kybernow.log` | Journal de la dernière session |

## Pas encore

- **Jalon 2, reste** : image de la dernière session sur l'accueil (aujourd'hui l'aperçu de la console), changement de profil depuis la barre.
- **Jalon 3, reste** : signature Authenticode (reportée, SignPath visé).
- **Jalon 4** : raccourcis de Windows envoyés à l'ordinateur, menu Touches, pointeur dessiné en mode jeu, mise à l'échelle du pointeur, écran redemandé quand le signal de vie le dit absent.
