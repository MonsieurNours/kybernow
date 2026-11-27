---
slug: desinstaller
titre: Désinstaller KyberNow
title: Désinstaller la Station et les apps · KyberNow
description: Retirez KyberNow Station de Windows, du Mac ou de Linux, puis l'app KyberNow Link de chaque appareil, en gardant ou non vos réglages.
rubrique: Entretien
univers: commun
duree: 5 min
connexes: [mettre-a-jour, compte-kybernow, installer-station-windows]
---

Chaque app KyberNow se retire comme les autres apps de son système. La Station vous demande si vous gardez vos réglages, utiles pour une réinstallation.

## Avant de commencer

- **L'ordinateur est rattaché à un compte KyberNow ?** Détachez-le d'abord : dans Admin, espace Compte KyberNow, « Détacher », ou sur la page du compte, « Détacher de votre compte ». Sinon, il reste dans la liste de vos ordinateurs, hors ligne.
- **Vous réinstallerez ?** Gardez vos réglages. La Station garde alors son identité auprès du relais : vos appareils, vos liens de partage et l'accès hors de chez vous marchent de nouveau sans rien refaire.
- **Vous quittez KyberNow pour de bon ?** Supprimez aussi votre compte, sur la page du compte, « Compte et sécurité », « Supprimer le compte ». Désinstaller les apps ne le supprime pas.

## KyberNow Station sur Windows

1. Ouvrez Paramètres Windows › Applications › Applications installées.
2. Choisissez KyberNow Station, puis « Désinstaller ».
3. Dites si vous gardez vos réglages.

Le désinstalleur retire les services, Admin, l'écran virtuel et son certificat, les règles du pare-feu et les raccourcis du menu Démarrer. Il remet le mode d'alimentation d'avant, et votre réseau en « Public » s'il l'avait passé en « Privé ». Le pilote de manettes ViGEmBus et le câble son virtuel VB-CABLE restent installés, car d'autres logiciels peuvent s'en servir : retirez-les vous-même si vous n'en avez plus l'usage.

## KyberNow Station sur Mac

1. Rouvrez le fichier KyberNow-Station-Mac.dmg, ou le menu de l'icône de la Station, dans la barre des menus.
2. Lancez « Désinstaller KyberNow Station ».
3. Dites si vous gardez vos réglages. macOS demande le mot de passe d'un administrateur du Mac.

La Station rend vos écrans et votre sortie son d'origine, puis retire l'app, sa sortie son, ses services et ses règles du pare-feu. Les autorisations données à la Station (écran, accessibilité, micro) sont oubliées.

## KyberNow Station sur Linux

Sur Ubuntu, Debian ou Fedora, retirez le paquet :

```sh
sudo apt remove kybernow-station     # Ubuntu, Debian
sudo dnf remove kybernow-station     # Fedora
```

La désinstallation arrête la Station dans les sessions ouvertes et referme les ports qu'elle avait ouverts dans le pare-feu.

Sur Bazzite, Fedora Silverblue ou Kinoite, et SteamOS, la Station est dans votre dossier personnel :

```sh
~/.local/lib/kybernow-station/current/bin/kybernow-station-home uninstall
```

Vos réglages, vos comptes et vos appairages restent pour une prochaine installation ; ajoutez `--purge` pour les retirer aussi.

## Les apps KyberNow Link

| Appareil | Comment la retirer |
|---|---|
| Mac | Glissez KyberNow Link du dossier Applications à la corbeille. |
| PC Windows | Paramètres Windows › Applications › Applications installées › KyberNow Link › « Désinstaller ». L'app propose d'effacer aussi la liste de vos ordinateurs, leurs réglages et leurs mots de passe ; sinon, ils seront là si vous la réinstallez. |
| Apple TV | Sur l'écran d'accueil, maintenez le pavé tactile appuyé sur l'icône de KyberNow, puis choisissez de supprimer l'app. |
| iPhone, iPad | Touchez longuement l'icône de KyberNow, puis « Supprimer l'app ». |
| Android | Paramètres › Applications, KyberNow, puis « Désinstaller ». |
| Téléviseur LG | Dans webOS Dev Manager, onglet Apps, ou en ligne de commande : `ares-install --device tv --remove fr.leslignesclaires.kybernow`. Quitter le mode développeur la retire aussi. |
| Linux, Steam Deck, SteamOS | Dans GNOME Logiciels ou Discover, ou dans un terminal (voir plus bas). Sur le Deck, retirez aussi KyberNow Link de la bibliothèque de Steam. |
| Raspberry Pi, l'app seule | `sudo apt remove kybernow-link`. Vos ordinateurs, vos réglages et le journal restent dans `~/.config/kybernow/`. |
| KyberNow Link OS | C'est tout le système de la carte. Pour rendre le Pi à un autre usage, copiez une autre image sur la carte avec Raspberry Pi Imager. |
| Navigateur | Rien à désinstaller. |

Sous Linux, dans un terminal, la première ligne retire l'app ; la seconde efface aussi vos ordinateurs, vos réglages et le journal :

```sh
flatpak uninstall --user fr.leslignesclaires.kybernow
rm -rf ~/.var/app/fr.leslignesclaires.kybernow
```

> Bon à savoir : vous donnez ou vendez un appareil ? Retirez-lui aussi l'accès à l'ordinateur. Dans Admin, espace Appareils Link, ouvrez l'appareil, puis « Retirer l'accès » ou « Oublier cet appareil ». S'il était connecté à votre compte, choisissez « Déconnecter » en face de lui, dans « Compte et sécurité ».
