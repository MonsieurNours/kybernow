---
slug: profils-et-qualite
titre: Profils, format vidéo, débit et HDR
title: Profils, format vidéo, débit et HDR · KyberNow
description: Choisissez le bon profil, le format vidéo, PyroWave, le débit et le HDR pour que l'image suive votre réseau et votre écran.
rubrique: Pendant la session
univers: commun
duree: 7 min
connexes: [clavier-et-raccourcis, plusieurs-ecrans-et-ordinateurs, hors-de-chez-vous]
---

Un profil règle d'un coup la définition, le format, le débit et la façon de réparer les pertes. Choisissez-le sur l'accueil, à côté de « Se connecter », puis affinez si vous le voulez dans les réglages de l'ordinateur.

## Au début et à la fin d'une session

Quand vous vous connectez, l'ordinateur crée un écran virtuel à la définition et à la fréquence de votre appareil, éteint ses propres écrans et envoie le son vers l'appareil (sur un PC, si Steam est installé ; sur un Mac et sous Linux, par la sortie son « KyberNow »). À la fin, les écrans et la sortie du son reviennent comme avant. L'ordinateur n'est pas verrouillé.

L'accueil annonce ce que la session prendra, par exemple « Écran de 2560 × 1440 à 120 Hz, à la taille de cet écran ». Pour garder les écrans de l'ordinateur tels quels, coupez « Écran virtuel » dans la rubrique Image des réglages de l'ordinateur.

## Les profils

| Profil | Ce qu'il fait |
|---|---|
| Jeu | « La réactivité d'abord, pour jouer. » La latence la plus basse. Des données de secours voyagent avec l'image et comblent les petites pertes, sans attendre un renvoi. |
| Bureau | « Un texte net, pour travailler. » Les couleurs en pleine définition (4:4:4) gardent le texte net. Ce qui se perd est renvoyé, pour que rien ne manque à l'image. |
| Max | « La meilleure image que cet appareil affiche. » Latence basse, texte net et débit élevé, avec les données de secours du profil Jeu. Pour un appareil relié par câble. |
| Auto | « Les meilleurs réglages pour ce réseau et cet appareil. » Un test d'une dizaine de secondes mesure votre réseau et votre appareil, puis propose des réglages. |
| Personnalisé | « Vos propres réglages. » Il apparaît dès qu'un réglage s'écarte du profil choisi. |

Le profil est gardé pour chaque ordinateur. En session, changez-le dans la barre (le nom du profil, puis la liste) : il s'applique tout de suite. Les apps Mac et Windows le proposent aussi dans le menu de leur icône, dans la barre des menus du Mac ou près de l'horloge de Windows.

> Bon à savoir : avec plusieurs écrans sur l'appareil, Bureau et Max prennent tous les écrans, Jeu un seul. Voir [Plusieurs écrans et plusieurs ordinateurs](/guide/plusieurs-ecrans-et-ordinateurs/).

## Le profil Auto

1. Choisissez « Auto ». La première fois, le panneau « Profil Auto » s'ouvre et le test démarre.
2. Cinq étapes se cochent l'une après l'autre : « Liaison » (câble, Wi-Fi ou relais), « Latence au repos », « Débit vers cet appareil », « Décodage de cet appareil » et « Encodeur de l'ordinateur ». Chacune affiche sa mesure, par exemple « 940 Mbit/s · latence en charge +3 ms ».
3. L'écran « Réglages proposés » donne chaque choix avec sa raison : le format, la définition, le texte net, le débit, le transport de l'image et le débit adaptatif.
4. Choisissez « Appliquer ». Rien ne change avant. « Annuler » garde le profil d'avant.

Le résultat est gardé pour cet ordinateur : la fois suivante, Auto s'applique sans test. Sous le débit, la carte « Débit » rappelle la date et le réseau du test (« Choisi par le test du … ») et propose « Refaire le test ». Choisi pendant une session, le test se fait à la fin de la session, quand le réseau est libre : l'app le dit (« Le test se fera à la fin de la session. »).

Si vous vous connectez depuis un autre réseau que celui du test, un bandeau « Autre réseau que celui du test » s'affiche quelques secondes. Le débit adaptatif s'en charge ; « Refaire le test » remesure.

Le profil Auto existe dans toutes les apps KyberNow Link, sauf dans le navigateur.

