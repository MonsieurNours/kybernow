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
| E-mails | Envoi réglable (SMTP dans la configuration du relais) ; le relais public passe par SendGrid (Twilio), expéditeur `hello@kybernow.app` |
| Base | SQLite dans le conteneur du relais, sauvegardée |
| Où se gère quoi | **Console web du relais** (tout, flotte comprise) et **Admin** (rattacher la Station, interrupteur, accès vus et bloqués) ; les apps se connectent et montrent les ordinateurs du compte |
| Juridique | Les Lignes Claires, responsable du traitement |
| Calendrier | **Tout, avant la première soumission à l'App Store** |
| Administrer | Gérer les accès de l'ordinateur dans la console, redémarrer et mettre à jour la Station depuis l'app, **y compris par le relais** ; ouvrir Admin sur le réseau (ci-dessous) ; **tout Admin de n'importe où** avec « Admin à distance », coupé au départ (« Admin depuis KyberNow Share », décidé le 25/11/2026) |
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

### Regarder, liens d'accès et groupes d'ordinateurs

Décidé le 25/11/2026.

**Regarder** (droit `view`, « Regarder » / “Watch”) : voir l'écran d'un ordinateur sans le prendre en main : ni clavier, ni souris, ni manette, ni presse-papiers.
- Phrase du droit : « Voir l'écran de l'ordinateur, sans clavier, souris ni manette. » ; exemple : « Votre fils qui veut vous montrer sa partie ». Anglais : “See the computer's screen, with no keyboard, mouse or controller.” ; “Your son who wants to show you his game”.
- C'est le **vrai écran**, tel quel : pas d'écran virtuel, rien ne change pour la personne qui utilise l'ordinateur. Regarder passe par le partage (KyberNow Share) : la personne rejoint le partage ouvert de l'ordinateur comme un invité **spectateur** ; s'il n'y en a pas, la Station en ouvre un pour elle, de l'écran principal du PC, ou de la session en cours si quelqu'un est connecté. Ce partage n'a pas de code à montrer ; il se ferme quand le dernier spectateur part.
- **Plusieurs écrans** : la barre propose « Écran » et la liste des écrans du PC ; en changer relance l'image sur l'autre écran.
- **L'hôte le sait** : le fil d'Admin dit « {prénom} regarde votre écran », le panneau du partage (Admin, icône, barre de session) la montre comme tout invité, avec « Retirer » ; l'hôte peut aussi lui donner les manettes ou le clavier, comme à tout invité.
- **Refus** : ordinateur prêté (« {nom} est prêté en ce moment. »), partage coupé dans Admin, comptes coupés, Station trop ancienne (« Cette Station ne permet pas encore de regarder : mettez-la à jour. »).
- Avec le droit « Ouvrir le bureau », la carte garde « Se connecter » ; « Regarder » est alors dans le menu de l'ordinateur.

**Liens d'accès** (« Liens d'accès » / “Access links”, page d'un ordinateur, pour qui gère ses accès) : ouvrir l'ordinateur à quelqu'un dont on n'a pas l'adresse, par un lien à **usage unique**.
- Créer : les droits (les mêmes tuiles), « Le lien sert pendant » 1, 7 (proposé) ou 30 jours, et « Accès jusqu'au (facultatif) ». Le lien (`https://{relais}/compte/lien/kla_…`) est **montré une seule fois**, avec « Copier le lien » et « Envoyez-le à une seule personne : il sert une fois et n'est montré qu'ici. » ; le relais n'en garde que l'empreinte. Dix liens en attente au plus par ordinateur (« Révoquez un lien avant d'en créer un autre. »).
- Ouvrir le lien : sans être connecté, la page propose de se connecter ou de créer un compte, puis revient au lien ; connecté : « {nom} vous ouvre {ordinateur} », les droits en mots, l'échéance éventuelle, « Accepter ». Accepter donne ces droits au compte (ajoutés à ceux qu'il a déjà sur cet ordinateur, l'échéance la plus lointaine l'emporte), et le lien est **utilisé** : il ne sert plus à personne. Le propriétaire qui ouvre son propre lien : « C'est votre ordinateur : ce lien est pour quelqu'un d'autre. », le lien reste en attente.
- Liste : « En attente · jusqu'au {date} », « Utilisé par {nom} le {date} », « Expiré », « Révoqué » ; « Révoquer » (« Révoquer ce lien ? Il ne servira plus. »). Révoquer un lien déjà utilisé ne retire pas l'accès : on le retire dans « Qui y a accès ».
- Refus à l'ouverture : « Ce lien a déjà servi. », « Ce lien a expiré. », « Ce lien a été révoqué. », « Ce lien est illisible. ».
- Journal : création, utilisation (qui) et révocation, au journal de la famille ou de l'équipe pour un de ses ordinateurs, et au journal du compte.

**Groupes d'ordinateurs** (famille ou équipe, onglet « Ordinateurs ») : une **liste d'ordinateurs** de la famille ou de l'équipe, choisis à la main (« Postes de l'atelier »).
- Créer (« Nom »), ajouter ou retirer un ordinateur, supprimer le groupe (« Supprimer le groupe « {nom} » ? Les droits donnés sur ce groupe partent avec lui. »). Un ordinateur peut être dans plusieurs groupes ; un ordinateur qui quitte la famille ou l'équipe quitte ses groupes.
- **Ouvrir un groupe** : à toute la famille ou l'équipe, à un de ses groupes de personnes, ou à un de ses membres, avec des droits et une échéance facultative. Ces droits valent pour tous les ordinateurs du groupe, **présents et à venir**, et s'ajoutent à ceux donnés ordinateur par ordinateur.
- Sur la page d'un ordinateur, « Qui y a accès » montre aussi les droits venus d'un groupe : « Par le groupe {groupe} », sans « Retirer » (on les change sur le groupe).
- **Déployer** : un jeton d'enrôlement peut ranger d'office les ordinateurs enrôlés dans un groupe d'ordinateurs (« Ranger dans le groupe »).
- Gérer les groupes d'ordinateurs : les Propriétaires et Administrateurs, avec la double authentification.

### Invitations, ajout de membres et sessions

Décidé le 25/11/2026.

