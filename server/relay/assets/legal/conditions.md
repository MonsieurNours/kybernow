# Conditions d'utilisation du service KyberNow

[English](conditions.en.md) · Français

> **Brouillon à faire relire par un juriste.** Ce texte n'est pas en vigueur et ne doit pas être publié tel quel. Il a été rédigé à partir du cadrage des comptes ([comptes.md](../comptes.md)) et de ce que fait réellement le relais ([relais.md](../relais.md)). Les passages entre crochets (`[À COMPLÉTER]`, `[À DÉCIDER]`) attendent une information ou une décision ; les notes « Pour le juriste » posent les questions à trancher et disparaissent de la version publiée. Version 0.1 du 23/11/2026.

## 1. Qui nous sommes

Le service KyberNow est fourni par **Matthieu Requenna, entrepreneur individuel, exerçant sous le nom Les Lignes Claires** (« Les Lignes Claires », « nous »).

- SIREN : 510 178 577 (SIRET 510 178 577 00019) `[À VÉRIFIER : identité légale au moment du lancement, et si une société la remplace]`
- Adresse : Bâtiment 1, appartement 3, 230 avenue du Maréchal Leclerc, 34070 Montpellier, France `[À VÉRIFIER]`
- Contact : contact@leslignesclaires.fr `[À DÉCIDER : adresse dédiée au service, par exemple support@kybernow.app]`

## 2. De quoi parlent ces conditions

Ces conditions s'appliquent au **service KyberNow** fourni par Les Lignes Claires, c'est-à-dire :

- les **comptes KyberNow** créés sur le relais public `share.kybernow.app` (inscription, connexion, sécurité, appareils connectés) ;
- la **console web** du relais (`https://share.kybernow.app/compte`), où vous gérez votre compte, vos ordinateurs, les personnes à qui vous les ouvrez et vos organisations ;
- le **relais public** `share.kybernow.app`, qui fait passer la connexion entre vos appareils et vos ordinateurs quand ils ne sont pas sur le même réseau, et qui sert au partage (KyberNow Share) ;
- les **e-mails** que le service vous envoie.

Elles ne s'appliquent pas :

- aux **logiciels KyberNow** eux-mêmes (KyberNow Station, Admin, les apps Link, le relais à installer soi-même), qui sont des logiciels libres publiés sous licence AGPL-3.0 et régis par cette licence. Vous pouvez les utiliser sans compte, sur votre réseau, par appairage ou par invitation ;
- aux **relais installés par d'autres** (une entreprise, une association, une personne) : chaque relais a ses propres comptes et ses propres règles, fixées par celui qui le tient. Les Lignes Claires n'y ont pas accès et n'en répondent pas ;
- aux **boutiques d'applications** (App Store, Google Play, Microsoft Store…), dont les conditions s'appliquent en plus quand vous y téléchargez une app.

La manière dont nous traitons vos données personnelles est décrite dans la [politique de confidentialité](confidentialite.md), qui fait partie de ces conditions.

## 3. Accepter ces conditions

Vous acceptez ces conditions en créant votre compte (case à cocher de la page « Créer un compte »). Si vous ne les acceptez pas, ne créez pas de compte : les logiciels KyberNow restent utilisables sans compte.

Vous devez avoir **`[À DÉCIDER : 15 ans]`** au moins pour créer un compte. En dessous, un parent ou tuteur crée et tient le compte.

> **Pour le juriste.** Âge minimum : la base légale du compte est le contrat, pas le consentement, mais un mineur peut-il conclure ce contrat seul (actes de la vie courante, article 1148 du Code civil) ? Faut-il un âge minimum, et lequel ?

Si vous créez un compte ou une organisation pour une entreprise, une association ou une administration, vous déclarez avoir le pouvoir de l'engager ; ces conditions s'appliquent alors aussi à elle. Pour une organisation qui nous confie des données de ses membres, le [contrat de sous-traitance](dpa.md) s'ajoute à ces conditions.

