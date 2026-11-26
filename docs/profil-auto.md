# Profil « Auto » et débit adaptatif

[English](profil-auto.en.md) · Français

Textes et écrans : [link-ui.md](link-ui.md), « Profil Auto et débit adaptatif ». Calcul du débit adaptatif : [debit-automatique.md](debit-automatique.md).

## Ce qui change

- **Débit adaptatif** devient un interrupteur de la carte « Débit » (allumé par défaut), pour tous les débits : les profils comme un débit réglé à la main. Coupé, le débit ne bouge jamais. Champ `adaptive` des réglages (absent : oui).
- **PyroWave par câble** n'est plus pris que par **Max** et **Auto** ; Jeu et Bureau restent en HEVC (`profile::pyrowave_format`).
- **Auto**, quatrième profil (`auto`) : un test des conditions, lancé seulement à la demande (en choisissant Auto, ou « Refaire le test »), qui propose des réglages ; rien ne change avant « Appliquer ». Le résultat est gardé par l'app pour cet ordinateur.

## Le test

Le cœur le mène (`apps/core/src/autotest.rs`) et en donne l'avancement aux apps, étape par étape. Chaque étape est cochée, ou passée avec sa raison. Environ 10 secondes.

| Étape | Clé | Ce qui est mesuré |
|---|---|---|
| 1. Liaison | `link` | câble, Wi-Fi ou relais (donné par l'app et par le chemin de la console) |
| 2. Latence au repos | `latency` | 20 allers-retours vers la Station (`GET /api/stream/ping`) en 2 s : le plus bas, la médiane, la gigue (écart entre le 9ᵉ et le 1ᵉʳ décile) |
| 3. Débit vers cet appareil | `bandwidth` | une rafale de 4 s envoyée par la Station (`GET /api/stream/probe`), lue aussi vite que possible : débit de chaque quart de seconde, médiane de la seconde moitié ; pendant la rafale, des allers-retours (`/api/stream/ping`) donnent la latence en charge |
| 4. Décodage de cet appareil | `decode` | des extraits vidéo embarqués (HEVC, HEVC 4:4:4, AV1, H.264, PyroWave 4:2:0 et 4:4:4), en 4K, décodés aussi vite que possible par le décodeur de l'app : images par seconde de chacun |
| 5. Encodeur de l'ordinateur | `encoder` | l'encodeur de la Station (`GET /api/stream/encoder`, voir « L'encodeur de l'ordinateur ») essayé au format, à la définition, à la fréquence et au débit choisis ; refusé : 50 Mbit/s de moins, jusqu'au premier débit accepté. Passée en PyroWave, avec une Station d'avant (« Mettez à jour {nom} pour essayer son encodeur. ») ou si la Station n'a pas pu l'essayer |

**Station d'avant** (pas de `/api/stream/probe`, réponse 404) : l'étape 3 est passée, « Mettez à jour {nom} pour mesurer le débit. », et le débit suit le type de liaison (câble 300 Mbit/s, Wi-Fi 80, relais 25, en 4K à 60 Hz).

## Les choix

Le cœur décide (`autotest::decide`), dans cet ordre, et donne la raison de chaque choix :

1. **Budget** : le débit mesuré, moins une marge (câble 40 %, Wi-Fi et relais 50 %), moins la correction d'erreurs (30 %, la part la plus haute qu'elle prend sans baisser le débit : [canal-de-retour.md](canal-de-retour.md)).
2. **Format** : PyroWave si l'ordinateur le propose, que l'appareil le décode assez vite, qu'il est relié par câble (pas par le relais) et que le budget couvre son débit à la définition de l'écran ; sinon HEVC (AV1 si l'appareil ne décode pas HEVC, H.264 en dernier).
3. **Définition et fréquence** : celles de l'écran de l'appareil, tant que le décodage tient la fréquence avec une marge de 20 % ; sinon la fréquence baisse (120 → 60 Hz), puis la définition (4K → 1440p → 1080p).
4. **Texte net (4:4:4)** : si l'appareil le décode assez vite et que le budget couvre un tiers de débit en plus (en PyroWave : la moitié).
5. **Débit** : le budget, au plus celui de Max, au plus le débit que l'encodeur de l'ordinateur a accepté (étape 5 ; en PyroWave, au plus son plafond sur l'ordinateur) et au moins 10 Mbit/s ; gardé calé sur la 4K à 60 Hz comme celui des profils.
6. **Transport** : Fiable si la gigue dépasse 10 ms ou si la latence en charge monte de plus de 50 ms (réseau qui retient les paquets) ; sinon Rapide.
7. **Débit adaptatif** : allumé.

Le HDR, l'affichage des images, le son et les commandes ne changent pas : ce sont des choix de l'utilisateur.

## Les réglages d'Auto

`AutoResult` (JSON, gardé par l'app pour cet ordinateur) : la date, le réseau du test (`network` : le nom donné par l'app, ou `relay`), les mesures, et les réglages choisis (`codec`, `yuv444`, `width`, `height`, `refresh`, `mbps`, `reliable` ; `AutoResult::settings(hdr, wired)` en donne les `StreamSettings` avec `profile: "auto"`). `codec: "pyrowave"` veut dire PyroWave par câble, `mbps` restant celui de HEVC (pris en Wi-Fi ou par le relais). En session, Auto se comporte comme un profil : le débit suit les pixels par seconde, le débit adaptatif agit.

**Autre réseau** que celui du test : le résultat reste, et un bandeau discret le dit au début de la session, avec « Refaire le test » (ouvre le panneau du test).

Sans test (Auto jamais appliqué) : choisir Auto ouvre le test ; « Annuler » garde le profil d'avant.

## Côté Station

`GET /api/stream/probe?ms=4000` (Admin, même accès que `/api/stream/info`, par le réseau ou le relais) : des octets aléatoires, envoyés aussi vite que la connexion les prend, pendant `ms` millisecondes (au plus 6 000), au plus 2 Gbit/s ; une seule rafale à la fois (429 sinon). `GET /api/stream/ping` : réponse vide immédiate. La rafale passe par HTTPS (TCP), pas par le QUIC de Kyber : elle mesure le chemin, pas l'encodeur.

## L'encodeur de l'ordinateur

Un encodeur matériel n'accepte pas n'importe quel débit : NVENC d'une RTX 3080 Ti refusait HEVC 4:2:0 à 593 Mbit/s en 4K 160 Hz (« erreur de la chaîne d'image » à chaque départ) et tenait 290. Chaque carte a ses limites, inconnues d'avance : on les cherche.

- **Sonde de la Station** : `GET /api/stream/encoder?codec=hevc&width=3840&height=2160&fps=160&mbps=593&yuv444=0&hdr=0` (Admin, même accès que `/api/stream/probe`, une sonde à la fois, 429 sinon) → `{"ok": true}`, `{"ok": false, "error": "…"}` (refusé), ou `{"ok": null, "error": "…"}` (pas pu essayer : outil absent, carte occupée). Admin lance `kybernow-encoder-check` (à côté de lui : `kybernow-encoder-check.exe` sur Windows, dans l'app sur Mac, dans le paquet sur Linux) avec l'encodeur réglé pour Kyber (`video.encoder` : NVENC, AMF, QSV sur Windows ; VideoToolbox sur Mac ; VA-API, NVENC sur Linux) : il ouvre l'encodeur de FFmpeg avec les options de Kyber et encode quelques images ; x264 est toujours accepté. `info` l'annonce (`encoder_check: true`) ; une Station d'avant répond 404.
- **Dans le test** (étape 5) : le cœur essaie le débit choisi (par écran, à la définition et à la fréquence choisies), puis 50 Mbit/s de moins à chaque refus, 50 Mbit/s au plus bas. Le résultat (`measures.encoder` : format, 4:4:4, définition, fréquence, `mbps`, `limited` si un débit a été refusé) borne le débit choisi ; raison du débit : « {nom} encode jusqu'à {n} Mbit/s ». Mesure de l'étape : « jusqu'à {n} Mbit/s en {format} », ou « {n} Mbit/s acceptés en {format} » quand le premier essai passe.
- **Plafond par ordinateur** : l'app garde pour chaque ordinateur le débit le plus haut connu de son encodeur (`encoder_max`, Mbit/s par écran, avec les réglages de l'ordinateur) et le donne à la session (`StreamSettings::encoder_max`, JSON `encoder_max`), qui ne demande jamais plus, quel que soit le profil ou le débit réglé à la main. Mis à jour par « Appliquer » (`AutoResult::settings` : `encoder_max` quand l'étape a trouvé une limite ; `encoder_ok` quand le débit essayé a passé du premier coup, et alors un plafond gardé plus bas est retiré) et par la session (ci-dessous). Pas de plafond connu : aucun.
- **En session** (toutes les Stations, celles d'avant comprises) : une image refusée par l'ordinateur avant toute image (`StreamerPipelineEvent` d'erreur) fait redescendre le débit de 50 Mbit/s par écran et relance le flux, jusqu'à 50 Mbit/s ; une fois l'image venue, la session le dit à l'app (`Notice::EncoderCeiling { mbps }`), qui le garde comme plafond de l'ordinateur. Plus bas que 50 : le repli habituel (4:2:0), puis trois relances au plus et l'échec « pas d'image ». Statistiques, débit visé : « {n} Mbit/s · plafond de l'encodeur ».

## Le décodage

Les extraits sont dans le cœur (`apps/core/samples/`, faits par `apps/core/scripts/samples.sh`) : 3840 × 2160, une image clé puis 29 images (HEVC, AV1, H.264), une image intra répétée (PyroWave). Le cœur les donne en paquets comme ceux de Kyber (`autotest::samples_for(caps)`) ; l'app les passe à son décodeur, aussi vite qu'il les prend, pendant 1 s au plus par extrait (en boucle), et rend les images décodées par seconde (`SampleDecoder`). Un format que l'app ne décode pas est passé sans essai.
