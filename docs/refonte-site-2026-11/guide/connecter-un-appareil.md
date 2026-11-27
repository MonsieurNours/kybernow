---
slug: connecter-un-appareil
titre: Connecter un appareil à votre ordinateur
title: Connecter un appareil à votre ordinateur · KyberNow
description: Installez KyberNow Link, ajoutez votre ordinateur par le réseau, un code, une adresse ou un QR code, puis ouvrez votre première session.
rubrique: Connecter un appareil
univers: commun
duree: 3 min
connexes: [ce-qu-il-vous-faut, compte-kybernow, hors-de-chez-vous, profils-et-qualite]
---

Sur chaque appareil, vous installez KyberNow Link, puis vous y ajoutez votre ordinateur une fois. Ensuite, il vous attend à l'accueil de l'app, et « Se connecter » ouvre la session.

## Le principe

1. La Station est installée et en marche sur l'ordinateur : [Windows](/guide/installer-station-windows/), [Mac](/guide/installer-station-mac/) ou [Linux](/guide/installer-station-linux/).
2. Vous installez KyberNow Link sur l'appareil. Chaque appareil a son article, plus bas.
3. Sur le même réseau, l'ordinateur apparaît tout seul dans l'app : chaque Station s'annonce sur le réseau local.
4. Vous lui donnez accès. Selon l'app, vous acceptez dans Admin le code à six chiffres qu'elle affiche, ou vous entrez le compte créé à l'installation de la Station.
5. L'ordinateur arrive sur l'accueil de l'app. « Se connecter » ouvre la session.

## Au premier lancement de l'app

KyberNow Link commence par trois écrans d'accueil. Le deuxième propose un compte KyberNow : « Connecter un compte », « Créer un compte », ou « Continuer sans compte ». Le compte est facultatif. Avec lui, les ordinateurs de votre compte arrivent d'eux-mêmes, sans rien ajouter. Voir [Votre compte KyberNow](/guide/compte-kybernow/).

Le troisième écran, « Ajoutez votre ordinateur », montre les ordinateurs trouvés sur le réseau, avec « Ajouter ». Vous pouvez aussi passer ces écrans avec « Passer » et ajouter l'ordinateur plus tard, avec « Ajouter un ordinateur ou une console » sur l'accueil.

## Accepter le code dans Admin

C'est la façon la plus simple, sur les apps qui la proposent : rien à taper sur l'appareil.

1. L'app affiche un code à six chiffres et l'envoie aux Stations du réseau.
2. Sur l'ordinateur, ouvrez KyberNow Admin. Un bandeau annonce que l'appareil demande l'accès à cet ordinateur, avec le code. Cliquez sur « Voir la demande ». Sur un PC, le panneau de l'icône près de l'horloge montre aussi la demande ; sur un Mac, une notification l'annonce.
3. Dans l'espace Appareils Link, vérifiez que le code est bien celui de l'appareil, puis cliquez sur « Accepter ».

L'appareil reçoit son propre accès, sans mot de passe à taper. Vous pourrez le lui retirer dans Appareils Link, sur sa carte, avec « Retirer l'accès ».

Le code ne vaut que quelques minutes, puis l'app en tire un autre. Il ne passe que sur le réseau local : par un VPN, ajoutez l'ordinateur par son adresse.

> Bon à savoir : si une session est en cours sur l'ordinateur, la Station attend sa fin pour redémarrer et prendre en compte le nouvel accès. L'appareil peut se connecter juste après.

## Les façons d'ajouter un ordinateur

| Façon | Quand la choisir | Ce qu'il faut |
|---|---|---|
| Sur le réseau | L'appareil est chez vous, sur le même réseau | Rien : l'ordinateur apparaît |
| Code à l'écran | Sur une télé ou un appareil où taper est pénible | Accepter le code dans Admin |
| Par adresse | Par un VPN, ou sur un réseau où l'ordinateur ne s'annonce pas | L'adresse IP ou le nom de l'ordinateur, et le port d'Admin |
| QR code ou lien | Un téléphone, une tablette ou un ordinateur qui a une caméra, ou un lien reçu | Admin › Appareils Link › « Ajouter un appareil » |
| Depuis votre Mac | Apple TV, iPhone ou iPad, quand l'app Mac connaît déjà l'ordinateur | Le même compte iCloud |
| Compte KyberNow | Sur tous vos appareils, chez vous et ailleurs | « Connecter un compte » dans l'app |
| Invitation | Un appareil loin de chez vous, sans que vous ayez à l'accepter | Un code et un mot de passe donnés par Admin |

