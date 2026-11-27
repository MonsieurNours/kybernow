# Corrections de l'audit du 27/11 : correctifs de Kyber

Zone : `server/kyber-patches` (correctifs, `apply.sh`, `build.sh`), `server/macos/kyber-patches`, `server/linux/kyber-patches`, `apps/macos/kyber-patches`, `apps/webos/kyber-patches`, `docs/kyber-changes.md`. Rapport d'origine : [../correctifs-kyber.md](../correctifs-kyber.md). Branche `claude/fix-correctifs-kyber-04ea9e`.

**Méthode.** Chaque correctif modifié est régénéré depuis des sources d'origine patchées, dans un arbre à part (`third_party/arbres-correctifs`) :

1. la série est posée correctif par correctif, en commits git ;
2. le commit du correctif est modifié, puis les suivants sont rejoués par-dessus ;
3. le `.patch` est réécrit depuis son commit.

Il y a une branche par variante : la série commune, la Station Mac, la Station Linux, et le moteur web (kyctl d'origine et ses correctifs). Rien n'est écrit à la main dans un diff.

## Constats corrigés

### Priorités et sécurités

| Id | Correctif | Ce qui change |
|---|---|---|
| KYB-01 | 0025-kymedia-ffmpeg-pyrowave, Linux 0010 | PyroWave met l'image en paquets dans un tampon à la taille du flux encodé (`pyrowave_encoder_get_mapped_raw_bitstream`), contrôle chaque paquet, puis copie l'image dans le paquet. |
| KYB-02 | nouveau 0085-kynet-plafond-donnees | `DataPacket::read` refuse une taille de plus de 16 Mio avant d'allouer. |
| KYB-03 | 0019-kycontroller-partage, 0019-kynput-partage, 0027-kynput-stylet | Le serveur d'entrées d'un invité ne connaît que son écran. Le crochet refuse une position ou un stylet sur un autre écran, et ne lui envoie pas le pointeur des autres. |
| KYB-04 | 0060-kycontroller-reprise | Réparation d'un invité : une par 150 ms au plus ; sur l'image de l'hôte, espacée comme ses images clés, et comptée comme signe de retard ; `single` vaut pour lui. |
| KYB-05 | nouveau 0004-txproto-son-voies-opus | Version interne de `upstream/txproto/0005`. |
| KYB-06 | 0003-txproto, 0009-libavgrab | Position WASAPI en erreur écartée. |
| KYB-07 | nouveau 0018-txproto-ecran-retire | Version interne de `upstream/txproto/0004`. |
| KYB-08, 09, 35 | 0050-kynet-canal-de-retour | Pertes dédoublonnées par fenêtre de 64 identifiants. Copie du son seulement après un envoi en un datagramme ; segment incohérent écarté au lieu de l'`assert` (64 segments au plus). 100 échantillons et 32 nouvelles pertes par période au plus. |
| KYB-10 | 0085-kycontroller-micro-droits | Point micro d'un invité toujours là. Droit suivi par un `watch` : `failed`/`not_allowed` au retrait ou sans droit, `stopped` au retour. Format déjà lu par les apps 0.11.7. |
| KYB-11 à 15 | 0070-kycontroller-enregistrement | Perte portée par l'image suivante dans la file ; son figé au changement de voies ; un seul enregistreur ; son seul écrit chaque seconde ; arrêt sous 1 Go libre (refus `disk_full` au départ). |
| KYB-16 | Mac 0008 | Fréquence annoncée à l'encodeur plafonnée à celle de l'invité. |
| KYB-28 | 0019-kynput-partage | 8 manettes branchées au plus, dans le crochet commun aux trois Stations. |
| KYB-38 | nouveau 0028-kynput-paquets-courts | Presse-papiers et contrôle trop courts refusés, sans panique. |
| STLIN-06 | Linux 0013 | Sortie Wayland introuvable : capture en échec, jamais un autre écran. |
| TRANS-09 | `build.sh` | Surcouche assemblée dans `kyber-overlay.new`, puis mise à la place d'un coup. Vérifié par deux compilations complètes. |

### Autres bugs

| Id | Correctif | Ce qui change |
|---|---|---|
| KYB-18, STMAC-19 | Mac 0013 | Anneau sans verrou entre le fil temps réel de Core Audio et un fil qui fabrique les trames : plus d'allocation, de verrou ni de journal dans le rappel. Pertes comptées, journal une fois par changement. |
| KYB-19, STMAC-17 | Mac 0006 | Nombre de voies remis à jour à chaque relecture et au démarrage (stéréo pour le repli ScreenCaptureKit) ; refus du 48 kHz écrit au journal. |
| KYB-20, STLIN-12 | Linux 0002 | Relâchement vers les deux pointeurs. Échec d'uinput réessayé toutes les 5 s au plus. |
| STLIN-13 | Linux 0005 | Refus d'uhid réessayé au branchement suivant passé 30 s ; `libc::O_NONBLOCK`. |
| KYB-22 | 0027 | `ClientDisconnected` relâche tout. |
| KYB-23 | 0019-kynput-partage | Un paquet injecté n'attend le crochet qu'une seconde. |
| KYB-24 | 0020-kyctlweb-image-cle | Le délai de 3 s court depuis la première réinitialisation, jusqu'à une image passée après l'image clé : fin de la boucle « image clé, erreur, reset ». |
| KYB-26, STMAC-16 | Mac 0011 | Tube de l'aide fermé avant l'attente ; aide morte relancée au rapport suivant avec toutes les manettes. |
| KYB-27 | Mac 0004 | Presse-papiers vidé au premier `Data`, formats demandés une fois. |
| KYB-30 | 0025-kymedia | `CloseHandle` sur la texture et la fence quand Vulkan refuse l'import. |
| KYB-32 (Windows, Mac) | 0025-kymedia, Mac 0010 | Image d'un autre format que celui de l'ouverture refusée (`EINVAL`) ; image corrigée rendue sur erreur. |
| KYB-33 | 0020-kycontroller-image-invite | Débit borné à `i32::MAX` ; invité « Comme l'hôte » sans image d'hôte plafonné à 150 Mbit/s. |
| KYB-34 | 0019-kycontroller-partage | `start_session` refusé à un utilisateur fermé ; s'il était en vol, son serveur d'entrées relâche tout avant l'arrêt. |
| KYB-36 | 0050, 0051, 0052, 0085-kycontroller-micro-droits | Version du canal : demande de version 0 refusée par la Station, autre version refusée par l'app. `uplink` dédoublonné. Commentaire de la redondance corrigé. Le garde-fou « pas de redondance sans canal » existait déjà. |
| KYB-37 | 0002, 0018, 0013-kynput, 0027, 0070, 0085-kyclient, apps 0060 | `wait_err` au journal ; forme de pointeur illisible pas relue ; `av_buffer_allocz` testé (WGC) ; collage web limité à 4 096 caractères ; stylet borné à son écran ; changement de bureau une fois par seconde pendant un UAC ; parseur SPS sans débordement ; statuts du flux montant bornés à 64, erreur de réponse écrite au journal ; deux drapeaux « en cours » pour `/recover` et l'image clé. |
| KYB-40 (web) | 0025-kyctlweb | `push` du décodeur de la page lu une fois. |
| KYB-42 | 0016, 0020-kycontroller | Fréquence des écrans en cache une seconde ; images clés espacées aussi sur une image d'invité lue par plusieurs invités. |
| KYB-50 | `apply.sh` | Ordre de pose octet par octet (`LC_ALL=C`), règle des familles écrite dans `docs/kyber-changes.md`. |
| KYB-51 | `docs/kyber-changes.md` | Redondance du son, série amont (maintenant posée), type `Pen`, `kybernow_webos`, hypothèse du QPC. |
| STCOM-05 | `apply.sh`, `build.sh` | Toute la série (commune, apps, Mac, Linux, moteur web) se pose sans « fuzz ». Un fuzz est un **échec par défaut** ; `KYBERNOW_PATCH_STRICT=0` le ramène à un avertissement. Mac 0009, 0030 et 0060-kyavservice régénérés. |
| STMAC-25 | Mac 0020 | Commentaire « 0016 » devenu « 0020 ». |
| Caméra virtuelle (bug de l'auteur, demandé par la coordination) | 0085-kycontroller-flux-montant | La caméra et le micro ne sont proposés que si `kybernow-camera --probe` ou `kybernow-micro --check` rend 0. La vérification se fait au démarrage de la session, en 1,5 s au plus, et le résultat est gardé 10 s. Sans réponse à temps, ou si l'outil ne démarre pas, ils sont proposés comme avant. Une app 0.11.7 voit alors une Station sans caméra, et rien d'autre ne change pour elle. |

## Constats laissés, et pourquoi

- **KYB-17** (WGC jamais repris après un repli DXGI) : M, à faire avec un PC pour l'essayer (agent relancé en session).
- **KYB-21** (presse-papiers donné en cours de partage) : aucun rôle ne donne le presse-papiers à un invité (`role_rights`, « jamais le presse-papiers »). Créer le gestionnaire pour tous ferait annoncer le presse-papiers aux apps des invités. Sans objet tant qu'aucun rôle ne le donne.
- **KYB-25** (`PacketSink` `Copy` sans `Drop`) : change l'API C et Rust qu'emploient `apps/core` et l'app Mac. À faire avec ces zones.
- **KYB-29** (métadonnées HDR de PyroWave et de VideoToolbox) : nouvel en-tête `KNPW` v2 et clés VideoToolbox, côté Station et côté décodeurs des apps. M, et plusieurs zones.
- **KYB-31** (repli de PyroWave selon le moteur) : il faut un champ `fallback_codec` de plus dans `KymuxStartVideo`, porté par 0025-kyavservice, 0025-kycontroller, Mac 0017 et Linux 0011.
- **KYB-32** (client web) : accepter un repli sans 4:4:4 en PyroWave 4:4:4 suppose de savoir ce que la Station envoie en repli. Pas vérifié : laissé.
- **KYB-37** : laissés en partie.
  - Boîte `colr nclx` du HEVC HDR : demande de lire la VUI.
  - `elst` du pré-saut Opus : compatibilité des lecteurs avec un MP4 fragmenté à vérifier.
  - Révision `kn_qrev` : il faut aussi la poser côté Admin.
- **KYB-39, KYB-41, KYB-43** et **KYB-40** (copie GPU Windows, `pw_convert` Linux) : optimisations M. Aucune n'est un défaut. Elles demandent une mesure sur PC, sous Linux, ou avec la Station.
- **KYB-44, KYB-45** : vague 2, comme demandé.
- **KYB-46, KYB-47** : factorisations M ; elles iront avec 44 et 45.
- **Hors zone** :
  - **KYB-48** : `server/linux/scripts/build-kyber.sh`, zone Stations.
  - **KYB-49** : les fonctions C de l'app Mac sont citées dans `apps/macos/README*`, zone de l'app Mac.
  - **KYB-50** (ordre de `scripts/kyber-client-patches.sh`) et **renumérotation** de 0085-kycontroller-micro-droits : ce nom est cité par `docs/micro*.md` et par `server/admin`. L'ordre est maintenant indépendant de la langue du système (`LC_ALL=C`).
- **À aligner à la fusion** : les scripts des Stations (branche fix-stations-mac-linux) font d'un fuzz un échec seulement si `KYBERNOW_PATCH_STRICT` existe. Il faut leur donner la même règle qu'`apply.sh` (strict par défaut, `=0` pour avertir). C'est noté dans le dossier partagé.
- **Mac, hors zone** : `server/macos/pyrowave/kn_pyrowave_metal.mm` borne la mise en paquets par `budget + n × 64 + 4096`. Ce n'est pas une borne sûre : il faudrait la même méthode que sous Windows et Linux.

## Vérifié, et comment

- **Pose sur les sources d'origine**, avec les vrais scripts, en mode strict. Résultat identique, fichier par fichier, aux branches de référence, et aucun fuzz (essai `patch -F0` dans l'ordre réel) :
  - Station Mac : `build-kyber.sh`, `KYBER_PATCH_ONLY=1` ;
  - Station Linux : même chose ;
  - apps : `scripts/kyber-client-patches.sh` pour la série de l'app Mac (avec PyroWave) et celle de webOS ;
  - moteur web : kyctl d'origine, correctifs `*-kywebplayer-*` et `*-kyctlweb-*`.
- **Essais Rust sur le Mac**, à l'état final :
  - kycontroller : 135 réussis ;
  - kynputservice : 16 ;
  - kymux-types : 3 ;
  - kyproto : 32 (`without_the_channel_the_former_transport_runs` échoue parfois, sans ces changements aussi : il dépend du temps) ;
  - kynput, bibliothèque sur la branche Mac : 73. Le module d'essais d'origine `macos/client_keyboard_mouse.rs` ne compile pas ; il a été écarté le temps de l'essai, sans rien commiter.
  - Essais ajoutés : plafond des paquets de données ; écran d'un invité ; stylet hors écran ; déconnexion qui relâche ; 8 manettes ; pertes dans le désordre ; fenêtre plafonnée ; segments incohérents ; paquets courts ; une place de démarrage ; `df` ; perte puis image clé ; son d'un écran immobile ; SPS abîmé ; statuts du droit micro ; `uplink` dédoublonné ; caméra non proposée si sa sonde répond non, proposée si elle ne répond pas à temps ; collage refusé sur le Mac.
- **Compilations complètes dans mes arbres** :
  - Station Mac (`build-kyber.sh`) : réussie au commit 9b8ec4d6 (anneau du son, voies, aide des manettes, presse-papiers compris).
  - Surcouche Windows avec MinGW (`build.sh` : FFmpeg et l'encodeur PyroWave, txproto, kycontroller, kynput.dll et kynputserver, moteur web) : réussie au commit 9b8ec4d6, mode strict par défaut, aucun fuzz.
  - Une première passe, au commit 4ce8ed9b, avait réussi pour les deux. Le dernier commit (sonde de la caméra et du micro) ne touche que kycontroller, du code commun à tous les systèmes, compilé et essayé sur le Mac.
- **Pas compilé** : le code propre à Linux (Linux 0002, 0005, 0010, 0013) ne se compile pas sur le Mac. Il est relu, et sa pose est vérifiée.
- **Jamais essayé sur un appareil.**

## À essayer sur appareil

- **PC** :
  - PyroWave en profil Max sur un contenu très détaillé ;
  - Windows en « 5.1 Surround » ;
  - un écran SudoVDA retiré en session ;
  - un invité clavier-souris face à deux écrans ;
  - un invité qu'Admin ferme pendant qu'il démarre (aucune touche ne reste enfoncée) ;
  - un enregistrement d'un écran immobile, et un double clic sur « Enregistrer » ;
  - un stylet en session, puis le client coupé (le trait s'arrête) ;
  - le client web sur un navigateur qui refuse le profil (la session s'arrête au bout de 3 s au lieu de boucler).
- **Mac** :
  - un invité limité à 30 i/s ;
  - la sortie KyberNow passée en 5.1 après le démarrage de Kyber (le son passe en 5.1) ;
  - le son sous forte charge (pas de craquement ; « frames of 10 ms lost » au journal sinon) ;
  - l'aide KyberNow Manettes tuée en pleine partie ;
  - un collage refusé par l'iPhone.
- **Linux** :
  - un écran `HEADLESS-n` d'un invité retiré ;
  - un glisser commencé en mode bureau puis le pointeur capturé ;
  - kynputserver démarré avant que `/dev/uinput` soit accessible, puis une DualSense branchée avant la règle udev.

## Notes de version

**Français**
- Sur la Station Windows et la Station Linux, PyroWave ne fait plus planter le service d'image sur une image très détaillée.
- Le son revient quand Windows sort en « 5.1 Surround ».
- Un écran retiré en pleine session ne laisse plus une image figée.
- Un invité de KyberNow Share ne peut plus agir sur un autre écran que le sien, ni forcer des images clés sur l'image de l'hôte.
- Un ami à qui vous donnez la parole en pleine session peut parler tout de suite.
- L'enregistrement est plus sûr : pas d'image abîmée après un ralentissement du disque, le son est gardé même quand l'écran ne bouge pas, et l'enregistrement s'arrête avant que le disque soit plein.
- Sur la Station Mac, le son reste net sous forte charge et suit le passage en 5.1, les manettes reviennent seules si leur aide s'arrête, et un collage refusé par votre appareil ne vide plus le presse-papiers du Mac.
- Sur la Station Linux, le pointeur et les manettes DualSense reviennent seuls après un raté au démarrage.
- Le client web ne tourne plus en boucle quand votre navigateur ne sait pas lire l'image.

**English**
- On the Windows and Linux Stations, PyroWave no longer crashes the picture service on a very detailed image.
- Sound comes back when Windows outputs "5.1 Surround".
- A display removed during a session no longer leaves a frozen picture.
- A KyberNow Share guest can no longer act on any display but their own, nor force keyframes on the host's picture.
- A friend you let speak during a session can talk right away.
- Recording is safer: no damaged picture after the disk slows down, the sound is kept even when the screen is still, and recording stops before the disk is full.
- On the Mac Station, sound stays clean under heavy load and follows a switch to 5.1, gamepads come back by themselves if their helper stops, and a paste refused by your device no longer clears the Mac's clipboard.
- On the Linux Station, the pointer and DualSense gamepads come back by themselves after a hiccup at startup.
- The web client no longer loops when your browser cannot play the picture.
