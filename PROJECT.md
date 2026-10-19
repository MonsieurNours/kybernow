# KyberNow — cadrage

Serveur Windows (NVIDIA) clé en main + GUI web d'administration, et clients natifs macOS / tvOS « à la Moonlight », construits sur [Kyber](https://kyber.tech) (SDK de streaming basse latence de l'équipe VLC, Rust + FFmpeg + VLC + QUIC).

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
| 2e client | Il prend la main : le 1er est déconnecté et l'écran virtuel recréé à ses dimensions |
| Ports | Tirés au hasard à l'installation (stream et console séparés), dans 20000–44999, hors plages réservées par Windows (Hyper-V/WSL) et ports Sunshine/Moonlight ; libres en TCP **et** UDP ; affichés et modifiables à l'écran de vérification ; annoncés en Bonjour |
| Installateur (UX) | Immersif, animé (fond « flux de signal », transitions, diagnostic pas à pas, installation vivante, célébration), didactique ; maquette : https://claude.ai/artifact/3BnFPgxYmz2etqHXM1cd8S ; respecte « réduire les animations » ; écran « À propos » (licences, site du développeur) |
| Admin web | Service Rust + SPA Svelte embarquée |
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
    └── KyberTV/        # app tvOS (SwiftUI)
```

### kyber-admin (Windows)
- Tourne en service Windows, HTTPS sur un port dédié, accès protégé par mot de passe.
- **Config** : lecture/écriture de `kyber_config.toml` en préservant les commentaires (`toml_edit`), validation par schéma, application avec redémarrage contrôlé du service Kyber.
- **Service** : start / stop / restart / état du service Kyber.
- **Supervision** : GPU, pilote, charge de l'encodeur (NVML), session active, logs en direct.
- **Appairage** : page « Appareils » de la console : QR code et lien `kybernow://adresse:port?name=…&fp=sha256:…` (adresse, port de la console, empreinte de son certificat ; aucun secret). L'app lit le QR code à la caméra du Mac ou le lien collé (ou ouvert : schéma `kybernow://`). L'empreinte vue est comparée à celle du lien ou de l'annonce ; par simple adresse, l'utilisateur la compare à celle de la console (4 premiers groupes). Puis compte de la console, vérifié et gardé dans le Trousseau ; certificat épinglé.
- **Découverte** : annonce Bonjour/mDNS `_kybernow._tcp` de la console (TXT `v`, `name`, `fp`), règle de pare-feu UDP 5353 pour kyber-admin.
- **Profils** : « Jeu » (faible latence, manette, haut fps) / « Bureau » (4:4:4, presse-papiers, multi-écrans).

### Écran virtuel (session de stream)
- **Début de session** : le client annonce sa résolution et sa fréquence à kyber-admin (nos apps) ou on prend le mode par défaut ; kyber-admin crée l'écran virtuel SudoVDA à ce mode, mémorise la disposition des écrans, n'active que l'écran virtuel, et la capture Kyber le suit.
- **Fin de session** : écran virtuel retiré, disposition d'origine restaurée.
- **Filet de sécurité** : le pilote retire l'écran s'il ne reçoit plus de signal de vie (3 s) ; kyber-admin restaure la disposition à son démarrage et quand plus aucun flux n'est encodé ; raccourci clavier d'urgence.
- ✅ **Validé le 2026-10-19** sur le PC (RTX 3080 Ti, 2 écrans 4K) : écran virtuel 3840 × 2160 à 160 Hz créé avant la connexion, écrans physiques éteints puis restaurés, stream reçu à 152 i/s, 8,3 ms de latence médiane.
- **Piège connu** : l'énumérateur d'écrans de Kyber voit l'ancienne disposition pendant ~50 ms ; `prepare` attend que Windows n'affiche plus que l'écran virtuel, puis 1,5 s, avant de répondre. Les apps devront choisir l'écran d'après la liste mise à jour.
- **Reste à valider** : le mode « Pour toute connexion » (écran créé en cours de flux), et la reprise de main par un 2e appareil (déconnexion du 1er).
- **Piège corrigé** : le chien de garde de SudoVDA (3 s) est armé dès la création de l'écran. Le signal de vie part donc d'un fil dédié dès `add`, pas seulement une fois la session active : sinon l'écran disparaissait pendant la mise en place (écran blanc côté client, échec d'initialisation de l'encodeur côté serveur).
- **Architecture** : le service pilote SudoVDA (IOCTL, signal de vie) ; un agent éphémère `kyber-admin display-agent`, lancé avec le jeton SYSTEM dans la session console, change la disposition (API CCD).
- **Concession de sécurité** : SudoVDA est auto-signé, son certificat est ajouté aux autorités racines du PC.

