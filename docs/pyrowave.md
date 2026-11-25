# PyroWave

[English](pyrowave.en.md) · Français

[PyroWave](https://github.com/Themaister/pyrowave) (licence MIT) est un format d'image par ondelettes, calculé en Vulkan sur la carte graphique : chaque image est indépendante (pas d'image de référence), l'encodage et le décodage prennent environ 0,1 à 0,25 ms, au prix d'un débit élevé (200 Mbit/s et plus). C'est un format de plus pour le réseau local, à côté de H.264, HEVC et AV1, sans rien changer aux flux existants.

Jalon 1 : Station Windows (ce document) et client Mac (`apps/core`, décodeur Mac). Les textes visibles (Admin, apps) sont provisoires, en attente de la maquette.

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

1. **Annonce** : `GET /api/stream/info` de KyberNow Admin porte `"pyrowave": { "available": true }`, ou `{ "available": false, "reason": … }` avec, dans l'ordre : `off` (réglage `[kyavserver] pyrowave = false`), `windows` (Station Mac), `vulkan` (pas de Vulkan 1.3 qui convient sur la carte qui encode). Une Station plus ancienne n'a pas ce champ : les apps ne demandent alors jamais PyroWave.
2. **Vulkan** : Admin lance une fois par carte, en fond au démarrage, l'outil de PyroWave `pyrowave-device-validation.exe --external --roundtrip --timeout 15 --luid <carte>` (dossier de Kyber) : Vulkan 1.3 avec les fonctions dont PyroWave a besoin, import des textures et des fences D3D11, aller-retour encodage puis décodage. Un processus à part : un pilote qui plante n'emporte pas Admin. Résultat gardé le temps qu'Admin tourne ; une autre carte est vérifiée à son tour.
3. **Demande** : l'app demande `codec: "PyroWave"` dans la requête de démarrage, avec `fallback_codec` (son format habituel, H.264 sans lui). L'API Rust de kyclient a `VideoSessionConfig::fallback_codec`, l'API C `kyclient_video_session_config_set_fallback_codec`.
4. **Acceptation** : kycontroller ne garde PyroWave que si la Station le propose (Windows et `[kyavserver] pyrowave` vrai ou absent), sinon il prend le format de repli (journal : « PyroWave not offered by this Station: HEVC instead »). kyavserver vérifie encore (H.264 sinon). Le client lit le format qui arrive dans le paquet de codec du flux.
5. **Encodeur** : PyroWave ne dépend pas de `[kyavserver] encoder` (NVENC, AMF, Quick Sync ou x264) : l'encodeur H.264/HEVC choisi reste celui des autres flux.

## La Station Windows

- **Bibliothèque** : `libpyrowave-shared-1.dll` (API C de PyroWave, runtime C++ lié en statique, Vulkan chargé à la demande par volk : pas de dépendance à `vulkan-1.dll`, kyavserver démarre sur un PC sans Vulkan). Sources au commit épinglé dans `server/kyber-patches/pyrowave.sh` (`a32b05e`, 11/11/2026), avec le Granite partiel de `checkout_granite.sh` (volk, en-têtes Vulkan), dans `third_party/pyrowave` ; compilée en MinGW depuis le Mac par `build.sh`.
- **Encodeur FFmpeg `libpyrowave`** (correctif 0025-kymedia, `libavcodec/libpyrowave_enc.c`, d'après la branche `pyrowave` de `HansKristian-Work/FFmpeg` pour l'identifiant `AV_CODEC_ID_PYROWAVE`) : il prend les images D3D11 de la capture sans copie par le processeur. Au démarrage, un device Vulkan sur la même carte que la capture (LUID de l'adaptateur DXGI, `pyrowave_create_device_by_compat`, priorité de file Vulkan selon `gpu_priority`), une fence D3D11 partagée importée en semaphore timeline. À chaque image : copie sur la carte de la texture capturée dans une texture partagée (handle NT, importée une fois dans Vulkan, recréée si la taille ou le format change), signal de la fence, puis `pyrowave_encoder_encode_gpu_scaled` qui attend la fence, convertit et réduit au besoin, et `pyrowave_encoder_packetize`.
- **Les deux moteurs** : txproto (`create_pyrowave` de `txproto/video.rs`, capture reliée directement à l'encodeur) et libavconv (`encoder_names`, filtre `null`). `encode.c` et `enc.c` n'ont rien à changer : l'encodeur se déclare matériel sur des images D3D11, ils lui passent le contexte d'images de la capture.
- **Plusieurs écrans** : un encodeur par écran, comme pour les autres formats ; chacun ouvre son device Vulkan sur la carte de son écran.
- **Image réduite d'un invité** (KyberNow Share) : PyroWave réduit lui-même l'image (options `out_width`, `out_height`) au lieu de `scale_d3d11` ; cadence plafonnée par la capture (txproto), `fps` de l'encodeur réglé sur elle pour le budget d'image.
- **Réglage** : `[kyavserver] pyrowave` (vrai par défaut), clé `video.pyrowave` d'Admin, Windows seulement (textes provisoires, marqués `TODO textes pyrowave`).
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

## État des essais

| Quoi | État |
|---|---|
| `libpyrowave-shared-1.dll` en MinGW depuis le Mac | compilé (25/11/2026), sans dépendance à `vulkan-1.dll` |
| FFmpeg avec l'encodeur `libpyrowave` | compilé (`--enable-libpyrowave`) |
| Socle (kyavservice, kycontroller, kyclient, txproto) et surcouche | `build.sh` complet depuis l'origine (25/11/2026) ; tests de kycontroller (149) et d'Admin (321) passés sur le Mac ; tests de kyavservice (`pyrowave.rs`, `hdr.rs`, `config.rs`) compilés pour Windows, pas lancés |
| Encodage d'une image capturée en D3D11 sur un PC | **à essayer** (section précédente) |
| Session complète vers le Mac | à essayer, avec le décodeur Mac |
| HDR10 (P010), 4:4:4, image réduite d'un invité dans une session, plusieurs écrans, moteur libavconv | à essayer |
