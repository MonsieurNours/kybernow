# KyberNow Share : le partage en local

[English](kybernow-share.en.md) · Français

KyberNow Share fait entrer quelqu'un d'autre sur votre ordinateur : un **spectateur**, qui voit et entend, ou un **joueur**, dont les manettes deviennent la manette suivante de Windows, et qui peut aussi recevoir le clavier et la souris. Chaque invité a une image à sa mesure, sans jamais gêner celle de l'hôte. Ce premier pas vaut **sur le même réseau** ; le relais (hors de chez soi) viendra ensuite ([ROADMAP.md](../ROADMAP.md)). Ce document est le contrat commun de la Station, du cœur Rust, de toutes les apps Link et du client web.

Le partage n'a pas d'écrans à lui : il reprend l'accueil, la connexion, la barre de session et les réglages de [link-ui.md](link-ui.md), qui donne les textes exacts (section « Partage »). Toutes les apps se comportent pareil et emploient les mêmes mots.

## Ce que voit l'utilisateur

### Partager

On partage de deux façons :

- **Depuis sa session** (l'hôte) : « Partager » dans la barre de session. Le partage montre son écran principal, et le son de l'ordinateur.
- **Depuis le PC lui-même**, sans session : « Partager un écran… » dans le menu de l'icône de KyberNow Station près de l'horloge, ou sur la page d'accueil d'Admin. On choisit l'écran à partager parmi les écrans du PC ; on continue d'utiliser son PC normalement, les invités voient cet écran et entendent le son du PC. Aucun écran virtuel n'est créé, les écrans du PC ne s'éteignent pas.

Partager ouvre un panneau : un **code** à six chiffres (« 123 456 »), un **lien** et son **QR code** (`kybernow://join?…`, ou l'adresse du client web), et la liste des invités. Le code vaut tant que le partage est ouvert ; « Nouveau code » le change (les invités déjà là restent). « Arrêter le partage » fait sortir tout le monde. Fermer la session de l'hôte arrête aussi le partage.

### Les invités

- Un invité **arrive en spectateur** : il voit et entend, n'envoie rien.
- Dans la liste, l'hôte donne ou reprend à chaque invité **deux droits indépendants** : **« Donner les manettes »** (ses manettes deviennent la manette suivante de Windows, une manette virtuelle à lui) et **« Donner le clavier et la souris »** ; « Reprendre les manettes », « Reprendre le clavier et la souris » les lui retirent, l'un sans toucher à l'autre. « Retirer » le fait sortir du partage ; il peut revenir avec le même code, « Nouveau code » l'en empêche (les invités déjà là restent).
- **Quatre invités au plus.** Au-delà, le code est refusé avec la raison.
- La Station impose ces droits : un spectateur n'a aucune entrée sur l'ordinateur, un invité seulement ce que l'hôte lui a donné (ses manettes, le clavier et la souris, ou les deux ; jamais le presse-papiers), même si son appareil essayait d'envoyer autre chose.
- Admin montre le partage (écran, invités, rôle de chacun), peut retirer un invité et arrêter le partage ; son fil le raconte (« Léa regarde votre écran », « Léa joue avec vous »).

### Rejoindre

- **« Rejoindre avec un code »** sur l'accueil de toutes les apps Link (à côté de « Ajouter un ordinateur ») : on tape les six chiffres ; l'app les envoie aux ordinateurs qu'elle trouve sur le réseau, celui qui partage répond. Ou on ouvre le lien, ou on scanne le QR code (apps qui ont un appareil photo). Dans le **navigateur**, le lien ouvre le client web en invité, sans mot de passe d'Admin.
- L'invité donne son prénom (proposé : le nom de l'appareil), retenu pour la fois suivante.
- La session d'invité est une session ordinaire à l'écran, avec une barre réduite : « Spectateur » ou « Joueur », « Quitter le partage », et les commandes que son rôle permet (manette, clavier et souris s'ils lui sont donnés). Pas de profil, pas de réglages de l'ordinateur, pas d'écran virtuel : un invité ne change jamais les écrans de l'ordinateur et ne prend jamais la main à l'hôte.
- Un invité n'est **pas ajouté à la liste des ordinateurs** de l'appareil : rien n'est gardé après le partage, sauf son prénom.
- Fin : « {hôte} a arrêté le partage. », « {hôte} vous a retiré du partage. » ou la raison d'un échec ; jamais « Reprendre la main ».

### L'image d'un invité

- **L'hôte passe toujours en premier.** Rien de ce que fait un invité ne change l'écran de l'ordinateur ni l'image de l'hôte, et l'image d'un invité n'est jamais au-dessus de celle de l'hôte (définition, fréquence, débit, 4:4:4) : un invité qui demande du 5K avec un hôte en 4K reçoit du 4K.
- L'invité choisit sa **« Qualité de l'image »** à l'écran « Rejoindre » et dans sa barre : « Auto » (proposé : d'après l'écran de l'appareil et ce que son décodeur suit), « Comme l'hôte », « 1080p · 60 Hz », « 720p · 30 Hz ». La changer relance son image seulement.
- Un invité qui suit l'image de l'hôte la **partage** (rien de plus pour la carte graphique). Les autres reçoivent une **image à part**, encodée pour eux depuis la même capture ; les invités qui demandent la même qualité partagent la leur. **Deux images à part au plus** en plus de celle de l'hôte (une sur une carte Intel) ; au-delà, l'invité reçoit l'image ouverte la plus proche, plus légère, et le sait (« L'ordinateur ne peut pas faire une image de plus : vous recevez {qualité}. »).
- Kyber **n'attend plus un invité en retard** : sur l'image de l'hôte, il passe seul sur une image plus légère (« Image allégée pour suivre. ») ; l'hôte ne voit rien. Sur son image à part, un invité peut redemander une image clé après une erreur ; sur l'image de l'hôte, ses demandes sont espacées.
- L'hôte voit la qualité de chaque invité dans son panneau (« 1080p · 60 Hz »), sans la changer.
- Un appareil qui ne lit pas le format de l'image qu'il recevrait le dit avant d'entrer (« Cet appareil ne lit pas l'image de cet ordinateur (HEVC). »).

