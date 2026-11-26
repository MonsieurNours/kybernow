# Débit automatique

[English](debit-automatique.en.md) · Français

Avec l'interrupteur « Débit adaptatif » des réglages (allumé par défaut, [profil-auto.md](profil-auto.md)), l'app baisse le débit de l'image quand le réseau sature, puis le remonte quand il redevient propre : pour un profil (Jeu, Bureau, Max, Auto) comme pour un débit réglé à la main. Coupé, le débit ne bouge jamais. Le calcul est dans le cœur (`apps/core/src/adaptive.rs`), donc le même dans toutes les apps qui l'utilisent ; le client web du navigateur n'en a pas encore.

## Ce que regarde l'app

Chaque seconde, la session lit ce que Kyber et le cœur mesurent déjà (`latency::Second`) :

- **l'aller-retour réseau** (celui de la connexion QUIC de Kyber), comparé à son plus bas des 10 dernières secondes : une hausse de plus de 15 ms, ou de plus de la moitié du repos, veut dire qu'une file se remplit quelque part (box, Wi-Fi, relais) ;
- **les paquets perdus**, rapportés aux paquets reçus (estimés d'après les octets, 1 200 par paquet) : 2 % ou plus, avec au moins 3 paquets perdus ;
- **les trous de l'image**, des pertes que la correction d'erreurs n'a pas pu réparer.

Une seconde chargée seule ne change rien : un pic de latence isolé, fréquent en Wi-Fi, passe sans effet. Il en faut deux d'affilée. Des pertes lourdes (10 % ou plus) font baisser tout de suite.

## Ce qu'elle fait

L'app garde une **part du débit du profil**, entre 1 (le débit du profil) et le plancher de 5 Mbit/s (celui de l'encodeur de la Station, Admin › Débit minimum) :

- réseau chargé : la part est multipliée par 0,7 ;
- 4 secondes propres d'affilée : elle remonte de 15 %, et de 5 % seulement près du débit qui avait chargé le réseau (pendant une minute) ;
- après chaque changement, 2 secondes sans décision (le réseau et l'encodeur s'y font), sauf pour des pertes lourdes.

C'est une part et non un débit : un changement de définition ou d'écrans garde la même marge. La session part du débit du profil (sur le réseau local, il passe d'emblée), la garde quand l'image est relancée (même réseau), et réapprend l'aller-retour au repos à chaque relance (le chemin a pu changer : réseau local, relais). Un nouveau profil la remet à 1.

Le débit automatique se met en pause quand l'app plafonne déjà le débit (fenêtre cachée, app en arrière-plan) ou n'affiche rien : ce qui passe alors ne dit rien du réseau. Il ne s'applique qu'à l'image de l'hôte : l'image à part d'un invité de KyberNow Share est ajustée par la Station (correctif 0020), et un emprunteur ne règle pas le débit.

Il s'ajoute au repli qui existait déjà (« pas d'image » : débit plafonné, puis 4:2:0) : le repli reste le dernier recours quand rien n'arrive du tout.

Chaque changement est écrit au journal de l'app : « débit automatique : 60 → 42 Mb/s (profil 60 Mb/s, aller-retour 41 ms, 0 paquets perdus) ».

## PyroWave : en pause, et « Passer en HEVC » si le réseau sature

Avec PyroWave ([pyrowave.md](pyrowave.md)), le débit automatique est **en pause**. PyroWave encode chaque image seule, avec un budget fixe : 30 % de débit en moins abîment toutes les images d'un coup, là où HEVC répartit la baisse. Et le débit de PyroWave est déjà ajusté au lien par la définition ramenée (plafond de la Station, du lien et de l'app), qui baisse la définition plutôt que la netteté.

Le réseau reste observé avec les mêmes signes. Quand il sature **5 secondes d'affilée** (`PYROWAVE_STRAIN`), le cœur envoie l'avis `PyroWaveStrained` : les apps montrent le bandeau « Le réseau sature », « L'image peut saccader. », avec « Passer en HEVC » (même forme que le bandeau du début, [link-ui.md](link-ui.md)). Une fois par session, pour l'hôte seulement, et pas quand l'app plafonne déjà le débit (fenêtre cachée). En HEVC, le débit automatique reprend la main.

## Côté Station : le débit change en direct

L'app envoie le nouveau débit par `/set_bitrate` de Kyber, qui le passe à l'encodeur. Le correctif `server/kyber-patches/0026-kymedia-ffmpeg-debit-en-direct.patch` (FFmpeg 8.1, Stations Windows, Mac et Linux) fait qu'un changement de débit ne coûte plus rien :

