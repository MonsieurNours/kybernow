# Politique de confidentialité du service KyberNow

[English](confidentialite.en.md) · Français

> **Brouillon à faire relire par un juriste.** Ce texte n'est pas en vigueur et ne doit pas être publié tel quel. Il décrit ce que le relais garde réellement d'après le cadrage ([comptes.md](../comptes.md), section « Données gardées par le relais ») et [relais.md](../relais.md) ; si le code change, ce texte doit changer avec lui. Les passages entre crochets attendent une information ou une décision ; les notes « Pour le juriste » disparaissent de la version publiée. Version 0.1 du 23/11/2026.

Cette politique dit quelles données personnelles le **service KyberNow** traite (les comptes, la console et le relais public `share.kybernow.app`), pourquoi, combien de temps, qui les voit, et comment exercer vos droits. Elle complète les [conditions d'utilisation](conditions.md).

Le site www.kybernow.app et les logiciels utilisés sans compte ont leurs propres règles, décrites dans les mentions légales du site : chez vous, sur votre réseau, les logiciels KyberNow ne transmettent rien aux Lignes Claires.

## 1. Le responsable du traitement

**Matthieu Requenna, entrepreneur individuel, exerçant sous le nom Les Lignes Claires**, SIREN 510 178 577, Bâtiment 1, appartement 3, 230 avenue du Maréchal Leclerc, 34070 Montpellier, France. `[À VÉRIFIER au lancement.]`

Contact pour vos données : **contact@leslignesclaires.fr** `[À DÉCIDER : adresse dédiée, par exemple confidentialite@kybernow.app]`.

Nous n'avons pas désigné de délégué à la protection des données : la loi ne nous y oblige pas. La personne qui répond à vos questions est l'éditeur lui-même.

> **Pour le juriste.** Confirmer que la désignation d'un DPO n'est pas obligatoire (article 37 du RGPD : pas de suivi régulier et systématique à grande échelle ?), compte tenu du journal des accès des organisations.

**Organisations.** Quand une entreprise, une association ou une administration utilise le service pour ses membres et ses ordinateurs, c'est **elle** qui décide qui a accès à quoi et qui lit son journal : elle est responsable de ces données, et nous les traitons pour elle, comme sous-traitant, selon un [contrat de sous-traitance](dpa.md). Pour ces données, adressez-vous d'abord à l'organisation ; nous lui transmettons les demandes que nous recevons.

## 2. Les données que nous traitons

### 2.1 Votre compte

| Donnée | À quoi elle sert |
|---|---|
| Adresse e-mail, nom | Vous identifier, vous écrire (vérification, mot de passe oublié, nouvel appareil, ajouts), vous retrouver quand quelqu'un vous ajoute par e-mail |
| Empreinte du mot de passe (Argon2id) | Vérifier votre mot de passe sans jamais le garder en clair |
| Secret de double authentification (chiffré), empreintes des codes de secours | Double authentification |
| Passkeys : identifiant, clé publique, compteur, nom, dates | Connexion par passkey (la clé privée reste sur votre appareil) |
| Dates : création, vérification, suppression demandée ; offre du compte | Gérer le compte |
| `[À DÉCIDER : version des conditions acceptée et date]` | Prouver l'acceptation des conditions |

### 2.2 Vos appareils et sessions

| Donnée | À quoi elle sert |
|---|---|
| Appareils connectés : nom (« iPad de Claire »), sorte, dernière venue ; sessions (empreinte du jeton, dates, double authentification faite) | Garder vos appareils connectés, vous les montrer, les déconnecter |
| Navigateurs de confiance (empreinte, échéance de 30 jours) | Ne pas redemander la double authentification sur ce navigateur |
| Jetons d'e-mail (empreinte, usage, échéance de 24 heures ou d'une heure) | Vérifier votre adresse, changer un mot de passe oublié |

### 2.3 Vos ordinateurs, vos droits et vos organisations

| Donnée | À quoi elle sert |
|---|---|
| Ordinateurs rattachés : identifiant de Station, nom (« PC du salon »), propriétaire (vous ou une organisation) | Montrer vos ordinateurs dans les apps, délivrer les accès |
| Droits : qui a quel droit (Session, Prêt, Partage, Administrer) sur quel ordinateur, échéance | Décider de l'accès |
| Organisations : nom, membres et rôles, groupes | Gérer la flotte d'une organisation |
| Personnes ajoutées par e-mail qui n'ont pas encore de compte : adresse e-mail, droits proposés | Leur envoyer l'invitation et rattacher les droits quand elles créent leur compte |
| Jetons d'accès délivrés (identifiant, compte, appareil, ordinateur, droits ; valables 5 minutes) | Ouvrir un ordinateur ; empêcher qu'un jeton serve deux fois |

