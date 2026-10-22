# Propositions du 22 octobre 2026 : HDR, latence Apple TV, app au démarrage

Trois sujets étudiés à la demande du 22 octobre 2026, sources à l'appui : le dépôt KyberNow (0.4.3), et les sources de Kyber 0.29.0 telles que KyberNow les compile (kymedia 0.29.0, txproto `7f84b9e`, libavconv `5ca80cc`, kysdk `ebca32a` avec kyctl `92c632f` et kymux `af10dbb`, FFmpeg n8.1 et nv-codec-headers 12.1.14 d'après les `wrap` de kymedia). Rien n'a été compilé ni mesuré : ce document propose, chiffre et pose les questions à trancher avant de coder.

## 1. HDR : ce qui manque dans Kyber, ce que coûterait un correctif

### Constat dans les sources

| Étape | Moteur historique (txproto) | Nouveau moteur (libavconv) |
|---|---|---|
| Capture | `IDXGIOutput1::DuplicateOutput` (`src/iosys_dxgi.c:804`), réserve de textures D3D11 déclarée en `AV_PIX_FMT_BGRA` (`:743-744`), copie de la texture acquise dans la réserve | Même appel (`libavgrab/dxgi.c:551`), même réserve BGRA (`:373-374`), et **refus explicite** de tout autre format que `B8G8R8A8_UNORM` (`:563`) |
| Conversion | Aucune : la texture BGRA va telle quelle à l'encodeur (`src/encode.c:103-107` reprend format et étiquettes de couleur de la source, `:172` réutilise la réserve D3D11 si l'encodeur accepte le format) | Aucune sous Windows : graphe de filtres `null` pour NVENC et AMF (`kyavservice/src/avconv/video.rs:389-482`), `hwmap`/`scale_qsv` pour QSV |
| Encodage | NVENC `preset p2, tune ull, zerolatency, intra-refresh, g 120`, `rgb_mode = yuv444` pour le 4:4:4, aucun profil 10 bits (`kyavservice/src/txproto/video.rs:141-213`) | Mêmes options (`avconv/video.rs:546-580`), AMF/QSV/x264 à côté |

Deux conséquences :

- **Sur un bureau Windows en HDR, l'image capturée est déjà dégradée** : l'API `DuplicateOutput` d'origine rend une version SDR 8 bits du bureau (Windows la ramène lui-même), le HDR est écrasé avant même l'encodeur. L'API `IDXGIOutput5::DuplicateOutput1` avec `DXGI_FORMAT_R16G16B16A16_FLOAT` donne la vraie image (scRGB linéaire, 16 bits flottants). C'est le seul moyen.
- **Donner du RGB 10 bits à NVENC ne suffit pas.** FFmpeg 8.1 accepte bien `X2BGR10` (texture D3D11 `R10G10B10A2`, `libavutil/hwcontext_d3d11va.c:100`) et `RGBAF16` (`R16G16B16A16_FLOAT`, `:101`), et NVENC l'encode en 10 bits (`nvenc.c:76-77, 2020-2023`). Mais pour toute entrée RGB compactée, FFmpeg force la matrice VUI à BT.601 et la plage limitée (`nvenc.c:1277-1282` pour H.264, `:1468-1473` pour HEVC, `:1666-1670` pour AV1) : NVENC fait la conversion RGB → YCbCr lui-même, avec cette matrice, quelles que soient les étiquettes. Un flux HDR10 exige BT.2020 non constant + PQ + primaires BT.2020. Il faut donc faire la conversion nous-mêmes, comme Sunshine et Apollo.

Ce qui est déjà en place et ne change pas :

