# PyroWave : le client Linux

[English](pyrowave-client-linux.en.md) · Français

Section client de [pyrowave.md](pyrowave.md) (le contrat et la Station), à y fusionner. Concerne l'app Linux (`apps/linux`) : Linux de bureau, Steam Deck et SteamOS au salon. Le Raspberry Pi n'a pas encore PyroWave : la bibliothèque est compilée et livrée (`--rpi`), mais la sonde le dit indisponible ; faisabilité (Pi 5 probable avec un correctif, Pi 4 et Pi 3 impossibles) dans [pyrowave-client-pi.md](pyrowave-client-pi.md). Règles d'interface : [link-ui.md](link-ui.md), « PyroWave » ; client Mac : [pyrowave-client-mac.md](pyrowave-client-mac.md).

## La bibliothèque

- [PyroWave](https://github.com/Themaister/pyrowave) (licence MIT), chemin **Vulkan**, au commit de la Station et du Mac (a32b05e, `apps/macos/PyroWave/COMMIT`), avec la partie de [Granite](https://github.com/Themaister/Granite) (MIT) que prend `checkout_granite.sh`. `apps/linux/scripts/build.sh` (étape 4) le récupère par git, le compile par CMake (cible `pyrowave-shared`, l'API C `pyrowave.h`) et pose `libpyrowave-shared.so.1`, `pyrowave.h` et les en-têtes de Vulkan de Granite dans le préfixe. Une trentaine de secondes ; le module des dépendances du Flatpak le met en cache avec FFmpeg et libkynput.
- **Bibliothèque partagée, chargée au lancement** (`dlopen`) : à côté de l'app (`<préfixe>/lib`, `/app/lib` dans le Flatpak), sinon le préfixe de compilation, sinon le chemin du système ; `KYBERNOW_PYROWAVE_LIB` en force une. Sans elle, PyroWave est indisponible et rien d'autre ne change. Le chargeur Vulkan est celui du runtime (`libvulkan.so.1`, ouvert par volk dans Granite) ; aucune autorisation de plus dans le Flatpak (`--device=all` y est déjà).
- **Demander PyroWave à Kyber** : `build.sh --kyber-pyrowave` (et `flatpak.sh ARCH --kyber-pyrowave`) pose le correctif kyclient 0025 de la Station (`VideoCodec::PyroWave`, `fallback_codec`) et allume la fonctionnalité `pyrowave` du cœur (celle de l'app, `pyrowave`, la transmet). Les deux vont ensemble ; comme les autres apps, seulement sur demande tant qu'ils ne sont pas allumés partout. Sans eux, l'app décode PyroWave mais ne le demande jamais.
- **SDK de Kyber partagé** : quand le `third_party/` partagé porte déjà les correctifs d'un autre travail (les correctifs de la branche ne s'y appliquent plus), `KYBERNOW_KYBER_HEAD=1` fait prendre à `sources.sh` le SDK tel qu'il est dans ses dépôts git (HEAD de kysdk et de ses sous-modules), en lecture seule ; `build.sh` y pose ensuite les correctifs de la branche.
- `build.sh --rpi` le compile aussi (le .deb et KyberNow Link OS le livrent, [pyrowave-client-pi.md](pyrowave-client-pi.md)) ; la sonde refuse encore le Pi.

## Décodage (`src/native/pyrowave.c`, `src/device/video.rs`)

- Le fil de l'image (un par écran) reconnaît le format `pwv1` (`0x7077_7631`, `FOURCC_PYROWAVE` du cœur) et passe ses paquets à PyroWave au lieu de FFmpeg. Un seul device PyroWave pour tout le processus (sa création compile les pipelines) ; ses appels passent sous un verrou (plusieurs écrans).
- **Paquet de configuration** (16 octets, `KNPW`, reconnu comme paquet de configuration ou par sa signature) : chroma, couleur, plage, définition ; un changement refait le décodeur. S'il manque, la définition et la chroma sont lues dans l'en-tête de séquence du flux (SDR, plage pleine), comme sur Mac.
- **Paquet média** : une image entière. `pyrowave_decoder_clear`, tout le paquet dans `push_packet`, puis décodage si l'image est complète ou assez complète d'après l'amont (`decode_is_ready`, partielle permise) ; sinon elle est sautée. Une image illisible est sautée aussi. Un trou dans le flux ne fait rien : **jamais d'image clé demandée**, l'image suivante se décode seule.
- **Voie simple (celle qui est écrite)** : PyroWave décode sur la carte graphique, puis relit les plans dans la mémoire du processeur (`pyrowave_decoder_decode_cpu_buffer_synchronous`), directement dans le tampon de l'image de l'app :
  - SDR 4:2:0 : **I420** (trois plans 8 bits), montré par SDL comme une image du processeur (`SDL_PIXELFORMAT_IYUV`) ;
  - HDR10 4:2:0 : **I010** (10 bits dans les bits de poids faible, l'échelle posée par l'amont), la même image que le décodage HEVC Main 10 par le processeur : la sous-surface HDR de Wayland la prend telle quelle (PQ, BT.2020), sinon SDL la reçoit ramenée en 8 bits (le repli existant) ;
  - 4:4:4 (SDR) : trois plans 8 bits relus, puis convertis en **BGRA** par la conversion du 4:4:4 de FFmpeg (`kn_yuv444_to_bgra` de `decoder.c`, BT.709, plage du paquet de configuration). Le HDR10 en 4:4:4 n'existe pas (le cœur impose 4:2:0 en HDR).
- **Temps** : `received` à l'arrivée du paquet, `decoded` après la relecture ; le temps de décodage des statistiques (« Décodage » du trajet d'une image) compte donc la relecture. Les images jamais montrées comptent dans « Images perdues ».
- Le chemin par fragments de l'amont (GPU mobiles) n'est pris que si le device le préfère, et jamais en HDR (il ne sait pas l'échelle des sorties 10 bits).
- **Sans copie (pas écrit)** : si la relecture coûte plus d'1 ms en 1440p sur le Steam Deck, l'étape suivante est un partage sans copie : image Vulkan exportée en dmabuf (`pyrowave_image_create` sait importer un dmabuf avec modificateur DRM) et montrée par EGL (`EGL_EXT_image_dma_buf_import`) sous SDL, comme la sous-surface HDR ; ou le rendu de la session en Vulkan. Pas mesurable dans la VM (voir « Mesures »).

## Capacités

- **Sonde au lancement** (`system::pyrowave_probe_start`, sur un fil pendant que la fenêtre s'ouvre) : la bibliothèque, un device Vulkan 1.3 qui convienne, un décodeur 128 × 128, l'image de test du Mac (1 812 octets, rampe de Y de 16 à 235, encodée par l'encodeur Metal du port : le même flux binaire) décodée et relue, quelques points à 4 près. Le journal dit le résultat, la carte et la durée (« PyroWave : prêt sur AMD Custom GPU 0405 (RADV VANGOGH) (Vulkan 1.3) »).
- **Indisponible** : le Raspberry Pi (pas même essayé), `KYBERNOW_PYROWAVE=0`, la bibliothèque absente, pas de Vulkan 1.3 qui convienne (`PYROWAVE_ERROR_NO_VULKAN`), un rendu logiciel seulement (llvmpipe/lavapipe : bien trop lent pour une session ; `KYBERNOW_PYROWAVE_CPU=1` le laisse servir, pour les essais), ou l'image de test fausse. La valeur PyroWave de « Format vidéo » est alors inactive avec la raison de link-ui : « Cet appareil ne sait pas décoder PyroWave. ».
- `Machine::pyrowave` = la sonde **et** `PYROWAVE_KYBER` du cœur (le correctif kyclient de la Station) : tant que la fonctionnalité `pyrowave` du cœur est éteinte, l'app n'annonce pas PyroWave (le journal le dit). `Capabilities::pyrowave` part au cœur : dans le plan des écrans, et dans `SessionParams::capabilities` d'une session à un écran quand la sonde a réussi.
- **Ce que la Station propose** : `info.pyrowave` de `GET /api/stream/info`, gardé dans `Host::pyrowave` (`pyrowave` dans `kybernow.json`).
- **Par câble** (`system::wired_to`) : au début de la session, l'interface que le système prend pour joindre l'ordinateur (l'adresse locale d'une socket UDP « connectée », sans rien envoyer, retrouvée par `getifaddrs`) est filaire (type Ethernet, ni Wi-Fi ni tunnel) et active. Sans route connue, le réseau de l'appareil. Pour le cœur, « Auto » est un profil ; l'app Linux en garde toujours un : `StreamSettings::wired` n'est donc donné que si « Format vidéo » est sur Auto (sinon un HEVC choisi deviendrait PyroWave), et il est gardé aux changements de profil en session.

## Interface (tiny-skia ; Bureau, Deck, Salon)

Textes de link-ui, mot pour mot (`src/pyrowave.rs`, sans SDL ni cœur) :

- **Format vidéo** : « PyroWave » après AV1, seulement si l'ordinateur le propose (ou s'il était déjà choisi) ; inactif avec sa raison (« {nom} ne propose pas PyroWave. », puis « Cet appareil ne sait pas décoder PyroWave. ») au bout de l'aide. Aides : PyroWave choisi, Auto par câble, Auto sans câble. PyroWave choisi en Wi-Fi ou par le relais : l'avertissement suit l'aide, avec une étiquette ambre (« Wi-Fi », « hors de chez vous »). Dans les réglages, « par câble » est le réseau de l'appareil (Ethernet actif) ; la session, elle, regarde la route vers l'ordinateur.
- **Texte net (4:4:4)** : permis avec PyroWave, aide « Avec PyroWave, presque sans coût : seul le débit monte. ».
- **En session**, quand le décodeur dit « PyroWave » et que la session passe par le Wi-Fi ou par le relais (jamais pour un invité) :
  - le **bandeau** du début, une fois, sous la barre : « PyroWave en Wi-Fi » / « PyroWave hors de chez vous », « L'image peut saccader. », « Passer en HEVC » et une croix ; il part seul au bout de 8 s. Au Deck et au salon, le toucher le prend ;
  - la **puce ambre** dans la barre du bureau, après « Par le relais » ; le format y dit « PyroWave » ;
  - à la manette (Deck, salon), une **tuile** du menu de session au nom de la puce, sous-titre « Passer en HEVC ».
  - « **Passer en HEVC** » relance l'image en HEVC pour cette session seulement (`Session::set_settings`, câble retiré pour que « Auto » ne reprenne pas PyroWave) ; le choix de l'ordinateur reste.
- **Statistiques** (session) : « Images perdues : 0,8 % » (les images de l'ordinateur jamais montrées, en part des images de la seconde ; ambre au-dessus de zéro) et « Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. » quand le cœur a ramené la définition pour tenir dans le « Débit maximal » de la Station (`Second::lowered`, texte de `Lowered::text`, gardé dans `Stats::lowered` ; passe à la ligne s'il le faut). Au moment où il la ramène, le cœur le dit aussi (`Notice::ResolutionLowered`) : la capsule de la session, sous le bandeau PyroWave quand celui-ci est là (bureau), en bas au Deck et au salon.
- Captures : `--preview`, `45-pyrowave-*` (bureau, deck, salon : par câble, Wi-Fi, Auto par câble et sans câble, inactif, session, bandeau, statistiques seules).

## Essai hors ligne (`apps/linux/tests/pyrowave/run.sh`)

- `encode.c` : l'encodeur Vulkan de l'amont par son API C (mire `testsrc2` de ffmpeg, 1,2 bit par pixel, paquets de 1 152 octets mis bout à bout comme la Station), puis `check.c` : la sonde, puis chaque image par `pyrowave.c` comme un paquet média ; PSNR de la première image, temps cadencé à 60 i/s et à la chaîne, image tronquée sautée et la suivante décodée. 4:2:0, 4:4:4 (BGRA) et HDR10 (I010).
- Sans encodeur Vulkan qui convienne (la VM : lavapipe a des sous-groupes de 4, l'encodeur de l'amont les veut de 16 à 64), `SAMPLES=DOSSIER` décode des flux faits ailleurs : `tests/pyrowave/samples-mac.sh DOSSIER [LxH] [images]` les fait sur un Mac à puce Apple avec l'encodeur Metal de l'amont (l'outil de l'essai du client Mac).

## Mesures

Machine virtuelle « KyberNow Linux » (Ubuntu 24.04 ARM64 sous Parallels, MacBook Pro M4 Max) : sa carte virtuelle (virgl) n'a pas Vulkan ; seul **lavapipe** (Vulkan 1.4 logiciel, llvmpipe) y tourne. Le décodeur y marche (la sonde, les flux du Mac), mais les temps sont ceux du processeur, pas d'une carte graphique : ils ne disent rien du Steam Deck, ni du coût de la relecture.

25/11/2026, flux encodés par l'encodeur Metal du Mac, 1280 × 720, 30 images, 1,2 bit par pixel (66 Mbit/s à 60 i/s), `KYBERNOW_PYROWAVE_CPU=1` :

| Format | Sortie | PSNR (1re image) | Décodage + relecture, cadencé à 60 i/s | À la chaîne |
|---|---|---|---|---|
| 4:2:0 SDR | I420 | Y 51,9 dB, Cb 58,0, Cr 58,9 | 10,7 ms (max. 11,7) | 97 i/s |
| 4:2:0 HDR10 | I010 | Y 51,9 dB (ramené en 8 bits) | 10,4 ms (max. 11,3) | 99 i/s |
| 4:4:4 SDR | BGRA | — (pixel du milieu juste : bleu BT.709) | 18,0 ms (max. 26,7) | 57 i/s |

Sonde : 33 ms au lancement de l'app (lavapipe refusé), 56 à 79 ms quand il est permis. Image tronquée à 60 % : sautée, la suivante décodée. Le flux binaire du port Metal se décode juste par le chemin Vulkan (la sonde et les PSNR le montrent).

## À essayer sur le Steam Deck

1. Construire et installer le Flatpak x86_64 (`apps/linux/scripts/flatpak.sh x86_64 --kyber-pyrowave` dans la VM pour qu'il demande PyroWave, ou celui d'une version) : `flatpak install --user KyberNow-Link-Linux-x86_64.flatpak`.
2. Lancer l'app une fois en mode Bureau depuis un terminal (Konsole) : `flatpak run fr.leslignesclaires.kybernow 2>&1 | grep -i pyrowave` doit dire « PyroWave : prêt sur AMD Custom GPU … (RADV VANGOGH) » et la durée de la sonde. « décodeur prêt, mais ce cœur ne sait pas encore le demander » : la fonctionnalité `pyrowave` du cœur est encore éteinte (correctif kyclient de la Station pas encore fusionné).
3. L'essai hors ligne sur le Deck (mesure de la relecture) : le Flatpak n'a pas de compilateur, donc dans une distrobox Ubuntu 24.04 (ou avec l'app compilée par `build.sh` sur un Linux x86_64), `apps/linux/tests/pyrowave/run.sh 2560 1440 120` (l'encodeur Vulkan tourne sur RADV) ; noter « décodage, relecture comprise » en 1440p 4:2:0 et 4:4:4. Au-delà d'1 ms de plus que le décodage seul, écrire la voie sans copie (dmabuf).
4. Avec une Station qui propose PyroWave (correctif 0025 et fonctionnalité `pyrowave` du cœur allumée) : Réglages › Image › Format vidéo › PyroWave ; une session par câble (dock et Ethernet), puis en Wi-Fi : le bandeau, la puce (bureau) ou la tuile (mode Jeu), « Passer en HEVC » ; les statistiques (Décodage, Images perdues, Définition ramenée avec un « Débit maximal » bas dans Admin). En mode Jeu (gamescope) et au salon, la même chose à la manette.
5. HDR10 en PyroWave sur un bureau KDE Plasma 6.3+ avec un écran HDR (le Deck OLED en mode Jeu reste en SDR, comme pour HEVC).
