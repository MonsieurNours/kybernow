# KyberNow : le HDR

[English](hdr.en.md) · Français

Le HDR donne à l'image des couleurs plus riches et des lumières plus vives, sur un écran qui sait les afficher : l'écran XDR d'un Mac, un iPhone, un téléviseur HDR derrière une Apple TV, une Shield ou l'app du téléviseur. Les jeux et Windows en HDR passent tels quels jusqu'à cet écran.

Ce document est le contrat commun de la Station (Admin, Kyber), du cœur Rust et de toutes les apps Link. Les textes exacts sont dans [link-ui.md](link-ui.md), section « HDR ». Il remplace l'étude du 22/10/2026 ([archives/propositions-2026-10-22.md](archives/propositions-2026-10-22.md), section 1), dont il reprend les constats dans les sources de Kyber 0.29. État : **cadré le 02/11/2026, rien d'écrit encore.**

## Ce que voit l'utilisateur

- **Le choix** : une puce « HDR » sur la carte de l'ordinateur, à l'accueil, à côté du profil, seulement quand l'écran de l'appareil est HDR. Elle est **allumée** la première fois (le HDR est automatique), et l'app retient le choix pour cet ordinateur. Le même interrupteur est dans Réglages › Image.
- **Tous les profils** : Jeu, Bureau, Max et Personnalisé passent en HDR. Le HDR n'existe qu'en 4:2:0 : avec le HDR allumé, « Texte net (4:4:4) » est inactif et le profil Bureau envoie du 4:2:0 ; le HDR éteint, le 4:4:4 revient. Le HDR ne fait pas passer le profil en « Personnalisé » : c'est un choix à part.
- **En session** : « HDR » dans la barre et dans le menu de l'icône, cochable. Le changer relance l'image (étape « Premières images », une à deux secondes), comme un profil choisi en session.
- **L'ordinateur** : Windows passe l'écran virtuel en HDR tout seul au début de la session et le remet en SDR à la fin. Rien à régler sur le PC.
- **Plusieurs écrans** : chaque écran de la session est en HDR si l'écran de l'appareil qui le reçoit l'est (un Mac avec son écran XDR et un écran externe SDR : le premier en HDR, l'autre en SDR).
- **L'écran de l'appareil change** (fenêtre glissée sur un écran SDR, téléviseur qui coupe le HDR) : l'image de cet écran repasse en SDR, puis en HDR quand l'écran le redevient, sans message ; le choix « HDR » reste.
- **Invités du partage** : un invité dont l'écran est HDR reçoit le HDR quand l'hôte est en HDR ; un invité sans écran HDR, ou dans un navigateur, reçoit une image à part convertie en SDR par la Station. L'invité n'a pas de puce : c'est automatique. L'ami à qui l'ordinateur est prêté a la puce et l'interrupteur de la barre, comme un hôte.
- **Statistiques** : « HDR » s'ajoute à ce qui a été transmis (« 3840 × 2160 · 120 Hz · HEVC · HDR · 80 Mbit/s »).

## Quand le HDR n'est pas possible

La puce reste visible mais inactive, avec la raison dans son aide, dans cet ordre :

