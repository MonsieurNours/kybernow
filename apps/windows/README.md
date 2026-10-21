# KyberNow pour Windows

[English](README.en.md) · Français

Le client de streaming de KyberNow pour Windows 10 (22H2) et 11 : votre ordinateur (serveur Kyber 0.29 + console KyberNow) en plein écran sur un PC, avec l'image, le son, le clavier, la souris et les manettes. Cible : un portable ou un ordinateur peu puissant qui se connecte à un plus puissant. Décisions et jalons : ligne « Client Windows » de [PROJECT.md](../../PROJECT.md) ; maquettes : https://claude.ai/artifact/JkUTEMQmxKjcYksaXRHgBf.

**Jalon 2 (l'app)** : l'app s'ouvre sur son accueil (Tauri 2, interface Svelte dans WebView2) : vos ordinateurs, leur ajout, leurs réglages, le passage vers la session et la barre de session. La session elle-même (jalon 1) reste une fenêtre Win32 en Direct3D 11 ; la ligne de commande demeure pour les essais.

C'est le premier client écrit sur le **cœur client en Rust** ([`apps/core`](../core), crate `kybernow-core`), qui pourra servir à Android et Linux ; les apps Apple restent en Swift.

## Ce que fait l'app

### Hors session (interface Svelte, `ui/`)

- **Accueil** : l'ordinateur choisi en plein cadre, son image (aperçu de son écran demandé à la console, une fois par minute au plus, arrivée en balayage à 45°) ou sinon sa **signature** (deux lueurs dont la teinte vient de l'empreinte de sa console), désaturée hors ligne ; son nom, son système, sa carte graphique, son adresse ; l'écran virtuel qui l'attend (« 1920 × 1200 · 60 Hz, comme cet écran ») ; **Se connecter** et le profil (Jeu, Bureau, Max) ; à droite la latence de la dernière session (médiane, 95e centile, durée) sur la règle d'une image ; en bas vos ordinateurs, ceux annoncés sur le réseau (ils s'allument en apparaissant) et l'ajout. En ligne ou non : la console est interrogée toutes les 8 s.
- **Ajouter un ordinateur** : le trouver **sur le réseau** (annonce Bonjour `_kybernow._tcp` de la console, empreinte comprise), **avec le lien** de la console (`kybernow://…`, collé) ou **par adresse** (l'empreinte de la console est alors affichée, les quatre premiers groupes à comparer à ceux de la console) ; puis y avoir accès **avec un code** à six chiffres à accepter dans la console (Accès › Appareils : l'appareil reçoit son propre compte, sans rien taper) ou **avec un compte** et son mot de passe, vérifiés auprès de la console. Pas de QR code à la caméra.
- **Réglages par ordinateur** (enregistrés à chaque changement) : profils, un réglage qui s'en écarte (format, couleurs, débit, transport) les rend « Personnalisé » ; écran (comme cet écran ou au plus 4K), format (ceux que la carte décode), 4:4:4, affichage synchronisé ou immédiat, débit (débit réel affiché) ; souris Auto, Bureau ou Jeu, manette vue par l'ordinateur (Xbox 360 ou fidèle), presse-papiers, son (Auto, stéréo, 5.1, 7.1, aucun), transport, écran virtuel ; carte **Ce PC** (carte graphique, ce qu'elle décode, écran, son) et **sur batterie** (alléger : 30 Hz au plus). Renommer, retirer.
- **Passage** : l'accueil coupé en deux le long de la couture à 45°, un tube néon allumé pendant la préparation (capsule : écran virtuel, connexion, premières images, Annuler), puis la fenêtre de la session se montre en plein écran à la première image et l'accueil se cache ; à la fin, il revient, les moitiés se referment et la dernière session s'y inscrit.
- **Barre de session** : fenêtre transparente au-dessus de la session, montrée quand le pointeur touche le haut de l'écran, cachée quand il la quitte (le clavier revient à la session) : symbole allumé, latence et sa courbe, définition, images par seconde, décodeur, profil, souris, plein écran, Quitter.
- **Mots de passe** dans le gestionnaire d'identifiants de Windows (`KyberNow:<id>`), jamais dans un fichier.

### Session

- **Session** (cœur, comme `SessionController.swift`) : préparation par la console (`POST /api/stream/prepare` : écran virtuel à la définition et à la fréquence de l'écran de ce PC, sortie son virtuelle au nombre de voies de la sortie de ce PC), connexion à Kyber avec le même compte, certificat de Kyber exigé tel que la console le donne (`kyber_fingerprint`), choix de l'écran (attend 6 s au plus que Kyber n'annonce plus que l'écran virtuel), signal de vie toutes les 5 s avec la latence (la console peut y demander la fin de la session), premier plan de l'ordinateur interrogé chaque seconde (souris Auto), fin propre (`POST /api/stream/end`, latence de la session). Reprise : reconnexion automatique de Kyber, écran redemandé à la console si Kyber a redémarré, image relancée (3 fois au plus) après une erreur de la chaîne d'image, image clé redemandée dès qu'une image de référence manque (toutes les 300 ms tant qu'elle n'est pas venue).
- **Console** : compte Basic, en-tête `x-kyber-admin`, certificat épinglé par son empreinte (`sha256:aa:bb:…`, SHA-256 du certificat). Sans empreinte donnée (`--fingerprint`), la première vue est affichée et retenue dans `%APPDATA%\KyberNow\consoles.txt` ; un certificat différent ensuite arrête la connexion.
- **Profils** Jeu, Bureau et Max, ramenés à ce que la carte graphique décode, et **débit** selon les pixels par seconde : mêmes règles et mêmes valeurs que l'app Mac (testées dans `apps/core`). Une option de format, de couleurs ou de débit rend le profil « Personnalisé ».
- **Image** : FFmpeg (celui de Kyber, lié en statique) décode par la carte graphique (**D3D11VA**, sans copie hors de la carte) ; au lancement, l'app décode pour de bon une petite image de chaque format (H.264, HEVC, HEVC 4:4:4, AV1, dans `src/probe/`) pour savoir ce que la carte sait faire. Si la carte ne décode pas un format, ou le décode dans un format de texture qu'on ne sait pas montrer, l'app décode par le **processeur** (fils par tranches seulement : les fils par image ajoutent une image de retard chacun). Rendu Direct3D 11 : un shader convertit le YUV (NV12, AYUV 4:4:4 ou trois plans) en RVB selon l'espace et la plage de couleurs du flux, à la taille de la fenêtre en gardant les proportions ; chaîne d'échange « flip », une image en attente au plus ; chaque paquet est décodé mais seule l'image la plus récente est montrée (si d'autres attendent, celle-ci est sautée : le retard ne s'accumule pas derrière la synchro verticale). Synchro verticale par défaut ; `--no-vsync` présente chaque image dès qu'elle est décodée (déchirure permise quand Windows le permet, utile avec un écran G-SYNC ou FreeSync).
- **Son** : libopus (masquage des pertes compris), WASAPI en mode partagé piloté par événement ; Windows convertit au format de la sortie. Réserve de 30 ms, plus de 100 ms en retard : le plus ancien est jeté. Voies remises dans l'ordre de Windows en 5.1 et 7.1.
- **Clavier** : les codes de touche de Windows tels quels (jeu 1, `0xE000` pour les touches étendues, comme le client de Kyber), sans les répétitions (l'ordinateur répète lui-même) ; Alt et F10 ne déclenchent pas de menu local. Les raccourcis de Windows (touche Windows, Alt+Tab) restent à ce PC pour l'instant (jalon 4).
- **Souris** : position absolue ramenée aux pixels de l'ordinateur, boutons (dont les deux latéraux), molette. Souris des jeux (relative, déplacements bruts par Raw Input, pointeur caché et retenu dans la fenêtre) : automatique quand la console voit un jeu en plein écran et que l'ordinateur cache son pointeur, ou forcée par Ctrl+Alt+Maj+M.
- **Pointeur** : celui de l'ordinateur (forme et point chaud) devient le pointeur de Windows au-dessus de l'image ; une forme entièrement transparente garde la précédente.
- **Manettes et presse-papiers** : par la configuration client de kynput sous Windows (SDL pour les manettes, vues par l'ordinateur comme des Xbox 360 ; presse-papiers texte dans les deux sens).
- **Paquets refusés** : la file d'envoi de kynput tient 64 paquets et refuse sans retour quand elle est pleine ; touches et boutons refusés sont réessayés toutes les 3 ms, dans l'ordre, pendant 200 ms au plus (comme `PacketQueue.swift`), positions, déplacements et molette partent une fois. Le total des refus va au journal.
- **Rien ne reste enfoncé** : quand la fenêtre perd le clavier, l'app relâche sur l'ordinateur les touches et boutons qu'elle y tient ; en fin de session, tout est relâché.
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
- **Jalon 3** : intégration à Windows (ordinateurs au clic droit de la barre des tâches, notifications, lien `kybernow://`), installateur par utilisateur, mises à jour.
- **Jalon 4** : raccourcis de Windows envoyés à l'ordinateur, réglage sur batterie, menu Touches, pointeur dessiné en mode jeu, mise à l'échelle du pointeur, écran redemandé quand le signal de vie le dit absent.