## 4. Votre compte

### 4.1 Créer et tenir son compte

- Un compte, c'est une adresse e-mail que vous vérifiez, un nom et un mot de passe. Vous pouvez y ajouter des passkeys et la double authentification ; la double authentification est **obligatoire** pour les Propriétaires et Administrateurs d'une organisation, et pour tout compte qui a le droit Administrer sur un ordinateur.
- Le compte se crée et se supprime **sur le site du relais**, pas dans les apps. Les apps s'y connectent par une fenêtre du système ou par un code à approuver depuis votre téléphone.
- Vous donnez des informations exactes, en particulier votre adresse e-mail : c'est par elle que vous récupérez votre compte et que nous vous prévenons d'un nouvel appareil.
- Un compte est **personnel**. Ne le partagez pas : pour ouvrir un ordinateur à quelqu'un, ajoutez cette personne avec son propre compte (section 5).

### 4.2 Garder son compte en sécurité

Vous gardez secrets votre mot de passe, vos codes de secours et l'accès à vos passkeys. Vous nous prévenez sans attendre si vous pensez que quelqu'un d'autre a pu entrer dans votre compte (contact en section 1), et vous fermez les sessions ouvertes depuis la page du compte. Tout ce qui est fait depuis votre compte est présumé fait par vous, sauf si vous nous aviez signalé une intrusion ou si elle vient d'un défaut de notre service.

## 5. Ordinateurs, droits et accès à distance

### 5.1 Ce que fait le service

Le service sert à **prendre la main sur des ordinateurs à distance** : voir leur écran, entendre leur son, et les commander au clavier, à la souris et à la manette. C'est puissant. Lisez cette section avec soin.

