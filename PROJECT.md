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
- **Appairage** : code/QR affiché dans l'UI → le client reçoit un jeton (JWT signé avec la clé du serveur), stocké dans le trousseau Apple.
- **Découverte** : annonce Bonjour/mDNS (`_kyber._tcp`).
- **Profils** : « Jeu » (faible latence, manette, haut fps) / « Bureau » (4:4:4, presse-papiers, multi-écrans).

### Écran virtuel (session de stream)
- **Début de session** : le client annonce sa résolution et sa fréquence à kyber-admin (nos apps) ou on prend le mode par défaut ; kyber-admin crée l'écran virtuel SudoVDA à ce mode, mémorise la disposition des écrans, n'active que l'écran virtuel, et la capture Kyber le suit.
- **Fin de session** : écran virtuel retiré, disposition d'origine restaurée.
- **Filet de sécurité** : le pilote retire l'écran s'il ne reçoit plus de signal de vie (3 s) ; kyber-admin restaure la disposition à son démarrage et quand plus aucun flux n'est encodé ; raccourci clavier d'urgence.
- ✅ **Validé le 2026-10-19** sur le PC (RTX 3080 Ti, 2 écrans 4K) : écran virtuel 3840 × 2160 à 160 Hz créé avant la connexion, écrans physiques éteints puis restaurés, stream reçu à 152 i/s, 8,3 ms de latence médiane.
- **Piège connu** : l'énumérateur d'écrans de Kyber voit l'ancienne disposition pendant ~50 ms ; `prepare` attend que Windows n'affiche plus que l'écran virtuel, puis 1,5 s, avant de répondre. Les apps devront choisir l'écran d'après la liste mise à jour.
- **Reste à valider** : le mode « Pour toute connexion » (écran créé en cours de flux), et la reprise de main par un 2e appareil (déconnexion du 1er).
- **Architecture** : le service pilote SudoVDA (IOCTL, signal de vie) ; un agent éphémère `kyber-admin display-agent`, lancé avec le jeton SYSTEM dans la session console, change la disposition (API CCD).
- **Concession de sécurité** : SudoVDA est auto-signé, son certificat est ajouté aux autorités racines du PC.

### Installeur
- Kyber à une version épinglée (binaires repris depuis la release officielle), SudoVDA, ViGEmBus, kyber-admin.
- Règles de pare-feu, génération des certificats TLS, plan d'alimentation hautes performances (la doc Kyber mesure 2,5 ms d'encodage NVENC contre 8 ms quand le GPU économise de l'énergie).
- Désinstallation propre.

### Clients Apple
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
- HDR et 4K à 120 Hz nécessaires dès la v1 ?
- Plusieurs utilisateurs simultanés sur le même PC ?
