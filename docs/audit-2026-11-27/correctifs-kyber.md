# Audit du 27/11/2026 : correctifs de Kyber

Zone : les correctifs posés sur les sources de Kyber 0.29. Ils sont environ 42 700 lignes de `.patch` réparties ainsi :

- `server/kyber-patches` : 59 correctifs, plus `upstream/` ;
- `server/macos/kyber-patches` : 21 ;
- `server/linux/kyber-patches` : 11 ;
- `apps/macos/kyber-patches` : 6 ;
- `apps/webos/kyber-patches` et `apps/webos/sdl-patches` : 1 chacun.

S'y ajoutent les scripts qui les posent : `server/kyber-patches/apply.sh`, `build.sh`, `server/macos/scripts/build-kyber.sh`, `server/linux/scripts/build-kyber.sh` et `scripts/kyber-client-patches.sh`.

**Méthode.** L'audit est une lecture statique seulement : rien n'a été compilé ni lancé. Six relectures ont tourné en parallèle, une par famille :

1. canal de retour et reprise ;
2. flux montant et enregistrement ;
3. PyroWave et HDR ;
4. partage et image invité ;
5. capture et encodage ;
6. kynput.

Les scripts, les doublons, la série `upstream/` et la doc ont été relus à part. Chaque constat retenu a été relu dans le correctif et, si besoin, dans les sources d'origine (`third_party`).

**Références et confiance.** Les numéros de ligne sont ceux du fichier `.patch`. La confiance vaut « vérifié » quand tout le chemin du code a été lu. Elle vaut « plausible » quand le code est lu mais que le scénario dépend d'un comportement à l'exécution qui n'a pas été observé.

Les points réglés par l'audit du 20/10 (`docs/archives/audit-2026-10-20.md`) ne sont pas repris. Seule exception : l'écran retiré (KYB-07), signalé à l'époque et resté ouvert.

## Synthèse

| Id | Catégorie | Correctif | Constat | Effort | Confiance |
|---|---|---|---|---|---|
| KYB-01 | bug critique | 0025-kymedia-ffmpeg-pyrowave, Linux 0010 | PyroWave écrit au-delà du tampon de sortie (Windows, Linux) | S | vérifié |
| KYB-02 | sécurité | 0085-kycontroller-flux-montant | Un paquet montant fait allouer jusqu'à 4 Gio sur la Station | S | vérifié |
| KYB-03 | sécurité | 0019-kycontroller-partage, 0019-kynput-partage | Un invité clavier-souris agit sur tous les écrans de l'hôte | M | vérifié |
| KYB-04 | sécurité | 0060-kycontroller-reprise, 0061 | Un invité force des images clés sur l'image de l'hôte sans espacement | S | vérifié |
| KYB-05 | bug | 0001 / txproto (moteur par défaut) | Pas de son quand Windows sort en 5.1 « side » : le correctif amont n'est pas posé | S | vérifié |
| KYB-06 | bug | 0003, 0009 | Un horodatage WASAPI marqué en erreur est quand même utilisé | S | vérifié |
| KYB-07 | bug | txproto DXGI | Un écran retiré fige le flux pour toujours : le correctif amont n'est pas posé | S | vérifié |
| KYB-08 | bug | 0050-kynet-canal-de-retour | La déduplication des pertes par seuil saute des pertes | S | vérifié |
| KYB-09 | bug | 0050 (redondance du son) | `assert!` du lecteur atteignable si le MTU change | S | plausible |
| KYB-10 | bug | 0085-kycontroller-micro-droits | Le droit micro accordé en session est sans effet ; son retrait n'est pas signalé | M | vérifié |
| KYB-11 | bug | 0070-kycontroller-enregistrement | Après une perte, des images indécodables sont écrites dans le MP4 | S | vérifié |
| KYB-12 | bug | 0070 | Un changement du nombre de voies est écrit sous l'ancien en-tête | S | vérifié |
| KYB-13 | bug | 0070 | Deux `POST /recording` simultanés lancent deux enregistreurs | S | vérifié |
| KYB-14 | bug | 0070 | Écran immobile : le son reste en mémoire, jamais écrit | S/M | vérifié |
| KYB-15 | bug | 0070 | Aucune limite de place disque | S/M | vérifié |
| KYB-16 | bug | Mac 0008-txproto-cadence-invite | La fréquence plafonnée de l'invité est ignorée par l'encodeur (VBV, budget PyroWave) | S | vérifié |
| KYB-17 | bug | 0018-txproto-capture-wgc | Un écran secondaire passé en DXGI n'en ressort jamais | M | plausible |
| KYB-18 | bug | Mac 0006, 0013 (son) | Allocations, journaux et mutex dans le fil temps réel de Core Audio | M | plausible |
| KYB-19 | bug | Mac 0006 | Nombre de voies figé à la création de l'entrée | S | plausible |
| KYB-20 | bug | Linux 0002-kynput-pointeur-wayland | Bouton de souris bloqué au passage absolu → relatif | S | vérifié |
| KYB-21 | bug | 0019-kynput-partage | Presse-papiers accordé en cours de partage sans effet | M | vérifié |
| KYB-22 | bug | 0027-kynput-stylet | Stylet (et touches) non relâchés à la déconnexion du client | S | plausible |
| KYB-23 | bug | 0019-kynput-partage | La liste `injected` peut laisser passer un paquet sans droit | S | plausible |
| KYB-24 | bug | 0014 + 0020-kyctlweb-image-cle | Décodeur web : redémarrages sans fin, écran noir, une IDR par seconde | S | vérifié |
| KYB-25 | bug | apps/macos 0001-kyclient-paquets-audio-video | `PacketSink` est `Copy` sans `Drop` : fuite si la tâche ne démarre pas | M | vérifié |
| KYB-26 | bug | Mac 0011-kynput-manettes-mac | `Drop` de l'aide : stdin jamais fermé, attente de 1 s puis SIGKILL | S | vérifié |
| KYB-27 | bug | Mac 0004-kynput-presse-papiers-mac | Presse-papiers du Mac vidé dès l'annonce, avant les données | S | vérifié |
| KYB-28 | sécurité | 0019-kynput, Linux 0005, Mac 0011 | Nombre de manettes virtuelles (uhid, CoreHID) illimité | S | vérifié |
| KYB-29 | bug | 0025 (HDR) | PyroWave HDR10 sans métadonnées MDCV/CLL ; idem en HEVC sur la Station Mac | M | vérifié |
| KYB-30 | bug | 0025-kymedia-ffmpeg-pyrowave | Poignées NT (texture, fence) perdues quand l'import échoue | S | vérifié |
| KYB-31 | bug | 0025-kyavservice, Mac 0017, Linux 0011 | Le repli de PyroWave ignore le codec demandé par le client selon le moteur | S | plausible |
| KYB-32 | bug | 0025-kyberweb, 0025-kymedia | Divers PyroWave : 4:4:4 web refusé, `s->hdr` figé, tampon perdu (Mac 0010) | S | plausible |
| KYB-33 | bug | 0016, 0020 | Débits non bornés (`as i32` négatif, plafond `u32::MAX` pour un invité) | S | vérifié |
| KYB-34 | bug | 0019-kycontroller-partage | Course dans `post_close` : un serveur d'entrées tué sans relâcher | S | plausible |
| KYB-35 | sécurité | 0050 | Rapports de pertes d'un lecteur non limités (CPU de la Station) | S | vérifié |
| KYB-36 | bug | 0050, 0051, 0052, 0085 | Compatibilité : version du canal non vérifiée, commentaire faux, `uplink` non dédoublonné | S | vérifié |
| KYB-37 | bug | divers | Mineurs (journaux faux, reprise Mac, qualité sans révision, kynput) | S | vérifié |
| KYB-38 | sécurité | origine kynput (hors correctifs) | `clipboard.rs` et `control_event.rs` paniquent sur un paquet court, même d'un spectateur | S | vérifié |
| KYB-39 | optimisation | 0050 | Une allocation et une copie par datagramme pour ajouter 4 octets | M | vérifié |
| KYB-40 | optimisation | 0025, Mac 0016, Linux 0010 | PyroWave : boucle `memmove` inutile, copie GPU par image, attentes CPU | S-M | vérifié |
| KYB-41 | optimisation | 0060-txproto, 0022-txproto | SEI insérés par `memmove` de toute l'image ; allocations par image | M | vérifié |
| KYB-42 | optimisation | 0020, 0016 | Appels Win32 sous verrou ; images clés non espacées entre invités | S | vérifié |
| KYB-43 | optimisation | Linux 0013 | Sway et Hyprland : `copy` sans dommages, bureau immobile encodé en continu | M | plausible |
| KYB-44 | factorisation | 0022, 0023, 0024 | `kn_timing.h` en 3 copies, `kn_hdr_*.h` en 2 copies identiques | M | vérifié |
| KYB-45 | factorisation | 0025, Mac 0016-0018, Linux 0010-0012 | PyroWave en triple (C et Rust) : origine directe de KYB-01 | M | vérifié |
| KYB-46 | factorisation | 0025-kyclient / 0025-kyctlweb ; 0020-kyctlweb / apps 0001 | Mêmes hunks dans deux correctifs | S | vérifié |
| KYB-47 | factorisation | kynput Mac et Linux, txproto, kycontroller | DualSense, `locate`, aides ObjC, envoi WGC, horloge WASAPI | M | vérifié |
| KYB-48 | code mort | `server/linux/scripts/build-kyber.sh:61-74` | Pose du stylet des apps déjà faite par `apply.sh` | S | vérifié |
| KYB-49 | code mort | apps/macos 0001 | Trois fonctions C sans appelant (et dangereuses avec KYB-25) | S | vérifié |
| KYB-50 | doc | scripts de pose | Ordre différent entre la Station et les apps ; dépendance de 0085 à l'ordre alphabétique | S | vérifié |
| KYB-51 | doc | docs/kyber-changes.md | Points à corriger ou à ajouter | S | vérifié |

