# Invitations : ajouter un ordinateur de loin

[English](invitations.en.md) · Français

Une invitation ajoute un appareil à l'ordinateur **sans que personne ait à l'accepter** sur place : un code à six chiffres et un mot de passe, valables quelques jours. L'appareil peut être n'importe où : il passe par le relais ([relais.md](relais.md)). C'est le moyen de donner un accès d'essai (l'App Review d'Apple, [app-store.md](app-store.md)) ou d'ajouter son propre téléphone quand on est loin de chez soi.

Une fois ajouté, l'appareil est un appareil appairé comme un autre : il a son compte propre, ouvre la session entière de l'ordinateur (le bureau, pas le lanceur du prêt), sur le réseau ou hors de chez soi. Il perd son accès quand l'invitation échoit ou qu'on la supprime.

Ce document est le contrat commun d'Admin, du cœur Rust, de toutes les apps Link et du client web. Les textes exacts sont dans [link-ui.md](link-ui.md), section « Invitations ».

## Ce que voit l'utilisateur

### Dans Admin (Appareils Link)

- Section **« Invitations »** : « Inviter un appareil… ». On donne un **nom** (« App Review ») et une **durée** (1, 7 ou 30 jours ; 7 proposés).
- Admin montre alors le **code** (« 123 456 »), le **mot de passe** (`k7mq-p2xa-9fz3`, douze signes sans ambiguïté, montré **une seule fois**), l'échéance et le QR code du lien des apps.
- Avec un relais réglé, créer une invitation **active « Accès hors de chez vous »** (l'appareil ajouté de loin doit pouvoir se connecter ensuite) ; Admin le dit. Sans relais, l'invitation ne marche que sur le réseau.
- La liste : nom, code, échéance, appareils ajoutés (« 2 / 5 »), avec leurs noms ; « Nouveau mot de passe » (l'ancien ne sert plus, les appareils déjà ajoutés gardent leur accès) ; « Supprimer » (les appareils ajoutés par l'invitation perdent leur accès).
- **Deux invitations au plus** en même temps, **cinq appareils** au plus par invitation.
- Dans la liste des appareils, un appareil ajouté par une invitation porte « Invitation « {nom} » ». Le retirer libère sa place.

### Dans les apps Link

1. **« Rejoindre avec un code »**, comme pour un partage : on tape les six chiffres (ou on ouvre le lien, on scanne le QR code).
2. La Station répond que c'est une invitation : l'app montre **« Ajouter {ordinateur} »**, avec le champ **« Mot de passe »**.
3. Bon mot de passe : l'ordinateur apparaît sur l'accueil, comme un ordinateur appairé ; l'app le sélectionne et dit « {ordinateur} est ajouté. ». Il s'ouvre ensuite par « Se connecter », sur le réseau ou par le relais.
4. Le même ordinateur déjà sur l'accueil (même empreinte d'Admin) n'est pas ajouté deux fois : il prend le nouvel accès.

Le client web est servi par l'ordinateur lui-même : un code d'invitation y dit seulement « Ce code sert à ajouter l'ordinateur dans l'app KyberNow Link. »

## Sécurité

- **Le code seul ne donne rien** : il trouve la Station (sur le réseau, ou chez le relais) ; seul le mot de passe ajoute l'appareil.
- **Mot de passe** : douze signes de `abcdefghjkmnpqrstuvwxyz23456789` (59 bits), tirés par Admin ; gardé en empreinte Argon2 (`invites.json`, dossier de données réservé à SYSTEM et aux Administrateurs). Majuscules, espaces et tirets ne comptent pas : tapé à la télécommande, il passe pareil.
- **Essais** : cinq mots de passe faux depuis une même adresse (un /64 en IPv6) en 10 minutes la bloquent 10 minutes ; cinquante en tout bloquent toutes les invitations 10 minutes (une ligne dans le fil). 0,8 s d'attente après chaque mot de passe faux.
- **Confiance** : par un code tapé, c'est le relais qui donne l'empreinte d'Admin, comme pour un partage ([relais.md](relais.md), « Confiance ») ; un relais menteur pourrait s'interposer et lire le mot de passe. Le lien et le QR code portent l'empreinte venue de la Station : à préférer quand on peut les transmettre.
- **Échéance** : à la fin, l'invitation est supprimée avec les appareils qu'elle a ajoutés (comptes Kyber supprimés, session en cours terminée).

## Admin

### Routes

