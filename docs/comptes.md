# Comptes KyberNow : cadrage

[English](comptes.en.md) · Français

Un **compte KyberNow** (adresse e-mail, mot de passe, double authentification, passkeys) donne accès, depuis n'importe quel appareil, à tous les ordinateurs auxquels il a droit : les siens, ceux qu'on lui a ouverts (famille, amis), ceux de son équipe. Le relais devient un service à la Parsec : il tient les comptes, les droits, les organisations, et **décide de l'accès** ; chaque relais installé ailleurs a ses propres comptes et reste entièrement indépendant.

Ce document est le cadrage, à valider avant la maquette et le code. État : **décisions du 23/11/2026** (tableau ci-dessous) ; les questions encore ouvertes sont à la fin.

## Décisions

| Sujet | Choix |
|---|---|
| Modèle | Gratuit pour l'instant ; le payant viendra (un champ « offre » sur le compte et l'organisation, rien d'autre aujourd'hui) |
| Confiance | **Le relais décide** : il délivre des jetons signés que la Station vérifie ; quatre garde-fous côté Station (plus bas) |
| Compte | **Facultatif** : réseau local, appairage par code, invitations ([invitations.md](invitations.md)) et relais sans comptes restent possibles |
| Flotte | Mes ordinateurs partout ; famille et amis ; équipes et entreprises (organisations, rôles, groupes, journal). Pas de SSO d'entreprise pour l'instant |
| Droits sur un ordinateur | **Session** (le bureau entier), **Prêt** (le lanceur, [pret.md](pret.md)), **Partage** (rejoindre ses partages sans code), **Administrer** |
| Relais auto-hébergé | Comptes propres à chaque relais ; une app peut être connectée à plusieurs relais |
| Connexion | E-mail et mot de passe, double authentification (TOTP), passkeys |
| Rattacher une Station | Depuis Admin (« Compte KyberNow › Se connecter ») |
| Invitations | Gardées, à côté des comptes |
| E-mails | Envoi réglable (SMTP dans la configuration du relais) ; le relais public passe par un prestataire de l'UE |
| Base | SQLite dans le conteneur du relais, sauvegardée |
| Où se gère quoi | **Console web du relais** (tout, flotte comprise) et **Admin** (rattacher la Station, interrupteur, accès vus et bloqués) ; les apps se connectent et montrent les ordinateurs du compte |
| Juridique | Les Lignes Claires, responsable du traitement |
| Calendrier | **Tout, avant la première soumission à l'App Store** |

## Ce que voit l'utilisateur

### Créer son compte et se connecter

