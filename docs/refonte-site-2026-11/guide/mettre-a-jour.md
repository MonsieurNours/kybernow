---
slug: mettre-a-jour
titre: Mettre à jour KyberNow
title: Mettre à jour la Station et les apps · KyberNow
description: Mettez à jour la Station en un clic depuis Admin ou depuis vos apps, choisissez de recevoir les bêtas et gardez chaque app KyberNow Link à jour.
rubrique: Entretien
univers: commun
duree: 5 min
connexes: [desinstaller, installer-station-windows, installer-votre-relais, faq]
---

La Station et toutes les apps KyberNow Link portent le même numéro de version. Gardez-les à la même version : la plupart se mettent à jour seules, les autres vous préviennent quand une version sort.

## La Station

### Depuis Admin

Admin cherche une nouvelle version toutes les 6 heures.

1. Ouvrez Admin, espace Système, page « Mises à jour ». « Vérifier maintenant » cherche tout de suite.
2. Choisissez « Installer maintenant ». Admin télécharge la version, vérifie qu'elle est bien signée par KyberNow, puis l'installe en une minute environ. Une session en cours est coupée : Admin vous le demande avant.
3. La page se recharge seule et annonce que la nouvelle version « est en place ».

Si la nouvelle version ne démarre pas correctement, la Station revient d'elle-même à la précédente et la page le dit. « Revenir à la version précédente » le fait aussi à la demande.

L'icône de la Station le propose aussi : près de l'horloge sous Windows (« Mettre à jour la Station » suivi du numéro de version), dans la barre des menus sur Mac. C'est pareil sur une Station Linux, depuis Admin, y compris sur Bazzite, Fedora Silverblue ou SteamOS, sans mot de passe.

Certaines versions s'installent avec l'installateur : Admin affiche « Cette version s'installe avec l'installateur. » Téléchargez-le, lancez-le sur l'ordinateur et gardez vos réglages.

<!-- capture : Admin, Système › Mises à jour, avec « Installer maintenant » -->

### Depuis vos apps

Quand une version de la Station est disponible pour l'ordinateur choisi, l'accueil de KyberNow Link affiche un bandeau : la nouvelle version « est disponible pour cet ordinateur. Environ une minute. »

- « Mettre à jour » installe la version et suit la Station jusqu'à son retour.
- « Plus tard » cache le bandeau pour cette version. Il revient au bout de 7 jours, ou plus tôt si une version plus récente sort.

Après la mise à jour de l'app elle-même, un avis rassemble les Stations à mettre à jour, avec « Tout mettre à jour ». La même chose se trouve dans les réglages de l'ordinateur, rubrique Station, « Mettre à jour la Station ».

Ce bandeau s'affiche sur les appareils ajoutés à l'ordinateur, et pour un compte qui a le droit « Administrer ». Hors de chez vous, l'app ne peut pas mettre à jour la Station : faites-le une fois rentré, ou ouvrez [Admin à distance](/guide/familles-et-equipes/).

### Recevoir les bêtas

Dans Admin, Système › « Mises à jour », choisissez le canal « Bêta » dans « Canal des mises à jour ». Les bêtas arrivent avant les versions stables ; elles peuvent encore changer.

### En dépannage, sous Windows

Si Admin ne répond plus, la Station se met aussi à jour depuis PowerShell, lancé en administrateur :

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

Ce chemin vérifie que le paquet n'est pas abîmé, sans les contrôles de signature d'Admin. Gardez-le pour le dépannage.

## Les apps KyberNow Link

| Appareil | Comment elle se met à jour | Pour recevoir les bêtas |
|---|---|---|
| Mac | Toute seule : elle cherche chaque jour, télécharge et installe. KyberNow Link › « Rechercher des mises à jour… » cherche tout de suite. | « Recevoir les versions bêta », dans KyberNow Link › Réglages… |
| PC Windows | Elle cherche au lancement, puis toutes les 6 heures, et vous le propose : « Installer et relancer », jamais pendant une session. | Réglages de l'app, « Canal des mises à jour » : « Aussi les bêtas » |
| Apple TV, iPhone, iPad | Par l'App Store, comme vos autres apps. | |
| Android | « Version X » apparaît sur l'accueil ; un bouton la télécharge et l'installe. La première fois, Android demande d'autoriser KyberNow à installer des applications. | Réglages de l'app › Mises à jour |
| Téléviseur LG | « Version X disponible » s'affiche sur l'accueil. Installez le nouveau fichier KyberNow-Link-webOS.ipk depuis l'ordinateur, comme la première fois. | Réglages › Ce téléviseur › Mises à jour |
| Linux, Steam Deck, SteamOS | « Version X disponible » s'affiche sur l'accueil. Installez le nouveau paquet par-dessus : double clic, ou `flatpak install --user` suivi du fichier. | Réglages de l'app › « Versions proposées » |
| Raspberry Pi, l'app seule | « Version X disponible » s'affiche sur l'accueil. Installez le nouveau paquet : `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`. | Réglages de l'app › « Versions proposées » |
| Navigateur | Rien à faire : la page vient de la Station et suit sa version. | |

Vos ordinateurs et vos réglages restent à chaque mise à jour. Pour le téléviseur LG, voir aussi [Jouer sur un téléviseur LG](/guide/televiseur-lg/).

### KyberNow Link OS sur Raspberry Pi

Sur l'accueil, ☰ ouvre les Réglages de l'appareil, puis « Mise à jour ». La nouvelle version s'essaie dans l'autre moitié de la carte, et le Pi revient tout seul à la version d'avant si elle n'atteint pas l'accueil. Il faut un Pi 4 ou un Pi 5 ; sur un Pi 3, recopiez la nouvelle image sur la carte. Une carte copiée avec une version antérieure à la 0.11.8 se recopie une fois. Le détail est dans [Brancher un Raspberry Pi à la télé](/guide/raspberry-pi/).

## Votre relais

Un relais installé chez vous se met à jour seul chaque nuit : voir [Installer votre propre relais](/guide/installer-votre-relais/).

## Si ça ne marche pas

- **« La mise à jour n'a pas abouti. Ouvrez Admin pour en savoir plus. »** La page « Mises à jour » d'Admin dit pourquoi. La Station est revenue à la version qui marchait.
- **« La Station vient de redémarrer. Réessayez dans une minute. »** Attendez une minute, puis recommencez.
- **Un appareil est en session.** L'app demande « Mettre à jour quand même » : la session sera coupée.
- **Admin ne propose rien alors qu'une version est sortie.** Choisissez « Vérifier maintenant ». Une bêta n'est proposée que sur le canal « Bêta ».
- **Android refuse d'installer la mise à jour.** Une app compilée à part, avec une autre signature, doit être désinstallée une fois avant d'installer la version publiée.