## Le format vidéo

Dans la rubrique Image des réglages de l'ordinateur, « Format vidéo » propose :

| Format | Pour quoi |
|---|---|
| Auto | Le choix du profil. HEVC la plupart du temps, PyroWave avec Max et Auto quand l'appareil est relié par câble. |
| HEVC | Le format par défaut : une image nette pour un débit raisonnable, en Wi-Fi comme hors de chez vous. |
| H.264 | Le plus ancien, lu par presque tous les appareils et navigateurs. Pas de HDR avec lui. |
| AV1 | Le plus récent, quand la carte graphique de l'ordinateur et votre appareil le savent. Il ne s'enregistre pas. |
| PyroWave | Presque aucun délai de compression, au prix de beaucoup de débit. Pour un câble Ethernet. |

PyroWave n'apparaît que si l'ordinateur le connaît. Grisé, il donne sa raison dans l'aide.

## PyroWave

PyroWave compresse et décompresse chaque image en une fraction de milliseconde. En échange, il envoie 5 à 10 fois plus de données que HEVC : environ 300 Mbit/s en 1080p. Il est fait pour un appareil branché par câble Ethernet sur le même réseau que l'ordinateur.

1. Sur l'ordinateur, ouvrez Admin, espace Image et son. La carte « PyroWave » affiche « Prêt » et le nom de la carte graphique (ou de la puce du Mac) quand l'ordinateur sait le faire. Allumez « Proposer PyroWave ».
2. Sur l'appareil, gardez « Format vidéo » sur « Auto » avec le profil Max ou Auto : l'app prend PyroWave quand l'appareil est relié par câble, HEVC sinon. Les profils Jeu et Bureau restent en HEVC.
3. Pour le forcer en Wi-Fi ou hors de chez vous, choisissez « PyroWave » dans « Format vidéo ». L'app vous prévient : au début de la session, un bandeau « PyroWave en Wi-Fi » ou « PyroWave hors de chez vous » propose « Passer en HEVC », et une puce ambre reste dans la barre.

Quand le débit manque, PyroWave garde d'abord la définition de votre écran, puis la baisse si le réseau ou l'ordinateur ne suit plus : un bandeau « Définition ramenée à … » le dit, avec la nouvelle définition et le débit qu'elle tient. Si le réseau sature plusieurs secondes de suite, le bandeau « Le réseau sature » propose aussi « Passer en HEVC ». Avec PyroWave, le texte net (4:4:4) ne coûte presque que du débit.

Admin règle aussi le « Débit maximal » de PyroWave : « Auto » suit la prise réseau de l'ordinateur, ou 300, 500 et 800 Mbit/s.

> Bon à savoir : la Station le propose sur Windows et Linux avec une carte graphique qui a Vulkan 1.3, et sur un Mac à puce Apple. Dans un navigateur, « Auto » ne le prend jamais : choisissez-le à la main. Parmi les Raspberry Pi, seul le Pi 5 le décode.

## Le débit

Le débit, c'est la quantité de données d'image envoyée chaque seconde. Plus il est haut, plus l'image est fine, et plus il faut de réseau.

« Débit » vaut « Auto » (le débit du profil) ou un débit choisi, par exemple « 80 Mbit/s ». Dessous, l'interrupteur « Débit adaptatif » est allumé par défaut :

- quand le réseau sature (un Wi-Fi chargé, une connexion partagée), l'app baisse le débit, puis le remonte dès que tout va mieux ;
- un pic passager ne change rien : il faut deux secondes chargées d'affilée ;
- coupé, le débit ne bouge jamais ;
- en PyroWave, il se met en pause, et la définition baisse à la place.

Le navigateur garde un débit fixe.

Les Statistiques montrent le débit visé et ce qui le fixe, par exemple « 42 / 60 Mbit/s · baissé, réseau saturé », « 80 Mbit/s · fixe » ou « 440 Mbit/s · plafond de l'encodeur ». Ouvrez-les d'un clic sur les mesures de la barre de session, ou avec le raccourci <kbd>⌃⌥⌘S</kbd> sur Mac, <kbd>Ctrl+Alt+Maj+S</kbd> sur Windows et Linux.

> Bon à savoir : chaque carte graphique a un débit au-delà duquel elle refuse d'encoder. Quand l'ordinateur refuse une image au départ, l'app redescend de 50 Mbit/s et retient ce plafond pour cet ordinateur, quel que soit le profil.