- **Sur le site du relais** (`https://share.kybernow.app/compte`, la console) : « Créer un compte » (e-mail, mot de passe), e-mail de vérification, puis proposition d'ajouter une passkey et la double authentification.
- **Dans les apps Link** : « Se connecter à un compte KyberNow » (accueil vide, réglages de l'app). L'app ne voit jamais le mot de passe :
  - iPhone, iPad, Mac, Windows, Linux de bureau, Android : la page de connexion du relais s'ouvre dans une fenêtre du système (`ASWebAuthenticationSession`, Custom Tabs, navigateur) ; passkeys et double authentification y marchent comme sur le site ;
  - Apple TV, Android TV, téléviseurs, salon, Deck : un code de huit signes et un QR code s'affichent ; on l'approuve depuis son téléphone (« Connecter l'Apple TV du salon ? ») ;
  - plusieurs relais : « Ajouter un compte » demande d'abord le relais (`share.kybernow.app` proposé).
- La création et la suppression du compte se font sur le site, pas dans l'app (App Store, règle 5.1.1(v) : l'app ne crée pas de compte).

### Mes ordinateurs, partout

- Une fois connectée, l'app montre à l'accueil **les ordinateurs du compte**, avec les mêmes cartes qu'aujourd'hui ; un petit signe distingue un ordinateur venu du compte (et, avec plusieurs comptes, lequel).
- « Se connecter » ouvre la session, sur le réseau ou par le relais, sans appairage. Un ordinateur déjà appairé sur cet appareil n'apparaît qu'une fois.

### Rattacher une Station

- Admin › **Compte KyberNow** › « Se connecter » : Admin ouvre la console du relais dans le navigateur (avec un code de rattachement) ; on se connecte, on choisit le compte ou l'organisation, on confirme « Rattacher PC du salon ».
- La Station est alors **à ce compte** (ou à cette organisation) ; « Accepter les comptes KyberNow » s'active, et Admin montre le compte, la liste des accès, et « Détacher ».
- Le rattachement n'a lieu que depuis Admin : personne ne peut rattacher la Station d'un autre.

### Ajouter des personnes

- Console › un ordinateur › **« Ajouter une personne »** : son e-mail et ses droits (Session, Prêt, Partage, Administrer), éventuellement une échéance.
- La personne reçoit un e-mail ; avec un compte, l'ordinateur apparaît chez elle ; sans compte, le lien lui propose d'en créer un.
- **Prêt** : les apps permises, les créneaux et le compte Steam restent réglés dans Admin (page « Prêt ») ; le droit « Prêt » remplace, pour un compte, le compte d'ami (identifiant et mot de passe locaux), qui reste possible sans compte.
- **Partage** : la personne rejoint les partages de l'ordinateur sans taper de code, et sans attendre l'accord de l'hôte (elle est connue) ; l'hôte la voit et peut la retirer comme tout invité.

### Flotte (organisations)

- Une **organisation** (« Les Lignes Claires ») a des membres, avec un rôle : **Propriétaire**, **Administrateur**, **Membre** ; des **groupes** (« Graphistes ») ; des **ordinateurs** rattachés à l'organisation plutôt qu'à une personne.
- Les droits se donnent à une personne ou à un groupe. Le journal de l'organisation dit qui a ouvert quoi, quand, d'où, et chaque changement de droits.
- Double authentification **obligatoire** pour les Propriétaires et Administrateurs d'une organisation, et pour tout compte qui a le droit Administrer.

## Architecture

### Pièces

| Pièce | Où | Rôle |
|---|---|---|
| Comptes | `server/relay` (nouveau module `accounts`, SQLite) | Comptes, sessions, passkeys, TOTP, organisations, groupes, droits, journal, e-mails |
| Console | `server/relay` (pages servies par le relais, comme la page du partage) | Créer le compte, se connecter, gérer ordinateurs, personnes, organisation ; approuver un appareil ou un rattachement |
| Jetons | `server/relay/tunnel` (format partagé) | Jeton d'accès signé par le relais, vérifié par la Station |
| Station | Admin (`accounts.rs`, `accounts_api.rs`) | Rattachement, interrupteur, vérification des jetons, liste des accès, blocage |
| Cœur | `apps/core` (`account.rs`) | Connexion de l'appareil (navigateur ou code), ordinateurs du compte, jeton pour chaque appel |
| Apps | toutes | Écran de connexion, accueil avec les ordinateurs du compte, réglages « Comptes » |

### Le jeton d'accès

- Le relais a une **clé de signature** (Ed25519), publiée à `GET /v1/keys` ; la Station l'**épingle au rattachement**.
- L'appareil connecté demande au relais un jeton pour un ordinateur : `POST /v1/access {station}` avec sa session d'appareil. Le relais vérifie le compte, ses droits sur cet ordinateur, l'interrupteur côté relais, et rend un jeton **valable 5 minutes** : `{v, relay, station, account, device, rights, name, iat, exp, jti}` signé.
- L'appareil le présente à Admin (`Authorization: KyberNow <jeton>`), sur le réseau ou par le relais. Admin vérifie : signature (clé épinglée), `station` = la sienne, `exp`, interrupteur actif, compte et appareil non bloqués localement. Il en tire les droits.
- **Session** : Admin ouvre la session comme pour un appareil appairé, mais Kyber est atteint avec un **jeton de Kyber** (comme les invités du partage), pas avec un compte Kyber : pas de compte à créer, pas de redémarrage de Kyber, et la révocation sur le relais vaut au jeton suivant (le signe de vie redemande un jeton toutes les 4 minutes).
- Le relais ne voit toujours pas l'image ni les entrées : le tunnel reste de bout en bout avec Admin épinglé.

### Les quatre garde-fous de la Station

1. **Interrupteur** dans Admin, « Accepter les comptes KyberNow » : coupé, Admin refuse tout jeton (les appareils appairés, les invitations et le réseau local marchent comme avant).
2. **Liste locale** : Admin garde chaque compte et appareil entré par un jeton (nom, e-mail, droits, dernière venue) et peut en **bloquer** un, sans passer par le relais ; un compte bloqué ici l'est même si le relais dit oui.
3. **Jetons courts et signés** : 5 minutes, clé du relais épinglée, `jti` jamais rejoué.
4. **Avis à chaque nouvel appareil** : la première fois qu'un appareil ouvre un ordinateur, le relais envoie un e-mail au propriétaire (ou aux Administrateurs de l'organisation) et Admin met une ligne dans le fil.

### Routes (premier jet)

