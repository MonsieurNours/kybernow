# Installer KyberNow Link sur Linux et le Steam Deck

[English](INSTALLATION.en.md) · Français

KyberNow Link pour Linux est un paquet **Flatpak**, joint à chaque version publiée sur GitHub : `KyberNow-Link-Linux-x86_64.flatpak` (Steam Deck, PC) et `KyberNow-Link-Linux-aarch64.flatpak` (machines ARM). Il s'installe sans droits d'administrateur ; le runtime de freedesktop vient de Flathub au premier lancement.

## Steam Deck (SteamOS)

1. **Mode Bureau** : bouton Steam › Alimentation › Passer au bureau.
2. **Télécharger** `KyberNow-Link-Linux-x86_64.flatpak` depuis la page des versions (https://github.com/MonsieurNours/kybernow/releases), avec Firefox ou le navigateur du Deck.
3. **Installer** : ouvrir Konsole et taper

   ```sh
   flatpak install --user ~/Téléchargements/KyberNow-Link-Linux-x86_64.flatpak
   ```

   (ou `~/Downloads/…` selon la langue du Deck). Un double clic sur le fichier ouvre aussi Discover, qui l'installe.
4. **L'ajouter à Steam** : ouvrir Steam en mode Bureau › Jeux › Ajouter un jeu non-Steam › cocher **KyberNow Link** › Ajouter.
5. **Revenir au mode Jeu** (icône « Retour au mode Jeu » du bureau). KyberNow Link est dans la bibliothèque, onglet Non-Steam.
6. **Disposition des commandes** : dans la fiche de KyberNow Link › manette › Disposition, choisir le modèle **« Manette avec pavé tactile en souris »** (Gamepad with Mouse Trackpad) : les sticks et boutons vont à la manette de l'ordinateur, le pavé droit déplace la souris, les gâchettes du pavé cliquent.

Ensuite, à la manette : A valide, B revient, Y ouvre les réglages de l'ordinateur, X ajoute un ordinateur, L1 et R1 changent de profil, Start ouvre les réglages de l'app. En session, **L1 + R1 + Start + Select** tenus ensemble (ou un toucher en haut de l'écran) ouvrent le menu ; « Clavier » y ouvre le clavier de Steam.

## SteamOS au salon (Steam Machine, PC sous SteamOS ou Bazzite)

Comme le Deck ; l'app prend d'elle-même l'interface du salon (texte plus grand, colonne « Prêt à jouer ») quand l'écran dépasse 1280 × 800 sous gamescope. Réglages de l'app › Interface force Bureau, Portable ou Salon.

## Linux de bureau (GNOME, KDE…)

```sh
flatpak install --user KyberNow-Link-Linux-x86_64.flatpak
flatpak run fr.leslignesclaires.kybernow
```

KyberNow Link apparaît aussi dans le menu des applications. Raccourcis en session : **Ctrl+Alt+Maj+D** se déconnecter, **Ctrl+Alt+Maj+F** plein écran, **Ctrl+Alt+Maj+M** souris des jeux ou du bureau, **Échap** tenu une seconde rend la souris.

## Mettre à jour

Un Flatpak installé depuis un fichier ne se met pas à jour seul : l'app signale une nouvelle version (« Version X disponible »), qu'on installe de la même façon (`flatpak install --user` du nouveau fichier ; les ordinateurs et réglages restent).

## Désinstaller

```sh
flatpak uninstall --user fr.leslignesclaires.kybernow
rm -rf ~/.var/app/fr.leslignesclaires.kybernow   # ordinateurs, réglages, journal
```

## Journal

`~/.var/app/fr.leslignesclaires.kybernow/config/kybernow/kybernow.log` (le précédent en `.1`). Lancée depuis un terminal (`flatpak run fr.leslignesclaires.kybernow`), l'app écrit aussi le journal à l'écran.
