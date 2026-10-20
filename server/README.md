# Serveur KyberNow (Windows)

[English](README.en.md) · Français

Tout ce qui tourne sur l'ordinateur partagé : le serveur Kyber 0.29 (corrigé, voir [docs/kyber-changes.md](../docs/kyber-changes.md)), la console d'administration, l'icône de la zone de notification et l'installateur qui pose le tout.

| Dossier | Contenu |
|---|---|
| `admin/` | Console `kyber-admin.exe` : service Windows (Rust, axum), HTTPS |
| `web/` | Interface de la console (Svelte), embarquée dans le binaire |
| `tray/` | Icône de la zone de notification `kybernow-tray.exe` |
| `installer/` | `KyberNow-Setup.exe`, voir [installer/README.md](installer/README.md) |
| `kyber-patches/` | Correctifs de Kyber côté serveur et leur script de compilation |

## La console (`kyber-admin`)

Installée par `KyberNow-Setup.exe`, en service Windows, sur un port tiré au hasard à l'installation (affiché à la fin, et dans l'icône de la zone de notification). Le compte est celui choisi pendant l'installation.

Identité v2 (Archivo, Martian Mono, signal Spectre, symbole « console »), quatre espaces par intention, menu à gauche et onglets en bas sur téléphone. Les choix s'appliquent aussitôt ; ceux de Kyber demandent un redémarrage, proposé en haut de page. Les anciennes adresses (`#/dashboard`, `#/logs`…) mènent à l'espace qui les reprend.

| Espace | Rôle |
|---|---|
| Vue d'ensemble (`#/ensemble`) | L'aperçu de l'écran en grand ; en session, l'appareil connecté, depuis quand, son profil, sa latence et « Mettre fin à la session » ; l'état en quatre cartes (serveur, écran virtuel, son, mise à jour), les derniers appareils et le fil du jour |
| Image et son (`#/image-son`) | Écran virtuel SudoVDA (déclenchement, mode par défaut, essai de 15 s, rallumer les écrans), son (déclenchement, voies, qualité Opus), clavier, souris et manettes (presse-papiers, Xbox 360, répétition) |
| Accès (`#/acces`) | Ajouter un appareil (QR code et lien `kybernow://`, empreinte du certificat), demandes d'accès par code (Apple TV : accepter, refuser), appareils connus (oublier, retirer l'accès), plusieurs appareils en même temps, comptes Kyber (Basic) |
| Système (`#/systeme`) | Mises à jour en un clic, serveur (démarrer, arrêter, redémarrer, carte graphique, ports), journaux par source (connexions, image et son, commandes, console ; filtre, suivi, téléchargement, niveau de détail), Avancé : réglages bruts de Kyber (encodeur NVENC, AMF, Quick Sync ou x264, choisi à l'installation selon la carte graphique ; moteur, rafraîchissement progressif, débit minimum, port, adresses, méthodes de connexion), éditeur de `kyber_config.toml`, mot de passe de la console |

**Appareils et fil d'événements** (`activity.rs`). Les apps KyberNow se présentent dans `POST /api/stream/prepare` (`device` : identifiant stable, nom, type, système, version de l'app ; `profile`), donnent leur latence médiane avec leur signal de vie (`latency_ms`) et celle de toute la session à la fin. La console garde les 50 derniers appareils (`devices.json`) et les 300 derniers événements, écrits en phrases (`events.json`) : connexions, déconnexions avec durée et latence, sessions interrompues (plus de signal de vie pendant 30 s), comptes, redémarrages, mises à jour. Une app d'avant 0.2.1 compte pour « un appareil » par compte. Routes de la console : `GET /api/devices`, `DELETE /api/devices/{id}`, `GET /api/events?limit=`, `POST /api/session/end`, `GET /api/preview?width=` (aperçu de l'écran, comme pour les apps). `GET /api/status` donne aussi la session en cours et la dernière (`activity`) et la mise à jour disponible.

