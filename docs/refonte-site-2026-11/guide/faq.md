---
slug: faq
titre: Questions fréquentes
title: Questions fréquentes · KyberNow
description: Prix, sécurité, compte, appareils compatibles, latence, jeu hors de chez vous et dépannage, les réponses aux questions qu'on nous pose le plus.
rubrique: FAQ
univers: commun
connexes: [ce-qu-il-vous-faut, connecter-un-appareil, hors-de-chez-vous, compte-kybernow]
---

# Découvrir KyberNow

## Qu'est-ce que KyberNow ?

KyberNow affiche votre ordinateur sur vos autres écrans et vous permet de le piloter, pour jouer comme pour travailler. Vous installez KyberNow Station sur l'ordinateur, puis l'app KyberNow Link sur vos appareils. KyberNow Admin, dans le navigateur, règle la Station, et KyberNow Share fait entrer quelqu'un d'autre. Voir [Ce qu'il vous faut pour commencer](/guide/ce-qu-il-vous-faut/).

## KyberNow est-il vraiment gratuit ?

Oui. KyberNow est un logiciel libre, sous licence AGPL-3.0, développé par Les Lignes Claires sans but commercial. Il n'y a ni abonnement ni publicité, et le relais public est ouvert à tous. Le compte KyberNow, les familles et les équipes sont gratuits aussi. Le code est sur [GitHub](https://github.com/MonsieurNours/kybernow).

## Pourquoi KyberNow est-il en bêta ?

KyberNow n'a pas encore atteint la version 1.0. Nous l'utilisons tous les jours, mais des réglages ou des écrans peuvent encore changer d'une version à l'autre. Gardez la Station et vos apps à jour (voir [Mettre à jour KyberNow](/guide/mettre-a-jour/)), et signalez les problèmes sur GitHub ou sur Discord.

## KyberNow peut-il remplacer Sunshine et Moonlight ?

Pour beaucoup d'usages, oui. Comme eux, KyberNow affiche votre ordinateur sur vos autres appareils, dans une app ou dans un navigateur. Il repose sur le moteur Kyber plutôt que sur le protocole GameStream : l'image est toujours chiffrée, l'écran virtuel à la taille de l'appareil est inclus, et la plupart des appareils s'ajoutent en acceptant un code dans Admin, sans rien recopier. La page Technologie compare les deux en détail.

## Est-ce du cloud gaming ?

Non. Le cloud gaming vous loue un ordinateur dans un centre de données, avec un abonnement. KyberNow se sert du vôtre : vous retrouvez vos jeux et vos logiciels tels que vous les avez installés, sans file d'attente. Chez vous, l'image va directement de votre ordinateur à votre appareil, sans passer par un serveur.

## Quel est le lien entre KyberNow et Kyber ?

KyberNow utilise Kyber, le moteur libre qui transporte l'image, le son et les commandes, conçu par des développeurs de VLC. Il y ajoute la Station (installateur, écrans et sortie son virtuels), Admin, les apps KyberNow Link et KyberNow Share. KyberNow n'est ni édité ni approuvé par l'équipe de Kyber, et n'y est pas affilié. Nos correctifs du moteur sont publiés.

# Compte et sécurité

## Faut-il un compte KyberNow ?

Non. Chez vous, l'app trouve l'ordinateur toute seule et vous l'ajoutez avec un code à six chiffres ; le partage et le prêt marchent aussi sans compte. Le compte, facultatif, fait apparaître vos ordinateurs sur tous vos appareils, les rend joignables hors de chez vous sans réglage, et permet de les ouvrir à votre famille ou à votre équipe. Voir [Votre compte KyberNow](/guide/compte-kybernow/).

## KyberNow est-il sûr ?

L'image, le son et vos frappes sont chiffrés entre l'ordinateur et l'app, et l'app vérifie qu'elle parle bien à votre ordinateur grâce à l'empreinte de son certificat. Un nouvel appareil n'entre que si vous acceptez son code dans Admin, ou avec un compte à qui vous avez donné des droits. Rien n'est à ouvrir sur votre box, et chaque accès reste visible dans Admin.

## Le relais de KyberNow peut-il voir mon écran ?

Pas dans les apps KyberNow Link : l'image, le son et vos frappes le traversent chiffrés de bout en bout. Dans un navigateur, avec une Station et un relais à jour, la connexion est aussi chiffrée jusqu'à votre ordinateur. Le relais voit les adresses IP, les débits et les codes de partage. Voir [Retrouver votre ordinateur hors de chez vous](/guide/hors-de-chez-vous/).

## Quelles données KyberNow collecte-t-il ?

Aucune statistique d'usage. Chez vous, rien ne part vers un serveur à nous : l'image va directement de l'ordinateur à vos appareils. Hors de chez vous ou pour un partage, la connexion passe par le relais, qui garde les adresses IP dans ses journaux quelques jours, jamais les codes. Avec un compte, le relais garde aussi votre compte, les droits donnés, le journal des accès et vos réglages synchronisés ; « Exporter mes données » vous en donne une copie.

## Mon entreprise peut-elle utiliser son annuaire ?

Oui. Une équipe peut se connecter avec l'annuaire de l'entreprise, par SSO ou OAuth : chacun entre avec son compte de travail. Un Propriétaire ou un Administrateur la règle dans la console de l'équipe. Voir [Familles et équipes](/guide/familles-et-equipes/).

## J'ai perdu un appareil, que faire ?

Dans Admin, espace Appareils Link, ouvrez l'appareil, puis « Retirer l'accès ». S'il était connecté à votre compte, choisissez aussi « Déconnecter » en face de lui, dans « Compte et sécurité ». Changer le mot de passe du compte déconnecte tous vos appareils d'un coup.

## J'ai oublié le mot de passe d'Admin, que faire ?

Sur l'ordinateur, ouvrez le menu de l'icône KyberNow Station, près de l'horloge, et choisissez « Réinitialiser le mot de passe d'Admin… ». L'icône affiche un code : Admin s'ouvre et vous le demande, avec votre nouveau mot de passe. Vos appareils déjà ajoutés le restent.

## J'ai oublié le mot de passe de mon compte KyberNow, que faire ?

Sur la page de connexion, choisissez « Mot de passe oublié ? » : un lien valable une heure part à votre adresse. Le nouveau mot de passe déconnecte vos autres appareils ; reconnectez-les avec « Connecter un compte ».

# Chez vous et ailleurs

## Puis-je utiliser KyberNow loin de chez moi ?

Oui. Avec un compte KyberNow, vos ordinateurs vous suivent partout. Sans compte, ajoutez l'appareil chez vous, puis allumez « Accès hors de chez vous » dans Admin. Un VPN comme Tailscale ou WireGuard marche aussi. L'ordinateur doit rester allumé. Voir [Retrouver votre ordinateur hors de chez vous](/guide/hors-de-chez-vous/).

## Faut-il ouvrir des ports sur ma box ?

Non, et mieux vaut ne pas le faire. C'est la Station qui ouvre la connexion vers le relais, et un VPN n'en a pas besoin non plus. Votre ordinateur reste ainsi invisible pour le reste d'internet.

## Combien de données consomme une session ?

Cela dépend du débit choisi. À 20 Mbit/s, une heure de session consomme environ 9 Go. Hors de chez vous, prenez le profil Jeu ou Bureau, ou laissez « Débit adaptatif » allumé : l'app baisse le débit quand le réseau sature. Voir [Profils, format vidéo, débit et HDR](/guide/profils-et-qualite/).

## Quelle latence attendre ?

Sur un réseau local, nous avons mesuré 9,4 ms entre la capture de l'image sur le PC et son affichage sur l'appareil. C'est une médiane : la moitié des images arrivent plus vite. Chez vous, la latence dépend surtout de la carte graphique, du réseau et de l'appareil, et l'app l'affiche en direct. Par le relais, comptez de 5 à 20 ms de plus en France.

## Puis-je installer mon propre relais ?

Oui, sur une machine Linux avec Docker et un nom de domaine. Vos partages, vos accès hors de chez vous et, si vous le voulez, vos comptes passent alors par une machine que vous tenez. Voir [Installer votre propre relais](/guide/installer-votre-relais/).

# Ordinateurs et appareils

## Sur quels ordinateurs installer la Station ?

Sur un PC sous Windows 10 ou 11, sur un Mac à puce Apple sous macOS 15 ou plus récent, ou sur un ordinateur sous Linux : Ubuntu, Debian, Fedora, et aussi Bazzite, Fedora Silverblue ou SteamOS. Voir [Installer sur Windows](/guide/installer-station-windows/), [sur Mac](/guide/installer-station-mac/) et [sur Linux](/guide/installer-station-linux/).

## Sur quels appareils retrouver mon ordinateur ?

Un Mac à puce Apple, un PC Windows, un PC Linux, le Steam Deck, une Apple TV 4K de 2021 ou plus récente, un iPhone ou un iPad, un appareil Android (téléphone, tablette, téléviseur), un téléviseur LG, un Raspberry Pi, et tout navigateur récent. Voir [Ce qu'il vous faut pour commencer](/guide/ce-qu-il-vous-faut/).

## Faut-il une carte graphique particulière ?

Non. Sur un PC Windows, la Station compresse l'image avec n'importe quelle carte NVIDIA, AMD ou Intel, même externe (eGPU) ; sans carte graphique, le processeur s'en charge. Seul le format AV1 demande une carte NVIDIA RTX 40 ou 50. Sur un Mac, la puce Apple suffit.

## Quelles définitions et fréquences d'image sont possibles ?

Celles de votre écran : à chaque session, l'ordinateur crée un écran virtuel à la définition et à la fréquence de votre appareil, jusqu'à 7680 × 4320 et 500 Hz. Certains appareils ont leur propre plafond : 4K à 60 Hz sur l'Apple TV, la NVIDIA Shield et le Raspberry Pi, 4K à 120 Hz sur les téléviseurs LG. Voir [Profils, format vidéo, débit et HDR](/guide/profils-et-qualite/).

## KyberNow gère-t-il le HDR ?

Oui, quand l'écran de votre appareil est HDR : une puce « HDR » apparaît sur la carte de l'ordinateur, allumée d'office, et les jeux arrivent avec leurs couleurs et leurs lumières. Sur un PC, il faut Windows 11 24H2 et une carte graphique qui encode en 10 bits. Voir [Profils, format vidéo, débit et HDR](/guide/profils-et-qualite/).

## Qu'est-ce que PyroWave ?

Un format d'image qui se compresse et se décompresse en une fraction de milliseconde, pour gagner encore du délai sur le réseau de la maison. Il demande beaucoup de débit, environ 300 Mbit/s en 1080p : en « Auto », l'app ne le prend que si votre appareil est relié par câble et que l'ordinateur le propose. Voir [Profils, format vidéo, débit et HDR](/guide/profils-et-qualite/).

## L'app Mac fonctionne-t-elle sur un Mac Intel ?

Non, elle demande un Mac à puce Apple. Sur un Mac Intel, ouvrez votre ordinateur dans un navigateur, de préférence Chrome ou Edge. Voir [Utiliser votre ordinateur dans un navigateur](/guide/navigateur/).

## KyberNow est-il sur l'App Store ?

Oui, pour l'iPhone, l'iPad et l'Apple TV : cherchez « KyberNow ». Il faut iOS 18 ou iPadOS 18 ou plus récent, et une Apple TV 4K de 2021 ou plus récente. Voir [Utiliser votre ordinateur depuis un iPhone ou un iPad](/guide/iphone-ipad/) et [Jouer sur la télé avec l'Apple TV](/guide/apple-tv/).

## KyberNow fonctionne-t-il sur Android ?

Oui. La même app sert au téléphone, à la tablette et au téléviseur, à partir d'Android 11. Elle s'installe à partir de son fichier APK, publié avec chaque version, puis se met à jour elle-même. Voir [Utiliser votre ordinateur depuis Android](/guide/android/).

## KyberNow fonctionne-t-il sur un téléviseur LG ?

Oui, sur les téléviseurs LG sous webOS 5 ou plus récent (modèles 2020 et après), jusqu'à la 4K à 120 Hz, avec manettes, Magic Remote et son 5.1 vers un ampli. L'app s'installe en mode développeur depuis un ordinateur, avec un compte LG gratuit ; pensez à prolonger ce mode de temps en temps. Voir [Jouer sur un téléviseur LG](/guide/televiseur-lg/).

## KyberNow fonctionne-t-il sur Linux et le Steam Deck ?

Oui. La même app fonctionne sur un PC Linux (x86_64 ou ARM), sur le Steam Deck et sur SteamOS au salon, avec une interface adaptée à chacun : souris et clavier sur un PC, manette sur le Deck et à la télé. Elle s'installe en paquet Flatpak, sans droits d'administrateur. Voir [Utiliser votre ordinateur depuis Linux ou un Steam Deck](/guide/link-linux-steam-deck/).

## KyberNow fonctionne-t-il sur un Raspberry Pi ?

Oui, avec un Pi 3, 4 ou 5. KyberNow Link OS se copie sur une carte microSD : le Pi démarre directement sur KyberNow Link, comme une console branchée à la télé. Vous pouvez aussi installer l'app seule sur Raspberry Pi OS 64 bits. Voir [Brancher un Raspberry Pi à la télé](/guide/raspberry-pi/).

## Puis-je jouer à ma PlayStation avec KyberNow ?

Oui. Une PS5 ou une PS4 de la maison apparaît sur l'accueil de KyberNow Link, à côté de vos ordinateurs. Vous l'associez une fois avec le code qu'elle affiche, puis « Jouer ». Voir [Jouer à votre PS5 ou PS4](/guide/playstation/).

# Partager et prêter

## Puis-je partager mon ordinateur avec un ami ?

Oui, avec KyberNow Share. Vous ouvrez un partage, et jusqu'à quatre invités entrent avec un code à six chiffres, un lien ou un QR code, dans l'app ou dans un navigateur, chez vous comme à distance. Un invité arrive en spectateur ; vous lui donnez, ou non, les manettes, le clavier et la souris. Voir [Partager votre écran avec KyberNow Share](/guide/partager/).

## Puis-je prêter mon ordinateur ?

Oui. Avec un code pour une séance, un compte d'ami créé dans Admin, ou le droit « Emprunter pour jouer » d'un compte KyberNow, votre ami joue depuis chez lui pendant que vous ne vous servez pas de l'ordinateur, avec les apps que vous permettez. Voir [Prêter votre ordinateur](/guide/preter/).

## Quelqu'un peut-il seulement regarder mon écran ?

Oui, avec le droit « Regarder » d'un compte KyberNow. La personne voit le vrai écran de l'ordinateur, sans clavier, souris ni manette, et Admin vous dit qu'elle regarde. Voir [Votre compte KyberNow](/guide/compte-kybernow/).

## Deux appareils peuvent-ils se connecter en même temps ?

Pas à la même session : le second prend la main, le premier est déconnecté, et l'écran virtuel est recréé à la taille du nouvel appareil. Pour faire entrer quelqu'un d'autre sans couper votre session, ouvrez un partage. Voir [Partager votre écran avec KyberNow Share](/guide/partager/).

## Puis-je ouvrir plusieurs ordinateurs à la fois ?

Oui, depuis les apps Mac, Windows et Linux : une fenêtre par ordinateur, quatre au plus. Seule la fenêtre au premier plan reçoit le clavier, la souris et les manettes. Voir [Plusieurs écrans et plusieurs ordinateurs](/guide/plusieurs-ecrans-et-ordinateurs/).

# Dépannage

## Mes écrans s'éteignent pendant la session, c'est normal ?

Oui. Pendant la session, l'ordinateur n'affiche que l'écran virtuel, à la taille de votre appareil, et éteint ses propres écrans. Ils se rallument à la fin. Si l'app se coupe sans prévenir, la Station attend 20 secondes, puis retire l'écran virtuel et rallume vos écrans ; au besoin, Admin a un bouton « Rallumer mes écrans », dans Image et son.

## Pourquoi le son sort-il encore des haut-parleurs du PC ?

Sur un PC Windows, la Station envoie le son vers vos appareils par les haut-parleurs virtuels de Steam. Installez Steam, puis relancez l'installateur de KyberNow Station et choisissez « Garder mes réglages » : l'option « Son sur vos appareils » devient disponible, et vos appareils restent ajoutés. Sur un Mac et sous Linux, la Station installe sa propre sortie son.

## Ma manette ne répond pas, que faire ?

Vérifiez d'abord qu'elle est reliée à votre appareil, en Bluetooth ou par câble ; plusieurs apps ont un écran pour tester les manettes. Sur un PC Windows, les manettes passent par le pilote ViGEmBus, installé avec la Station ; après la première installation, il faut parfois redémarrer. L'ordinateur accepte quatre manettes au plus, et tant que le menu de session est ouvert, la manette pilote l'app et le jeu ne reçoit rien.

## L'image reste noire ou se fige, que faire ?

Mettez d'abord la Station et l'app à jour. Essayez ensuite une définition plus basse, ou un autre format (H.264 ou HEVC) dans les réglages de l'ordinateur, dans l'app. Branchez l'ordinateur en Ethernet si possible, et vérifiez qu'aucun autre logiciel de jeu à distance ne tourne dessus. Les journaux d'Admin (Système › Journaux) aident à trouver la cause.

## Pourquoi mon ordinateur n'apparaît-il pas dans l'app ?

Vérifiez que l'appareil est sur le même réseau que l'ordinateur, et que la Station tourne (son icône est près de l'horloge, ou dans la barre des menus sur Mac). Vous pouvez aussi l'ajouter « Par adresse », avec l'adresse de l'ordinateur et le port d'Admin. Voir [Connecter un appareil à votre ordinateur](/guide/connecter-un-appareil/).

## Où poser une autre question ?

Sur le [forum d'aide du Discord](https://discord.gg/sVy2mu5WdT), en français ou en anglais. Pour signaler un problème, les [tickets GitHub](https://github.com/MonsieurNours/kybernow/issues) sont ouverts à tous.
