---
slug: link-windows
titre: Utiliser votre ordinateur depuis un PC Windows
title: KyberNow Link sur Windows · KyberNow
description: Installez KyberNow Link sur un PC Windows x64 ou ARM64, sans droits d'administrateur, puis ajoutez votre ordinateur par un code ou un lien.
rubrique: Connecter un appareil
univers: commun
duree: 2 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, plusieurs-ecrans-et-ordinateurs, compte-kybernow]
---

L'app Windows sert surtout sur un PC portable ou un ordinateur plus modeste, pour retrouver votre ordinateur principal. L'image est décodée par la carte graphique quand elle le peut, et nous avons mesuré environ 10 ms de latence sur un PC x64.

## Avant de commencer

- Windows 10 22H2 ou Windows 11, sur un PC x64 ou ARM64. Les PC ARM64, comme ceux à puce Snapdragon, demandent Windows 11.
- La Station installée sur l'ordinateur à retrouver.

## Installer l'app

1. Téléchargez [KyberNow-Link-Setup.exe](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Setup.exe) et ouvrez-le. Le même fichier sert aux PC x64 et ARM64 : il installe de lui-même la bonne version.
2. Si Windows SmartScreen bloque l'ouverture, cliquez sur « Informations complémentaires », puis sur « Exécuter quand même ». L'installateur et l'app ne portent pas encore de signature Authenticode.
3. **Bienvenue** : l'installateur vérifie Windows, le processeur, la carte graphique, l'écran, le réseau local et le moteur d'affichage de Microsoft (WebView2), qu'il télécharge s'il manque. Cochez « Raccourci sur le Bureau » si vous le voulez, puis cliquez sur « Installer ». L'app s'installe pour votre compte Windows seulement, sans droits d'administrateur.
4. **Compte KyberNow**, facultatif : « Se connecter » ou « Créer un compte » ouvre la page du relais dans la fenêtre, avec la passkey par Windows Hello. L'app s'ouvrira connectée, avec les ordinateurs de votre compte. « Plus tard » passe cet écran.
5. **C'est prêt** : cliquez sur « Ouvrir KyberNow Link ».

## Ajouter votre ordinateur

Sur l'accueil, choisissez « Ajouter un ordinateur ou une console », puis l'une des trois façons :

- **Sur le réseau** : votre ordinateur apparaît tout seul. Cliquez sur « Choisir ».
- **Avec le lien d'Admin** : dans Admin, espace Appareils Link, « Ajouter un appareil » affiche un lien `kybernow://`. Cliquez sur ce lien depuis ce PC, ou collez-le dans l'app : elle s'ouvre sur l'ajout, avec l'ordinateur et son empreinte déjà vérifiés.
- **Par adresse** : l'adresse IP ou le nom de l'ordinateur, et le port d'Admin. C'est le choix à faire par un VPN, comme Tailscale. Comparez ensuite les quatre premiers groupes de l'empreinte avec ceux qu'affiche Admin, puis cliquez sur « C'est la même ».

## Donner accès à ce PC

- **Avec un code** : l'app affiche un code à six chiffres. Acceptez-le dans Admin, espace Appareils Link (sur un PC, aussi dans le panneau de l'icône près de l'horloge). Rien à taper : ce PC reçoit son propre accès, que vous pourrez retirer depuis Admin.
- **Avec un compte** : l'identifiant et le mot de passe choisis à l'installation de la Station, ou un compte d'appareil. Windows les range dans son gestionnaire d'identifiants. Cliquez sur « Ajouter ».

Choisissez ensuite un profil (Jeu, Bureau, Max ou Auto), puis « Se connecter » : la session s'ouvre.

## Pendant la session

- Amenez la souris en haut de l'écran pour afficher la barre de session.
- Les raccourcis de l'app sont Ctrl+Alt+Maj suivi d'une lettre : D pour se déconnecter, F pour passer du plein écran à la fenêtre, M pour passer de la souris du bureau à la souris des jeux.
- Le menu « Touches » de la barre envoie « Ctrl + Alt + Suppr », « Touche ⊞ seule », « Alt + Tab » et « Impr. écran ».

Le détail est dans [Clavier, souris et raccourcis](/guide/clavier-et-raccourcis/).

## Près de l'horloge et dans la barre des tâches

Fermer la fenêtre de l'accueil ne quitte pas l'app : elle reste dans la zone de notification, près de l'horloge, parfois sous la flèche ^. Son menu liste vos ordinateurs avec leur état. Un clic droit sur l'icône de KyberNow Link, dans la barre des tâches, affiche aussi vos ordinateurs : un clic lance la session.

## Si ça ne marche pas

- **« Windows trop ancien ».** Faites les mises à jour de Windows jusqu'à Windows 10 22H2 au moins.
- **« Pilote de base de Windows ».** Le processeur décodera l'image, plus lentement. Installez le pilote de la carte graphique.
- **« Admin a refusé la demande » ou « La demande a expiré ».** Cliquez sur « Nouveau code », puis acceptez-le dans Admin.
- **Le lien `kybernow://` n'ouvre pas l'app.** Ouvrez KyberNow Link une fois pour terminer son installation, ou collez le lien dans l'app.
- **L'empreinte est différente.** N'allez pas plus loin. Vérifiez l'adresse, ou ajoutez l'ordinateur sur le réseau ou par le lien.
