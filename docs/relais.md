# Le relais de KyberNow : partager et se connecter hors de chez soi

[English](relais.en.md) · Français

Le relais fait passer KyberNow par internet quand l'appareil n'est pas sur le réseau de l'ordinateur. Il sert à deux choses :

- **Partager hors de chez soi** (KyberNow Share, [kybernow-share.md](kybernow-share.md)) : l'invité tape le même code à six chiffres, ouvre le lien ou scanne le QR code, où qu'il soit. L'hôte **accepte chaque invité** venu d'internet. Sur le même réseau, rien ne change : pas d'accord à donner.
- **Se connecter à son ordinateur hors de chez soi** : vos appareils appairés ouvrent l'ordinateur par le relais quand ils ne le trouvent pas sur le réseau. Réglage d'Admin « Accès hors de chez vous », **coupé au départ**.

Ce document est le contrat commun du relais, d'Admin, du cœur Rust, des apps Link et du client web : les noms qu'il donne sont définitifs. Les textes visibles sont dans [link-ui.md](link-ui.md) (section « Partage », « Hors de chez vous »).

## En bref

- Le relais public est `share.kybernow.app`, chez l'auteur. **Un seul port public, 443, en TCP et en UDP** (la box le renvoie vers le 8443 de la machine). Une adresse de relais sans port, c'est le 443, et les liens n'écrivent jamais `:443` ; un relais installé ailleurs peut porter son port (`relais.exemple.org:9443`). Il est ouvert à toute Station, sans plafond de débit. Chacun peut installer le sien (image Docker, [server/relay/README.md](../server/relay/README.md)) ; la Station choisit son relais dans Admin.
- Le relais ne voit pas ce que les apps échangent avec la Station : elles parlent à Admin en TLS **de bout en bout** (certificat d'Admin épinglé), et à Kyber avec son empreinte. Il voit les adresses, les débits et les codes de partage. Le **navigateur** aussi, depuis la 0.11.8 : il ouvre la Station par `https://<id>.station.share.kybernow.app`, dont le certificat et la clé sont sur la Station, et le relais fait passer le TLS sans le déchiffrer (« Navigateur chiffré de bout en bout », plus bas). Avec une Station ou un relais plus anciens, le navigateur passe par `https://<id>.share.kybernow.app`, que le relais déchiffre pour le faire suivre.
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

Chaque bout (la Station, une app) ouvre vers le relais, sur son port (443 pour le relais public) :

| Connexion | Transport | ALPN | Contenu |
|---|---|---|---|
| Contrôle | QUIC (UDP) | `kn-ctl/1` | Les messages (premier flux bidirectionnel), puis un flux QUIC bidirectionnel par connexion TCP relayée |
| Image | QUIC (UDP) | `kn-media/1` | Les paquets UDP relayés, en datagrammes QUIC |
| Repli | TLS 1.3 sur TCP | `kn-tcp/1` | Tout à la fois, multiplexé par yamux, quand l'UDP ne passe pas |

- Nom présenté (SNI) : le nom du relais (`share.kybernow.app`). Le relais présente son certificat ; les apps et la Station le vérifient par les autorités habituelles du web (`webpki-roots`), ou par son empreinte quand l'adresse du relais en porte une (relais installé chez soi, certificat signé par lui-même) : `relais.exemple.org:9443#sha256:ab:cd:…` (SHA-256 du certificat, même forme que l'empreinte d'Admin ; le nom n'est alors pas vérifié).
- QUIC : MTU de départ 1280 octets, recherche jusqu'à 1472. Signe de vie toutes les 10 s pour la Station, 5 s pour une app ; coupure après 30 s sans nouvelles.
- `kn-ctl/1` garde le contrôle de congestion habituel (Cubic). `kn-media/1` a un contrôle **permissif** : fenêtre très grande, pas d'espacement, pertes ignorées ; la file d'envoi des datagrammes est bornée (les plus anciens sont jetés). Le débit se règle à l'intérieur, de bout en bout : la connexion QUIC de Kyber, et le débit automatique de l'app ([debit-automatique.md](debit-automatique.md)), qui voit l'aller-retour et les pertes à travers le tunnel.
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
| `hello` | Station → relais | `v`, `role: "station"`, `version` (Admin), `admin_fp` (empreinte d'Admin), `sig` (signature de la Station sur `admin_fp`, ci-dessous), `codes` (codes à garder, après une reconnexion) ; depuis la 0.11.8 : `browser_tls` (vrai : la Station sert les flux `browser_tls`) et `tls_ready` (vrai : elle tient un certificat valable pour `<id>.<tls_zone>`) |
| `hello` | app → relais | `v`, `role: "client"`, `station` (identifiant, ou absent pour une connexion qui ne sert qu'à `status`), `token` |
| `welcome` | relais → bout | `v`, `id` (identifiant de la Station, ou du client), `relay` (version du relais) ; pour une app : `online` (la Station est-elle là) ; pour la Station : `codes` (ceux de `hello` que le relais a gardés ; les autres sont à réserver de nouveau) |
| `error` | relais → bout | `req` (si c'est une réponse), `code` : `version`, `bad_request`, `unknown_station`, `too_many`, `banned`, `replaced`, `no_code`, `not_bound`, `no_accounts`, `enroll_refused` ; `message` facultatif (journaux, jamais affiché tel quel) |
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
| `attach_request` / `attach_code` | Station → relais / relais → Station | Comptes KyberNow : code de rattachement (docs/comptes.md, « Rattachement et appareils sur le relais ») |
| `attach_status`, `detach` | Station → relais | Où en est le rattachement ; détachée dans Admin |
| `tls_zone` | relais → Station | `zone` (`station.share.kybernow.app`), juste après `welcome` : le relais fait passer le TLS des noms `<id>.<zone>` jusqu'à la Station (0.11.8 ; jamais envoyé par un relais d'avant, ni passage coupé) |
| `admin_status` | Station → relais | `admin_lan`, `remote_admin` (docs/comptes.md, « Admin depuis KyberNow Share ») et, depuis la 0.11.8, `tls_ready` ; à chaque changement |
| `attached` / `detached` | relais → Station | Rattachée (relais, compte, propriétaire, clés) ; détachée dans la console |
| `enroll` | Station → relais | Comptes KyberNow, lot 5 : enrôlement (installation silencieuse) par un jeton d'organisation ; réponse `attached` (avec `req`) ou `error` `enroll_refused` (raison dans `message`) / `no_accounts` (docs/comptes.md, « Personnes, droits et organisations » et « Installateurs ») |

**Signature** : `sig` est la signature Ed25519, par la clé de la Station, du texte `kybernow-relay/1 admin_fp ` suivi de `admin_fp`, en base64. La clé publique est celle du certificat client : le relais la redonne (`station_key`, SPKI en DER, base64) avec le code, et tout appareil qui connaît l'identifiant de la Station vérifie que la clé lui correspond et que la signature tient.

### Connexions TCP relayées

- L'app ouvre un flux bidirectionnel sur `kn-ctl/1` (après `welcome`), écrit une trame `{"t":"open","service":"admin_tls"|"kyber_tcp"}`, puis ses octets.
- Le relais ouvre un flux sur la connexion de la Station et écrit d'abord l'**en-tête**, une trame que seul le relais écrit (une app ne peut pas le forger) : `{"t":"stream","v":1,"kind":…,"client":"<id>","ip":"<adresse publique>","host":null|"<nom demandé>"}`. Puis il copie dans les deux sens ; fermer un côté le ferme de l'autre (demi-fermeture), une coupure coupe les deux.
- `kind` : `admin_tls` (une app vers Admin, TLS de bout en bout), `browser_tls` (un navigateur, ou l'autorité de certification, vers Admin : le TLS tel quel depuis le premier octet, `host` = le nom présenté ; seulement vers une Station qui l'a annoncé, « Navigateur chiffré de bout en bout »), `admin_http` (un navigateur, déchiffré par le relais, HTTP/1.1 en clair dans le tunnel, `host` = le nom demandé), `kyber_tcp` (kyclient vers Kyber : contrôle HTTPS, WebSocket).
- Une Station absente : le relais coupe le flux de l'app.

### Paquets UDP relayés

- Ouverture : l'app envoie `open_flow {req, key}` ; le relais passe `flow_open` à la Station ; Admin accepte (client autorisé, voir « Garde d'Admin ») ou refuse ; l'app reçoit `flow_ready {req, flow}` ou `flow_refused`. Chaque bout a ses propres numéros de flux ; le relais les traduit.
- Datagramme sur `kn-media/1` : `[numéro de flux, entier variable de QUIC][paquet UDP]`.
- **Règle de taille** : un paquet qui ne tient pas dans un datagramme (`max_datagram_size`) part, s'il fait 1200 octets au plus, dans un flux unidirectionnel de secours de `kn-media/1` (un paquet par flux, même forme) ; plus grand, il est jeté (la recherche de MTU de Kyber redescend d'elle-même). 64 paquets de secours au plus attendent de partir sur une connexion (2 s chacun) ; au-delà, le paquet est perdu, comme en UDP.
- La connexion `kn-media/1` commence par un flux unidirectionnel ouvert par le bout qui se connecte, une trame `{"t":"media"}` (Station, reconnue à son certificat) ou `{"t":"media","token":"…"}` (app).
- Côté Station, l'agent tient pour chaque flux un socket UDP vers Kyber, gardé 25 s après une coupure : un client qui revient avec le même identifiant et la même `key` retrouve le même socket, et Kyber ne voit pas de nouvelle connexion.