- **NVENC** : FFmpeg remettait l'encodeur à zéro et forçait une image clé à chaque changement de débit. Le correctif change le débit sans remise à zéro ni image clé (changement dynamique, que NVENC accepte quand la carte l'annonce). La mémoire de l'encodeur (VBV), réglée par défaut à 2 secondes du débit de départ, suit le nouveau débit : sinon, un débit baissé laisserait passer les rafales de l'ancien.
- **VideoToolbox** : FFmpeg ne lisait le débit qu'à la création de la session ; un changement en cours était ignoré. Le correctif le donne à la session (`AverageBitRate`) avant l'image suivante.
- **VA-API** (Station Linux, cartes AMD et Intel) : FFmpeg n'envoyait les paramètres du débit qu'avec les images IDR (toutes les 10 s sur la Station Linux). Le correctif les met à l'échelle du nouveau débit (mémoire HRD comprise) et les envoie avec l'image suivante, sans image clé.

Sans ce correctif (Station plus ancienne), le débit automatique fonctionne quand même, mais chaque changement coûte une image clé sur NVENC et reste sans effet sur la Station Mac (sur la Station Linux en VA-API, il attend l'image IDR suivante).

## Côté Station : chaque image tient dans sa part

Changer le débit ne suffit pas : il faut aussi que chaque image reste à peu près dans sa part de ce débit (le débit divisé par la fréquence). Une image beaucoup plus lourde, au moment d'un changement de scène ou d'une fenêtre qui s'ouvre, met plusieurs images de temps à partir, et toutes celles qui la suivent attendent derrière elle.

Avant, rien ne l'en empêchait :

- **NVENC** gardait la mémoire de l'encodeur (VBV) que FFmpeg règle par défaut : 2 secondes du débit. Une seule image pouvait donc peser jusqu'à 120 fois sa part.
- **Station Mac** : aucun intervalle d'images clés n'était donné à VideoToolbox, et FFmpeg en met une **toutes les 12 images** (5 par seconde à 60 Hz), chacune bien plus lourde que les autres. L'option `low_delay` passée par Kyber ne faisait rien (c'est un drapeau de FFmpeg, pas une option).
- **AMF** laissait l'intervalle d'images clés au pilote, **VA-API** en mettait une toutes les 120 images (2 s).

Les correctifs `server/kyber-patches/0030-*` (Stations Windows, Mac et Linux) et `server/macos/kyber-patches/0030-*` (VideoToolbox) règlent les encodeurs pour le plus faible retard :

- **Une image à la fois.** Nouvelle option de FFmpeg `kn_vbv_frames` (correctif FFmpeg `0030-avcodec-kybernow-vbv`, posé par `0030-kymedia-ffmpeg-vbv-une-image.patch`) : la mémoire de l'encodeur vaut une image au débit de crête (le débit lui-même, sauf réglage contraire). FFmpeg la calcule d'après la fréquence de l'écran à l'ouverture de l'encodeur, puis la recalcule à chaque changement de débit (`/set_bitrate`), avant l'image suivante et toujours sans image clé.
- **Un plancher pour les images clés.** Jamais moins de 150 kbit ni de 0,1 bit par point (207 kbit en 1080p, 829 kbit en 4K) : à bas débit et haute fréquence, une seule image (8 kbit à 500 kbit/s en 60 Hz) ne pourrait pas contenir une image clé lisible.
- **Une image clé toutes les 600 images** (10 s à 60 Hz) partout, comme NVENC le faisait déjà. Un client qui a perdu une image en demande une (`/force_idr`).

| Encodeur | Réglage |
| --- | --- |
| NVENC (Windows, Linux) | débit constant (`rc cbr`), mémoire d'une image |
| AMF (Windows) | débit constant, mémoire d'une image imposée (`enforce_hrd`), sans remplissage, mode faible latence ; débit de crête et mémoire changés en direct avec le débit |
| Quick Sync (Windows) | débit constant (choisi par FFmpeg quand la crête vaut le débit), mémoire d'une image |
| VA-API (Linux) | débit constant, mémoire d'une image, prise telle quelle à chaque changement de débit |
| VideoToolbox (Mac) | fenêtre `DataRateLimits` d'une image au lieu d'une seconde, changée en direct avec le débit |
| x264 (secours) | mémoire d'une image |

PyroWave garde son propre réglage. Chaque option peut être remplacée dans `kyber_config.toml`, section `[kyavserver.video_encoder_config]` (par exemple `kn_vbv_frames = 0` : retour à la mémoire de FFmpeg).

**Mode faible latence d'Apple écarté.** Mesuré sur la Station Mac (puce Apple), `EnableLowLatencyRateControl` ignore la fenêtre `DataRateLimits` (mêmes tailles d'image avec ou sans) et encode plus lentement que le mode normal, qui sort déjà une image pour une image reçue. Il reste donc éteint.

