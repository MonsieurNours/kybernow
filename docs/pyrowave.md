# PyroWave

[English](pyrowave.en.md) · Français

[PyroWave](https://github.com/Themaister/pyrowave) (licence MIT) est un format d'image par ondelettes, calculé en Vulkan sur la carte graphique : chaque image est indépendante (pas d'image de référence), l'encodage et le décodage prennent environ 0,1 à 0,25 ms, au prix d'un débit élevé (200 Mbit/s et plus). C'est un format de plus pour le réseau local, à côté de H.264, HEVC et AV1, sans rien changer aux flux existants.

Jalon 1 : Station Windows (ce document) et client Mac (`apps/core`, décodeur Mac) ; puis la Station Mac (encodeur Metal, section [La Station Mac](#la-station-mac)). Les textes visibles (Admin, apps) sont provisoires, en attente de la maquette.

## Contrat commun

Toutes les apps et la Station s'y tiennent.

- **Identifiant de codec** kymux et kycom : `0x7077_7631` (« pwv1 »), à côté de `KYMUX_CODEC_H264`, `KYMUX_CODEC_H265` et `KYMUX_CODEC_AV1` (`kymux_sink.rs` du moteur libavconv, `packet_sink.c` de txproto). Variante `PyroWave` des enums `VideoCodec` : `kyavservice-types`, `kyctl-common` (`"PyroWave"` dans le JSON de la requête de démarrage), `kyclient` (et son API C : `KYCLIENT_VIDEO_CODEC_PYRO_WAVE`, nom donné par cbindgen), `kyclient-rs`.
- **Paquet de configuration** (`is_config`, l'extradata de l'encodeur), 16 octets : `"KNPW"` | version u8 = 1 | chroma u8 (0 = 4:2:0, 1 = 4:4:4) | couleur u8 (0 = SDR BT.709, 1 = HDR10 BT.2020 PQ) | plage u8 (0 = pleine, 1 = réduite) | largeur u32 LE | hauteur u32 LE. Renvoyé à chaque changement de définition (l'encodeur est alors recréé, comme pour les autres formats).
- **Paquet média** : une image complète, la concaténation des paquets de `pyrowave_encoder_packetize`, toujours marquée image clé ; pts comme aujourd'hui. Le décodeur accepte cette concaténation telle quelle (`pyrowave_decoder_push_packet` lit les blocs à la suite). Une image perdue est simplement sautée : pas d'image clé à demander.
- **Frontière de paquet** : 1 152 octets (option `packet_size` de l'encodeur). Elle ne sert qu'au découpage interne du flux : kynet réassemble toujours le paquet média entier (RaptorQ, `video_unreliable_fec.rs`) avant de le rendre au client, quelle que soit la taille de ses datagrammes. Elle ne s'aligne donc sur rien côté réseau (vérifié dans le code de kyproto, pas mesuré).
- **Débit** : `maximum_bitstream_size = débit_bps / 8 / fps` (multiple de 4), relu à chaque image : un changement de débit en cours de session (`/set_bitrate`) s'applique à l'image suivante.
- **En SDR** : entrée BGRA de la capture, YCbCr BT.709 **plage pleine**. **En HDR10** : la capture convertit déjà en P010 (PQ, BT.2020, **plage réduite**, correctif 0022) ; PyroWave le prend tel quel, sans conversion.
- **4:4:4** : en SDR seulement (la capture HDR10 est en 4:2:0).

## Ce que la Station annonce et accepte

1. **Annonce** : `GET /api/stream/info` de KyberNow Admin porte `"pyrowave": { "available": true }`, ou `{ "available": false, "reason": … }` avec, dans l'ordre : `off` (réglage `[kyavserver] pyrowave = false`), `windows` (ni Windows ni Mac), `vulkan` (pas de Vulkan 1.3 qui convient sur la carte qui encode), `metal` (Station Mac : puce qui ne convient pas ou encodeur qui ne démarre pas, [La Station Mac](#la-station-mac)). Une Station plus ancienne n'a pas ce champ : les apps ne demandent alors jamais PyroWave.
2. **Vulkan** : Admin lance une fois par carte, en fond au démarrage, l'outil de PyroWave `pyrowave-device-validation.exe --external --roundtrip --timeout 15 --luid <carte>` (dossier de Kyber) : Vulkan 1.3 avec les fonctions dont PyroWave a besoin, import des textures et des fences D3D11, aller-retour encodage puis décodage. Un processus à part : un pilote qui plante n'emporte pas Admin. Résultat gardé le temps qu'Admin tourne ; une autre carte est vérifiée à son tour.
3. **Demande** : l'app demande `codec: "PyroWave"` dans la requête de démarrage, avec `fallback_codec` (son format habituel, H.264 sans lui). L'API Rust de kyclient a `VideoSessionConfig::fallback_codec`, l'API C `kyclient_video_session_config_set_fallback_codec`.
4. **Acceptation** : kycontroller ne garde PyroWave que si la Station le propose (Windows ou Mac, et `[kyavserver] pyrowave` vrai ou absent), sinon il prend le format de repli (journal : « PyroWave not offered by this Station: HEVC instead »). kyavserver vérifie encore (H.264 sinon). Le client lit le format qui arrive dans le paquet de codec du flux.
5. **Encodeur** : PyroWave ne dépend pas de `[kyavserver] encoder` (NVENC, AMF, Quick Sync ou x264) : l'encodeur H.264/HEVC choisi reste celui des autres flux.
6. **Débit maximal** : `pyrowave.max_mbps` (Mbit/s), présent quand il est connu : le réglage « Débit maximal » d'Admin (`video.pyrowave_max_mbps`, `[kyavserver] pyrowave_max_mbps` lu par Admin seul : `auto` par défaut, `300`, `500`, `800`) ; en Auto, d'après la prise réseau de la Station (`TransmitLinkSpeed` de la première carte avec une adresse, Ethernet et carte de la passerelle d'abord, relue toutes les minutes) : 80 % de sa vitesse en Ethernet, 50 % en Wi-Fi (débit radio annoncé), arrondi aux 50 Mbit/s du dessous (1 Gbit/s → 800, 2,5 Gbit/s → 2000, 100 Mbit/s → 50). Prise inconnue : absent, pas de plafond de l'ordinateur.

## Définition ramenée

Règle des apps ([link-ui.md](link-ui.md), « PyroWave ») : quand le débit dont PyroWave a besoin dépasse le plafond, l'app baisse la définition (puis la fréquence), jamais la netteté, et le dit. Le calcul est dans le cœur (`apps/core/src/pyrowave.rs`, voir [apps/core/README.md](../apps/core/README.md)), le même pour toutes les apps.

- **Débit nécessaire** : largeur × hauteur × fréquence × bits par pixel, la somme des écrans d'une session à plusieurs écrans. **1,5 bit par pixel en 4:2:0** (SDR ou HDR10 : le haut de ce que donne PyroWave à netteté pleine, ~1,2 à 1,5) ; **2,25 en 4:4:4** (moitié plus : deux fois plus d'échantillons de couleur, qui pèsent moins que la luminance). Soit ≈ 187 Mbit/s en 1080p60, ≈ 747 Mbit/s en 4K60, ≈ 1 120 Mbit/s en 4K60 4:4:4. Valeurs à confirmer sur des flux de la Station.
- **Plafond** : le plus petit de ceux qui sont connus : `pyrowave.max_mbps` de la Station, le débit du lien que l'app donne (vitesse de sa prise, ou débit mesuré), le débit réglé à la main dans l'app (format sans profil). Aucun : la définition de l'appareil reste.
- **Choix** : la plus grande définition paire qui tient, proportions gardées, à la fréquence de l'écran, jusqu'à 720 lignes ; puis 120, 100, 60, 50 ou 30 Hz, avec de nouveau la plus grande définition qui tient ; rien ne tient : 720p à 30 Hz. À plusieurs écrans, la même échelle pour tous, positions comprises (ils restent bord à bord).
- **Demandée à la Station** : c'est la définition de l'écran virtuel (`prepare`), au démarrage et à chaque adaptation ; quand le plafond ou le format change en session, l'écran virtuel est refait si la définition doit changer. Sans écran virtuel, la Station garde son écran : seul le débit est plafonné. Kyber reçoit au moins le débit nécessaire à cette définition (profil), jamais plus que le plafond.
- **Dit aux apps** : avis `ResolutionLowered` (bandeau « Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. », retiré quand elle revient) et `Second::lowered` (ligne des Statistiques), dans les ponts C, JNI et les apps Windows et Linux.

## La Station Windows

- **Bibliothèque** : `libpyrowave-shared-1.dll` (API C de PyroWave, runtime C++ lié en statique, Vulkan chargé à la demande par volk : pas de dépendance à `vulkan-1.dll`, kyavserver démarre sur un PC sans Vulkan). Sources au commit épinglé dans `server/kyber-patches/pyrowave.sh` (`a32b05e`, 11/11/2026), avec le Granite partiel de `checkout_granite.sh` (volk, en-têtes Vulkan), dans `third_party/pyrowave` ; compilée en MinGW depuis le Mac par `build.sh`.
- **Encodeur FFmpeg `libpyrowave`** (correctif 0025-kymedia, `libavcodec/libpyrowave_enc.c`, d'après la branche `pyrowave` de `HansKristian-Work/FFmpeg` pour l'identifiant `AV_CODEC_ID_PYROWAVE`) : il prend les images D3D11 de la capture sans copie par le processeur. Au démarrage, un device Vulkan sur la même carte que la capture (LUID de l'adaptateur DXGI, `pyrowave_create_device_by_compat`, priorité de file Vulkan selon `gpu_priority`), une fence D3D11 partagée importée en semaphore timeline. À chaque image : copie sur la carte de la texture capturée dans une texture partagée (handle NT, importée une fois dans Vulkan, recréée si la taille ou le format change), signal de la fence, puis `pyrowave_encoder_encode_gpu_scaled` qui attend la fence, convertit et réduit au besoin, et `pyrowave_encoder_packetize`.
- **Les deux moteurs** : txproto (`create_pyrowave` de `txproto/video.rs`, capture reliée directement à l'encodeur) et libavconv (`encoder_names`, filtre `null`). `encode.c` et `enc.c` n'ont rien à changer : l'encodeur se déclare matériel sur des images D3D11, ils lui passent le contexte d'images de la capture.
- **Plusieurs écrans** : un encodeur par écran, comme pour les autres formats ; chacun ouvre son device Vulkan sur la carte de son écran.
- **Image réduite d'un invité** (KyberNow Share) : PyroWave réduit lui-même l'image (options `out_width`, `out_height`) au lieu de `scale_d3d11` ; cadence plafonnée par la capture (txproto), `fps` de l'encodeur réglé sur elle pour le budget d'image.
- **Réglage** : `[kyavserver] pyrowave` (vrai par défaut), clé `video.pyrowave` d'Admin, Windows et Mac (textes provisoires, marqués `TODO textes pyrowave`). « Débit maximal » : `video.pyrowave_max_mbps` (voir l'annonce, point 6), Windows et Mac ; l'aide Auto de la maquette (« ici {n} Mbit/s ») reste à faire dans la carte PyroWave d'Admin.
- **Surcouche** : `libpyrowave-shared-1.dll` et `pyrowave-device-validation.exe` dans `third_party/payload/kyber-overlay/`, donc dans l'installateur et les mises à jour.

## Essai sur le PC

`build.sh` produit aussi un essai qui n'est pas livré : `third_party/pyrowave/test-win64/` (`pyrowave-capture-test.exe`, `pyrowave-device-validation.exe` et les DLL dont ils ont besoin). Il capture l'écran principal comme la Station (duplication DXGI, images D3D11 en BGRA), les encode avec l'encodeur `libpyrowave` de la Station, écrit les paquets, puis décode la dernière image avec le décodeur de PyroWave pour vérifier le flux.

1. Sur le Mac : `server/kyber-patches/build.sh` (ou seulement l'essai, si la surcouche est déjà à jour : voir la section « Essai PyroWave » du script).
2. Copier le dossier `third_party/pyrowave/test-win64` sur le PC, par exemple dans `C:\pyrowave-test`.
3. Sur le PC, **dans la session ouverte** (la duplication de l'écran ne marche pas depuis un service), ouvrir PowerShell dans ce dossier et lancer :
   ```
   .\pyrowave-capture-test.exe
   ```
   Attendu : « Carte : … », « Écran : 3840x2160 … », la ligne de FFmpeg « PyroWave 1.0.0 on … », « En-tête : 4b4e505701000000… » (« KNPW », version 1, 4:2:0, SDR, plage pleine, puis la taille), « 120 images encodées : … ms en moyenne », « Décodée : … (image plausible) », puis « OK ». Bouger une fenêtre pendant l'essai donne des images différentes.
4. Image réduite d'un invité : `.\pyrowave-capture-test.exe 120 50 1280 720` (120 images, 50 Mbit/s, 1280 × 720).
5. Ouvrir `pyrowave-test.y4m` dans VLC : c'est la dernière image, décodée.
6. Vulkan tel qu'Admin le vérifie : `.\pyrowave-device-validation.exe --external --roundtrip --timeout 15; $LASTEXITCODE` : 0 attendu (2 : pas de Vulkan 1.3 qui convient, 4 : pas d'import D3D11, 5 : aller-retour faux).

À noter pour le compte rendu : la sortie complète du programme, et le moteur réglé dans Admin.

Puis, une fois la Station installée avec la surcouche (et un client qui sait le demander) : `GET /api/stream/info` dit `"pyrowave": { "available": true }` ; une session PyroWave écrit au journal de kyavserver « PyroWave encoder 3840x2160 4:2:0, SDR (BT.709, full) … » et la ligne « Timing in ms … libpyrowave » toutes les 5 s.

## La Station Mac

- **Bibliothèque** : `libkn-pyrowave-metal.a` (`server/macos/pyrowave/build.sh`), le port Metal de PyroWave vendu dans `apps/macos/PyroWave` (commit `a32b05e`, le même que le décodeur de l'app Mac) et la conversion de KyberNow (`kn_pyrowave_metal.mm`, interface C dans `kn_pyrowave_metal.h`), en bibliothèque statique liée dans `libtxproto.dylib` (FFmpeg y est statique). Pas de nouvelle dylib dans l'app : seulement Metal, IOSurface et `libc++` du système. `build-kyber.sh` la compile dans le rootfs avant FFmpeg, avec `kn-pyrowave-metal.pc`, et extrait FFmpeg à nouveau quand son wrap change (comme `build.sh` sous Windows).
- **Encodeur FFmpeg `libpyrowave_metal`** (correctif Mac 0016, `libavcodec/libpyrowave_metal_enc.c`) : mêmes options et même contrat que `libpyrowave`. Il prend les images VideoToolbox de ScreenCaptureKit (`CVPixelBuffer` sur une IOSurface) sans copie par le processeur ; txproto lui donne le contexte d'images de la capture (`encode.c` inchangé). Selon l'image :
  - **NV12** (4:2:0 SDR à la définition de l'appareil, le cas ordinaire) : ScreenCaptureKit livre du NV12 BT.709 pleine plage (option `pyrowave_nv12` de la capture, correctifs Mac 0017 et 0019), que le port Metal lit tel quel : aucune conversion ;
  - **BGRA** (4:4:4, ou image réduite d'un invité) : un passage de calcul Metal convertit en YCbCr BT.709 pleine plage dans trois plans R8 sur des IOSurface (moyenne de 2 × 2 points pour la chroma en 4:2:0 ; réduction par échantillonnage linéaire au passage), que le port lit ensuite ;
  - **P010** (HDR10 de la capture, correctif Mac 0010 : PQ, BT.2020, plage réduite, blanc déjà remonté) : tel quel ; réduit sur le GPU en P010 pour un invité.
  L'encodeur de PyroWave a sa propre file Metal : la conversion est attendue avant lui (`waitUntilCompleted`), ce qui coûte environ 1 ms à 60 i/s quand le GPU est au repos ; d'où le NV12 de la capture dans le cas ordinaire.
- **Le moteur** : txproto seulement (il n'y a pas de capture Mac dans libavconv), même `create_pyrowave` que sous Windows (correctif Mac 0017). kycontroller propose PyroWave sur Mac (0018).
- **Plusieurs écrans** : un encodeur par écran, chacun sur la puce (un seul GPU).
- **Image réduite d'un invité** : réduite par l'encodeur (`out_width`, `out_height`), cadence plafonnée par ScreenCaptureKit (correctif Mac 0008).
- **Puce** : famille Apple7 ou plus (`pyrowave_device_is_supported` : toutes les puces M, la Station Mac ne tourne que sur elles). `gpu_priority` est accepté mais sans effet (Metal n'a pas de priorité de file à régler).
- **Admin** : `kybernow-pyrowave-check` (à côté d'Admin dans l'app, `KYBERNOW_PYROWAVE_CHECK` pour un autre chemin) vérifie une fois par lancement, en fond, que la puce convient et que l'encodeur démarre (aller-retour d'une mire 256 × 256 encodée puis décodée) ; code 0 bon, 2 puce, 4 encodeur, 5 aller-retour faux. Sinon `reason: "metal"`. Réglages « PyroWave » et « Débit maximal » comme sous Windows ; en Auto, la prise du Mac : vitesse négociée de l'Ethernet (`ifconfig`, ligne `media`), ou débit radio du Wi-Fi (`system_profiler SPAirPortDataType`, relu au plus toutes les 5 minutes) quand le Wi-Fi est la prise retenue.
- **Paquet** (`build-station-app.sh`, notarisé comme avant) : `kybernow-pyrowave-check` en plus dans `Contents/MacOS`, signé avec les mêmes droits que les autres exécutables (exécution renforcée ; Metal n'en demande pas d'autre) ; l'encodeur est dans `libtxproto.dylib`, déjà là. `run-kyber.sh package` met l'outil dans l'app de Kyber, `run-station.sh` à côté d'Admin. Rien d'autre ne change pour la notarisation.
- **Journal de kyavserver** : « PyroWave encoder 3840x2160 4:2:0, SDR (BT.709, full), input 3840x2160 nv12 … on Apple M4 Max (Metal) » à l'ouverture, puis toutes les 600 images la taille moyenne et les temps de conversion et d'encodage ; « Capture: NV12 BT.709 full range for PyroWave » côté capture.

## Essai sur le Mac

`server/macos/pyrowave/capture-test.sh` (après `build-kyber.sh`, avec le même `KYBER_DIR`) : `capture-test.m` capte un écran par ScreenCaptureKit comme txproto (NV12, BGRA ou P010 PQ), passe les images à l'encodeur FFmpeg `libpyrowave_metal` de la Station à 60 i/s (l'image la plus récente, qu'elle ait changé ou non), écrit les paquets, puis le banc de décodage de l'app Mac (`apps/macos/tests/pyrowave/bench`, `VideoDecoder` puis le décodeur Metal) les décode : temps, PSNR de la première image contre ce que l'encodeur a vu, image tronquée sautée. Il faut l'autorisation « Enregistrement de l'écran » pour le terminal.

Mesures du 25/11/2026 (M4 Max, macOS 27.2, écran principal capté à la définition demandée, 1,2 bit par point à 60 i/s, autres compilations en cours sur le Mac) : temps de la Station par image, de l'envoi à l'encodeur au paquet prêt (moyenne, 95e centile), et décodage cadencé par l'app Mac.

| Cas | Encodage | Décodage (app Mac) | PSNR Y / Cb / Cr |
|---|---|---|---|
| 1440p 4:2:0 SDR, NV12 de la capture | 1,7 ms (1,8) | 1,2 ms | 52 / 73 / 73 dB |
| 1440p 4:2:0 SDR, BGRA converti | 3,0 ms (5,1) | 1,7 ms | 51 / 66 / 65 dB |
| 1440p 4:4:4 SDR | 3,0 ms (3,7) | 1,7 ms | 51 / 65 / 63 dB |
| 1440p 4:2:0 HDR10 (P010) | 1,7 ms (2,3) | 1,2 ms | 57 / 57 / 57 dB |
| 4K 4:2:0 SDR, NV12 de la capture | 2,5 ms (4,3) | 2,4 ms | 61 / 83 / 82 dB |
| 4K 4:2:0 SDR, BGRA converti | 4,2 ms (6,0) | 2,2 ms | 61 / 67 / 67 dB |
| 4K 4:4:4 SDR | 5,4 ms (7,1) | 2,5 ms | 61 / 67 / 64 dB |
| 4K 4:2:0 HDR10 (P010) | 2,2 à 5,2 ms (selon les essais) | 1,5 à 2,1 ms | 60 / 57 / 56 dB |
| Invité 1080p (4K réduit), SDR / HDR10 | 2,3 / 2,8 ms | 1,1 / 1,2 ms | — |

Le PSNR compare au YCbCr que l'encodeur a reçu (calculé à part pour le BGRA) : il mesure PyroWave à ce débit, pas la conversion. Détail du BGRA en 4K 4:2:0 (journal de l'encodeur, 600 images) : conversion 1,9 ms dont 0,1 à 0,3 ms de GPU, le reste étant l'attente entre les deux files et le réveil du GPU ; encodage 2,3 ms. À la chaîne (GPU à pleine fréquence), la même image : conversion 0,7 ms, encodage 1,1 ms. Débit tenu : chaque image remplit son budget (`débit / 8 / fps`) à quelques octets près. Contrôle d'Admin : « PyroWave sur Apple M4 Max : 24528 octets, PSNR 55.2 dB », code 0.

**Session réelle Station Mac → app Mac** : pas faite (la Station installée de l'auteur et une app Mac d'essai tournaient sur ce Mac). Marche à suivre :

1. Kyber avec PyroWave : `KYBER_DIR=<copie privée> server/macos/scripts/build-kyber.sh` (copie de `third_party/kyber-desktop-macos` par `cp -cR`, sans son `rootfs-arm64-macos` ni `kysdk/kymedia/builddir-macos`).
2. Station : `KYBER_DIR=<copie privée> server/macos/scripts/build-station-app.sh` (signée Developer ID, sans notarisation), puis installer `server/macos/dist/KyberNow Station.app` par l'installateur du .dmg (ou, sans toucher à la Station installée, Station d'essai : arrêter l'Admin installé par `launchctl bootout gui/$(id -u)/fr.leslignesclaires.kybernow.station.admin`, puis `KYBER_DIR=<copie privée> server/macos/scripts/run-station.sh`, qui pose `kybernow-pyrowave-check` à côté d'Admin).
3. Vérifier l'annonce : `curl -sk -u <compte> https://<mac>:<port d'Admin>/api/stream/info | jq .pyrowave` → `{"available": true, "max_mbps": …}` ; journal d'Admin : « PyroWave : la puce convient ».
4. App Mac : compilée avec le correctif kyclient 0025 posé et la fonctionnalité `pyrowave` du cœur (voir [pyrowave-client-mac.md](pyrowave-client-mac.md)), sur un appareil relié par câble ou avec « Format vidéo » réglé sur PyroWave.
5. Attendu au journal de kyavserver (dans `~/Library/Logs/KyberNow`, ou `kyber/log/kyber.log` de la Station d'essai) : « Capture: NV12 BT.709 full range for PyroWave », « PyroWave encoder … (Metal) », puis toutes les 600 images « PyroWave: 600 frames … conversion 0.00 ms, encode … ms ». Essayer aussi 4:4:4 (texte net), HDR10, deux écrans et un invité de Share.

## État des essais

| Quoi | État |
|---|---|
| `libpyrowave-shared-1.dll` en MinGW depuis le Mac | compilé (25/11/2026), sans dépendance à `vulkan-1.dll` |
| FFmpeg avec l'encodeur `libpyrowave` | compilé (`--enable-libpyrowave`) |
| Socle (kyavservice, kycontroller, kyclient, txproto) et surcouche | `build.sh` complet depuis l'origine (25/11/2026) ; tests de kycontroller (149) et d'Admin (321) passés sur le Mac ; tests de kyavservice (`pyrowave.rs`, `hdr.rs`, `config.rs`) compilés pour Windows, pas lancés |
| Encodage d'une image capturée en D3D11 sur un PC | **à essayer** (section précédente) |
| Session complète vers le Mac | à essayer, avec le décodeur Mac |
| HDR10 (P010), 4:4:4, image réduite d'un invité dans une session, plusieurs écrans, moteur libavconv | à essayer |
| Définition ramenée (cœur) et « Débit maximal » (Admin) | tests unitaires passés sur le Mac (25/11/2026 : cœur 357, Admin 323) ; jamais vue en session (il faut le correctif kyclient dans le cœur) ; vitesse de la prise lue par Windows jamais essayée |
| Station Mac : encodeur `libpyrowave_metal` et Kyber compilés | `build-kyber.sh` sur une copie privée (25/11/2026) ; tests de kyavservice sur le Mac (33 sur 34 : l'échec, `rtp_start_audio_error_reports_audio_pipeline_type`, attend un événement du son RTP, sans rapport) et de kycontroller (111), Admin (324) |
| Station Mac : encodage d'une capture réelle, décodage par l'app Mac | **fait** (25/11/2026, [Essai sur le Mac](#essai-sur-le-mac)) : 1440p et 4K, 4:2:0 et 4:4:4, SDR et HDR10, invité 1080p |
| Station Mac : session complète, plusieurs écrans, Admin installé | à essayer (marche à suivre ci-dessus) ; vitesse de la prise lue sur Mac essayée par les tests seulement |
