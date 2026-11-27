---
slug: installer-station-mac
titre: Installer KyberNow Station sur Mac
title: Installer KyberNow Station sur Mac · KyberNow
description: Installez KyberNow Station sur un Mac à puce Apple, donnez les trois autorisations de macOS et retrouvez ce Mac sur vos autres écrans.
rubrique: Installer la Station
univers: commun
duree: 5 min
connexes: [ce-qu-il-vous-faut, connecter-un-appareil, clavier-et-raccourcis, compte-kybernow]
---

Le Mac devient une Station comme un PC : même Admin, mêmes apps pour s'y connecter, avec un écran virtuel à la taille de chaque appareil et le son jusqu'au 7.1. L'installation prend environ deux minutes, et rien ne change sur le Mac avant que vous cliquiez sur « Installer ».

## Avant de commencer

- Un Mac à puce Apple (M1 ou plus récent), sous macOS 15 Sequoia ou plus récent.
- Le mot de passe de votre session macOS, demandé une fois.
- Un câble Ethernet vers la box, si possible : l'installateur le préfère au Wi-Fi pour jouer.

## Ouvrir l'installateur

1. Sur le Mac à partager, téléchargez [KyberNow-Station-Mac.dmg](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Mac.dmg) et ouvrez-le.
2. Lancez « Installer KyberNow Station ». L'app est signée et notarisée par Apple : macOS l'ouvre sans avertissement.

## Bienvenue

L'installateur analyse le Mac : puce Apple, version de macOS, réseau local, ports réseau, écrans et son sur vos appareils. Une ligne en rouge empêche l'installation (un Mac à processeur Intel, ou une version de macOS trop ancienne).

Il choisit deux ports réseau, l'un pour le flux (l'image, le son et les commandes), l'autre pour KyberNow Admin. « Modifier » permet d'en choisir d'autres, puis « Valider ». Cliquez sur « Continuer ».

Si la Station est déjà sur ce Mac, un écran vous demande quoi faire de ses réglages. « Garder mes réglages » garde les comptes, les appareils ajoutés, le certificat et les ports : vos appareils se reconnectent sans rien refaire. « Repartir de zéro » crée un nouveau compte, de nouveaux ports et un nouveau certificat : il faudra ajouter de nouveau chaque appareil.

## Votre compte

1. Choisissez un identifiant.
2. Choisissez un mot de passe d'au moins 10 caractères.
3. Cliquez sur « Continuer ».

Ce compte protège le Mac. Il vous sert à ouvrir Admin et à vous connecter depuis vos appareils, qui peuvent le mémoriser. Le Mac n'en garde qu'une empreinte, jamais le mot de passe lui-même. D'autres comptes se créent ensuite dans Admin, espace Utilisateurs.

## Options

Tout est déjà réglé, et tout se change plus tard dans Admin.

- **Écran virtuel à la taille de chaque appareil** : l'image arrive à la définition et à la fréquence de l'appareil, en HDR s'il le demande.
- **Son sur vos appareils** : le son sort de votre appareil, jusqu'au 7.1, et le Mac reste muet. Une sortie son « KyberNow » est ajoutée au Mac.
- **Démarrer avec le Mac** : la Station est disponible dès l'ouverture de votre session.
- **Garder le Mac joignable** : pas de veille pendant une session, et réveil par le réseau quand un appareil le demande.
- **Manettes** : la manette de votre appareil est vue par le Mac comme une vraie manette.

Cliquez sur « Installer ». macOS demande alors le mot de passe de votre Mac, une seule fois : pour copier l'app dans Applications, ajouter la sortie son, ouvrir le pare-feu s'il est allumé et permettre le réveil par le réseau.

## Installation

Vous n'avez rien à faire : chaque étape s'affiche au fur et à mesure (moteur Kyber, certificat du Mac, sortie son KyberNow, démarrage avec le Mac, KyberNow Admin, accès depuis vos appareils).

