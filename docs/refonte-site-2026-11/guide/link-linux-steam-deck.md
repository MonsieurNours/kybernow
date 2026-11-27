---
slug: link-linux-steam-deck
titre: Utiliser votre ordinateur depuis Linux ou un Steam Deck
title: KyberNow Link sur Linux et Steam Deck · KyberNow
description: Installez KyberNow Link en Flatpak sur un PC Linux, un Steam Deck ou une Steam Machine, et jouez à votre ordinateur à la manette.
rubrique: Connecter un appareil
univers: commun
duree: 10 min
connexes: [connecter-un-appareil, clavier-et-raccourcis, profils-et-qualite, installer-station-linux]
---

La même app sert au PC sous Linux, au Steam Deck et au salon, sur une Steam Machine ou un PC sous SteamOS. Elle se livre en paquet Flatpak, qui s'installe pour vous seul, sans droits d'administrateur.

## Ce que fait l'app

- Sur un PC, elle s'utilise à la souris et au clavier, avec une barre de session en haut de l'écran.
- Sur le Deck, elle prend une interface portable, à la manette, avec la légende des boutons.
- Sur une télé, elle passe à l'interface du salon, lisible à trois mètres, avec sa colonne « Prêt à jouer » : manettes, son, écran et décodeurs.
- Elle décode l'image par la carte graphique quand elle le peut (VA-API), gère la fréquence variable des écrans qui l'ont, le son stéréo, 5.1 ou 7.1, et quatre manettes avec leurs vibrations.

## Sur un PC sous Linux

1. Téléchargez le paquet de votre machine : [KyberNow-Link-Linux-x86_64.flatpak](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Linux-x86_64.flatpak) pour la plupart des PC, [KyberNow-Link-Linux-aarch64.flatpak](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Linux-aarch64.flatpak) pour une machine ARM.
2. Double-cliquez sur le fichier : GNOME Logiciels ou KDE Discover l'installe. Vous pouvez aussi ouvrir un terminal dans le dossier du fichier et taper :

   ```sh
   flatpak install --user KyberNow-Link-Linux-x86_64.flatpak
   ```

   L'app a besoin du socle commun des apps Flatpak, le runtime org.freedesktop.Platform 25.08. S'il manque, Flatpak propose de le télécharger depuis Flathub : acceptez.
3. Ouvrez KyberNow Link depuis le menu des applications.

