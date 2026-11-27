---
slug: navigateur
titre: Utiliser votre ordinateur dans un navigateur
title: KyberNow dans un navigateur · KyberNow
description: Ouvrez votre ordinateur dans Chrome, Edge, Safari ou Firefox, sans rien installer, depuis une tablette ou un ordinateur d'emprunt.
rubrique: Connecter un appareil
univers: commun
duree: 2 min
connexes: [connecter-un-appareil, hors-de-chez-vous, partager, profils-et-qualite]
---

Sans rien installer, vous retrouvez votre ordinateur dans un navigateur, sur n'importe quel appareil. C'est pratique pour travailler depuis une tablette ou un ordinateur d'emprunt.

## Avant de commencer

- Un navigateur récent : Chrome ou Edge de préférence, Safari et Firefox aussi.
- Un appareil sur le même réseau que l'ordinateur, ou relié à lui par un VPN.
- Le compte créé à l'installation de la Station, ou un autre compte d'Admin › Utilisateurs.

## Ouvrir la session

1. Dans le navigateur, ouvrez l'adresse d'Admin suivie de `/link`, par exemple `https://192.168.1.20:31415/link`. L'adresse exacte s'affiche à la fin de l'installation de la Station, et dans Admin, Vue d'ensemble › « Ouvrir dans le navigateur ».
2. Le navigateur affiche un avertissement de sécurité. C'est normal : la Station chiffre la connexion avec un certificat qu'elle a créé elle-même. Acceptez-le pour continuer.
3. Connectez-vous avec l'identifiant et le mot de passe de la Station.
4. La session s'ouvre dans la page. Amenez le pointeur en haut de l'image pour afficher la barre de session.

<!-- capture : page /link dans Chrome, barre de session ouverte -->

## Quel navigateur choisir

- **Chrome et Edge** reçoivent l'image directement, avec la latence la plus basse.
- **Safari, sur Mac comme sur iPhone, et Firefox** reçoivent l'image par Admin, avec un peu plus de latence.

Si le navigateur n'arrive pas à démarrer l'image en HEVC, la page repasse d'elle-même en H.264 et vous le signale par « Image en H.264 : ce navigateur n'a pas pu démarrer le HEVC. »

## Le navigateur et les apps

Le navigateur dépanne bien, pour le bureau surtout. Pour jouer, préférez une app KyberNow Link : le navigateur n'a que le son stéréo, et il ne redemande pas l'image quand une partie se perd en route.

> Bon à savoir : hors de chez vous, le navigateur passe par le relais de KyberNow pour rejoindre un partage ou ouvrir Admin à distance. Pour retrouver votre ordinateur en entier, utilisez une app KyberNow Link, ou un VPN. Voir [Retrouver votre ordinateur hors de chez vous](/guide/hors-de-chez-vous/).

Un ami que vous invitez à [partager votre écran](/guide/partager/) peut aussi rejoindre la session depuis son navigateur, avec le lien du partage.

## Si ça ne marche pas

- **La page ne s'ouvre pas.** Vérifiez l'adresse et le port, et que l'appareil est sur le même réseau que l'ordinateur. L'adresse commence par `https://`.
- **« Image sans le son : ce navigateur n'a pas pu démarrer l'audio. »** Le navigateur a refusé de lancer le son. Rechargez la page, ou essayez Chrome ou Edge.
- **La page dit que Kyber n'a pas pu démarrer l'image.** Réessayez dans un instant. Si le message revient, recopiez la ligne « Détails » pour la signaler.
- **L'image saccade en Wi-Fi.** Rapprochez-vous de la box, ou passez à une app KyberNow Link, qui répare l'image quand une partie se perd.