- **Rattacher un ordinateur.** Vous rattachez à votre compte, ou à une organisation, un ordinateur où KyberNow Station est installée, depuis cet ordinateur (KyberNow Admin ou l'installateur). Vous ne rattachez que des ordinateurs que vous avez le droit d'administrer : les vôtres, ou ceux dont le propriétaire ou l'employeur vous l'a permis.
- **Ouvrir un ordinateur à quelqu'un.** Depuis la console, vous donnez à une personne, à une organisation ou à un groupe des droits sur un ordinateur :
  - **Session** : le bureau entier de l'ordinateur, avec le clavier et la souris ;
  - **Prêt** : les seules applications que vous avez choisies dans Admin, aux créneaux que vous avez fixés ;
  - **Partage** : rejoindre vos partages **sans code et sans votre accord** à chaque fois ;
  - **Administrer** : gérer les accès de l'ordinateur, le redémarrer et mettre à jour la Station, y compris par le relais, et ouvrir Admin sur le réseau.
- **Le relais décide.** Quand un compte a un droit sur un ordinateur, le relais lui délivre un jeton d'accès que la Station accepte sans vous redemander votre accord. Vous gardez la main sur la Station : l'interrupteur « Accepter les comptes KyberNow » dans Admin, la liste des accès, le blocage d'un compte ou d'un appareil, et un e-mail à chaque nouvel appareil.

### 5.2 Vos responsabilités sur vos ordinateurs

En rattachant un ordinateur et en l'ouvrant à d'autres, vous restez responsable de ce que vous choisissez :

- **À qui vous donnez accès, et quels droits.** Une personne qui a le droit Session peut faire sur l'ordinateur tout ce que vous pourriez faire assis devant : lire vos fichiers, utiliser vos comptes déjà ouverts, installer un logiciel. Ne donnez ce droit qu'à quelqu'un en qui vous avez confiance, et retirez-le quand il ne sert plus (une échéance peut le faire pour vous).
- **Les personnes qui utilisent l'ordinateur.** Si d'autres personnes se servent de l'ordinateur (famille, collègues, élèves), vous les prévenez qu'il peut être ouvert à distance, par qui, et comment le voir ou l'empêcher.
- **Les logiciels et comptes que vous prêtez.** Le droit Prêt et le partage font utiliser vos applications, jeux et comptes (un compte Steam par exemple) par quelqu'un d'autre. Vous vérifiez que les licences et les conditions de ces logiciels et services le permettent ; Les Lignes Claires n'en répondent pas.
- **Les mises à jour.** Vous tenez à jour KyberNow Station et le système de vos ordinateurs.

### 5.3 Vos responsabilités sur les ordinateurs qu'on vous ouvre

Quand quelqu'un vous ouvre son ordinateur :

- vous n'utilisez que les droits qu'on vous a donnés, dans le but pour lequel on vous les a donnés ;
- vous ne cherchez pas à en obtenir d'autres, ni à contourner le Prêt, les créneaux, les blocages ou les garde-fous de la Station ;
- vous respectez la vie privée des personnes dont vous voyez l'écran, les fichiers ou les conversations ;
- vous sortez quand on vous le demande, et quand vos droits sont retirés.

Entrer ou rester sans autorisation dans un ordinateur, ou y modifier des données, est une infraction pénale (articles 323-1 et suivants du Code pénal).

### 5.4 Ordinateurs d'une organisation

Une organisation qui rattache les ordinateurs de son personnel ou les ouvre à ses membres agit sous sa propre responsabilité d'employeur ou de responsable : informer les personnes concernées, consulter les représentants du personnel quand la loi le demande, ne pas se servir du journal pour surveiller le travail au-delà de ce que la loi permet. Elle est responsable des données de ses membres qu'elle nous confie, et nous les traitons pour elle selon le [contrat de sous-traitance](dpa.md).

## 6. Organisations

- Une **organisation** regroupe des membres avec un rôle : **Propriétaire**, **Administrateur** ou **Membre**, des groupes et des ordinateurs. Les Propriétaires et Administrateurs ajoutent et retirent les membres, donnent les droits et voient le journal de l'organisation (qui a ouvert quoi, quand, d'où, et chaque changement de droits).
- La personne qui crée l'organisation en est la première Propriétaire. Une organisation a toujours au moins un Propriétaire : le dernier ne peut ni partir ni supprimer son compte sans désigner un autre Propriétaire ou supprimer l'organisation.
- Quand un membre quitte l'organisation, il perd les droits qu'il tenait d'elle. Son compte reste à lui.
- `[À DÉCIDER : sort d'une organisation dont aucun Propriétaire ne répond plus (procédure de reprise, preuve demandée).]`

## 7. Usage acceptable

Vous n'utilisez pas le service pour :

- accéder à un ordinateur sans l'autorisation de la personne qui en a le droit, ou surveiller quelqu'un à son insu ;
- commettre ou faciliter une infraction, porter atteinte aux droits d'autrui (vie privée, droits d'auteur, secrets d'affaires), harceler ou tromper quelqu'un ;
- envoyer des invitations ou ajouts non sollicités en nombre, ou vous servir des e-mails du service pour joindre des personnes qui ne vous connaissent pas ;
- attaquer, sonder ou surcharger le relais, ses comptes ou d'autres utilisateurs, ou contourner ses limites (nombre d'essais, nombre de codes, flux par client) ;
- revendre ou mettre à disposition le relais public comme un service à vous, ou en faire un relais pour d'autres usages que KyberNow ;
- créer des comptes en masse, par automate, ou sous l'identité de quelqu'un d'autre.

Pour un usage intensif ou professionnel au-delà de ce que permet l'offre en cours, vous pouvez aussi **installer votre propre relais** : le logiciel est libre.

## 8. Ce que nous faisons, et nos limites

### 8.1 Le service