| Où | Route | Quoi |
|---|---|---|
| Relais | `GET /v1/keys` | Clé publique de signature |
| Relais | `POST /v1/devices/code` → `{code, poll}` ; `POST /v1/devices/poll` | Connexion d'un appareil par code (téléviseurs) |
| Relais | `GET /compte/connexion?appareil=…` | Connexion d'un appareil par la fenêtre du système (retour par le schéma `kybernow://compte`) |
| Relais | `GET /v1/computers` | Ordinateurs du compte, avec les droits et l'identifiant de Station |
| Relais | `POST /v1/access {station}` | Jeton d'accès (5 min) |
| Relais | `POST /v1/stations/attach` (Station, par sa connexion de contrôle) | Rattachement, après approbation dans la console |
| Admin | `GET/PUT /api/account` (session d'Admin) | Compte rattaché, interrupteur, détacher |
| Admin | `GET /api/account/access`, `POST /api/account/access/{id}/block` | Liste locale, blocage |
| Admin | toutes les routes d'un appareil appairé | Acceptent aussi `Authorization: KyberNow <jeton>` selon les droits |

### Données gardées par le relais

| Donnée | Pourquoi | Durée |
|---|---|---|
| E-mail, empreinte Argon2 du mot de passe, secret TOTP (chiffré), passkeys (clés publiques) | Se connecter | Vie du compte |
| Appareils connectés (nom, sorte, dernière venue) | Sessions d'appareil, révocation | Vie de l'appareil |
| Stations rattachées (identifiant, nom) | Ordinateurs du compte | Jusqu'au détachement |
| Organisations, membres, groupes, droits | Flotte | Vie de l'organisation |
| Journal (accès, changements) | Sécurité, flotte | 12 mois |
| Adresses IP | Sécurité, limites | Quelques jours (journaux), comme aujourd'hui |

Compte supprimé : effacé sous 30 jours (sauvegardes comprises). Export des données depuis la console.

### E-mails

SMTP réglé dans `relay.toml` (hôte, port, identifiants, expéditeur) ; sans SMTP, un relais auto-hébergé n'a ni vérification d'e-mail ni mot de passe oublié (la console le dit, l'administrateur du relais crée les comptes). Messages en français et en anglais : vérification, mot de passe oublié, nouvel appareil, ajout à un ordinateur ou à une organisation, double authentification changée.

### Sécurité du relais

- Mots de passe : Argon2id ; 10 caractères au moins ; liste des mots de passe connus refusée.
- Console : cookie de session `__Host-`, `Secure`, `HttpOnly`, `SameSite=Strict`, protection CSRF ; limites d'essais par adresse et par compte.
- Clé de signature : dans un fichier à part, réservé au relais ; rotation possible (deux clés publiées, la Station épingle les deux pendant la transition).
- Un relais piraté peut délivrer des jetons pour toute Station dont l'interrupteur est actif : c'est le prix du « relais décide », borné par les garde-fous ci-dessus.

## Juridique et magasins

- **RGPD** (Les Lignes Claires, responsable du traitement) : registre des traitements, politique de confidentialité du service (relais public), conditions d'utilisation, contrat de sous-traitance pour les organisations, notification d'une fuite sous 72 h, prestataire d'e-mails dans l'UE.
- **App Store** : connexion dans l'app, création et suppression sur le site (pas de création dans l'app) ; pas de Sign in with Apple (ni Google ni autre connexion tierce) ; un **compte de démonstration** dans les notes de l'App Review, plus simple que l'invitation.
- **Payant, plus tard** : abonnement vendu dans l'app iOS = achat intégré d'Apple (3.1.1) ; offre vendue directement aux organisations = possible hors achat intégré (3.1.3(c)).

## Lots

1. **Relais, comptes** : base SQLite, création, vérification, connexion, mot de passe oublié, TOTP, passkeys, console (compte), e-mails, limites.
2. **Rattachement et jetons** : clé de signature, rattachement depuis Admin, jeton d'accès, vérification dans Admin, les quatre garde-fous.
3. **Apps** : connexion par fenêtre du système et par code, ordinateurs du compte à l'accueil, jeton dans chaque appel du cœur, plusieurs relais.
4. **Personnes et droits** : ajouter par e-mail, droits Session, Prêt, Partage, Administrer ; prêt et partage avec un compte.
5. **Organisations** : membres, rôles, groupes, ordinateurs de l'organisation, journal, double authentification obligatoire.
6. **Juridique et App Store** : textes, compte de démonstration, notes de l'App Review mises à jour.

Chaque lot a sa maquette validée avant le code, comme les invitations.

## Questions ouvertes

1. **Administrer** : que permet ce droit ? Proposé : gérer les accès de cet ordinateur dans la console, redémarrer et mettre à jour la Station depuis l'app (y compris par le relais, ce qu'on refuse aujourd'hui) ; **pas** l'interface d'Admin entière par le relais pour l'instant.
2. **Partage avec un compte** : entrer sans l'accord de l'hôte (proposé), ou garder l'accord ?
3. **Mot de passe d'Admin** : reste-t-il l'accès local à Admin, ou un compte Administrer peut-il aussi ouvrir Admin sur le réseau ?
4. **Organisations** : un ordinateur peut-il appartenir à une personne et être partagé avec une organisation, ou seulement à l'une ou l'autre ?
5. **Compte de démonstration pour Apple** : sur le relais public, avec le droit Session sur la Station de démonstration (proposé).
