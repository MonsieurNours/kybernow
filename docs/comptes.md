# Comptes KyberNow : cadrage

[English](comptes.en.md) · Français

Un **compte KyberNow** (adresse e-mail, mot de passe, double authentification, passkeys) donne accès, depuis n'importe quel appareil, à tous les ordinateurs auxquels il a droit : les siens, ceux qu'on lui a ouverts (famille, amis), ceux de son équipe. Le relais devient un service à la Parsec : il tient les comptes, les droits, les organisations, et **décide de l'accès** ; chaque relais installé ailleurs a ses propres comptes et reste entièrement indépendant.

Ce document est le cadrage, à valider avant la maquette et le code. État : **décisions du 23/11/2026** (tableau ci-dessous), maquette : https://claude.ai/artifact/RSTsS6BYAt9SN9GEMEgiuc ; la question encore ouverte est à la fin.

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
| Rattacher une Station | Depuis l'installateur de la Station, ou depuis Admin (« Compte KyberNow › Se connecter ») |
| Installateurs | Étape facultative « Compte KyberNow » pendant l'installation de la Station (pages du relais, « Autre relais… », propriétaire, accès activés) ; jeton d'enrôlement pour déployer une flotte ; connexion proposée aussi par l'installateur de Link Windows |
| Invitations | Gardées, à côté des comptes |
| E-mails | Envoi réglable (SMTP dans la configuration du relais) ; le relais public passe par un prestataire de l'UE |
| Base | SQLite dans le conteneur du relais, sauvegardée |
| Où se gère quoi | **Console web du relais** (tout, flotte comprise) et **Admin** (rattacher la Station, interrupteur, accès vus et bloqués) ; les apps se connectent et montrent les ordinateurs du compte |
| Juridique | Les Lignes Claires, responsable du traitement |
| Calendrier | **Tout, avant la première soumission à l'App Store** |
| Administrer | Gérer les accès de l'ordinateur dans la console, redémarrer et mettre à jour la Station depuis l'app, **y compris par le relais** ; ouvrir Admin sur le réseau (ci-dessous). Pas l'interface d'Admin entière par le relais |
| Partage avec un compte | La personne qui a le droit Partage **entre sans l'accord de l'hôte** ; l'hôte la voit et peut la retirer |
| Admin sur le réseau | Le mot de passe d'Admin reste ; **un compte qui a le droit Administrer ouvre aussi Admin** (« Se connecter avec KyberNow », sur le réseau seulement) |
| Personne et organisation | Un ordinateur rattaché à une personne peut être **ouvert à une organisation** ou à l'un de ses groupes, comme à une personne |

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

### Dans les installateurs

- **Station** (installateur Windows et installateur Mac) : une étape **« Compte KyberNow »**, facultative (« Plus tard » : on rattache ensuite dans Admin), **pendant l'installation** (l'installation continue en fond). Elle affiche les **pages du relais** (inscription, connexion, passkey par Windows Hello ou Touch ID, double authentification) dans la fenêtre de l'installateur.
  - **Relais** : `share.kybernow.app` proposé ; « Autre relais… » ouvre un champ d'adresse (hôte, port, empreinte pour un relais signé par lui-même), vérifié avant de continuer. L'adresse choisie devient le relais de la Station (Admin › Hors de chez vous).
  - **Propriétaire** : la personne ; si le compte administre des organisations, un choix « Moi » / « {organisation} ».
  - **Rattachement** dès qu'Admin démarre ; « Accepter les comptes KyberNow » et « Accès hors de chez vous » s'activent, l'installateur le dit, Admin permet de couper.
- **Déploiement d'une flotte** : la console d'une organisation donne un **jeton d'enrôlement** (durée, nombre de Stations) ; l'installateur le prend en ligne de commande, en silencieux (`KyberNow-Station-Setup.exe /quiet /enroll=<jeton> /relay=<adresse>`), et rattache la Station à l'organisation sans écran.
- **KyberNow Link pour Windows** : l'installateur propose aussi, facultatif, « Se connecter à un compte KyberNow » (mêmes pages du relais) ; l'app démarre alors connectée. Les autres apps se connectent depuis leur accueil.

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

