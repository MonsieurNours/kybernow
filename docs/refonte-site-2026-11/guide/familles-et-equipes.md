---
slug: familles-et-equipes
titre: Familles et équipes
title: Gérer une famille ou une équipe · KyberNow
description: Réunissez votre famille ou votre équipe, donnez des rôles, ouvrez des groupes d'ordinateurs, suivez le journal et déployez des Stations en nombre.
rubrique: Comptes et équipes
univers: commun
duree: 15 min
connexes: [compte-kybernow, installer-station-windows, hors-de-chez-vous, installer-votre-relais]
---

Une famille ou une équipe réunit des personnes et des ordinateurs. Vous ouvrez un ordinateur, ou tout un groupe d'ordinateurs, à ses membres en une fois, et le journal garde la trace de chaque accès.

## Avant de commencer

- Un [compte KyberNow](/guide/compte-kybernow/) protégé par une passkey ou par la double authentification : créer une famille ou une équipe l'exige.
- Les ordinateurs à partager, avec KyberNow Station installée.

## Famille ou équipe

Une famille et une équipe fonctionnent de la même façon, avec les mêmes rôles, droits et groupes : « Famille Martin » d'un côté, « L'atelier » de l'autre. Une équipe a en plus l'onglet « Déployer », pour installer des Stations en nombre.

1. Sur la page du compte, ouvrez « Familles et équipes », puis « Créer une famille ou une équipe ».
2. Choisissez l'un ou l'autre et donnez-lui un nom. Vous en devenez Propriétaire.
3. La page de la famille ou de l'équipe a ses onglets : « Membres », « Groupes », « Ordinateurs », « Journal », et « Déployer » pour une équipe.

<!-- capture : la page d'une équipe, onglet « Membres » -->

## Les rôles

| Rôle | Ce qu'il peut faire |
|---|---|
| Propriétaire | Tout : membres, rôles, groupes, ordinateurs et journal. Lui seul nomme ou retire un autre Propriétaire, et fait sortir un ordinateur de l'équipe. |
| Administrateur | Invite et retire des membres, crée les groupes, rattache des ordinateurs, les ouvre aux membres, lit le journal et, dans une équipe, crée des jetons d'enrôlement. |
| Membre | Utilise les ordinateurs qui lui sont ouverts, avec les droits donnés. |

Les Propriétaires et les Administrateurs ont tous les droits sur les ordinateurs de la famille ou de l'équipe. Ils doivent avoir la double authentification ou une passkey : sans elle, ils n'ont que les droits d'un membre. Une famille ou une équipe garde toujours au moins un Propriétaire.

## Inviter les membres

1. Onglet « Membres », « Inviter un membre ».
2. Tapez l'adresse e-mail et choisissez le rôle.
3. Avec un compte, la personne trouve l'invitation dans « Invitations », sur sa page du compte, et l'accepte. Sans compte, l'e-mail lui propose d'en créer un ; l'invitation l'attend.

Une invitation expire après 30 jours. « Renvoyer l'invitation » la relance pour 30 jours, une fois par heure au plus.

## Les groupes de personnes

Un groupe réunit des membres qui ont les mêmes besoins : « Graphistes », « Montage », « Les enfants ».

1. Onglet « Groupes », « Créer un groupe », puis ajoutez-y des membres.
2. Sur la page d'un ordinateur, « Ouvrir à une famille ou une équipe » : choisissez toute la famille ou l'équipe, ou l'un de ses groupes, puis les droits.

Les droits sont ceux d'un compte : « Regarder », « Ouvrir le bureau », « Emprunter pour jouer », « Rejoindre vos partages » et « Administrer » (voir [Votre compte KyberNow](/guide/compte-kybernow/)). Un membre qui a reçu « Administrer » sans être Propriétaire ni Administrateur ne peut donner que les droits qu'il a lui-même.

## Les groupes d'ordinateurs

Un groupe d'ordinateurs réunit des postes à ouvrir ensemble : « Postes de l'atelier », « PC de la salle de jeu ».

