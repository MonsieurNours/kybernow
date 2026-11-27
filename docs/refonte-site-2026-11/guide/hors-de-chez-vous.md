---
slug: hors-de-chez-vous
titre: Retrouver votre ordinateur hors de chez vous
title: Retrouver votre ordinateur hors de chez vous · KyberNow
description: À l'hôtel, au bureau ou chez des amis, rejoignez votre ordinateur par le relais de KyberNow ou par un VPN, sans rien ouvrir sur votre box.
rubrique: Hors de chez vous
univers: commun
duree: 5 min
connexes: [compte-kybernow, installer-votre-relais, partager, profils-et-qualite]
---

Chez vous, l'image va directement de l'ordinateur à votre appareil, sans passer par un serveur. Ailleurs, vos appareils le retrouvent par le relais de KyberNow ou par un VPN, sans rien régler sur votre box.

## Avant de commencer

- L'ordinateur reste allumé, relié à internet, avec KyberNow Station.
- Pour jouer, une connexion qui envoie assez vite depuis la maison (le débit montant de votre box) et, en face, un réseau correct.

## Le relais de KyberNow

Un relais est un serveur sur internet qui fait passer la connexion entre votre appareil et votre ordinateur. Celui de KyberNow, share.kybernow.app, est ouvert à tous, sans plafond de débit ; il est tenu par Les Lignes Claires, en France. Vous pouvez aussi [installer le vôtre](/guide/installer-votre-relais/).

C'est la Station qui ouvre la connexion vers le relais, et seulement quand elle en a besoin : un partage ouvert, l'accès hors de chez vous allumé, une invitation en cours. Rien à ouvrir sur la box.

## Avec un compte KyberNow

C'est le plus simple. [Rattachez l'ordinateur à votre compte](/guide/compte-kybernow/) : le rattachement allume « Accès hors de chez vous ». Sur chaque appareil connecté au même compte, l'ordinateur apparaît sur l'accueil et s'ouvre de n'importe où, sans appairage.

## Avec un appareil déjà ajouté

1. Ajoutez d'abord l'appareil chez vous, sur le réseau de la maison (voir [Connecter un appareil à votre ordinateur](/guide/connecter-un-appareil/)).
2. Dans Admin, espace Appareils Link, section « Hors de chez vous », allumez « Accès hors de chez vous ». Il est coupé au départ. « Relais joint » s'affiche quand la Station a trouvé le relais.
3. Ailleurs, ouvrez KyberNow Link comme d'habitude. L'ordinateur est marqué « Hors de chez vous » à la place de « En ligne », et la barre de session affiche « Par le relais ».

<!-- capture : Admin, section « Hors de chez vous », avec « Relais joint » et l'interrupteur allumé -->

## Ajouter un appareil quand vous êtes loin

Une invitation ajoute un appareil de loin, sans que personne ait à l'accepter devant l'ordinateur. Préparez-la avant de partir, ou depuis [Admin à distance](/guide/familles-et-equipes/).

1. Dans Admin, espace Appareils Link, section « Invitations », choisissez « Inviter un appareil… ».
2. Donnez-lui un nom et une durée : 1, 7 ou 30 jours. « Créer l'invitation ».
3. Admin affiche un code à six chiffres et un mot de passe. Notez le mot de passe : il n'est montré qu'une fois.
4. Sur l'appareil, dans KyberNow Link, « Rejoindre avec un code », puis le mot de passe. L'ordinateur s'ajoute à l'accueil.

Créer une invitation allume « Accès hors de chez vous ». Deux invitations au plus peuvent servir en même temps, cinq appareils chacune. À la fin de la durée, les appareils qu'elle a ajoutés perdent leur accès.

## Ce qui change par le relais