- Nous faisons de notre mieux pour que le relais et la console fonctionnent, soient sûrs et à jour. KyberNow est en **bêta** : des fonctions changent, certaines n'ont pas encore été essayées partout, des interruptions peuvent arriver (maintenance, panne, mise à jour).
- Le relais public est tenu **en France**, sur du matériel des Lignes Claires. `[À VÉRIFIER : hébergement réel au lancement, et lieu des sauvegardes.]` Nous ne garantissons ni une disponibilité chiffrée ni un débit. `[À DÉCIDER : engagement de disponibilité pour l'offre payante.]`
- Dans les apps KyberNow Link, l'image, le son et vos commandes sont **chiffrés de bout en bout** entre l'app et l'ordinateur : le relais ne peut pas les lire. Dans un navigateur, le relais déchiffre la connexion pour la faire suivre, sans rien en garder ([confidentialité](confidentialite.md)).
- Nous pouvons faire évoluer le service : ajouter, changer ou retirer une fonction. Si un changement vous retire une fonction importante, nous vous prévenons à l'avance par e-mail.

### 8.2 Responsabilité

- Nous répondons des dommages causés par un manquement de notre part à ces conditions, dans les limites de la loi.
- Nous ne répondons pas : de ce que font les personnes à qui **vous** avez donné des droits ; de l'usage que vous faites des ordinateurs qu'on vous ouvre ; des pertes liées à un ordinateur, un réseau, une box ou un logiciel qui ne sont pas les nôtres ; d'une interruption due à un cas de force majeure ; des relais tenus par d'autres.
- `[À DÉCIDER avec le juriste : plafond de responsabilité pour les professionnels ; pour l'offre gratuite.]`
- Rien dans ces conditions ne limite notre responsabilité en cas de faute lourde ou intentionnelle, de dommage corporel, ni les droits que vous donne la loi en tant que consommateur, en particulier la garantie de conformité des services numériques.

> **Pour le juriste.** (1) Les clauses limitatives sont-elles valables face à un consommateur pour un service gratuit (article R. 212-1 du Code de la consommation) ? (2) Le régime des contenus et services numériques (articles L. 224-25-1 et suivants du Code de la consommation) s'applique-t-il dès maintenant, le service étant gratuit mais fourni contre des données personnelles ? (3) Faut-il une rédaction distincte pour les professionnels (organisations) ?

## 9. Gratuité et offre payante à venir

- **Aujourd'hui, le service est gratuit.** Aucun moyen de paiement ne vous est demandé.
- Une **offre payante** viendra plus tard (par exemple pour les organisations ou des usages plus lourds). Le jour où elle arrive :
  - nous vous prévenons par e-mail **au moins `[À DÉCIDER : 30]` jours** avant, avec ses prix et ce qu'elle comprend ;
  - **rien ne devient payant sans votre accord exprès** : vous n'êtes jamais facturé parce que vous continuez à utiliser le service ;
  - si une fonction que vous utilisez gratuitement devient payante, vous pouvez la garder en souscrivant, y renoncer, ou supprimer votre compte et récupérer vos données ;
  - l'offre payante aura ses propres conditions de vente (prix, durée, renouvellement, rétractation, résiliation, remboursement), que vous accepterez à part.
- Un abonnement vendu dans une app iOS passe par l'achat intégré d'Apple et suit aussi les règles d'Apple ; une offre vendue directement aux organisations se règle avec nous.

> **Pour le juriste.** Le site dit aujourd'hui « KyberNow est un projet communautaire et non commercial ». Cette phrase devra changer avant l'offre payante. Vérifier aussi l'obligation d'un médiateur de la consommation (articles L. 612-1 et suivants) dès le service gratuit ou seulement à l'offre payante.

## 10. Suspension

Nous pouvons **suspendre** un compte, une organisation ou l'accès d'un ordinateur au relais, en tout ou en partie :

- en cas de manquement grave à ces conditions (section 7 en particulier) ;
- pour protéger le service, d'autres utilisateurs ou un ordinateur, par exemple si un compte semble piraté ou sert à une attaque ;
- sur demande d'une autorité, quand la loi nous y oblige.