**Invitations** (une personne ajoutée à un ordinateur par son adresse, un membre invité dans une famille ou une équipe) :
- Une invitation en attente **expire après 30 jours** ; les droits qu'elle portait ne passent plus au compte. Elle reste dans la liste, « Invitation expirée », jusqu'à ce qu'on la renvoie ou la retire.
- Sous l'adresse : « Invitation envoyée le {date} · expire le {date} » ; expirée : « Invitation expirée le {date} ».
- **« Renvoyer l'invitation »** : renvoie l'e-mail et repart pour 30 jours (aussi pour une invitation expirée) ; une fois par heure au plus et par invitation (« Invitation déjà renvoyée il y a moins d'une heure. ») ; avis « Invitation renvoyée à {adresse}. » ; une ligne au journal.
- Anglais : “Invitation sent on {date} · expires on {date}”, “Invitation expired on {date}”, “Resend the invitation”, “Invitation already resent less than an hour ago.”, “Invitation resent to {address}.”.

**Ajouter quelqu'un** (« Ajouter une personne », « Inviter un membre », « Ouvrir un groupe » à un membre) :
- Le champ de l'adresse **propose les personnes déjà connues** à mesure qu'on tape : les membres de vos familles et équipes, et les personnes à qui vous avez déjà ouvert un ordinateur, « Prénom Nom — adresse ». Sans script : une liste du navigateur (`datalist`).
- **Pas de doublon** : les adresses sont comparées sans tenir compte des majuscules ni des espaces ; ajouter quelqu'un qui a déjà un accès à cet ordinateur change ses droits au lieu d'ajouter une ligne (« {nom} avait déjà accès : ses droits sont mis à jour. ») ; inviter un membre déjà dans la famille ou l'équipe est refusé (« {nom} en fait déjà partie. ») ; une invitation en attente pour la même adresse est remplacée, pas doublée. La page « Personnes » montre chaque personne une fois, même ajoutée par son adresse sur un ordinateur et par son compte sur un autre.
- Anglais : “{name} already had access: their rights are updated.”, “{name} is already a member.”.

**Sessions** :
- Le jeton d'accès qu'un appareil présente à une Station vaut **2 minutes** (au lieu de 5) ; les apps le renouvellent d'elles-mêmes. Une Station accepte toujours jusqu'à 5 minutes (relais plus ancien).
- Une **session d'appareil** (une app connectée au compte) **expire après 90 jours sans servir** : l'app dit « Ce compte a été déconnecté de cet appareil : reconnectez-vous. », comme après une déconnexion depuis la page du compte.
- « Compte et sécurité », liste des appareils : « Utilisé il y a {durée} » ; aide « Un appareil qui ne sert pas pendant 90 jours est déconnecté. » / “Used {duration} ago”; “A device unused for 90 days is signed out.”.

### Réglages synchronisés et déplacer un ordinateur

Décidé le 25/11/2026.

**Réglages synchronisés** : les réglages d'un ordinateur du compte suivent la personne d'un appareil à l'autre.
- Ce qui suit : le **profil** (Auto, Jeu, Bureau…), le **format vidéo**, le **débit**, le **texte net (4:4:4)**, le **HDR** et le **mode immersif** de chaque ordinateur. Ce qui reste sur l'appareil : son écran et sa définition, son son, ses manettes, ses raccourcis, ses préférences d'app.
- Gardés par le relais, par compte et par ordinateur (un petit objet, 4 Ko au plus) ; le dernier changement l'emporte. Un appareil les lit en démarrant, à chaque rafraîchissement du compte et avant une session ; il les envoie quand on change un de ces réglages. Une valeur qu'un appareil ne sait pas appliquer (format qu'il ne décode pas, HDR sans écran HDR, mode immersif sur un téléviseur) est gardée telle quelle, mais l'appareil applique ce qu'il peut, comme aujourd'hui.
- Pour un ordinateur à la fois appairé et dans un compte (même Station), les réglages du compte valent aussi.
- Réglages de l'app, section « Comptes » : « Synchroniser les réglages des ordinateurs » (allumé par défaut), aide « Le profil, le format, le débit, le HDR et le mode immersif de chaque ordinateur vous suivent sur vos appareils. » / “Sync computer settings”; “Each computer's profile, format, bitrate, HDR and immersive mode follow you on your devices.”.
- Supprimés avec le compte ; un ordinateur détaché ou qui n'est plus ouvert au compte perd les siens.

**Déplacer un ordinateur** (page d'un ordinateur, « Déplacer… » / “Move…”) : vers vous, ou vers une famille ou une équipe que vous gérez, depuis vous ou depuis une famille ou une équipe que vous gérez (Propriétaire ou Administrateur, double authentification, des deux côtés).
- Choisir la destination, puis un **aperçu** « Ce qui change » / “What changes” : « Perdent l'accès : » les familles, équipes et groupes de l'ancien propriétaire et les accès venus d'un groupe d'ordinateurs ; « Quitte les groupes d'ordinateurs : {groupes} » ; « Liens en attente annulés : {n} ». Une case « Garder les accès donnés à des personnes » / “Keep the access given to people” (cochée) garde les droits donnés une par une à des personnes ; décochée, ils partent aussi.
- Bouton « Déplacer {ordinateur} » / “Move {computer}”, puis l'avis « {ordinateur} est maintenant à {destination}. » / “{computer} now belongs to {destination}.”. Le journal des deux côtés le note ; la Station l'apprend comme un rattachement (Admin montre le nouveau propriétaire).
- Refus : « Vous ne gérez pas {destination}. », « Activez d'abord la double authentification. ».

## L'espace du compte

Décidé le 25/11/2026 avec la maquette (https://claude.ai/artifact/KVvUYFTNCdRFRmYTSTtXev). La console du relais (`/compte/…`) devient une appli : un **menu latéral fixe** et une **colonne de contenu de largeur constante** (920 px), quelle que soit la page ; au téléphone (moins de 880 px), le menu devient une **barre d'onglets en bas**.

- **Menu** : « Ordinateurs » (avec leur nombre), « Personnes », « Familles et équipes » (puis chacune, avec son initiale dans sa couleur), « Compte et sécurité » ; en bas, « Pour bien démarrer » (étapes faites sur 4) et la personne connectée, avec « Se déconnecter ». Le logo est celui de KyberNow Share.
- **Famille ou équipe** : une organisation a désormais un **genre**, choisi à sa création (`kind` : `family` ou `team`, migration 6 ; les organisations d'avant sont des équipes). Les deux ont les mêmes droits, rôles et groupes ; les mots et les exemples changent (« Famille Martin », « L'atelier »), une famille a la couleur ambre, une équipe le bleu ciel, et seule une équipe a l'onglet « Déployer » (jetons d'enrôlement). « Familles et équipes » explique le principe par un schéma : des personnes, réunies dans un groupe, ouvrent des ordinateurs.
- **Droits, en mots de tous les jours** : Session devient « Ouvrir le bureau », Prêt « Emprunter pour jouer », Partage « Rejoindre vos partages », Administrer reste. Chaque droit a une phrase et un exemple (« Votre neveu qui joue le mercredi après-midi ») ; on les coche en tuiles. Les noms techniques (`session`, `lend`, `share`, `admin`) ne changent pas.
- **Pages** : « Ordinateurs » (cartes, « Rattacher un ordinateur », « Ouverts pour vous », une section par famille ou équipe) ; la page d'un ordinateur (« Qui y a accès », « Ce que chaque droit permet », « Réglages ») ; « Personnes » (`/compte/personnes` : tous ceux à qui l'on a ouvert ses ordinateurs) ; « Familles et équipes » (`/compte/familles`) ; la page d'une famille ou d'une équipe (onglets Membres, Groupes, Ordinateurs, Journal, Déployer) ; « Compte et sécurité » (`/compte`).
- **Panneaux** : « Ajouter une personne », « Ouvrir à une famille ou une équipe », « Inviter un membre », « Créer un groupe », « Nouveau jeton » et « Rattacher un ordinateur » s'ouvrent dans un panneau à droite, par l'ancre de l'adresse (`#ajouter`), sans script ; un formulaire refusé rouvre son panneau.
- **Tour d'accueil** (`/compte/bienvenue?etape=0…3`) : plein écran, une idée par écran avec son illustration : bienvenue, protéger son compte (passkey, ou la double authentification), rattacher son ordinateur (la page se recharge toutes les 5 s tant qu'aucun n'est rattaché), l'ouvrir à une famille ou une équipe. Il s'ouvre après la vérification de l'adresse, et la première fois que « Ordinateurs » s'ouvre ; « Passer » ou la fin le marquent vu (`welcomed_at`, migration 6). Une passkey ajoutée depuis le tour ramène à l'étape suivante (`suite=bienvenue`).
- **« Pour bien démarrer »** : rattacher un ordinateur, protéger son compte, ouvrir un ordinateur à quelqu'un, créer sa famille ou son équipe ; la carte disparaît quand les quatre sont faites.
- **Transitions** : les pages passent de l'une à l'autre par les transitions du navigateur (`@view-transition`) : le menu reste, le contenu glisse ; les étapes du tour glissent de droite à gauche. `prefers-reduced-motion` les coupe. Toujours aucun script, sauf `passkey.js`.
- **Après la connexion**, l'espace s'ouvre sur « Ordinateurs » (`/compte/ordinateurs`), plus sur la page du compte.
- **Voir les pages sans relais** : `KN_PAGES_DIR=<dossier> cargo test ecrire_les_pages -- --ignored` (dans `server/relay`) écrit toutes les pages, en français et en anglais, avec les fichiers du relais ; `python3 -m http.server` les sert.

## Admin depuis KyberNow Share

Décidé avec l'auteur le 25/11/2026 ; **revient sur une décision du cadrage** (« Pas l'interface d'Admin entière par le relais », tableau des décisions) : avec un nouvel interrupteur, coupé au départ, **tout Admin** s'ouvre de n'importe où, par le relais, pour un compte qui a le droit Administrer. Validée et écrite le 25/11/2026 (tunnel, relais, Admin, icône Windows et Mac) ; pas encore essayée sur une vraie Station.

### Ce que voit l'utilisateur

- **Le lien « Ouvrir Admin »**, pour qui a le droit Administrer (propriétaire, ou propriétaire et administrateur de la famille ou de l'équipe, avec la double authentification) : dans l'en-tête de la page d'un ordinateur (bouton), sur sa carte dans « Ordinateurs » (petit raccourci « Admin »), et sur chaque ligne de l'onglet « Ordinateurs » d'une famille ou d'une équipe.
- **Un clic, déjà connecté** : le lien mène à « Ouvrir Admin de {ordinateur} ? », puis « Confirmer avec une passkey » ; Admin s'ouvre, session ouverte, sans son mot de passe.
- **Où ça marche** :
  - « Admin à distance » allumé sur la Station : partout, par `https://<id>.share.kybernow.app` ;
  - coupé : seulement sur le réseau de l'ordinateur, par son adresse locale ; la page le dit (« Admin à distance est coupé sur {ordinateur} : ce lien ne marche que chez vous, sur le même réseau. Allumez-le dans Admin › Hors de chez vous. ») et propose quand même « Ouvrir sur le réseau ».
  - Station hors ligne : le lien est grisé, « Hors ligne ».
- **Interrupteur dans Admin**, Hors de chez vous : « Admin à distance » (« Les administrateurs de cet ordinateur ouvrent Admin depuis KyberNow Share, de n'importe où. »), **coupé au départ** ; il demande « Accès hors de chez vous » allumé et la Station rattachée à un compte. Il ne s'allume que **sur place** (session d'Admin sur le réseau) ; il se coupe de partout.
- **Avertissements à chaque ouverture à distance** :
  - l'ordinateur le dit : sur Mac, une notification « Admin est ouvert à distance par {nom} » ; sous Windows, le panneau de l'icône s'ouvre avec la même ligne (l'icône n'a pas de notifications) ; et Admin montre un bandeau « Admin ouvert à distance, par KyberNow Share. » à qui l'a ouvert ainsi ;
  - une ligne dans le fil d'Admin et, pour l'ordinateur d'une famille ou d'une équipe, dans son journal ;
  - un e-mail aux propriétaires (et aux administrateurs d'une famille ou d'une équipe), quand le relais envoie des e-mails : « Admin de {ordinateur} ouvert à distance par {nom}, {date}, depuis {ville ou adresse}. » ;
  - la session d'Admin ouverte par le relais **se ferme après 30 minutes sans activité** (et au plus 8 heures).
- **Passkey exigée** à distance : il faut confirmer par une passkey à chaque ouverture, même déjà connecté. Un compte sans passkey voit « Ajoutez d'abord une passkey à votre compte : elle protège l'ouverture d'Admin à distance. » Sur le réseau, la règle reste celle d'aujourd'hui (double authentification faite).
- **Tout Admin** à distance : les mêmes pages et réglages que sur le réseau, y compris le mot de passe d'Admin, les ports, le certificat, le compte rattaché et la désinstallation. Une seule exception, de bon sens : « Admin à distance » ne s'allume pas à distance.

### Comment

- **Adresse locale** : la Station donne au relais, dans `hello` (et `admin_status` quand elles changent), ses adresses d'Admin sur le réseau (`admin_lan`: `["https://192.168.1.20:8443", "https://pc-salon.local:8443"]`) et `remote_admin` (l'interrupteur). Le relais les garde **en mémoire seulement**, pour la Station connectée, et ne les montre qu'à un compte qui la gère (`/v1/computers` les porte pour ces comptes ; la console aussi). Ni base, ni journal.
- **Départ** : la console mène à `GET /compte/admin?id=<station>` (relais) : il vérifie le droit Administrer et la double authentification, puis renvoie vers le départ d'Admin :
  - à distance : `https://<id>.share.kybernow.app/api/login/kybernow/start` ;
  - sur le réseau : `<admin_lan>/api/login/kybernow/start`.
- **Admin** : `GET /api/login/kybernow/start` (nouveau) tire le `nonce` (cookie `kyber_admin_kn`, ou `__Host-kyber_admin_kn` par le relais) et renvoie vers `https://<relais>/compte/ouvrir-admin?station=…&nonce=…&retour=<son origine>`, comme le bouton « Se connecter avec KyberNow » d'aujourd'hui.
- **Relais, `/compte/ouvrir-admin`** : `retour` est soit une adresse du réseau local (règle d'aujourd'hui), soit **exactement** `https://<id>.<domaine du relais>[:port public]` de cette Station (nouveau). Pour celle-ci : Station connectée avec `remote_admin` vrai, et **assertion de passkey** sur la page de confirmation (défi lié au navigateur, comme pour « Supprimer le compte »). Le jeton délivré porte `via: "relay"` en plus (2 minutes depuis la migration 8, usage unique, `jti` = `nonce`).
- **Admin, par le relais** (`Via::Relay { kind: admin_http }`) : la garde ouvre **toutes** les routes de la session d'Admin à une session ouverte par `/api/login/kybernow` **par le relais** avec un jeton `via: "relay"`, `remote_admin` allumé, droit Administrer. Sinon, rien ne change (« Garde d'Admin selon la voie », relais.md). Cookie de session par le relais : `__Host-kyber_admin_session` (`Secure`, `HttpOnly`, `SameSite=Strict`, sans `Domain`). Inactivité : 30 minutes ; au plus 8 heures. Couper « Admin à distance » ferme aussitôt ces sessions ; détacher la Station aussi.
- **Exception** : `PUT /api/relay` qui allumerait `remote_admin` est refusé par le relais (`403 network_only`).
- **Avertissements** : Admin note `admin_distance_ouvert {compte, nom}` dans son fil, montre la notification par l'icône (`/api/local/state` porte `remote_admin_sessions`), et dit au relais `admin_opened {account}` ; le relais écrit au journal de la famille ou de l'équipe et envoie l'e-mail (`Message::AdminOpened`), au plus un par compte et par heure.
- **Confiance** : par le navigateur, le relais déchiffre le trafic (relais.md, « Le relais ne voit pas… ») : son exploitant pourrait voir les pages d'Admin. Le dire à l'interrupteur (« Le relais voit ce qui passe, comme pour un invité dans le navigateur. ») et dans les conditions du relais public.

### Textes

| Où | Français | English |
|---|---|---|
| Console, bouton | « Ouvrir Admin » ; raccourci des cartes : « Admin » | “Open Admin”; card shortcut: “Admin” |
| Console, confirmation | « Ouvrir Admin de {ordinateur} ? » ; « De n'importe où, par KyberNow Share. » ou « Sur le réseau de l'ordinateur. » ; « Confirmer avec une passkey » ; « Annuler » | “Open {computer}'s Admin?”; “From anywhere, through KyberNow Share.” or “On the computer's network.”; “Confirm with a passkey”; “Cancel” |
| Console, coupé | « Admin à distance est coupé sur {ordinateur} : ce lien ne marche que chez vous, sur le même réseau. Allumez-le dans Admin › Hors de chez vous. » ; « Ouvrir sur le réseau » | “Remote Admin is off on {computer}: this link only works at home, on the same network. Turn it on in Admin › Away from home.”; “Open on the network” |
| Console, sans passkey | « Ajoutez d'abord une passkey à votre compte : elle protège l'ouverture d'Admin à distance. » | “Add a passkey to your account first: it protects opening Admin remotely.” |
| Admin, interrupteur | « Admin à distance » ; « Les administrateurs de cet ordinateur ouvrent Admin depuis KyberNow Share, de n'importe où. Le relais voit ce qui passe, comme pour un invité dans le navigateur. » ; « S'allume seulement ici, sur le réseau. » | “Remote Admin”; “This computer's administrators open Admin from KyberNow Share, from anywhere. The relay sees what goes through, as for a guest in the browser.”; “Can only be turned on here, on the network.” |
| Admin, bandeau à distance | « Admin ouvert à distance, par KyberNow Share. » ; « Se déconnecter » | “Admin opened remotely, through KyberNow Share.”; “Sign out” |
| Notification (icône) | « Admin est ouvert à distance par {nom}. » | “Admin is open remotely by {name}.” |
| E-mail | « Admin de {ordinateur} ouvert à distance » ; « {nom} a ouvert Admin de {ordinateur} le {date}, depuis {lieu}. Ce n'est pas normal ? Coupez « Admin à distance » dans Admin, sur l'ordinateur. » | “{computer}'s Admin opened remotely”; “{name} opened {computer}'s Admin on {date}, from {place}. Not expected? Turn off “Remote Admin” in Admin, on the computer.” |

### Essais à prévoir

Relais : `retour` d'une autre Station refusé ; `remote_admin` faux, passkey absente ou assertion d'un autre compte refusées ; adresses locales montrées aux seuls gérants. Admin : sans jeton `via: "relay"`, rien de plus par le relais qu'aujourd'hui ; session à distance fermée à 30 minutes, à l'interrupteur coupé, au détachement ; `remote_admin` ne s'allume pas par le relais. Sur la vraie Station Windows et Mac : notification, e-mail, journal.

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
- L'appareil connecté demande au relais un jeton pour un ordinateur : `POST /v1/access {station}` avec sa session d'appareil. Le relais vérifie le compte, ses droits sur cet ordinateur, l'interrupteur côté relais, et rend un jeton **valable 2 minutes** (5 jusqu'au 25/11/2026, voir « Sessions ») : `{v, relay, station, account, device, rights, name, iat, exp, jti}` signé.
- L'appareil le présente à Admin (`Authorization: KyberNow <jeton>`), sur le réseau ou par le relais. Admin vérifie : signature (clé épinglée), `station` = la sienne, `exp`, interrupteur actif, compte et appareil non bloqués localement. Il en tire les droits.
- **Session** : Admin ouvre la session comme pour un appareil appairé, mais Kyber est atteint avec un **jeton de Kyber** (comme les invités du partage), pas avec un compte Kyber : pas de compte à créer, pas de redémarrage de Kyber, et la révocation sur le relais vaut au jeton suivant (le signe de vie, toutes les 5 secondes, en rend un neuf ; le jeton d'accès est redemandé toutes les 90 secondes).
- Le relais ne voit toujours pas l'image ni les entrées : le tunnel reste de bout en bout avec Admin épinglé.

### Les quatre garde-fous de la Station

1. **Interrupteur** dans Admin, « Accepter les comptes KyberNow » : coupé, Admin refuse tout jeton (les appareils appairés, les invitations et le réseau local marchent comme avant).
2. **Liste locale** : Admin garde chaque compte et appareil entré par un jeton (nom, e-mail, droits, dernière venue) et peut en **bloquer** un, sans passer par le relais ; un compte bloqué ici l'est même si le relais dit oui.
3. **Jetons courts et signés** : 2 minutes (une Station en accepte jusqu'à 5, relais plus ancien), clé du relais épinglée, `jti` jamais rejoué.
4. **Avis à chaque nouvel appareil** : la première fois qu'un appareil ouvre un ordinateur, le relais envoie un e-mail au propriétaire (ou aux Administrateurs de l'organisation) et Admin met une ligne dans le fil.

### Routes (premier jet)

| Où | Route | Quoi |
|---|---|---|
| Relais | `GET /v1/keys` | Clé publique de signature |
| Relais | `POST /v1/devices/code {name}` → `{code, poll, expires, interval, url, url_complete}` ; `POST /v1/devices/poll {poll}` | Connexion d'un appareil par code (téléviseurs) |
| Relais | `GET /tv` ; `GET/POST /compte/appareil?code=…` | Saisie du code d'un téléviseur ; « Connecter {appareil} ? », « Ce n'est pas moi » |
| Relais | `GET /compte/connexion?appareil=…&retour=kybernow://compte[&etat=…]` ; `GET/POST /compte/connecter-appareil` ; `POST /v1/devices/token {code}` | Connexion d'un appareil par la fenêtre du système (retour par le schéma `kybernow://compte?code=…`, code échangé contre la session d'appareil) |
| Relais | `POST /v1/devices/revoke` | L'app se déconnecte (sa session d'appareil) |
| Relais | `GET /v1/computers` | Ordinateurs du compte, avec les droits et l'identifiant de Station |
| Relais | `POST /v1/access {station}` | Jeton d'accès (2 min) |
| Relais | `attach_request` → `attach_code`, puis `attached` ; `attach_status` ; `detach` / `detached` (Station, par sa connexion de contrôle) | Rattachement, après approbation dans la console (`/compte/rattacher?code=…`) ; détachement des deux côtés |
| Admin | `GET/PUT /api/account` (session d'Admin) | Compte rattaché, interrupteur, détacher |
| Admin | `POST /api/account/attach` (session d'Admin) | « Se connecter » : code de rattachement demandé au relais, page de la console à ouvrir |
| Admin | `GET /api/local/account`, `PUT /api/local/account/relay`, `POST`/`DELETE /api/local/account/attach`, `POST /api/local/account/enroll` (jeton local ; pour régler le relais, rattacher et enrôler, aussi le jeton de l'installateur, `x-kyber-installer`, lu dans `installer-token` du dossier de données, réservé aux administrateurs) | L'installateur : relais choisi, rattachement pendant l'installation, enrôlement d'une installation silencieuse (« Installateurs ») |
| Relais | `enroll` → `attached` (Station, par sa connexion de contrôle) | Enrôlement par un jeton d'organisation (lot 5) |
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
- **Vérification par la Station**, dans cet ordre : clé épinglée (une des clés du rattachement), version, `relay` = le relais du rattachement, `station` = la sienne, `exp - iat` de 5 minutes au plus (`MAX_LIFETIME_SECS`, indépendant de la durée des jetons délivrés, `LIFETIME_SECS` = 2 minutes : une Station à jour accepte encore les jetons d'un relais plus ancien), `iat` pas dans le futur et `exp` pas passé, à **60 s** près (horloges), puis le `jti`.
- **`jti` jamais rejoué** : le même jeton sert à tous les appels de son appareil pendant ses 2 minutes (session, signe de vie), mais un `jti` n'est jamais accepté pour un autre contenu ; pour redémarrer ou mettre à jour la Station, le jeton est **à usage unique** (consommé, il ne sert plus à rien d'autre). Les `jti` sont oubliés à leur échéance.
- En-tête : `Authorization: KyberNow <jeton>`.

### Admin (lot 2)

- **Réglages** : section `[account]` de `kyber-admin.toml` : `relay`, `owner` (`kind` personne ou organisation, `id`, `name`, `email`), `keys` (clés épinglées), `pinned_at`, `accept` (l'interrupteur). Le rattachement (`attached` du relais, voir plus bas) appelle `accounts::pin` ; il allume l'interrupteur et « Accès hors de chez vous », qu'Admin permet de couper.
- **Liste locale** : `account-access.json` du dossier de données, une entrée par compte et appareil (noms, droits du dernier jeton, première et dernière venue, `lan` ou `relay`) ; blocage d'un appareil ou du compte entier. Une première venue met une ligne dans le fil. Détacher efface la liste.
- **Routes** (session d'Admin, jamais par le relais) :

  | Route | Corps | Réponse |
  |---|---|---|
  | `GET /api/account` | | `{attached, relay, owner, keys:[{kid, fingerprint}], pinned_at, accept, test, station, console, can_attach, attaching}` |
  | `PUT /api/account` | `{accept?: bool, detach?: true, cancel_attach?: true}` | La même vue ; allumer sans rattachement : `409 not_attached` ; détacher le dit au relais |
  | `POST /api/account/attach` | | `{url, expires}` : la page de la console à ouvrir ; `409 no_relay` ou `no_accounts`, `503 relay_unreachable` |
  | `GET /api/account/access` | | `{entries:[{id, account, device, name, email, device_name, rights, first_seen, last_seen, via, blocked, account_blocked, owner}]}` |
  | `POST /api/account/access/{id}/block` | `{blocked: bool, account?: bool}` | La liste ; `account` : tout le compte |

- **Selon les droits**, sur le réseau comme par le relais (classe `Device` de la garde, « Accès hors de chez vous » actif) :
  - **Session** : les routes `/api/stream/*` et celles de l'hôte (partage, prêt) comme un appareil appairé ; `prepare` rend en plus `kyber_token` `{token, user, expires}`, le jeton de Kyber (2 min, la durée du jeton d'accès) sous le compte `kn-account-…`, jamais écrit dans la configuration de Kyber ; le signe de vie en rend un neuf (lot 3), que Kyber relit s'il se reconnecte ;
  - **Partage** : `POST /api/share/join` sans `code` entre tout de suite (ni code, ni accord de l'hôte), sous le nom de la personne ; l'hôte le voit et peut le retirer ;
  - **Administrer** : `POST /api/stream/station/restart` et `/update`, aussi **par le relais** (fermés par le relais à un compte Kyber), avec un jeton à usage unique ;
  - **Prêt** : porté par le jeton et montré dans la liste ; le lanceur avec un compte vient au lot 4.
- **Refus** : `{error, code}`, `code` parmi `accounts_off`, `not_attached`, `bad_token`, `token_expired` (401 : l'app redemande un jeton), `blocked`, `no_right` (403).
- **Mode d'essai** : `KYBERNOW_ACCOUNT_TEST=1` au démarrage épingle une clé locale (`account-test.key`, relais `essai.kybernow.invalid`) quand la Station n'est rattachée à aucun relais ; `kyber-admin account-token --rights session,share` écrit un jeton signé par elle pour cette Station.
- **Code** : `server/admin/src/accounts.rs` (réglages, vérification, liste, routes), branché dans `api.rs` (`stream_user`, redémarrer et mettre à jour), `relay.rs` (garde) et `share_api.rs` (rejoindre).
- **Interface** : espace « Compte KyberNow » d'Admin (`#/compte`) : rattachée, le compte, le relais, la clé épinglée, « Gérer dans la console », « Détacher », l'interrupteur et la liste (« Bloquer », « Bloquer le compte », « Débloquer ») ; pas rattachée, « Se connecter » (actif quand un relais est réglé, `can_attach`) ouvre la console du relais dans le navigateur, puis la page attend la confirmation (`attaching`, « Rouvrir la page », « Annuler »).

### Rattachement et appareils sur le relais (lot 2, moitié relais du lot 3)

- **Clé de signature** : `signing.key` (Ed25519 en PKCS#8), à côté de `secret_key`, créée au premier démarrage, réservée au relais (0600). Rotation : l'administrateur du relais dépose `signing.next.key` ; elle est publiée avec la clé en cours (`GET /v1/keys`) et donnée aux Stations qui se rattachent ou se reconnectent (`attached`) ; quand toutes l'ont, elle remplace `signing.key`. Le nom du relais dans les jetons (`relay`) est l'hôte de `public_url`, avec son port hors du 443.
- **Messages de la connexion de contrôle** (docs/relais.md, « Trames et messages » ; un relais ou une Station d'avant les comptes les ignore) :

  | Message | Sens | Champs |
  |---|---|---|
  | `attach_request` | Station → relais | `req`, `name` (le nom de l'ordinateur) |
  | `attach_code` | relais → Station | `req`, `code`, `url` (`https://<relais>/compte/rattacher?code=…`), `expires` (900 s) ; relais sans comptes : `error {req, code: "no_accounts"}` |
  | `attached` | relais → Station | `req` (en réponse à `attach_status`), `relay`, `account`, `owner {kind, id, name, email}`, `keys` (comme `GET /v1/keys`) |
  | `attach_status` | Station → relais | `req` ; à chaque connexion d'une Station rattachée : réponse `attached` (clés à jour) ou `detached` |
  | `detach` | Station → relais | Détachée dans Admin |
  | `detached` | relais → Station | `req` (en réponse à `attach_status`) ; détachée dans la console |
  | `enroll` | Station → relais | `req`, `token` (le jeton d'enrôlement), `name` ; réponse `attached` avec ce `req`, ou `error {req, code}` : `enroll_refused` (`message` : `unknown`, `expired`, `revoked`, `used_up`), `no_accounts` (voir « Installateurs » et « Personnes, droits et organisations ») |

- **Rattacher** : Admin › « Se connecter » demande un code (gardé 15 minutes en mémoire par le relais, un par Station) et ouvre la page dans le navigateur ; on se connecte (la page est retenue, cookie `__Host-kn_suite`), « Rattacher {nom} », propriétaire « Moi » (les organisations viennent au lot 5), « Rattacher » ; le relais note le lien (table `stations`) et envoie `attached`. Station hors ligne : elle l'apprend à sa prochaine connexion (`attach_status`). Admin ne prend un `attached` que si « Se connecter » a été demandé dans les 15 minutes, sauf pour mettre à jour les clés du même rattachement.
- **Détacher** : depuis la page du compte (« Ordinateurs », « Détacher » : `detached` à la Station) ou dans Admin (`detach` au relais, dès qu'il est joint).
- **Sessions d'appareil** : distinctes des sessions du navigateur ; jeton porteur de 256 bits (`Authorization: Bearer …`), gardé en empreinte ; **90 jours sans servir** (`devices.seen_at`, noté au plus toutes les 5 minutes ; `DEVICE_IDLE_SECS`) et elle est refusée comme une session déconnectée (`401 bad_session`, les apps disent « Ce compte a été déconnecté de cet appareil : reconnectez-vous. »), effacée, `appareil_expire` au journal du compte ; le ménage du relais efface aussi celles qu'on ne présente plus ; « Appareils connectés » sur la page du compte (« Utilisé il y a {durée} », l'aide des 90 jours, « Déconnecter ») ; un nouveau mot de passe les ferme toutes, la suppression du compte aussi (ses Stations sont alors détachées et reçoivent `detached`). Fenêtre du système : `retour` vaut `kybernow://compte` (rien d'autre), `etat` est rendu tel quel ; après « Connecter », une page renvoie vers `kybernow://compte?code=…&etat=…` (code à usage unique, 5 minutes, échangé à `POST /v1/devices/token`). Code : huit signes `XXXX-XXXX` (ni voyelle, ni 0, 1), 10 minutes, demandé toutes les 5 secondes (`202` en attente, `200` la session, `403 denied`, `410 expired`) ; 20 codes demandés et 10 codes tapés faux par adresse en 15 minutes.
- **Jeton d'accès** : `GET /v1/computers` rend `{computers:[{station, name, rights, owner, mine, online, attached_at}]}`, et pour une Station connectée son empreinte d'Admin signée par elle (`station_key`, `admin_fp`, `sig`, comme `GET /v1/codes/{code}`, lot 3) ; `POST /v1/access {station}` rend `{token, station, rights, expires, lifetime}`, `404 unknown_station` pour une Station qui n'est pas au compte, `401 bad_session` sans session d'appareil. Au lot 2, les ordinateurs d'un compte sont ceux qu'il possède, avec tous les droits ; les lots 4 et 5 y ajoutent ceux qu'on lui ouvre et ceux de ses organisations (« Personnes, droits et organisations »).
- **Avis à chaque nouvel appareil** : le premier jeton d'un appareil pour une Station (table `device_stations`) envoie un e-mail au propriétaire (« Nouvel appareil sur {ordinateur} ») et met une ligne `nouvel_appareil` au journal.
- **Base** : migration 3 (`stations`, `devices`, `device_codes`, `device_stations`) ; la migration 2 est celle du lot 1b.
- **Code** : `server/relay/src/accounts/stations.rs` (clé, rattachement), `devices.rs` (appareils, API, jetons), `web/attach_pages.rs` et `web/device_pages.rs` (pages) ; le hub (`hub.rs`) fait suivre les messages.

### Personnes, droits et organisations (lots 4 et 5)

Fait côté relais (console, API, base) et côté Admin ; les apps sont décrites dans « Apps (lot 3) », l'étape compte des installateurs dans « Installateurs ».

- **Droits d'un compte sur un ordinateur** : tous pour son propriétaire (une personne), et pour les Propriétaires et Administrateurs de l'organisation propriétaire ; sinon l'union des **droits donnés** pas échus, à lui, à une organisation dont il est membre, ou à l'un de ses groupes. **Administrer ne compte qu'avec la double authentification** (TOTP active ou une passkey, qui la vaut), propriétaire compris : sans elle, le jeton et `/v1/computers` n'ont pas `admin`, et la page « Personnes » le dit. De même, un Propriétaire ou Administrateur d'organisation sans double authentification ne gère pas l'organisation et n'a que les droits donnés, comme un membre.
- **Gérer les accès** d'un ordinateur (« Personnes ») : son propriétaire, les Propriétaires et Administrateurs de l'organisation propriétaire, et toute personne qui a le droit Administrer (effectif).
- **Ajouter une personne** : son adresse, ses droits, une échéance facultative (la fin du jour choisi, UTC). Avec un compte vérifié, le droit est à elle tout de suite ; sans compte, il attend l'adresse et passe au compte **quand elle est vérifiée** (inscription, lien du mot de passe oublié, ou compte créé par l'administrateur du relais). Ajouter de nouveau la même adresse change ses droits. Un e-mail le dit (« {nom} vous ouvre {ordinateur} »), avec la console ou l'inscription.
- **Ouvrir à une organisation** : un ordinateur (d'une personne ou d'une organisation) s'ouvre à toute une organisation dont on est membre, ou à l'un de ses groupes, avec des droits ; son journal le note.
- **Organisation** : la créer demande la double authentification (le créateur en est Propriétaire). Inviter un membre par son adresse (avec un compte vérifié : membre tout de suite, e-mail ; sans compte : invitation en attente, reprise à la vérification de l'adresse). Seul un Propriétaire nomme ou retire un Propriétaire ; une organisation en garde toujours un (« Quitter » et changer le rôle du dernier sont refusés). Un compte supprimé : son organisation reçoit un autre Propriétaire (le plus ancien Administrateur, sinon le plus ancien membre), ou part avec ses ordinateurs s'il n'y a plus personne.
- **Journal de l'organisation** : chaque changement (membres, rôles, invitations, groupes, droits donnés et retirés, ordinateurs rattachés, détachés, enrôlés, jetons) et chaque ouverture d'un de ses ordinateurs (qui, quel appareil, d'où ; une ouverture : un premier jeton après une heure sans), et chaque nouvel appareil. Adresses IP oubliées après 7 jours, lignes après 12 mois ; vu des Propriétaires et Administrateurs seulement.
- **Avis de nouvel appareil** : pour l'appareil d'une personne ajoutée, l'e-mail part au propriétaire et nomme la personne (« avec le compte KyberNow de Léa ») ; pour un ordinateur d'organisation, aux Propriétaires et Administrateurs, avec une ligne au journal.
- **Rattacher** : la page « Rattacher {nom} » propose « Moi » et les organisations que le compte gère (Propriétaire ou Administrateur, double authentification) ; `attached` porte alors `owner {kind: "organization", id: "org_…", name}`. Rattacher à un autre propriétaire efface les droits donnés et les premières venues.
- **Jetons d'enrôlement** (« Déployer des Stations ») : un nom, une durée (1 à 365 jours), un nombre de Stations (1 à 10 000), et des droits donnés à un groupe sur chaque Station enrôlée ; `kne_` et 32 signes (160 bits), **montré une seule fois**, gardé en empreinte. La Station le présente par sa connexion de contrôle (`enroll`) ; le relais la rattache à l'organisation (même réponse si elle l'est déjà, sans compter une Station de plus), donne les droits au groupe, note le journal. Révoquer empêche de nouveaux enrôlements ; les Stations enrôlées restent.
- **Admin sur le réseau** (« Se connecter avec KyberNow ») : la page de connexion d'Admin propose le bouton quand la Station est rattachée et accepte les comptes, **jamais par le relais**. Admin tire un `nonce` (16 octets, 10 minutes, cookie `kyber_admin_kn`, `SameSite=Lax`) et ouvre `https://<relais>/compte/ouvrir-admin?station=…&nonce=…&retour=<adresse d'Admin>`. Connectée, avec le droit Administrer (double authentification), la personne confirme « Ouvrir Admin de {ordinateur} ? » ; le relais délivre un jeton d'accès (2 min, appareil `web`) dont le **`jti` est le `nonce`**, et renvoie vers `<retour>/api/login/kybernow?token=…` (relais publiés) ou, mieux, `<retour>/api/login/kybernow#token=…` : depuis la 0.11.8, Admin sert alors une petite page qui lit le fragment (jamais envoyé au serveur ni gardé dans l'historique ou les journaux), l'efface et le poste en formulaire à `POST /api/login/kybernow/return` (`token=…`, sans l'en-tête d'Admin : une navigation) ; un relais peut aussi poster ce formulaire lui-même. `retour` n'est qu'une adresse du réseau local (IP privée, lien local ou boucle locale, nom sans point, `.local`, `.lan`, `.home.arpa`), sans chemin. Admin vérifie le jeton (droit Administrer, **usage unique**) et que son `jti` est le `nonce` de ce navigateur, puis ouvre sa session ; une ligne dans le fil.
- **Prêt avec un compte** : `POST /api/lend/status` et `/api/lend/borrow` acceptent `Authorization: KyberNow <jeton>` avec le droit Prêt à la place de l'identifiant et du mot de passe d'un compte d'ami ; l'emprunt se fait sous le nom du compte (pas de créneaux : ils restent ceux des comptes d'amis ; Steam : « Son compte Steam »). Les apps permises restent celles de la page « Prêt ».
- **Enrôlement côté Admin** : deux entrées, un seul message `enroll` au relais. L'installateur silencieux passe le jeton par la route locale `POST /api/local/account/enroll` (réponse tout de suite, pour son code de sortie : « Installateurs ») ; un outil de déploiement peut aussi déposer `[account] enroll = "kne_…"` dans `kyber-admin.toml` (avec le relais dans `[relay]`) : Admin le présente dès que le relais est joint, prend `attached` comme un rattachement demandé (comptes acceptés, accès hors de chez vous actif), puis l'efface ; refusé pour de bon (`enroll_refused`), une ligne dans le fil et le jeton est effacé ; relais injoignable : nouvel essai, de plus en plus espacé (10 min au plus).
- **Liens d'accès** (relais) : `kla_` et 32 signes (160 bits), gardé en empreinte avec son début (`kla_7qm2x…`) pour la liste ; montré une seule fois, sur la page qui suit sa création. Dix en attente au plus par ordinateur. L'accepter est une transaction : le lien est marqué utilisé seulement s'il est encore en attente, puis ses droits s'ajoutent à ceux du compte sur l'ordinateur (union ; l'échéance la plus lointaine, « sans fin » l'emporte ; un droit déjà échu est remplacé) ; deux personnes qui acceptent en même temps : une seule l'obtient, l'autre lit « Ce lien a déjà servi. ». Administrer, donné par un lien, demande la double authentification comme tout droit donné (la page du lien le dit avant « Accepter »). Précisions de la mise en œuvre : « C'est votre ordinateur : ce lien est pour quelqu'un d'autre. » vaut aussi pour les Propriétaires et Administrateurs de la famille ou de l'équipe propriétaire et pour qui a créé le lien ; un lien dont l'auteur ne gère plus les accès de l'ordinateur ne sert plus (« Ce lien a été révoqué. ») ; « Révoquer » est proposé pour un lien en attente ou déjà utilisé (sans effet sur l'accès) ; les liens partent quand l'ordinateur est détaché ou rattaché à un autre propriétaire, et un lien fermé (utilisé, révoqué, expiré) est oublié après 30 jours. Ouvert sans être connecté, le lien est retenu une heure (cookie signé `__Host-kn_suite`) : après la connexion, ou après la vérification de l'adresse d'un compte tout juste créé, on y revient (la page retenue passe avant le tour d'accueil). Les liens inconnus sont limités comme les codes (5 par adresse IP en 15 minutes) ; le lien n'est jamais écrit dans les journaux. Journal : `lien_cree`, `lien_utilise` (qui), `lien_revoque` dans celui de la famille ou de l'équipe propriétaire ; dans celui du compte, `lien_cree`, `lien_revoque`, `lien_utilise` (pour l'auteur) et `lien_accepte` (pour qui l'accepte).
- **Groupes d'ordinateurs** (relais) : les droits donnés sur un groupe (à toute la famille ou l'équipe, à un de ses groupes de personnes, à un de ses membres) comptent dans les deux seules requêtes des droits (`granted_rights` et `visible_stations`, `people.rs`), pour chaque ordinateur du groupe à cet instant, en plus des droits donnés ordinateur par ordinateur ; l'échéance est respectée. Un droit donné à un membre ne vaut que tant qu'il est membre. Seuls les ordinateurs de la famille ou de l'équipe entrent dans ses groupes ; un ordinateur détaché, ou rattaché à un autre propriétaire, en sort. « Ranger dans le groupe » d'un jeton d'enrôlement range chaque Station enrôlée dans ce groupe (`enroll`). Journal : `groupe_ordinateurs_cree`, `_supprime`, `_ajoute`, `_retire` (un ordinateur), `_ouvert`, `_ferme` (un droit donné), et `station_enrolee` nomme le groupe.

#### Pages de la console

| Route | Écran |
|---|---|
| `GET /compte/ordinateurs` | « Ordinateurs » : « À vous », « Ouverts pour vous », une section par organisation ; « Créer une organisation » |
| `GET /compte/ordinateur?id=…` | « Personnes » d'un ordinateur : droits par personne (Session, Prêt, Partage, Administrer), « Ajouter une personne », « Ouvrir à une organisation », « Retirer », « Détacher cet ordinateur » ; ses droits seulement pour qui ne gère pas |
| `POST /compte/ordinateur/ajouter` | `id`, `email`, `session`/`lend`/`share`/`admin` (`1`), `fin` (`aaaa-mm-jj` ou `jj/mm/aaaa`, facultatif) |
| `POST /compte/ordinateur/ouvrir` | `id`, `a` (`org_…`, ou `org_…:<groupe>`), droits |
| `POST /compte/ordinateur/retirer` | `id`, `acces` |
| `POST /compte/ordinateur/renvoyer` | « Renvoyer l'invitation » d'une adresse : `id`, `acces` ; une fois par heure ; l'adresse a depuis un compte vérifié : l'ordinateur passe à lui |
| `GET/POST /compte/organisations/creer` | `nom` ; double authentification exigée |
| `GET /compte/organisation?id=org_…&vue=membres\|groupes\|ordinateurs\|journal` | « Organisation » : membres et rôles, invitations en attente, groupes, ordinateurs et leurs accès, journal |
| `POST /compte/organisation/inviter`, `/role`, `/retirer`, `/quitter`, `/renommer`, `/renvoyer` | `email` et `role` ; `compte` et `role` ; `compte` ou `invitation` ; rien ; `nom` ; `invitation` (« Renvoyer l'invitation », une fois par heure) |
| `POST /compte/organisation/groupes/creer`, `/supprimer`, `/membre` | `nom` ; `groupe` ; `groupe`, `compte`, `action` (`ajouter`, `retirer`) |
| `GET /compte/organisation/deployer?id=…` ; `POST /compte/organisation/jetons/creer`, `/revoquer` | « Déployer des Stations » ; `nom`, `jours`, `nombre`, `groupe`, droits, `groupe_ordinateurs` (« Ranger dans le groupe », facultatif) ; `jeton` |
| `POST /compte/ordinateur/liens/creer`, `/revoquer` | « Liens d'accès » de la page d'un ordinateur (qui gère ses accès ; panneaux `#lien` et `#lien-<n>` pour confirmer) : `id`, droits, `jours` (`1`, `7`, `30`), `fin` (facultatif) ; `id`, `lien` |
| `GET/POST /compte/lien/kla_…` | « {nom} vous ouvre {ordinateur} » : sans être connecté, « Se connecter » et « Créer un compte » (puis retour ici) ; connecté, les droits, l'échéance et « Accepter » (le POST) ; les refus |
| `POST /compte/organisation/groupes-ordinateurs/creer`, `/supprimer`, `/ordinateur`, `/ouvrir`, `/retirer` | Onglet « Ordinateurs » (Propriétaires et Administrateurs ; panneaux `#groupe-ordinateurs`, `#ordinateurs-<n>`, `#supprimer-ordinateurs-<n>`) : `nom` ; `groupe` ; `groupe`, `ordinateur`, `action` (`ajouter`, `retirer`) ; `groupe`, `a` (`org`, `grp:<n>`, `compte:<n>`), droits, `fin` ; `groupe`, `acces` |
| `GET/POST /compte/ouvrir-admin` | « Ouvrir Admin de {ordinateur} ? » (`station`, `nonce`, `retour`) |
| `POST /compte/ordinateur/deplacer` | « Déplacer… » : `id`, `vers` (`moi`, `org_…`) → « Ce qui change » ; `action=deplacer`, `garder` (`1`) → le déplacement (voir « Réglages synchronisés et déplacer un ordinateur (migration 9) ») |

La barre de la console (« Ordinateurs », chaque organisation, « Compte ») est aussi en haut de `/compte`. Le détachement depuis « Personnes » revient à « Ordinateurs ».

**Invitations, ajout de membres** (voir plus haut « Invitations, ajout de membres et sessions ») : une invitation (`grants` avec `email`, `org_invites`) expire `INVITE_SECS` (30 jours) après `sent_at` ; `claim_invitations(compte, maintenant)` ne reprend que celles qui valent encore et laisse les autres ; ajouter (ou remplacer) une invitation remet `sent_at` à maintenant ; « Renvoyer l'invitation » aussi, et note `resent_at` (`RESEND_GAP_SECS`, une heure, entre deux renvois). Donner un droit à un compte (`grant_person`, `set_member`) retire une invitation de son adresse au même endroit. Journal : `invitation_renvoyee` au journal du compte, et à celui de la famille ou de l'équipe (propriétaire de l'ordinateur, ou celle de l'invitation). Les suggestions (`known_people`, 200 au plus) : les membres des familles et équipes du compte (et leurs invitations en attente), les personnes et adresses ajoutées à ses ordinateurs ou par lui ; jamais personne d'autre. « Ouvrir un groupe » à un membre se choisit dans une liste (`compte:<n>`), sans adresse à taper : pas de suggestions là. « Personnes » (`/compte/personnes`) regroupe par compte (une invitation dont l'adresse a un compte vérifié va avec lui), par adresse sinon, par famille, équipe ou groupe ; chaque ligne nomme ses ordinateurs et l'union de ses droits. Les suggestions s'écrivent « Prénom Nom · adresse » (le point médian des pages : elles n'emploient pas le tiret cadratin).

#### API et messages

- `GET /v1/computers` : les siens, ceux qu'on lui a ouverts (à lui, à ses organisations, à ses groupes), et tous ceux des organisations qu'il gère ; `owner` vaut `{kind: "person", id: "acc_…", name}` ou `{kind: "organization", id: "org_…", name}`, `mine` dit s'il en est le propriétaire, `rights` ses droits à cet instant. `POST /v1/access` : les mêmes droits, `404 unknown_station` sans aucun droit.
- Connexion de contrôle : `enroll {req, token, name}` (Station → relais) ; réponse `attached {req, …}`, ou `error {req, code: "enroll_refused", message}` avec `unknown`, `expired`, `revoked` ou `used_up` ; relais sans comptes : `no_accounts`. Le crate `tunnel` le porte (`Agent::enroll`).
- Admin : `POST /api/login/kybernow {origin, lang}` → `{url, expires}` (sans session, `403 network_only` par le relais, `409 unavailable` sans rattachement ou interrupteur coupé) ; `GET /api/login/kybernow?token=…` → la session d'Admin (page qui revient à l'accueil) ou le refus ; `GET /api/session` dit `kybernow_login`.

#### Base

- Migration 4 : `grants` (Station, `account` ou `email` d'une invitation, `rights` en mots `session,share`, `expires_at`, `invited_by`, `created_at`) ; `device_stations.seen_at` (dernière venue, pour le journal).
- Migration 5 : `organizations` (identifiant public `org_…`, nom, offre), `org_members` (rôle `owner`, `admin`, `member`), `org_invites`, `org_groups`, `group_members`, `org_log` (qui, quoi en JSON, adresse, quand), `enroll_tokens` (empreinte, début du jeton, nombre, utilisés, groupe et droits, échéance, révocation) ; `stations` reconstruite (`owner_kind` `person` ou `organization`, `account` ou `org`, `enrolled_by`) ; `grants.org` et `grants.grp`.
- Migration 8 : `grants.sent_at`, `grants.resent_at`, `org_invites.sent_at`, `org_invites.resent_at` (l'envoi des invitations). Les invitations déjà en attente reçoivent l'heure de la migration : 30 jours pleins à partir de la mise à jour du relais, plutôt qu'une expiration soudaine de celles envoyées il y a plus d'un mois.
- Migration 7 : `access_links` (Station, empreinte, début du lien, droits, `link_expires_at`, `access_expires_at`, `created_by`, `created_at`, `used_by`, `used_at`, `revoked_at`) ; `station_groups` (organisation, nom unique dans l'organisation), `station_group_members` (groupe, Station) ; `group_grants` (groupe d'ordinateurs, et `account`, `org` ou `grp`, droits, échéance, `invited_by`) : une table à part, `grants` reste « les droits donnés sur une Station » ; `enroll_tokens.sgroup` (« Ranger dans le groupe »). Pas de clé étrangère vers `stations` (comme `grants`) : détacher ou rattacher à un autre propriétaire efface les lignes de la Station.
- Code : `server/relay/src/accounts/people.rs` (droits donnés, droits d'un compte, invitations et leur envoi, migration 8, suggestions), `orgs.rs` (organisations, journal, jetons, `enroll`), `links.rs` (liens d'accès, migration 7), `station_groups.rs` (groupes d'ordinateurs), `console_pages.rs` (pages), `web/console.rs`, `web/org_pages.rs` (et les groupes d'ordinateurs), `web/link_pages.rs` (liens d'accès), `web/admin_login.rs` ; essais : `web/tests/lot6.rs`, `web/tests/invitations.rs` ; Admin : `accounts.rs` (connexion avec KyberNow, enrôlement), `lend_api.rs` (prêt avec un compte) ; interface : `server/web/src/pages/Login.svelte`.

### Réglages synchronisés et déplacer un ordinateur (migration 9)

Fait côté relais le 25/11/2026 (le cœur et les apps lisent et écrivent les réglages de leur côté).

- **Base** : migration 9, `computer_settings` (`account` → `accounts` avec `ON DELETE CASCADE`, `station`, `"values"` : le JSON tel qu'envoyé, `updated_at` ; clé `(account, station)`, index sur `station`). Pas de clé étrangère vers `stations`, comme `grants`.
- **API des appareils** (session d'appareil, `Authorization: Bearer …`, comme `/v1/computers`) :
  - `GET /v1/settings` → `200 {"settings":[{"station":"<id>","updated_at":<unix>,"values":{…}}]}`, seulement les ordinateurs sur lesquels le compte a des droits à cet instant (ceux de `/v1/computers`) ; les lignes des autres sont effacées à cette lecture.
  - `PUT /v1/settings/<station>` `{"values":{…}}` → `200 {"updated_at":<unix>}` ; le dernier l'emporte ; les valeurs sont gardées telles quelles (une clé inconnue reste, pour les apps plus récentes). `404 {"error":"unknown_station"}` sans droit (ou identifiant illisible), `400 {"error":"bad_values"}` si `values` n'est pas un objet, `413 {"error":"too_large"}` au-delà de 4096 octets une fois sérialisé, `401 bad_session` sans session, `405` pour une autre méthode.
  - Clés connues : `profile` (texte, ou `null` pour Auto), `codec` (texte), `yuv444` (booléen), `mbps` (nombre), `hdr` (booléen), `immersive` (`"fullscreen"`, `"always"`, `"never"`).
- **Ménage** : avec le compte (clé étrangère ; et les lignes des autres comptes sur ses ordinateurs, ou sur ceux d'une famille ou d'une équipe qui part avec lui) ; toutes les lignes d'un ordinateur détaché (console ou Admin) ou rattaché à un autre propriétaire ; après un déplacement, celles des comptes qui n'y ont plus droit ; à la lecture (`GET`) et au ménage périodique du relais (toutes les 30 s), celles d'un ordinateur que le compte ne voit plus. L'export des données les porte (`synced_settings`).
- **Déplacer** : `POST /compte/ordinateur/deplacer` (`id`, `vers` = `moi` ou `org_…`) rend « Ce qui change » ; avec `action=deplacer` (et `garder=1`, la case) il déplace. Le panneau `#deplacer` de la page d'un ordinateur choisit la destination : « Moi » (sauf s'il est déjà à vous) et les familles et équipes dont on est Propriétaire ou Administrateur, sauf la propriétaire actuelle. « Déplacer… » est proposé à son propriétaire et aux Propriétaires et Administrateurs de sa famille ou de son équipe (pas à qui a seulement le droit Administrer) ; sans double authentification, le refus le dit. Côté Admin (`accounts::on_attached`), un `attached` non demandé est pris quand le relais est le même et que ses clés sont celles déjà épinglées : seul le propriétaire affiché change.
- **Ce qui part** (une transaction, `moves.rs`) : les droits donnés à des familles, équipes ou groupes, **sauf ceux donnés à la destination ou à l'un de ses groupes** (ils gardent un sens chez elle) ; ses groupes d'ordinateurs (et les accès qui en venaient) ; les liens en attente (révoqués, ils restent dans la liste) ; les droits donnés à des personnes et les invitations si la case est décochée ; le droit donné au nouveau propriétaire (une personne), qui a désormais tout ; les premières venues des appareils (le nouveau propriétaire est prévenu de chaque appareil, comme après un rattachement). `enrolled_by` est remis à rien. L'aperçu est calculé des mêmes données.
- **Journaux** : `ordinateur_deplace {station, from, to}` au journal de l'ancienne et de la nouvelle famille ou équipe ; `ordinateur_deplace` au journal du compte de qui déplace (la personne d'un côté, c'est lui).
- **La Station** : le relais lui envoie `attached` (sans `req`) avec le nouveau `owner`, comme après « Rattacher ». **À faire dans Admin** : aujourd'hui, `on_attached` ignore un `attached` non demandé dont le propriétaire change (seules les clés et les noms du même propriétaire suivent) ; pour qu'Admin montre le nouveau propriétaire, il faut l'accepter quand le relais est le même et les clés sont celles déjà épinglées. Le jeton d'accès ne dépend pas du propriétaire : les accès marchent sans ce changement.
- **Code** : `server/relay/src/accounts/settings.rs` (migration 9, API), `moves.rs` (aperçu, déplacement), `web/move_pages.rs`, `console_pages.rs` (`move_preview`, panneau `#deplacer`) ; essais : `web/tests/deplacer.rs`.

### Installateurs

Maquette : rangée « Installateurs ». Code : `server/installer/src/account.rs` (déroulé, vérification du relais, routes d'Admin), `quiet.rs` (installation silencieuse), `ui/src/lib/account.svelte.ts` et `AccountStep.svelte` (interface commune à Windows et au Mac) ; `apps/windows/setup/src/account.rs` et `ui/src/screens/Account.svelte` (Link).

- **Station, pendant l'installation** (Windows et Mac) : l'écran Installation devient « Pendant ce temps : votre compte KyberNow » (« Se connecter », « Créer un compte », « Relais : share.kybernow.app · Autre relais… », « Plus tard ») ; l'installation continue en fond, sa progression reste en haut. L'étape est sautée pour une Station déjà rattachée qu'on réinstalle en gardant ses réglages (`[account] relay` de `kyber-admin.toml`).
- **« Autre relais… »** : adresse `hôte[:port][#sha256:…]`, vérifiée en direct par l'installateur (`GET /v1/health` : joint, version ; `GET /v1/keys` : comptes activés ou non ; autorités du web, ou l'empreinte d'un relais signé par lui-même). Un relais sans comptes est gardé pour le partage et l'accès hors de chez soi, sans rattachement. Le relais choisi devient celui de la Station (Admin › Hors de chez vous), même avec « Plus tard ».
- **Rattachement** : dès que la tâche « admin » est finie (Admin démarré), l'installateur appelle Admin par ses routes locales (ci-dessous) ; Admin demande le code au relais (`attach_request`) et rend la page `/compte/rattacher?code=…`, que l'installateur affiche **dans sa fenêtre** (vue enfant de wry : WebView2, WKWebView), sans quitter le relais (tout autre lien part dans le navigateur ; « Créer un compte » remplace la première page de connexion par l'inscription, le cookie de suite du relais ramène au rattachement). « À qui est cet ordinateur ? » est la page du relais (« Moi » ou une organisation que le compte gère, lot 5). L'installateur suit Admin (`GET /api/local/account` toutes les 2 s, 15 minutes au plus) jusqu'à `attached` ; l'écran final montre les trois accès : rattaché (compte ou organisation, relais), « Comptes KyberNow acceptés », « Accès hors de chez vous activé ». Un relais signé par lui-même : la page s'ouvre dans le navigateur (la vue de la fenêtre refuserait son certificat).
- **Routes locales d'Admin** (loopback et jeton local, comme `/api/local/*` ; jamais par le relais) :

  | Route | Corps | Réponse |
  |---|---|---|
  | `GET /api/local/account` | | La vue de `GET /api/account`, plus `server` (le relais réglé) et `remote_access` |
  | `PUT /api/local/account/relay` | `{relay}` | Règle le relais (« Plus tard ») ; la même vue |
  | `POST /api/local/account/attach` | `{relay?}` | Règle le relais puis comme `POST /api/account/attach` : `{url, expires}` |
  | `DELETE /api/local/account/attach` | | Abandonne le rattachement en cours (« Retour », « Plus tard ») |
  | `POST /api/local/account/enroll` | `{token, relay?}` | Enrôlement : la vue, rattachée ; `400 bad_token` (forme), `403 bad_token` (refusé par le relais : `enroll_refused`), `409 no_relay` / `no_accounts`, `503 relay_unreachable` |

  Toutes refusent une Station déjà rattachée (`409 already_attached`) : se rattacher ailleurs passe par « Détacher » dans Admin. Le jeton local est celui de l'icône : la personne devant l'ordinateur peut déjà accepter un appairage ; elle peut aussi rattacher une Station qui ne l'est pas.
- **Enrôlement** (déploiement d'une flotte) : `KyberNow-Station-Setup.exe /quiet /enroll=<jeton> /relay=<adresse>` installe sans écran ni WebView2 (options d'office, réglages gardés d'une installation existante, compte d'installation `kybernow` au mot de passe tiré au hasard : on reprend la main par l'icône, « Réinitialiser le mot de passe d'Admin… »), puis passe le jeton à Admin (`POST /api/local/account/enroll`, 3 minutes de patience au démarrage d'Admin), qui l'envoie au relais par sa connexion de contrôle : `{"t":"enroll","req":3,"token":"kne_…","name":"GRAPH-01"}`. Le relais répond comme à un rattachement confirmé dans la console, `attached` portant le même `req` (propriétaire : l'organisation du jeton), ou `error {req, code: "enroll_refused" | "no_accounts"}` (`enroll_refused` porte la raison : `unknown`, `expired`, `revoked`, `used_up`) ; un relais d'avant l'enrôlement ne répond pas (10 s : `no_accounts`). Admin épingle la réponse comme un `attached` demandé (interrupteur et accès hors de chez soi allumés). Codes de sortie : `0`, `3010` (installée, à redémarrer), `1` (installation échouée), `2` (installée, pas enrôlée) ; tout est écrit dans `install.log`. Sur Mac : `kybernow-ui --quiet --enroll=<jeton> --relay=<adresse>` (l'exécutable de « Installer KyberNow Station » du .dmg), dans la session de l'utilisateur (pas en root) ; macOS demande une fois le mot de passe d'administrateur, les autorisations se donnent ensuite (icône, ou profil PPPC). Le jeton n'est vérifié qu'en forme par Admin (8 à 256 signes `A-Z a-z 0-9 _ - . ~`) ; le relais décide (lot 5 : durée, nombre de Stations, révocation). Essai : `server/relay/tunnel/tests/enrolement.rs` et `accounts::tests::enrolement_avec_un_relais_simule` d'Admin, avec le relais simulé du tunnel (fonction `test-relay`).
- **KyberNow Link pour Windows** : à la première installation, étape facultative « Votre compte KyberNow » entre Bienvenue et Installation (pas pour une mise à jour). « Se connecter » ouvre dans la fenêtre `https://<relais>/compte/connexion?appareil=<nom du PC>&retour=kybernow://compte&etat=<16 octets au hasard>` ; le retour `kybernow://compte?code=…&etat=…` est intercepté (l'`etat` doit être le même), le code échangé (`POST /v1/devices/token`), et la session d'appareil **remise à l'app** (lot 3) :
  - le **compte entier**, comme l'app le garde elle-même (`apps/windows/src/store.rs`, `save_account`) : identifiant générique du Gestionnaire d'identification de Windows, cible `KyberNowCompte:<relais>|<identifiant du compte>` (`<relais>` sous sa forme normale : `hôte` ou `hôte:port` hors du 443, `#sha256:…` pour un relais signé par lui-même ; `Account::key` du cœur), utilisateur l'e-mail du compte, secret le JSON `{"relay","token","account":{"id","name","email"},"device":{"id","name"}}` en UTF-8 (`kybernow_core::account::Account`), persistance `LOCAL_MACHINE` ; un compte déjà là pour le même relais et le même compte est remplacé. Rien dans un fichier (`%APPDATA%\KyberNow\accounts.json` est la liste des ordinateurs des comptes que tient l'app, sans rien de secret).

  L'app lit ses comptes au démarrage et démarre connectée. « Créer un compte » ouvre l'inscription du site dans la fenêtre, puis « Compte créé : se connecter ». « Plus tard » : l'app se connecte depuis son accueil. Désinstaller en effaçant les données oublie aussi les comptes (`--forget-all` retire les mots de passe `KyberNow:*` et, à part, les comptes `KyberNowCompte:*`).

Écarts de cette étape :

- le choix « Moi » / « {organisation} » et le nom de l'ordinateur ne sont pas des écrans de l'installateur : c'est la page de rattachement du relais, affichée dans sa fenêtre ;
- la page d'un relais signé par lui-même s'ouvre dans le navigateur, pas dans la fenêtre ;
- l'installation silencieuse prend les options d'office (pas de `/user=` ni `/password=`, qui se verraient dans la liste des processus) ;
- sur Mac, l'étape doit être finie (rattachée ou « Plus tard ») avant les autorisations : leur fin relance Admin, qui oublierait un rattachement en cours.

### Apps (lot 3)

- **Cœur** (`apps/core/src/account.rs`) : tout ce qui parle au relais pour les apps, testable sans Kyber.
  - **Fenêtre du système** : `sign_in(relais, nom de l'appareil)` rend l'adresse `https://<relais>/compte/connexion?appareil=…&retour=kybernow://compte&etat=…` (`etat` : 16 octets au hasard) ; `finish_sign_in` lit le retour (`etat` vérifié, `erreur=annule` : annulé) et échange le code (`POST /v1/devices/token`).
  - **Code** : `tv_code` (`POST /v1/devices/code`, avec `url_label` à écrire et `issued_at` pour « Le code change dans {n} min. »), `tv_poll` (`202` en attente, `200` le compte, `403`, `410`), `tv_wait` (une demande toutes les 5 s au moins, relais muet toléré, annulable).
  - **Compte** (`Account {relay, token, account, device}`) : gardé **entier** par l'app dans le coffre de son système (Trousseau sans synchronisation iCloud, Keystore d'Android, Gestionnaire d'identification de Windows, fichier réservé ou Secret Service sous Linux) ; `Debug` masque le jeton, `summary()` le laisse de côté ; clé `relais|compte` (plusieurs comptes, plusieurs relais). `sign_out` : `POST /v1/devices/revoke` (`401` : déjà fait).
  - **Ordinateurs** : `computers` (`GET /v1/computers`) ; l'empreinte d'Admin n'est gardée que si la signature de la Station tient (clé ↔ identifiant) ; `remember_fingerprints` garde celles déjà vues (le relais ne les donne que Station connectée) ; `merge` : un ordinateur une fois (même empreinte d'Admin ou même Station chez le relais ; l'appairé reste, avec la pastille du compte) ; `origin` : la pastille (« Compte », « Compte · {relais} », l'organisation, « Ouvert par {prénom} », « Appairé »). `AccountComputer::route` : la route par le relais, Admin épinglé.
  - **Jeton d'accès** : `StationAccess` demande `POST /v1/access` et garde le jeton jusqu'à 30 secondes de son échéance (`EXPIRY_MARGIN` ; 90 secondes pour un jeton de 2 minutes, 4 minutes au plus avec un relais plus ancien ; `expires` compte aussi, l'appareil ayant pu dormir) ; `single_use` pour redémarrer ou mettre à jour la Station (Admin le consomme) ; `invalidate` après un refus.
  - **Admin** : `Console::account(hôte, port, accès, empreinte)` porte `Authorization: KyberNow <jeton>` à chaque appel, sur le réseau comme par le relais (`with_relay`) ; `401 token_expired` ou `bad_token` : jeton neuf et second essai (info, prepare, signe de vie, HDR, consoles) ; sans jeton, `ConsoleError::Account`.
  - **Session** : `SessionParams::account` (`{"account": …, "station": …}` dans le JSON des ponts) ; le jeton d'accès est demandé avant la sonde du réseau ; Kyber est joint avec le jeton de Kyber de `prepare` (`kyber_token`), jamais un compte Basic ; le signe de vie en rend un neuf (Admin), que Kyber relit à sa reconnexion.
  - **Erreurs** (`AccountError`, codes 70 à 80 des ponts) : session de l'appareil fermée (l'app oublie le compte), ordinateur retiré du compte, code refusé ou échu, retour refusé, annulé, trop d'essais, relais sans comptes, adresse illisible, relais muet, le reste.
- **Ponts** : `apps/apple/src/account.rs` (`kn_account_*`, `kn_console_new_account`, `KN_ACCOUNT_*`) et `apps/android/jni/src/account.rs` (`Native.account*`, `consoleNewAccount`) ; les apps Windows, Linux et webOS appellent le cœur directement.
- **Textes** : [link-ui.md](link-ui.md), « Comptes KyberNow ».

### Données gardées par le relais

| Donnée | Pourquoi | Durée |
|---|---|---|
| E-mail, empreinte Argon2 du mot de passe, secret TOTP (chiffré), passkeys (clés publiques) | Se connecter | Vie du compte |
| Appareils connectés (nom, sorte, dernière venue) | Sessions d'appareil, révocation | Jusqu'à la déconnexion, ou 90 jours sans servir |
| Stations rattachées (identifiant, nom) | Ordinateurs du compte | Jusqu'au détachement |
| Droits donnés (personne ou adresse invitée, droits, échéance) | Personnes ajoutées | Jusqu'au retrait ou au détachement ; une invitation échue part après 7 jours |
| Organisations, membres, invitations, groupes, droits | Flotte | Vie de l'organisation |
| Jetons d'enrôlement (empreinte, début, nombre) | Déployer une flotte | Vie de l'organisation |
| Réglages synchronisés (par compte et par ordinateur, 4 Ko au plus) | Les réglages qui suivent la personne | Tant que le compte a des droits sur l'ordinateur |
| Journal (accès, changements ; journal de l'organisation) | Sécurité, flotte | 12 mois (adresses IP : 7 jours) |
| Adresses IP | Sécurité, limites | Quelques jours (journaux), comme aujourd'hui |

Compte supprimé : effacé sous 30 jours (sauvegardes comprises). Export des données depuis la console.

### E-mails

SMTP réglé dans `relay.toml` (hôte, port, identifiants, expéditeur) ; sans SMTP, un relais auto-hébergé n'a ni vérification d'e-mail ni mot de passe oublié (la console le dit, l'administrateur du relais crée les comptes). Messages en français et en anglais : vérification, mot de passe oublié, nouvel appareil, ajout à un ordinateur ou à une organisation, double authentification changée, passkey ajoutée, compte supprimé, Admin ouvert à distance.

**Mise en forme** (`server/relay/src/accounts/mail.rs`) : chaque e-mail part en HTML avec sa version texte, rendues depuis le même contenu (un titre, des paragraphes, un encadré pour le détail ou le code, un bouton, une note « Ce n'était pas vous ? » et un pied qui dit pourquoi l'e-mail arrive). Charte claire : fond `#F2F5F6`, carte blanche, bouton du signal `#5340E6`, polices du système. Tableaux et styles en ligne pour Outlook et les webmails ; logo en PNG (`/assets/kybernow-share-mail.png`, sans version, pour que les e-mails déjà envoyés le gardent), Gmail et Outlook ne montrant pas le SVG. Avec SendGrid, l'en-tête `X-SMTPAPI` coupe le suivi des clics et des ouvertures : les liens restent ceux du relais. Pour les regarder : `KN_PAGES_DIR=… cargo test ecrire_les_e_mails -- --ignored`.

### Sécurité du relais

- Mots de passe : Argon2id ; 10 caractères au moins ; liste des mots de passe connus refusée.
- Console : cookie de session `__Host-`, `Secure`, `HttpOnly`, `SameSite=Lax`, protection CSRF (jeton par formulaire, `Sec-Fetch-Site` vérifié) ; limites d'essais par adresse et par compte.
- Clé de signature : dans un fichier à part, réservé au relais ; rotation possible (deux clés publiées, la Station épingle les deux pendant la transition).
- Un relais piraté peut délivrer des jetons pour toute Station dont l'interrupteur est actif : c'est le prix du « relais décide », borné par les garde-fous ci-dessus.

## Juridique et magasins

- **RGPD** (Les Lignes Claires, responsable du traitement) : registre des traitements, politique de confidentialité du service (relais public), conditions d'utilisation, contrat de sous-traitance pour les organisations, notification d'une fuite sous 72 h, prestataire d'e-mails : SendGrid (Twilio, États-Unis, cadre UE–États-Unis et clauses contractuelles types).
- **App Store** : connexion dans l'app, création et suppression sur le site (pas de création dans l'app) ; pas de Sign in with Apple (ni Google ni autre connexion tierce) ; un **compte de démonstration** dans les notes de l'App Review, plus simple que l'invitation.
- **Payant, plus tard** : abonnement vendu dans l'app iOS = achat intégré d'Apple (3.1.1) ; offre vendue directement aux organisations = possible hors achat intégré (3.1.3(c)).

## Lots

1. **Relais, comptes** : base SQLite, création, vérification, connexion, mot de passe oublié, TOTP, passkeys, console (compte), e-mails, limites.
2. **Rattachement et jetons** : clé de signature, rattachement depuis Admin, jeton d'accès, vérification dans Admin, les quatre garde-fous.
3. **Apps** : connexion par fenêtre du système et par code, ordinateurs du compte à l'accueil, jeton dans chaque appel du cœur, plusieurs relais.
4. **Personnes et droits** : ajouter par e-mail, droits Session, Prêt, Partage, Administrer ; prêt et partage avec un compte. **Fait côté relais et Admin** (« Personnes, droits et organisations »).
5. **Organisations** : membres, rôles, groupes, ordinateurs de l'organisation, journal, double authentification obligatoire. **Fait côté relais et Admin** ; l'installateur prend le jeton d'enrôlement (« Installateurs »).
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
| `GET/POST /compte/supprimer` | « Supprimer le compte » : mot de passe ou passkey redemandé, puis effacement (ordinateurs rattachés détachés, appareils connectés fermés) et e-mail de confirmation |
| `POST /compte/export` | « Exporter mes données » : `kybernow-compte.json` (compte, conditions acceptées, sécurité sans secret, sessions, ordinateurs rattachés, appareils connectés, journal) |
| `GET /conditions`, `/confidentialite` | Conditions d'utilisation et politique de confidentialité ([juridique](juridique/README.md)), brouillons marqués comme tels, servis avec les comptes |

### Base (SQLite, `accounts.db` dans le volume du relais)

`accounts` (id, e-mail unique en minuscules, nom, empreinte Argon2id, vérifié le, créé le, supprimé le, offre) · `email_tokens` (empreinte SHA-256 du jeton, compte, usage `verify`/`reset`, code à six chiffres, échéance) · `sessions` (empreinte du jeton, compte, appareil, créée, vue, échéance, 2FA faite) · `totp` (compte, secret chiffré, activée le) · `recovery_codes` (compte, empreinte, utilisé le) · `passkeys` (compte, identifiant, clé publique, compteur, nom, créée, vue) · `trusted_browsers` (empreinte, compte, échéance) · `audit` (compte, type, adresse, quand). Migrations versionnées au démarrage.

Migration 2 (lot 1b) : sur `accounts`, `webauthn_user` (identifiant WebAuthn, 32 octets aléatoires), `terms_version`, `terms_accepted_at`, `privacy_version`, `privacy_accepted_at` ; sur `passkeys`, `alg` (COSE) et `transports` ; `webauthn_used` (empreinte d'un défi déjà servi, échéance). La migration 3 est celle du lot 2 (voir « Rattachement et appareils sur le relais »).

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

Écarts des lots 4 et 5 :

- la double authentification exigée pour Administrer vaut **aussi pour le propriétaire** de l'ordinateur (au lot 2, il avait tous les droits sans condition) ; une passkey la vaut, comme à la connexion ;
- un compte existant ajouté à un ordinateur ou à une organisation l'est **tout de suite**, sans accepter (l'e-mail le prévient) ; une adresse sans compte attend la vérification de l'adresse ;
- pas d'échéance pour les droits donnés à une organisation ou à un groupe (seulement aux personnes), ni de rôle par groupe ; les créneaux et le compte Steam du prêt restent ceux des comptes d'amis ;
- le journal de l'organisation garde les ouvertures de ses propres ordinateurs ; celles d'un ordinateur de personne ouvert à l'organisation restent dans le journal du compte du propriétaire ;
- « Copier » le jeton d'enrôlement n'est pas un bouton (les pages n'ont pas de script) : le jeton se sélectionne d'un clic ;
- la page « Ajouter une personne » est un formulaire de la page, pas une fenêtre par-dessus comme dans la maquette.

## Question ouverte

1. **Compte de démonstration pour Apple** : sur le relais public, avec le droit Session sur la Station de démonstration (proposé).