**Cartes graphiques** (`gpu.rs`), pour NVIDIA, AMD et Intel. Un fil dédié relit toutes les 5 minutes les cartes de Windows (`Win32_VideoController`, même tri que l'installateur : cartes PCI NVIDIA, AMD, Intel en état de marche, avec le pilote du fabricant, dans cet ordre ; la première est celle qui encode) et relève chaque seconde les compteurs « GPU Engine » de Windows (`\GPU Engine(*)\Utilization Percentage`, WDDM 2.x, ceux du Gestionnaire des tâches), rattachés à chaque carte par son LUID (DXGI). Il en tire la charge de chaque carte, celle de son encodeur et de son décodeur (moteur le plus occupé ; « Video Codec », moteur commun de certaines cartes AMD et Intel, compte pour les deux), et sait si un flux est en cours : `kyavserver.exe` occupe le moteur d'encodage d'une carte (le flux reste « en cours » 15 s après la dernière image encodée, un écran immobile pouvant ne rien donner à encoder), ou NVML compte une session NVENC. Ce signal sert à l'aperçu de l'écran (pas de capture pendant un flux) et aux filets de sécurité de l'écran virtuel et du son (déclenchement « pour toute connexion », session d'une app sans nouvelles). NVML ajoute pour les cartes NVIDIA les sessions NVENC, images par seconde, latence d'encodage, état de performance, température, puissance et mémoire. Dans `GET /api/status`, `gpu.devices` donne pour chaque carte `name`, `vendor` (`nvidia`, `amd`, `intel`), `driver` (numéro NVIDIA, 560.94, ou celui de Windows), `encoding`, `gpu_utilization`, `encoder_utilization`, `decoder_utilization` et, pour NVIDIA, les champs de NVML ; `gpu` vaut `null` sans carte compatible. Sans compteurs « GPU Engine » (Windows trop ancien), seul NVML renseigne l'activité.

**Mettre fin à une session.** Une app KyberNow l'apprend à son signal de vie suivant (`"end": true`, au plus 5 s) et se déconnecte proprement ; un autre client (navigateur, ligne de commande) est coupé en redémarrant Kyber. `POST /api/local/end` fait de même depuis l'icône, réservé à l'ordinateur lui-même.

**Appairage par code (Apple TV)** (`pairing.rs`). Sans rien taper à la télécommande : l'Apple TV tire un code à 6 chiffres, l'affiche et l'envoie à toutes les consoles trouvées en Bonjour, sans compte : `POST /api/pair/request` (`{"code", "device": {id, name, kind, os, app}}` → `{"id", "expires_in"}`). La demande apparaît dans Accès › Appareils avec le nom de l'appareil et son code en grand (Accepter, Refuser), dans un bandeau sur les autres espaces avec un badge sur Accès, et dans le panneau de l'icône, qui s'ouvre de lui-même pour une nouvelle demande et permet d'accepter ou refuser (`POST /api/local/pair/{id}/accept|refuse`, réservé à l'ordinateur lui-même). On n'accepte que si le code est celui affiché sur le téléviseur. L'appareil interroge `GET /api/pair/request/{id}`, sans compte : `pending`, `refused`, `expired` (aussi pour une demande inconnue), ou `accepted` avec `username` et `password`, livrés une seule fois (la demande disparaît aussitôt). Accepter (`POST /api/pair/accept/{id}`, session de la console) crée pour l'appareil **un compte Kyber à lui** : identifiant tiré de son nom (« Apple-TV-du-salon-3f9a »), mot de passe aléatoire de 128 bits. La console et Kyber l'acceptent tous les deux (Basic, comme un compte saisi à la main) ; l'app l'enregistre comme pour un ajout normal, sans rien changer côté client. Kyber ne lit ses comptes qu'au démarrage : la console le redémarre aussitôt (en arrière-plan) si personne n'est connecté, sinon le bandeau « redémarrez le serveur » s'affiche. L'appareil entre dans la liste avant sa première session (« Appairé à l'instant », « accès propre ») ; « Retirer l'accès » (`DELETE /api/devices/{id}`) supprime son compte, demande la fin de sa session s'il en a une, et Kyber suit au redémarrage (la console le refuse tout de suite). Dans Comptes, ces comptes portent « Accès propre à … », sans changement de mot de passe. Un appareil appairé de nouveau perd son ancien compte. Limites (demandes sans compte) : code de 6 chiffres exactement, 10 demandes par minute et 2 en attente par adresse, 6 en attente en tout, une nouvelle demande d'un même appareil remplace la précédente ; 5 min pour accepter, puis 2 min pour venir chercher l'accès (sinon le compte créé est supprimé). Les demandes ne vivent qu'en mémoire. `GET /api/status` et `GET /api/local/state` donnent les demandes en attente (`pair_requests`).
Pourquoi pas un jeton JWT propre à l'appareil, signé par la console avec la clé de Kyber ? Kyber ne sait pas en révoquer un seul (il faudrait changer la clé, donc couper tous les jetons), et l'app devrait passer par `kyclient_auth_credentials_new_jwt` et le renouveler. Le compte propre est révocable un par un et ne change rien aux apps.

**Pour les apps.** La console s'annonce en Bonjour (`_kybernow._tcp`, TXT `v`, `name`, `fp`, `kyber`), d'où la règle de pare-feu UDP 5353. `kyber=1` : un serveur Kyber est derrière (sa configuration est là et son service Windows existe) ; `kyber=0` : console seule (Mac de développement, installation incomplète), aucune session n'y démarrerait et les apps ne la proposent pas. Une annonce sans `kyber` (console d'une version précédente) reste proposée. Elle sert aussi l'API de session :
- `GET /api/stream/info` : nom, système (`os`, par exemple « Windows 11 Pro 24H2 », absent si inconnu), GPU, AV1, écran virtuel, haut-parleurs. `GET /api/status` (console web) donne aussi `os`. Sous Windows, le nom vient du registre (ProductName et DisplayVersion, « Windows 11 » d'après le numéro de build, ProductName disant encore « Windows 10 ») ; sur le Mac de développement, de `sw_vers` ; sous Linux, de `PRETTY_NAME` dans `/etc/os-release` ;
- `GET /api/stream/preview?width=960` : aperçu de l'écran principal tel qu'il est en ce moment, pour le fond de l'accueil des apps. JPEG (qualité 70), largeur bornée entre 320 et 1600 (960 par défaut), hauteur au ratio, en-tête `X-Captured-At` (RFC 3339). `503` et `{"error": "…"}` si l'écran ne peut pas être capturé : personne de connecté, écran verrouillé ou demande de sécurité affichée, agent indisponible. La dernière capture est resservie pendant 10 s (5 s pour un échec) ; pendant une session (app KyberNow ou autre client de Kyber : kyavserver encode, sur une carte NVIDIA, AMD ou Intel), aucune capture n'est faite : la dernière est resservie, ou `503`. La capture est faite par un agent lancé dans la session Windows (`display-agent preview`, GDI, écran principal réduit à la largeur demandée), qui écrit le JPEG dans `preview\` ; sur le Mac de développement, par `screencapture` (il faut l'autorisation d'enregistrer l'écran) ;
- `POST /api/stream/prepare` : écran virtuel à la taille du client, son au bon nombre de voies ; l'app s'y présente (`device`, `profile`) ;
- `POST /api/stream/heartbeat` : signal de vie (sans lui, l'écran virtuel disparaît), avec la latence du moment ; la réponse dit si la console demande la fin de la session (`end`) et si l'écran virtuel est en place (`active`). Un nouvel appel à `prepare` par l'appareil déjà en session (écran redemandé après un redémarrage de Kyber) continue la même session : ni nouvelle connexion au fil d'événements ni session de plus pour l'appareil ;
- `GET /api/stream/foreground` : app au premier plan sur l'ordinateur, en plein écran ou non, jeu ou non (mode Auto de la souris des apps). La console l'obtient d'un agent qui tourne dans la session Windows le temps d'une session (`display-agent foreground`, état écrit toutes les 400 ms). L'agent s'arrête seul sans signal de vie pendant 20 s ;
- `POST /api/stream/end` : écrans et sortie audio d'origine rendus, avec un compte rendu ; l'app y donne la latence médiane de la session ;
- `GET /api/pairing` : lien d'appairage ;
- `POST /api/pair/request`, `GET /api/pair/request/{id}` : appairage par code, sans compte (voir plus haut).

**Mises à jour** (compte de la console) : `GET /api/update` (version installée, dernière version publiée, notes), `POST /api/update/check`, `POST /api/update/install`. Si le PowerShell de l'installation s'arrête sans avoir remplacé la console, `GET /api/update` donne `install_error` (avec la fin du journal) et l'installation peut être relancée. `GET /api/local/state` (réservé à l'ordinateur lui-même, pour l'icône) donne aussi `update_available`, `version`, le nom de l'ordinateur, son adresse et la session en cours (`activity`).

L'authentification est en Basic, avec le compte Kyber. Les apps épinglent le certificat de la console par son empreinte.

**Client web** (`/ecran/`, sources dans `server/web/src/ecran/`, entrée « Ouvrir dans le navigateur » de la Vue d'ensemble). Ouvrir l'ordinateur depuis un navigateur, sans rien installer : accueil sur l'aperçu de l'écran, profils Bureau / Jeu, réglages gardés dans le navigateur, barre de session (latence, mesures détaillées, son, menu Touches avec Ctrl+Alt+Suppr, plein écran avec toutes les touches sous Chrome et Edge), et en tactile une languette, un pavé tactile ou le toucher direct, et une rangée de touches au-dessus du clavier de l'appareil. Fonctionnement :
- **Compte** : la session de la console (même mot de passe). `GET /api/web/ticket` (session requise) signe un jeton JWT HS256 (`aud` « kyber », `sub` : le premier compte Kyber, 12 h) avec la clé que Kyber connaît, posée par `init` dans `[kycontroller.auth.jwt]`. Les routes `/api/stream/*` acceptent aussi la session de la console : le client web prépare l'écran et envoie son signal de vie comme une app.
- **Moteur** : `kyclient_wasm` de Kyber, chargé depuis `/ecran/kyclient_wasm.js` (avec `kyclient_wasm_bg.wasm`, `audio_worklet.js`, `spinlock.js`). La console relaie ces fichiers depuis le `/webclient/` de Kyber installé : toujours de la même version que le serveur. La page et ces fichiers sont servis avec `Cross-Origin-Opener-Policy` et `Cross-Origin-Embedder-Policy` (mémoire partagée du son).
- **Relais** (`kyber_proxy.rs`) : le moteur parle à kycontroller sur l'adresse de la page, et kycontroller refuse les autres origines. La console relaie donc `/session/*`, `/kymux/*`, `/ws` (WebSocket), `/capabilities`, `/enumerate_displays`, `/stop_streaming`, `/set_bitrate`, `/force_idr` et `/websocket/*` vers kycontroller sur cette machine (TLS, certificat de Kyber non vérifié, connexion locale). Dans `/kymux/start_mux`, le port de l'image annoncé par le moteur (celui de la console) est remplacé par celui de Kyber. L'image elle-même ne passe pas par la console : WebTransport sur le port de Kyber, vérifié par l'empreinte de son certificat.
- **Safari, iPhone, iPad** : leur WebTransport ne vérifie pas un certificat par son empreinte ; l'image passe par WebSocket (`/websocket/*`, relayé par la console, un peu plus de latence). Ailleurs, sans première image en 12 s en WebTransport, la page bascule aussi sur WebSocket.
- **Souris** : le moteur capture le pointeur dès que l'ordinateur masque son curseur, ce que Windows fait sans cesse sans souris branchée. La page n'autorise la capture qu'en mode Jeu, ou en Auto quand la console voit un jeu en plein écran (`GET /api/stream/foreground`). `Ctrl + Alt + Maj` rend la souris et affiche la barre ; une languette en haut de l'image l'affiche aussi. Le moteur dessine le pointeur de l'ordinateur en curseur de la toile et pose `cursor: none` quand l'ordinateur le dit masqué (sans souris branchée, toujours) : hors capture, la page remet une flèche. Hors capture aussi, le moteur envoie à chaque mouvement un déplacement relatif puis la position absolue, appliqués tous deux par l'ordinateur ; le relatif (accéléré par Windows, perdu hors de la fenêtre) décalait les pointeurs : la page le met à zéro, seule la position compte, comme l'app Mac en mode bureau. Sauf quand le pointeur de l'ordinateur est masqué (Windows le cache à la moindre frappe) : le moteur passe alors en souris relative et n'envoie plus que le déplacement ; Windows ne réaffichant son pointeur que sur une position absolue, la page envoie alors elle-même la position sur la connexion WebSocket des entrées (paquet kynput `[2][écran u32][x i16][y i16]`, écran repris des positions du moteur ou de `/enumerate_displays`) ; en WebTransport, où l'on ne peut pas glisser un message entier dans le flux, le déplacement passe tel quel.

Le `/webclient/` d'origine de Kyber reste servi par kycontroller, pour le dépannage.

Les changements d'écran, de son et de pointeur sont faits dans la session de l'utilisateur par `kyber-admin.exe display-agent …`, lancé par le service.

| Emplacement | Contenu |
|---|---|
| `C:\ProgramData\KyberNow\kyber-admin.toml` | réglages : `listen`, dossier de Kyber, empreinte du mot de passe, écran virtuel, son |
| `C:\ProgramData\KyberNow\tls\` | certificat de la console |
| `C:\ProgramData\KyberNow\backups\` | versions précédentes de `kyber_config.toml` |
| `C:\ProgramData\KyberNow\display-session.json`, `audio-session.json`, `pointer-session.json` | ce qu'il faut restaurer en fin de session, même après un plantage |
| `C:\ProgramData\KyberNow\update\<version>\` | paquet téléchargé, contenu, journal `update.log` (les deux dernières versions) |
| `C:\ProgramData\KyberNow\update\resultat.json` | échec de la dernière mise à jour (écrit par `update.ps1`, lu par la console à son démarrage) |
| `C:\ProgramData\KyberNow\preview\` | capture en cours de l'aperçu de l'écran (effacée dès qu'elle est lue) |
| `C:\ProgramData\KyberNow\devices.json`, `events.json` | appareils connus et dernière session, fil des événements de la vue d'ensemble |
| `C:\ProgramData\KyberNow\foreground\` | veille du plein écran pendant une session (`run` : signal de vie, `state.json` : dernier état) |
| `C:\ProgramData\KyberNow\kyber-admin.log` | journal du service |

Commandes : `kyber-admin.exe service install|uninstall`, `kyber-admin.exe reset-password` (puis redémarrer le service), `kyber-admin.exe init …` (utilisée par l'installateur), `kyber-admin.exe` seul pour lancer en console (diagnostic).

## Développer sur le Mac

```bash
cd server/web && npm run build                       # ou npm run dev (proxy /api vers la console)
cd ../admin && cargo run -- --http --listen 127.0.0.1:9443 \
    --data-dir dev-data --kyber-dir dev-data/kyber    # copier un kyber_config.toml dans dev-data/kyber
```

Hors Windows, le service Kyber, l'écran virtuel et le son sont simulés, et la console s'annonce en Bonjour avec `kyber=0` (les apps ne la montrent pas) ; `--announce-kyber` l'annonce avec `kyber=1` pour tester l'ajout et l'appairage depuis les apps (en HTTPS, sans `--http`, et sur `0.0.0.0` pour que l'Apple TV la joigne) ; pour les cartes graphiques, seul NVML est essayé (absent sur le Mac).

Client web contre le Kyber du PC : lancer la console en HTTPS (le moteur appelle toujours `https://`) avec `KYBERNOW_KYBER_ADDR=192.168.1.100:30241` (adresse et port de Kyber sur le PC), puis `npm run dev` dans `server/web` (relais vers `https://127.0.0.1:9448`) ou ouvrir `https://localhost:<port>/ecran/`. L'image ne peut arriver que si le navigateur joint Kyber sur l'adresse de la page : sur le Mac, l'écran d'accueil et les réglages se vérifient, la session se teste sur le PC.

## Compiler pour Windows (depuis le Mac)

```bash
server/build-windows.sh            # console seule → server/dist/kyber-admin.exe
server/kyber-patches/build.sh      # Kyber corrigé → third_party/payload/kyber-overlay/
server/installer/build.sh          # tout → server/installer/dist/KyberNow-Setup.exe
```

## Mettre à jour un ordinateur déjà installé

En un clic : page **Mises à jour** de la console (ou « Installer la version X… » dans le menu de l'icône). La console télécharge la dernière version publiée sur GitHub, vérifie son empreinte et lance `update.ps1`. Détails, mise à jour à la main et publication d'une version : [docs/releases.md](../docs/releases.md).

`server/installer/update.ps1` remplace la console, l'icône et la surcouche Kyber sans réinstaller ni changer les ports. Il prépare `kyber.new`, `admin.new` et `tray.new`, bascule par renommage (la version précédente reste dans `kyber.prev`, `admin.prev`, `tray.prev`), puis vérifie que la console répond en 60 s ; sinon il remet la version précédente. Il installe aussi les haut-parleurs de Steam s'ils manquent et ajoute la règle de pare-feu Bonjour. `server/installer/package-update.sh` fabrique son paquet (`dist/kybernow-update.zip`).