### 2.4 Le journal

| Donnée | À quoi elle sert |
|---|---|
| Événements de sécurité du compte : connexion, échec, nouvel appareil, double authentification changée, avec la date et l'adresse IP | Vous montrer l'activité de votre compte, détecter une intrusion |
| Journal des accès et des droits : qui a ouvert quel ordinateur, quand, depuis quelle adresse IP et quel appareil ; chaque changement de droits et par qui | Sécurité ; pour une organisation, savoir qui a accédé à ses ordinateurs |

### 2.5 Le passage par le relais

Le relais fait passer la connexion entre vos appareils et vos ordinateurs. Il voit :

- les **adresses IP** des appareils et des ordinateurs, l'heure des connexions, le volume et le débit des données ;
- les **codes de partage**, le temps de mettre les personnes en relation (jamais écrits dans les journaux) ;
- dans les apps KyberNow Link, **rien de ce que vous voyez ou tapez** : l'image, le son, le clavier, la souris et la manette sont chiffrés de bout en bout entre l'app et l'ordinateur ;
- dans un **navigateur**, le relais déchiffre la connexion pour la faire suivre à l'ordinateur ; il n'en garde rien.

### 2.6 Ce que nous n'avons pas

Pas de moyen de paiement (le service est gratuit), pas de mesure d'audience, pas de publicité, pas de cookie de suivi, pas de connexion par Apple, Google ou un autre tiers. Nous ne voyons pas vos écrans, vos fichiers ni vos frappes.

### 2.7 Vos échanges avec nous

Si vous nous écrivez : votre adresse, votre message et notre réponse.

## 3. Pourquoi, et sur quelle base légale

| Finalité | Base légale (article 6 du RGPD) |
|---|---|
| Créer et tenir votre compte, vous connecter, vos appareils, vos ordinateurs et vos droits, les e-mails de service | **Exécution du contrat** (6.1.b) : les conditions d'utilisation |
| Gérer les organisations, leurs membres, leurs groupes et leur journal | Pour le compte de l'organisation, qui en est responsable ([contrat de sous-traitance](dpa.md)) ; pour nous, **exécution du contrat** avec elle |
| Sécurité du compte et du relais : journal de sécurité, avis de nouvel appareil, limites d'essais, journaux techniques avec les adresses IP, lutte contre les abus | **Intérêt légitime** (6.1.f) : protéger vos comptes, vos ordinateurs et le service |
| Répondre à vos demandes et à l'exercice de vos droits | **Obligation légale** (6.1.c) pour les droits ; **intérêt légitime** pour le reste |
| Garder ce que la loi impose, répondre à une autorité qui le demande | **Obligation légale** (6.1.c) |
| `[Plus tard : facturation de l'offre payante]` | Exécution du contrat et obligation légale (comptabilité) |

Nous n'utilisons pas vos données pour de la publicité, du profilage ou des décisions automatisées qui vous concernent. La seule décision automatique est l'accès lui-même : le relais délivre un jeton si votre compte a le droit, et limite les essais trop nombreux.

## 4. Combien de temps

| Donnée | Durée |
|---|---|
| Compte (e-mail, nom, mot de passe, double authentification, passkeys) | Vie du compte |
| Appareils connectés et sessions | Vie de l'appareil : jusqu'à sa déconnexion ou la fin de la session `[À PRÉCISER : durée d'une session d'appareil inactive]` |
| Navigateurs de confiance | 30 jours |
| Jetons d'e-mail | 24 heures (vérification) ou une heure (mot de passe) |
| Jetons d'accès | 5 minutes ; leur identifiant le temps d'empêcher qu'ils resservent |
| Ordinateurs rattachés | Jusqu'au détachement |
| Droits | Jusqu'à leur retrait ou leur échéance |
| Organisations, membres, groupes | Vie de l'organisation ; un membre retiré disparaît tout de suite |
| Personnes ajoutées sans compte | `[À DÉCIDER : 30 jours sans création de compte, puis effacées]` |
| Journal (sécurité, accès, droits) | **12 mois** |
| Journaux techniques du relais (adresses IP, heures de connexion) | **Quelques jours** (journaux tournés par taille) `[À PRÉCISER : durée maximale en jours]` |
| Codes de partage | Le temps du partage ; jamais dans les journaux |
| Comptes jamais vérifiés | `[À DÉCIDER : 7 jours]` |
| Comptes inactifs | `[À DÉCIDER : 3 ans sans connexion, après deux e-mails d'avertissement]` |
| Échanges avec nous | `[À DÉCIDER : 3 ans après le dernier échange]` |

