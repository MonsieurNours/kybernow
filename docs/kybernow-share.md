# KyberNow Share : le partage en local

[English](kybernow-share.en.md) · Français

KyberNow Share fait entrer quelqu'un d'autre sur votre ordinateur : un **spectateur**, qui voit et entend, ou un **joueur**, dont les manettes deviennent la manette suivante de Windows. Ce premier pas vaut **sur le même réseau** ; le relais (hors de chez soi) et l'encodage à part pour un invité à distance viendront ensuite ([ROADMAP.md](../ROADMAP.md)). Ce document est le contrat commun de la Station, du cœur Rust, de toutes les apps Link et du client web.

Le partage n'a pas d'écrans à lui : il reprend l'accueil, la connexion, la barre de session et les réglages de [link-ui.md](link-ui.md), qui donne les textes exacts (section « Partage »). Toutes les apps se comportent pareil et emploient les mêmes mots.

## Ce que voit l'utilisateur

### Partager

On partage de deux façons :

- **Depuis sa session** (l'hôte) : « Partager » dans la barre de session. Le partage montre son écran principal, et le son de l'ordinateur.
- **Depuis le PC lui-même**, sans session : « Partager un écran… » dans le menu de l'icône de KyberNow Station près de l'horloge, ou sur la page d'accueil d'Admin. On choisit l'écran à partager parmi les écrans du PC ; on continue d'utiliser son PC normalement, les invités voient cet écran et entendent le son du PC. Aucun écran virtuel n'est créé, les écrans du PC ne s'éteignent pas.

Partager ouvre un panneau : un **code** à six chiffres (« 123 456 »), un **lien** et son **QR code** (`kybernow://join?…`, ou l'adresse du client web), et la liste des invités. Le code vaut tant que le partage est ouvert ; « Nouveau code » le change (les invités déjà là restent). « Arrêter le partage » fait sortir tout le monde. Fermer la session de l'hôte arrête aussi le partage.

### Les invités

- Un invité **arrive en spectateur** : il voit et entend, n'envoie rien.
- Dans la liste, l'hôte peut **« Faire jouer »** un invité : ses manettes deviennent la manette suivante de Windows (une manette virtuelle à lui). Puis **« Donner le clavier et la souris »**, s'il le veut ; « Reprendre le clavier et la souris » les lui retire. « Retirer » le fait sortir du partage.
- **Quatre invités au plus.** Au-delà, le code est refusé avec la raison.
- La Station impose ces droits : un spectateur n'a aucune entrée sur l'ordinateur, un joueur seulement ses manettes (et le clavier et la souris si l'hôte les lui a donnés), même si son appareil essayait d'envoyer autre chose.
- Admin montre le partage (écran, invités, rôle de chacun), peut retirer un invité et arrêter le partage ; son fil le raconte (« Léa regarde votre écran », « Léa joue avec vous »).

### Rejoindre

- **« Rejoindre avec un code »** sur l'accueil de toutes les apps Link (à côté de « Ajouter un ordinateur ») : on tape les six chiffres ; l'app les envoie aux ordinateurs qu'elle trouve sur le réseau, celui qui partage répond. Ou on ouvre le lien, ou on scanne le QR code (apps qui ont un appareil photo). Dans le **navigateur**, le lien ouvre le client web en invité, sans mot de passe d'Admin.
- L'invité donne son prénom (proposé : le nom de l'appareil), retenu pour la fois suivante.
- La session d'invité est une session ordinaire à l'écran, avec une barre réduite : « Spectateur » ou « Joueur », « Quitter le partage », et les commandes que son rôle permet (manette, clavier et souris s'ils lui sont donnés). Pas de profil, pas de réglages de l'ordinateur, pas d'écran virtuel : un invité ne change jamais les écrans de l'ordinateur et ne prend jamais la main à l'hôte.
- Un invité n'est **pas ajouté à la liste des ordinateurs** de l'appareil : rien n'est gardé après le partage, sauf son prénom.
- Fin : « {hôte} a arrêté le partage. », « {hôte} vous a retiré du partage. » ou la raison d'un échec ; jamais « Reprendre la main ».

### Ce qui reste comme avant

- L'image est **commune** : l'invité reçoit le format choisi par l'hôte (H.264, HEVC, AV1) et sa définition. Un appareil qui ne lit pas ce format le dit avant d'entrer (« Cet appareil ne lit pas l'image de cet ordinateur (HEVC). »). L'encodage à part vient avec l'accès à distance.
- Un invité lent peut ralentir l'image de tous (Kyber attend le plus lent) : sur le même réseau, c'est rare ; l'encodage à part le réglera.

## Échange avec Admin

Tout en JSON, sur le port d'Admin, certificat épinglé par empreinte comme aujourd'hui. Les noms ci-dessous font foi pour la Station, le cœur et le client web.

**Invité** (sans compte ; limité comme l'appairage : 10 essais par minute et par adresse, codes de 6 chiffres) :

- `POST /api/share/join` — `{"code":"123456","name":"Léa","device":"<identifiant de l'appareil>","app":"mac|windows|linux|ios|tvos|android|webos|web"}` → `200 {"guest":"<id>","secret":"<jeton de l'invité>","role":"spectator|player|player_kbm","host":"<prénom de l'hôte ou nom de l'ordinateur>","computer":"<nom>","kyber":{"port":12345,"fingerprint":"sha256:…","token":"<JWT>","expires":120},"display":{"id":2221634456,"width":1920,"height":1080,"refresh":60},"codec":"h264|hevc|av1"|null,"yuv444":false,"beat":5}`. Erreurs : `404 {"error":"no_share"}`, `409 {"error":"full"}`, `429` (trop d'essais). `codec` est le format de l'image en cours (absent si l'image n'a pas encore démarré) : l'invité vérifie qu'il sait le lire avant d'ouvrir l'image.
- `POST /api/share/beat` — en-tête `x-kybernow-guest: <secret>`, `{"guest":"<id>"}`, toutes les `beat` secondes → `{"role":"…","token":"<JWT renouvelé>","expires":120,"end":null|"stopped"|"removed"}`. Sans signe de vie pendant 20 s, l'invité sort.
- `POST /api/share/leave` — même en-tête, `{"guest":"<id>"}`.

**Hôte** (compte de l'appareil, comme `prepare` ; seulement l'appareil en session) :

- `POST /api/share/open` `{}` → `{"code":"123456","link":"kybernow://join?host=…&port=…&fp=…&code=123456","web":"https://<hôte>:<port>/link/?join=123456","display":{"id":…,"label":"Écran 1"}}` (son écran principal).
- `GET /api/share` → `{"open":true,"code":"…","link":"…","web":"…","display":{…},"guests":[{"id":"…","name":"Léa","app":"ios","role":"spectator","since":<horodatage>}]}` ; la même chose dans la réponse au signe de vie de l'hôte (`heartbeat`, champ `share`, absent sans partage).
- `POST /api/share/code` → nouveau code (même réponse que `open`).
- `POST /api/share/guest/{id}` `{"role":"spectator|player|player_kbm"}` ; `DELETE /api/share/guest/{id}` (retirer) ; `DELETE /api/share` (arrêter).

**Le PC lui-même** (routes locales de l'icône près de l'horloge, jeton de l'utilisateur du PC) : `GET /api/local/displays` → `[{"id":2221634456,"name":"\\\\.\\DISPLAY1","label":"Écran 1","width":3840,"height":2160,"primary":true}]` ; `POST /api/local/share/open {"display":<id>}` et les mêmes routes que l'hôte sous `/api/local/share…`. Admin (sa page d'accueil) les mêmes avec sa session.

**`info`** : `"share":{"available":true,"enabled":true}` (`enabled` faux si `multi_client` est coupé) ; absent pour une Station antérieure.

**Lien** : `kybernow://join?host=<adresse>&port=<port d'Admin>&fp=<empreinte d'Admin>&code=<6 chiffres>` ; navigateur : `https://<adresse>:<port>/link/?join=<6 chiffres>`.

**Kyber** : l'invité se connecte avec le JWT (connexion par jeton, comme le client web), sur l'écran donné, sans demander d'entrées s'il est spectateur. Le jeton porte le rôle et l'écran (revendications `kn_role`, `kn_display`), que Kyber impose (correctif 0019).

## Station

- **Les participants** : Admin passe d'une seule session à un hôte et des invités (rôle, prénom, appareil, dernier signe de vie). L'hôte reste ce qu'est la session aujourd'hui (préparation, écran virtuel, reprise de main par un autre appareil de l'hôte) ; un invité ne passe jamais par `prepare` ni par la reprise de main. Les gardes (redémarrage, mise à jour, fin de session, comptes en attente) voient les invités.
- **API** :
  - hôte : `POST /api/share/open` (écran, depuis sa session), `GET /api/share`, `POST /api/share/code` (nouveau code), `POST /api/share/guest/{id}` (rôle, clavier et souris), `DELETE /api/share/guest/{id}`, `DELETE /api/share` ;
  - PC local : les mêmes par les routes locales de l'icône (`/api/local/share…`, jeton de l'utilisateur du PC), plus la liste des écrans physiques ;
  - invité : `POST /api/share/join {code, nom}` (public, limité comme l'appairage), puis signe de vie `POST /api/share/beat` et `POST /api/share/leave`. La réponse donne un jeton d'accès Kyber à durée courte (renouvelé par le signe de vie), le port et l'empreinte de Kyber, l'écran à recevoir, le rôle, et pour le navigateur un cookie limité au relais et à `/api/share/*`.
  - `info` annonce le partage (`share`), pour que les apps sachent si la Station le connaît.
- **Kyber (correctif 0019)** : le rôle porté par le jeton (spectateur, joueur, joueur avec clavier et souris), appliqué à l'ouverture des entrées (aucune pour un spectateur ; seulement les manettes pour un joueur), changé en cours de partage sans couper l'image ; l'écran autorisé imposé (la liste des écrans filtrée, un autre refusé) ; une session de Kyber qu'Admin peut fermer tout de suite (retirer un invité) ; débit et images clés de l'image commune réservés à l'hôte.
- **Partage depuis le PC** : liste des écrans physiques (nom, cadre, identifiant de Kyber), un état « partage » qui empêche la création automatique d'un écran virtuel (mode « Pour toute connexion ») et le changement de la sortie son ; l'icône près de l'horloge et l'accueil d'Admin le lancent, le montrent et l'arrêtent.
- `multi_client` de Kyber doit être actif (c'est le réglage par défaut) : s'il est coupé, « Partager » le dit.

## Cœur Rust

- **Le rôle de la session** : hôte (comme aujourd'hui) ou invité (spectateur, joueur). Une session d'invité rejoint par `share_join` au lieu de `prepare`, n'envoie ni `foreground` ni changement de mode, d'écran ou de débit, ne redémarre jamais la Station, refuse l'empreinte de Kyber sans celle d'Admin, et part par `share_leave`. Ses entrées suivent ses droits (aucune chaîne d'entrées pour un spectateur ; manettes seulement pour un joueur, clavier et souris quand ils sont donnés), changés en cours de partage par le signe de vie.
- **Côté hôte** : `share_open`, `share_code`, `share_guest`, `share_kick`, `share_close` ; la liste des invités arrive avec le signe de vie de l'hôte (`Platform::on_guests`).
- **Rejoindre** : envoyer le code aux Stations trouvées sur le réseau (comme l'appairage), lire le lien `kybernow://join?host&port&fp&code`.
- **Le format** : avant d'entrer, l'invité vérifie qu'il sait lire le format de l'image de l'hôte.
- Pont C et JNI : les mêmes fonctions, pour toutes les apps.

## Apps et client web

Toutes les apps Link (Mac, Windows, Linux, iPhone, iPad, Apple TV, Android, téléviseurs LG) et le client web : « Rejoindre avec un code » sur l'accueil, la saisie du code (pavé à l'écran sur les téléviseurs, comme l'appairage), la session d'invité et sa barre réduite ; « Partager » et le panneau de l'hôte dans la barre de session. Un seul composant par famille d'apps quand c'est possible (KyberKit pour le Mac, l'iPhone, l'iPad et l'Apple TV). Les textes viennent de `link-ui.md`.

## Hors de ce premier pas

- Rejoindre hors du réseau (relais), l'encodage à part pour un invité à distance : étapes suivantes de KyberNow Share.
- Le prêt de l'ordinateur à tour de rôle (compte Windows dédié, lanceur) : plus tard.
- Plusieurs écrans partagés à la fois : le partage montre un écran.
