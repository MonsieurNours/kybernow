# Installer KyberNow Link sur Linux, le Steam Deck et le Raspberry Pi

[English](INSTALLATION.en.md) · Français

KyberNow Link pour Linux est un paquet **Flatpak**, joint à chaque version publiée sur GitHub : `KyberNow-Link-Linux-x86_64.flatpak` (Steam Deck, PC) et `KyberNow-Link-Linux-aarch64.flatpak` (machines ARM). Il s'installe sans droits d'administrateur ; le runtime de freedesktop vient de Flathub au premier lancement. Pour le **Raspberry Pi**, c'est un paquet **.deb** (`KyberNow-Link-RaspberryPi-arm64.deb`), voir plus bas.

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

## Raspberry Pi (Raspberry Pi OS)

Pour un Raspberry Pi 3, 4 ou 5 (Pi 400, 500 et Compute Module compris) sous **Raspberry Pi OS 64 bits** (Debian 13 « trixie », bureau ou Lite avec un bureau), le paquet est un `.deb` qui utilise les décodeurs de la puce : HEVC sur les Pi 4 et 5 (jusqu'à la 4K à 60 Hz), H.264 sur les Pi 3 et 4.

1. **Télécharger** `KyberNow-Link-RaspberryPi-arm64.deb` depuis la page des versions (https://github.com/MonsieurNours/kybernow/releases).
2. **Installer** (le terminal, ou un double clic dans le gestionnaire de fichiers) :

   ```sh
   sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb
   ```

   apt installe aussi ce qui manque (SDL2, libopus, libdrm…). KyberNow Link apparaît dans le menu, rubrique Jeux ; en ligne de commande : `kybernow`.
3. **Au téléviseur, à la manette** : Réglages de l'app › Interface › Salon donne l'interface à trois mètres (A valide, B revient, Y réglages, X ajouter ; menu de session par L1 + R1 + Start + Select). `kybernow --ui tv` fait de même.

Ce que le Pi permet est dit dans Réglages › Image (choix grisés, étiquettes « décodeur du Pi » et « processeur ») : 4K à 60 Hz au plus sur la sortie HDMI des Pi 4 et 5, 1080p à 120 Hz si le téléviseur l'annonce, pas de fréquence variable ; sur le Pi 5 le H.264 passe par le processeur (1080p), le HEVC par la puce ; les couleurs 4:4:4 par le processeur, 1080p au plus.

**Dépôt apt** (quand il sera publié) : les mises à jour viendront alors avec `sudo apt upgrade`. Le dépôt se déclare une fois :

```sh
curl -fsSL https://www.kybernow.app/apt/kybernow.gpg | sudo tee /usr/share/keyrings/kybernow.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/kybernow.gpg] https://www.kybernow.app/apt stable main" | sudo tee /etc/apt/sources.list.d/kybernow.list
sudo apt update && sudo apt install kybernow-link
```

Désinstaller : `sudo apt remove kybernow-link` ; les ordinateurs, réglages et le journal sont dans `~/.config/kybernow/`.

**Pas encore essayé sur une carte** (23/10/2026) : le paquet est construit et installé dans une machine virtuelle ARM64 ; le décodeur HEVC du Pi, la sortie HDMI et les manettes Bluetooth restent à vérifier sur un vrai Raspberry Pi. Le Flatpak aarch64 marche aussi sur le Pi, mais sans le décodeur de la puce (HEVC par le processeur).

## Mettre à jour

Un Flatpak installé depuis un fichier ne se met pas à jour seul : l'app signale une nouvelle version (« Version X disponible »), qu'on installe de la même façon (`flatpak install --user` du nouveau fichier ; les ordinateurs et réglages restent). Même chose pour le `.deb` du Raspberry Pi (`sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`), tant que le dépôt apt n'est pas déclaré.

## Désinstaller

```sh
flatpak uninstall --user fr.leslignesclaires.kybernow
rm -rf ~/.var/app/fr.leslignesclaires.kybernow   # ordinateurs, réglages, journal
```

## Journal

`~/.var/app/fr.leslignesclaires.kybernow/config/kybernow/kybernow.log` (le précédent en `.1`). Lancée depuis un terminal (`flatpak run fr.leslignesclaires.kybernow`), l'app écrit aussi le journal à l'écran.