**Mesures** (Mac de l'auteur pour VideoToolbox, VM Linux arm64 pour x264 ; 1080p60, bureau synthétique avec un changement de scène par seconde et un défilement, débit de 10 puis 4 puis 20 Mbit/s changé en cours ; retard moyen pour envoyer chaque image sur un lien au débit visé, file comprise) :

| | Images clés sur 600 | Retard moyen d'envoi | Plus long |
| --- | --- | --- | --- |
| HEVC, avant | 50 | 1 152 ms | 1 990 ms |
| HEVC, après | 1 | 20 ms | 284 ms |
| H.264, avant | 50 | 156 ms | 629 ms |
| H.264, après | 1 | 14 ms | 208 ms |
| x264, avant | 3 | 1 880 ms | 4 269 ms |
| x264, après | 1 | 9 ms | 44 ms |

x264 tient la mémoire à la lettre (plus grosse image : 30 Ko au lieu de 479 Ko). VideoToolbox, lui, laisse les changements de scène plus lourds que leur part (il ne tient pas exactement la fenêtre) : c'est le plancher du retard restant sur Mac. Le temps d'encodage ne s'allonge pas (quelques millisecondes par image, un peu moins qu'avant). Le banc d'essai est `server/kyber-patches/tools/encoder-frame-bench.c` (NVENC, AMF, Quick Sync, VideoToolbox ; options d'avant ou d'après).

## Dans les Statistiques : le débit visé

Chaque seconde, le cœur donne aux apps le débit qu'il demande à l'ordinateur et ce qui le fixe (`Second::target`, `adaptive::TargetBitrate`) ; les Statistiques le montrent sous « Débit » (ligne « Débit visé » et trait sur la courbe, [link-ui.md](link-ui.md)). Quand plusieurs choses jouent, c'est celle qui fixe la valeur qui est dite, dans l'ordre où la session les applique : le plafond de l'app (« 20 Mbit/s · fenêtre cachée »), puis le repli (« 15 Mbit/s · flux allégé »), puis le débit automatique (« 42 / 60 Mbit/s · baissé, réseau saturé ») ; sinon la nature du réglage : « 320 Mbit/s · PyroWave, Auto en pause », « 80 Mbit/s · réglé à la main », « 60 Mbit/s · Auto ». Un plafond au-dessus du débit ne fixe rien. API et ponts : [apps/core/README.md](../apps/core/README.md#débit-visé-dans-les-statistiques-srcadaptivers-srcsessionrs-srclatencyrs).

## Ce qui n'est pas fait

- Rien ne prévient d'une liaison trop faible, en dehors du débit visé des Statistiques (ci-dessous ; pas dans l'app des téléviseurs LG).
- **Cartes AMD** (encodeur AMF) : le débit change en direct depuis la 0.11.1 (correctif FFmpeg 0023 de 0026) ; **Intel** (Quick Sync) : chaque changement remet l'encodeur à zéro (une image clé), ce que FFmpeg fait de lui-même.
- **Mémoire d'une image** (correctifs 0030) : mesurée sur la Station Mac seulement. NVENC, AMF, Quick Sync et VA-API sont compilés, pas encore essayés sur une vraie carte ; à vérifier : pas d'image sautée ni de blocs au changement de scène, image clé lisible à bas débit, débit moyen proche du débit visé.
- Pas de baisse de définition ni de fréquence : seul le débit bouge.
- La correction d'erreurs ne bouge plus seule : avec une Station qui a le canal de retour, elle suit les pertes mesurées et reste dans la part de 30 % que le profil lui laisse ; au-delà, le débit demandé baisse d'autant (`TargetInputs::repair_pm`, raison « baissé »), PyroWave compris. Avec une Station d'avant, elle garde sa part fixe de 30 % ([canal-de-retour.md](canal-de-retour.md)).
- Avec le canal de retour, les pertes sont celles que l'app compte sur les datagrammes de la Station (`Second::feedback`), et non plus une estimation d'après les compteurs de QUIC.
- Le client web du navigateur garde son débit fixe, et ne montre pas le bandeau « Le réseau sature » en PyroWave.
- Réglages faits par le raisonnement et les tests du cœur, pas encore sur un vrai réseau saturé (Wi-Fi chargé, 4G, relais).

## Tests

`cargo test -p kybernow-core adaptive` (le calcul : réseau local propre, file qui monte, pic de Wi-Fi isolé, pertes lourdes, bureau immobile, plancher, remontée, remontée prudente, aucune image, nouveau chemin, trous) et `… debit_automatique` (la session : profil seulement, pause sous un plafond de l'app, débit réglé à la main intact, débit visé et sa raison), `… debit_vise` (raisons, priorités, textes). Le correctif FFmpeg est compilé pour le Mac (VideoToolbox) et pour Windows (NVENC, compilation croisée), pas encore essayé sur une Station. Correctifs 0030 : `cargo test -p kyavservice rate_control` (options de chaque encodeur), et banc d'essai VideoToolbox (FFmpeg corrigé compilé pour le Mac, encodage d'images synthétiques, mesures ci-dessus).
