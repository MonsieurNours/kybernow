# PyroWave : le client Mac

[English](pyrowave-client-mac.en.md) · Français

Section client de [pyrowave.md](pyrowave.md) (le contrat et la Station), à y fusionner. Concerne l'app Mac ; KyberKit étant partagé, l'Apple TV et l'iPhone/iPad compilent le même décodeur.

## Le port Metal de PyroWave

- Sources : le dossier `metal/` de [PyroWave](https://github.com/Themaister/pyrowave) (licence MIT), « port assisté par IA », peu maintenu en amont. Repris tel quel dans `apps/macos/PyroWave/upstream`, au commit de `apps/macos/PyroWave/COMMIT` (a32b05e, le même que la Station). `apps/macos/scripts/vendor-pyrowave.sh [commit]` le reprend ; changer de commit, c'est relire le diff de `metal/` et repasser l'essai hors ligne.
- Compilé dans KyberKit (`project.yml` des apps Mac, Apple TV et iPhone/iPad : `pyrowave_common.mm`, `pyrowave_decoder.mm`, `pyrowave_bitstream.cpp`, ARC, C++17) ; Swift le voit par le module `CPyroWave` (`PyroWave/module`). Aucune bibliothèque du `Vendor/` n'est réécrite : l'app peut rester ouverte pendant une compilation. L'encodeur du port n'est pas dans l'app (seulement dans l'essai).
- Les shaders sont du MSL en texte (`shaders/pyrowave_msl.h`), compilés au premier décodeur : c'est la sonde du lancement qui paie ce coût, hors du fil principal.
- GPU : famille Apple7 et suivantes (M1, A14), groupes SIMD de 32. Ni Intel ni AMD, ni l'Apple TV 4K de 2021 (A12).

## Décodage (`KyberKit/PyroWaveDecoder.swift`)

- `VideoDecoder` reconnaît le format `pwv1` (`0x7077_7631`) et passe ses paquets à `PyroWaveDecoder`, au lieu de VideoToolbox.
- Paquet de configuration (16 octets, `KNPW`, reconnu par `is_config` ou par sa signature) : chroma 4:2:0 ou 4:4:4, SDR BT.709 ou HDR10 BT.2020 PQ, plage pleine ou réduite, largeur, hauteur. Un changement recrée le décodeur PyroWave, le pool de tampons et les textures de chroma. S'il manque, la définition et la chroma sont lues dans l'en-tête de séquence du flux (repli en SDR, plage pleine : l'encodeur n'y écrit pas la couleur de façon fiable).
- Paquet média : une image entière (paquets PyroWave bout à bout). `pyrowave_decoder_clear`, puis tout le paquet dans `push_packet`, puis le décodage est encodé dans un tampon de commandes Metal et soumis ; le fil réseau de Kyber rend la main tout de suite. Une image incomplète (moins de 90 % des blocs ou bandes basses manquantes) ou illisible est sautée ; rien n'est demandé à l'ordinateur, l'image suivante se décode seule. Au plus 3 images sur le GPU à la fois : au-delà, l'image est sautée plutôt que mise en file.
- Sortie : un `CVPixelBuffer` IOSurface bi-plan, du même format que ceux de VideoToolbox. SDR : `420v`/`420f` ou `444v`/`444f` (8 bits) ; HDR10 : `x420`/`xf20` ou `x444`/`xf44` (10 bits calés en haut de 16 bits). PyroWave écrit Y directement dans le plan de l'IOSurface (r8Unorm ou r16Unorm) ; Cb et Cr sortent dans deux textures 16 bits, qu'une petite passe de calcul entrelace dans le plan CbCr. Étiquettes : matrice, primaires et transfert BT.709, ou BT.2020 et PQ (SMPTE ST 2084). Le rendu Metal (`VideoRenderer`) n'a rien de propre à PyroWave : conversion YCbCr → RGB selon la matrice et la plage, chaîne HDR pour une image PQ.
- Valeurs : PyroWave décode des valeurs normalisées. En r16Unorm, une valeur v devient le code v · 65 535, soit la convention P010 (10 bits en haut de 16) que le rendu lit ; milieu de chroma attendu à 0,5.
- Temps : `received` à l'arrivée du paquet, `decoded` à la fin du tampon de commandes (GPU terminé) ; l'image part au rendu depuis ce fil, comme une image de VideoToolbox. Le temps de décodage des statistiques (`VideoStats.decodeMs`) et le trajet d'une image en tiennent compte sans changement.
- Perte : `dataLost()` (trou dans le flux) ne fait rien pour PyroWave ; jamais de demande d'image clé.

