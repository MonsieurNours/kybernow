---
slug: stylet-micro-webcam
titre: Stylet, micro, caméra et enregistrement
title: Stylet, micro, caméra et enregistrement · KyberNow
description: Dessinez au stylet, parlez dans un appel avec le micro de votre appareil, filmez-vous avec sa caméra et enregistrez la session sur l'ordinateur.
rubrique: Pendant la session
univers: commun
duree: 6 min
connexes: [partager, preter, clavier-et-raccourcis]
---

Le stylet, le micro et la caméra de votre appareil arrivent sur l'ordinateur comme s'ils y étaient branchés. Vous pouvez aussi enregistrer la session dans un fichier vidéo, sur l'ordinateur.

## Le stylet

Le stylet d'une tablette graphique branchée sur votre appareil, l'Apple Pencil d'un iPad ou le stylet d'un appareil Android arrive sur l'ordinateur comme un vrai stylet. Il transmet la pression, l'inclinaison, la gomme, le bouton du stylet et le survol (la pointe tout près de la surface, sans la toucher). Les logiciels de dessin en tirent l'épaisseur et l'opacité du trait ; les autres voient un clic gauche.

Il n'y a rien à régler. Il faut :

- une app KyberNow Link sur Mac, Windows, Linux, iPad ou Android ;
- une Station Windows, Mac ou Linux à jour.

Le stylet suit la souris du bureau. En souris des jeux, il reste une souris. Sur un PC, bouton du stylet tenu et pointe posée font un clic droit. Sur l'iPad, l'Apple Pencil écrit toujours là où il touche, quel que soit le mode du toucher. Sous Linux, l'ordinateur reçoit tout sauf la rotation du stylet.

Un invité du partage a le stylet quand vous lui donnez le clavier et la souris. Dans le navigateur, le stylet reste une souris.

## Le micro

Le micro de l'appareil arrive sur l'ordinateur, pour parler dans un jeu, un appel ou une visio. Il marche avec les apps Mac, iPhone et iPad, Android, Windows, Linux et Steam Deck, et sur un Raspberry Pi avec un micro USB. L'Apple TV et les téléviseurs LG n'envoient pas de micro.

1. En session, choisissez « Micro » dans la barre (ou la tuile « Micro » des menus en tuiles), ou le raccourci <kbd>⌃⌥⌘V</kbd> sur Mac, <kbd>Ctrl+Alt+Maj+V</kbd> sur Windows et Linux.
2. La première fois, le système de l'appareil demande l'accès au micro : acceptez.
3. L'avis « Micro envoyé à Bureau-Salon. » (le nom de votre ordinateur) confirme l'envoi. Tant que le micro part, la puce « Micro » de la barre reste allumée (sur iPhone, iPad, Android et au salon, une petite icône de micro).

Le micro est coupé à la première session avec un ordinateur. Ensuite, chaque session reprend votre dernier choix pour cet ordinateur. Micro coupé, rien n'est envoyé.

Sur l'ordinateur, pendant l'envoi, le micro virtuel devient le micro par défaut : « CABLE Output » sur un PC, « Micro KyberNow » sur un Mac et sous Linux. Une app d'appel qui suit le micro du système le prend toute seule ; sinon, choisissez-le dans ses réglages. Le micro d'avant revient à la fin.

> Bon à savoir : sur un PC, le micro virtuel est VB-CABLE, installé avec la Station. VB-CABLE, de VB-Audio Software, est offert en donationware : vb-cable.com.

Dans les Réglages de l'app, « Micro » choisit le micro de l'appareil (« Celui du système » par défaut), et « Réduire l'écho et le bruit », allumé, retire le son de l'ordinateur et le bruit autour de vous. Coupez-le pour un micro de studio.

Un ami à qui vous prêtez l'ordinateur parle comme vous. Les invités d'un partage parlent seulement si vous allumez « Laisser les invités parler » dans le panneau du partage.

## La caméra

La caméra de l'appareil part vers l'ordinateur, qui la voit comme une webcam ordinaire, « KyberNow Camera ». Teams, Zoom, Discord, OBS et les navigateurs la proposent dans leur liste.

Elle marche avec les apps Mac, iPhone et iPad, Android (téléphones et tablettes), Windows, Linux (bureau et Steam Deck), et sur l'Apple TV avec un iPhone ou un iPad posé à côté, par Caméra de continuité (tvOS 17 ou plus récent). Elle sert pendant vos sessions et pendant un prêt. Les invités d'un partage n'envoient pas leur caméra.

1. En session, ouvrez « Caméra » dans la barre (ou la tuile « Caméra », ou le raccourci <kbd>⌃⌥⌘C</kbd> / <kbd>Ctrl+Alt+Maj+C</kbd>).
2. Choisissez la caméra dans la liste : « Caméra avant » ou « Caméra arrière » sur un téléphone, sinon son nom. Sur l'Apple TV, « Choisir un iPhone ou un iPad… ».
3. Choisissez « Allumer la caméra ». Un point rouge et « Caméra » allumée dans la barre montrent qu'elle envoie ; barre fermée, une petite capsule « ● Caméra » reste en haut à gauche de l'image.
4. Dans l'app de visio de l'ordinateur, choisissez « KyberNow Camera ».