**Compte supprimé** : toutes ses données sont effacées **sous 30 jours, sauvegardes comprises**. Les lignes du journal d'une organisation qui le concernent restent à l'organisation jusqu'à la fin de leur durée de 12 mois `[À DÉCIDER : ou anonymisées à la suppression du compte]`.

> **Pour le juriste.** (1) Le journal garde l'adresse IP 12 mois, alors que les journaux techniques la gardent quelques jours : faut-il tronquer ou effacer l'adresse du journal plus tôt (par exemple après 3 mois) ? (2) Le décret n° 2021-1362 sur la conservation des données d'identification s'applique-t-il au service (accès, hébergement, ni l'un ni l'autre) ? Si oui, certaines données devraient être gardées un an après la suppression du compte, ce qui contredit « sous 30 jours ».

## 5. Qui voit vos données

- **Les Lignes Claires** : l'éditeur, seul à administrer le relais aujourd'hui.
- **Les personnes avec qui vous partagez** : le propriétaire d'un ordinateur (ou les Administrateurs de son organisation) voit votre nom, votre adresse e-mail, vos droits et vos accès à cet ordinateur ; il reçoit un e-mail la première fois qu'un de vos appareils l'ouvre ; la Station garde une liste locale des comptes et appareils entrés. Les Propriétaires et Administrateurs d'une organisation voient ses membres et son journal.
- **Nos sous-traitants**, chacun pour sa seule tâche :

| Sous-traitant | Tâche | Où |
|---|---|---|
| `[À COMPLÉTER : prestataire d'e-mails]` | Envoyer les e-mails du service (adresse du destinataire, contenu du message) | Union européenne `[À VÉRIFIER : pays, et absence de transfert hors UE chez ce prestataire et ses propres sous-traitants]` |
| `[À COMPLÉTER : hébergeur ou fournisseur d'accès du relais, si le relais n'est plus sur le matériel des Lignes Claires]` | Héberger le relais | France |
| `[À COMPLÉTER : stockage des sauvegardes, s'il est externe]` | Garder les sauvegardes chiffrées de la base | `[À COMPLÉTER]` |

- **Les autorités** (police, justice, CNIL) quand la loi nous oblige à leur répondre, sur demande écrite et dans les limites de cette demande.

Nous ne vendons ni ne louons vos données, et ne les donnons à personne d'autre.

## 6. Transferts hors de l'Union européenne

Le relais est en France ; le prestataire d'e-mails est dans l'Union européenne. Nous ne transférons pas vos données hors de l'Union européenne. Si cela devait changer, cette politique le dirait avant, avec la garantie employée (décision d'adéquation ou clauses contractuelles types de la Commission européenne).

Quand vous ouvrez un ordinateur depuis un autre pays, la connexion passe par le relais en France : c'est vous qui la faites passer, pas nous.

## 7. Sécurité