### Format du jeton (lot 2)

Défini dans le crate partagé `server/relay/tunnel` (module `access`) : le relais signe, la Station vérifie, le cœur des apps le transporte sans le lire.

- **Forme compacte** : `<charge>.<signature>`, chacune en base64url sans remplissage ; 4 Kio au plus. La charge est le JSON ci-dessous ; la signature Ed25519 porte sur `kybernow-access/1 ` suivi de la charge encodée (aucune autre signature de la clé du relais ne passe pour un jeton).
- **Charge** :

  ```json
  {"v":1,"relay":"share.kybernow.app","station":"<identifiant de la Station>",
   "account":"<compte>","device":"<appareil>","rights":["session","share"],
   "name":{"account":"Léa Martin","email":"lea@exemple.fr","device":"iPad de Léa"},
   "iat":1795000000,"exp":1795000300,"jti":"<16 octets en base32>"}
  ```

  `rights` : `session`, `lend`, `share`, `admin` (un mot inconnu d'une version suivante est ignoré). `name` porte ce qu'Admin affiche dans sa liste (l'e-mail est facultatif).
- **Clé publiée** (`GET /v1/keys`) : `{"keys":[{"kid":"<8 octets du SHA-256 de la clé, base32>","alg":"Ed25519","key":"<32 octets, base64url>"}]}` ; deux clés pendant une rotation. Admin montre l'empreinte `sha256:…` de la clé brute.
- **Vérification par la Station**, dans cet ordre : clé épinglée (une des clés du rattachement), version, `relay` = le relais du rattachement, `station` = la sienne, `exp - iat` de 5 minutes au plus, `iat` pas dans le futur et `exp` pas passé, à **60 s** près (horloges), puis le `jti`.
- **`jti` jamais rejoué** : le même jeton sert à tous les appels de son appareil pendant ses 5 minutes (session, signe de vie), mais un `jti` n'est jamais accepté pour un autre contenu ; pour redémarrer ou mettre à jour la Station, le jeton est **à usage unique** (consommé, il ne sert plus à rien d'autre). Les `jti` sont oubliés à leur échéance.
- En-tête : `Authorization: KyberNow <jeton>`.

### Admin (lot 2)

- **Réglages** : section `[account]` de `kyber-admin.toml` : `relay`, `owner` (`kind` personne ou organisation, `id`, `name`, `email`), `keys` (clés épinglées), `pinned_at`, `accept` (l'interrupteur). Le rattachement (dialogue avec le relais) appellera `accounts::pin` ; il allume l'interrupteur, qu'Admin permet de couper.
- **Liste locale** : `account-access.json` du dossier de données, une entrée par compte et appareil (noms, droits du dernier jeton, première et dernière venue, `lan` ou `relay`) ; blocage d'un appareil ou du compte entier. Une première venue met une ligne dans le fil. Détacher efface la liste.
- **Routes** (session d'Admin, jamais par le relais) :

  | Route | Corps | Réponse |
  |---|---|---|
  | `GET /api/account` | | `{attached, relay, owner, keys:[{kid, fingerprint}], pinned_at, accept, test, station, console, can_attach}` |
  | `PUT /api/account` | `{accept?: bool, detach?: true}` | La même vue ; allumer sans rattachement : `409 not_attached` |
  | `GET /api/account/access` | | `{entries:[{id, account, device, name, email, device_name, rights, first_seen, last_seen, via, blocked, account_blocked, owner}]}` |
  | `POST /api/account/access/{id}/block` | `{blocked: bool, account?: bool}` | La liste ; `account` : tout le compte |

- **Selon les droits**, sur le réseau comme par le relais (classe `Device` de la garde, « Accès hors de chez vous » actif) :
  - **Session** : les routes `/api/stream/*` et celles de l'hôte (partage, prêt) comme un appareil appairé ; `prepare` rend en plus `kyber_token` `{token, user, expires}`, le jeton de Kyber (5 min) sous le compte `kn-account-…`, jamais écrit dans la configuration de Kyber ;
  - **Partage** : `POST /api/share/join` sans `code` entre tout de suite (ni code, ni accord de l'hôte), sous le nom de la personne ; l'hôte le voit et peut le retirer ;
  - **Administrer** : `POST /api/stream/station/restart` et `/update`, aussi **par le relais** (fermés par le relais à un compte Kyber), avec un jeton à usage unique ;
  - **Prêt** : porté par le jeton et montré dans la liste ; le lanceur avec un compte vient au lot 4.
- **Refus** : `{error, code}`, `code` parmi `accounts_off`, `not_attached`, `bad_token`, `token_expired` (401 : l'app redemande un jeton), `blocked`, `no_right` (403).
- **Mode d'essai** : `KYBERNOW_ACCOUNT_TEST=1` au démarrage épingle une clé locale (`account-test.key`, relais `essai.kybernow.invalid`) quand la Station n'est rattachée à aucun relais ; `kyber-admin account-token --rights session,share` écrit un jeton signé par elle pour cette Station.
- **Code** : `server/admin/src/accounts.rs` (réglages, vérification, liste, routes), branché dans `api.rs` (`stream_user`, redémarrer et mettre à jour), `relay.rs` (garde) et `share_api.rs` (rejoindre).
- **Interface** : espace « Compte KyberNow » d'Admin (`#/compte`) : rattachée, le compte, le relais, la clé épinglée, « Gérer dans la console », « Détacher », l'interrupteur et la liste (« Bloquer », « Bloquer le compte », « Débloquer ») ; pas rattachée, « Se connecter », inactif tant que le relais ne propose pas le rattachement (`can_attach`).

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
- Console : cookie de session `__Host-`, `Secure`, `HttpOnly`, `SameSite=Lax`, protection CSRF (jeton par formulaire, `Sec-Fetch-Site` vérifié) ; limites d'essais par adresse et par compte.
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

## Lot 1 : comptes sur le relais

Maquette : rangée « Inscription et connexion » de https://claude.ai/artifact/RSTsS6BYAt9SN9GEMEgiuc.

### Pages (servies par le relais, français et anglais, comme la page du partage)

| Route | Écran |
|---|---|
| `GET/POST /compte/inscription` | « Créer un compte » : nom, adresse e-mail, mot de passe (10 caractères au moins, jauge, mots de passe connus refusés), conditions et confidentialité ; refusé quand la page vient d'une app (`?appareil=`) |
| `GET/POST /compte/verifier` | « Vérifiez votre e-mail » : lien valable 24 h, ou code à six chiffres ; « Renvoyer l'e-mail », « Le changer » |
| `GET /compte/proteger` | « Protégez votre compte », après l'activation : passkey (conseillée), double authentification ; « Plus tard » |
| `GET/POST /compte/connexion` | « Se connecter » : passkey d'abord, puis e-mail et mot de passe ; « Mot de passe oublié ? » ; « Créer un compte » sauf depuis une app |
| `GET/POST /compte/code` | « Double authentification » : code TOTP, « Ne plus demander sur cet appareil pendant 30 jours », « Utiliser un code de secours » |
| `GET/POST /compte/oubli`, `/compte/nouveau-mot-de-passe` | Lien valable une heure ; réponse toujours la même (« Si un compte existe à cette adresse, un lien vient d'y partir. ») ; un nouveau mot de passe ferme les autres sessions |
| `GET /compte` | Le compte (en attendant la console du lot 4) : e-mail, sécurité (passkeys, double authentification, codes de secours), sessions ouvertes, « Se déconnecter », « Supprimer le compte » |
| `POST /compte/connexion/passkey` | « Continuer avec une passkey » (lot 1b) : la session s'ouvre entière, sans code de double authentification |
| `POST /compte/passkeys/ajouter`, `/renommer`, `/retirer` | Ajouter une passkey (depuis `/compte` ou `/compte/proteger`), la renommer, la retirer |
| `GET/POST /compte/supprimer` | « Supprimer le compte » : mot de passe ou passkey redemandé, puis effacement et e-mail de confirmation |
| `POST /compte/export` | « Exporter mes données » : `kybernow-compte.json` (compte, conditions acceptées, sécurité sans secret, sessions, journal) |
| `GET /conditions`, `/confidentialite` | Conditions d'utilisation et politique de confidentialité ([juridique](juridique/README.md)), brouillons marqués comme tels, servis avec les comptes |

### Base (SQLite, `accounts.db` dans le volume du relais)

`accounts` (id, e-mail unique en minuscules, nom, empreinte Argon2id, vérifié le, créé le, supprimé le, offre) · `email_tokens` (empreinte SHA-256 du jeton, compte, usage `verify`/`reset`, code à six chiffres, échéance) · `sessions` (empreinte du jeton, compte, appareil, créée, vue, échéance, 2FA faite) · `totp` (compte, secret chiffré, activée le) · `recovery_codes` (compte, empreinte, utilisé le) · `passkeys` (compte, identifiant, clé publique, compteur, nom, créée, vue) · `trusted_browsers` (empreinte, compte, échéance) · `audit` (compte, type, adresse, quand). Migrations versionnées au démarrage.

Migration 2 (lot 1b) : sur `accounts`, `webauthn_user` (identifiant WebAuthn, 32 octets aléatoires), `terms_version`, `terms_accepted_at`, `privacy_version`, `privacy_accepted_at` ; sur `passkeys`, `alg` (COSE) et `transports` ; `webauthn_used` (empreinte d'un défi déjà servi, échéance). La migration 3 est réservée au lot 2.

### Réglages (`relay.toml`, section `[accounts]`)

`enabled` (coupé par défaut sur un relais auto-hébergé), `database`, `secret_key` (fichier : chiffrement des secrets TOTP et signature des cookies), `public_url`, et `[accounts.smtp]` : `host`, `port`, `starttls`, `user`, `password_file`, `from`. Sans SMTP : ni vérification par e-mail ni mot de passe oublié ; l'administrateur du relais crée les comptes (`kybernow-relay accounts add`).

### Sécurité

Cookie `__Host-kn_session` (`Secure`, `HttpOnly`, `SameSite=Lax`), jeton de 256 bits gardé en empreinte ; jeton CSRF par formulaire ; Argon2id ; 10 essais de connexion par adresse et 5 par compte en 15 minutes ; e-mails et jetons jamais dans les journaux ; réponses identiques qu'un compte existe ou non (connexion, oubli, inscription d'une adresse déjà prise : un e-mail le dit au titulaire).

Passkeys (lot 1b) : vérification écrite sur `ring`, sans bibliothèque WebAuthn (celles en Rust tirent OpenSSL, absent de l'image) : CBOR et clés COSE lus par le relais, ES256, Ed25519 et RS256, aucune attestation demandée, vérification de l'utilisateur exigée (une passkey vaut double authentification), compteur qui recule refusé. Les défis ne sont pas gardés en base : signés par la clé du relais, liés au navigateur (cookie CSRF) et à l'usage, valables 10 minutes, notés une fois servis (à usage unique). Les pages gardent un seul script, `/assets/passkey.js`, servi par le relais (`script-src 'self'` sur `/compte/…` seulement) ; sans lui, les boutons des passkeys n'apparaissent pas.

### Ordre

1a. Base, inscription, vérification, connexion, déconnexion, mot de passe oublié, double authentification et codes de secours, SMTP, limites, page du compte. **Fait.**
1b. Passkeys (WebAuthn), suppression du compte, export des données ; version et date des conditions acceptées, pages `/conditions` et `/confidentialite` ; effacement des comptes jamais vérifiés après 7 jours. **Fait.**

Écarts du lot 1b :

- la suppression efface le compte **tout de suite** (pas de délai de 30 jours en base) ; les 30 jours sont ceux de la rotation des sauvegardes ;
- les textes juridiques servis sont ceux du relais public, en brouillon ; un relais auto-hébergé n'a pas encore de réglage pour servir les siens, et les versions précédentes ne sont pas encore consultables ;
- la connexion par passkey ne propose pas encore la saisie automatique (`mediation: "conditional"`) : un bouton suffit pour l'instant.

## Question ouverte

1. **Compte de démonstration pour Apple** : sur le relais public, avec le droit Session sur la Station de démonstration (proposé).
