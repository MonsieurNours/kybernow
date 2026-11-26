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

**Station d'avant** (pas de `/api/stream/probe`, réponse 404) : l'étape 3 est passée, « Mettez à jour {nom} pour mesurer le débit. », et le débit suit le type de liaison (câble 300 Mbit/s, Wi-Fi 80, relais 25, en 4K à 60 Hz).

## Les choix

Le cœur décide (`autotest::decide`), dans cet ordre, et donne la raison de chaque choix :

1. **Budget** : le débit mesuré, moins une marge (câble 40 %, Wi-Fi et relais 50 %), moins la correction d'erreurs (30 %).
2. **Format** : PyroWave si l'ordinateur le propose, que l'appareil le décode assez vite, qu'il est relié par câble (pas par le relais) et que le budget couvre son débit à la définition de l'écran ; sinon HEVC (AV1 si l'appareil ne décode pas HEVC, H.264 en dernier).
3. **Définition et fréquence** : celles de l'écran de l'appareil, tant que le décodage tient la fréquence avec une marge de 20 % ; sinon la fréquence baisse (120 → 60 Hz), puis la définition (4K → 1440p → 1080p).
4. **Texte net (4:4:4)** : si l'appareil le décode assez vite et que le budget couvre un tiers de débit en plus (en PyroWave : la moitié).
5. **Débit** : le budget, au plus celui de Max et au plus 290 Mbit/s par écran en HEVC, AV1 et H.264 (ce qu'un encodeur matériel accepte, `ENCODER_MAX_MBPS` ; en PyroWave, au plus son plafond sur l'ordinateur) et au moins 10 Mbit/s ; gardé calé sur la 4K à 60 Hz comme celui des profils.
6. **Transport** : Fiable si la gigue dépasse 10 ms ou si la latence en charge monte de plus de 50 ms (réseau qui retient les paquets) ; sinon Rapide.
7. **Débit adaptatif** : allumé.

Le HDR, l'affichage des images, le son et les commandes ne changent pas : ce sont des choix de l'utilisateur.

## Les réglages d'Auto

`AutoResult` (JSON, gardé par l'app pour cet ordinateur) : la date, le réseau du test (`network` : le nom donné par l'app, ou `relay`), les mesures, et les réglages choisis (`codec`, `yuv444`, `width`, `height`, `refresh`, `mbps`, `reliable` ; `AutoResult::settings(hdr, wired)` en donne les `StreamSettings` avec `profile: "auto"`). `codec: "pyrowave"` veut dire PyroWave par câble, `mbps` restant celui de HEVC (pris en Wi-Fi ou par le relais). En session, Auto se comporte comme un profil : le débit suit les pixels par seconde, le débit adaptatif agit.

**Autre réseau** que celui du test : le résultat reste, et un bandeau discret le dit au début de la session, avec « Refaire le test » (ouvre le panneau du test).

Sans test (Auto jamais appliqué) : choisir Auto ouvre le test ; « Annuler » garde le profil d'avant.

## Côté Station

`GET /api/stream/probe?ms=4000` (Admin, même accès que `/api/stream/info`, par le réseau ou le relais) : des octets aléatoires, envoyés aussi vite que la connexion les prend, pendant `ms` millisecondes (au plus 6 000), au plus 2 Gbit/s ; une seule rafale à la fois (429 sinon). `GET /api/stream/ping` : réponse vide immédiate. La rafale passe par HTTPS (TCP), pas par le QUIC de Kyber : elle mesure le chemin, pas l'encodeur.

## Le décodage

Les extraits sont dans le cœur (`apps/core/samples/`, faits par `apps/core/scripts/samples.sh`) : 3840 × 2160, une image clé puis 29 images (HEVC, AV1, H.264), une image intra répétée (PyroWave). Le cœur les donne en paquets comme ceux de Kyber (`autotest::samples_for(caps)`) ; l'app les passe à son décodeur, aussi vite qu'il les prend, pendant 1 s au plus par extrait (en boucle), et rend les images décodées par seconde (`SampleDecoder`). Un format que l'app ne décode pas est passé sans essai.
