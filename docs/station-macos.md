# KyberNow Station pour Mac — cadrage

[English](station-macos.en.md) · Français

Cadré le 03/11/2026. Ce document fixe les choix, le découpage et l'état des jalons. Le prévu suit dans [ROADMAP.md](../ROADMAP.md).

## Le but

Un Mac à puce Apple devient une Station comme un PC Windows : mêmes fonctions, même Admin, même icône, et les apps KyberNow Link s'y connectent sans savoir (presque) qu'il s'agit d'un Mac. Pour le jeu (Steam pour Mac, CrossOver, Game Porting Toolkit) comme pour le bureau à distance, avec les profils Jeu, Bureau et Max.

## État

**Jalon 1, en cours (03/11/2026)** : serveur Kyber compilé pour la puce Apple avec les 30 correctifs serveur de Windows, sans en changer un (`server/macos/scripts/build-kyber.sh`, pose commune `server/kyber-patches/apply.sh`), lancé en LaunchAgent signé par `server/macos/scripts/run-kyber.sh`. Essayé sur le M4 Max, en local avec le client Kyber de ce Mac :

- les trois écrans du Mac trouvés par ScreenCaptureKit, une fois l'enregistrement de l'écran autorisé pour « KyberNow Station (Kyber) » ;
- image en HEVC par VideoToolbox (environ 29 Mbit/s mesurés pour 20 demandés sur un écran très animé, kyavserver à 3 % de processeur), son en Opus stéréo à 256 kbit/s (sortie de l'écran choisi), image clé à la demande (0005) ;
- HEVC 4:4:4 : l'encodeur s'ouvre (image 4:4:4 réellement reçue : à vérifier avec une app KyberNow) ;
- H.264 : refusé par VideoToolbox sur l'écran principal (−12903), sans doute au-delà de 4 096 de large en pixels réels ; l'écran virtuel à la définition de l'appareil l'évitera ;
- `kVTCompressionPropertyKey_MaxFrameDelayCount` n'est pas pris en compte par l'encodeur HEVC : VideoToolbox peut garder des images en réserve, à mesurer avec la latence ;
- le client Kyber (lecteur VLC) manque souvent d'échantillons de son : à revoir avec le moteur de son du cœur.

Reste pour finir le jalon : une image sur un autre appareil (les apps KyberNow passent par Admin, jalon 2 ; ou le client Kyber d'origine sur le PC), la latence de bout en bout, les entrées (kynputserver, autorisation Accessibilité).

**Jalon 2, en cours (03/11/2026)** : Admin sur Mac. `kyber-admin` reconnaît une Station Mac (LaunchAgent de Kyber), pilote Kyber par launchd, s'annonce avec `kyber=1` sous le nom du Mac, donne ses vraies cartes réseau pour le réveil, et `prepare` prend l'écran du Mac tel qu'il est (pas d'écran virtuel ni de son virtuel avant les jalons 3 et 4) ; détails dans [server/README.md](../server/README.md#kybernow-station-pour-mac-en-cours). Station d'essai : `server/macos/scripts/run-station.sh` (certificat propre à la Station, comptes posés par `kyber-admin init`). Vérifié sur le M4 Max : annonce Bonjour, `info`, `prepare`, puis session Kyber avec le compte et l'empreinte donnés par Admin ; **image reçue sur l'iPhone par l'app KyberNow** (03/11/2026 : écran principal du Mac en HEVC, 70 Mbit/s demandés, son Opus stéréo). Vu pendant cet essai : le premier flux s'est coupé au bout de 4 s puis l'app l'a relancé (« Force IDR failed: Timeout » côté Kyber), à comprendre ; la souris et le clavier sont refusés par macOS (`CGPostMouseEvent` → 1002 : pas d'autorisation Accessibilité, jalon 5) ; l'aperçu de l'accueil échoue (« could not create image from display » : Admin n'a pas l'enregistrement de l'écran). Reste : l'aperçu de l'accueil, la supervision de la puce et le jeton local de l'icône (encore ceux du Mac de développement).

**Jalon 5, en cours (03/11/2026)** : souris et clavier. Correctif Kyber propre au Mac `server/macos/kyber-patches/0002` (injection par `CGEvent`, voir [kyber-changes.md](kyber-changes.md)) ; autorisation Accessibilité donnée par l'auteur à « KyberNow Station (Kyber) ». **Souris et clavier vérifiés depuis le PC** (03/11/2026). Piège trouvé pendant l'essai : un Mac branché deux fois au même réseau (Ethernet et Wi-Fi) répondait en UDP depuis l'adresse de sa route principale ; le PC, qui visait l'adresse Wi-Fi (gardée depuis l'appairage), rejetait ces réponses et ne recevait jamais l'image. Cause : quinn-udp, la bibliothèque QUIC de Kyber, demande l'adresse de départ par `IPV6_PKTINFO` (socket double pile) ou `IP_RECVDSTADDR`, que macOS ignore à l'envoi ; seul `IP_PKTINFO` est respecté (essai du 03/11/2026). Correctif `server/macos/crates/quinn-udp-0.5.14-source-mac.patch` : Kyber répond depuis l'adresse appelée, quelle qu'elle soit. En plus, Admin n'annonce que l'adresse de la route principale (l'Ethernet avant le Wi-Fi, comme le veut l'auteur), relue toutes les 15 s. **Vérifié depuis le PC avec le Wi-Fi allumé** (03/11/2026 : session par l'adresse Wi-Fi, image, souris et clavier). Mais toute la session restait alors sur le Wi-Fi : `prepare` donne désormais `kyber_host`, l'adresse Ethernet, à un appareil du même réseau, et le cœur des apps y prend l'image (Admin reste joint par l'adresse retenue) ; à essayer avec une app Windows compilée depuis cette branche. Pointeur : forme et position annoncées aux apps par kynput (correctif Mac `0003`, voir [kyber-changes.md](kyber-changes.md)), lues sur le vrai pointeur depuis un fil (56 × 80 pixels à l'échelle Retina) ; **vérifié depuis le PC** (03/11/2026 : pointeur visible, à sa place, formes suivies) ; pointeur masqué (jeu, frappe) suivi par `CGCursorIsVisible`, essayé avec une app qui le cache par `NSCursor.hide`, `CGDisplayHideCursor` puis `setHiddenUntilMouseMoves` (frappe) ; en souris du bureau, l'app de l'appareil garde son pointeur, comme face à un PC : le pointeur caché sert au mode Auto, avec le plein écran qu'Admin voit désormais sur Mac (`foreground.rs`, fenêtre au premier plan qui couvre un écran, hors navigateurs et lecteurs). Presse-papiers : texte et HTML dans les deux sens, avec les droits du service (correctif Mac `0004`) ; aller-retour sur le vrai presse-papiers vérifié depuis un fil ; **vérifié depuis le PC** (03/11/2026, copier-coller dans les deux sens). Touches système : la touche ⊞ du PC est ⌘ sur le Mac ; ⊞+Tab restait à Windows (vue des tâches) : l'app Windows prend désormais toute touche tenue avec ⊞ (à essayer avec l'app de la prochaine version) ; ⌃⌘Q et ⇧⌘Q refusés aux invités et aux emprunts (correctif Mac `0005`, choix de l'auteur) ; le menu « Touches » des apps face à un Mac (⌘⇥, ⌘ Espace…) viendra avec le jalon 8, avec l'option « Ctrl agit comme ⌘ ». **Mac verrouillé vérifié depuis le PC** (03/11/2026) : session ouverte sur un Mac déjà verrouillé (⌃⌘Q), image de l'écran de verrouillage, mot de passe tapé depuis le PC qui déverrouille (journal de `loginwindow` : verrouillage 17:29:09, session 17:29:20, déverrouillage 17:29:40) ; verrouillé en cours de session, l'image reste. Rien à ajouter : Kyber tourne dans la session de l'utilisateur, que le verrouillage ne ferme pas. Hors champ, comme choisi : la fenêtre de connexion après une fermeture de session ou un redémarrage (plus de session, donc plus de Kyber). Le jalon 5 est fait, sauf le menu « Touches » et l'option « Ctrl agit comme ⌘ » des apps (jalon 8).

**Jalon 3, en cours (03/11/2026)** : écran virtuel. Admin lance l'agent `kybernow-display` (Swift, `server/macos/display-agent`, piloté par `display/mac.rs`) : écrans `CGVirtualDisplay` à la définition et à la fréquence de l'appareil, en Retina dès 2048 pixels de large (une interface lisible sur téléphone, tablette, Mac ou téléviseur 4K ; 1080p en 1x), jusqu'à 4 écrans placés comme ceux de l'appareil. Pendant la session, **les écrans du Mac recopient l'écran virtuel principal** au lieu de s'éteindre (choix de l'auteur, 03/11) : seul l'écran virtuel est actif et principal, en 0,4 s. Les éteindre (`CGSConfigureDisplayEnabled`, privé) n'était appliqué par macOS qu'avec plusieurs secondes de retard, parfois à la sortie du programme seulement. Filet de sécurité : l'écran virtuel vit avec l'agent, qui vit avec Admin, et les changements sont faits « pour ce programme seulement » ; si l'un s'arrête, macOS rend les écrans du Mac à leur place (vérifié, y compris une session sans signal de vie, close au bout de 20 s). Correctif Kyber propre au Mac (`server/macos/kyber-patches/0001`, voir [kyber-changes.md](kyber-changes.md)) : txproto suit les écrans qui apparaissent et disparaissent, et les annonce en pixels. Vérifié sur le M4 Max : `prepare` à 2868 × 1320 à 120 Hz en 1,8 à 2,3 s, Kyber n'annonce plus que l'écran virtuel (2868 × 1320 en (0, 0)) et le capture, changement de mode en session, fin de session. Pièges : un écran Retina se déclare en points (macOS en fait le mode Retina ; déclaré en pixels, il prend le double et son mode « moitié » n'est pas utilisable pour le bureau) ; un écran recopié garde son mode (la recopie est suspendue le temps d'un changement) ; à l'apparition de l'écran virtuel, macOS change aussi le mode de l'écran principal du Mac, rendu ensuite. **Session depuis l'iPhone à sa définition vérifiée par l'auteur** (03/11/2026 : écran virtuel prêt en 1,6 s, 2868 × 1320 à 120 Hz, image du premier coup, HEVC à 56 Mbit/s, écrans du Mac rendus à la fin). Reste : le multi-écrans depuis un Mac à plusieurs écrans, le HDR des écrans virtuels (proposé plus tard).

## Choix

| Sujet | Choix |
|---|---|
| Mac pris en charge | Puce Apple seulement (M1 et suivantes), macOS 15 Sequoia ou plus récent. Pas de Mac Intel |
| Moteur | Le serveur macOS de Kyber 0.29 (kycontroller, kyavserver avec txproto, kynputserver), avec nos correctifs. Pas de moteur à nous |
| Admin | Le même `kyber-admin`, compilé pour macOS, mêmes routes et même interface ; les parties propres à Windows reçoivent leur équivalent Mac derrière les mêmes modules |
| Écran virtuel | `CGVirtualDisplay`, API privée d'Apple (celle de BetterDisplay et DeskPad) : définition et fréquence de l'appareil, jusqu'à 4 écrans ; pendant la session, les écrans du Mac recopient l'écran virtuel (choix du 03/11/2026, à la place de les éteindre). Pas d'App Store, et une version de macOS peut la casser : on le surveille à chaque bêta |
| Son | Périphérique audio virtuel KyberNow dès la première version (plug-in Core Audio, 2 à 8 voies), comme les haut-parleurs virtuels sous Windows |
| Manettes | Pas dans la première version : clavier et souris. Le droit d'Apple pour un périphérique HID virtuel (`com.apple.developer.hid.virtual.device`) est demandé tout de suite ; les manettes arrivent dès qu'il est accordé |
| Mac verrouillé | On diffuse l'écran de déverrouillage et on peut taper le mot de passe. Après un redémarrage (FileVault), il faut ouvrir la session devant le Mac ou régler l'ouverture automatique |
| Première version | Tout ce que fait la Station Windows, sauf les manettes : écran virtuel et multi-écrans, son, profils, appairage, réveil, Share et relais, Prêt du PC, HDR, app au début de la session, mises à jour depuis Admin |
| Essais | Sur le Mac de l'auteur (M4 Max) en Station, vers l'Apple TV, l'iPhone, le PC et le téléviseur LG |

## Ce qui existe déjà

- **Kyber sait servir depuis un Mac** (0.29, « Add macOS server support ») : capture ScreenCaptureKit de l'image et du son (`txproto/src/iosys_screencapturekit.m`), encodage VideoToolbox (H.264, HEVC), clavier et souris injectés par `kynput/src/macos/host_keyboard_mouse.rs`, LaunchAgent `tech.kyber.server` dans la session de l'utilisateur et à l'écran de connexion. Le moteur libavconv n'a pas de capture Mac : txproto seulement.
- **Admin tourne déjà sur Mac** pour le développement, avec des pilotes simulés (écran virtuel, son, réveil, catalogue d'apps, jeton local, aperçu par `screencapture`, nom du système par `sw_vers`). Les modules propres à Windows sont derrière `cfg(windows)` : on remplace les simulations par du vrai.
- **L'icône** (`server/tray`) est faite de briques qui existent sur Mac (tao, tray-icon, wry) ; seuls ses appels au système sont pour Windows.
- **Les apps lisent déjà ce que la Station sait faire** dans `GET /api/stream/info` (`virtual_display.max_screens`, `speakers`, `hdr`, `wake`, `os`). Une Station Mac s'y décrit, et les apps s'adaptent sans savoir que c'est un Mac.

## Ce qui manque, brique par brique

| Brique | Windows | Mac |
|---|---|---|
| Services | Services `kyber` et `kyber-admin` (SYSTEM), agents lancés dans la session de l'utilisateur | Admin en LaunchDaemon (root) ; Kyber et un agent KyberNow en LaunchAgent dans la session de l'utilisateur et à l'écran de connexion |
| Capture | DXGI / WGC (txproto) | ScreenCaptureKit (txproto, existe) ; HDR par `captureDynamicRange` (macOS 15) |
| Encodeur | NVENC, AMF, QSV | VideoToolbox. Pas d'AV1 à l'encodage sur les puces Apple connues ; 4:4:4 en HEVC à vérifier par puce |
| Écran virtuel | SudoVDA + API CCD | `CGVirtualDisplay` dans l'agent ; écrans physiques éteints ou recopiés (API privée de configuration des écrans, à choisir aux essais) ; l'écran disparaît de lui-même si l'agent meurt, ce qui fait le filet de sécurité |
| Son | Haut-parleurs de Steam, `IPolicyConfig` | Plug-in Core Audio « KyberNow » (2, 6 ou 8 voies) choisi comme sortie le temps de la session, sortie d'origine rendue à la fin ; Kyber le lit en entrée Core Audio (correctif txproto) |
| Clavier, souris | kynput (SendInput) | kynput Mac repris (0002) : `CGEvent`, mouvements relatifs pour les jeux, défilement fin ; touches système (0005 : ⌃⌘Q et ⇧⌘Q refusés aux invités et aux emprunts, comme Win+L et Ctrl+Alt+Suppr sous Windows) ; autorisation Accessibilité |
| Pointeur | Formes annoncées par la capture (0002, 0006) | Forme et position du pointeur du système lues par kynput et annoncées comme sous Windows (correctif Mac 0003) |
| Presse-papiers | kynput | `NSPasteboard` dans kynput Mac (texte et HTML, dans les deux sens ; correctif Mac 0004) |
| Manettes | ViGEmBus | HID virtuel (droit d'Apple) — plus tard |
| Supervision | NVML, compteurs « GPU Engine » | Nom de la puce, charge de l'encodeur et du GPU (IOReport), sessions VideoToolbox |
| Réveil | WoL, `powercfg` | WoL si « Réveil pour l'accès au réseau » (`pmset womp`), cartes lues par `getifaddrs` |
| Aperçu de l'accueil | Agent, GDI | Agent, `SCScreenshotManager` |
| App au début | Menu Démarrer, `ShellExecuteEx` | `/Applications` et leurs icônes, `NSWorkspace` ; suggestions Steam Big Picture, etc. |
| Jeu en plein écran (souris Auto) | Agent | Agent : app au premier plan en plein écran, hors navigateurs et lecteurs vidéo |
| Prêt du PC | Mode console dans la session de l'hôte | Même principe dans la session de l'hôte |
| Installation | Installateur (Rust, WebView2) | Paquet `.pkg` signé Developer ID et notarisé, puis une fenêtre d'accueil qui fait donner les autorisations (Enregistrement de l'écran, Accessibilité) |
| Mises à jour | Manifeste minisign, applicateur SYSTEM | Même manifeste, applicateur root, même retour arrière |

## Les autorisations de macOS

- **Enregistrement de l'écran** et **Accessibilité** : données une fois, à l'installation, par la fenêtre d'accueil qui explique pourquoi.
- **Rappel mensuel de macOS 15** pour les apps qui enregistrent l'écran : à demander à Apple, le droit `com.apple.developer.persistent-content-capture` (prévu pour le bureau à distance) l'évite. Sans lui, la Station doit le dire clairement dans Admin et dans l'icône.
- **Écran de connexion** : la capture et la frappe y passent par un processus lancé à l'écran de connexion (`LimitLoadToSessionType` `LoginWindow`, comme le LaunchAgent de Kyber) : à vérifier en premier, c'est la part la moins sûre.

## Ce que voient les apps KyberNow Link

Le moins de changements possible, et seulement par ce que la Station annonce :

- `GET /api/stream/info` donne déjà `os` (« macOS 26.1 ») ; il gagne `gamepads` (faux tant que les manettes manquent). Les apps de salon disent alors « Manettes indisponibles sur cet ordinateur » au lieu de proposer la ligne des manettes.
- **Clavier** : un PC, un téléphone ou un téléviseur qui se connecte à un Mac envoie Ctrl là où le Mac attend ⌘. Une option « Ctrl agit comme ⌘ », proposée seulement face à une Station Mac, réglée par défaut côté cœur.
- **Réveil** : le texte d'aide parle du réglage du Mac au lieu de la carte réseau et du BIOS.
- Les règles et les textes sont d'abord écrits dans [link-ui.md](link-ui.md), puis repris dans toutes les apps.

## Découpage

Chaque jalon se termine par un essai réel sur le M4 Max.

1. **Kyber sur ce Mac.** Construire kycontroller, kyavserver (txproto) et kynputserver pour `aarch64-apple-darwin` avec nos correctifs, les lancer à la main et recevoir l'image sur l'Apple TV et le PC. Trier les correctifs : ceux de kycontroller, kynet et kyavservice valent pour tous (image clé, file d'envoi, débit par écran, partage, son HD…) ; ceux de txproto et libavgrab touchent surtout DXGI, WGC et WASAPI, à reprendre un par un pour ScreenCaptureKit. Mesurer encodage et latence en HEVC, et le 4:4:4.
2. **Admin sur Mac.** LaunchDaemon et LaunchAgents, redémarrage de la Station, supervision de la puce, réveil, aperçu, jeton local, nom du système : les simulations deviennent réelles. Appairage, Bonjour et client web marchent alors comme sous Windows.
3. **Écran virtuel.** Agent Swift avec `CGVirtualDisplay`, un à quatre écrans placés comme ceux de l'appareil, écrans du Mac éteints puis rendus, veille du mode (politique « Laisser faire » / « Remettre »).
4. **Son.** Plug-in Core Audio KyberNow, bascule de la sortie, lecture par Kyber, 5.1 et 7.1.
5. **Entrées.** kynput Mac revu (CGEvent, relatif, touches système), presse-papiers, pointeur, souris Auto ; écran de connexion.
6. **Icône, paquet, mises à jour.** Icône de la barre des menus, `.pkg` notarisé avec sa fenêtre d'autorisations, désinstallation, mise à jour depuis Admin.
7. **Fonctions avancées.** Share et relais, app au début de la session, Prêt du PC, HDR.
8. **Apps Link.** `gamepads`, option Ctrl/⌘, textes d'aide ; puis les manettes quand Apple aura donné son accord.

## À vérifier en premier

- Capture et frappe à l'écran de connexion d'un Mac verrouillé.
- Écrans physiques éteints pendant que `CGVirtualDisplay` tourne, puis rendus à l'identique ; quatre écrans virtuels à la fois.
- HDR d'un écran `CGVirtualDisplay` et sa capture.
- HEVC 4:4:4 par VideoToolbox selon la puce.
- Latence de ScreenCaptureKit à 120 Hz (profondeur de file, image envoyée dès qu'elle change).
- Réponse d'Apple pour le HID virtuel et pour le rappel mensuel d'enregistrement de l'écran.