1. Onglet « Ordinateurs », « Créer un groupe d'ordinateurs », puis ajoutez-y des ordinateurs de la famille ou de l'équipe.
2. « Ouvrir le groupe » : à toute la famille ou l'équipe, à un groupe de personnes ou à un membre, avec les droits et, si vous voulez, une date de fin.

Ces droits valent pour tous les ordinateurs du groupe, ceux d'aujourd'hui et ceux que vous y ajouterez. Ils s'ajoutent aux droits donnés ordinateur par ordinateur. Sur la page d'un ordinateur, « Qui y a accès » les montre sous « Par le groupe » suivi de son nom ; ils se changent sur le groupe. Un ordinateur peut être dans plusieurs groupes. « Supprimer le groupe » retire aussi les droits donnés sur lui.

## Les ordinateurs de la famille ou de l'équipe

- **Un nouvel ordinateur** : au rattachement, dans Admin, espace Compte KyberNow, choisissez la famille ou l'équipe comme propriétaire à la place de « Moi ».
- **Un ordinateur déjà à vous** : sur sa page, « Déplacer… » vers la famille ou l'équipe (voir [Votre compte KyberNow](/guide/compte-kybernow/)).
- **Un ordinateur qui reste à vous** : « Ouvrir à une famille ou une équipe » le partage sans changer de propriétaire.

## Le journal

L'onglet « Journal » dit qui a ouvert quel ordinateur, quand, avec quel appareil et depuis quelle adresse. Il note aussi chaque changement : membres, rôles, invitations, groupes, droits donnés et retirés, ordinateurs rattachés, détachés ou déployés, jetons, nouveaux appareils, ouvertures d'Admin à distance. Les adresses IP sont oubliées après 7 jours, les lignes après 12 mois. Seuls les Propriétaires et les Administrateurs le lisent.

## La connexion d'entreprise (SSO et OAuth)

Votre équipe peut se connecter avec l'annuaire de votre entreprise, par SSO ou OAuth : chacun entre avec son compte de travail habituel. Un Propriétaire ou un Administrateur la règle dans la console de l'équipe.

## Déployer des Stations (équipes)

Pour installer la Station sur des dizaines de postes, sans écran ni clic, l'équipe donne un jeton d'enrôlement : chaque Station installée avec lui se rattache d'elle-même à l'équipe.

### Créer le jeton

1. Onglet « Déployer », « Nouveau jeton ».
2. Donnez un « Nom », une « Durée (jours) » de 1 à 365 jours et un « Nombre de Stations » de 1 à 10 000.
3. Choisissez le « Groupe qui reçoit les droits » et les « Droits donnés au groupe » sur chaque Station enrôlée. Pour ranger d'office les postes dans un groupe d'ordinateurs, choisissez-le dans « Ranger dans le groupe ».
4. « Créer le jeton ». Il commence par `kne_` et n'est « Montré une seule fois : copiez-le maintenant. » Un clic le sélectionne.

### Installer sur Windows

Lancez l'installateur en administrateur, avec le jeton et l'adresse du relais (share.kybernow.app, ou le vôtre) :

```
KyberNow-Station-Setup.exe /quiet /enroll=<jeton> /relay=<relais>
```

La Station s'installe sans écran, avec les options par défaut, et garde les réglages d'une installation existante. Le code de sortie dit comment cela s'est passé :

| Code | Signification |
|---|---|
| 0 | Installée et rattachée à l'équipe |
| 3010 | Installée, l'ordinateur doit redémarrer |
| 1 | Installation échouée |
| 2 | Installée, mais pas enrôlée |

Le détail est écrit dans `install.log`. L'installation silencieuse crée le compte d'Admin `kybernow` avec un mot de passe tiré au hasard : pour reprendre la main sur un poste, choisissez « Réinitialiser le mot de passe d'Admin… » dans le menu de l'icône KyberNow Station, près de l'horloge.

### Installer sur Mac

Lancez, dans la session de l'utilisateur et sans `sudo`, le programme `kybernow-ui` de « Installer KyberNow Station », dans le fichier KyberNow-Station-Mac.dmg :