## Constats

### KYB-01 — Bug critique : PyroWave écrit au-delà du tampon de sortie (Windows, Linux)

- **Où**
  - `server/kyber-patches/0025-kymedia-ffmpeg-pyrowave.patch:672-676`
  - `server/linux/kyber-patches/0010-kymedia-ffmpeg-pyrowave-vulkan.patch:736-740`
- **Description**
  - Le tampon est alloué à `nb_packets * packet_size` (`ff_get_encode_buffer`), puis `pyrowave_encoder_packetize(..., pkt->data, capacity)` est appelé.
  - Dans `third_party/pyrowave/pyrowave_encoder.cpp:1102`, `packetize` ignore la taille : `(void)size;`. Seul un `assert` la vérifie (`:1141`), et il disparaît car `pyrowave.sh:57` compile en `Release`.
  - Un bloc 32×32 plus gros que la frontière (1 152 octets) ouvre un paquet à lui seul, plus gros que la frontière (`:1131-1137`). L'écriture totale (en-tête + somme des blocs) peut donc dépasser `n × 1152`.
  - Le Mac a relevé le défaut et le contourne (`server/macos/pyrowave/kn_pyrowave_metal.mm:389-393` : « la place se compte sur le budget, pas sur n × packet_size »). Ce n'a jamais été reporté sur Windows ni sur Linux.
- **Scénario** : profil Max ou Auto en filaire, contenu très détaillé ou 4:4:4. Deux blocs de 2 000 octets donnent `n = 2`, donc 2 304 octets alloués pour plus de 4 000 écrits. Le tas de libavcodec est corrompu et kyavservice plante par intermittence. Le défaut est indétectable aux essais à débit modéré.
- **Correctif** : allouer selon la taille réelle (`pyrowave_encoder_get_mapped_raw_bitstream()` / budget + en-tête + `get_meta_required_size()`), comme sur le Mac. Mieux, un seul code commun (KYB-45).
- **Effort** : S. **Confiance** : vérifié. Jamais reproduit sur un PC.

### KYB-02 — Sécurité : un paquet montant fait allouer jusqu'à 4 Gio sur la Station

- **Où**
  - `0085-kycontroller-flux-montant.patch:243` (`register_data_endpoint`) et `:401` (`recv`).
  - Le défaut lui-même est dans l'origine : `kysdk/kymux/kymux-types/src/data.rs:52`, `BytesMut::zeroed(size as usize)` avec `size` lu tel quel dans l'en-tête (u32).
- **Description** : avant 0085, aucun point de données n'était servi par kycontroller. 0085 ouvre cette surface. Les plafonds des aides (64 Kio pour le micro, 8 Mio pour la caméra) arrivent après l'allocation.
- **Scénario** : un appareil de l'hôte, un ami qui emprunte ou un invité avec `kn_mic`, bogué ou malveillant, envoie un en-tête `FF FF FF FF` sur le point caméra et sur le point micro. Résultat : 8 Gio demandés par session. Soit kycontroller avorte et toutes les sessions tombent, soit la mémoire du PC est saturée.
- **Correctif** : plafonner `DataPacket::read` (par exemple à 16 Mio, erreur `InvalidData`) par un petit correctif kymux posé avec 0085. Le plafond est à proposer aussi en amont.
- **Effort** : S. **Confiance** : vérifié.

### KYB-03 — Sécurité : un invité clavier-souris agit sur tous les écrans de l'hôte

- **Où**
  - `0019-kycontroller-partage.patch:1040-1049` : `start_input` reçoit `HostConfig { display_list }` complet, et le `statemachine` d'origine renvoie la liste complète à chaque changement.
  - `0019-kynput-partage.patch:207` : `MousePosition` (et `Pen`, depuis 0027) passe sur le seul droit `keyboard_mouse`, sans contrôle du `display_id`.
- **Scénario** : l'hôte partage l'écran 2. Un invité `kbm` envoie `MousePosition{display_id: 1}` et clique sur l'écran privé de l'hôte. Les paquets `Cursor` de l'écran 1 lui donnent en plus la position du pointeur là-bas. C'est contraire à « seulement l'écran du partage » (`docs/kyber-changes.md`, 0019).
- **Correctif**
  - Pour une session `share`, ne passer que l'écran de l'invité à `HostConfig`, au démarrage comme à la mise à jour.
  - Dans `State::on_packet`, refuser `MousePosition` et `Pen` d'un autre `display_id`.
  - Borner ou documenter les déplacements relatifs (`MouseMove`), qui peuvent franchir le bord de l'écran.
- **Effort** : M. **Confiance** : vérifié pour les positions absolues et le curseur ; plausible pour le relatif.

### KYB-04 — Sécurité : un invité force des images clés sur l'image de l'hôte

- **Où**
  - `0060-kycontroller-reprise.patch:164-168` : `repair` traite l'invité avant le test `single`.
  - `0060-kycontroller-reprise.patch:89-108` : `guest_recover` appelle `video_recover` sur toutes les images de l'invité, celle de l'hôte comprise.
  - S'y ajoutent `0061-kycontroller-reprise-canal-de-retour.patch:28` et `0060-txproto-reprise.patch:286-290`.
- **Description**
  - `/force_idr` d'un invité passe par `keyframe_for_guest`, qui espace les images clés de 2 s sur l'image partagée, puis par `lag_sign`.
  - `/recover` (et la réparation automatique de 0061) n'a aucune de ces limites.
  - Tout encodeur sans référence longue (H.264, AMF, QSV, x264, moteur avconv, VideoToolbox) répond à chaque demande par une image clé (`kn_recovery_before_frame` → -1). L'anti-doublon de 200 ms ne filtre que les bornes plus anciennes.