La caméra est éteinte au début de chaque session. Sur iPhone, iPad et Android, si l'app passe en arrière-plan, l'ordinateur garde la dernière image et l'envoi reprend à votre retour.

« Définition de la caméra », dans la rubrique Commandes des réglages de l'ordinateur, vaut 720p (par défaut) ou 1080p, toujours à 30 images par seconde. En 1080p, la caméra demande environ 4 Mbit/s vers l'ordinateur.

Un seul appareil à la fois envoie sa caméra à un ordinateur.

### La première fois sur l'ordinateur

- **Mac** : une notification demande d'autoriser l'extension caméra de KyberNow Station dans Réglages Système › Général › Ouverture et extensions.
- **Linux** : la caméra passe par PipeWire, que les navigateurs et OBS lisent. Pour la donner à toutes les apps, installez v4l2loopback, puis redémarrez : voir [Installer KyberNow Station sur Linux](/guide/installer-station-linux/).
- **Windows 10** : Zoom, Teams, Discord, OBS, Chrome et Edge la voient ; l'app Caméra de Windows non. Sous Windows 11, toutes les apps la voient.

## Enregistrer la session

L'ordinateur enregistre l'image et le son de votre session dans un fichier MP4, sans réencodage : la carte graphique ne travaille pas plus.

1. En session, choisissez « Enregistrer » dans la barre (ou <kbd>⌃⌥⌘R</kbd> / <kbd>Ctrl+Alt+Maj+R</kbd>, ou la tuile « Enregistrer » au salon).
2. La pastille rouge « Enregistrement · 02:14 » compte le temps. Un clic dessus arrête l'enregistrement.
3. À l'arrêt, l'avis « Enregistrement terminé : retrouvez-le dans Admin › Enregistrements. » s'affiche.

Tout le monde dans la session voit la pastille : un invité du partage lit « Hugo enregistre la session. », sans pouvoir l'arrêter. Fermer la session arrête l'enregistrement.

Dans Admin, espace Enregistrements, vous trouvez les fichiers (« Lire », « Télécharger », « Supprimer »), le dossier et la durée de conservation :

- le dossier par défaut est Vidéos › KyberNow sur un PC et sous Linux, Films › KyberNow sur un Mac ; « Modifier » en choisit un autre ;
- « Garder » vaut « 30 jours » par défaut, ou « 7 jours », « 90 jours », « Toujours » : les enregistrements plus anciens sont effacés ;
- « Permettre l'enregistrement des sessions » coupe la fonction pour tout l'ordinateur.

Seul l'écran principal est enregistré, sans le pointeur. PyroWave et AV1 ne s'enregistrent pas : passez en HEVC ou H.264. Pendant un prêt de l'ordinateur, on n'enregistre pas.

## Si ça ne marche pas

- **« Micro » est grisé.** Son infobulle donne la raison. « L'ordinateur n'a pas de micro virtuel : voyez Admin › Image et son. » : sur un PC, choisissez « Installer VB-CABLE » sur la carte « Micro virtuel ». « Mettez à jour la Station pour envoyer le micro. » : la Station est ancienne.
- **« KyberNow Link n'a pas accès au micro. »** Autorisez-le dans les réglages de confidentialité de l'appareil (Réglages Système sur Mac, paramètres de confidentialité de Windows), puis rallumez « Micro ».
- **L'app de visio n'entend rien.** Choisissez « CABLE Output » (PC) ou « Micro KyberNow » (Mac, Linux) comme micro dans ses réglages.
- **« Caméra » n'apparaît pas dans la barre.** L'ordinateur ne propose pas la caméra pour cette session, ou l'appareil n'en a pas. Mettez la Station à jour.
- **« iPhone de Léa envoie déjà sa caméra à Bureau-Salon. »** Éteignez la caméra sur l'autre appareil.
- **« La caméra est utilisée par une autre app. »** Fermez l'autre app qui filme sur votre appareil.
- **« Bureau-Salon n'a pas pu ouvrir sa caméra virtuelle. »** Sur un Mac, vérifiez l'autorisation de l'extension caméra ; ailleurs, redémarrez la Station.
- **Le stylet se comporte comme une souris.** La souris est en souris des jeux, ou la Station est ancienne. Sous Windows, une tablette Wacom dont « Windows Ink » est coupé reste une souris.
- **« Moins de 2 Go libres sur l'ordinateur : impossible d'enregistrer. »** Libérez de la place, ou choisissez un autre dossier dans Admin › Enregistrements.
- **« PyroWave et AV1 ne s'enregistrent pas : passez en HEVC ou H.264. »** Changez « Format vidéo » dans les réglages de l'ordinateur, ou le profil.