## Échange avec Admin

Tout en JSON, sur le port d'Admin, certificat épinglé par empreinte comme aujourd'hui. Les noms ci-dessous font foi pour la Station, le cœur et le client web.

**Invité** (sans compte ; limité comme l'appairage : 10 essais par minute et par adresse, codes de 6 chiffres) :

- `POST /api/share/join` — `{"code":"123456","name":"Léa","device":"<identifiant de l'appareil>","app":"mac|windows|linux|ios|tvos|android|webos|web"}` → `200 {"guest":"<id>","secret":"<jeton de l'invité>","role":"spectator|player|kbm|player_kbm","host":"<prénom de l'hôte ou nom de l'ordinateur>","computer":"<nom>","kyber":{"port":12345,"fingerprint":"sha256:…","token":"<JWT>","expires":120},"display":{"id":2221634456,"width":1920,"height":1080,"refresh":60},"codec":"h264|hevc|av1"|null,"yuv444":false,"beat":5}`. Erreurs : `404 {"error":"no_share"}`, `409 {"error":"full"}`, `429` (trop d'essais). `codec` est le format de l'image en cours (absent si l'image n'a pas encore démarré) : l'invité vérifie qu'il sait le lire avant d'ouvrir l'image.
- `POST /api/share/beat` — en-tête `x-kybernow-guest: <secret>`, `{"guest":"<id>"}`, toutes les `beat` secondes → `{"role":"…","token":"<JWT renouvelé>","expires":120,"end":null|"stopped"|"removed"}`. Sans signe de vie pendant 20 s, l'invité sort.
- `POST /api/share/leave` — même en-tête, `{"guest":"<id>"}`.

**Hôte** (compte de l'appareil, comme `prepare` ; seulement l'appareil en session) :

- `POST /api/share/open` `{}` → `{"code":"123456","link":"kybernow://join?host=…&port=…&fp=…&code=123456","web":"https://<hôte>:<port>/link/?join=123456","display":{"id":…,"label":"Écran 1"}}` (son écran principal).
- `GET /api/share` → `{"open":true,"code":"…","link":"…","web":"…","display":{…},"guests":[{"id":"…","name":"Léa","app":"ios","role":"spectator","since":<secondes Unix>}]}` ; la même chose dans la réponse au signe de vie de l'hôte (`heartbeat`, champ `share`, absent sans partage).
- `POST /api/share/code` → nouveau code (même réponse que `open`).
- `POST /api/share/guest/{id}` `{"role":"spectator|player|kbm|player_kbm"}` (`player` : les manettes ; `kbm` : le clavier et la souris ; `player_kbm` : les deux) ; `DELETE /api/share/guest/{id}` (retirer) ; `DELETE /api/share` (arrêter).

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
- **Précisions de la Station** (ce qu'elle fait en plus de « Échange avec Admin », sans rien y changer) :
  - réponses : les routes de l'hôte et du PC rendent toutes la même vue du partage, qui contient celle du contrat (`code`, `link`, `web`, `display`) et ajoute `open`, `origin` (`host` ou `local`), `host`, `guests` (avec `device` et `last_beat`), `max_guests`, `opened_at`, et dans `display` `index`, `width`, `height`, `refresh` ; `GET /api/local/displays` ajoute `index`, `refresh`, `x`, `y`, et ne garde que les écrans que Kyber capture quand il répond ;
  - routes de l'hôte : `device_id` facultatif (corps, ou requête pour `GET` et `DELETE`), comme le signe de vie, pour distinguer deux appareils sur le même compte ; refus `{"error": <texte>, "code": …}` avec `not_in_session`, `disabled`, `no_share`, `unknown_guest` ;
  - invité : refus en plus de ceux du contrat, `400 {"error":"bad_request"}` (code ou prénom invalide), `503 {"error":"unavailable"}` (Kyber sans clé de jetons) ; `429` porte `{"error":"too_many","retry_after":<s>}` et `Retry-After` ; `beat` et `leave` d'un invité inconnu (parti, sorti sans nouvelles, ou plus d'une minute après sa sortie) : `404 {"error":"unknown_guest"}` ;
  - `beat` donne aussi `display` (`id`, `width`, `height`, `refresh`) : l'écran du partage d'un hôte suit son écran principal quand il se prépare de nouveau (écran virtuel recréé sous un autre identifiant) ; le jeton renouvelé porte alors le nouveau `kn_display`, que Kyber ne prend qu'à une nouvelle connexion (pas au renouvellement) ;
  - le même appareil (`device`) qui rejoint de nouveau reprend sa place (même `guest`, même rôle, nouveau `secret`), sans compter pour un invité de plus ;
  - jeton de Kyber d'un invité : `sub` = `kn-guest-<guest>`, et `kn_rev` (croissant à chaque changement de rôle) en plus de `kn_role` et `kn_display` ;
  - partage depuis le PC : refusé pendant la session d'un appareil (`code` `session`), pendant qu'un appareil se prépare ou qu'une autre connexion a pris les écrans ou le son (`busy`) ; une session qui commence (`prepare` d'un appareil) l'arrête, même si elle s'est croisée avec l'ouverture (revu une fois le partage ouvert, puis chaque seconde) ; un seul partage à la fois ;
  - Kyber (0019) : un spectateur peut ouvrir ses entrées, Kyber n'en laisse rien passer vers l'ordinateur ; « Faire jouer » s'applique alors sans relancer l'image. Un invité qui relance son image coûte une image clé à l'encodeur commun (celle que Kyber force pour tout client qui arrive), sans changer sa configuration ; son `/force_idr` est sans effet. Chaque invité a son propre serveur d'entrées, donc ses propres manettes virtuelles (huit annoncées au plus sont retenues) ; un invité tient deux sessions de Kyber au plus (un troisième login : `409`), et six WebSockets au plus par le relais d'Admin ;
  - les gardes nomment les invités (« Un partage est en cours avec Léa et Max : le redémarrage l'arrêtera ») ; avec `force`, le partage s'arrête avant (les invités lisent `stopped`).

## Cœur Rust

Détail et noms exacts : [apps/core/README.md](../apps/core/README.md#kybernow-share-srcsharers-srcsessionrs-srcinputpipeliners) ; pont C : [coeur-apple.md](coeur-apple.md).

- **Le rôle de la session** (`SessionParams::role`) : `SessionRole::Host` (comme aujourd'hui) ou `SessionRole::Guest` (la réponse de `share_join` et ce que l'appareil décode ; `SessionParams::guest`, `SessionParams::from_json` pour les ponts). Une session d'invité rejoint par `share_join` au lieu de `prepare`, se connecte à Kyber avec le jeton (renouvelé par le signe de vie), reçoit l'écran donné par la réponse (et s'y reconnecte quand le signe de vie en donne un autre), n'envoie ni `heartbeat` d'hôte ni `foreground`, ni changement de mode, d'écran, de profil ou de débit, ni image clé, ne redémarre jamais la Station, refuse un Kyber sans l'empreinte qu'Admin donne (jamais de premier contact), et part par `share_leave` (jamais `end`, pas de rapport d'écrans rendus). Ses entrées suivent ses deux droits indépendants (`ShareRole` : `spectator`, `player`, `kbm`, `player_kbm` ; `InputRights` : aucune chaîne d'entrées pour un spectateur ; les manettes, le clavier et la souris, ou les deux, chacun donné ou repris sans toucher à l'autre ; jamais le presse-papiers), changés en cours de partage par le signe de vie (`Platform::on_share_role`). Fin par le partage (`end: stopped|removed`) : `Platform::on_share_end`, avec le texte de [link-ui.md](link-ui.md).
- **Côté hôte** : `Console::share_open`, `share_state`, `share_code`, `share_guest`, `share_kick`, `share_close` ; la liste des invités arrive avec le signe de vie de l'hôte (`Platform::on_guests`) ; `Info::share` et `ShareAvailability` pour « Partager ».
- **Rejoindre** : `share::join` envoie le code aux Stations trouvées sur le réseau (comme l'appairage) et rend la première qui l'accepte ; `share::parse_link` lit le lien `kybernow://join?host&port&fp&code` (et celui du client web).
- **Le format** : avant d'entrer, l'invité vérifie qu'il sait lire le format de l'image de l'hôte (`share::check_codec`, `share::join_checked`, et la session elle-même).
- Pont C et JNI : les mêmes fonctions, pour toutes les apps (`kn_share_*`, `kn_console_share_*` ; `Native.share*`, `Native.consoleShare*`).

## Apps et client web

Toutes les apps Link (Mac, Windows, Linux, iPhone, iPad, Apple TV, Android, téléviseurs LG) et le client web : « Rejoindre avec un code » sur l'accueil, la saisie du code (pavé à l'écran sur les téléviseurs, comme l'appairage), la session d'invité et sa barre réduite ; « Partager » et le panneau de l'hôte dans la barre de session. Un seul composant par famille d'apps quand c'est possible (KyberKit pour le Mac, l'iPhone, l'iPad et l'Apple TV). Les textes viennent de `link-ui.md`.

**Client web** (`server/web/src/link/`) : même session que celle de l'hôte (même page, `Session.svelte`), avec une barre réduite pour l'invité. Il n'a pas de cœur Rust : il fait lui-même l'échange avec Admin ci-dessus (`app: "web"`, `device` : l'identifiant de ce navigateur) et garde le cookie `kybernow_guest`, qui ouvre seulement le relais vers Kyber et `/api/share/*` ; le moteur de Kyber ne se charge donc qu'après `join`, et le format de l'image se vérifie alors (sinon `leave`). Le moteur ouvre ses entrées dès le départ et la page filtre selon le rôle ; Kyber refuse de toute façon ce que le rôle ne permet pas. L'hôte, dans le navigateur : « Partager » dans la barre, panneau d'Admin sur les routes de l'hôte, invités lus dans la réponse du signe de vie (`share`).

## Hors de ce premier pas

- Rejoindre hors du réseau (relais) : étape suivante de KyberNow Share.
- Le prêt de l'ordinateur à tour de rôle (compte Windows dédié, lanceur) : plus tard.
- Plusieurs écrans partagés à la fois : le partage montre un écran.
