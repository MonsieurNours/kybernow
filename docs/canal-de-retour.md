# Canal de retour et transport ajusté au réseau

[English](canal-de-retour.en.md) · Français

Ce document est le contrat du canal de retour app → Station (correctifs Kyber 0050 à 0052) et de ce qui s'en sert : la réparation des datagrammes vidéo (RaptorQ), l'attente des paquets en désordre et la redondance du son. État : **écrit et compilé, essayé seulement par les tests du cœur et de kyproto ; jamais sur un vrai réseau** (voir « À essayer »).

## Pourquoi

Jusqu'ici, la Station ne savait rien de ce que l'app recevait. Elle envoyait donc partout la même chose :

- **30 % de réparation RaptorQ** sur chaque image (et au moins 2 datagrammes), que le réseau perde 0 % ou 10 % des paquets. Sur un câble propre, c'est 30 % du débit pour rien ; avec PyroWave (plusieurs centaines de Mbit/s), des dizaines de Mbit/s.
- Côté app, **50 ms d'attente** chaque fois qu'une image manque alors que la suivante est là, avant de passer outre. Une image perdue au-delà de la réparation figeait donc l'image 50 ms de plus.
- **Le son en RaptorQ** : chaque paquet Opus de 10 ms tient dans un datagramme, auquel s'ajoutaient 2 datagrammes de réparation. Trois fois plus de paquets pour protéger quelques centaines d'octets.

Et le débit automatique de l'app (`apps/core/src/adaptive.rs`) jugeait les pertes à partir des compteurs de QUIC, qui comptent ce que chaque côté perd *en envoyant* (les pertes de l'app vers la Station s'y mêlent à celles de l'image), rapportés au nombre de paquets estimé d'après les octets reçus.

## Négociation (anciennes Stations, anciennes apps)

Rien ne change tant que les deux côtés ne sont pas d'accord :

