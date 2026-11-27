---
slug: installer-station-windows
titre: Installer KyberNow Station sur Windows
title: Installer KyberNow Station sur Windows · KyberNow
description: Installez KyberNow Station sur votre PC Windows en trois minutes, écran par écran, puis ouvrez KyberNow Admin pour la régler.
rubrique: Installer la Station
univers: commun
duree: 5 min
connexes: [ce-qu-il-vous-faut, connecter-un-appareil, compte-kybernow, mettre-a-jour]
---

L'installateur met en place la Station sur le PC à partager : le moteur Kyber, KyberNow Admin, l'écran virtuel, les manettes et le son. Comptez environ trois minutes, et rien ne change sur le PC avant que vous cliquiez sur « Installer ».

## Avant de commencer

- Un PC sous Windows 10 ou 11, et un compte administrateur.
- Le pilote de votre carte graphique (NVIDIA, AMD ou Intel), à jour.
- Steam installé, pour que le son parte vers vos appareils.
- Le détail est dans [Ce qu'il vous faut pour commencer](/guide/ce-qu-il-vous-faut/).

## Ouvrir l'installateur

1. Sur le PC à partager, téléchargez [KyberNow-Station-Setup.exe](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Station-Setup.exe) et ouvrez-le.
2. Si Windows SmartScreen bloque l'ouverture, cliquez sur « Informations complémentaires », puis sur « Exécuter quand même ».
3. Windows demande l'autorisation d'administrateur, parce que l'installateur ajoute des pilotes et des services. La fenêtre indique « Éditeur inconnu » : les fichiers de KyberNow pour Windows ne portent pas encore de signature Authenticode, celle qui nomme l'éditeur d'un programme. Acceptez.

## Bienvenue

L'installateur examine le PC, ligne par ligne : carte graphique, pilote graphique, réseau local, ports réseau, écran principal, mode d'alimentation et son sur vos appareils. Un point orange signale un conseil, sans bloquer l'installation.

Il choisit au hasard deux ports réseau libres, entre 20000 et 44999, en évitant ceux de Windows, de Steam et de Sunshine. Un port est un numéro d'entrée sur le PC : vos appareils passent par l'un pour le flux (l'image, le son et les commandes), par l'autre pour KyberNow Admin. « Modifier » permet d'en choisir d'autres, puis « Valider ».

Si le réseau du PC est classé « Public » dans Windows, l'installateur le passera en « Privé » : sans cela, vos appareils ne verraient pas le PC.

Cliquez sur « Continuer ».

<!-- capture : écran Bienvenue avec la liste des vérifications -->

## Réinstallation

Cet écran n'apparaît que si la Station est déjà sur le PC.

- « Garder mes réglages » ne remplace que les programmes et les pilotes. Comptes, appareils ajoutés, certificat, réglages d'Admin et ports restent tels quels : vos appareils se reconnectent sans rien refaire.
- « Repartir de zéro » crée un nouveau compte, de nouveaux ports et un nouveau certificat. Tous vos appareils perdent l'accès : il faudra les ajouter de nouveau.

## Votre compte

1. Choisissez un identifiant.
2. Choisissez un mot de passe d'au moins 10 caractères. Une jauge vous dit sa solidité.
3. Cliquez sur « Continuer ».

Ce compte reste sur le PC. Il vous sert à ouvrir Admin et à connecter vos appareils, qui peuvent le mémoriser. Le PC n'en garde qu'une empreinte, jamais le mot de passe lui-même. D'autres comptes se créent ensuite dans Admin, espace Utilisateurs.

## Options

Tout est déjà coché pour vous, et tout se change plus tard dans Admin.

- **Écran virtuel à la taille de chaque appareil** : le PC crée un écran à la définition et à la fréquence de l'appareil qui se connecte. Le pilote libre qui crée cet écran (SudoVDA) n'est pas signé par Microsoft : son certificat est ajouté aux certificats de confiance de Windows, et retiré si vous désinstallez la Station.
- **Manettes** : la manette de votre appareil est vue par le PC comme une vraie manette (pilote ViGEmBus), une manette Xbox 360 par défaut. Un redémarrage sera peut-être demandé.
- **Son sur vos appareils** : le son sort de votre appareil, jusqu'au 7.1, et le PC reste muet. Les haut-parleurs virtuels de Steam s'en chargent.
- **Démarrer avec l'ordinateur** : la Station est prête dès le démarrage, avant même que vous ouvriez votre session Windows.
- **Performances maximales** : le processeur et la carte graphique gardent toute leur vitesse pendant une session, et le délai de l'image reste stable.

Cliquez sur « Installer ».

## Installation

Vous n'avez rien à faire : chaque étape s'affiche au fur et à mesure. L'installateur pose le moteur Kyber, le certificat du PC, le service de Windows, l'écran virtuel, les manettes, le son, le micro de vos appareils (VB-CABLE) et Admin.

Il ouvre aussi le pare-feu de Windows, pour votre réseau privé seulement : les deux ports, et le port 5353, qui permet à vos appareils de trouver le PC tout seuls.

### Pendant ce temps : votre compte KyberNow

L'installation continue en fond pendant que l'installateur vous propose de rattacher le PC à un compte KyberNow. C'est facultatif : le compte fait apparaître le PC sur tous vos appareils, de chez vous ou d'ailleurs, et permet de l'ouvrir à d'autres personnes.

1. Choisissez « Se connecter » si vous avez déjà un compte, ou « Créer un compte » (une adresse e-mail et un mot de passe, ou une passkey).
2. La page du relais s'ouvre dans la fenêtre de l'installateur. Connectez-vous, puis confirmez le rattachement. Si votre compte gère une famille ou une équipe, choisissez à qui le PC appartient.
3. L'installateur affiche « Rattaché à votre compte », avec « Comptes KyberNow acceptés » et « Accès hors de chez vous activé ».

Le relais proposé est `share.kybernow.app`. « Autre relais… » sert si vous avez [votre propre relais](/guide/installer-votre-relais/). « Plus tard » passe cet écran : vous pourrez rattacher le PC dans Admin › Compte KyberNow. Voir [Votre compte KyberNow](/guide/compte-kybernow/).

## Cet ordinateur est prêt

L'écran de fin affiche :

- l'adresse du PC, à entrer dans un appareil qui ne le trouve pas tout seul ;
- un QR code, « Ouvrir Admin sur votre téléphone » ;
- l'adresse à ouvrir dans un navigateur pour utiliser le PC sans app ;
- « Redémarrer maintenant », si les manettes l'exigent : elles ne fonctionneront qu'après le redémarrage, tout le reste fonctionne déjà.

« Ouvrir Admin » ouvre les réglages dans votre navigateur. La première fois, le navigateur affiche un avertissement de sécurité. C'est normal : la Station chiffre la connexion avec un certificat qu'elle a créé elle-même, et que le navigateur ne connaît pas. Acceptez-le pour continuer. Les apps KyberNow Link, elles, reconnaissent ce certificat à son empreinte.

## L'icône et Admin

Une icône KyberNow Station apparaît près de l'horloge de Windows. Sa pastille est verte quand la Station est en marche, violette pendant une session. Un clic ouvre un panneau avec l'état de la Station et l'accès à Admin ; un clic droit, un menu avec « Ouvrir Admin », « Redémarrer la Station », « Partager un écran… » et « Prêter l'ordinateur… ».

Admin se range en espaces : Vue d'ensemble, Image et son, Enregistrements, Appareils Link, Prêt, Utilisateurs, Système et Compte KyberNow.

Il ne reste plus qu'à [connecter un appareil](/guide/connecter-un-appareil/).

> Bon à savoir : pour installer la Station sur les PC d'une équipe sans passer par ces écrans, l'installateur se lance aussi en silencieux, avec un jeton d'enrôlement. Voir [Familles et équipes](/guide/familles-et-equipes/).

## Si ça ne marche pas

- **« Aucune carte graphique compatible ».** Installez le pilote du fabricant (NVIDIA, AMD ou Intel), puis relancez l'installateur. Sans lui, le processeur compresse l'image, plus lentement.
- **« Steam n'est pas installé ».** Le son sortira aussi du PC. Installez Steam, puis relancez l'installation pour que le son parte seulement vers vos appareils.
- **Un message parle de Microsoft Edge WebView2.** Ce composant de Microsoft manque sur le PC, et l'installateur en a besoin pour s'afficher. Installez-le depuis la page de Microsoft qui s'ouvre, puis relancez l'installateur.
- **« Kyber est déjà installé ».** L'installateur le retire et le remplace par la Station, qui contient le même moteur.
- **Les manettes ne répondent pas.** Redémarrez le PC une fois.
- **Vos appareils ne voient pas le PC.** Vérifiez qu'ils sont sur le même réseau. Par un VPN, ajoutez le PC par son adresse : voir [Connecter un appareil à votre ordinateur](/guide/connecter-un-appareil/).
- **Le mode d'alimentation n'a pas changé.** Windows l'a refusé, et tout le reste est installé. Choisissez « Performances élevées » dans les réglages d'alimentation de Windows.