### Repli TCP (`kn-tcp/1`)

TLS 1.3 sur TCP (le même port), ALPN `kn-tcp/1`, certificat client pour la Station ; yamux par-dessus (le bout qui se connecte est le client yamux). Premier flux : les messages (comme `kn-ctl/1`). Deuxième flux, ouvert par le bout qui se connecte après `welcome` : les datagrammes, une trame `{"t":"media"…}` comme ci-dessus puis des enregistrements `[longueur de la suite, 2 octets gros-boutistes][numéro de flux][paquet UDP]`. Les autres flux : les connexions TCP relayées, comme sur QUIC. Une Station en repli et une app en QUIC (ou l'inverse) se parlent normalement.

### Reconnexion

- La Station n'est connectée au relais que si un partage est ouvert ou si « Accès hors de chez vous » est actif. Essais après 1, 2, 4, 8, 16 puis 30 s (±20 %). À chaque reconnexion : `hello` avec les codes en cours ; le relais garde les codes d'une Station partie 120 s.
- QUIC suit les changements d'adresse (Wi-Fi vers 4G) sans reconnexion.
- Une seule connexion par Station : la dernière gagne, l'ancienne reçoit `error {code: "replaced"}`.

## Le relais sur le web

Sur TCP, le relais regarde le début de chaque connexion TLS :

- ALPN `kn-tcp/1` : le repli du tunnel ;
- nom `share.kybernow.app` : ses pages ;
- nom `<id>.station.share.kybernow.app` : le navigateur vers la Station `<id>`, **sans déchiffrer** (le TLS passe tel quel jusqu'à la Station, « Navigateur chiffré de bout en bout ») ;
- nom `<id>.share.kybernow.app` : le navigateur vers la Station `<id>`, déchiffré par le relais (Stations et liens d'avant la 0.11.8) ;
- autre nom : connexion fermée.

| Adresse | Réponse |
|---|---|
| `GET /` | Présentation de KyberNow Share et saisie du code (français ou anglais selon le navigateur, ou `?lang=fr` / `?lang=en`) ; le formulaire envoie `/j?code=…` |
| `GET /j/{code}` | Redirige (302) vers `https://<id>.share.kybernow.app/link/?join=<code>` (avec `:<port>` quand le port public annoncé du relais n'est pas le 443) ; code inconnu, ou Station absente : une page qui le dit |
| `GET /v1/codes/{code}` | `200 {"station":"<id>","station_key":"<SPKI base64>","admin_fp":"sha256:…","sig":"<base64>","online":true}` ; `online:false` : Station partie depuis moins de 120 s, les autres champs vides ; `404 {"error":"unknown_code"}` ; `400 {"error":"bad_request"}` ; `429 {"error":"too_many","retry_after":<s>}` avec `Retry-After` |
| `GET /privacy` | Ce que le relais voit et garde (français ou anglais) |
| `GET /assets/<fichier>?v=<version>` | Feuille de styles, polices, logo et favicon de la page, embarqués dans le binaire ; gardés un an (`immutable`, l'adresse porte la version) ; fichier inconnu : page « introuvable » |
| `GET /v1/health` | `200 {"ok":true,"version":"…"}` ; les apps iPhone et iPad l'appellent avant d'ouvrir le tunnel (réveil de la radio cellulaire) |
| `https://<id>.share…/*` | Mandataire HTTP/1.1 vers Admin de la Station (WebSocket compris) ; Station absente : page « ordinateur injoignable » (503) |

Les pages du relais n'ont ni script, ni cookie, ni mesure d'audience, et ne chargent rien d'un autre site : politique de contenu `default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'`. Elles n'écrivent jamais l'adresse du relais (chemins relatifs) : la même page sert sur le port d'écoute du relais et sur son port public (`https://share.kybernow.app`, sans port au 443). Détails : [server/relay/README.md](../server/relay/README.md#la-page-daccueil).

Mandataire : `Host` passé tel quel, `X-Forwarded-For` et `Forwarded` retirés (l'adresse du navigateur voyage dans l'en-tête du flux), `Domain=` retiré de tout `Set-Cookie`. Une origine par Station : le code d'une Station ne lit jamais les cookies d'une autre.

Codes : le relais **réserve les codes**, uniques entre toutes les Stations qui lui sont connectées. Quatre codes au plus par Station : celui du partage, celui du prêt et ceux des invitations (deux au plus). Les journaux du relais ne contiennent jamais un code.

## Navigateur chiffré de bout en bout (0.11.8)

Décidé avec l'auteur le 27/11/2026 (audit, STWIN-17). Jusqu'à la 0.11.7, le navigateur (« Admin à distance », le client web des invités) passe par `https://<id>.share.kybernow.app` : le relais présente son certificat générique `*.share.kybernow.app`, déchiffre, et fait suivre en HTTP dans le tunnel (`admin_http`). Il peut donc tout lire, cookie de session d'Admin compris. Depuis la 0.11.8, le relais **fait passer le TLS par le nom présenté** (SNI) jusqu'à la Station, qui tient sa propre clé et son propre certificat.

### Le nom : une zone que le certificat du relais ne couvre pas

- Le navigateur ouvre la Station par **`https://<id>.<tls_zone>`**, `tls_zone` valant `station.<domaine du relais>` : `https://<id>.station.share.kybernow.app` pour le relais public.
- Pourquoi pas `<id>.share.kybernow.app` : le relais garde son certificat générique `*.share.kybernow.app` pour les Stations 0.11.7 ; il vaut pour tous ces noms, et un relais qui le tient pourrait s'interposer sans que le navigateur voie rien. Un certificat générique ne couvre qu'une étiquette : `*.share.kybernow.app` ne vaut pas pour `<id>.station.share.kybernow.app`. Le relais ne tient **aucune clé valable** pour la nouvelle zone, et ne demande jamais de certificat pour elle.
- DNS : le joker `*.share.kybernow.app` répond déjà pour `<id>.station.share.kybernow.app` (RFC 4592, tant que `station.share.kybernow.app` n'a pas d'enregistrement à lui) ; un `*.station.share.kybernow.app` explicite vers la même machine est plus clair (déploiement, plus bas).
- Le relais n'annonce la zone (message `tls_zone`, juste après `welcome`) que si son port public est le 443 (l'autorité de certification ne valide que sur le 443) et que `tls_zone` n'est pas vide dans sa configuration (`tls_zone = ""` coupe le passage).

### Le certificat de la Station : ACME, défi TLS-ALPN-01, relayé tel quel

- Admin tire une clé ECDSA P-256 qui **ne quitte jamais la Station** (`<données>/relay/tls/cert.pem`, la clé et la chaîne ensemble, réservé à SYSTEM et aux Administrateurs sous Windows, à son compte sur Mac et Linux), une nouvelle à chaque certificat,, et demande un certificat pour `<id>.<tls_zone>` à Let's Encrypt (ACME, RFC 8555), avec son propre compte ACME (`<données>/relay/tls/account.key`). `KYBERNOW_ACME_DIRECTORY` remplace l'annuaire (essais, Let's Encrypt « staging »).
- Défi **TLS-ALPN-01** (RFC 8737) : l'autorité ouvre une connexion TLS vers `<id>.<tls_zone>:443` avec l'ALPN `acme-tls/1`. Le relais la fait passer comme celle d'un navigateur (flux `browser_tls`) ; Admin répond avec le certificat du défi (extension `acmeIdentifier`, critique : le SHA-256 de l'autorisation de clé). Le relais n'a rien à savoir d'ACME, et ne peut pas répondre à sa place : il ne connaît pas la clé du compte ACME de la Station.
- Pourquoi pas HTTP-01 : il faudrait le port 80, que le relais public n'ouvre pas, et le relais servirait lui-même la réponse. Pourquoi pas DNS-01 : il faudrait que le relais écrive dans le DNS pour chaque Station, avec son jeton d'API.
- Quand : agent joint avec `tls_zone`, et un besoin du navigateur : « Admin à distance » allumé, ou un partage, un code de prêt ou une invitation ouverts. Renouvellement quand il reste moins d'un tiers de la durée de validité. Échec : nouvel essai après 5 min, 15 min, 1 h, 4 h puis toutes les 24 h, ou après le `Retry-After` de l'autorité. Un seul essai à la fois.
- Admin garde les numéros de série des certificats qu'il a obtenus (`<données>/relay/tls/issued.json`) et dit au relais `tls_ready` (dans `hello` et `admin_status`) quand il tient un certificat valable pour le nom en cours.

### Ce que fait le relais

- Sur TCP, il lit le `ClientHello` sans répondre. Nom `<id>.<tls_zone>` : si la Station `<id>` est connectée et a annoncé `browser_tls`, il ouvre vers elle un flux `browser_tls` (en-tête comme `admin_http`, `host` = le nom) et y copie les octets tels quels, `ClientHello` compris, dans les deux sens, coupés après 5 minutes sans un octet (comme `admin_tls`). Sinon (Station absente, ou d'avant la 0.11.8), il ferme : il n'a pas de certificat pour montrer une page.
- Liens : quand la Station est connectée avec `tls_ready`, `/j/<code>`, « Ouvrir Admin » (`/compte/admin`) et la console mènent à `https://<id>.<tls_zone>` ; sinon à `https://<id>.share.kybernow.app`, comme avant. `/compte/ouvrir-admin` accepte les deux comme `retour` de cette Station.
- `<id>.share.kybernow.app` reste servi comme avant (`admin_http`), pour les Stations 0.11.7 et les liens déjà donnés.

### Ce que fait Admin

- Un flux `browser_tls` : TLS avec la clé de la Station (ALPN `http/1.1`, ou `acme-tls/1` pendant un défi seulement), puis le même HTTP que `admin_http`, avec la voie `Via { kind: browser_tls }` : même garde, mêmes règles pour les invités et pour « Admin à distance ». Pas de certificat valable : connexion fermée.
- **Pas de retour en arrière** : dès que la Station a tenu un certificat de bout en bout avec ce relais (`[relay] e2e = "<relais>"` dans `kyber-admin.toml`), « Admin à distance » ne passe plus par `admin_http` : le départ, le retour et les sessions y sont refusés (`403 {"code":"remote_admin_e2e"}`), et les sessions à distance déjà ouvertes par `admin_http` sont fermées à ce moment-là. Un relais qui cesserait d'annoncer `tls_zone` (mis à jour vers l'arrière, ou qui voudrait voir) ne rouvre donc pas le chemin déchiffré. Changer de relais efface cette marque ; rallumer « Admin à distance » sur place aussi (c'est l'accord explicite de l'hôte, si son relais ne fait vraiment plus passer le TLS).
- Les invités (client web, `/link/`) passent par les deux noms : le relais choisit le chemin chiffré dès qu'il est prêt.
- **Surveillance des journaux publics** : une fois par jour, tant que la Station a un certificat de bout en bout, Admin demande à `crt.sh` les certificats émis pour son nom (Certificate Transparency : toute autorité reconnue par les navigateurs doit y publier ce qu'elle émet). Un numéro de série que la Station n'a pas obtenu elle-même : une ligne d'avertissement dans le fil, `remote_tls.foreign` dans `GET /api/relay`, et « Admin à distance » suspendu jusqu'à ce que l'hôte le rallume sur place. `crt.sh` injoignable : rien ne change, on réessaie le lendemain.
- `GET /api/relay` gagne `remote_tls` : `{"state":"off"|"pending"|"ready"|"error"|"unsupported"|"lost","name":"<id>.station.share.kybernow.app"|null,"until":<secondes Unix>|null,"reason":null|"<texte>","foreign":false}`. `unsupported` : relais d'avant la 0.11.8, ou passage coupé ; `lost` : le relais n'annonce plus la zone alors que la Station a déjà été de bout en bout avec lui (« Admin à distance » suspendu).

### Compatibilité

| Station | Relais | Navigateur |
|---|---|---|
| 0.11.7 | 0.11.7 ou 0.11.8 | `<id>.share…`, déchiffré par le relais, comme avant (la Station n'annonce pas `browser_tls` : le relais ne lui envoie jamais de flux `browser_tls`, et ferme `<id>.station…`) |
| 0.11.8 | 0.11.7 | `<id>.share…`, comme avant (pas de message `tls_zone` : pas de certificat demandé, « Admin à distance » marche comme en 0.11.7) |
| 0.11.8 | 0.11.8 | `<id>.station…`, de bout en bout dès que le certificat est là ; `<id>.share…` en attendant, et pour les invités qui ont un ancien lien |

Les champs ajoutés (`browser_tls`, `tls_ready`) et le message `tls_zone` sont ignorés par les versions qui ne les connaissent pas.

### Ce que le relais voit encore, et ce qu'il peut encore

- Il voit le nom présenté (donc l'identifiant de la Station), les adresses, les volumes et les heures, plus rien du contenu.
- Pour se faire passer pour la Station, il faudrait que l'exploitant du domaine fasse émettre un autre certificat pour `<id>.station…` (il tient le DNS, il le peut) : ce certificat serait publié dans les journaux Certificate Transparency, la Station le verrait et suspendrait « Admin à distance ». Les navigateurs, eux, ne permettent pas d'épingler un certificat : c'est la meilleure garantie possible pour un site ouvert dans un navigateur.
- **Les jetons des comptes** : c'est toujours le relais qui signe le jeton `via: "relay"` qui ouvre une session à distance (après la passkey, vérifiée par lui). Un relais compromis pourrait en forger un et ouvrir lui-même Admin, en passant par le chemin chiffré comme un navigateur. Le fermer demande une preuve que la Station vérifie elle-même (« Plus tard »).
- Les journaux Certificate Transparency sont publics : chaque Station qui a eu un certificat y figure, sous son identifiant (déjà présent dans ses liens de partage). La page `/privacy` du relais le dit.

### Déploiement sur le relais public (à faire par l'auteur)

1. DNS (Gandi) : ajouter `*.station.share` (A et AAAA, comme `*.share`) vers la même adresse. Vérifier : `dig +short abc.station.share.kybernow.app`. Si la zone `kybernow.app` a des enregistrements CAA, ils doivent permettre `letsencrypt.org`.
2. Image du relais 0.11.8 (`ghcr.io/monsieurnours/kybernow-relay`) : rien à régler, `tls_zone` vaut `station.share.kybernow.app` par défaut. lego et le certificat générique ne changent pas (ils servent encore les Stations 0.11.7).
3. Vérifier : `/v1/health` (version 0.11.8) ; une Station 0.11.8 avec « Admin à distance » obtient son certificat en moins d'une minute (`GET /api/relay` : `remote_tls.state = "ready"`) ; `https://<id>.station.share.kybernow.app` s'ouvre, et le certificat montré par le navigateur est celui de la Station (émis pour ce nom seul, pas `*.share`).
4. Limites de Let's Encrypt : 50 nouveaux certificats par semaine pour `kybernow.app` (les renouvellements n'y comptent pas). Au-delà, les Stations attendent et réessaient (le navigateur passe par l'ancien nom en attendant). Quand le nombre de Stations s'en approche : demander une hausse à Let's Encrypt (formulaire « Rate Limit Adjustment »), ou inscrire `station.share.kybernow.app` à la Public Suffix List (chaque Station devient alors son propre domaine pour les limites et pour les cookies ; ne jamais y inscrire `share.kybernow.app`, qui porte les passkeys des comptes).
5. Retour en arrière : `KN_RELAY_TLS_ZONE=off` (ou `tls_zone = ""`) ; les Stations qui avaient été de bout en bout suspendent « Admin à distance » (état `lost`) jusqu'à ce que l'hôte le rallume sur place.

## Admin

### Réglages

| Route | Contenu |
|---|---|
| `GET /api/relay` | `{"server":"share.kybernow.app","remote_access":false,"id":"<identifiant>","state":"off"|"connecting"|"joined"|"unreachable","reason":null|"<texte>","transport":null|"quic"|"tcp","locked_until":null|<secondes Unix>,"sessions":[{"device":"<nom>","since":<secondes Unix>}]}` |
| `PUT /api/relay` | `{"server":"…","remote_access":true}` (chaque champ facultatif) → même réponse |

Session d'Admin seulement, jamais par le relais. `server` vide : pas de relais (ni partage hors de chez soi, ni accès). `state` : `off` (rien ne demande le relais : pas de partage ouvert et accès coupé), `connecting`, `joined`, `unreachable` avec `reason` (dans la langue d'Admin). Défaut : `share.kybernow.app` (port 443), accès coupé. L'installateur garde le dossier `relay\` des données (même identifiant après une mise à jour ou une réinstallation qui garde les données).

### `info`

`GET /api/stream/info` ajoute `"relay":{"server":"share.kybernow.app","id":"<identifiant>","remote_access":true}` quand un relais est réglé (absent sinon, et pour une Station antérieure). L'app le garde pour chaque ordinateur.

### Partage

- **Vue du partage** (`open`, `GET /api/share`, `code`, signe de vie de l'hôte, routes locales) :
  - `relay` : `null` (pas de relais) ou `{"state":"connecting"|"joined"|"unreachable"}` (`connecting` : agent en connexion ou code pas encore réservé ; `unreachable` : connexion échouée) ;
  - relais joint : `link` porte en plus `&relay=<relais>&sid=<identifiant>`, et `web` devient `https://share.kybernow.app/j/<code>` (le relais garde son port quand ce n'est pas le 443 : `https://relais.exemple.org:9443/j/<code>`, `&relay=relais.exemple.org%3A9443`) ; `web_local` garde toujours l'adresse sur le réseau (`https://<adresse>:<port>/link/?join=<code>`) ;
  - `guests[]` gagne `pending` (vrai pour une demande pas encore acceptée) et `via` (`"lan"` ou `"relay"`). Une demande est spectatrice, `since` est l'heure de la demande ; elle ne prend pas de place tant qu'elle n'est pas acceptée.
- **Codes** : relais joint, Admin ne tire plus le code lui-même : il demande `reserve` (« Nouveau code » : `reserve` puis `release` de l'ancien). Relais injoignable : code tiré par Admin, proposé au relais à la connexion (remplacé seulement s'il est pris). Le code vaut toujours sur le réseau, sans accord.
- **Rejoindre par le relais** : `POST /api/share/join` (même corps) → `202 {"pending":true,"guest":"<id>","secret":"<secret>","expires":120}`. Pas encore de prénom de l'hôte : il n'est donné qu'après l'accord. Refus en plus de ceux du partage : `409 {"error":"pending_full"}` (trois demandes en attente au plus), `403 {"error":"refused"}` (appareil refusé il y a moins de 10 minutes).
- **Attendre** : `POST /api/share/wait`, en-tête `x-kybernow-guest: <secret>`, `{"guest":"<id>"}`, réponse tenue 25 s au plus : `202 {"pending":true}` (toujours en attente, rappeler), `200` (la réponse de `join` sur le réseau, avec `"relay":true`), `403 {"error":"refused"}`, `410 {"error":"expired"}` (pas de réponse en 120 s), `410 {"error":"stopped"}` (le partage s'est arrêté pendant l'attente : « Ce partage est arrêté. »). `POST /api/share/leave` annule une demande.
- **Accepter, refuser** : `POST /api/share/guest/{id}/accept`, `POST /api/share/guest/{id}/refuse` (hôte), et `/api/local/share/guest/{id}/accept|refuse` (le PC) → la vue du partage. Les places se comptent à l'acceptation : `409 {"error":"full"}` si les quatre sont prises.
- La réponse de `join` et de `wait` porte `"relay":true` par le relais, et `GET /api/session` aussi : le client web prend alors l'image en WebSocket (raison `relay`), jamais en WebTransport.
- Cookie du navigateur par le relais : `__Host-kybernow_guest` (`Secure`, `Path=/`, `SameSite=Strict`, `HttpOnly`, sans `Domain`) ; par le relais, Admin ne lit que ce nom (`kybernow_guest` seulement sur le réseau) : la page d'une autre Station, sur une origine sœur, peut poser un `kybernow_guest` pour tout `share.kybernow.app`, jamais un cookie `__Host-`.

### Fil

« {appareil} est en session hors de chez vous » au début d'une session par le relais ; « Accès hors de chez vous bloqué 10 min pour {adresse} après trop d'essais. » au blocage d'une adresse ; « Accès hors de chez vous bloqué 10 min après trop d'essais. » au blocage de tout l'accès ([link-ui.md](link-ui.md)).

## Garde d'Admin selon la voie

Admin juge « local » ce qui vient de la boucle locale (`/api/local/*`, `local_or_admin`). Le relais ne doit jamais passer pour local :

- L'agent **sert lui-même** ce qui arrive par le relais, dans le processus d'Admin (`admin_tls` : TLS sur le certificat d'Admin, puis HTTP ; `browser_tls` : TLS sur le certificat de la Station pour `<id>.<tls_zone>`, puis HTTP ; `admin_http` : HTTP), jamais en se reconnectant en TCP sur `127.0.0.1`. L'adresse du pair est celle que donne l'en-tête du flux, **forcée hors de la boucle locale et des adresses privées**, et la requête porte la voie (`Via::Relay {kind, client}`).
- Un `Router` à part, `relay_guard`, n'ouvre que :
  - à un **invité**, quand un partage est ouvert : `/api/share/join|wait|beat|quality|leave`, `/link/*`, `/assets/*`, `GET /api/session` réduit, le mandataire d'Admin vers Kyber (`kyber_proxy`) avec le cookie d'un invité accepté (WebSocket seulement, jamais `/kymux/start_mux`) ;
  - à un **appareil appairé** (compte Basic), seulement quand « Accès hors de chez vous » est actif : `/api/stream/*` (sauf `/api/stream/station/*`) et les routes de l'hôte `/api/share*` ; accès coupé : `403 {"error":…,"code":"remote_access_off"}` ;
  - à un **appareil invité**, quand une invitation est ouverte et que « Accès hors de chez vous » est actif : `POST /api/pair/invite` (code et mot de passe de l'invitation, [invitations.md](invitations.md)) ; `POST /api/share/join` passe aussi quand une invitation est ouverte (il répond `"invite": true`) ;
  - **jamais** par le relais : le redémarrage et la mise à jour de la Station (`/api/stream/station/restart`, `/api/stream/station/update` : une action lourde, qui se fait sur place ou dans Admin), `/api/login`, `/api/setup`, `/api/password`, `/api/web/ticket`, `/api/pair*` (sauf `/api/pair/invite`), `/api/pairing`, `/api/invites*`, `/api/local/*`, la configuration, les comptes, les journaux, le service, l'écran, le son, la mise à jour d'Admin, les appareils, les apps, `/api/events`, l'interface d'Admin. L'appairage par code se fait sur le réseau seulement ; une invitation ajoute un appareil de loin.
  - **sauf « Admin à distance »** (docs/comptes.md, « Admin depuis KyberNow Share ») : un navigateur (`browser_tls`, ou `admin_http` tant que la Station n'a jamais été de bout en bout avec ce relais : « Navigateur chiffré de bout en bout ») avec une session ouverte par `/api/login/kybernow` grâce à un jeton `via: "relay"` (passkey), cookie `__Host-kyber_admin_session`, interrupteur allumé : tout Admin, la garde remettant ce cookie en cookie de session. Le départ et le retour de cette connexion (`GET /api/login/kybernow/start`, `GET /api/login/kybernow`) passent aussi, interrupteur allumé. « Admin à distance » ne s'allume pas par le relais (`PUT /api/relay`, `403 network_only`).
- **Blocage** : 10 échecs du compte Basic par le relais depuis une même adresse (un /64 en IPv6) en 10 minutes bloquent **cette adresse** 10 minutes (`429 {"code":"locked"}`) ; le propriétaire, d'ailleurs, passe toujours. Garde-fou : 100 échecs en 10 minutes, toutes adresses confondues, bloquent tout l'accès des appareils par le relais 10 minutes (`locked_until` de `GET /api/relay`). Chaque blocage met une ligne dans le fil.
- **Kyber** : l'agent ne joint Kyber que pour un client autorisé (un invité accepté, tant qu'il donne signe de vie ; un appareil appairé après `prepare`, pour 30 s renouvelées par le signe de vie), et toujours **depuis et vers l'adresse du réseau local** de l'ordinateur, jamais par la boucle locale (le correctif 0019 réserve `/kybernow/*` à la boucle locale). Kyber écoute sur toutes les adresses.
- Admin autorise un **client du relais** (son identifiant) : celui qui a fait le `wait`, le `beat` ou le `prepare`/`heartbeat` réussi. Un client qui se reconnecte garde son identifiant.

### Précisions de la Station

Ce qu'Admin fait en plus de ce qui précède, sans rien y changer (`server/admin/src/relay.rs`, `share.rs`, `share_api.rs`) :

- **Réglages** : `[relay]` de `kyber-admin.toml`, `server` absent = le relais public, vide = pas de relais ; `PUT /api/relay` relit l'adresse sous sa forme normale, sans port pour le 443 (`https://share.kybernow.app/` et `share.kybernow.app:443` deviennent `share.kybernow.app`) et répond `400 {"error": <texte>}` si elle est illisible. `/api/local/state` (l'icône) donne aussi `relay` : `server`, `remote_access`, `state`, `reason`. `reason` est le texte de l'agent du tunnel, en français ; un relais qui refuse pour de bon (banni, version) : `unreachable` avec « refusé par le relais ».
- **Vue du partage** : `relay.state` vaut `joined` seulement quand le code du partage est réservé chez le relais ; « en cours de connexion », ou le code pas encore réservé, vaut `connecting` ; un échec, `unreachable`. Admin lance la connexion dès l'ouverture du partage (sans attendre la seconde suivante) et garde l'agent connecté 5 minutes après le dernier partage, pour que le suivant soit prêt tout de suite. Une réservation qui échoue est retentée toutes les 10 s ; le partage arrêté, son code est rendu au relais (l'agent reste connecté si l'accès est actif).
- **Rejoindre** : par le relais, un invité déjà là qui rejoint de nouveau entre aussitôt (`200`, comme sur le réseau) s'il le prouve : son secret (`x-kybernow-guest`), le cookie `__Host-kybernow_guest` de son navigateur, ou le même client du relais (celui qui tient son autorisation vers Kyber) ; sinon, c'est une nouvelle demande, et l'invité garde sa place (l'identifiant d'appareil, choisi par l'appareil, ne prouve rien) ; un `beat` avant l'accord rend `202 {"pending":true}` ; `wait` d'une demande inconnue (mauvais secret, sortie depuis plus d'une minute) : `404 {"error":"unknown_guest"}`. Le blocage de 10 minutes après un refus vise l'appareil (`device`), ou l'adresse s'il n'en donne pas. Une demande retirée par l'hôte (`DELETE …/guest/{id}`) vaut un refus, sans blocage.
- **`GET /api/session` réduit** : `{"configured", "authenticated": false, "relay": true, "version"}`.
- **Garde** : partage fermé, `join`, les pages et le mandataire vers Kyber rendent `404` ; `beat`, `wait` et `leave` passent (« arrêté » pendant une minute). Par le relais, le mandataire vers Kyber relaie `/session/login`, `/ws`, `/websocket/*`… mais refuse `/kymux/start_mux` : c'est ce qui garde l'image du navigateur en WebSocket. Une requête du relais refusée porte `{"error": <texte>, "code": "relay_forbidden"}` ; le cookie de session d'Admin est retiré de toute requête du relais avant la garde.
- **Adresse forcée** : une adresse locale ou privée donnée par le relais (un client derrière la même box que lui) devient `100::1` (bloc IPv6 « à jeter ») ; les limites par adresse (`join`) la comptent comme une seule.
- **Blocage** : il ne vise que les routes des appareils (compte Basic) : un invité n'est pas bloqué par lui. Il compte par l'adresse que donne le relais (une adresse locale ou privée devient `100::1`, comptée comme une seule). Un essai en cours compte d'avance : quand les échecs et les essais en cours atteindraient la limite, un nouvel essai attend (`429 locked`, `retry_after` 1) ; des requêtes simultanées ne la dépassent donc jamais. Un succès n'efface pas les échecs.
- **Kyber** : une autorisation dure 30 s, renouvelée par `wait`, `beat`, `prepare` et le signe de vie de l'hôte ; elle tombe au départ de l'invité, à son retrait, à son silence, à l'arrêt du partage, à la fin de session de l'appareil ou, pour les appareils, dès que « Accès hors de chez vous » est coupé, et ferme alors ses flux UDP et ses connexions TCP vers Kyber.
- **Fil** : en plus, « {prénom} demande à rejoindre (hors de chez vous) », « {prénom} : demande refusée », « Accès hors de chez vous activé » et « … coupé ».
- `KYBERNOW_RELAY_TCP=1` force le repli TCP de l'agent (essais).

## Liens

- Apps : `kybernow://join?host=<adresse>&port=<port d'Admin>&fp=<empreinte d'Admin>&code=<6 chiffres>&relay=<relais>&sid=<identifiant>` (avec `relay` et `sid` quand le relais est joint ; le relais garde son `#sha256:…` éventuel, encodé). L'app essaie d'abord le réseau (`host`, `port`), puis le relais vers `sid`, Admin épinglé sur `fp`.
- Navigateur : `https://share.kybernow.app/j/<code>`, qui redirige vers `https://<id>.share.kybernow.app/link/?join=<code>`. Pas de lien universel sur iPhone pour l'instant : la page propose « Ouvrir dans l'app ».
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

- `apps/core/src/relay.rs` (runtime tokio à lui, deux fils) : `relay::Tunnel::open(&RelayRoute {server, station, admin_fp})`, `admin_addr()`, `kyber_addr(port)`, `stats()` ; un tunnel par (relais, Station), fermé après 30 s sans usage ; `lookup(&relais, code)`, `presence`. Partage : `share::join_anywhere`, `share::join_link`, `share::wait`, `share::cancel` ([apps/core/README.md](../apps/core/README.md), section « Relais »).
- `SessionParams::route` : `Route::Direct`, `Route::Relay(RelayRoute)`, `Route::Auto(Option<RelayRoute>)` ; `Console::with_relay(Option<RelayRoute>)`.
- Indicateur : `Platform::on_route(&RouteInfo {transport: direct|relay|relay_tcp, relay, rtt_client, rtt_station})` ; puce « Par le relais », ligne « Chemin » des statistiques, `route` dans « Dernière session ».
- Adresses : sans port, c'est le 443 ; la forme normale l'omet (`share.kybernow.app`) et garde un autre port (`relais.exemple.org:9443`) : `relay::normalize`, `RelayInfo::route`, `parse_link`, `kn_relay_normalize`. Le crate du tunnel : `RelayAddr::web_authority()` pour les adresses web et les liens, `authority()` (toujours avec le port) pour se connecter. `join_anywhere` interroge toujours le relais public, `https://share.kybernow.app/v1/codes/{code}`, en plus de ceux des ordinateurs connus.
- Refus typés, jamais reconnus au texte : `RelayError::AccessOff` (Admin par le relais, 403 `remote_access_off`), `RelayError::StationOffline` (ordinateur absent du relais), `RelayError::Unreachable` ; codes des ponts 32, 31, 30 ; en session, `Platform::on_relay_error(erreur, texte)` (C `on_relay_error`, JNI `onRelayError`) avec le texte de link-ui et le nom de l'ordinateur ; `kn_relay_error_text(code, nom)`, `Native.relayErrorText`. Invité en attente quand l'hôte arrête le partage : `wait` répond `{"error":"stopped"}`, `ShareError::Stopped` (« Ce partage est arrêté. », code 28).
- `KYBERNOW_RELAY=force` (toujours par le relais, pour essayer à la maison) ou `off`.
- Points d'écoute : toujours sur `127.0.0.1`, jamais `0.0.0.0` ; TCP lié sur le port 0, puis UDP sur le même numéro (on recommence s'il est pris). iPhone, iPad, Apple TV : pas d'autorisation « Réseau local » pour la boucle locale ; les sockets se recréent au retour au premier plan (port changé : la session repart).
- Ponts : `kn_relay_*`, `kn_share_wait` ; JNI `Native.relay*`, `Native.shareWait`.

## Consoles PlayStation

Une Station fait suivre la partie d'une console PlayStation de son réseau (contrat : [playstation.md](playstation.md)). Admin seul : Kyber n'y est pour rien.

- **Les consoles vues** : Admin cherche les consoles de son réseau (`SRCH` en diffusion, ports 9302 et 987, et en direct vers celles déjà vues, toutes les 30 s, et à la demande : `GET /api/consoles` cherche de nouveau quand la dernière recherche a plus de 10 s, une seconde au plus) et garde leur identifiant (`host-id`), nom, modèle, état et adresse sur le réseau. `GET /api/consoles` → `{"consoles":[{"host_id":"C0151B2DAE08","name":"PS5 de Matthieu","model":"ps5"|"ps4","state":"ready"|"standby","seen":<secondes Unix>}]}` (jamais l'adresse ; une console muette depuis 5 minutes n'y est plus) ; pour un appareil appairé (compte Basic), en direct ou par le relais (« Accès hors de chez vous » allumé), comme `info`. Une Station d'avant les consoles répond `404`.
- **Les flux** : `open_flow`, `flow_open` et les connexions TCP relayées portent `"to":"console","host_id":"<identifiant>","port":<port>` au lieu d'aller à Kyber. Une connexion TCP vers une console s'ouvre avec le service `console_tcp` (`{"t":"open","service":"console_tcp","to":"console","host_id":…,"port":…}`) ; le relais écrit l'en-tête `{"t":"stream","v":1,"kind":"console_tcp",…,"to":"console","host_id":…,"port":…}`. Le relais fait suivre ces champs sans rien décider ; un `to: "console"` sans identifiant ou sans port est refusé (`bad_request`), jamais pris pour Kyber. Admin ne joint que l'adresse qu'il a lui-même trouvée pour cet identifiant, depuis l'adresse du réseau local de l'ordinateur, et seulement ces ports : TCP 9295 (session, canal de contrôle, association) et 997 (PS4) ; UDP 9295 (recherche `SRC3`/`SRC2` de l'association), 9296 (Takion), 9302 et 987 (état, réveil). Identifiant inconnu ou port autre : refus (`console_unknown`, `port_forbidden`) ; client sans signe de vie : `not_authorized`. Jamais une adresse donnée par l'app : la Station n'est pas un mandataire ouvert.
- **Autorisation** : celle d'un appareil appairé hors de chez soi, renouvelée par son signe de vie (`POST /api/consoles/beat`, 30 s ; l'app le redonne toutes les 10 s) pendant l'état, le réveil et la partie ; la réponse est celle de `GET /api/consoles`. Elle ne vaut que pour les consoles : elle n'ouvre pas Kyber, et la fin d'une session Kyber ne ferme pas les flux d'une console. « Accès hors de chez vous » coupé : flux et connexions vers les consoles fermés tout de suite. Un invité de KyberNow Share n'y a jamais accès (compte Basic exigé). Sur le réseau, le signe de vie n'autorise rien (il n'y a pas de flux).
- **Paquets** : sans changement (un paquet UDP par datagramme ; la console envoie des paquets d'au plus 1 200 octets, demandés par l'app). Le socket vers la console est gardé 25 s après une coupure, comme vers Kyber.
- **Version du relais** : un relais d'avant les consoles coupe `console_tcp` (service inconnu) et fait suivre `open_flow` sans `to` : Admin y voit un flux vers Kyber, refusé à un client qui n'a pas fait `prepare`. Le relais public doit donc être à jour.
- **Cœur** : `relay::Tunnel::console_addr(host_id, port)` (TCP et UDP sur `127.0.0.1:L`, comme `kyber_addr`) ; un seul flux UDP par port de la console (les réponses vont au dernier socket de l'app qui a parlé : la recherche et le réveil en prennent un neuf à chaque essai), libéré après 15 s de silence, pour rester sous les quatre flux par client du relais ; la découverte, le réveil, l'association, la session et Takion de `playstation` prennent une `Endpoint` (adresse et ports, `playstation::endpoint`) : par le relais, la boucle locale et les ports du tunnel, MTU de Takion à 1 200. `playstation::remote::Remote::open(&console_d_admin_par_le_relais, host_id)` : le signe de vie, puis les quatre ports (9302, 987, 9295, 9296), et le signe de vie toutes les 10 s tant qu'il vit. Ponts Apple : `kn_console_consoles`, `kn_ps_remote_open` (code 40 : la Station ne voit pas la console), `kn_ps_remote_address`, `kn_ps_remote_state`, `kn_ps_remote_route`, `kn_ps_search_at` ; les autres `kn_ps_*` prennent l'adresse de `kn_ps_remote_address` comme une adresse IPv4.
- **Stations** : Windows et Mac (le même Admin, `server/admin`).
- **Essayé** le 03/11/2026, sur la boucle locale du Mac (le faux relais des essais du cœur, qui fait suivre les ports comme une Station) vers la vraie PS5 Pro de l'auteur : état « en veille » lu par le tunnel, réveil, session (l'en-tête `Host` porte `127.0.0.1`, la console l'accepte), canal de contrôle, flux en 1080p H.265 avec la MTU de 1 200 octets, 300 images sans perte. Pas encore essayé : le vrai relais (`share.kybernow.app`) avec un vrai Admin sur une Station Windows ou Mac, et l'app Mac hors de chez soi.

## Confiance

- **Le relais** est vérifié par son certificat (autorités du web, ou empreinte épinglée).
- **Code tapé** : c'est le relais qui donne l'empreinte d'Admin. Un relais menteur pourrait s'interposer ; c'est le prix du code à six chiffres. L'hôte voit et accepte chaque invité.
- **Lien et QR code** : `fp` et `sid` viennent de la Station ; l'échange est de bout en bout, le relais ne peut pas s'interposer.
- **Appareils appairés** : empreintes épinglées à l'appairage (sur le réseau). Par le relais, jamais de premier contact, sauf l'invitation : par un code tapé, l'empreinte vient du relais (comme pour un partage), d'où le mot de passe en plus ; par son lien ou son QR code, elle vient de la Station ([invitations.md](invitations.md)).
- **Navigateur** : depuis la 0.11.8, chiffré de bout en bout jusqu'à la Station (`<id>.station.share.kybernow.app`, certificat et clé de la Station) ; le relais ne peut se faire passer pour elle qu'en faisant émettre un autre certificat, que les journaux publics montrent et que la Station surveille (« Navigateur chiffré de bout en bout »). Avec une Station ou un relais plus anciens, le relais déchiffre (vrai certificat `*.share.kybernow.app`). Les apps, elles, restent chiffrées de bout en bout.
- **« Admin à distance » et comptes KyberNow** : c'est le relais qui signe les jetons des comptes KyberNow (droits Session, Administrer…, et le jeton `via: "relay"` qui ouvre Admin à distance). Il ne lit plus la session d'Admin ouverte à distance (0.11.8, de bout en bout), mais un relais compromis pourrait forger un jeton et ouvrir lui-même une session. Qui le contrôle a donc encore la main sur les Stations rattachées qui ont « Admin à distance » ou les comptes acceptés : un relais personnel n'est sûr que s'il est aussi bien tenu que la Station. Sans « Admin à distance » ni comptes, le relais ne fait que transporter des flux chiffrés.
- Le relais public est en France ; il garde les adresses IP dans ses journaux quelques jours (journaux tournés par taille), jamais les codes. Page de confidentialité sur le relais (`/privacy`).

## Limites et protections du relais

- Recherche de code : 10 par minute et 60 par heure par adresse, 30 par minute par /24 (IPv4) ou /48 (IPv6), 600 par minute en tout. Admin garde ses 10 essais de `join` par minute et par adresse réelle.
- QUIC : 16 connexions par adresse, 60 négociations par minute par adresse, 4096 en tout ; au-delà d'un seuil, le relais demande une preuve d'adresse (Retry).
- TCP : 32 connexions par adresse ; TLS et en-têtes HTTP en 10 s, 16 Kio d'en-têtes au plus.
- 4 Stations par adresse, 20 connexions de Station par minute et par adresse, une connexion par Station (la dernière gagne), 8 clients par Station, dont 2 au plus depuis une même adresse (un /64 en IPv6), 4 flux UDP par client. L'identifiant d'une Station est dans tous ses liens : pour qu'il ne suffise pas à la bloquer, un client sans flux ni connexion relayée pendant 30 s est renvoyé (sa place se libère), et à une Station pleine le client inactif depuis le plus longtemps cède sa place au nouveau ; si tous sont occupés, le nouveau reçoit `error {code: "too_many"}`. Une Station ne garde jamais plus de 4 codes, reconnexion après reconnexion.
- 1024 messages au plus en attente vers un bout : un bout qui ne lit pas ce que le relais lui envoie (`pong`, `presence`…) est coupé. Repli TCP : 64 Mio au plus en attente par connexion (fenêtre yamux).
- Pas de plafond de débit. Le relais ne fait suivre que vers des Stations connectées, et Admin ne laisse passer que des clients autorisés. Liste de bannissement relue à chaud ; plafond global facultatif, coupé par défaut ; métriques Prometheus et alerte de débit.

## Installer un relais chez soi

Une machine Linux avec Docker, un nom de domaine dont la zone DNS accepte un jeton d'API (Gandi par défaut, pour le certificat générique), un port ouvert en TCP **et** UDP, le 443 de préférence. Le navigateur chiffré de bout en bout demande le 443 public et que `*.station.<domaine>` mène au relais (le joker `*.<domaine>` suffit d'ordinaire) ; sur un autre port, le relais ne l'annonce pas et le navigateur passe par `<id>.<domaine>`, déchiffré, comme avant. Le relais écoute sur un port de la machine (`RELAY_LISTEN`, 8443 par défaut) et annonce le port public que la box renvoie vers lui (`RELAY_PUBLIC_PORT`) ; le pas à pas est dans [server/relay/README.md](../server/relay/README.md), « Les ports et la box ». L'image du relais est publiée à chaque version (`ghcr.io/monsieurnours/kybernow-relay`, par `.github/workflows/relay-image.yml`) ; le service `updater` du compose la reprend chaque nuit. Dans Admin de chaque Station : « Hors de chez vous › Relais » = l'adresse avec son port public, `relais.exemple.org` pour le 443, `relais.exemple.org:9443` pour un autre.

## Plus tard

- « Admin à distance » sans confiance dans le relais pour l'identité : une passkey enregistrée sur place auprès de la Station (sur `<id>.station…`, après un code montré par Admin sur le réseau), que la Station vérifie elle-même à chaque ouverture, en plus du jeton du relais. Un relais compromis ne pourrait alors plus ouvrir Admin. À décider avec l'auteur (une étape de plus pour l'hôte).

- Image du navigateur en WebTransport par le relais (sauf Safari) : le relais lirait le nom dans le premier paquet QUIC et ferait suivre sans déchiffrer.
- Partage équitable du lien montant de la maison entre plusieurs invités.