- **Transport** : les informations de couleur (VUI) et les métadonnées HDR statiques (SEI) voyagent dans le flux HEVC ou AV1 lui-même. L'en-tête de configuration de kymux (`CodecPacketHeader { codec, rotation, frame_size }`, `kymux-types/src/av.rs:52`) n'a rien à apprendre.
- **Rendu des apps Apple** : `VideoRenderer` lit déjà les plans 10 bits (`x420`, `VideoRenderer.swift:539-544`, textures `r16Unorm`/`rg16Unorm`) et la matrice BT.2020 (`:564`). Il manque la sortie : `bgra8Unorm` et sRGB (`:152, :185-188`).
- **Décodeurs** : HEVC Main 10 en matériel sur Apple TV 4K (A12 et suivantes), sur toutes les puces M, sur toute carte NVIDIA depuis Pascal, AMD VCN, Intel Kaby Lake. Pas de H.264 10 bits sur NVENC ni AMF : HDR = HEVC ou AV1 seulement. Le 4:4:4 10 bits n'a pas de format D3D11 que NVENC accepte (`yuv444p16` n'existe qu'en mémoire centrale) : HDR en 4:2:0 d'abord.
- **Écran virtuel** : SudoVDA gère le HDR (registre `hdrBits`, 10 par défaut), mais « seulement complètement sur Windows 11 24H2 », « pas sur Windows 10 » (README de SudoVDA). Admin devra activer le HDR de l'écran virtuel à la demande (API CCD, `DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE`, ou `DISPLAYCONFIG_SET_HDR_STATE` en 24H2) et le laisser éteint sinon.

### Correctif proposé (les deux moteurs, un seul shader)