### Qualité d'image : 4:4:4, HDR, VRR (Kyber 0.29)
- **4:4:4** ✅ validé le 2026-10-19 (HEVC, 3840 × 2160 à 160 Hz, client `--444`). Coût par rapport au 4:2:0 : encodage NVENC ~9 ms en médiane (p95 46 ms), décodage ~6,6 ms, latence totale ~18 ms en médiane. À proposer dans le profil « Bureau » (texte net), pas dans le profil « Jeu ».
- **HDR** ❌ impossible avec Kyber 0.29 : la capture (DXGI Desktop Duplication) est en BGRA 8 bits. Garder le HDR Windows désactivé sur l'écran virtuel.
- **Son multicanal et HD** (correctif `server/kyber-patches/0001`) : le moteur txproto capture déjà la sortie Windows avec tous ses canaux ; le débit Opus, fixé à 128 kbit/s par Kyber, suit désormais le nombre de canaux. Réglage `[kyavserver] audio_quality` : `standard` (128 kbit/s), `hd` (défaut : 256 kbit/s en stéréo, 576 en 5.1, 768 en 7.1), `studio` (maximum d'Opus) ; `audio_bitrate` pour une valeur exacte. Le moteur libavconv reste en stéréo. L'app propose Auto, Stéréo, 5.1, 7.1 ou Aucun ; Auto suit la sortie audio du Mac.
- **VRR** : côté serveur, Kyber envoie une image quand l'écran change (débit d'images suivant le contenu). Côté client, l'app présente chaque image dès qu'elle est décodée (Metal, au plus une image en attente d'écran, la plus récente remplace les autres) : sur un écran ProMotion / Adaptive-Sync, l'écran se cale sur l'arrivée des images. Synchro verticale désactivable (latence minimale, déchirure possible). Pas de VRR sur Apple TV.
- **Décodage par l'app (2026-10-20)** : correctif `apps/macos/kyber-patches/0001` sur libkyclient (`kyclient_video_player_config_set_packet_sink`) : les paquets vidéo (Annex B, jeux de paramètres, image clé, pts) vont à l'app au lieu de VLC. KyberKit les décode avec VideoToolbox (matériel exigé, mode temps réel, sortie dans les formats compressés sans perte des puces Apple, lus sans copie par Metal) et les dessine avec Metal (conversion YCbCr → RGB selon la matrice du flux). Mesuré hors ligne sur M4 Max en 2560 × 1440 : 2,0 ms de décodage en 4:2:0, 2,7 ms en 4:4:4. VLC ne sert plus qu'au son et à l'AV1. Code sans AppKit (décodeur, rendu, capacités) : repris tel quel sur iOS/tvOS.
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
- **État (2026-10-19)** : `apps/macos` (XcodeGen, `project.yml`). KyberKit = façade Swift de libkyclient/libkynput (module C `CKyber`), vue de session (image décodée par l'app et dessinée avec Metal ; VLC pour l'AV1), clavier (kVK → HID → scancode), souris absolue, curseur du PC affiché en `NSCursor` et jamais masqué en usage bureau. App : ajout manuel, mot de passe dans le Trousseau, profils Jeu/Bureau/Max, `prepare` (écran de ce Mac plafonné à la 4K, canaux de sa sortie audio), signal de vie, fin propre (⌃⌥⌘D), plein écran (⌃⌥⌘F). Avant le premier build : `scripts/vendor-kyber.sh`, puis `scripts/build-kyclient.sh` (libkyclient avec les correctifs KyberNow), puis `xcodegen generate`.
- **KyberKit** : `libclient` + `libkynput` + VLC compilés en xcframework (macOS arm64/x86_64, tvOS arm64 + simulateur), façade Swift (connexion, surface vidéo, entrées, métriques).
- **App** : liste des hôtes (Bonjour + ajout manuel), appairage, plein écran, réglages (résolution, fps, codec, débit, profil), manettes (framework GameController), clavier, souris et presse-papiers sur macOS, overlay de métriques.

## Risques à lever d'abord (prototypes rapides)

- ✅ **A. Référence de bout en bout** (validé : 9,4 ms médian, voir docs/prototype-A-results.md) : Kyber 0.29 sur le PC Windows ↔ client macOS en ligne de commande ; mesure de la latence et de la stabilité.
- **B. tvOS** : compiler le SDK client Rust (cible tvOS, moins bien supportée par Rust que macOS) et la couche VLC du fork Kyber pour tvOS. Solution de repli : demander l'accès à `kyber-ios` ou à leur couche Swift.
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