Dans Admin, l'espace Appareils Link affiche, sous « Ajouter un appareil », le QR code d'appairage, « Copier le lien » et l'empreinte de l'ordinateur. Le lien contient l'adresse de l'ordinateur et l'empreinte de son certificat, jamais de mot de passe.

Une invitation se crée dans le même espace, section « Invitations ». Sur l'appareil, elle s'utilise avec « Rejoindre avec un code » : voir [Ajouter un appareil quand vous êtes loin](/guide/hors-de-chez-vous/).

« Rejoindre avec un code » sert aussi à entrer dans le partage d'un ami ou à emprunter son ordinateur : voir [Partager votre écran avec KyberNow Share](/guide/partager/) et [Prêter votre ordinateur](/guide/preter/).

## Ce que propose chaque app

| App | Façons d'ajouter | Accès |
|---|---|---|
| [Mac](/guide/link-mac/) | Sur ce réseau, Par adresse, QR code ou lien | Le compte de la Station |
| [Windows](/guide/link-windows/) | Sur le réseau, Avec le lien d'Admin, Par adresse | Un code ou le compte de la Station |
| [Linux et Steam Deck](/guide/link-linux-steam-deck/) | Sur le réseau, Par adresse | Un code ou le compte de la Station |
| [Apple TV](/guide/apple-tv/) | Depuis votre Mac, Sur le réseau, Code à l'écran, Adresse | Un code ou le compte de la Station |
| [iPhone et iPad](/guide/iphone-ipad/) | Depuis votre Mac, Sur le réseau, Code à l'écran, Adresse, QR code d'appairage | Un code ou le compte de la Station |
| [Android](/guide/android/) | Réseau, Code, Adresse, QR code | Un code ou le compte de la Station |
| [Téléviseur LG](/guide/televiseur-lg/) | Sur le réseau, Par adresse et port | Un code |
| [Raspberry Pi](/guide/raspberry-pi/) | Sur le réseau, Par adresse | Un code ou le compte de la Station |
| [Navigateur](/guide/navigateur/) | L'adresse d'Admin suivie de /link | Le compte de la Station |

## Vérifier l'empreinte

Chaque Station prouve son identité avec un certificat qu'elle a créé à l'installation. Son empreinte est une suite de groupes de caractères.

- Trouvé sur le réseau, ou ajouté par un lien ou un QR code, l'ordinateur est déjà vérifié : l'app connaît son empreinte et la compare toute seule.
- Ajouté par son adresse, l'app vous montre l'empreinte. Comparez les quatre premiers groupes avec ceux qu'affiche Admin, dans Appareils Link, sous « Ajouter un appareil ». S'ils sont identiques, c'est bien votre ordinateur. S'ils diffèrent, n'allez pas plus loin.

## Ensuite, à l'accueil

L'ordinateur s'affiche avec son état : « En ligne », « Hors ligne », ou « Hors de chez vous » quand l'app le joint par le relais. Hors ligne, « Réveiller l'ordinateur » l'allume à distance, si le réveil par le réseau est autorisé dans Admin › Système.

Sous « Se connecter », choisissez un profil : Jeu, Bureau, Max ou Auto. Voir [Profils, format vidéo, débit et HDR](/guide/profils-et-qualite/).

## Si ça ne marche pas

- **L'ordinateur n'apparaît pas.** Vérifiez qu'il est allumé, que la Station tourne et que l'appareil est sur le même réseau. Sur un PC, le réseau doit être classé « Privé » dans Windows. Par un VPN, passez par l'adresse.
- **Le code a expiré, ou Admin l'a refusé.** Demandez un nouveau code dans l'app et acceptez-le plus vite.
- **« Mot de passe refusé : mettez-le à jour dans les réglages. »** Le mot de passe du compte a changé dans Admin. Entrez le nouveau dans les réglages de l'ordinateur, rubrique Station.
- **« La Station a été réinstallée : vérifiez son empreinte dans les réglages. »** La Station a été réinstallée avec « Repartir de zéro ». Comparez la nouvelle empreinte avec celle d'Admin, puis ajoutez l'ordinateur de nouveau.
- **« La connexion a échoué : l'ordinateur ne répond pas ».** Vérifiez qu'il est allumé et que KyberNow Station tourne, puis réessayez.
- **L'app demande de mettre la Station à jour.** La Station est trop ancienne pour cette façon d'ajouter. Mettez-la à jour : voir [Mettre à jour KyberNow](/guide/mettre-a-jour/).