| Route | Qui | Quoi |
|---|---|---|
| `GET /api/invites` | Session d'Admin | `{"invites":[…],"max":2,"days":[1,7,30],"relay":{"set":bool,"remote_access":bool}}` ; une invitation : `id`, `name`, `code`, `created`, `expires`, `devices` (`[{id,name,kind,last_seen}]`), `max_devices`, `last_used`, `link` (lien des apps, `kybernow://join?…&code=…`, avec `relay` et `sid` quand le relais tient le code), `relay` (`null` ou `{"state":"connecting"|"joined"|"unreachable"}`) |
| `POST /api/invites` | Session d'Admin | `{"name","days"}` → la vue, plus `invite`, `password` (en clair, seulement ici) et `remote_access_turned_on` ; refus `400` : nom vide, durée inconnue, deux invitations déjà |
| `DELETE /api/invites/{id}` | Session d'Admin | Supprime l'invitation et ses appareils ; rend la vue |
| `POST /api/invites/{id}/password` | Session d'Admin | Nouveau mot de passe : la vue, plus `invite` et `password` |
| `POST /api/share/join` | Tous | Avec un code d'invitation (pas de partage sous ce code) : `404 {"error":"no_share","invite":true,"computer":"<nom>"}` |
| `POST /api/pair/invite` | Tous (sur le réseau, et par le relais) | `{"code","password","device":{id,name,kind,os,app}}` → `200 {"username","password","hostname","address","port","fingerprint","relay":{"server","id","remote_access"},"invite":{"name","expires"}}` ; refus `{"error":<code>,"message":<texte>}` : `404 no_invite`, `401 bad_password`, `409 full`, `429 too_many` (`retry_after`, en-tête `Retry-After`), `400 bad_request`, `503 unavailable` ; par le relais, `403 remote_access_off` (accès hors de chez vous coupé) |

### Relais

- Garde ([relais.md](relais.md), « Garde d'Admin selon la voie ») : `POST /api/pair/invite` passe par le relais quand une invitation est ouverte et que l'accès hors de chez vous est actif ; `POST /api/share/join` passe aussi quand une invitation est ouverte. Les routes `/api/invites*` ne passent jamais.
- Codes : la Station tient chez le relais le code de chaque invitation ouverte, avec ceux du partage et du prêt (quatre codes au plus par Station : d'où deux invitations) ; un code pris ailleurs est remplacé par celui que donne le relais. Une invitation ouverte garde l'agent connecté au relais.

### Compte

L'appareil reçoit un compte Kyber à lui, comme après un appairage accepté (`Pairing` : nom de l'appareil et quatre caractères, secret de 128 bits) ; Kyber redémarre dès qu'il ne sert personne pour le prendre. Un appareil qui revient par la même invitation remplace son ancien compte. L'invitation garde la liste de ses appareils (`devices`).

### Fil

« Invitation « {nom} » créée » ; « {appareil} ajouté par l'invitation « {nom} » » ; « Invitation « {nom} » supprimée, avec ses appareils » ; « Invitation « {nom} » échue : ses appareils sont retirés » ; « Invitations bloquées 10 min pour {adresse} après trop de mots de passe faux. » ; « Invitations bloquées 10 min après trop de mots de passe faux. »

## Cœur et apps

- `share::ShareError::Invite(Box<invite::InviteCode>)` (`kind` `invite`) : « Rejoindre » est tombé sur un code d'invitation ; `InviteCode { code, target, route, computer }` dit quelle Station, par quel chemin, et le nom de l'ordinateur. Comme `ShareError::Lend`, il vient de `join_anywhere`, `join_link` et `join`.
- `invite::redeem(&InviteCode, mot_de_passe, &DeviceInfo) -> Result<Invited, InviteError>` (bloquant, 15 s au plus) : `Invited { hostname, address, port, fingerprint, username, password, relay, invite_name, expires, via_relay }`, à garder comme un ordinateur appairé (adresse et port d'Admin, empreinte épinglée, compte, `info.relay`).
- `InviteError` : `BadPassword`, `NoInvite`, `Full`, `TooManyTries { seconds }`, `AccessOff`, `Other(ConsoleError)` ; `text(lang, ordinateur)`, `kind()` (`bad_password`, `no_invite`, `full`, `too_many`, `access_off`, `other`), `code()` pour les ponts (60 à 65).
- Ponts : C (`apps/apple`, `kn_invite_redeem`), JNI (`apps/android/jni`), et les apps en Rust (Windows, Linux, webOS) appellent le cœur directement.
- Une Station antérieure ne connaît pas les invitations : elle répond « Aucun partage avec ce code », rien à faire de plus.