1. L'app ajoute à sa requête de démarrage (`POST /start_session`, `StartRequest`) un champ `feedback` : `{"v": 1, "audio_redundancy": true}`. Une Station d'avant l'ignore (serde laisse passer les champs inconnus) et envoie comme avant.
2. Une Station qui le comprend numérote ses datagrammes vers cette app à partir de là (voir « Numérotation »), ouvre le point d'arrivée du canal de retour **avant** les flux d'image et de son, et répond dans `StartSessionResponse` : `"feedback": {"v": 1, "endpoint": <id>, "audio_redundancy": true|false}`. Une app d'avant ne demande rien et ne reçoit rien de neuf.
3. L'app se branche sur `endpoint`. Si la réponse n'a pas de `feedback` (Station d'avant), l'app garde ses 50 ms et ne rapporte rien.

Le client web (moteur `kyber-web`) ne demande jamais le canal : il garde le transport d'avant.

Le relais (`server/relay/tunnel`) ne voit rien de tout cela : tout passe à l'intérieur de la connexion QUIC chiffrée de Kyber, qu'il transporte telle quelle (UDP, ou TCP en repli). Aucun changement du relais.

## Numérotation des datagrammes (Station → app)

Une fois le canal négocié, **chaque datagramme** que la Station envoie sur cette connexion (image, son, battements du canal, toutes voies confondues) porte un numéro de connexion :

```text
[id de voie | 0x8000 : u16][contenu de la voie…][numéro : u32, gros-boutiste]
```

- Le bit fort de l'id de voie dit « numéroté ». Les ids de voie de kyproto vont de 2 en 2 à partir de 0 ou 1 : ils n'atteignent jamais 0x8000.
- Le numéro est **à la fin** : le routeur de l'app le retire sans recopier le datagramme (`Bytes::slice`), et les voies ne voient pas la différence (elles ignorent l'id qu'elles lisent).
- Il coûte 4 octets par datagramme : la taille utile d'une voie (`max_datagram_size`) baisse de 4 quand la numérotation est en marche.
- L'app comprend toujours un datagramme numéroté, qu'elle ait demandé le canal ou non : une Station ne numérote que pour une app qui l'a demandé.
- L'app ne numérote pas ce qu'elle envoie.

## Le point d'arrivée `feedback`

Une voie kyproto ordinaire (enregistrée par la Station, rejointe par l'app), en **datagrammes seulement** : un message par datagramme, en MessagePack **avec noms de champs** (`rmp_serde::to_vec_named`). Un champ inconnu est ignoré, un champ absent prend sa valeur par défaut, un message d'un type inconnu est ignoré : les versions suivantes ajoutent des champs sans casser les précédentes. Un message perdu ne perd rien d'important : les compteurs sont cumulés depuis le début.

### `Report` — app → Station, toutes les 100 ms (même vide)

| Champ | Type | Sens |
|---|---|---|
| `n` | u32 | numéro du rapport |
| `hi` | u32 | plus haut numéro de datagramme reçu |
| `received`, `lost`, `late` | u64 | cumul depuis le début : reçus ; jugés perdus (manquants 50 ms après qu'un suivant est arrivé) ; arrivés après avoir été jugés perdus |
| `expected_i`, `lost_i` | u32 | sur cet intervalle : datagrammes dont le sort s'est décidé, et parmi eux les perdus (avant toute réparation) |
| `reorder_us` | u32 | retard des datagrammes arrivés dans le désordre : 99e centile sur 2 s (0 s'il n'y en a pas) |
| `wait_us` | u32 | attente d'image manquante en place dans l'app |
| `streams` | liste | une entrée par voie d'image : `ep` (id de voie), `delivered` (dernier numéro de paquet d'image rendu au décodeur), `frames` (paquets rendus, cumul), `lost` (paquets abandonnés, cumul) |
| `losses` | liste | les 8 dernières pertes d'image (`FrameLoss`, ci-dessous), répétées pour couvrir un message perdu |

### `Lost` — app → Station, aussitôt

Un `FrameLoss` envoyé dès que l'app renonce à un paquet d'image (réparation insuffisante, attente écoulée) : `id` (u32, croissant, pour dédoublonner), `ep` (id de voie), `first` (numéro du premier paquet perdu, numérotation `kypacket_seq` de la voie), `count`. Il est répété dans les rapports suivants. **C'est l'entrée de la session « opti-pertes »** (réparation sans image clé).

### `Beat` — Station → app, toutes les 100 ms

| Champ | Type | Sens |
|---|---|---|
| `n` | u32 | numéro du battement |
| `report` | u32 | dernier rapport lu |
| `repair_pm` | u16 | part de réparation visée, en ‰ des symboles source (300 = les 30 % d'avant) |
| `repair_ratio_pm` | u16 | réparation réellement envoyée sur la dernière seconde, en ‰ des octets source |
| `loss_p95_pm` | u16 | pertes brutes, 95e centile sur 5 s, en ‰ |
| `audio_redundancy` | bool | le son porte le paquet précédent |
| `rtt_us` | u32 | aller-retour QUIC vu de la Station |
| `quic_lost` | u64 | paquets QUIC perdus par la Station (cumul) |

## Ce que la Station en fait

### Réparation des images

Pour chaque intervalle rapporté (au moins 20 datagrammes décidés), la part perdue ; sur les 5 dernières secondes, son **95e centile** `p95`. La probabilité de perte qu'on protège : `p = min(35 %, 0,5 % + 1,25 × p95)` (jamais moins de 0,5 %, même sur un câble parfait).

Pour une image de `K` symboles source, la Station envoie le **plus petit nombre `R` de symboles de réparation** tel que la probabilité de perdre plus de `R` des `K + R` datagrammes (loi binomiale de paramètre `p`) reste sous **1 sur 1 000**. `R` vaut au moins 1, et au plus la moitié de `K` (au moins 3 pour les toutes petites images). Calcul exact jusqu'à 128 symboles (table refaite quand `p` change), approximation normale au-delà.

Ordres de grandeur, réseau propre (`p` = 0,5 %) : 1 symbole de réparation pour une image d'un symbole, 2 pour 10, 3 pour 50, 6 pour 300 (2 %). Avant : 2, 3, 15 et 90.

Repli sûr : avant la première seconde de rapports, ou après une seconde sans rapport, la Station revient aux 30 % et 2 au moins d'avant.

**Dans le budget du débit vidéo.** Le débit d'un profil est déjà calculé pour laisser 30 % de réparation ([profil-auto.md](profil-auto.md) : « moins la correction d'erreurs (30 %) »). Tant que la réparation reste sous ces 30 %, l'encodeur garde le débit visé et le réseau porte moins qu'avant. Quand elle dépasse 30 % (réseau qui perd beaucoup), l'app baisse le débit qu'elle demande à `débit visé × 1,3 / (1 + r)` (`r` = `repair_ratio_pm` du battement) : image et réparation ensemble ne dépassent jamais ce que la session envoyait avant. Raison affichée dans les Statistiques : « baissé » (débit automatique).

### Son : le paquet précédent dans le même datagramme

Avec `audio_redundancy`, le son passe en datagrammes simples (`Unreliable`) et **chaque datagramme porte aussi le paquet Opus précédent** :

```text
[id de voie : u16][kypacket_seq : u32][fin | redondance (bit 30) | n° : u32][longueur du paquet : u16][paquet][paquet précédent]
```

- Un datagramme perdu ne coûte aucun son : le suivant, 10 ms plus tard, le rapporte. Il faut en perdre deux de suite pour un trou de 10 ms.
- Autant de datagrammes que sans protection (un par 10 ms), au lieu de trois avec RaptorQ. Le débit du son double (le paquet précédent voyage deux fois), soit environ 256 kbit/s de plus en son HD : peu, comparé à l'image.
- Si les deux paquets ne tiennent pas dans un datagramme (son 7.1 « studio »), le paquet précédent part dans un datagramme à lui, au format simple : un récepteur ignore un paquet déjà reçu.
- Sans `audio_redundancy` (Station ou app d'avant), le son garde `UnreliableFec` et ses 30 %.

## Ce que l'app en fait

### Attente des paquets en désordre

L'attente d'une image manquante vaut `min(50 ms, max(8 ms, 2 × reorder_us + 4 ms))`, recalculée à chaque rapport, 50 ms tant que l'app n'a pas vu 500 datagrammes numérotés. Elle se règle sur le **désordre mesuré** des datagrammes (grâce à leurs numéros), pas sur la gigue des images : attendre ne sert qu'à laisser arriver un datagramme doublé par un suivant ; un datagramme perdu n'arrive jamais, et la gigue d'une file d'attente retarde aussi les suivants, sans désordre. Sur un câble ou un Wi-Fi ordinaire (qui livre dans l'ordre), l'attente tombe à 8 ms : une image perdue fige 42 ms de moins.

Le son garde ses 10 ms (le paquet de secours arrive avec le suivant, 10 ms après).

### Débit automatique

Chaque seconde, la session reçoit le bilan du canal (`Metric::Feedback` de kyclient) : datagrammes attendus et perdus (bruts, avant réparation), désordre, attente, part de réparation, pertes d'image. Le débit automatique juge alors les pertes sur ces nombres exacts, au lieu de les estimer à partir des compteurs de QUIC et des octets reçus.

## Pour les autres chantiers

- **opti-pertes** (réparation sans image clé) : côté Station, `kyproto::FeedbackHost::subscribe()` rend les événements `FeedbackEvent::Lost { endpoint, first, count, frames }`, où `frames` donne, pour chaque paquet perdu encore connu, son `pts` et s'il était une image clé (la Station garde les 512 derniers paquets envoyés par voie), et `FeedbackEvent::Report` avec `delivered` par voie (accusé de réception des images rendues au décodeur). kycontroller garde le `FeedbackHost` de la session dans ses services (`services.feedback`).
- Ajouter un champ : dans `Report`, `Beat` ou `FrameLoss`, avec `#[serde(default)]`, sans changer `v`. Un nouveau type de message : ignoré par les versions d'avant. `v` ne change que pour un changement incompatible.

## À essayer sur un vrai réseau

- Câble : part de réparation vers 2 à 7 % selon la taille des images, aucune image perdue ; attente à 8 ms.
- Wi-Fi chargé (micro-ondes, deuxième appareil qui télécharge) : la part suit les pertes, les images perdues restent rares ; vérifier que les pertes en rafale (Wi-Fi qui décroche 20 à 50 ms) ne coûtent pas plus qu'avant.
- Relais en UDP et en TCP (en TCP, aucune perte : part au minimum).
- Son : couper 1 datagramme sur 20 (`pfctl`/`dnctl` ou Network Link Conditioner) : aucun trou audible.
- Station d'avant + app nouvelle, Station nouvelle + app d'avant : session normale, 30 % et 50 ms comme avant.
