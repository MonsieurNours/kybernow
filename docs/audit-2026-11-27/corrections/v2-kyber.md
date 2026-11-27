# Vague 2 de l'audit du 27/11 : correctifs de Kyber

Zone : le contenu des correctifs de Kyber (`server/kyber-patches`, `server/macos/kyber-patches`, `server/linux/kyber-patches`, `apps/macos/kyber-patches`, `apps/webos/kyber-patches`), `server/macos/pyrowave` et `docs/kyber-changes*.md`. Pas les scripts qui posent les correctifs. Plan : [../vague-2.md](../vague-2.md) ; rapport d'origine : [../correctifs-kyber.md](../correctifs-kyber.md) ; vague 1 : [correctifs-kyber.md](correctifs-kyber.md). Branche `claude/v2-kyber-331fc6`.

**Méthode.** Comme à la vague 1, rien n'est écrit à la main dans un diff. Un dépôt « série » part des sources d'origine de Kyber (`git archive`) et pose chaque correctif en un commit, dans l'ordre réel : branche `commun` (Station Windows : serveur puis `apps/macos`), `mac` (plus `server/macos`), `linux` (plus `server/linux`), `web` (kyctl d'origine, `kywebplayer` et `kyctlweb`). Un correctif se modifie dans son commit, les suivants sont rejoués par-dessus, puis chaque `.patch` changé est réécrit depuis son commit. Les correctifs internes de FFmpeg (dans le wrap) ont leur propre dépôt (FFmpeg 8.1 d'origine, un commit par correctif du wrap, une branche par Station). Un dernier contrôle repose toute la série depuis l'origine, depuis les fichiers du dépôt, en `patch -F0`, dans les six ordres : Station Windows, Mac, Linux, moteur web, apps (Mac) et apps (webOS).

Le travail a été partagé en deux : cette session pour kymedia (kyavservice, txproto, libavgrab et libavconv, FFmpeg) et `server/macos/pyrowave` ; une seconde, sur des fichiers disjoints, pour kyctl, kymux et kynput (partie « Rust » ci-dessous), ses commits repris sur cette branche.

## Fait

### Une seule source (KYB-44 à 47, partie kymedia)

| Id | Correctif | Ce qui change |
|---|---|---|
| KYB-44 | 0022-libavgrab-hdr, 0023-libavgrab-latence, 0024-libavgrab-priorite-gpu | `kn_hdr_capture.h`, `kn_hdr_meta.h` et `kn_timing.h` n'existent plus qu'une fois, dans `subprojects/txproto/src` : libavgrab (`dxgi.c`) et libavconv (`enc.c`) les incluent de là (`#include "../../txproto/src/…"`, chemin relatif au fichier, valable pour meson et pour le `configure` de libavconv). 0024-libavgrab ne retouche plus trois copies. |
| KYB-45 (C) | 0025-kymedia-ffmpeg-pyrowave (FFmpeg 0021), Mac 0016 (FFmpeg 0022), Linux 0010 (FFmpeg 0024) | Nouveau `libavcodec/pyrowave_common.h` : options (`FF_PYROWAVE_OPTIONS`), définition et contrôles, budget d'image, en-tête `KNPW`, noms d'erreurs, contrôle de version et mise en paquets des deux encodeurs Vulkan. Les encodeurs Mac et Linux ne gardent que Metal et Vulkan. |
| KYB-45 (Rust), KYB-31 (kyavservice) | 0025-kyavservice, Mac 0017, Linux 0011 | `pyrowave::ENCODER` par système et `engine_encodes(moteur)` (txproto : Windows et Mac ; libavconv : Windows et Linux) au même endroit. `resolve` prend le `fallback_codec` du client (nouveau champ de `KymuxStartVideo`, serde par défaut) au lieu d'imposer H.264. Mac 0017 ne garde que `nv12_capture`, Linux 0011 que `render_node`. |
| KYB-47 (txproto) | 0003-txproto, 0009-libavgrab | Horloge WASAPI dans `kn_audio_clock.h` (txproto), lue aussi par libavgrab (cause de KYB-06). |
| KYB-47 (txproto) | 0020-txproto-cadence | La boucle WGC envoie par `kn_send_frame`, comme la duplication. |

### Les autres points

| Id | Correctif | Ce qui change |
|---|---|---|
| KYB-29 | 0025-kymedia (FFmpeg 0021), 0025-kyavservice ; Mac 0010-txproto, nouveau Mac 0010-kymedia-ffmpeg-hdr-mac (FFmpeg 0031) | En-tête `KNPW` version 2 (48 octets) avec les métadonnées HDR10 de la capture, servi seulement quand l'app le demande (`pyrowave_header` ≥ 2 → option `header_version`) : une app 0.11.7 reçoit toujours la version 1 de 16 octets. Format dans `docs/kyber-changes.md`, « En-tête KNPW version 2 ». Station Mac : la capture joint un mastering display (Display P3, crête = marge EDR × blanc SDR) à chaque image HDR, que VideoToolbox écrit dans le flux HEVC (`MasteringDisplayColorVolume`, `ContentLightLevelInfo`) et PyroWave dans la v2. |
| KYB-01 (Mac) | `server/macos/pyrowave`, Mac 0016 | `kn_pw_frame` met l'image en paquets dans un tampon à la taille du flux encodé plus l'en-tête, puis l'encodeur la copie : même méthode que Windows et Linux. La place d'avant (`budget + n × 64 + 4096`) n'était pas une borne et le port Metal tronquait alors l'image sans le dire. API simplifiée (`kn_pw_encode`, `kn_pw_frame`) ; `kn_pw_packetize` et `kn_pw_config_packet` retirés (l'en-tête est écrit par le code commun de FFmpeg). Commit de PyroWave plus écrit en dur (lu dans `apps/macos/PyroWave/COMMIT`). |
| KYB-17 | 0018-txproto-capture-wgc, 0020 | Après un repli sur la duplication, l'agent WGC est cherché de nouveau toutes les 3 s (sans attente ni journal s'il n'y est pas), puis 6, 12… 60 s tant que WGC s'ouvre sans image ; la duplication n'est arrêtée qu'une fois l'agent trouvé ; une image retenue d'avant est jetée. |
| KYB-40 | FFmpeg 0021, 0022 (par `kn_pw_frame`), 0024 | Mise en paquets en un seul appel, avec une frontière qu'aucune image n'atteint (les blocs sont écrits bout à bout quelle que soit la frontière) : plus de comptage de paquets ni de `memmove`. Linux : conversion par le processeur avec des tables de positions faites une fois par taille, au lieu de deux divisions par point. |
| Écran figé (question de « Réparer une perte sans image clé ») | 0060-txproto-reprise, nouveau Mac 0060-txproto-reprise-mac | L'encodeur vidéo de txproto garde la dernière image reçue ; quand une réparation (`recover`) ou une image clé (`force_idr`) est demandée et qu'aucune image n'arrive en 50 ms (`repeat_ms`), il la réencode avec un pts plus récent (et sans ses mesures de capture). File de l'encodeur : attente bornée et réveillable (`sp_frame_fifo_pop_timed`, `sp_frame_fifo_wake`). Réserve de la capture D3D11 portée de 6 à 7 textures (duplication, WGC, NvFBC) ; ScreenCaptureKit de 3 à 4 surfaces. |

### Partie Rust (kyctl, kymux, kynput)

| Id | Correctif | Ce qui change |
|---|---|---|
| KYB-31, KYB-45 | 0025-kycontroller ; Mac 0018 et Linux 0012 **supprimés** | PyroWave proposé selon le moteur de `[kyavserver] backend`, même règle que `engine_encodes` ; kycontroller passe le `fallback_codec` du client et la version d'en-tête à `KymuxStartVideo`. |
| KYB-29 | 0025-kyclient, 0025-kyctlweb, 0025-kycontroller | `StartRequestVideo.pyrowave_header` (serde par défaut, ignoré d'une Station plus ancienne), réglé pour toute l'app par `set_pyrowave_header` (Rust, C, kyclient-rs) ; l'en-tête entre dans l'identité de l'encodeur : une app 0.11.7 ne rejoint jamais une image d'invité en v2. Pas de champ dans `VideoSessionConfig` : le littéral du cœur (`apps/core/src/session.rs`) compile tel quel. |
| KYB-25, KYB-49 | apps/macos 0001-kyclient | `PacketSink` n'est plus `Copy` ni `Clone` : `Closed` part une fois, par son flux ou à son abandon s'il n'a jamais servi (départ refusé, point manquant, erreur). Champs publics gardés pour la phase 2b. Les trois fonctions C sans appelant sont retirées. |
| KYB-37 | 0020-kycontroller | Révision `kn_qrev` de la qualité d'un invité (jeton) et `rev` de `POST /kybernow/quality` ; sans elle, comme avant. Admin doit l'écrire (ci-dessous). |
| KYB-37 | 0070-kycontroller | Boîte `colr nclx` tirée de la VUI du SPS HEVC ; liste d'édition (`elst`) pour le pré-saut Opus. |
| KYB-39 | 0050-kynet | Numéro écrit sur place dans un tampon qui lui garde la place (plus d'allocation ni de copie par datagramme) ; pertes récentes dans un anneau indexé, `MAX_GAP` 2 048. |
| KYB-40 (web) | 0025-kyctlweb | Plus de décodeur WebCodecs créé pour rien en PyroWave. |
| KYB-47 | 0005, 0019, 0020, 0060-kycontroller ; 0027, Mac 0020, Linux 0006 | Une seule boucle vers les flux vidéo d'une session (`to_video_streams`, pour `/force_idr` et `/recover`) ; 0019 n'ajoute plus ce que 0020 retirait ; `PenAvailable` en une règle dans 0027 ; `send_inputs` réutilise `switch_desktop`. |
| STWIN-50 | 0085 → **0086-kycontroller-micro-droits** | Renuméroté (toujours posé après 0085-kycontroller-flux-montant) ; citations mises à jour dans `docs/micro(.en).md`, `server/admin/src/lend_api.rs` et `share_api.rs`. |

## Laissé, et pourquoi

- **Écran figé sous Linux** : la Station Linux encode avec libavconv, pas avec txproto (`encode.c`) ; là, `VideoRecover` et `VideoForceIdr` passent par un crochet de kyavservice appelé à l'arrivée d'une image (`encoder_hook.rs`). Réencoder sans image demande une attente bornée dans l'ordonnanceur de libavconv (`sched.c`, `thread_queue.c`) et une commande de plus dans `libavconv-rs`, à écrire sans pouvoir l'essayer (pas de VM). À faire avec un essai sur une Station Linux ; la capture PipeWire n'envoie elle aussi rien sur un écran immobile.
- **KYB-40, Windows** (une copie GPU par image dans `libpyrowave`) : importer chaque texture de la réserve de capture demande un essai sur PC (droits de partage des textures de la capture, carte par carte).
- **KYB-40, Mac** : la `MTLTexture` créée par image (pas de copie, quelques microsecondes) n'est pas mise en cache : la garder retiendrait les IOSurface de ScreenCaptureKit hors de sa réserve, pour un gain non mesuré. L'attente CPU après la conversion attend une entrée « tampon de commandes ou `MTLSharedEvent` » dans PyroWave (STMAC-21, zone de l'app Mac).
- **KYB-41** : le SEI de reprise inséré par `memmove` (une centaine de microsecondes par seconde de vidéo) passerait par `sei_data` de NVENC, mais pas pour VideoToolbox, AMF ni QSV : deux chemins à la place d'un, à essayer sur PC. Les deux petites allocations de données annexes HDR par image n'ont pas de coût mesurable.
- **KYB-43** (Sway, Hyprland : `copy` sans dommages) : revenir à `copy_with_damage` arrête l'image sur un bureau immobile ; il faut l'envoi périodique de la dernière image dans la capture Wayland, à essayer sur ces compositeurs (pas de VM ici).
- **KYB-47, kynput** (module DualSense commun à Mac 0011 et Linux 0005, `locate()` du Mac en un module avec cache, aides Objective-C) : le code Linux ne se compile pas ici (ni VM ni sysroot) et les entrées de la Station Mac ne s'essaient pas sans session ; refondre environ 250 lignes de pilotes uhid et CoreHID à l'aveugle n'est pas sûr. À faire avec une Station Mac en session et la VM Linux.
- **KYB-47, `set_bitrate`** : garde sa boucle, car il répartit le débit entre les flux et garde la configuration de chacun.
- **KYB-46** (mêmes morceaux dans 0025-kyclient et 0025-kyctlweb, 0020-kyctlweb et apps 0001) : il faut d'abord une famille `*-kyctlcommon-*` dans les scripts de pose, hors de ma zone : `apply.sh` (`repo_of` → kyctl, rien dans `client_repo_of`), la boucle du moteur web de `build.sh` (avec `kywebplayer` et `kyctlweb`, sur la copie d'origine) et `scripts/kyber-client-patches.sh` (`kyber_client_repo`, `kyber_station_patches`, `kyber_known_client_patches`). Ensuite seulement, les morceaux communs y seront déplacés.
- **Admin** (hors zone) : écrire `kn_qrev` dans les jetons d'invité, relevé à chaque changement de qualité comme `kn_rev`, et le même nombre en `rev` de `POST /kybernow/quality`.
- **Cœur** (session v2-coeur) : l'essai de `apps/core/src/media.rs` envoie `Closed` à la main puis lâche le sink : avec KYB-25, cela fait deux `Closed`, donc une double libération ; détail dans la note `v2-kyber-packetsink.md` du dossier partagé.
- **Citations périmées hors zone** : `PROJECT(.en).md` (l. 114-115) et `apps/macos/README(.en).md` (l. 91) citent les fonctions C retirées (KYB-49) ; `docs/station-linux(.en).md` (l. 210) cite 0012-kycontroller-pyrowave-linux, supprimé ; le commentaire de `server/kyber-patches/apply.sh` (l. 14) cite 0085-kycontroller-micro-droits.
- **Observé, hors de mes changements** : en 4:4:4, sur une image de bruit très dense (`noise=alls=60` et plus), le port Metal de PyroWave plafonne vers 4,4 Mo par image en 1080p puis rend des images de 50 à 100 Ko, avec l'ancienne Station comme avec la nouvelle (`ffmpeg … -vf format=bgra,noise=alls=60,hwupload -c:v libpyrowave_metal -chroma444 1 -b:v 4000M -f framecrc -`). En 4:2:0, la taille suit le budget. À regarder dans `apps/macos/PyroWave` (zone de l'app Mac).

## Vérifié, et comment

- **Pose** : toute la série se pose depuis l'origine, en `-F0`, dans les six ordres (contrôle `verifier.sh` de mon arbre de travail).
- **Station Mac** (`build-kyber.sh` dans mon arbre, avec les vrais scripts, mode strict) : compilation complète (FFmpeg avec `libpyrowave_metal` et `videotoolboxenc`, txproto, libavconv, kyavserver, kynput, kycontroller).
  - `kybernow-pyrowave-check` : aller-retour encodé puis décodé, PSNR 55,2 dB.
  - `ffmpeg` de la Station, mire 1080p et 4K vers `libpyrowave_metal` : en-tête de 16 octets sans option, de 48 avec `-header_version 2` ; images 4:2:0 identiques octet pour octet (même CRC) à celles de la Station d'avant ; bruit 4K à 4 Gbit/s : images de 8,33 Mo, le budget, sans erreur.
- **Surcouche Windows** (`build.sh`, MinGW) : voir plus bas.
- **Essais Rust** (Mac, toute la série posée) : kyavservice 37 réussis (dont les 7 de `pyrowave.rs` : moteurs, repli du client, en-tête v2, `nv12_capture`) ; `rtp_start_audio_error_reports_audio_pipeline_type`, essai d'origine qui attend un échec de la capture audio RTP, échoue sur ce Mac où la capture réussit : il ne touche à rien de ces changements. kycontroller 142 réussis (nouveaux : moteurs PyroWave, en-tête, invité v1 face à une image v2, `kn_qrev`, couleurs de la VUI, `colr`, `elst`) ; kyclient 18 (dont `MediaSinks` et `PacketSink`) ; kyproto 33 (dont l'anneau des pertes) ; kymux-types 3 ; kynput 73 (le module d'essais d'origine `macos/client_keyboard_mouse.rs`, qui ne compile pas, écarté le temps de l'essai). Moteur web : `cargo check` en wasm32. kynput pour Windows : `cargo check`. Un MP4 écrit par l'enregistreur (HEVC 10 bits HDR10 de x265 et Opus) : ffprobe lit BT.2020 PQ plage réduite et un son qui commence à −6,5 ms. kyclient-rs n'est pas compilé : ses liaisons viennent d'un en-tête installé périmé, déjà avant.
- **Essais ajoutés** (C, sur le Mac) : en-tête `KNPW` v1 et v2 (`pyrowave_common.h`, 9 vérifications) ; horloge audio commune (8 cas) ; `sp_frame_fifo_pop_timed` et `sp_frame_fifo_wake` sur la libtxproto compilée (5 cas : délai, réveil d'un autre fil, image poussée pendant l'attente, réveil anticipé, fin de flux) ; `pw_convert` à tables identique à l'ancienne version sur 20 cas (tailles, 4:2:0 et 4:4:4, BGR et RGB).
- **Pas essayé** : le réencodage de la dernière image en session réelle (il faut une Station et une app), la reprise de WGC (il faut un PC et un agent relancé), les métadonnées HDR dans un flux HEVC de la Station Mac lues par un appareil.

## À essayer sur appareil

- **PC** : arrêter l'agent WGC d'un écran secondaire puis le relancer (Admin) : l'écran revient en WGC dans les 3 s ; écran immobile, couper le Wi-Fi une seconde : l'image est réparée sans attendre un mouvement (journal « No image since … ms: the last one again ») ; PyroWave en HDR avec une app 0.11.7 (en-tête v1, comme avant).
- **Mac** : HEVC HDR10 vers un appareil (métadonnées présentes, image moins terne sur un écran peu lumineux) ; écran immobile et perte : même essai que sur PC ; capture sans saccade avec 4 surfaces.
- **Linux** : PyroWave en profil Max (code commun) ; image d'un invité par le chemin processeur.
- **Repli** : Station Mac en `backend = "libavconv"`, Station Linux en txproto, app qui demande PyroWave avec repli HEVC : HEVC reçu, pas H.264.
- **Apps** : départ refusé (invité sans image, 409) répété : mémoire stable ; enregistrement HDR lu par QuickTime, VLC ou un téléviseur (vu en HDR, son calé) ; canal de retour à 100 Mbit/s et pertes en rafale ; stylet sur les trois Stations ; une qualité d'invité poussée par Admin qui cède à un jeton plus récent (quand Admin écrira `kn_qrev`).

## Notes de version

**Français**
- Une image perdue sur un écran immobile est réparée tout de suite, sans attendre que quelque chose bouge (Stations Windows et Mac).
- Un écran secondaire repasse en capture Windows Graphics Capture quand son agent redémarre, au lieu de rester décalé ou noir jusqu'à la fin de la session.
- Sur la Station Mac, le HDR10 indique la luminosité de l'écran du Mac : l'image s'adapte mieux aux écrans moins lumineux.
- PyroWave : la Station Mac ne tronque plus une image très détaillée, et le repli quand PyroWave n'est pas possible respecte le format choisi par votre appareil.
- Une app qui redémarre après un refus de la Station ne garde plus de mémoire en trop.
- Les enregistrements HDR sont reconnus comme tels par les lecteurs, et leur son est calé à l'image près.

**English**
- An image lost on a still screen is repaired right away, without waiting for something to move (Windows and Mac Stations).
- A secondary display goes back to Windows Graphics Capture when its agent restarts, instead of staying offset or black until the end of the session.
- On the Mac Station, HDR10 carries the brightness of the Mac's display: the picture adapts better to dimmer screens.
- PyroWave: the Mac Station no longer truncates a very detailed image, and the fallback when PyroWave is not possible follows the format your device chose.
- An app that starts again after the Station refused it no longer keeps memory it does not need.
- HDR recordings are recognised as HDR by players, with their sound in sync to the frame.