Pendant ce temps, l'installateur vous propose de rattacher le Mac à votre compte KyberNow, comme sur un PC. C'est facultatif : « Se connecter » ou « Créer un compte », puis la page du relais, où vous confirmez le rattachement. La passkey peut passer par Touch ID. « Plus tard » passe cet écran ; vous pourrez le faire dans Admin › Compte KyberNow. Voir [Votre compte KyberNow](/guide/compte-kybernow/).

## Autorisations

macOS vous demande trois accords pour « KyberNow Station ». Pour chacun, cliquez sur « Ouvrir le réglage » ou « Autoriser », puis activez « KyberNow Station » dans le réglage de macOS. La ligne se coche toute seule.

1. **Enregistrement de l'écran et de l'audio du système**, pour envoyer l'image du Mac à vos appareils.
2. **Accessibilité**, pour que le clavier et la souris de vos appareils pilotent le Mac.
3. **Micro**, pour lire le son de la session sur la sortie KyberNow, que macOS range avec les micros. Aucun vrai micro n'est écouté.

« Plus tard » passe cet écran, mais la Station marche alors à moitié : pas d'image, pas de commandes ou pas de son. Vous pourrez donner ces accords depuis l'icône de la barre des menus, menu Autorisations.

<!-- capture : écran Autorisations, trois lignes cochées -->

## Ce Mac est prêt

L'écran de fin donne l'adresse du Mac, à entrer dans un appareil qui ne le trouve pas tout seul, un QR code pour « Ouvrir Admin sur votre téléphone », et l'adresse à ouvrir dans un navigateur pour utiliser le Mac sans app. « Ouvrir Admin » ouvre les réglages dans votre navigateur. La première fois, celui-ci affiche un avertissement de sécurité, parce que la Station signe elle-même sa connexion : acceptez-le.

L'icône KyberNow Station est dans la barre des menus, en haut à droite, avec l'état de la Station et l'accès à Admin. Admin a les mêmes espaces que sur un PC.

Il ne reste plus qu'à [connecter un appareil](/guide/connecter-un-appareil/).

## Pendant une session

- Le Mac crée un écran virtuel à la taille de l'appareil et éteint ses propres écrans, comme un PC. Ils se rallument à la fin, dans la même disposition.
- Le son part vers l'appareil par la sortie KyberNow, en stéréo, 5.1 ou 7.1, et la sortie d'avant revient à la fin.
- Un Mac verrouillé se diffuse aussi : vous tapez son mot de passe depuis votre appareil.
- La Station tourne dans votre session. Après un redémarrage du Mac, ouvrez la session une fois devant lui.
- Depuis un PC ou une télé, la touche ⊞ vaut ⌘. Le menu « Touches » de la barre de session propose les combinaisons du Mac, et le réglage « Ctrl agit comme ⌘ » vous laisse copier et coller avec Ctrl. Voir [Clavier, souris et raccourcis](/guide/clavier-et-raccourcis/).

> Bon à savoir : macOS 15 rappelle de temps en temps qu'une app enregistre l'écran. Confirmez : c'est la Station.

## Si ça ne marche pas

- **« Installation annulée ».** MacOS a besoin du mot de passe de votre Mac pour mettre l'app dans Applications et ajouter la sortie son. Relancez l'installateur et tapez-le.
- **Pas d'image sur l'appareil.** L'accord Enregistrement de l'écran manque. Donnez-le depuis l'icône de la barre des menus, menu Autorisations.
- **Le clavier et la souris ne font rien.** L'accord Accessibilité manque.
- **Pas de son.** L'accord Micro manque. Si l'installateur dit que la sortie son est posée mais que macOS ne la montre pas encore, elle apparaîtra au prochain démarrage du Mac.
- **Rien ne répond après un redémarrage.** Ouvrez votre session sur le Mac. La Station démarre avec elle.