Flatpak est déjà installé sur la plupart des distributions (sinon, voir flatpak.org/setup). S'il ne connaît pas encore le dépôt Flathub, ajoutez-le et installez le runtime à la main :

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Platform//25.08
```

## Sur le Steam Deck

Vous installez l'app depuis le mode Bureau, puis vous la lancez depuis le mode Jeu, comme un jeu.

1. Passez en mode Bureau : bouton Steam › Alimentation › Passer au bureau.
2. Avec Firefox ou un autre navigateur, téléchargez [KyberNow-Link-Linux-x86_64.flatpak](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-Linux-x86_64.flatpak).
3. Dans le dossier des téléchargements, double-cliquez sur le fichier : Discover l'installe. Vous pouvez aussi ouvrir Konsole et taper :

   ```sh
   flatpak install --user ~/Téléchargements/KyberNow-Link-Linux-x86_64.flatpak
   ```

   (`~/Downloads/` si le Deck est en anglais.)
4. Ajoutez l'app à Steam : ouvrez Steam en mode Bureau, Jeux › Ajouter un jeu non-Steam, cochez KyberNow Link, puis Ajouter.
5. Revenez au mode Jeu, avec l'icône « Retour au mode Jeu » du bureau. KyberNow Link est dans la bibliothèque, onglet Non-Steam.
6. Dans la fiche de KyberNow Link, icône de manette › Disposition, choisissez le modèle « Manette avec pavé tactile en souris ». Les sticks et les boutons vont à la manette de l'ordinateur, le pavé droit déplace la souris, et ses clics cliquent.

## Au salon

Sur une Steam Machine, un PC sous SteamOS ou Bazzite branché à la télé, ou le Deck sur sa station d'accueil, l'installation est la même. L'app passe d'elle-même à l'interface du salon dès que l'écran dépasse 1280 × 800 en mode Jeu. Réglages de l'app › Interface permet d'imposer Bureau, Portable ou Salon.

## Ajouter votre ordinateur

1. Lancez KyberNow Link. Sur le même réseau, votre ordinateur apparaît tout seul : choisissez-le.
2. L'app affiche un code à six chiffres. Acceptez-le dans Admin, espace Appareils Link (sur un PC, aussi dans le panneau de l'icône près de l'horloge). Rien à taper.
3. Choisissez un profil, puis « Se connecter ».

Par un VPN, choisissez « Ajouter un ordinateur ou une console », puis « Par adresse » : l'adresse IP ou le nom de l'ordinateur, et le port d'Admin. Comparez ensuite les quatre premiers groupes de l'empreinte avec ceux qu'affiche Admin. Sur le Deck, tapez avec le pavé à l'écran ou le clavier de Steam.

## À la manette

Sur le Deck et au salon :

- A valide, B revient, Y ouvre les réglages de l'ordinateur, X ajoute un ordinateur.
- L1 et R1 changent de profil, Start ouvre les réglages de l'app.
- En session, L1 + R1 + Start + Select tenus ensemble, ou un toucher en haut de l'écran, ouvrent le menu de session. « Clavier » y ouvre le clavier de Steam.

Sur un PC, au clavier : Ctrl+Alt+Maj+D se déconnecte, Ctrl+Alt+Maj+F passe en plein écran, Ctrl+Alt+Maj+M passe de la souris du bureau à la souris des jeux, et Échap tenu une seconde rend la souris. Voir [Clavier, souris et raccourcis](/guide/clavier-et-raccourcis/).

## Réglages conseillés

- **Sur le Deck** : ceux par défaut, c'est-à-dire le profil Jeu, la définition « Comme cet écran » (1280 × 800 sur le Deck, à sa fréquence) et « Affichage des images » sur Auto. « Alléger sur batterie », activé, limite le Deck débranché à 60 images par seconde et 20 Mbit/s.
- **Sur un écran à fréquence variable** (FreeSync, VRR) : laissez « Affichage des images » sur Auto. Chaque image s'affiche dès qu'elle est décodée, et l'écran suit la cadence du flux. L'écran du Steam Deck, lui, ne la gère pas.
- **Pour un texte bien net au bureau** : activez Réglages de l'ordinateur › Image › Texte net (4:4:4). Il ne marche qu'en HEVC et demande plus de débit ; le processeur décode si la carte graphique ne sait pas le faire.
- **Le réseau** : un Wi-Fi 5 GHz ou 6 GHz près de la box, ou un câble Ethernet sur la station d'accueil.

## Mettre à jour l'app

Un Flatpak installé depuis un fichier ne se met pas à jour seul. L'app vous signale une nouvelle version (« Version X disponible ») : téléchargez le nouveau fichier et installez-le de la même façon. Vos ordinateurs et réglages restent.

## Si ça ne marche pas

- **Flatpak ne trouve pas le runtime.** Ajoutez le dépôt Flathub et installez org.freedesktop.Platform 25.08, avec les deux commandes plus haut.
- **Le pavé du Deck ne déplace pas la souris.** Vérifiez la disposition « Manette avec pavé tactile en souris » dans la fiche de KyberNow Link.
- **L'app reste en interface portable sur la télé.** Imposez Salon dans Réglages de l'app › Interface.
- **Pour signaler un problème.** Joignez le journal de l'app, dans `~/.var/app/fr.leslignesclaires.kybernow/config/kybernow/kybernow.log`.