- **Un peu plus de latence.** Le relais ajoute un aller-retour, de 5 à 20 ms en France selon la distance.
- **Le bon profil.** Le profil Max est prévu pour le réseau de la maison : hors de chez vous, prenez Jeu, Bureau ou Auto. Avec « Débit adaptatif », allumé au départ, l'app baisse le débit quand le réseau sature, puis le remonte. Un débit réglé à la main doit rester sous le débit montant de votre box.
- **Le forfait mobile.** À 20 Mbit/s, une heure de session consomme environ 9 Go.
- **Les réseaux qui filtrent.** Si le réseau où vous êtes bloque l'UDP (certains hôtels, des réseaux d'entreprise), la connexion passe en TCP et l'app affiche « Par le relais (TCP) » : bon pour regarder, moins bon pour jouer.
- **Pas de redémarrage ni de mise à jour depuis l'app.** Hors de chez vous, l'app ne peut ni redémarrer ni mettre à jour la Station. Faites-le une fois rentré, ou par Admin à distance.
- **Une protection contre les essais.** Après trop d'essais ratés depuis une même adresse, la Station la bloque 10 minutes.
- **Dans un navigateur**, le relais sert à rejoindre un partage et à ouvrir Admin à distance. Pour retrouver votre ordinateur en entier, utilisez une app KyberNow Link.

## Ce que voit le relais

Dans les apps KyberNow Link, l'image, le son et vos frappes restent chiffrés de bout en bout : ils traversent le relais sans qu'il puisse les lire, et l'app vérifie qu'elle parle bien à votre ordinateur grâce à l'empreinte de son certificat, une suite de caractères qui l'identifie.

Dans un navigateur, avec une Station et un relais à jour, la connexion est aussi chiffrée jusqu'à votre ordinateur : l'ordinateur a son propre certificat, et le relais fait passer la connexion sans pouvoir l'ouvrir. La Station surveille les registres publics des certificats et suspend Admin à distance si un certificat qu'elle n'a pas demandé apparaît pour son adresse. Avec une Station plus ancienne, le relais déchiffre ce qui passe dans le navigateur pour le faire suivre : utilisez alors une app, ou mettez la Station à jour.

Le relais voit les adresses IP, les débits, les heures et les codes de partage. Celui de KyberNow garde les adresses IP dans ses journaux quelques jours, jamais les codes ; sa page de confidentialité, share.kybernow.app/privacy, le détaille. Avec un compte, il garde aussi votre compte, les droits donnés, le journal des accès (les adresses IP 7 jours) et vos réglages synchronisés.

## Par un VPN

Un VPN est un réseau privé et chiffré entre vos appareils, comme Tailscale ou WireGuard. Il marche sans relais : l'image va directement de votre ordinateur à votre appareil.

1. Installez le même VPN sur l'ordinateur et sur l'appareil, connectés au même compte ou au même réseau.
2. Par VPN, l'ordinateur n'apparaît pas tout seul dans l'app. Choisissez « Ajouter un ordinateur ou une console », puis « Par adresse » : son adresse dans le VPN (par exemple 100.x.y.z avec Tailscale) et le port d'Admin. Vous pouvez aussi scanner le QR code d'Admin, espace Appareils Link, « Ajouter un appareil ».
3. Comparez les quatre premiers groupes de l'empreinte avec ceux qu'affiche Admin, puis donnez l'accès comme chez vous.

Sur l'Apple TV, l'iPhone et l'iPad, le code à six chiffres ne passe pas par le VPN : entrez l'identifiant et le mot de passe du compte de la Station.

> Bon à savoir : n'ouvrez pas les ports de KyberNow sur internet depuis votre box (une « redirection de ports »). Ni le relais ni un VPN n'en ont besoin, et votre ordinateur reste invisible pour le reste d'internet.

## Si ça ne marche pas

- **« N'est pas joignable hors de chez vous (accès coupé dans Admin) ».** Allumez « Accès hors de chez vous » dans Admin, section « Hors de chez vous ».
- **« N'est pas joignable : l'ordinateur est éteint ou en veille ».** Allumez l'ordinateur, ou empêchez sa mise en veille quand vous partez.
- **Admin affiche « Relais injoignable ».** Vérifiez l'adresse du champ « Relais » et la connexion internet de l'ordinateur.
- **L'app affiche « Par le relais (TCP) ».** Le réseau où vous êtes bloque l'UDP. Essayez un autre réseau, le partage de connexion de votre téléphone par exemple.
- **L'image saccade.** Passez au profil Jeu ou Bureau, ou baissez le débit dans les réglages de l'ordinateur, dans l'app. À la maison, un câble Ethernet entre l'ordinateur et la box aide aussi.