Nous vous prévenons par e-mail, avec la raison, avant la suspension quand c'est possible, sinon tout de suite après, sauf si la loi ou une enquête nous en empêche. Vous pouvez nous répondre pour contester ; nous rétablissons le compte si la raison disparaît. Une suspension pour protéger un compte piraté est levée dès que vous l'avez repris en main.

## 11. Résiliation et suppression du compte

### 11.1 Par vous

Vous pouvez **supprimer votre compte à tout moment**, sans frais, depuis la page du compte (« Supprimer le compte »). Avant, vous pouvez **exporter vos données** depuis la console.

- Vos sessions et appareils sont déconnectés tout de suite, et vos ordinateurs détachés : ils ne sont plus joignables par un compte, mais restent utilisables sur le réseau, par appairage et par invitation.
- Les droits que d'autres comptes avaient sur vos ordinateurs tombent avec eux ; ceux que vous aviez sur les ordinateurs des autres tombent aussi.
- Si vous êtes le dernier Propriétaire d'une organisation, vous désignez d'abord un autre Propriétaire ou supprimez l'organisation.
- Vos données sont effacées **sous 30 jours**, sauvegardes comprises, sauf ce que la loi nous oblige à garder plus longtemps ([confidentialité](confidentialite.md)).

### 11.2 Par nous

Nous pouvons fermer un compte :

- après une suspension restée sans réponse ou en cas de manquement grave ou répété, avec un préavis de `[À DÉCIDER : 15]` jours par e-mail, sauf urgence ;
- `[À DÉCIDER : compte jamais vérifié, effacé après 7 jours ? compte inactif depuis 3 ans, effacé après deux e-mails d'avertissement ?]` ;
- si nous arrêtons le service public de comptes : nous vous prévenons au moins `[À DÉCIDER : 3]` mois avant, et vous pouvez exporter vos données et passer à votre propre relais.

## 12. Propriété intellectuelle

Le code de KyberNow est un logiciel libre publié sous licence AGPL-3.0 ; ses sources sont sur GitHub. Le nom, le symbole et l'identité visuelle de KyberNow ne sont pas couverts par cette licence : vous ne les utilisez pas pour faire croire qu'un service ou un relais vient des Lignes Claires. KyberNow utilise Kyber, logiciel libre publié sous licence AGPL-3.0, et n'est ni édité ni approuvé par l'équipe de Kyber.

Ce que vous mettez dans le service (noms d'ordinateurs, d'organisations, de groupes) reste à vous ; vous nous permettez seulement de le garder et de l'afficher pour faire fonctionner le service.

## 13. Changer ces conditions

Nous pouvons modifier ces conditions, par exemple pour l'offre payante, une nouvelle fonction ou une loi nouvelle. Pour un changement important, nous vous prévenons par e-mail et dans la console **au moins 30 jours** avant son entrée en vigueur, en disant ce qui change. Si vous ne l'acceptez pas, vous pouvez supprimer votre compte avant cette date. Les corrections mineures (une coquille, une adresse) s'appliquent tout de suite. Les versions précédentes restent consultables. `[À DÉCIDER : où ; garder la date et la version acceptées par chaque compte.]`

## 14. Droit applicable et litiges

- Ces conditions sont soumises au **droit français**. Si vous êtes un consommateur résidant dans un autre pays de l'Union européenne, vous gardez la protection que vous donne la loi de votre pays.
- En cas de désaccord, écrivez-nous d'abord (section 1) : nous cherchons une solution à l'amiable.
- Consommateur, vous pouvez aussi recourir gratuitement à un médiateur de la consommation : `[À COMPLÉTER : nom et adresse du médiateur]`. Vous pouvez saisir le tribunal compétent selon les règles de droit commun.
- Professionnel, les litiges vont au tribunal compétent de `[À DÉCIDER : Montpellier]`.
- Ces conditions existent en français et en anglais ; en cas de différence, la version française prévaut.
