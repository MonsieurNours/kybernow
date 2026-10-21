# KyberNow — cadrage

[English](PROJECT.en.md) · Français

Serveur Windows (toute carte graphique : NVIDIA, AMD, Intel, eGPU compris ; sinon le processeur) clé en main + GUI web d'administration, et clients natifs macOS / tvOS « à la Moonlight », construits sur [Kyber](https://kyber.tech) (SDK de streaming basse latence de l'équipe VLC, Rust + FFmpeg + VLC + QUIC).

## Décisions prises

| Sujet | Choix |
|---|---|
| Produit | **KyberNow**, développé par Les Lignes Claires (https://www.leslignesclaires.fr) — nom à valider avec Kyber avant toute diffusion publique |
| Fondation | Kyber (serveur + SDK client C), version épinglée |
| Usage | Jeu **et** bureau à distance → profils « Jeu » / « Bureau » |
| Distribution | Usage perso / équipe (AGPLv3 OK, pas d'App Store public) |
| Réseau | LAN (découverte Bonjour) + accès distant via VPN (Tailscale/WireGuard, documenté) |
| Installeur Windows | Notre propre installeur **NSIS** (compilable sur Mac), qui pose lui-même les fichiers de Kyber 0.29 (pas d'appel à l'installeur officiel) |
| Mot de passe | Demandé pendant l'installation : même identifiant/mot de passe pour Kyber (Basic) et kyber-admin |
| Écran virtuel | Pilote **SudoVDA** (MIT/CC0, celui d'Apollo), isolé derrière une interface pour passer à Kidd quand Kyber le publiera |
| Déclenchement | Réglable dans l'admin : jamais / nos apps seulement / tous les clients (mode par défaut pour ceux qui n'envoient pas leur résolution) |
| Fin de session | Écran virtuel retiré, écrans physiques restaurés, **pas** de verrouillage de Windows |
| 2e client | Il prend la main **avec préavis** (décidé le 2026-10-20) : la console demande au 1er de finir (`end` à son signal de vie suivant, 5 s au plus), attend qu'il ait rendu la main (7 s), puis recrée l'écran virtuel aux dimensions du 2e. Un seul appareil à la fois ; la session est identifiée par l'appareil (`device_id`), pas par le compte |
| Encodeur | Choisi à l'installation selon les cartes graphiques en service (eGPU compris), de la plus à la moins performante : NVIDIA → `nvenc`, sinon AMD → `amf`, sinon Intel → `qsv`, sinon le processeur → `x264`. Portable hybride (Intel + NVIDIA) : NVENC. Aucun encodeur présenté comme expérimental ; une mise à jour ne touche jamais au choix de l'utilisateur |
| Ports | Tirés au hasard à l'installation (stream et console séparés), dans 20000–44999, hors plages réservées par Windows (Hyper-V/WSL) et ports Sunshine/Moonlight ; libres en TCP **et** UDP ; affichés et modifiables à l'écran de vérification ; annoncés en Bonjour |
| Installateur (UX) | Immersif, animé (fond « flux de signal », transitions, diagnostic pas à pas, installation vivante, célébration), didactique ; maquette : https://claude.ai/artifact/3BnFPgxYmz2etqHXM1cd8S ; respecte « réduire les animations » ; écran « À propos » (licences, site du développeur) |
| Admin web | Service Rust + SPA Svelte embarquée. Console v2 (20/10/2026) : quatre espaces par intention, Vue d'ensemble · Image et son · Accès · Système ; l'image de l'ordinateur d'abord ; réglages bruts de Kyber rangés dans Système › Avancé ; menu à gauche, onglets en bas sur téléphone. Appareils connus et fil d'événements tenus par la console (les apps se présentent en début de session) ; fin de session demandée depuis la console ou l'icône. Icône près de l'horloge : un panneau (WebView2) au clic gauche, le menu de Windows au clic droit. Installateur en quatre étapes (l'analyse pendant la bienvenue) |
| Client web (20/10/2026) | Nouveau client servi par la console (`/ecran`), même compte, pour cet ordinateur seulement ; entrée « Ouvrir dans le navigateur » sur la Vue d'ensemble. Moteur `kyclient_wasm` de Kyber, interface refaite sur le modèle de l'app Mac : profils Bureau / Jeu, réglages de l'app (protocole, tampons et format choisis seuls, rangés dans « Avancé »), barre de session avec menu Touches (Ctrl+Alt+Suppr…). PC, Mac, tablette (pavé tactile ou toucher direct) et téléphone ; thème sombre seulement. Le `/webclient/` de Kyber reste en place pour le dépannage. Maquettes : rangée « Dans le navigateur » de https://claude.ai/artifact/YGA7unwqztvuA26CMtdbv8 |
| Identité (v2, 20/10/2026) | Symbole « Relais » (carré coupé à 45° : l'ordinateur, le lien, vous ; sans lettre, il survit à un changement de nom), signal Spectre `#9A8CFF` en néon allumé seulement dans les moments actifs, Archivo + Martian Mono ; « ordinateur » pour la machine partagée, « appareil » pour ce qui s'y connecte, aucun système nommé hors des informations techniques ; tout dans `brand/v2/` ; charte : https://claude.ai/artifact/R3mNbayaFan3XPCtbmzCFt ; maquettes : https://claude.ai/artifact/YGA7unwqztvuA26CMtdbv8 |
| Site (20/10/2026) | **www.kybernow.app**, dans `site/` : statique, sans npm, tout vendorisé (Three.js r170, polices, icônes). Français à la racine, anglais sous `/en/`, langue détectée et choix retenu ; guide pas à pas et FAQ (`/guide/`), mentions légales (`/mentions-legales/`, éditeur Matthieu Requenna EI « Les Lignes Claires », SIREN 510 178 577) ; hébergé sur GitHub Pages (workflow `.github/workflows/pages.yml`, domaine à brancher) ; Discord communautaire (https://discord.gg/sVy2mu5WdT : Bienvenue, Communauté, Aide en forums, Vocal, Équipe privée ; mode Communauté). Présente KyberNow comme « streaming d'ordinateur nouvelle génération » (votre propre cloud), le trajet d'une image mesuré (9,4 ms), QUIC, la comparaison avec Sunshine + Moonlight, et les téléchargements (Windows et Mac par liens `releases/latest`, Apple TV installée depuis un Mac avec Xcode, navigateur). Signale que KyberNow est en bêta tant qu'il n'est pas en 1.0 (badge, encart, FAQ). Dit qu'il utilise Kyber (lien), qu'il n'y est pas affilié, et qu'il est communautaire et non commercial. Scène 3D du symbole et néon selon la charte. Voir `site/README.md` |
| Apple | SwiftUI, macOS puis tvOS ; compte Apple Developer payant dispo |
| Environnement | Dev sur Mac, PC Windows NVIDIA accessible sur le LAN |

## Ce que Kyber fournit (v0.29.0, 2026-10-09)

- Serveur Windows 10/11 : service Windows (`kyservice` → `kycontroller`, `kyavserver`, `kynputserver`), icône de zone de notification, NVENC recommandé, port unique `8080` TCP+UDP.
- Config unique `kyber_config.toml` (encodeur, débit mini, port, TLS, JWT/OIDC, presse-papiers, manette, écrans virtuels « Kidd »). La plupart des changements nécessitent un redémarrage. Surcharge possible via `KYBER_CONFIG`.
- Manettes : pilote ViGEmBus requis côté serveur.
- Client web servi par le contrôleur sur `/webclient/`.
- SDK client : cœur Rust, bindings C (`libclient`, `libkynput`) via cbindgen ; rendu via VLC (fork). App `kyber-ios` (iOS/tvOS) **non publique**.
- Pas de traversée de NAT → LAN / VPN.
- Sources : https://gitlab.com/kyber (docs : `kyber/kyber`, apps : `kyber/apps/*`, cœur : `kyber/core/*`).

## Architecture cible

```
kyber-apps/
├── server/
│   ├── admin/          # kyber-admin : service Windows en Rust (axum)
│   ├── web/            # SPA Svelte, compilée et embarquée dans admin
│   └── installer/      # NSIS : Kyber + SudoVDA + ViGEmBus + kyber-admin + pare-feu + alimentation
└── apple/
    ├── KyberKit/       # Swift package : xcframework du SDK C Kyber + API Swift
    ├── KyberMac/       # app macOS (SwiftUI)
    └── KyberTV/        # app tvOS (SwiftUI) — réalisé dans apps/tvos
```

### kyber-admin (Windows)
- Tourne en service Windows, HTTPS sur un port dédié, accès protégé par mot de passe.
- **Config** : lecture/écriture de `kyber_config.toml` en préservant les commentaires (`toml_edit`), validation par schéma, application avec redémarrage contrôlé du service Kyber.
- **Service** : start / stop / restart / état du service Kyber.
- **Supervision** : carte graphique et pilote (`Win32_VideoController`), charge de la carte et de son encodeur, flux en cours (compteurs « GPU Engine » de Windows, pour NVIDIA, AMD et Intel) ; en plus pour NVIDIA, par NVML : sessions NVENC, images par seconde, latence, alimentation, température. Session active, logs en direct.
- **Appairage** : page « Appareils » de la console : QR code et lien `kybernow://adresse:port?name=…&fp=sha256:…` (adresse, port de la console, empreinte de son certificat ; aucun secret). L'app lit le QR code à la caméra du Mac ou le lien collé (ou ouvert : schéma `kybernow://`). L'empreinte vue est comparée à celle du lien ou de l'annonce ; par simple adresse, l'utilisateur la compare à celle de la console (4 premiers groupes). Puis compte de la console, vérifié et gardé dans le Trousseau ; certificat épinglé. **Par code (2026-10-20, Apple TV)** : l'appareil envoie un code à 6 chiffres à la console sans compte, on l'accepte dans Accès › Appareils ou le panneau de l'icône ; il reçoit un compte Kyber à lui (mot de passe aléatoire), accepté par la console et Kyber, révocable dans Appareils (« Retirer l'accès »), sans changement côté app. Voir [server/README.md](server/README.md).
- **Découverte** : annonce Bonjour/mDNS `_kybernow._tcp` de la console (TXT `v`, `name`, `fp`), règle de pare-feu UDP 5353 pour kyber-admin.
- **Accueil des apps** : `GET /api/stream/info` donne le système de l'ordinateur (« Windows 11 Pro 24H2 », pensé pour un serveur multi-plateforme), `GET /api/stream/preview` un aperçu JPEG de l'écran principal (capture GDI par l'agent dans la session de l'utilisateur, 10 s de cache, jamais pendant une session), fond de l'accueil quand l'app n'a pas d'image de la dernière session.
- **Profils** : « Jeu » (faible latence, manette, haut fps) / « Bureau » (4:4:4, presse-papiers, multi-écrans).
- **Client web** (`/ecran/`, 20/10/2026) : servi par la console, avec sa session. La console signe un jeton JWT pour Kyber (clé posée à l'installation), relaie les appels HTTPS et WebSocket du moteur `kyclient_wasm` à kycontroller (qui refuse les autres origines) et sert les fichiers du moteur depuis le `/webclient/` installé ; l'image arrive directement de Kyber en WebTransport. Détails dans [server/README.md](server/README.md). **Reste à valider sur le PC** : la session de bout en bout (image, son, clavier, souris, manette, presse-papiers), la reprise de main d'un autre appareil, le tactile et le clavier des téléphones et tablettes (les gestes sont traduits en événements de souris et de clavier envoyés au moteur).

### Écran virtuel (session de stream)
- **Début de session** : le client annonce sa résolution et sa fréquence à kyber-admin (nos apps) ou on prend le mode par défaut ; kyber-admin crée l'écran virtuel SudoVDA à ce mode, mémorise la disposition des écrans, n'active que l'écran virtuel, et la capture Kyber le suit.
- **Fin de session** : écran virtuel retiré, disposition d'origine restaurée.
- **Filet de sécurité** : le pilote retire l'écran s'il ne reçoit plus de signal de vie (3 s) ; kyber-admin restaure la disposition à son démarrage et quand plus aucun flux n'est encodé (toute carte graphique) ; raccourci clavier d'urgence.
- ✅ **Validé le 2026-10-19** sur le PC (RTX 3080 Ti, 2 écrans 4K) : écran virtuel 3840 × 2160 à 160 Hz créé avant la connexion, écrans physiques éteints puis restaurés, stream reçu à 152 i/s, 8,3 ms de latence médiane.
- **Piège connu** : l'énumérateur d'écrans de Kyber voit l'ancienne disposition pendant ~50 ms ; `prepare` attend que Windows n'affiche plus que l'écran virtuel, puis 1,5 s, avant de répondre. Les apps devront choisir l'écran d'après la liste mise à jour.
- **Reste à valider** : le mode « Pour toute connexion » (écran créé en cours de flux), et la reprise de main par un 2e appareil (déconnexion du 1er).
- **Piège corrigé** : le chien de garde de SudoVDA (3 s) est armé dès la création de l'écran. Le signal de vie part donc d'un fil dédié dès `add`, pas seulement une fois la session active : sinon l'écran disparaissait pendant la mise en place (écran blanc côté client, échec d'initialisation de l'encodeur côté serveur).
- **Architecture** : le service pilote SudoVDA (IOCTL, signal de vie) ; un agent éphémère `kyber-admin display-agent`, lancé avec le jeton SYSTEM dans la session console, change la disposition (API CCD).
- **Concession de sécurité** : SudoVDA est auto-signé, son certificat est ajouté aux autorités racines du PC.

### Qualité d'image : 4:4:4, HDR, VRR (Kyber 0.29)
- **4:4:4** ✅ validé le 2026-10-19 (HEVC, 3840 × 2160 à 160 Hz, client `--444`). Coût par rapport au 4:2:0 : encodage NVENC ~9 ms en médiane (p95 46 ms), décodage ~6,6 ms, latence totale ~18 ms en médiane. À proposer dans le profil « Bureau » (texte net), pas dans le profil « Jeu ».
- **HDR** ❌ impossible avec Kyber 0.29 : la capture (DXGI Desktop Duplication) est en BGRA 8 bits. Garder le HDR Windows désactivé sur l'écran virtuel.
- **Son multicanal et HD** (correctif `server/kyber-patches/0001`) : le moteur txproto capture déjà la sortie Windows avec tous ses canaux ; le débit Opus, fixé à 128 kbit/s par Kyber, suit désormais le nombre de canaux. Réglage `[kyavserver] audio_quality` : `standard` (128 kbit/s), `hd` (défaut : 256 kbit/s en stéréo, 576 en 5.1, 768 en 7.1), `studio` (maximum d'Opus) ; `audio_bitrate` pour une valeur exacte. Le moteur libavconv garde aussi le 5.1 et le 7.1 (0008) ; `audio_layout = "stereo"` l'en empêche, pour le client web qui ne lit que la stéréo. L'app propose Auto, Stéréo, 5.1, 7.1 ou Aucun ; Auto suit la sortie audio du Mac.
- **VRR** : côté serveur, Kyber envoie une image quand l'écran change (débit d'images suivant le contenu). Côté client, l'app présente chaque image dès qu'elle est décodée (Metal, au plus une image en attente d'écran, la plus récente remplace les autres) : sur un écran ProMotion / Adaptive-Sync, l'écran se cale sur l'arrivée des images. Synchro verticale désactivable (latence minimale, déchirure possible). Pas de VRR sur Apple TV.
- **Décodage par l'app (2026-10-20)** : correctif `apps/macos/kyber-patches/0001` sur libkyclient (`kyclient_video_player_config_set_packet_sink`) : les paquets vidéo (Annex B, jeux de paramètres, image clé, pts) vont à l'app au lieu de VLC. KyberKit les décode avec VideoToolbox (matériel exigé, mode temps réel, sortie dans les formats compressés sans perte des puces Apple, lus sans copie par Metal) et les dessine avec Metal (conversion YCbCr → RGB selon la matrice du flux). Mesuré hors ligne sur M4 Max en 2560 × 1440 : 2,0 ms de décodage en 4:2:0, 2,7 ms en 4:4:4. VLC ne sert plus que de secours (l'AV1 est aussi décodé par l'app depuis le 2026-10-20, puces M3 et suivantes). Code sans AppKit (décodeur, rendu, capacités, son) : repris tel quel sur iOS/tvOS.
- **Son par l'app (2026-10-20)** : le même correctif fait passer les paquets Opus à l'app (`kyclient_audio_session_config_set_packet_sink`). libopus (multistream, masquage des pertes) → Core Audio, demandes de 5 ms à la carte son, tampon adaptatif d'une vingtaine de ms (VLC : 50 ms fixes, plus le sien). Côté serveur, `server/kyber-patches/0004` : paquets Opus de 10 ms en `lowdelay` (la capture WASAPI, elle, reste à la période de Windows). Son et image partent chacun dès qu'ils sont prêts ; l'écart est affiché dans la barre de session. Liste complète des changements : [docs/kyber-changes.md](docs/kyber-changes.md).
- **Souris, presse-papiers, débit, latence (2026-10-20)** : souris Auto, Bureau ou Jeu (relative, GCMouse), réglable dans la barre de session et par ⌃⌥⌘M ; Auto passe en mode jeu quand la console voit un jeu en plein écran sur l'ordinateur (pas un navigateur ni un lecteur vidéo), pas au simple masquage du pointeur que Windows fait sans cesse sans souris branchée ; presse-papiers texte partagé dans les deux sens (`apps/macos/kyber-patches/0002` sur libkynput) ; débit des profils proportionné aux pixels par seconde de la session ; latence de bout en bout (capture → affichage) mesurée avec les horodatages et le recalage d'horloge de Kyber, dans la barre et sur l'accueil. Console et installateur : « ordinateur » au lieu de « PC » partout.
- **Reprise après une perte (2026-10-20)** : sous rafraîchissement progressif, NVENC n'envoie jamais d'image clé ; après une perte au-delà de la FEC, VideoToolbox figeait l'image. L'app demande une image clé (`POST /force_idr`, `server/kyber-patches/0005` sur kycontroller, désormais dans la surcouche), et relance le flux sans réponse en 1,5 s.
- **Versions et mises à jour (2026-10-20)** : dépôt public https://github.com/MonsieurNours/kybernow, un numéro de version pour tout KyberNow, versions publiées par `scripts/release.sh` (notes : `CHANGELOG.md`) : arbre propre sur `main`, commit de version avant la construction, release en brouillon puis tag (binaires = commit étiqueté), canal bêta par prereleases, essai `--dry-run` ; CI GitHub Actions (Rust, Windows, Svelte, scripts, Apple TV) ; réglages versionnés et migrés (`kyber-admin migrate`). L'ordinateur se met à jour en un clic depuis la console (paquet vérifié par SHA-256, `update.ps1` lancé détaché, icône relancée dans la session de l'utilisateur), l'app Mac par Sparkle (EdDSA). Reste : signature Developer ID et notarisation. Voir [docs/releases.md](docs/releases.md).
- **Définition** : « Comme cet écran » suit l'écran du Mac au pixel près (5K, 6K), jusqu'à 7680 × 4320 (limite de la console et de l'écran virtuel), 4096 de large au plus en H.264 (limite de NVENC). Option « Au plus 4K » pour alléger l'encodage, le décodage et le débit.
- **Capacités du client** : au lancement, l'app vérifie les décodeurs matériels (H.264, HEVC, AV1) et décode pour de bon une image HEVC 4:4:4 en exigeant le matériel. Les profils s'y ramènent (sans 4:4:4 matériel : 4:2:0 avec plus de débit ; sans HEVC : H.264), la carte « Ce Mac » des réglages l'affiche, avec la définition et la fréquence de l'écran (variable ou non).

### Sortie audio virtuelle (session de stream)
- **Pourquoi** : sans elle, le son sort aussi des haut-parleurs du PC, au format réglé dans Windows, et disparaît si la sortie était l'HDMI d'un écran éteint par l'écran virtuel.
- **Pilote** : Steam Streaming Speakers (Valve, signé par Microsoft, stéréo à 7.1), installé par l'installateur depuis le dossier de Steam s'il est présent (comme Sunshine et Apollo). Aucun pilote libre n'est signé (Virtual-Audio-Driver exige le mode test) ; plan B : notre propre pilote signé par Microsoft (certificat EV + Partner Center au nom des Lignes Claires).
- **Session** : `prepare` reçoit `audio_channels` (2, 6, 8 ; 0 = son sur le PC). Le son bascule vers les haut-parleurs virtuels réglés à ce format (`IPolicyConfig`, comme le panneau Son) avant l'écran virtuel ; à la fin, la sortie d'origine revient après les écrans (elle peut revenir avec eux). Même filets de sécurité que l'écran virtuel (état sur disque, 20 s sans nouvelles). Page « Son » de la console : déclenchement et nombre de canaux.

### Pointeur de souris
- **Constat** : Kyber n'incruste pas le curseur dans l'image ; le client le dessine d'après les annonces du PC et bascule en souris relative dès que Windows le dit masqué, sans délai. D'où un curseur qui clignote ou disparaît.
- **Correctifs** : txproto n'annonce un masquage qu'après 250 ms (`server/kyber-patches/0002`) ; la console coupe « Masquer le pointeur pendant la frappe » le temps de la session ; l'app macOS gardera un curseur toujours visible en usage bureau et réservera la souris relative aux jeux.
- **Reste possible** : Windows sans souris physique masque son curseur en permanence ; à surveiller sur un PC sans souris.

### Correctifs de Kyber
- `server/kyber-patches/build.sh` recompile kyavserver et txproto pour Windows depuis macOS (voir docs/build-notes.md) et assemble `third_party/payload/kyber-overlay/`, posée par l'installateur par-dessus Kyber 0.29. La chaîne kyavserver (txproto, Lua, SPIRV-Cross, runtime MinGW) passe en UCRT ; aucun autre binaire de Kyber n'en dépend.

### Installeur
- Kyber à une version épinglée (binaires repris depuis la release officielle), SudoVDA, ViGEmBus, kyber-admin.
- Règles de pare-feu, génération des certificats TLS, plan d'alimentation hautes performances (la doc Kyber mesure 2,5 ms d'encodage NVENC contre 8 ms quand le GPU économise de l'énergie).
- Désinstallation propre.

### Clients Apple
- **État (2026-10-20)** : voir [apps/macos/README.md](apps/macos/README.md). `apps/macos` (XcodeGen, `project.yml`). KyberKit = façade Swift de libkyclient/libkynput (module C `CKyber`), vue de session (image décodée par l'app et dessinée avec Metal ; VLC pour l'AV1), clavier (kVK → HID → scancode), souris absolue, curseur du PC affiché en `NSCursor` et jamais masqué en usage bureau. App : ajout par Bonjour, adresse, QR code ou lien (empreinte vérifiée), mot de passe dans le Trousseau, profils Jeu/Bureau/Max, `prepare` (définition et fréquence de l'écran de ce Mac, voies du son), signal de vie, fin propre (⌃⌥⌘D), plein écran (⌃⌥⌘F). Avant le premier build : `scripts/vendor-kyber.sh`, puis `scripts/build-kyclient.sh` (libkyclient avec les correctifs KyberNow), puis `xcodegen generate`.
- **KyberKit** : `libclient` + `libkynput` + VLC compilés en xcframework (macOS arm64/x86_64, tvOS arm64 + simulateur), façade Swift (connexion, surface vidéo, entrées, métriques).
- **App Apple TV (2026-10-20)** : `apps/tvos` (XcodeGen), voir [apps/tvos/README.md](apps/tvos/README.md) ; maquette : https://claude.ai/artifact/RfGbfguTyAP1jjwFASVRyK. KyberKit partagé avec le Mac (fichiers sans AppKit d'`apps/macos/KyberKit`, plus `apps/tvos/KyberKitTV` : vue UIKit, manettes par GameController, clavier et souris), modèle et découverte Bonjour de l'app Mac. Décisions : 4K à 60 Hz au plus, Apple TV 4K A12 et suivantes (tvOS 18), HEVC ; accueil en scène plein écran ; ajout par iCloud depuis le Mac, Bonjour (clavier de l'iPhone), code à accepter dans la console, ou adresse ; jusqu'à 4 manettes (Xbox, DualSense…), vibrations, Siri Remote en souris, clavier et souris Bluetooth ; menu par le bouton Retour ou L1 + R1 + Start + Select ; son multicanal HDMI, audio spatial des AirPods, alerte de latence (AirPods, HomePod), son laissé sur l'ordinateur ; Top Shelf, écran de test des manettes, profils par utilisateur tvOS. Session validée de bout en bout sur une Apple TV.
- **Publication iCloud par l'app Mac (2026-10-20)** : `CloudPublisher` écrit les ordinateurs dans le magasin clé-valeur d'iCloud (clé `hosts`) et leurs mots de passe dans le Trousseau iCloud (service `fr.leslignesclaires.kybernow.cloud`) à chaque changement ; réglage « Synchroniser avec iCloud » (⌘,), actif par défaut. Droits iCloud dans `KyberNow.entitlements`, avec un profil « macOS App Development » (Debug) et un profil « Developer ID » (Release), voir [apps/macos/README.md](apps/macos/README.md#signature-et-icloud). Vérifié sur ce Mac : les deux versions se lancent et publient.
- **App** : liste des hôtes (Bonjour + ajout manuel), appairage, plein écran, réglages (résolution, fps, codec, débit, profil), manettes (framework GameController), clavier, souris et presse-papiers sur macOS, overlay de métriques.

## Risques à lever d'abord (prototypes rapides)

- ✅ **A. Référence de bout en bout** (validé : 9,4 ms médian, voir docs/prototype-A-results.md) : Kyber 0.29 sur le PC Windows ↔ client macOS en ligne de commande ; mesure de la latence et de la stabilité.
- ✅ **B. tvOS** (2026-10-20) : libkynput et libkyclient de Kyber 0.29 se compilent pour l'Apple TV et son simulateur avec Rust nightly (`-Zbuild-std`, comme le `build-ios.sh` de Kyber), correctifs KyberNow compris ; VLC n'est pas nécessaire (l'app décode l'image et le son : des fonctions vides satisfont l'éditeur de liens). Script : `apps/tvos/scripts/build-kyber-tvos.sh`.
- **C. API du contrôleur** : inventorier ce que `kycontroller` expose en HTTP (état de session, métriques) et le format exact des JWT attendus (`docs/Architecture/Authentication.md`).

## Phases

0. Prototypes A, B, C
1. Serveur : kyber-admin + UI Svelte + installeur NSIS + écran virtuel
2. KyberKit + app macOS
3. App tvOS
4. Finitions : Wake-on-LAN, HDR, sélection d'écran, mises à jour automatiques

## Questions ouvertes

- Nom du produit / identifiant de bundle.
- Plusieurs utilisateurs simultanés sur le même PC ?
