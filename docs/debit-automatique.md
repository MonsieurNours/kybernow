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

## Dans les Statistiques : le débit visé

Chaque seconde, le cœur donne aux apps le débit qu'il demande à l'ordinateur et ce qui le fixe (`Second::target`, `adaptive::TargetBitrate`) ; les Statistiques le montrent sous « Débit » (ligne « Débit visé » et trait sur la courbe, [link-ui.md](link-ui.md)). Quand plusieurs choses jouent, c'est celle qui fixe la valeur qui est dite, dans l'ordre où la session les applique : le plafond de l'app (« 20 Mbit/s · fenêtre cachée »), puis le repli (« 15 Mbit/s · flux allégé »), puis le débit automatique (« 42 / 60 Mbit/s · baissé, réseau saturé ») ; sinon la nature du réglage : « 320 Mbit/s · PyroWave, Auto en pause », « 80 Mbit/s · réglé à la main », « 60 Mbit/s · Auto ». Un plafond au-dessus du débit ne fixe rien. API et ponts : [apps/core/README.md](../apps/core/README.md#débit-visé-dans-les-statistiques-srcadaptivers-srcsessionrs-srclatencyrs).

## Ce qui n'est pas fait

- Rien ne prévient d'une liaison trop faible, en dehors du débit visé des Statistiques (ci-dessous ; pas dans l'app des téléviseurs LG).
- **Cartes AMD** (encodeur AMF) : le débit change en direct depuis la 0.11.1 (correctif FFmpeg 0023 de 0026) ; **Intel** (Quick Sync) : chaque changement remet l'encodeur à zéro (une image clé), ce que FFmpeg fait de lui-même.
- Pas de baisse de définition ni de fréquence : seul le débit bouge.
- La correction d'erreurs garde sa part fixe de 30 %.
- Le client web du navigateur garde son débit fixe, et ne montre pas le bandeau « Le réseau sature » en PyroWave.
- Réglages faits par le raisonnement et les tests du cœur, pas encore sur un vrai réseau saturé (Wi-Fi chargé, 4G, relais).

## Tests

`cargo test -p kybernow-core adaptive` (le calcul : réseau local propre, file qui monte, pic de Wi-Fi isolé, pertes lourdes, bureau immobile, plancher, remontée, remontée prudente, aucune image, nouveau chemin, trous) et `… debit_automatique` (la session : profil seulement, pause sous un plafond de l'app, débit réglé à la main intact, débit visé et sa raison), `… debit_vise` (raisons, priorités, textes). Le correctif FFmpeg est compilé pour le Mac (VideoToolbox) et pour Windows (NVENC, compilation croisée), pas encore essayé sur une Station.