## Capacités

`DecoderCapabilities.pyrowave` : au lancement (`DecoderCapabilities.prepare()`, hors du fil principal), création du device PyroWave (pipelines compilés), d'un décodeur 128 × 128, décodage d'une image de test intégrée (1 812 octets, rampe de Y de 16 à 235, encodée par l'encodeur Metal du port) et relecture de quelques points, à 4 près. Environ 55 ms sur M4 Max. Le cœur reçoit `pyrowave` dans les capacités du décodeur (voir apps/core/README.md).

## Essai hors ligne (`apps/macos/tests/pyrowave/run.sh`)

Mire `testsrc2` de ffmpeg (120 images, BT.709, plage réduite) encodée par l'encodeur Metal du port (outil `encode`, à 1,2 bit par pixel à 60 i/s, une image = paquets bout à bout comme un paquet média de la Station), puis décodée par `VideoDecoder` (`pwv1`) → `PyroWaveDecoder` (outil `bench`) : temps de décodage cadencé à 60 i/s, débit à la chaîne, PSNR de la première image contre la source, image tronquée sautée sans casser la suivante, sonde du lancement. HDR10 : même source 8 bits étiquetée HDR (sortie 10 bits), pour mesurer ce chemin.

Mesures du 25/11/2026, MacBook Pro M4 Max (GPU 40 cœurs), macOS 27.2.1 ; temps de décodage moyen cadencé à 60 i/s (médiane ; maximum) et images par seconde à la chaîne :

| Définition | Format | Débit | SDR (8 bits) | HDR10 (10 bits) | À la chaîne |
|---|---|---|---|---|---|
| 2560 × 1440 | 4:2:0 | 265 Mbit/s | 1,28 ms (1,23 ; 3,6) | 1,43 ms (1,46 ; 2,0) | 1 000 à 1 700 i/s |
| 2560 × 1440 | 4:4:4 | 265 Mbit/s | 1,74 ms (1,76 ; 3,1) | 1,69 ms (1,64 ; 3,2) | 690 à 780 i/s |
| 3840 × 2160 | 4:2:0 | 596 Mbit/s | 2,04 ms (2,08 ; 5,0) | 1,88 ms (1,87 ; 5,7) | 665 à 710 i/s |
| 3840 × 2160 | 4:4:4 | 597 Mbit/s | 2,88 ms (2,94 ; 5,7) | 3,18 ms (3,15 ; 5,8) | 540 à 550 i/s |

PSNR de la première image : 64 dB en Y (1440p 4:2:0), 56 dB en Y et 49 à 51 dB en chroma en 4:4:4 (même débit pour trois fois plus d'échantillons). Cadencé, le GPU redescend en fréquence entre deux images : une image prend plus longtemps qu'à la chaîne (1,3 ms contre 0,6 à 1 ms). Une seconde passe, la machine occupée par ailleurs, donne de 1,3 à 4 ms en moyenne et jusqu'à 17 ms au pire en 1440p, 1,4 à 2,5 ms en 4K : le GPU partagé compte plus que la définition. Pas encore mesuré sur une puce M1 ou une Apple TV, ni avec un flux de la Station.

## Le cœur

`apps/core` connaît PyroWave (format, capacités, annonce de la Station, règle d'« Auto » par câble) : voir [apps/core/README.md](../apps/core/README.md), « PyroWave ». L'app Mac envoie `pyrowave` dans les capacités du décodeur ; elle ne donne pas encore `wired` (câble Ethernet) dans les réglages, ni l'écran du format : ils viendront avec les maquettes.

## Ce qui attend la Station

- Le correctif kyclient (identifiant de codec, `VideoCodec::PyroWave`) : sans lui, Kyber n'envoie jamais `pwv1` et le cœur ne demande pas PyroWave.
- La façon dont la Station annonce PyroWave à l'app (`GET /api/stream/info`).