## Le texte net et le transport de l'image

« Texte net (4:4:4) » envoie les couleurs en pleine définition : le texte et les traits fins restent nets, pour un débit plus élevé. Il est grisé quand le HDR est allumé.

« Transport de l'image », dans la rubrique Réseau des réglages de l'ordinateur :

- « Rapide » convient au réseau local et au jeu ;
- « Fiable » renvoie ce qui se perd, au prix d'un peu de délai. C'est le bon choix sur un réseau irrégulier.

## Le HDR

Sur un écran HDR (l'écran XDR d'un Mac, un iPhone, un iPad, une télé HDR derrière l'Apple TV ou un appareil Android TV, un téléviseur LG), une puce « HDR » apparaît sur la carte de l'ordinateur, à côté du profil. Elle est allumée d'office et l'app retient votre choix pour chaque ordinateur. En session, « HDR » se coche et se décoche dans la barre ; l'image se relance en une à deux secondes.

Les jeux et le bureau en HDR arrivent tels quels. L'ordinateur passe son écran virtuel en HDR au début de la session et le remet comme avant à la fin. Tous les profils passent en HDR, en 4:2:0 : le texte net (4:4:4) revient quand vous éteignez le HDR. Le HDR ne compte pas dans le profil, il ne le fait pas passer en « Personnalisé ».

Côté ordinateur, il faut :

- sur un PC, Windows 11 24H2 et une carte graphique qui encode en 10 bits (HEVC ou AV1) ;
- sur un Mac, rien de plus : la Station passe en HDR de la même façon.

Une Station Linux envoie l'image en SDR. Dans Admin, espace Image et son, « HDR » vaut « Si l'appareil le demande » par défaut, ou « Jamais ».

### Régler le HDR de votre écran

L'ordinateur et les jeux règlent leur image sur la luminosité de votre écran. Pour la régler à l'œil, comme sur une console, ouvrez Réglages de l'app › « Réglage du HDR » (ou « Régler le HDR de cet écran… » sous « HDR », dans la rubrique Image des réglages de l'ordinateur).

1. « Luminosité maximale » : montez jusqu'à ce que le symbole du centre disparaisse à peine.
2. « Luminosité sur tout l'écran » : même geste.
3. « Noir » : baissez jusqu'à ce que le symbole disparaisse à peine.
4. « Blanc du bureau » : choisissez la luminosité des fenêtres et du texte de l'ordinateur.

Le réglage vaut à la session suivante. « Revenir au réglage automatique » efface vos valeurs. Sur un téléviseur, passez-le en mode Jeu avec le HGIG : un mappage dynamique des tons fausse le réglage.

Les Statistiques disent « HDR » quand il est transmis. Un invité du partage sans écran HDR, ou dans un navigateur, reçoit une image convertie en SDR.

## Si ça ne marche pas

- **La puce « HDR » est grise.** Son aide donne la raison : « Le HDR demande Windows 11 24H2 sur l'ordinateur. », « La carte graphique de l'ordinateur ne fait pas le HDR. », « Le HDR demande le format HEVC ou AV1. » (repassez « Format vidéo » sur « Auto »), « Cet appareil ne décode pas le HDR. »
- **« L'ordinateur n'a pas pu passer en HDR : image en SDR. »** Windows a refusé le HDR pour cette session. La session continue en SDR ; réessayez à la suivante.
- **« PyroWave » est grisé.** Son aide dit par exemple « Bureau-Salon ne propose pas PyroWave. » (avec le nom de votre ordinateur) : allumez « Proposer PyroWave » dans Admin. Si la carte dit « Indisponible », son message indique quoi faire (mettre à jour le pilote de la carte graphique, puis redémarrer la Station).
- **L'image saccade en PyroWave.** Le réseau ne porte pas son débit : choisissez « Passer en HEVC », ou reliez l'appareil par câble.
- **L'image est floue quand le Wi-Fi est chargé.** C'est le débit adaptatif qui baisse le débit. Les Statistiques le confirment (« baissé, réseau saturé ») ; rapprochez-vous de la box ou passez au câble.
- **Le test Auto affiche « Mettez à jour Bureau-Salon pour mesurer le débit. »** La Station est ancienne : mettez-la à jour, puis « Refaire le test ».
