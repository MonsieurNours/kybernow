# PyroWave : le client web

[English](pyrowave-client-web.en.md) · Français

Section client de [pyrowave.md](pyrowave.md) (le contrat et la Station), à y fusionner. Concerne le client web `/link` servi par Admin (`server/web/src/link`). Règles d'interface : [link-ui.md](link-ui.md), « PyroWave ».

Il n'y a ni Vulkan ni Metal dans un navigateur, et WebCodecs ne connaît pas PyroWave : le décodeur est écrit en **WebGPU** (WGSL), dans la page, d'après les shaders de l'amont (`wavelet_dequant.comp`, `idwt.comp`, via le port Metal vendu dans `apps/macos/PyroWave/upstream/shaders`, commit a32b05e) et `pyrowave_bitstream.cpp`.

## Décodeur (`server/web/src/link/pyrowave/`)

- `bitstream.ts` : paquet de configuration `KNPW`, version 1 (16 octets, celle d'une Station 0.11.7) ou 2 (48 octets, métadonnées HDR10 lues et écrites au journal de la page ; une version plus récente se lit par ses 16 premiers octets, comme le cœur), en-tête de séquence du flux (repli : définition et chroma, SDR plage pleine, comme le Mac), disposition des blocs 32 × 32 et lecture d'une image (les blocs reçus mis bout à bout, table bloc → position). Image prête comme `decode_is_ready(true)` de l'amont : complète, ou plus de 90 % des blocs avec les deux bandes les plus basses entières ; sinon sautée.
- `shaders.ts` : déquantification (un groupe de 128 fils par bloc 32 × 32, un fil par sous-bloc 4 × 2 : plans de bits, échelle, zone morte, signes), transformée inverse CDF 9/7 par relèvement sur 5 niveaux (lecture transposée avec miroir par `textureGather`, comme l'amont), puis YCbCr → RGB dans la toile : BT.709 SDR, ou BT.2020 PQ ramené en SDR par la courbe de toutes les apps et de la Station (`rules::hdr::sdr_curve` : genou à 0,75 du blanc SDR de 203 nits, 4 fois le blanc ramené à 1, sur la plus grande composante) (la toile du client web est SDR ; le client web ne demande jamais le HDR), plage pleine ou réduite, chroma 4:2:0 interpolée. Valeurs en float32 partout (`r32float`).
- **Sous-groupes** : la somme préfixe des coefficients non nuls (position des signes) passe par `subgroupInclusiveAdd` quand le navigateur a la fonctionnalité `subgroups`, sinon par la mémoire du groupe. La variante à sous-groupes suppose les fils d'un sous-groupe consécutifs : la sonde le vérifie en décodant l'image de test et prend l'autre si l'image est fausse. Mesuré : aucune différence notable entre les deux (tableau plus bas), la déquantification ne pèse pas.
- `decoder.ts` : device et pipelines (partagés), un décodeur par configuration (textures des 5 niveaux, plans Y, Cb, Cr, groupes de liaison préparés une fois), envoi du flux par `writeBuffer`, une passe de calcul (≈ 45 à 60 dispatchs) puis une passe de rendu. Au plus 3 images sur la carte graphique : au-delà, l'image est sautée.
- `live.ts` : le décodeur des sessions. Le moteur web de Kyber (correctif 0025-kyctlweb) lui passe les paquets `pwv1` par `globalThis.__kybernowPyroWave.create(output)` : `push(données, pts, is_config)` rend vrai si une image sortira. L'image est dessinée dans une toile WebGPU hors écran, prise tout de suite (`transferToImageBitmap`), puis rendue au moteur en `VideoFrame` quand la carte graphique a fini : il la dessine dans la toile de la session comme une image de WebCodecs (même cadence, mêmes mesures de latence). Une image perdue ou incomplète est sautée, jamais d'image clé demandée. Temps de décodage (arrivée → image prête) et images perdues (sautées, ou trou dans les pts) pour les Statistiques.

## Moteur web (correctifs 0025-kyctlweb et 0025-kyberweb)

Voir [kyber-changes.md](kyber-changes.md). Le moteur web part d'un kyctl d'origine (`build.sh`) : 0025-kyctlweb y reprend la variante `PyroWave` et `fallback_codec` de 0025-kyclient, et kywebplayer passe les paquets `pwv1` à la page au lieu de WebCodecs. 0025-kyberweb ajoute le codec `pyrowave` et `fallback_codec` à la configuration JS. La page reconnaît un moteur qui connaît PyroWave au réglage `fallback_codec` (`engineKnowsPyroWave()`).

## Capacités et règles

- **Sonde au chargement** (`main.ts` → `pyrowave/state.svelte.ts`) : WebGPU présent, un adaptateur (`high-performance`), shaders compilés, et le décodage pour de bon de l'image de test du client Mac (128 × 128, rampe de Y de 16 à 235, Cb et Cr à 128, 1 812 octets), relue à 4 près. 0,3 à 1,2 s selon le navigateur, en fond.
- **Offert** dans « Format vidéo » seulement si la Station l'annonce (`pyrowave.available` de `GET /api/stream/info` ; champ absent : valeur cachée), que la sonde a passé et que le moteur de Kyber connaît PyroWave. Sinon valeur inactive avec sa raison : « {nom} ne propose pas PyroWave. » ou « Ce navigateur ne sait pas décoder PyroWave. ». Un choix PyroWave qui n'est plus offert est gardé, « Auto » s'applique.
- **Auto ne prend jamais PyroWave** : un navigateur ne sait pas s'il est relié par câble. Choisi à la main, il est toujours averti : texte « réseau inconnu » (proposé dans link-ui), ou celui du relais.
- **Par le relais** (WebSocket) : permis, avec l'avertissement du relais, sans plafond de débit au relais (règle commune). Ce qui tient vraiment au débit de PyroWave par WebSocket (relais, et Safari ou Firefox sur le réseau local, qui passent toujours par WebSocket) n'est pas mesuré : à essayer.
- **Repli** : la requête porte `fallback_codec` (le format d'Auto, HEVC ou H.264) ; une Station qui ne le propose plus prend ce format. Une image qui ne démarre pas en PyroWave : repli en H.264 de la page (`fallback.ts`), comme pour HEVC.
- **Débit et définition** : la règle du cœur ([pyrowave.md](pyrowave.md), « Définition ramenée »), même calcul (`pyrowave/rules.ts`, testé avec les cas du cœur). Plafond : `pyrowave.max_mbps` de la Station et le débit réglé à la main (curseur jusqu'à 1 000 Mbit/s avec PyroWave). La définition ramenée est celle de l'écran virtuel demandé à `prepare` ; elle est dite au début de la session et dans les Statistiques. 4:4:4 avec les profils Bureau et Max : PyroWave le fait lui-même, quel que soit ce que WebCodecs décode.
- **Pour l'hôte seulement** : un invité du partage suit l'image de l'hôte (PyroWave de l'hôte : image à part dans un format qu'il lit) ; un ami à qui l'ordinateur est prêté garde le format d'Auto.

## Validation

Outils dans `server/web/tests/pyrowave/` :

1. `prepare.sh <dossier>` : mire `testsrc2` (30 images, BT.709, plage réduite) encodée par l'encodeur Metal du port (outil `encode` de `apps/macos/tests/pyrowave`, 1,2 bit par pixel à 60 i/s), en 1440p et 4K, 4:2:0 et 4:4:4 ; première image décodée par le décodeur Metal du port (`refdump`, la référence, en float32).
2. `deno run --allow-read --unstable-webgpu tests/pyrowave/offline.ts <dossier>` : essai sans tête (WebGPU de Deno).
3. `deno run -A tests/pyrowave/serve.ts <dossier>` puis `http://127.0.0.1:8765/bench.html` dans un navigateur : sonde, image 0 contre la référence et la source, image tronquée, cadencé à 60 i/s (temps de la carte graphique par `timestamp-query`, et jusqu'à l'image prête), à la chaîne, avec et sans sous-groupes, chemin complet du client (toile → `VideoFrame`, couleurs comparées à la source), et le pont tel que le moteur l'appelle. Les résultats s'écrivent dans `<dossier>/results.txt`.

**Justesse** (tous navigateurs, identique) : écart à la référence Metal de 0,24 à 0,31 code (8 bits) au plus, PSNR contre la source identique à celui du Mac (64,3 dB en Y en 1440p 4:2:0, 55,8 dB en 4:4:4). Couleurs du chemin complet : 4,4 codes d'écart moyen à la source (361 points, bords de la mire compris). Image tronquée sautée, la suivante se décode ; pont : 28 images acceptées sur 28 sorties, image tronquée sautée, 2 images manquantes sur 30 comptées (6,7 %).

**Mesures** du 25/11/2026, MacBook Pro M4 Max, macOS 27.2 ; médiane cadencée à 60 i/s (image prête : de l'envoi à la fin de la carte graphique ; GPU : temps de la carte graphique), et images par seconde à la chaîne :

| Navigateur | Définition | Format | Image prête | GPU | À la chaîne |
|---|---|---|---|---|---|
| Chrome 158 (sans tête), sous-groupes | 2560 × 1440 | 4:2:0 | 4,5 ms | 3,1 ms | 1 010 i/s |
| | 2560 × 1440 | 4:4:4 | 3,5 ms | 2,2 ms | 658 i/s |
| | 3840 × 2160 | 4:2:0 | 4,3 ms | 2,5 ms | 538 i/s |
| | 3840 × 2160 | 4:4:4 | 6,8 ms | 4,5 ms | 401 i/s |
| Chrome 158, sans sous-groupes | 2560 × 1440 | 4:2:0 | 3,3 ms | 2,0 ms | 887 i/s |
| | 3840 × 2160 | 4:2:0 | 5,5 ms | 3,9 ms | 662 i/s |
| | 3840 × 2160 | 4:4:4 | 6,5 ms | 4,2 ms | 403 i/s |
| Safari 27.2 (pas de sous-groupes) | 2560 × 1440 | 4:2:0 | 4 à 6 ms | 2,3 à 4,6 ms | 675 i/s |
| | 2560 × 1440 | 4:4:4 | 5 ms | 2,5 à 3,0 ms | 490 i/s |
| | 3840 × 2160 | 4:2:0 | 5 à 6 ms | 2,9 à 3,3 ms | 475 i/s |
| | 3840 × 2160 | 4:4:4 | 7 ms | 4,3 à 4,6 ms | 300 i/s |

Lecture du flux sur le processeur : 0,4 à 0,9 ms. Chemin complet (toile → `VideoFrame`, 1440p 4:2:0) : 2,7 à 4,9 ms de médiane dans Chrome, 8 ms dans Safari. Sonde : 0,4 à 1,2 s dans Chrome, 0,9 s dans Safari, 0,3 s dans le navigateur intégré de l'app Claude (Chromium, sous-groupes). À la chaîne, le débit vaut à peu près celui du décodeur Metal (665 à 710 i/s en 4K 4:2:0) ; cadencé, la carte graphique redescend en fréquence entre deux images : une image prend 2 à 3 fois plus que le décodeur Metal (1,3 à 2,9 ms). Safari arrondit `performance.now()` à la milliseconde.

## Où PyroWave sera indisponible

Là où la sonde ne passe pas : sans WebGPU (navigateurs qui ne l'ont pas ou l'ont coupé ; Firefox ne l'a pas partout), sans adaptateur (carte graphique refusée par le navigateur, bureau à distance), ou une image de test fausse. Et face à un moteur de Kyber sans le correctif 0025 (Station plus ancienne), ou une Station qui ne le propose pas (Station Mac, pas de Vulkan 1.3). Essayé ici : Chrome, Safari et le navigateur intégré sur le Mac ; ni Firefox, ni Edge, ni Windows, Linux, Android, iPhone ou iPad.

## À essayer

- Une session PyroWave face à une Station Windows qui le propose, dans Chrome (WebTransport) puis Safari (WebSocket sur le réseau local), en 1440p et 4K, 4:2:0 et 4:4:4 : le débit tient-il en WebSocket ? Le bandeau, la puce par le relais, « Passer en HEVC », la définition ramenée avec un plafond de Station.
- Le repli quand la Station ne propose plus PyroWave (`fallback_codec`) et l'image qui ne démarre pas (repli en H.264).
- WebGPU sur d'autres machines (Windows avec une carte NVIDIA, AMD, Intel ; Firefox ; iPad).