- **Scénario** : un invité en Wi-Fi qui perd en rafales, ou un client modifié qui boucle sur `POST /recover` avec `last_pts = u64::MAX`, déclenche une IDR à chaque image sur le flux de l'hôte. La qualité et le débit de l'hôte s'effondrent.
- **Correctif**
  - Faire passer `guest_recover` par `keyframe_for_guest` et `guest_keyframe_delay` sur les clés `Shared`, et compter un `lag_sign`.
  - Appliquer `single` aux invités.
  - Mettre `FORCE_IDR_MIN_INTERVAL` (150 ms) sur `/recover`.
  - Côté txproto, au plus une image clé de repli toutes les 150 ms.
- **Effort** : S. **Confiance** : vérifié.

### KYB-05 — Bug : pas de son avec txproto quand Windows sort en 5.1 « side »

- **Où** : `0001-kyavservice-son-hd.patch:217-257`, qui garde la disposition capturée. Le correctif qui manque est `upstream/txproto/0005-encode-fall-back-to-a-channel-layout-libopus-takes.patch`.
- **Description** : libopus refuse toute disposition hors de l'ordre Vorbis, et `avcodec_open2` échoue. libavconv est protégé (`0008:84-90`, `output_layout`), txproto (le moteur par défaut) non. `docs/kyber-changes.md:180` le reconnaît : « aucun » équivalent KyberNow. Le correctif n'existe que dans la série destinée à l'amont, que rien ne pose (`grep upstream` sur tous les scripts : aucune occurrence).
- **Scénario** : ampli ou barre de son réglé sur « 5.1 Surround » dans Windows, ou sortie virtuelle à voies latérales. La Station est muette.
- **Correctif** : ajouter une version interne de `upstream/txproto/0005` (`encode.c` seul).
- **Effort** : S. **Confiance** : vérifié (absence du correctif).

### KYB-06 — Bug : un horodatage WASAPI en erreur est utilisé

- **Où** : `0003-txproto-son-continu.patch:45` et `0009-libavgrab-son-continu.patch:114`, qui prennent `qpc` dès qu'il n'est pas nul.
- **Description** : les versions amont excluent `AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR` (`upstream/txproto/0001…:73`, `upstream/libavconv/0001…:126`). Les correctifs réellement posés ne l'ont jamais repris. La doc présente la vérification comme un ajout de la série amont.
- **Scénario** : changement de périphérique ou sortie de veille, pilote qui signale l'erreur. La resynchronisation se cale sur une position fausse : saut des pts, « Queue input is backward in time », décalage du son.
- **Correctif** : une ligne `qpc_valid` dans chacun des deux fichiers.
- **Effort** : S. **Confiance** : vérifié.

### KYB-07 — Bug : un écran retiré fige le flux (DXGI, txproto)

- **Où** : boucle DXGI de txproto. Le correctif qui manque est `upstream/txproto/0004-dxgi-stop-with-an-error-when-the-captured-output-is-.patch`.
- **Description** : signalé par l'audit du 20/10 (§15), corrigé seulement dans la série amont, « jamais ajouté à la surcouche » (même audit, ligne 503). La Station Mac a son équivalent (Mac 0001, `is_unplugged`).
- **Scénario** : un écran virtuel SudoVDA est retiré, ou un HDMI débranché, pendant une session multi-écrans. Le client garde une image figée sans aucun événement.
- **Correctif** : reprendre `upstream/txproto/0004` en interne.
- **Effort** : S. **Confiance** : vérifié pour l'absence du correctif ; scénario plausible.

### KYB-08 — Bug : la déduplication des pertes saute des pertes