| Cas | Raison |
|---|---|
| Station sans HDR (version d'avant, ou Admin réglé sur « Jamais ») | « L'ordinateur ne propose pas le HDR. » |
| Windows d'avant Windows 11 24H2 | « Le HDR demande Windows 11 24H2 sur l'ordinateur. » |
| Encodeur sans 10 bits (x264, carte trop ancienne) | « La carte graphique de l'ordinateur ne fait pas le HDR. » |
| Format vidéo réglé sur H.264 | « Le HDR demande le format HEVC ou AV1. » (« Auto » prend HEVC ou AV1 quand le HDR est allumé.) |
| Décodeur de l'appareil sans HEVC 10 bits ni AV1 10 bits | « Cet appareil ne décode pas le HDR. » |
| Sans écran virtuel, écran du PC en SDR | « L'écran de l'ordinateur n'est pas en HDR. » (sans écran virtuel, Admin ne touche pas aux écrans du PC) |

Si Windows refuse de passer l'écran virtuel en HDR au début de la session, la session continue en SDR et l'app le dit une fois : « L'ordinateur n'a pas pu passer en HDR : image en SDR. »

## La chaîne

### Station : Kyber (correctifs 0022)

Constat dans Kyber 0.29 : les deux moteurs capturent par `IDXGIOutput1::DuplicateOutput`, qui rend un bureau ramené en SDR 8 bits par Windows, et refusent tout autre format que BGRA ; FFmpeg force la matrice BT.601 pour toute entrée RGB donnée à NVENC. Le HDR exige donc une capture en 16 bits flottants et une conversion faite par Kyber lui-même.

1. **Capture FP16**, seulement pour un écran demandé en HDR : `IDXGIOutput5::DuplicateOutput1` avec `[R16G16B16A16_FLOAT, B8G8R8A8_UNORM]` dans txproto (`iosys_dxgi.c`) et libavgrab (`dxgi.c`) ; l'agent Windows Graphics Capture des écrans secondaires (0018) ouvre son `Direct3D11CaptureFramePool` en `R16G16B16A16Float` et son anneau de textures partagées dans ce format. Sans HDR : chemin d'aujourd'hui, inchangé.
2. **Conversion sur le GPU**, juste après la capture : un compute shader HLSL commun (scRGB linéaire × 80/10 000 nits → primaires BT.2020 → PQ → Y'CbCr BT.2020 non constant, plage limitée, 10 bits → P010, chroma moyennée 2 × 2), compilé à l'exécution par `d3dcompiler_47.dll` (présente sur tout Windows 10 et 11). Aucune courbe sur l'image HDR : au-delà de 10 000 nits, la valeur est écrêtée. Images étiquetées `BT2020` / `SMPTE2084` / `BT2020_NCL` / plage MPEG.
3. **Encodeurs** : NVENC, AMF et QSV prennent des images D3D11 P010 : HEVC Main 10, AV1 10 bits. Pas de H.264 en HDR, pas de x264 en HDR, pas de 4:4:4 en HDR. Métadonnées statiques (SEI mastering display et MaxCLL) d'après la luminance que Windows donne pour l'écran virtuel (`IDXGIOutput6::GetDesc1`), pour les téléviseurs qui les attendent.
4. **Image à part en SDR** (0020) : pour un invité sans HDR, la même texture FP16 passe par un second shader : blanc SDR à la valeur du curseur « Luminosité du contenu SDR » de Windows, courbe douce au-dessus (genou à 75 % du blanc, compression jusqu'à 4 fois le blanc), primaires BT.709, gamma, NV12 8 bits, puis la réduction de taille d'aujourd'hui.
5. **Drapeau de session**, par écran : `hdr` sur le chemin de `yuv444` (`VideoSessionConfig` de kyclient et son API C, requête de démarrage de kycontroller, `VideoStreamConfig`, IPC `KymuxStartVideo`, `VideoConfig` des deux moteurs). Le transport n'a rien à apprendre : la VUI et les SEI voyagent dans le flux.

Correctifs : `0022-txproto-hdr`, `0022-libavgrab-hdr`, `0022-kyavservice-hdr`, `0022-kycontroller-hdr`, `0022-kyclient-hdr` (et la part WGC dans Admin, hors Kyber). Les deux correctifs de capture sont remontables à Kyber tels quels.

### Station : Admin

- **Réglage** (page Image et son) : « HDR » : « Si l'appareil le demande » (par défaut) ou « Jamais ».
- **Ce que la Station annonce** : `GET /api/stream/info` gagne `hdr: { available: bool, reason?: "off" | "windows" | "encoder" }` ; `reason` donne la ligne du tableau ci-dessus.
- **Préparation** : `prepare` reçoit `hdr: bool` pour chaque écran ; Admin active le HDR de l'écran virtuel créé (`DISPLAYCONFIG_SET_HDR_STATE`, Windows 11 24H2), attend que Windows le confirme (2 s au plus), et répond pour chaque écran le HDR obtenu. À la fin de la session, au redémarrage d'Admin après un plantage, et quand l'écran est retiré : HDR remis comme avant. Sans écran virtuel, Admin ne change rien et répond le HDR que l'écran du PC a déjà.
- **Journal** : une ligne par écran : HDR demandé, obtenu, luminance donnée par Windows, format de capture.

### Cœur Rust et apps

- **Cœur** (`apps/core`) : le réglage `hdr` (allumé par défaut) est gardé pour chaque ordinateur ; la session envoie `hdr` pour chaque écran quand le réglage est allumé, que l'écran de l'appareil est HDR, que la Station l'annonce et que le décodeur suit ; elle impose alors 4:2:0 et HEVC ou AV1. L'app donne au cœur, pour chaque écran, « HDR ou non » et le prévient quand ça change ; le cœur relance l'image de cet écran.
- **Apple** (Mac, iPhone, iPad, Apple TV) : écran HDR quand sa marge EDR possible dépasse 1 (`maximumPotentialExtendedDynamicRangeColorComponentValue` sur Mac, `potentialEDRHeadroom` sur iPhone et iPad) ; sur l'Apple TV, quand le téléviseur accepte le HDR10 (`AVPlayer.eligibleForHDRPlayback`). Rendu : `CAMetalLayer` en `rgba16Float`, espace `itur_2100_PQ`, `wantsExtendedDynamicRangeContent`, `edrMetadata` ; le shader de `VideoRenderer` lit déjà les plans 10 bits et la matrice BT.2020, il écrit les valeurs PQ telles quelles. Apple TV : demande au téléviseur de passer en HDR par `AVDisplayManager` (`AVDisplayCriteria` avec une description de format HEVC PQ/BT.2020), réglage « Adapter la plage dynamique » de tvOS requis, sinon le HDR suit la sortie choisie dans tvOS.
- **Téléviseurs LG** : décodage par la chaîne vidéo du téléviseur (NDL) déjà en place ; métadonnées HDR10 données au téléviseur par `NDL_DirectVideoSetHDRInfo` pour qu'il passe en mode HDR ; écran HDR d'après les capacités annoncées par webOS. À vérifier en premier sur la G2 : c'est le point le moins sûr.
- **Deuxième temps** : Android (`Display.getHdrCapabilities`, `MediaCodec` avec `KEY_COLOR_TRANSFER` ST 2084, sortie sur une `SurfaceView`) ; Windows (chaîne d'échange `R10G10B10A2`, `SetColorSpace1(RGB_FULL_G2084_NONE_P2020)`, `SetHDRMetaData`, écran HDR d'après `IDXGIOutput6`) ; Linux (gestion des couleurs de Wayland, à étudier), Raspberry Pi après. Le navigateur reste en SDR.

## Interfaces entre les morceaux

Fixées avant d'écrire, pour que Kyber, Admin, le cœur et les apps avancent en même temps.

**Kyber (0022).** `kyclient::VideoSessionConfig` gagne `hdr: bool` (API C `kyclient_video_session_config_set_hdr`, `kyclient-rs` : `set_hdr`) ; il suit `yuv444` jusqu'à kycontroller (requête de démarrage, champ `hdr` par écran), `VideoStreamConfig` et `KymuxStartVideo` (`hdr`). Le drapeau de l'**hôte** décide de la capture de l'écran (FP16 ou non) ; celui de chaque client dit ce qu'il affiche : un client `hdr: false` sur un écran capturé en HDR reçoit une image à part en SDR (0020), un client `hdr: true` partage l'image HDR. `hdr: true` avec H.264, x264 ou `yuv444` : refusé par kyavservice, l'écran repart en SDR et kycontroller le note au journal. Anneau WGC (0018) : l'en-tête de la mémoire partagée gagne `format` (un `DXGI_FORMAT`) ; l'agent capture en `R16G16B16A16Float` quand l'écran est en HDR pour Windows (`IDXGIOutput6::GetDesc1`, espace `RGB_FULL_G2084_NONE_P2020`), en BGRA sinon, sans drapeau à lui passer ; kyavserver suit le format de l'en-tête.

**Admin.** `GET /api/stream/info` : `hdr: { available: bool, reason?: "off" | "windows" | "encoder" }`. `POST /api/stream/prepare` : `hdr: bool` (session à un écran) et `screens[i].hdr: bool` ; réponse : `hdr: [bool]`, le HDR obtenu pour chaque écran, dans l'ordre de `screens` (un seul élément pour un écran) ; Admin allume le HDR de l'écran virtuel **avant** de lancer les agents WGC et de répondre. `POST /api/stream/hdr` (appareil en session seulement, sinon `409`) : `{ "device_id": "…", "hdr": [bool] }`, réponse `{ "hdr": [bool] }`, pour la puce de la barre et un écran de l'appareil qui change. Réglage d'Admin : `hdr: "auto" | "never"` (« Si l'appareil le demande », « Jamais »). HDR d'origine noté pour chaque écran (`hdr-session.json`) et remis à la fin, au redémarrage après un plantage et quand l'écran est retiré.

**Cœur.** `StreamSettings.hdr: bool` (`SettingsJson.hdr`, absent : `false`) : le choix « HDR » de l'app pour cet ordinateur, que l'app garde (allumé par défaut sur un écran HDR). `Capabilities.hdr: bool` : le décodeur lit le HEVC Main 10 ou l'AV1 10 bits et l'app sait l'afficher. `DeviceScreen.hdr: bool` (multi-écrans) et `SessionParams.screen_hdr: bool` (un écran) : l'écran de l'appareil est HDR. `profile::hdr_state(info, settings, caps, screen_hdr) -> HdrState { shown, available, reason }` donne la puce (cachée, active, inactive avec la raison traduite par le catalogue du cœur) ; la session s'en sert pour envoyer `hdr` à `prepare` et à kyclient, et impose alors 4:2:0 et HEVC ou AV1. En session : `Session::set_hdr(bool)` (barre, menu) et `Session::screen_hdr_changed(screen, bool)` (écran de l'appareil) appellent `POST /api/stream/hdr` et relancent l'image des écrans touchés ; `Platform::on_hdr(&[bool])` dit à l'app, à chaque démarrage de l'image, quels écrans sont en HDR (le rendu change de chaîne) ; l'avis « L'ordinateur n'a pas pu passer en HDR : image en SDR. » passe par `on_notice`. Statistiques : `hdr` dans la seconde et dans la dernière session. Pont C : `kn_session_set_hdr`, `kn_session_screen_hdr_changed`, `kn_hdr_state` et le rappel `on_hdr` ; JNI au deuxième temps.

## Livraison

1. **Première version** : Station (Kyber, Admin), cœur, apps Apple et téléviseurs LG. Essais : écran XDR du Mac, Apple TV sur téléviseur HDR, LG G2, carte NVIDIA du PC.
2. **Version suivante** : Android, Windows, Linux.

Coût estimé de la première version : environ 18 jours (l'étude du 22/10 en comptait 14, avant la capture WGC des écrans secondaires, l'image à part des invités et l'app LG).