```
kybernow-ui --quiet --enroll=<jeton> --relay=<relais>
```

macOS demande une fois le mot de passe d'un administrateur du Mac. Les autorisations (écran, accessibilité, micro) se donnent ensuite depuis l'icône de la Station, ou par un profil de configuration de votre outil de gestion des Mac.

> Bon à savoir : un outil de déploiement peut aussi écrire le jeton dans les réglages de la Station, `[account] enroll = "kne_…"` dans `kyber-admin.toml`, avec le relais dans `[relay]`. Admin le présente dès que le relais répond, puis l'efface.

### Suivre et arrêter un jeton

La liste des jetons dit pour chacun combien de Stations il a servi, et s'il est « Échu », « Épuisé » ou « Révoqué ». « Révoquer le jeton » empêche tout nouvel enrôlement ; les Stations déjà enrôlées restent dans l'équipe.

## Ouvrir Admin depuis la page du compte

Si vous administrez un ordinateur, « Ouvrir Admin » est sur sa page, sur sa carte (le raccourci « Admin ») et sur sa ligne dans l'onglet « Ordinateurs » de l'équipe. Confirmez « Ouvrir Admin de » l'ordinateur avec « Confirmer avec une passkey » : Admin s'ouvre déjà connecté, sans son mot de passe.

Chez vous, sur le même réseau que l'ordinateur, cela marche tout de suite. La page de connexion d'Admin propose aussi « Se connecter avec KyberNow », sur le réseau seulement.

### Admin à distance

Pour ouvrir Admin de n'importe où, allumez d'abord « Admin à distance » :

1. Sur place, sur le réseau de l'ordinateur, ouvrez Admin, Appareils Link › « Hors de chez vous ».
2. Vérifiez que l'ordinateur est rattaché à un compte, avec « Accepter les comptes KyberNow », et que « Accès hors de chez vous » est allumé.
3. Allumez « Admin à distance ». Il est coupé au départ et ne s'allume que d'ici ; il se coupe de partout.
4. Ajoutez une passkey à votre compte si ce n'est pas fait : elle est demandée à chaque ouverture à distance.

À distance, tout Admin est là, avec les mêmes pages et les mêmes réglages. Seul « Admin à distance » ne s'y allume pas.

Chaque ouverture à distance se voit. Le Mac affiche une notification et l'icône de Windows ouvre son panneau, avec « Admin est ouvert à distance par » suivi du nom. Admin montre le bandeau « Admin ouvert à distance, par KyberNow Share. » Le fil d'Admin et le journal de l'équipe le notent, et les Propriétaires et les Administrateurs reçoivent un e-mail. La session se ferme après 30 minutes sans activité, et au bout de 8 heures dans tous les cas.

Sous l'interrupteur, Admin dit si la connexion est « Chiffré de bout en bout, jusqu'à cet ordinateur : le relais ne voit pas ce qui passe. » Avec un relais qui ne le permet pas, Admin l'écrit aussi : ce relais voit alors ce qui passe.

## Si ça ne marche pas

- **Impossible de créer une famille ou une équipe.** Activez d'abord la double authentification, ou ajoutez une passkey, dans « Compte et sécurité ».
- **« Vous ne gérez pas » la famille ou l'équipe choisie.** Il faut y être Propriétaire ou Administrateur pour y déplacer un ordinateur.
- **L'installation silencieuse rend le code 2.** Le jeton est échu, épuisé ou révoqué, ou le relais n'a pas de comptes. Regardez l'onglet « Déployer » et `install.log`.
- **« Admin à distance est coupé » sur cet ordinateur.** Le lien ne marche que chez vous : choisissez « Ouvrir sur le réseau », ou allumez « Admin à distance » sur place.
- **« Ouvrir Admin » est grisé, avec « Hors ligne ».** L'ordinateur est éteint, en veille ou sans internet.
- **« Ajoutez d'abord une passkey à votre compte »** s'affiche. Ajoutez-en une dans « Compte et sécurité », puis recommencez.
