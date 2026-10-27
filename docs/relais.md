# Le relais de KyberNow : partager et se connecter hors de chez soi

[English](relais.en.md) · Français

Le relais fait passer KyberNow par internet quand l'appareil n'est pas sur le réseau de l'ordinateur. Il sert à deux choses :

- **Partager hors de chez soi** (KyberNow Share, [kybernow-share.md](kybernow-share.md)) : l'invité tape le même code à six chiffres, ouvre le lien ou scanne le QR code, où qu'il soit. L'hôte **accepte chaque invité** venu d'internet. Sur le même réseau, rien ne change : pas d'accord à donner.
- **Se connecter à son ordinateur hors de chez soi** : vos appareils appairés ouvrent l'ordinateur par le relais quand ils ne le trouvent pas sur le réseau. Réglage d'Admin « Accès hors de chez vous », **coupé au départ**.

Ce document est le contrat commun du relais, d'Admin, du cœur Rust, des apps Link et du client web : les noms qu'il donne sont définitifs. Les textes visibles sont dans [link-ui.md](link-ui.md) (section « Partage », « Hors de chez vous »).

## En bref

- Le relais public est `share.kybernow.app:8443`, chez l'auteur. **Un seul port, 8443, en TCP et en UDP.** Il est ouvert à toute Station, sans plafond de débit. Chacun peut installer le sien (image Docker, [server/relay/README.md](../server/relay/README.md)) ; la Station choisit son relais dans Admin.
- Le relais ne voit pas ce que les apps échangent avec la Station : elles parlent à Admin en TLS **de bout en bout** (certificat d'Admin épinglé), et à Kyber avec son empreinte. Il voit les adresses, les débits et les codes de partage. Le **navigateur** est l'exception : le relais présente un vrai certificat (`*.share.kybernow.app`) et déchiffre ce trafic pour le faire suivre.
- Kyber n'est pas modifié. Les apps n'ont pas à changer kyclient : le cœur ouvre un tunnel et donne à kyclient une adresse sur la boucle locale de l'appareil (`127.0.0.1:L`, en TCP et en UDP sur le même numéro).
- Coût : un aller-retour de plus par le relais (à peu près nul quand le relais est dans la même maison que l'ordinateur, 5 à 20 ms en France sinon), moins d'un demi-milliseconde de traitement, une trentaine d'octets par paquet.

## Les pièces

| Pièce | Où | Rôle |
|---|---|---|
| Serveur relais | `server/relay` (Linux, Docker) | Reçoit les Stations et les appareils, fait suivre les flux, réserve les codes, sert la page web et le navigateur |
| Tunnel | `server/relay/tunnel` (crate `kybernow-tunnel`) | Messages, trames, identifiant de Station, transports QUIC et TLS, flux UDP ; partagé par le relais, Admin et le cœur |
| Agent de la Station | Admin (`relay.rs`) | Tient la connexion au relais, sert dans son processus ce qui arrive par le relais, joint Kyber par l'adresse du réseau local |
| Tunnel des apps | cœur (`apps/core/src/relay.rs`) | Ouvre le tunnel vers une Station, expose Admin et Kyber sur la boucle locale |

## Le tunnel

### Connexions

Chaque bout (la Station, une app) ouvre vers le relais, sur le port 8443 :

| Connexion | Transport | ALPN | Contenu |
|---|---|---|---|
| Contrôle | QUIC (UDP) | `kn-ctl/1` | Les messages (premier flux bidirectionnel), puis un flux QUIC bidirectionnel par connexion TCP relayée |
| Image | QUIC (UDP) | `kn-media/1` | Les paquets UDP relayés, en datagrammes QUIC |
| Repli | TLS 1.3 sur TCP | `kn-tcp/1` | Tout à la fois, multiplexé par yamux, quand l'UDP ne passe pas |

- Nom présenté (SNI) : le nom du relais (`share.kybernow.app`). Le relais présente son certificat ; les apps et la Station le vérifient par les autorités habituelles du web (`webpki-roots`), ou par son empreinte quand l'adresse du relais en porte une (relais installé chez soi, certificat signé par lui-même) : `relais.exemple.org:8443#sha256:ab:cd:…` (SHA-256 du certificat, même forme que l'empreinte d'Admin ; le nom n'est alors pas vérifié).
- QUIC : MTU de départ 1280 octets, recherche jusqu'à 1472. Signe de vie toutes les 10 s pour la Station, 5 s pour une app ; coupure après 30 s sans nouvelles.
- `kn-ctl/1` garde le contrôle de congestion habituel (Cubic). `kn-media/1` a un contrôle **permissif** : fenêtre très grande, pas d'espacement, pertes ignorées ; la file d'envoi des datagrammes est bornée (les plus anciens sont jetés). Kyber a déjà son propre contrôle de débit à l'intérieur.
- Ordre : `kn-ctl/1` d'abord ; `kn-media/1` après la réponse `welcome`. Si QUIC ne répond pas en 3 s, le bout passe au repli `kn-tcp/1`, et retente QUIC toutes les 5 minutes. Le repli se signale (« Par le relais (TCP) ») : bon pour regarder, moins bon pour jouer.

### Qui est qui

- **Station** : une clé Ed25519 créée par Admin au premier usage (`<dossier de données>/relay/station.key`, dossier protégé comme le reste des données), et un certificat signé par elle-même, présenté comme **certificat client** sur les trois connexions. **Identifiant de la Station** : les 16 premiers octets du SHA-256 de sa clé publique (SPKI en DER), en base32 minuscule sans remplissage (alphabet RFC 4648 `a-z2-7`), soit 26 caractères, utilisable comme étiquette DNS (`<id>.share.kybernow.app`). Une réinstallation qui garde les données garde l'identifiant ; une réinstallation à neuf en donne un nouveau.
- **App** : pas de certificat. Elle tire un **jeton** (32 octets aléatoires, en hexadécimal) pour chaque tunnel et le garde d'une reconnexion à l'autre. **Identifiant du client** : les 10 premiers octets du SHA-256 de `kn-client/1` suivi du jeton, en base32 minuscule (16 caractères). Le relais le calcule et le donne à la Station : le même client garde le même identifiant après une reconnexion, et personne ne peut prendre celui d'un autre sans son jeton. Une nouvelle connexion avec le même identifiant remplace l'ancienne.
- **Navigateur** : le relais donne un identifiant tiré au hasard à chaque connexion TCP (16 caractères).

### Trames et messages

Une trame : longueur sur 4 octets (gros-boutiste, 65 536 au plus), puis un objet JSON en UTF-8 dont le champ `t` donne le type. Un type inconnu est ignoré, un champ inconnu aussi : on ajoute sans casser. `hello.v` donne la version du protocole (1) ; le relais répond avec la sienne dans `welcome.v`, ou `error {code: "version"}` et ferme.

Le bout qui se connecte ouvre le premier flux bidirectionnel de `kn-ctl/1` (ou le premier flux yamux) : c'est le flux des messages. `req` est un numéro choisi par celui qui demande, rendu dans la réponse.

| Message | Sens | Champs |
|---|---|---|
| `hello` | Station → relais | `v`, `role: "station"`, `version` (Admin), `admin_fp` (empreinte d'Admin), `sig` (signature de la Station sur `admin_fp`, ci-dessous), `codes` (codes à garder, après une reconnexion) |
| `hello` | app → relais | `v`, `role: "client"`, `station` (identifiant, ou absent pour une connexion qui ne sert qu'à `status`), `token` |
| `welcome` | relais → bout | `v`, `id` (identifiant de la Station, ou du client), `relay` (version du relais) ; pour une app : `online` (la Station est-elle là) ; pour la Station : `codes` (ceux de `hello` que le relais a gardés ; les autres sont à réserver de nouveau) |
| `error` | relais → bout | `req` (si c'est une réponse), `code` : `version`, `bad_request`, `unknown_station`, `too_many`, `banned`, `replaced`, `no_code`, `not_bound` ; `message` facultatif (journaux, jamais affiché tel quel) |
| `reserve` | Station → relais | `req`, `preferred` (code voulu, facultatif) |
| `reserved` | relais → Station | `req`, `code` (le code voulu s'il est libre, sinon un autre) |
| `release` | Station → relais | `code` |
| `open_flow` | app → relais | `req`, `key` (numéro de flux de l'app, stable tant que son point d'écoute vit) |
| `flow_open` | relais → Station | `flow`, `client`, `key`, `ip` (adresse publique du client) |
| `flow_accept` / `flow_refuse` | Station → relais | `flow` ; `reason` pour un refus |
| `flow_ready` / `flow_refused` | relais → app | `req`, `flow` ; `reason` pour un refus |
| `flow_close` | dans les deux sens | `flow` (le numéro de flux de celui qui reçoit) |
| `client_gone` | relais → Station | `client` (plus aucune connexion de ce client) |
| `station` | relais → app | `online` (la Station part ou revient) |
| `stats` | relais → app | `rtt_station_ms` (aller-retour relais ↔ Station), toutes les 5 s |
| `status` | app → relais | `req`, `stations` (identifiants, 16 au plus) |
| `presence` | relais → app | `req`, `online` (un booléen par identifiant) |
| `ping` / `pong` | dans les deux sens | `n` ; sert de signe de vie et de mesure sur le repli TCP |

**Signature** : `sig` est la signature Ed25519, par la clé de la Station, du texte `kybernow-relay/1 admin_fp ` suivi de `admin_fp`, en base64. La clé publique est celle du certificat client : le relais la redonne (`station_key`, SPKI en DER, base64) avec le code, et tout appareil qui connaît l'identifiant de la Station vérifie que la clé lui correspond et que la signature tient.

### Connexions TCP relayées

- L'app ouvre un flux bidirectionnel sur `kn-ctl/1` (après `welcome`), écrit une trame `{"t":"open","service":"admin_tls"|"kyber_tcp"}`, puis ses octets.
- Le relais ouvre un flux sur la connexion de la Station et écrit d'abord l'**en-tête**, une trame que seul le relais écrit (une app ne peut pas le forger) : `{"t":"stream","v":1,"kind":…,"client":"<id>","ip":"<adresse publique>","host":null|"<nom demandé>"}`. Puis il copie dans les deux sens ; fermer un côté le ferme de l'autre (demi-fermeture), une coupure coupe les deux.
- `kind` : `admin_tls` (une app vers Admin, TLS de bout en bout), `admin_http` (un navigateur, déchiffré par le relais, HTTP/1.1 en clair dans le tunnel, `host` = le nom demandé), `kyber_tcp` (kyclient vers Kyber : contrôle HTTPS, WebSocket).
- Une Station absente : le relais coupe le flux de l'app.

### Paquets UDP relayés

- Ouverture : l'app envoie `open_flow {req, key}` ; le relais passe `flow_open` à la Station ; Admin accepte (client autorisé, voir « Garde d'Admin ») ou refuse ; l'app reçoit `flow_ready {req, flow}` ou `flow_refused`. Chaque bout a ses propres numéros de flux ; le relais les traduit.
- Datagramme sur `kn-media/1` : `[numéro de flux, entier variable de QUIC][paquet UDP]`.
- **Règle de taille** : un paquet qui ne tient pas dans un datagramme (`max_datagram_size`) part, s'il fait 1200 octets au plus, dans un flux unidirectionnel de secours de `kn-media/1` (un paquet par flux, même forme) ; plus grand, il est jeté (la recherche de MTU de Kyber redescend d'elle-même).
- La connexion `kn-media/1` commence par un flux unidirectionnel ouvert par le bout qui se connecte, une trame `{"t":"media"}` (Station, reconnue à son certificat) ou `{"t":"media","token":"…"}` (app).
- Côté Station, l'agent tient pour chaque flux un socket UDP vers Kyber, gardé 25 s après une coupure : un client qui revient avec le même identifiant et la même `key` retrouve le même socket, et Kyber ne voit pas de nouvelle connexion.

### Repli TCP (`kn-tcp/1`)

TLS 1.3 sur TCP 8443, ALPN `kn-tcp/1`, certificat client pour la Station ; yamux par-dessus (le bout qui se connecte est le client yamux). Premier flux : les messages (comme `kn-ctl/1`). Deuxième flux, ouvert par le bout qui se connecte après `welcome` : les datagrammes, une trame `{"t":"media"…}` comme ci-dessus puis des enregistrements `[longueur de la suite, 2 octets gros-boutistes][numéro de flux][paquet UDP]`. Les autres flux : les connexions TCP relayées, comme sur QUIC. Une Station en repli et une app en QUIC (ou l'inverse) se parlent normalement.

### Reconnexion

- La Station n'est connectée au relais que si un partage est ouvert ou si « Accès hors de chez vous » est actif. Essais après 1, 2, 4, 8, 16 puis 30 s (±20 %). À chaque reconnexion : `hello` avec les codes en cours ; le relais garde les codes d'une Station partie 120 s.
- QUIC suit les changements d'adresse (Wi-Fi vers 4G) sans reconnexion.
- Une seule connexion par Station : la dernière gagne, l'ancienne reçoit `error {code: "replaced"}`.

## Le relais sur le web

Sur TCP 8443, le relais regarde le début de chaque connexion TLS :

- ALPN `kn-tcp/1` : le repli du tunnel ;
- nom `share.kybernow.app` : ses pages ;
- nom `<id>.share.kybernow.app` : le navigateur vers la Station `<id>` ;
- autre nom : connexion fermée.

| Adresse | Réponse |
|---|---|
| `GET /` | Accueil et saisie du code (français ou anglais selon le navigateur) |
| `GET /j/{code}` | Redirige (302) vers `https://<id>.share.kybernow.app:8443/link/?join=<code>` ; code inconnu, ou Station absente : une page qui le dit |
| `GET /v1/codes/{code}` | `200 {"station":"<id>","station_key":"<SPKI base64>","admin_fp":"sha256:…","sig":"<base64>","online":true}` ; `online:false` : Station partie depuis moins de 120 s, les autres champs vides ; `404 {"error":"unknown_code"}` ; `400 {"error":"bad_request"}` ; `429 {"error":"too_many","retry_after":<s>}` avec `Retry-After` |
| `GET /privacy` | Ce que le relais voit et garde (français ou anglais) |
| `GET /v1/health` | `200 {"ok":true,"version":"…"}` ; les apps iPhone et iPad l'appellent avant d'ouvrir le tunnel (réveil de la radio cellulaire) |
| `https://<id>.share…:8443/*` | Mandataire HTTP/1.1 vers Admin de la Station (WebSocket compris) ; Station absente : page « ordinateur injoignable » (503) |

Mandataire : `Host` passé tel quel, `X-Forwarded-For` et `Forwarded` retirés (l'adresse du navigateur voyage dans l'en-tête du flux), `Domain=` retiré de tout `Set-Cookie`. Une origine par Station : le code d'une Station ne lit jamais les cookies d'une autre.

Codes : le relais **réserve les codes**, uniques entre toutes les Stations qui lui sont connectées. Quatre codes au plus par Station. Les journaux du relais ne contiennent jamais un code.

## Admin

### Réglages

| Route | Contenu |
|---|---|
| `GET /api/relay` | `{"server":"share.kybernow.app:8443","remote_access":false,"id":"<identifiant>","state":"off"|"connecting"|"joined"|"unreachable","reason":null|"<texte>","transport":null|"quic"|"tcp","locked_until":null|<secondes Unix>,"sessions":[{"device":"<nom>","since":<secondes Unix>}]}` |
| `PUT /api/relay` | `{"server":"…","remote_access":true}` (chaque champ facultatif) → même réponse |

Session d'Admin seulement, jamais par le relais. `server` vide : pas de relais (ni partage hors de chez soi, ni accès). `state` : `off` (rien ne demande le relais : pas de partage ouvert et accès coupé), `connecting`, `joined`, `unreachable` avec `reason` (dans la langue d'Admin). Défaut : `share.kybernow.app:8443`, accès coupé. L'installateur garde le dossier `relay\` des données (même identifiant après une mise à jour ou une réinstallation qui garde les données).

### `info`

`GET /api/stream/info` ajoute `"relay":{"server":"share.kybernow.app:8443","id":"<identifiant>","remote_access":true}` quand un relais est réglé (absent sinon, et pour une Station antérieure). L'app le garde pour chaque ordinateur.

### Partage

- **Vue du partage** (`open`, `GET /api/share`, `code`, signe de vie de l'hôte, routes locales) :
  - `relay` : `null` (pas de relais) ou `{"state":"joined"|"unreachable"}` ;
  - relais joint : `link` porte en plus `&relay=<relais>&sid=<identifiant>`, et `web` devient `https://share.kybernow.app:8443/j/<code>` ; `web_local` garde toujours l'adresse sur le réseau (`https://<adresse>:<port>/link/?join=<code>`) ;
  - `guests[]` gagne `pending` (vrai pour une demande pas encore acceptée) et `via` (`"lan"` ou `"relay"`). Une demande est spectatrice, `since` est l'heure de la demande ; elle ne prend pas de place tant qu'elle n'est pas acceptée.
- **Codes** : relais joint, Admin ne tire plus le code lui-même : il demande `reserve` (« Nouveau code » : `reserve` puis `release` de l'ancien). Relais injoignable : code tiré par Admin, proposé au relais à la connexion (remplacé seulement s'il est pris). Le code vaut toujours sur le réseau, sans accord.
- **Rejoindre par le relais** : `POST /api/share/join` (même corps) → `202 {"pending":true,"guest":"<id>","secret":"<secret>","expires":120}`. Pas encore de prénom de l'hôte : il n'est donné qu'après l'accord. Refus en plus de ceux du partage : `409 {"error":"pending_full"}` (trois demandes en attente au plus), `403 {"error":"refused"}` (appareil refusé il y a moins de 10 minutes).
- **Attendre** : `POST /api/share/wait`, en-tête `x-kybernow-guest: <secret>`, `{"guest":"<id>"}`, réponse tenue 25 s au plus : `202 {"pending":true}` (toujours en attente, rappeler), `200` (la réponse de `join` sur le réseau, avec `"relay":true`), `403 {"error":"refused"}`, `410 {"error":"expired"}` (pas de réponse en 120 s), `404 {"error":"no_share"}` (partage arrêté). `POST /api/share/leave` annule une demande.
- **Accepter, refuser** : `POST /api/share/guest/{id}/accept`, `POST /api/share/guest/{id}/refuse` (hôte), et `/api/local/share/guest/{id}/accept|refuse` (le PC) → la vue du partage. Les places se comptent à l'acceptation : `409 {"error":"full"}` si les quatre sont prises.
- La réponse de `join` et de `wait` porte `"relay":true` par le relais, et `GET /api/session` aussi : le client web prend alors l'image en WebSocket (raison `relay`), jamais en WebTransport.
- Cookie du navigateur par le relais : `__Host-kybernow_guest` (`Secure`, `Path=/`, `SameSite=Strict`, `HttpOnly`, sans `Domain`) ; Admin lit les deux noms.

### Fil

« {appareil} est en session hors de chez vous » au début d'une session par le relais ; « Accès hors de chez vous bloqué 10 min après trop d'essais. » au blocage ([link-ui.md](link-ui.md)).

## Garde d'Admin selon la voie

Admin juge « local » ce qui vient de la boucle locale (`/api/local/*`, `local_or_admin`). Le relais ne doit jamais passer pour local :

- L'agent **sert lui-même** ce qui arrive par le relais, dans le processus d'Admin (`admin_tls` : TLS sur le certificat d'Admin, puis HTTP ; `admin_http` : HTTP), jamais en se reconnectant en TCP sur `127.0.0.1`. L'adresse du pair est celle que donne l'en-tête du flux, **forcée hors de la boucle locale et des adresses privées**, et la requête porte la voie (`Via::Relay {kind, client}`).
- Un `Router` à part, `relay_guard`, n'ouvre que :
  - à un **invité**, quand un partage est ouvert : `/api/share/join|wait|beat|quality|leave`, `/link/*`, `/assets/*`, `GET /api/session` réduit, le mandataire d'Admin vers Kyber (`kyber_proxy`) avec le cookie d'un invité accepté (WebSocket seulement, jamais `/kymux/start_mux`) ;
  - à un **appareil appairé** (compte Basic), seulement quand « Accès hors de chez vous » est actif : `/api/stream/*` et les routes de l'hôte `/api/share*` ; accès coupé : `403 {"error":…,"code":"remote_access_off"}` ;
  - **jamais** par le relais : `/api/login`, `/api/setup`, `/api/password`, `/api/web/ticket`, `/api/pair*`, `/api/pairing`, `/api/local/*`, la configuration, les comptes, les journaux, le service, l'écran, le son, la mise à jour d'Admin, les appareils, les apps, `/api/events`, l'interface d'Admin. L'appairage se fait sur le réseau seulement.
- **Blocage** : 20 échecs du compte Basic par le relais en 10 minutes bloquent l'accès par le relais 10 minutes (`429 {"code":"locked"}`), avec une ligne dans le fil.
- **Kyber** : l'agent ne joint Kyber que pour un client autorisé (un invité accepté, tant qu'il donne signe de vie ; un appareil appairé après `prepare`, pour 30 s renouvelées par le signe de vie), et toujours **depuis et vers l'adresse du réseau local** de l'ordinateur, jamais par la boucle locale (le correctif 0019 réserve `/kybernow/*` à la boucle locale). Kyber écoute sur toutes les adresses.
- Admin autorise un **client du relais** (son identifiant) : celui qui a fait le `wait`, le `beat` ou le `prepare`/`heartbeat` réussi. Un client qui se reconnecte garde son identifiant.

## Liens

- Apps : `kybernow://join?host=<adresse>&port=<port d'Admin>&fp=<empreinte d'Admin>&code=<6 chiffres>&relay=<relais>&sid=<identifiant>` (avec `relay` et `sid` quand le relais est joint ; le relais garde son `#sha256:…` éventuel, encodé). L'app essaie d'abord le réseau (`host`, `port`), puis le relais vers `sid`, Admin épinglé sur `fp`.
- Navigateur : `https://share.kybernow.app:8443/j/<code>`, qui redirige vers `https://<id>.share.kybernow.app:8443/link/?join=<code>`. Pas de lien universel sur iPhone (il faudrait le port 443) : la page propose « Ouvrir dans l'app ».
- QR code : le lien des apps.

## Rejoindre avec un code, hors de chez soi

1. L'app envoie le code aux Stations du réseau (comme aujourd'hui) **et en même temps** demande `GET /v1/codes/{code}` au relais par défaut et aux relais des ordinateurs qu'elle connaît (`info.relay.server`).
2. Une Station du réseau répond en 1,5 s : l'invité entre comme aujourd'hui. Sinon, si le relais connaît le code : tunnel vers `station`, `POST /api/share/join` de bout en bout, Admin épinglé sur `admin_fp` (et `sig` vérifiée quand l'app connaît `sid`).
3. « En attente de l'accord de l'hôte… » : `POST /api/share/wait` jusqu'à `200`, `403` ou `410`. « Annuler » : `leave`.
4. Session d'invité ordinaire, par le relais : Admin par le tunnel, kyclient sur `127.0.0.1:L` (le `kyber.port` de la réponse remplacé par L).

## Se connecter à son ordinateur, hors de chez soi

- `Route::Auto` : l'app demande `info` en direct (1,5 s) ; sur un échec réseau, elle passe par le relais (`info.relay` gardé). **Empreinte changée : jamais de bascule** vers le relais (l'erreur d'empreinte reste).
- Session d'hôte par le relais : `prepare`, `heartbeat`, `end`, `foreground` par Admin dans le tunnel ; kyclient sur `127.0.0.1:L` pour le `kyber_port` de `prepare`. Par le relais, les empreintes d'Admin et de Kyber sont obligatoires : jamais de premier contact.
- Présence sur l'accueil : `status` au relais ; un ordinateur joignable seulement par le relais est « Hors de chez vous ».
- En session, on reste sur le relais jusqu'à la fin.

## Cœur et apps

- `apps/core/src/relay.rs` (runtime tokio à lui, deux fils) : `Tunnel::open(RelayRoute {server, station, admin_fp})`, `admin_addr()`, `kyber_addr(port)`, `stats()` ; un tunnel par (relais, Station), fermé après 30 s sans usage ; `lookup(code)`.
- `SessionParams::route` : `Route::Direct`, `Route::Relay(RelayRoute)`, `Route::Auto(Option<RelayRoute>)` ; `Console::with_relay(Option<RelayRoute>)`.
- Indicateur : `Platform::on_route(&RouteInfo {transport: direct|relay|relay_tcp, relay, rtt_client, rtt_station})` ; puce « Par le relais », ligne « Chemin » des statistiques, `route` dans « Dernière session ».
- `KYBERNOW_RELAY=force` (toujours par le relais, pour essayer à la maison) ou `off`.
- Points d'écoute : toujours sur `127.0.0.1`, jamais `0.0.0.0` ; TCP lié sur le port 0, puis UDP sur le même numéro (on recommence s'il est pris). iPhone, iPad, Apple TV : pas d'autorisation « Réseau local » pour la boucle locale ; les sockets se recréent au retour au premier plan (port changé : la session repart).
- Ponts : `kn_relay_*`, `kn_share_wait` ; JNI `Native.relay*`, `Native.shareWait`.

## Confiance

- **Le relais** est vérifié par son certificat (autorités du web, ou empreinte épinglée).
- **Code tapé** : c'est le relais qui donne l'empreinte d'Admin. Un relais menteur pourrait s'interposer ; c'est le prix du code à six chiffres. L'hôte voit et accepte chaque invité.
- **Lien et QR code** : `fp` et `sid` viennent de la Station ; l'échange est de bout en bout, le relais ne peut pas s'interposer.
- **Appareils appairés** : empreintes épinglées à l'appairage (sur le réseau). Par le relais, jamais de premier contact.
- **Navigateur** : le relais déchiffre (vrai certificat `*.share.kybernow.app`). Les apps, elles, restent chiffrées de bout en bout.
- Le relais public est en France ; il garde les adresses IP dans ses journaux quelques jours (journaux tournés par taille), jamais les codes. Page de confidentialité sur le relais (`/privacy`).

## Limites et protections du relais

- Recherche de code : 10 par minute et 60 par heure par adresse, 30 par minute par /24 (IPv4) ou /48 (IPv6), 600 par minute en tout. Admin garde ses 10 essais de `join` par minute et par adresse réelle.
- QUIC : 16 connexions par adresse, 60 négociations par minute par adresse, 4096 en tout ; au-delà d'un seuil, le relais demande une preuve d'adresse (Retry).
- TCP : 32 connexions par adresse ; TLS et en-têtes HTTP en 10 s, 16 Kio d'en-têtes au plus.
- 4 Stations par adresse, une connexion par Station (la dernière gagne), 8 clients par Station, 4 flux UDP par client.
- Pas de plafond de débit. Le relais ne fait suivre que vers des Stations connectées, et Admin ne laisse passer que des clients autorisés. Liste de bannissement relue à chaud ; plafond global facultatif, coupé par défaut ; métriques Prometheus et alerte de débit.

## Installer un relais chez soi

Une machine Linux avec Docker, un nom de domaine dont la zone DNS accepte un jeton d'API (Gandi par défaut, pour le certificat générique), le port 8443 ouvert en TCP **et** UDP. Le pas à pas est dans [server/relay/README.md](../server/relay/README.md). Dans Admin de chaque Station : « Hors de chez vous › Relais » = `relais.exemple.org:8443`.

## Plus tard

- Image du navigateur en WebTransport par le relais (sauf Safari) : le relais lirait le nom dans le premier paquet QUIC et ferait suivre sans déchiffrer.
- Partage équitable du lien montant de la maison entre plusieurs invités.