1. **Capture FP16** quand la session le demande : `DuplicateOutput1` avec `[R16G16B16A16_FLOAT, B8G8R8A8_UNORM]`, réserve D3D11 en `AV_PIX_FMT_RGBAF16`. Sans HDR demandé : rien ne change (ancienne API, BGRA). Un fichier C par moteur (`iosys_dxgi.c`, `libavgrab/dxgi.c`), option transmise comme `cursor_move_rate` l'est déjà (0006, 0008).
2. **Conversion dans le nœud de capture**, juste après la copie de la texture : un compute shader HLSL commun (scRGB linéaire × 80/10 000 nits → primaires BT.2020 → PQ → Y'CbCr BT.2020 non constant, plage limitée, 10 bits → deux plans P010, chroma moyennée 2 × 2), compilé à l'exécution par `d3dcompiler_47.dll` (présente sur tout Windows 10/11, comme Sunshine : rien à changer dans la chaîne MinGW). La réserve de sortie est en `AV_PIX_FMT_P010`, les images étiquetées `BT2020` / `SMPTE2084` / `BT2020_NCL` / plage MPEG. Le filtre `scale_d3d11` de FFmpeg 8.1 (`libavfilter/vf_scale_d3d11.c`) sort bien du P010 par le VideoProcessor, mais sans aucun réglage d'espace de couleur (pas de `SetStreamColorSpace1`) : inutilisable tel quel, et l'étendre déplacerait le travail dans FFmpeg plutôt que dans Kyber.
3. **Encodeurs** : NVENC, AMF et QSV acceptent tous des images D3D11 P010 (`nvenc.c:62, 83`, `amfenc.c:113, 118`, `qsvenc.c:1988`) : profil Main 10 implicite en HEVC, AV1 10 bits sur Ada. Le SDR 8 bits reste exactement le chemin actuel. x264 exclu (4K60 10 bits sur processeur n'est pas réaliste).
4. **Drapeau de session** `hdr: bool` sur le chemin de `yuv444` : `VideoSessionConfig` de kyclient (`kyclient/src/lib.rs:835`, API C `capi.rs:866`), requête HTTP de kycontroller (`kymux_controller/mod.rs:369`), `VideoStreamConfig` (`avservices_manager.rs:401`), IPC `KymuxStartVideo` (`kyavservice-types/src/lib.rs:62-69`), `VideoConfig` des deux moteurs. Une cinquantaine d'endroits, mécaniques.
5. **Admin** : `prepare` reçoit `hdr`, active le HDR de l'écran virtuel après sa création, le remet à la fin ; refus clair avant Windows 11 24H2 ; page Image et son : « HDR : jamais / si l'appareil le demande ». Le curseur Windows « luminosité du contenu SDR » s'applique à l'image comme sur un vrai écran.
6. **Apps** : Mac, Apple TV, iPhone : `CAMetalLayer` en `rgba16Float` (ou `bgr10a2Unorm`), `colorspace` ITU-R 2100 PQ, `wantsExtendedDynamicRangeContent`, `edrMetadata` ; le shader écrit directement les valeurs PQ (matrice BT.2020 déjà là), Core Animation fait le reste. Sur l'Apple TV, demander au téléviseur de passer en HDR par `AVDisplayManager` avec une description de format HEVC portant les extensions de couleur PQ/BT.2020 (le seul initialiseur documenté est `init(refreshRate:formatDescription:)`, tvOS 11.2 ; réglage « Adapter la plage dynamique » de tvOS requis). App Windows : chaîne d'échange `R10G10B10A2`, `SetColorSpace1(RGB_FULL_G2084_NONE_P2020)`, `SetHDRMetaData`, textures P010 (`render.rs:238, 374-381`). Client web : WebCodecs décode le HEVC 10 bits dans Chrome, mais aucun canvas HDR exploitable : exclu, le serveur n'envoie du HDR qu'à qui le demande.
7. **Métadonnées statiques** (mastering display, MaxCLL) : facultatives pour nos propres clients (le PQ se détecte sur l'image décodée) ; à ajouter plus tard en SEI ou par Admin (`IDXGIOutput6::GetDesc1` donne la luminance de l'écran) si un téléviseur les exige.

### Coût

| Bloc | Estimation | Remarque |
|---|---|---|
| Capture FP16, deux moteurs | 1,5 j | C, chemin SDR intact |
| Shader + intégration dans les deux captures | 3 j | 1 j de shader, 1 j par moteur ; Sunshine (GPL v3, compatible AGPL) sert de référence |
| Encodeurs 10 bits et étiquettes VUI | 1 j NVENC, +1 j AMF/QSV | AMF et QSV « compilés, non vérifiés » : pas de carte AMD ni Intel sous la main |
| Drapeau de session (kyclient, kycontroller, types, kyavservice) | 1 j | Correctif serveur et correctif client |
| Admin (écran virtuel HDR, `prepare`, réglage, garde 24H2) | 1,5 j | API CCD déjà maîtrisée dans `display/` |
| Apps Apple (rendu EDR, bascule du téléviseur) | 2 j | Une seule `VideoRenderer` pour les trois apps |
| App Windows | 1,5 j | |
| Mesures sur le PC (débit, latence 10 bits, pics NVENC) | 2 j | +15 à 25 % de débit à qualité égale |
| **Total** | **≈ 14 j** (dont ≈ 7 j de correctif Kyber) | 2 à 3 semaines calendaires avec le matériel |

### Est-ce possible ici ?

Écrire, oui : les sources de Kyber sont lisibles depuis cette session (clonées pour cette étude), les correctifs C, HLSL et Rust peuvent être produits et vérifiés applicables (`patch --dry-run` sur les sources exactes de 0.29). Compiler et vérifier, non : la chaîne MinGW/meson décrite dans [build-notes.md](build-notes.md) vit sur le Mac, et l'essai exige le PC avec sa carte NVIDIA, Windows 11 24H2 et un écran ou téléviseur HDR. Le découpage naturel : une série `0016-txproto-capture-hdr`, `0017-libavgrab-capture-hdr`, `0018-kyavservice-hdr`, `0019-kycontroller-hdr`, côté client `0004-kyclient-hdr`, produite ici, compilée par `build.sh` sur le Mac, essayée sur le PC. Les deux correctifs de capture sont remontables à Kyber tels quels (la série `upstream/` existe déjà).

### Questions

1. Périmètre de départ : profil Jeu en HEVC 4:2:0 10 bits sur Apple TV et Mac, ou aussi le Bureau (le 4:4:4 10 bits demande un autre chemin d'encodage) ?
2. L'ordinateur est-il en Windows 11 24H2 ? Sans cela, SudoVDA ne garantit pas la bascule HDR, et l'étude ne peut pas être vérifiée.
3. Le téléviseur de l'Apple TV et l'écran du Mac sont-ils HDR (et lequel : HDR10, Dolby Vision, XDR) ? Cela décide des premiers essais.
4. Tone mapping côté serveur : aucun (valeurs au-delà de 10 000 nits écrêtées, comme Sunshine), ou une courbe simple ? Je propose aucun.
5. Veut-on l'option « HDR » par profil dans les apps, ou un réglage par ordinateur « HDR quand l'écran de l'appareil le permet » (automatique) ? Je propose l'automatique, avec un interrupteur dans Avancé.
6. Remonter les deux correctifs de capture à Kyber dès qu'ils tournent, ou attendre la série complète ?

## 2. Latence sur l'Apple TV

### Ce que fait la chaîne aujourd'hui

- **Capture** à la fréquence du téléviseur : `prepare` demande un écran virtuel à 60 Hz au plus (`Platform.swift:15, 22`), donc Kyber capture à 60 images par seconde au mieux : un changement à l'écran attend en moyenne 8,3 ms avant d'être capturé.
- **Réseau** : profil Jeu en transport rapide (FEC), file de réassemblage qui attend jusqu'à 50 ms en cas de perte (`kyproto/.../video_unreliable_fec.rs:457`), réception sur l'unique fil de kyclient (`mainloop_engine.rs:46`), mémoire de socket à 8 Mo (correctif client 0003, appliqué pour tvOS aussi).
- **Décodage** VideoToolbox asynchrone, temps réel ; **rendu** Metal « la plus récente gagne », sans garde d'image en vol ; `present` puis Core Animation montre l'image au balayage suivant, **à 60 Hz fixes** : 8,3 ms d'attente en moyenne, 16,7 au pire, pas de VRR sur Apple TV.
- **La latence affichée sur Apple TV n'est pas mesurée** : faute d'instant de présentation, le rendu suppose l'image montrée 8,3 ms après la fin du GPU (`VideoRenderer.swift:295-300`). Le chiffre est une estimation, sans doute optimiste d'au moins un balayage de composition.
- **Composition** : la couche Metal est sous un `.overlay` SwiftUI permanent (un `ultraThinMaterial` et un noir, tous deux à opacité 0 menu fermé, `SessionView.swift:30-37`) et sous le `Passage` (`:42-45`). Sur iOS et tvOS, une couche Metal plein écran, opaque, sans rien par-dessus peut partir directement à l'écran sans passer par le compositeur ; une couche transparente par-dessus suffit à l'en empêcher. Un balayage de plus, c'est 16,7 ms à 60 Hz.
- **Après l'Apple TV** : le téléviseur lui-même (10 ms en mode Jeu, 30 à 80 ms sinon) ; l'Apple TV n'annonce pas l'ALLM, le mode Jeu se règle à la main sur le téléviseur.
- **Entrées** : GameController sur une file série (fait en 0.3.x), axes regroupés par 4 ms, Bluetooth à la cadence de la manette. Rien de gros à gagner.
- **Son** : tampon d'entrée/sortie demandé à 5 ms, sortie HDMI par le téléviseur (son retard à lui).
- **À noter** : d'après les README, l'app Apple TV sur le cœur Rust (0.3.1 et suivantes) est installée mais n'a pas encore tourné sur une vraie Apple TV. L'impression de latence vient donc des versions 0.2.x en Swift, qui gardaient la garde « une image en vol » et le décodage synchrone.

### Propositions, par gain attendu

| # | Proposition | Gain estimé | Coût | Risque |
|---|---|---|---|---|
| L1 | **Mesurer pour de vrai** : essayer `addPresentedHandler` sur l'Apple TV (l'API existe depuis tvOS 10.3 ; le code actuel range tvOS avec le simulateur d'iOS, sans doute faute d'essai sur l'appareil) ; sinon caler la mesure sur un `CADisplayLink`. Puis Instruments (Metal System Trace) pour voir si la couche est composée ou directe | Aucun en soi, mais tout le reste en dépend | 0,5 j | nul |
| L2 | **Capture à 120 Hz pour un téléviseur à 60 Hz** : écran virtuel à 120 Hz (SudoVDA le fait), l'Apple TV montre à chaque balayage l'image la plus récente. Capture : −4 ms en moyenne ; affichage : l'image montrée a au plus 8,3 ms au lieu de 16,7, −4 ms en moyenne ; et le jeu lui-même rend à 120 i/s s'il le peut | **≈ −8 ms** en moyenne, de bout en bout | 1 j (réglage de l'app, `prepare` à 120 Hz, mesures) | Décodage à 120 i/s : en flux P sans image B, aucune image ne peut être sautée avant décodage ; si l'A15 ne tient pas le 4K à 120, retomber à 1440p120 ou à 60. Débit et charge d'encodage doublés |
| L3 | **Couche Metal seule à l'écran** menu fermé : retirer de la hiérarchie (et non mettre à opacité 0) le voile, le matériau et le `Passage` une fois ouvert ; ne poser les capsules que quand elles sont visibles | jusqu'à **−16,7 ms** si Instruments montre une composition | 1 j | aucun si la mesure L1 est faite avant |
| L4 | **Mode Jeu du téléviseur** : le dire dans l'app (carte « Prêt à jouer », première session) et dans le guide du site ; garder « Caler le téléviseur sur la session » actif (évite un téléviseur en 50 Hz en Europe) | 20 à 70 ms sur un téléviseur mal réglé | 0,5 j | nul |
| L5 | **Réseau** : Ethernet (Apple TV 4K 128 Go) plutôt que Wi-Fi ; correctif serveur qui ramène l'attente de la FEC de 50 ms à deux intervalles d'image sur le LAN (déjà noté dans l'audit, §3) ; vérifier que le profil Jeu de l'Apple TV est bien en transport rapide | 0 à 40 ms sous pertes | 1 j | correctif kymux à remonter |
| L6 | **GPU tenu éveillé** côté serveur (fréquences NVENC verrouillées pendant la session, ou image de maintien) : efface les pics d'encodage à 100 ms après un écran immobile (audit, §3) | p95 | 1 j | NVML |
| L7 | **Décodage sur l'Apple TV** : relire le temps de décodage réel dans le HUD (4K HEVC sur A15 : 4 à 8 ms attendus) ; si trop haut, profil Jeu en 1440p sur Apple TV | selon mesure | 0 | |

Le cœur est déjà partagé : L1, L3 et L5 profitent au Mac et à l'iPhone.

### Questions

1. Quelle Apple TV (2021 A12, 2022 A15, Wi-Fi ou Ethernet) et quel téléviseur (fréquence, mode Jeu activé ?) ?
2. Avec quelle version la latence paraissait élevée (0.2.x en Swift, ou le cœur Rust) ? Que disait le HUD : latence, décodage, images par seconde, pertes ?
3. En 4K ou en 1440p ? Un essai en 1080p60 donnerait tout de suite la part du décodage.
4. Accepte-t-on, pour l'expérience L2, de doubler le débit (profil Jeu à 120 Mbit/s en 4K) ?

## 3. Une app ouverte quand un appareil se connecte

### Où ça se branche

Admin connaît déjà chaque appareil (`devices.json`, `activity.rs:66-84`, champs ajoutés avec `#[serde(default)]` sans migration) et déroule `prepare` dans l'ordre son → veille du plein écran → écran virtuel → session ouverte (`api.rs:730-795`). Le service vit en session 0 ; ce qui doit tourner devant l'utilisateur passe par `kyber-admin display-agent …` dans sa session (`display/agent.rs`), soit avec le jeton SYSTEM (écrans), soit **avec le compte de l'utilisateur** (`launch_as_user`, `:71-105`, comme l'icône). Une app de jeu doit tourner avec son compte : c'est ce second chemin.

### Proposition

**Données** : sur chaque appareil, `launch` : `{ kind: app | uri | path, target, name, icon, close_at_end: bool, if_not_running: bool }`. `kind: app` désigne une entrée du catalogue (identifiant d'application Windows, AUMID) ; `uri` une adresse comme `steam://open/bigpicture` ; `path` un exécutable ou un raccourci choisi à la main.

**Catalogue, sans chemin à taper** : un agent `display-agent apps` énumère `shell:AppsFolder` dans la session de l'utilisateur (tout ce que montre le menu Démarrer : apps classiques par leurs raccourcis, apps du Store par leur AUMID, jeux Steam ou Epic ajoutés au menu), avec nom, identifiant et icône (64 px, PNG), écrit dans `apps.json` et un dossier d'icônes, à la demande (« Actualiser ») et au premier affichage. Par-dessus, des **suggestions détectées** avec leur ouverture et leur fermeture : Steam Big Picture (`steam://open/bigpicture`, `steam://close/bigpicture`), Playnite plein écran (`Playnite.FullscreenApp.exe` dans `%LOCALAPPDATA%\Playnite`), Xbox (AUMID `Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App`), GOG Galaxy, Epic (`com.epicgames.launcher://…`), EA app, Ubisoft Connect, Battle.net, présents ou non sur l'ordinateur.

**Interface** (Accès › Appareils, menu de l'appareil → « Au début de la session ») : un sélecteur avec un champ de recherche, la rangée des suggestions, la grille de toutes les apps avec leurs icônes, et « Autre… » (explorateur minimal côté serveur depuis Programmes et Bureau, ou une adresse collée, en dernier recours). Deux interrupteurs : « Fermer l'app à la fin de la session » et « Seulement si elle n'est pas déjà ouverte ». La ligne de l'appareil affiche l'icône choisie.

**Exécution** : une fois l'écran virtuel en place (l'app s'ouvre ainsi sur le bon écran, à la bonne définition), `display-agent launch` avec le jeton de l'utilisateur : `ShellExecuteW` pour une adresse ou un raccourci, `IApplicationActivationManager::ActivateApplication` pour une app du Store ; si l'app tourne déjà, la ramener au premier plan. À la fin (`/api/stream/end`, reprise de main, signal de vie perdu) : l'adresse de fermeture si elle existe (Big Picture), sinon `WM_CLOSE` à la fenêtre principale des processus que nous avons lancés, jamais de `TerminateProcess` sur une app qu'on n'a pas lancée. Session Windows verrouillée : rien n'est lancé, une ligne dans le fil d'événements l'explique. La réponse de `prepare` dit à l'app ce qui s'ouvre (« Steam Big Picture s'ouvre sur l'ordinateur »).

**Coût** : agent catalogue + icônes 2 j ; données, API, lancement et fermeture 1,5 j ; sélecteur Svelte en deux langues 2 j ; essais sur le PC 1 j : **≈ 6,5 j**. Le catalogue sert ensuite aux « profils par jeu » de l'audit (§13).

### Questions

1. Un choix **par appareil** seulement, ou **par appareil et par profil** (Jeu → Big Picture, Bureau → rien) ? Je propose par appareil, avec « sauf en profil Bureau » comme simple case à cocher.
2. Faut-il aussi proposer le choix **depuis l'app Link** au moment de se connecter (liste d'apps à la Moonlight), ou Admin suffit-il ?
3. Fermer l'app à la fin : actif par défaut pour les suggestions qui savent se fermer (Big Picture), inactif pour le reste ?
4. Un appareil de type « tv » doit-il recevoir Steam Big Picture par défaut dès l'appairage, ou rien tant que l'administrateur n'a pas choisi ?
5. L'explorateur « Autre… » est-il voulu, ou le catalogue du menu Démarrer suffit-il (un programme qui n'y est pas se règle en l'y épinglant) ?

## Ordre proposé

1. Apple TV : L1 (mesurer) puis L2 et L3, qui tiennent dans une semaine et tranchent par la mesure.
2. App au démarrage : indépendante de Kyber, livrable en une version.
3. HDR : la plus longue, à lancer une fois les réponses aux questions 1 à 3 connues ; les correctifs de capture peuvent partir en premier, le reste de la chaîne suit.