- **Où** : `0050-kynet-canal-de-retour.patch:907-915` (`HostState::new_loss`, seuil haut), utilisé en `:969-983`.
- **Scénario**
  - Les pertes 3 et 4 sont signalées. Le datagramme `Lost(3)` se perd ou arrive après `Lost(4)`.
  - Le rapport suivant répète [3, 4], mais 3 est jugé ancien et n'est jamais traité.
  - Avec 0061, seule la perte 4 est réparée : l'image réparatrice peut dépendre de la perte 3. L'image reste corrompue jusqu'au `/recover` de l'app (un aller-retour, jusqu'à 1 s).
- **Correctif** : une fenêtre des identifiants déjà traités (anneau de 64 ou bitmap) au lieu du seuil.
- **Effort** : S. **Confiance** : vérifié.

### KYB-09 — Bug : la redondance du son peut faire paniquer le lecteur

- **Où** : `0050:1745-1764` (envoi), `0050:1815-1835` (réception). L'`assert!(self.total_segments.is_none())` est dans `set_segment` (`audio_unreliable.rs:691`, origine).
- **Description** : la copie du paquet précédent est envoyée en « segment 0 fin », en supposant qu'il était parti d'un seul tenant. Or `max_datagram_size` peut grandir entre deux paquets (découverte du MTU par QUIC).
- **Scénario** : un paquet de son d'environ 1 250 octets (multicanal à haut débit) part en deux segments. Le MTU grandit et la copie arrive en segment unique : la tâche `process` panique et le son est perdu pour la session.
- **Correctif** : n'utiliser la forme combinée que si le précédent est parti en un datagramme ; à la réception, rejeter le segment incohérent au lieu de l'`assert`.
- **Effort** : S. **Confiance** : plausible.

### KYB-10 — Bug : le droit micro d'un invité ne suit pas la session

- **Où** : `0085-kycontroller-micro-droits.patch:177-182` (point micro enregistré seulement si le droit existe au démarrage) et `:262-269` (MEDIA jeté sans STATUS).
- **Scénario**
  - L'hôte accorde « Laisser les invités parler » à un invité déjà connecté : aucun point micro n'existe tant que l'invité ne relance pas le flux, contrairement à `POST /kybernow/role` (`mic`) décrit dans la doc.
  - Au retrait du droit, l'app affiche toujours « Micro » allumé et continue d'envoyer pour rien.
- **Correctif**
  - Toujours enregistrer le point micro d'un invité quand l'aide existe.
  - Contrôler le droit au START (STATUS `refused/not_allowed`).
  - Remplacer l'`AtomicBool` par un `watch<bool>` surveillé par `serve`, qui envoie un STATUS à chaque changement.
- **Effort** : M. **Confiance** : vérifié.

### KYB-11 — Bug : après une perte, l'enregistrement écrit des images indécodables

- **Où** : `0070-kycontroller-enregistrement.patch:1485-1488` (drapeau `behind` posé à l'envoi) et `:1625-1629` (lu par l'écrivain).
- **Description** : `behind` est un drapeau partagé, pas une marque dans la file. L'écrivain le lit sur le prochain paquet qu'il traite, qui précède la perte (jusqu'à 2 048 paquets en file devant elle).
- **Scénario** : la file contient `[K, P1, P2]`, puis des paquets sont perdus, puis arrive `P3`. L'écrivain passe en `need_key` trop tôt, reprend à K, écrit P1, P2, puis P3 qui dépend d'images absentes. Le fichier a des artefacts jusqu'à l'image clé suivante. C'est justement le cas visé : un disque lent.
- **Correctif** : porter la perte sur le prochain `Msg::Video` mis en file (`Msg::Video(p, at, after_loss)`).
- **Effort** : S. **Confiance** : vérifié.

### KYB-12 — Bug : un changement du nombre de voies est écrit sous l'ancien en-tête

- **Où** : `0070:1699-1711`. Le commentaire dit « the sound stops in this file », mais `self.audio` est seulement mis à jour, et les paquets suivants sont écrits dans la piste dont le `dOps` porte l'ancien nombre de voies.
- **Scénario** : le son passe de stéréo à 5.1 en cours de partie. Le son du fichier est illisible ou bruité jusqu'à la fin.
- **Correctif** : drapeau `audio_frozen` jusqu'au prochain `open()`, ou nouvelle partie comme pour la vidéo.
- **Effort** : S. **Confiance** : vérifié. Cas rare.

### KYB-13 — Bug : deux `POST /recording` simultanés

- **Où** : `0070:1842` (test `r.active.is_some()`), puis l'installation de l'enregistreur en `:1894-1914`, séparée du test par des `await`.
- **Scénario** : double clic, ou deux appareils au même moment. Le second écrase `r.active`, le premier fichier devient orphelin, et le superviseur du premier peut se poser sur le second.
- **Correctif** : réserver la place (`Starting`) dans le même verrou, avant le premier `await`.
- **Effort** : S. **Confiance** : vérifié.

### KYB-14 — Bug : écran immobile, le son n'est plus écrit

- **Où** : `0070:878` et `:945`. `flush_fragment` n'est appelé que par `write_video` et `finish`.
- **Scénario** : film en pause ou bureau figé pendant 20 minutes, puis coupure de courant. Tout le son depuis la dernière image est perdu, ce qui contredit « se lit jusqu'à sa dernière seconde ». La taille affichée ne bouge plus.
- **Correctif** : vider aussi un fragment quand le son dépasse environ 1 s, ou sur le délai de 500 ms de `run`.
- **Effort** : S/M. **Confiance** : vérifié dans le code. Que l'encodeur n'envoie aucune image sur un écran figé reste plausible.

### KYB-15 — Bug : aucune limite de place disque pour l'enregistrement

- **Où** : `0070:1590-1601`. Seul un échec d'écriture arrête l'enregistrement.
- **Scénario** : un enregistrement oublié remplit le disque système (Vidéos). Windows et kycontroller en souffrent avant l'arrêt. Le nettoyage à 30 jours d'Admin ne couvre pas ce cas.
- **Correctif** : contrôler la place libre au démarrage puis toutes les N secondes dans le superviseur (arrêt sous 5 Go, `EndReason::Disk`).
- **Effort** : S/M. **Confiance** : vérifié.

### KYB-16 — Bug : sur la Station Mac, la cadence plafonnée d'un invité est ignorée par l'encodeur

- **Où** : `server/macos/kyber-patches/0008-txproto-cadence-invite.patch:19-21`. Il plafonne ScreenCaptureKit (`setMinimumFrameInterval`), mais `capture->framerate` reste la fréquence de l'écran (`iosys_screencapturekit.m:887`). Elle passe ensuite dans `fe->avg_frame_rate` (`:274`), puis dans `avctx->framerate`.
- **Conséquences**
  - `0030-kymedia-ffmpeg-vbv-une-image.patch:142-148` dimensionne le tampon VBV pour une image à 60 i/s alors que l'invité est à 30. VideoToolbox limite chaque image à la moitié de sa part.
  - Même effet sur le budget par image de PyroWave Metal (`Mac 0016:257`, `avctx->framerate`).
  - Windows fait le bon calcul (`0020-txproto-cadence.patch:47-55`, `kn_frame_rate`).
- **Correctif** : dans 0008, ramener `capture->framerate` à `min(fréquence de l'écran, max_framerate)`.
- **Effort** : S. **Confiance** : vérifié.

### KYB-17 — Bug : un écran secondaire repassé en DXGI n'en ressort jamais

- **Où** : `0018-txproto-capture-wgc.patch:784-797`. À l'arrêt de WGC (agent arrêté, battement absent pendant 3 s, aucune image après 2,5 s), le flux passe en duplication DXGI jusqu'à la fin de la session.
- **Scénario** : l'agent redémarre (plantage, relance par Admin). L'écran SudoVDA reste noir ou décalé : c'est précisément le défaut de DXGI qui justifiait WGC (mémoire « Multi-écrans : pièges SudoVDA »).
- **Correctif** : depuis la boucle DXGI, retenter `kn_wgc_open` toutes les 2 à 3 s tant que le marqueur de session liste l'écran.
- **Effort** : M. **Confiance** : vérifié dans le code ; impact plausible.

### KYB-18 — Bug : travail interdit dans le fil temps réel de Core Audio (Station Mac)

- **Où** : `server/macos/kyber-patches/0006-txproto-son-kybernow.patch:120-155`, puis `0013-txproto-son-par-10-ms.patch:37`.
- **Description** : à chaque bloc de 10 ms, l'IOProc appelle `av_frame_alloc`, `av_frame_get_buffer`, `av_buffer_allocz`, `sp_log` (mutex) et `sp_frame_fifo_push` (mutex).
- **Scénario** : contention mémoire ou de verrou, le cycle déborde, le son craque.
- **Correctif** : une file SPSC sans verrou dans l'IOProc (le tampon `kn_acc` existe déjà), l'émission des trames dans un fil à part, et le pool `capture->audio_pool`.
- **Effort** : M. **Confiance** : plausible.

### KYB-19 — Bug : nombre de voies figé sur la Station Mac

- **Où** : `Mac 0006:223`. Le nombre de voies est lu à la création de l'entrée, alors que la capture relit le périphérique au démarrage (`:207`).
- **Autres défauts du même correctif**
  - Les replis vers ScreenCaptureKit (`:184`, `:191`) capturent en stéréo malgré 6 voies annoncées.
  - Le passage à 48 kHz (`AudioObjectSetPropertyData`, `:171`) n'est pas vérifié.
- **Scénario** : Admin bascule sur la sortie KyberNow 5.1 après le démarrage de kyavserver. L'entrée annonce 2 voies, la capture en envoie 6.
- **Correctif** : mettre `entry->channels` à jour à chaque `update_entries`, et annoncer 2 voies en repli.
- **Effort** : S. **Confiance** : plausible.

### KYB-20 — Bug : bouton de souris bloqué en session Wayland (Station Linux)

- **Où** : `server/linux/kyber-patches/0002-kynput-pointeur-wayland.patch:168-170` (clics vers le pointeur absolu quand il est actif) et `:179` (`inject_move` le coupe au premier mouvement relatif). `FlushKeys` ne relâche que vers le périphérique actif.
- **Scénario** : glisser-déposer en mode bureau, puis capture du pointeur (immersif ou jeu) avant le relâchement. Le relâchement part vers l'autre périphérique uinput et le compositeur garde le bouton enfoncé.
- **Correctif** : retenir le périphérique qui a enfoncé chaque bouton et y renvoyer le relâchement, ou relâcher sur les deux.
- **Effort** : S. **Confiance** : vérifié.

### KYB-21 — Bug : presse-papiers accordé en cours de partage sans effet

- **Où** : `0019-kynput-partage.patch:86-99`. Le gestionnaire de presse-papiers n'est créé qu'au démarrage, et seulement si le droit existe déjà. `SetRights` (`:136-145`) ne change que le filtre.
- **Scénario** : un invité commence spectateur, puis reçoit le presse-papiers. Rien ne passe jusqu'à sa reconnexion, sans message. Le retrait du droit, lui, fonctionne.
- **Correctif** : créer le gestionnaire selon la configuration seule et laisser le hook filtrer, ou redémarrer le pipeline sur ce changement de droit.
- **Effort** : M. **Confiance** : vérifié.

### KYB-22 — Bug : stylet et touches non relâchés à la déconnexion du client

- **Où** : `0027-kynput-stylet.patch:460` (`ClientDisconnected => None`), avec la répétition toutes les 50 ms de `host_pen.rs` (`:143-153`, `:324-330`).
- **Scénario** : le client tombe pendant que le service continue (reconnexion par le relais). Le stylet Windows réinjecte son dernier état « en contact » et le trait continue.
- **Correctif** : dans `pipeline_state`, répondre à `ClientDisconnected` par `on_flush_keys()`. Cela relâche aussi touches, boutons et manettes.
- **Effort** : S. **Confiance** : plausible (dépend de la durée de vie du service après la déconnexion).

### KYB-23 — Bug : la liste `injected` peut laisser passer un paquet sans droit

- **Où** : `0019-kynput-partage.patch:234-242` et `:278`. Les paquets injectés sont reconnus par égalité de contenu. Un paquet injecté puis jeté avant le hook (`filterable && !process`) reste dans la liste.
- **Scénario** : plus tard, un paquet identique du client passe une fois malgré les droits, par exemple un `Plug` qui rebranche une manette fantôme chez un spectateur.
- **Correctif** : vider `injected` après la boucle de `set_rights`, ou marquer l'injection par un drapeau.
- **Effort** : S. **Confiance** : plausible.

### KYB-24 — Bug : le décodeur du client web peut redémarrer sans fin

- **Où** : `0020-kyctlweb-image-cle.patch:170-182`, dans `reset()` (issu de `0014-kywebplayer-decodeur.patch:103`). Chaque `reset()` remet `recovering_since = Some(now)` : le délai de 3 s avant `DecoderError` ne court plus quand l'image clé arrive puis échoue.
- **Scénario** : le navigateur accepte la configuration mais échoue à chaque décodage (profil HEVC ou 4:4:4 mal annoncé). La boucle « image clé, erreur, reset, nouvelle demande » tourne sans fin : écran noir, et une IDR forcée chaque seconde sur la Station.
- **Correctif** : `recovering_since.get_or_insert(now)`, remis à zéro seulement à la première image décodée, ou `DecoderError` après 3 réinitialisations sans image.
- **Effort** : S. **Confiance** : vérifié à la lecture ; scénario plausible.

### KYB-25 — Bug : `PacketSink` est `Copy` et n'a pas de `Drop` (fuite côté apps)

- **Où** : `apps/macos/kyber-patches/0001-kyclient-paquets-audio-video.patch:468` (`#[derive(Clone, Copy, Debug)]`). La boîte de `apps/core/src/media.rs` n'est libérée que par le paquet `Closed` de la tâche de transfert.
- **Scénario** : la mémoire n'est jamais rendue quand la tâche ne démarre pas :
  - `start_session` refusé (409 `ShareNoImage`, 403) ;
  - pas de point audio (`:253`) ;
  - moins de points vidéo que de configurations (`zip`, `:229`) ;
  - erreur en cours de boucle.

  Un invité qui réessaie fait fuir à chaque tentative les gestionnaires vidéo et son.
- **Correctif** : `PacketSink` non `Copy`, avec un `Drop` qui envoie `Closed` tant qu'il n'a pas été remis à une tâche (`Option::take`).
- **Effort** : M. **Confiance** : vérifié (type et absence de `Drop`) ; gravité plausible.

### KYB-26 — Bug : arrêt de l'aide manettes du Mac par SIGKILL

- **Où** : `server/macos/kyber-patches/0011-kynput-manettes-mac.patch:369-386`. `Drop for Helper` fait `flush()` puis attend la fin de l'aide, mais stdin n'est fermé qu'après la fin de `drop()`.
- **Scénario** : à chaque arrêt, l'aide ne voit jamais la fin de fichier. Elle est tuée au bout de 1 s sans retirer ses manettes. Le fil est aussi bloqué 1 s à chaque échec d'envoi (`:499-506`).
- **Correctif** : `stdin: Option<ChildStdin>`, et `drop(self.stdin.take())` avant d'attendre.
- **Effort** : S. **Confiance** : vérifié.

### KYB-27 — Bug : le presse-papiers du Mac est vidé dès l'annonce d'un changement

- **Où** : `server/macos/kyber-patches/0004-kynput-presse-papiers-mac.patch:266-271`. Sur un `Change`, le Mac efface son presse-papiers avant de recevoir les données. Les `formats` ne sont pas dédoublonnés avant les `DataRequest` (`:268`).
- **Scénario** : l'appareil répond `DataUnavailable` (refus de collage sur iOS), et le Mac perd son contenu. Un `Change` avec des formats répétés provoque autant de requêtes.
- **Correctif** : n'effacer qu'au premier `Data` reçu (comme la version Wayland, paresseuse), et dédoublonner.
- **Effort** : S. **Confiance** : vérifié (le scénario iOS est plausible).

### KYB-28 — Sécurité : nombre de manettes virtuelles illimité

- **Où**
  - `0019-kynput-partage.patch:166` et `:217-231` : `MAX_PLUGS` ne borne que les manettes retenues, pas les branchements.
  - Création des périphériques : `server/linux/kyber-patches/0005-kynput-manettes-linux.patch:212-236` et `:662-670` (uhid), `Mac 0011:510-537` (CoreHID).
- **Scénario** : un client qui a les manettes envoie des milliers de `Plug` avec des identifiants différents. Les descripteurs de fichiers et les nœuds `/dev/input` s'épuisent.
- **Correctif** : refuser un `Plug` au-delà de 8 manettes branchées (dans le hook, ou dans les deux gestionnaires).
- **Effort** : S. **Confiance** : vérifié.

### KYB-29 — Bug : PyroWave HDR10 sans métadonnées ; idem en HEVC sur la Station Mac

- **Où**
  - L'en-tête `KNPW` (`0025-kymedia-ffmpeg-pyrowave.patch:382-394`) ne porte que le drapeau couleur et la plage.
  - L'ajout des métadonnées ne vise que `_nvenc` (`0022-txproto-hdr.patch:1403`).
  - Les correctifs Mac 0009 et 0010 ne passent ni `MasteringDisplayColorVolume` ni `ContentLightLevelInfo` à VideoToolbox.
- **Scénario** : flux HDR10 en PyroWave, ou HEVC Main 10 depuis la Station Mac. Le client suppose une luminance par défaut et adapte mal l'image aux écrans peu lumineux.
- **Correctif**
  - `KNPW` version 2, avec MDCV et CLL en fin (les clients ignorent les octets en trop).
  - Sur le Mac, les clés `kVTCompressionPropertyKey_*`.
- **Effort** : M. **Confiance** : vérifié (absence).

### KYB-30 — Bug : poignées NT perdues quand l'import PyroWave échoue (Windows)

- **Où** : `0025-kymedia-ffmpeg-pyrowave.patch:366-371` (texture partagée) et `:532-536` (fence). `pyrowave.h:227/238` : la poignée n'est prise en charge que si l'import réussit.
- **Scénario** : `pw_create_shared` est rappelée à chaque changement de format ou de taille, et chaque échec perd une poignée.
- **Correctif** : `CloseHandle()` sur les chemins d'erreur.
- **Effort** : S. **Confiance** : vérifié.

### KYB-31 — Bug : le repli de PyroWave dépend du moteur

- **Où**
  - `0025-kyavservice-pyrowave.patch:341` : `resolve` impose H.264.
  - `0025-kycontroller-pyrowave.patch:137-146` : kycontroller prend le `fallback_codec` du client.
  - Mac 0017 ne touche que txproto, Linux 0011 que avconv ; kycontroller annonce PyroWave dès Mac 0018 et Linux 0012.
- **Scénario** : Station Mac en avconv, ou Linux en txproto. PyroWave est annoncé, puis kyavservice retombe en H.264 au lieu du HEVC demandé.
- **Correctif** : passer `fallback` à `resolve` et un seul `STATION` (KYB-45).
- **Effort** : S. **Confiance** : plausible.

### KYB-32 — Bugs PyroWave mineurs

- **Client web** (`0025-kyberweb-pyrowave.patch:121-124`) : la prise en charge du repli est vérifiée avec `config.yuv444`. En PyroWave 4:4:4 avec un repli H.264, WebCodecs refuse alors que le décodeur WebGPU suffirait. Plausible.
- **Windows** (`0025-kymedia-ffmpeg-pyrowave.patch:453`, `:602-610`, `:635`) : `s->hdr` est figé à l'ouverture alors que la texture suit le format de chaque image. Rendre `EINVAL` si le format diffère de `hwfc->sw_format`. Plausible.
- **Mac** (`server/macos/kyber-patches/0010-txproto-hdr-mac.patch:196-208`) : le tampon `gained` est perdu sur les retours d'erreur précédant son transfert. Plausible.
- **Effort** : S chacun.

### KYB-33 — Bug : débits non bornés

- **Où**
  - `0016-kycontroller-debit-par-ecran.patch:222`, repris par `set_bitrate` de 0020 : `per_stream as i32` devient négatif au-delà de 2,1 Gbit/s demandés.
  - `0020-kycontroller-image-invite.patch:2179` et `:3896` : sans image d'hôte, l'invité « Comme l'hôte » obtient le débit qu'il demande (`cap = u32::MAX`).
- **Correctif** : une seule fonction `set_stream_bitrate` qui borne partout, plus un plafond de configuration pour un invité sans hôte.
- **Effort** : S. **Confiance** : vérifié.

### KYB-34 — Bug : course dans `post_close`

- **Où** : `0019-kycontroller-partage.patch:764`. Entre `services.input.take()` et `remove_sessions_if`, la session de l'invité existe encore. Le bannissement ne bloque que `login` et `renew`.
- **Scénario** : un `start_session` en vol relance un serveur d'entrées, tué ensuite sans relâcher ses touches. Une touche peut rester enfoncée sur le PC.
- **Correctif** : marquer la session « fermée » sous le verrou des sessions avant de libérer les entrées, et refuser `start_session` à une session marquée ou bannie.
- **Effort** : S. **Confiance** : plausible.

### KYB-35 — Sécurité : rapports de pertes non limités

- **Où** : `0050:1469-1482` (`LossWindow`) et `:961-978`. Chaque rapport valide ajoute un échantillon, et la fenêtre de 5 s est triée toutes les 100 ms.
- **Scénario** : un lecteur bogué ou malveillant envoie des milliers de rapports par seconde, et le CPU de la Station monte.
- **Correctif** : plafonner les échantillons (environ 100), ou ignorer un rapport arrivé moins de 50 ms après le précédent.
- **Effort** : S. **Confiance** : vérifié.

### KYB-36 — Bug : durcissement de la compatibilité du protocole

- **Version du canal** (`0052:70-75`, `0051:208-214`) : la Station ignore `asked.v` et l'app ne vérifie pas `fb.v`. Une future v2 serait acceptée en silence. Répondre `min(v, FEEDBACK_VERSION)` et refuser `v < 1`.
- **Commentaire faux** (`0050:1697-1702`, répété dans `docs/kyber-changes.md`) : « tout récepteur lit les deux formes ». Un récepteur sans 0050 lirait le numéro de datagramme comme 2^30 et tenterait `segments.resize(2^30+1)`, ce qui le ferait planter.
  - Aujourd'hui c'est protégé par la négociation : 0052 n'active la redondance que sur demande d'un client qui a 0050 et 0051.
  - À faire : corriger la phrase, et un garde-fou côté Station pour que `audio_redundancy` ne soit jamais vrai sans `feedback`.
- **Liste `uplink` non dédoublonnée** (`0085-kycontroller-flux-montant.patch:238`) : `["camera", "camera", …]` crée N points et N tâches `serve`. Passer par un `HashSet` et ignorer les types inconnus.
- **Effort** : S. **Confiance** : vérifié.

### KYB-37 — Bugs mineurs (regroupés)

**Journaux et capture (txproto)**
- `0002-txproto-curseur-stable.patch:102` et `0006-txproto-pointeur-deplacements.patch:232` journalisent `err` au lieu de `wait_err`. C'est corrigé dans `upstream/txproto/0003`, pas en interne.
- `0018:593` : une forme de pointeur illisible est relue (GetIconInfo, GetDIBits) à chaque réveil. Retenir le `HCURSOR` en échec.
- `0018:730` : `av_buffer_allocz` est utilisé sans test de NULL (même défaut dans l'origine).

**Reprise (app Mac)**
- `apps/macos/kyber-patches/0060-kyclient-reprise.patch:74` : `/recover` et `RequestKeyframe` partagent le même drapeau « en cours », et une demande d'image clé est jetée sans trace. Deux drapeaux, ou rejouer la dernière demande.

**Image invité**
- `0020:3070` : la qualité retenue s'impose pendant 10 minutes même si le jeton porte un `kn_quality` plus récent. Ajouter une révision (`kn_qrev`), comme pour les rôles.

**Enregistrement**
- Pas de boîte `colr nclx` pour le HEVC HDR (`0070:524-552`) ; certains lecteurs lisent alors un HDR10 comme du SDR. Plausible.
- Pré-saut Opus sans `elst` (`0070:584-604`).
- Parseur SPS : `ue() + 8` et `width_mbs * 16` sans `checked_*` (`0070:314`, `:434`).

**kynput**
- Collage web envoyé caractère par caractère sans limite (`0013-kynput-client-web.patch:323-333`).
- Position du stylet Windows non bornée à l'écran (`0027:358-360`).
- `switch_desktop` rappelé toutes les 50 ms pendant un UAC (`0027:145-153`).

**Flux montant (kyclient)**
- La file des STATUS est sans borne (`0085-kyclient-flux-montant.patch:399`).
- Les erreurs serde sont fondues en `InvalidState` sans journal (`:43-44`).

### KYB-38 — Sécurité : panique de l'origine de kynput sur un paquet court

- **Où** : hors correctifs, dans l'origine : `kysdk/kynput/kynput/src/types/clipboard.rs:103-104` et `control_event.rs:57` (`get_u32`, `copy_to_bytes(len)`).
- **Description** : la lecture a lieu avant le hook de droits de 0019. Même un spectateur de Share peut faire paniquer la tâche réseau des entrées. Les correctifs, eux, sont propres : `Pen::deserialize` vérifie sa taille (`apps/macos 0005:432-435`).
- **Correctif** : des contrôles `remaining()`, dans un petit correctif kynput, à ajouter aussi à `upstream/kynput/`.
- **Effort** : S. **Confiance** : vérifié.

### KYB-39 — Optimisation : une copie par datagramme pour numéroter

- **Où** : `0050:2063-2068`. Avec la numérotation active, chaque datagramme d'image et de son est recopié dans un `BytesMut` pour ajouter 4 octets.
- **Coût** : à 100 Mbit/s, environ 10 000 allocations et copies par seconde en plus.
- **Côté lecteur** (`0050:97-109`) : une rafale de pertes jusqu'à 16 384 insère autant d'entrées `BTreeMap` sous verrou, puis chaque datagramme tardif fait une recherche linéaire dans `recent_lost` (4 096 entrées).
- **Correctif**
  - Réserver `SEQ_SIZE` à la construction du datagramme, ou `try_into_mut` puis écrire le numéro sur place.
  - Côté lecteur, baisser `MAX_GAP` et utiliser un anneau indexé.
- **Effort** : M. **Confiance** : vérifié.

### KYB-40 — Optimisation : PyroWave, chemin chaud

- **Boucle `memmove` inutile** (`0025-kymedia:682-686`, `Linux 0010:746-750`, `kn_pyrowave_metal.mm:411-416`) : `packetize` écrit les paquets bout à bout (`packet_offset = output_offset`), donc `offset == total` à chaque fois. Un seul `packetize` avec une frontière `SIZE_MAX` évite aussi `compute_num_packets` et sa lecture des métadonnées. Effort S, vérifié.
- **Windows** (`0025-kymedia:613-620`) : une copie GPU de l'image entière à chaque image. Importer une fois chaque texture du pool de capture (cache comme `pw_import` sous Linux). Effort M.
- **Mac**
  - Attente CPU après la conversion et après le gain HDR (`0010-txproto-hdr-mac.patch:176`) : `MTLSharedEvent` à la place.
  - Une `MTLTexture` créée par image : la mettre en cache par IOSurface.
- **Linux** (`0010:632-668`) : `pw_convert` fait deux divisions par point. Tables d'index par taille, ou swscale.
- **Client web** (`0025-kyctlweb-pyrowave.patch:128`, `:176`) : un décodeur WebCodecs créé pour rien, et `push` relu à chaque image.

### KYB-41 — Optimisation : SEI et données annexes

- `0060-txproto-reprise.patch:397-402` : le SEI est inséré par `av_grow_packet` + `memmove` de toute l'image, à chaque image.
- `0022-txproto-hdr.patch:578-579` et `:1452-1456` : deux allocations de données annexes par image HDR, et l'image clé est réallouée et décalée.
- **Correctif** : `sei_data` de NVENC ; tampons HDR créés une fois et attachés par `av_buffer_ref`.
- **Effort** : M. **Confiance** : vérifié.

### KYB-42 — Optimisation : verrous et images clés de l'image invité

- `0020:3161` et `0016:158` : `refresh_rate_at` (EnumDisplayDevices, EnumDisplaySettings) est appelé sous le verrou `displays`, dans `relocate` (qui tient services et manager), et pour chaque flux de `GET /kybernow/sessions` qu'Admin interroge toutes les 2 s. Garder la fréquence en cache à l'annonce `DisplayList`.
- `0020:3824` : `keyframe_for_guest` force l'image clé tout de suite sur une image `Guest`, même partagée par plusieurs invités (`Join`). Un invité qui perd des paquets impose une IDR toutes les 150 ms aux autres. Appliquer `KeyframeSpacing` quand `client_count > 1`.
- **Effort** : S. **Confiance** : vérifié.

### KYB-43 — Optimisation : capture Wayland sans dommages (Sway, Hyprland)

- **Où** : `server/linux/kyber-patches/0013-kyavservice-ecrans-wayland.patch:210`, qui remplace `copy_with_damage` par `copy`.
- **Effet** : un bureau immobile est capturé et encodé (x264, sur le CPU) à pleine fréquence.
- **Correctif** : garder `copy_with_damage` et renvoyer la dernière image une fois par seconde, comme la boucle WGC (`0018:672-675`).
- **Effort** : M. **Confiance** : plausible.

### KYB-44 — Factorisation : en-têtes C copiés à l'identique

- **`kn_timing.h`** : trois copies identiques de 467 lignes, vérifiées par extraction et `diff` :
  - `0023-txproto-latence.patch` (txproto/src) ;
  - `0023-libavgrab-latence.patch:61` (libavconv/libavconv) ;
  - `0023-libavgrab-latence.patch:718` (libavconv/libavgrab).

  0024 modifie les trois à l'identique (`0024-txproto-priorite-gpu.patch:45`, `0024-libavgrab-priorite-gpu.patch:4` et `:166`).
- **`kn_hdr_capture.h`** (844 lignes) et **`kn_hdr_meta.h`** (240 lignes) : identiques octet pour octet entre `0022-txproto-hdr.patch` et `0022-libavgrab-hdr.patch`.
- **Risque** : une correction faite dans une copie est oubliée dans l'autre. C'est déjà arrivé à l'horloge WASAPI (KYB-06) et à PyroWave (KYB-01).
- **Correctif**
  - Une seule source (`server/kyber-patches/shared/*.h`), copiée par `apply.sh` dans chaque arbre après la remise à l'origine.
  - Au minimum, une seule copie pour libavconv : `libavconv/` et `libavgrab/` sont dans le même sous-projet meson.
- **Effort** : M. **Confiance** : vérifié.

### KYB-45 — Factorisation : PyroWave en triple

- **C** : `pw_result_name`, `pw_frame_budget`, la mise en paquets, la table d'options, l'en-tête `KNPW` et les contrôles d'ouverture sont recopiés :
  - Windows : `0025-kymedia:248-262`, `:568-582`, `:658-697`, `:713-721`, `:382-394` ;
  - Linux : `0010:216-230`, `:560-574`, `:722-760`, `:777-786`, `:445-457` ;
  - Mac : `0016:254-268`, `:327-336`, pour une partie.
- **Rust** : Mac 0017 et Linux 0011 réécrivent chacun `ENCODER` et la condition « Windows », de deux façons différentes. Mac 0018 et Linux 0012 n'existent que pour élargir une condition de 0025-kycontroller.
- **Correctif**
  - 0025 ajoute `libavcodec/pyrowave_common.h`. Les correctifs Mac et Linux ne gardent que D3D11, Metal ou Vulkan (environ 120 lignes de moins chacun), dans l'ordre du wrap (`docs/build-notes.md:133`).
  - Côté Rust : `STATION = cfg!(any(windows, target_os = "macos", target_os = "linux"))` et `ENCODER` par plateforme dans 0025. Mac 0018 et Linux 0012 disparaissent ; 0017 et 0011 ne gardent que `nv12_capture` et `render_node`.
- **Pourquoi** : KYB-01 vient de cette triple copie, corrigée une fois sur trois.
- **Effort** : M. **Confiance** : vérifié.

### KYB-46 — Factorisation : mêmes hunks dans deux correctifs

- **PyroWave côté client** : `0025-kyclient-pyrowave` et `0025-kyctlweb-pyrowave` ajoutent les mêmes lignes à `kyctl-common/src/enums.rs` (`PyroWave`), `kyctl-common/src/kymux.rs` (`fallback_codec`), `kyclient/src/lib.rs` et `kyclient/src/kymux_backend/host_service.rs` (vérifié par extraction).
- **Demande d'image clé** (`Message::RequestKeyframe`, `keyframe_in_flight`) : copiée dans `0020-kyctlweb-image-cle.patch:77` et `apps/macos 0001:369`.
- **Cause** : le moteur web part d'un kyctl d'origine (`build.sh:307-325`), d'où ces copies.
- **Correctif** : sortir les parties communes dans un correctif `*-kyctlcommon-*` (ou `*-kyclient-*` sans dépendance aux apps) posé sur les deux arbres.
- **Effort** : S/M. **Confiance** : vérifié.

### KYB-47 — Factorisation : kynput, txproto, kycontroller

**kynput**
- Code DualSense en double entre Linux (`0005:433-541`) et Mac (`0011:177-320`) : `PadState`, `hat`, `stick8`, rapport `0x01`, vibrations et descripteur de 273 octets. Le correctif Linux le dit lui-même. Un module pur `kynput/src/utils/dualsense.rs`. Effort M.
- `locate()` du Mac en trois exemplaires (`0002:164-180`, `0003:206-218`), que 0015 corrige deux fois à l'identique. Un module `macos/displays.rs`, avec un cache des écrans.
- Aides Objective-C en double (`0003:95-145`, `0004:98-125`).
- `switch_desktop()` (0027) n'est pas réutilisé par `send_inputs`.
- `PenAvailable` réécrit par Mac 0020 et Linux 0006 : un seul `cfg!(any(...))` dans 0027.

**txproto**
- La boucle WGC (0018) garde sa copie de l'envoi et du comptage alors que 0020 a créé `kn_send_frame`.
- L'horloge WASAPI est dupliquée entre 0003 et 0009 (cause de KYB-06) : un `kn_audio_clock.h` partagé.

**kycontroller**
- La boucle « abonnements vidéo → manager → avservice » est écrite trois fois (`set_bitrate` 0016/0020, `force_idr` 0005, `repair` 0060).
- 0019 ajoute un court-circuit invité à `force_idr`, que 0020 retire.

### KYB-48 — Code mort : pose du stylet dans le script Linux

- **Où** : `server/linux/scripts/build-kyber.sh:61-74`. Ce bloc pose `apps/macos/kyber-patches/*-kynput-stylet.patch` « s'il manque ».
- **Description** : `apply.sh:88` repose déjà **toujours** tous les correctifs `apps/macos` (`REAPPLY=("$ROOT"/apps/macos/kyber-patches/*.patch)`). Le test `apply --reverse --check` réussit donc et le bloc ne fait jamais rien. Son commentaire (« Sous Windows et sur Mac, la copie de Kyber l'a déjà par les scripts des apps ») date d'avant cette règle.
- **Correctif** : retirer le bloc.
- **Effort** : S. **Confiance** : vérifié.

### KYB-49 — Code mort : fonctions C de l'app Mac sans appelant

- **Où** : `apps/macos/kyber-patches/0001-kyclient-paquets-audio-video.patch:20` (`kyclient_video_player_config_set_packet_sink`), son équivalent audio, et `:68` (`kyclient_client_request_keyframe`).
- **Description** : aucun appelant hors de la doc. `apps/core` passe par l'API Rust.
- **Risque** : avec KYB-25, une réutilisation depuis le C (configuration recopiée) donnerait deux `Closed`, donc deux libérations de la même boîte.
- **Correctif** : les retirer, et mettre `docs/kyber-changes.md` à jour.
- **Effort** : S. **Confiance** : vérifié par grep sur tout le dépôt hors `third_party`.

### KYB-50 — Doc / dette : ordre de pose et numéros en double

- **Numéros en double** : plusieurs `0019`, `0020`, `0022`, `0025`, `0060`, `0085`. Ce sont des familles, un correctif par dépôt ou par moteur. Dans un même dépôt, ils touchent des fichiers distincts (vérifié pour 0020, 0022, 0025, 0030 et 0060 dans kymedia, pour 0022 et 0025 dans kyctl). L'ordre est donc sans effet, **sauf pour deux correctifs** :
  - `0085-kycontroller-micro-droits` se pose sur `0085-kycontroller-flux-montant` (mêmes fichiers : `kymux_controller/mod.rs`, `uplink.rs`). L'ordre ne tient qu'au tri alphabétique du glob (`f` < `m`). Un renommage, ou un tri `LC_COLLATE` qui ignore la ponctuation, suffirait à le casser.
  - La règle « même numéro = même famille, posés par ordre alphabétique » n'est écrite nulle part.
- **Ordre différent entre la Station et les apps**
  - `apply.sh` pose les `*-kyclient-*` du serveur (0022, 0025, 0051, 0085) **avant** ceux d'`apps/macos` (0001, 0060).
  - `scripts/kyber-client-patches.sh`, avec chaque appelant (`apps/macos/scripts/build-kyclient.sh:100-106` et les autres), pose `apps/macos` **avant** les `*-kyclient-*` du serveur.
  - Les deux ordres s'appliquent aujourd'hui sans conflit de contexte, mais l'état produit n'est garanti identique que tant qu'aucun hunk ne se chevauche. `docs/kyber-changes.md` (« après ceux d'`apps/macos/kyber-patches` ») ne décrit que l'ordre des apps.
- **Repli sans git d'`apps/linux/scripts/build.sh`** (`:90-110`, Flatpak) : il pose les correctifs avec `patch -F0` et saute ceux « déjà posés ». C'est une troisième logique de pose, à garder alignée à la main.
- **Correctif**
  - Renuméroter `0085-kycontroller-micro-droits` en `0086`.
  - Écrire la règle des familles dans `docs/build-notes.md`.
  - Choisir un ordre unique : serveur `*-kyclient-*` puis `apps/macos`, comme `apply.sh`, ou l'inverse. L'appliquer dans `kyber-client-patches.sh` et chez ses appelants, idéalement depuis une seule liste produite par une fonction commune.
- **Effort** : S. **Confiance** : vérifié.

### KYB-51 — Doc : `docs/kyber-changes.md`

Chaque correctif des six dossiers est cité dans `docs/kyber-changes.md` et dans `docs/kyber-changes.en.md` (vérifié par nom de fichier, tables FR et EN alignées). Le correctif SDL de webOS est décrit dans `apps/webos/README` et `docs/build-notes.md`, ce qui est logique puisqu'il ne touche pas Kyber. Reste à corriger ou à ajouter :

- **Redondance du son (0050)** : la phrase « tout récepteur lit les deux formes » est fausse (KYB-36).
- **Série amont** : rien n'indique, dans la table principale, que trois corrections n'existent que dans la série `upstream/` et manquent à la Station (KYB-05 à KYB-07).
- **Type `Pen` (11)** : il pourrait entrer en collision avec un futur type de Kyber. À noter.
- **`--cfg kybernow_webos`** sert aussi au client Linux. Le nom trompe ; `kybernow_client_only` serait plus clair.
- **Horloge audio** : l'hypothèse « `av_gettime_relative()` lit le QPC » (vraie avec MinGW et winpthreads, fausse avec MSVC) mérite une ligne dans `kn_timing.h` ou dans build-notes.
- **Fonctions C de l'app Mac** : la ligne de `apps/macos 0001` décrit des fonctions C mortes (KYB-49).

## Dix priorités

1. **KYB-01** : borner le tampon de sortie de PyroWave sous Windows et Linux. Corruption mémoire, S.
2. **KYB-02** : plafonner `DataPacket::read`. Une Station entière peut tomber à cause d'un seul appareil, S.
3. **KYB-03** : limiter les entrées d'un invité à son écran. Promesse de Share non tenue, M.
4. **KYB-04** : espacer les réparations demandées par un invité. Il dégrade l'image de l'hôte, S.
5. **KYB-05, KYB-06, KYB-07** : poser en interne les trois corrections de la série amont (son 5.1, horodatage, écran retiré). Trois S.
6. **KYB-11 à KYB-15** : les cinq défauts de l'enregistrement, avant de le présenter comme fini. S chacun.
7. **KYB-10 et KYB-21** : droits accordés en cours de session (micro, presse-papiers) suivis en direct.
8. **KYB-08 et KYB-09** : déduplication des pertes et redondance du son du canal de retour.
9. **KYB-16** : cadence réelle de l'invité sur la Station Mac (VBV et budget PyroWave). Une ligne.
10. **KYB-44 et KYB-45** : une seule source pour `kn_timing.h`, `kn_hdr_*.h` et le code commun de PyroWave, avant le prochain correctif qui les touche. Deux des bugs ci-dessus viennent de ces copies.

## Ce qui est sain

**Compatibilité du protocole**
- Tous les champs ajoutés (`feedback`, `uplink`, `fallback_codec`, `rights`, `system_keys`, `scale`, `max_fps`) sont en `serde(default)`, sans `deny_unknown_fields` des deux côtés.
- Une app ancienne ne demande rien de neuf, et une Station ancienne ignore ce qu'elle ne connaît pas.
- `/recover` retombe sur `/force_idr` sur une réponse 404, et `Pen` n'est envoyé qu'après `PenAvailable`.

**Authentification**
- JWT à algorithme fixe, `exp` sans tolérance, `aud` obligatoire. `kn_admin` est refusé à `login` et `renew`.
- Les routes `/kybernow` (enregistrement compris) exigent la boucle locale et ce jeton.
- `kn_rev` est croissant, et un invité ne redevient pas hôte au renouvellement.

**Verrous et tâches**
- Aucun verrou std tenu à travers un `await` dans 0019, 0020, 0050, 0070 et 0085, et l'ordre des verrous est cohérent.
- Les `Task` s'annulent à leur destruction, et les aides micro et caméra sont en `kill_on_drop`.
- L'enregistrement écrit sur un fil à part et n'est jamais bloquant pour l'envoi (`try_send`).

**MP4**
- 64 bits là où il le faut (`tfdt`, `tfra`, `mehd`), sans `stco` dans les fragments.
- `avcC`, `hvcC`, `dOps` conformes. Nom de fichier assaini, dossier fixé par Admin seul.

**Files « dernière image » (0040, 0041)**
- Pas d'interblocage à l'arrêt, la fin de flux est conservée, le son est exclu, et les paramètres ne sont jamais jetés.

**HDR**
- SEI HEVC 137/144 et OBU AV1 MDCV/CLL corrects (ordre G, B, R, unités, échappement).
- Synchronisation D3D11 → Vulkan par fence partagée, et import DMA-BUF propre.

**Entrées**
- Format uhid conforme au noyau, presse-papiers Wayland borné et sans écho.
- Touches système refusées aux invités avec les mêmes règles que kynput.
- FFI CoreFoundation libérée correctement.

**Scripts de pose**
- Partout le même principe depuis le 25/11 : remise à l'origine des seuls fichiers touchés, puis pose de tout dans l'ordre. `set -euo pipefail` dans les trois scripts de Station, ce qui rend fiable le `patch | sed`.
- Empreintes (`.kybernow-correctifs-apps`, `.kybernow-patches`, `kyclient.stamp`) qui évitent les recompilations inutiles.
- Verrou de construction (`scripts/verrou.sh`).

**Série `upstream/`**
- Propre et cohérente avec son `README` : un sujet par correctif, comportements nouveaux désactivés par défaut. `0012-kynput-compilation-croisee` y est repris tel quel, ce qui est voulu.