- Mots de passe gardés en empreinte **Argon2id** ; 10 caractères au moins ; mots de passe connus refusés.
- **Double authentification** (application d'authentification) et **passkeys** ; double authentification obligatoire pour les rôles qui administrent.
- Secrets de double authentification **chiffrés** ; jetons de session, d'e-mail et codes de secours gardés en **empreinte** seulement.
- Console : connexion chiffrée (HTTPS), cookies réservés au site (`__Host-`, `Secure`, `HttpOnly`), protection contre les requêtes forgées, **limites d'essais** par adresse et par compte ; réponses identiques qu'un compte existe ou non.
- Accès aux ordinateurs par des **jetons signés valables 5 minutes**, que la Station vérifie ; sur la Station, un interrupteur, une liste des accès avec blocage, et un avis à chaque nouvel appareil.
- Dans les apps, **chiffrement de bout en bout** entre l'app et l'ordinateur : le relais ne lit ni l'image ni les commandes.
- Adresses e-mail et jetons **jamais écrits dans les journaux** ; codes de partage non plus.
- Base sauvegardée `[À COMPLÉTER : fréquence, chiffrement, lieu des sauvegardes]` ; clé de signature dans un fichier réservé au relais.

Aucun système n'est sûr à cent pour cent. Si vous découvrez une faille, écrivez-nous `[À DÉCIDER : adresse de signalement, fichier security.txt]`.

## 8. En cas de violation de données

Si une violation de vos données arrive (fuite, perte, accès par quelqu'un qui n'en avait pas le droit) :

- nous la notifions à la **CNIL dans les 72 heures** après en avoir eu connaissance, sauf si elle ne présente pas de risque pour vous ;
- si elle présente un **risque élevé** pour vous (par exemple si des accès à vos ordinateurs ont pu être délivrés à tort), nous vous prévenons **sans tarder**, par e-mail, en disant ce qui s'est passé, ses conséquences probables, ce que nous faisons et ce que vous pouvez faire (changer votre mot de passe, couper « Accepter les comptes KyberNow » dans Admin, vérifier la liste des accès) ;
- pour une organisation, nous prévenons ses Propriétaires dans le délai fixé par le [contrat de sous-traitance](dpa.md) ;
- nous tenons un registre de toutes les violations, même celles qui ne sont pas notifiées.

## 9. Cookies

La console dépose seulement les cookies nécessaires au service, qui ne demandent pas votre consentement :

| Cookie | À quoi il sert | Durée |
|---|---|---|
| `__Host-kn_session` | Vous garder connecté | Durée de la session `[À PRÉCISER]` |
| Navigateur de confiance | Ne pas redemander la double authentification | 30 jours |
| Protection des formulaires | Empêcher les requêtes forgées | Durée de la page |
| `__Host-kybernow_guest` (partage dans un navigateur) | Vous reconnaître comme invité d'un partage | Durée du partage `[À VÉRIFIER]` |

Pas de cookie de mesure d'audience, de publicité ou de réseau social. Les pages du relais ne chargent rien d'un autre site.

## 10. Vos droits

Vous avez le droit :

- d'**accéder** à vos données et d'en avoir une copie ;
- de les faire **rectifier** (votre nom et votre adresse se changent dans la console) ;
- de les faire **effacer** (« Supprimer le compte » dans la console) ;
- de **limiter** leur traitement, par exemple pendant que nous vérifions une contestation ;
- de les **emporter** dans un format lisible par une machine (« Exporter mes données » dans la console) ;
- de vous **opposer** à un traitement fondé sur notre intérêt légitime, pour des raisons tenant à votre situation (nous pouvons continuer pour des motifs impérieux, comme la sécurité de comptes d'autres personnes) ;
- de donner des **directives sur le sort de vos données après votre décès**.

**Comment faire.** La plupart se font dans la console, sans nous écrire. Sinon, écrivez à l'adresse de la section 1, depuis l'adresse de votre compte. Nous répondons **sous un mois** (trois mois pour une demande complexe, et nous vous le disons dans le premier mois). Nous pouvons vous demander de prouver que le compte est le vôtre, par exemple en vous connectant ; nous ne demandons pas de pièce d'identité sans raison.

**Données d'une organisation** : adressez-vous à l'organisation ; si vous nous écrivez, nous lui transmettons votre demande.

**Réclamation.** Si vous pensez que vos droits ne sont pas respectés, vous pouvez saisir la **CNIL** (Commission nationale de l'informatique et des libertés, 3 place de Fontenoy, TSA 80715, 75334 Paris Cedex 07, www.cnil.fr), ou l'autorité de votre pays dans l'Union européenne.

## 11. Relais installés par d'autres

Un relais KyberNow installé par quelqu'un d'autre (votre entreprise, une association) a ses propres comptes. Ses données ne passent pas par Les Lignes Claires : c'est celui qui tient ce relais qui en est responsable et qui doit vous informer.

## 12. Changements

Nous mettons cette politique à jour quand le service change. Pour un changement important (une nouvelle donnée, un nouveau sous-traitant, une nouvelle finalité), nous vous prévenons par e-mail et dans la console avant qu'il s'applique. La date de la dernière version est en tête de ce texte.
