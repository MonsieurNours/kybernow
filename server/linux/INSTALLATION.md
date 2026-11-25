# Installer KyberNow Station sur Linux

[English](INSTALLATION.en.md) · Français

KyberNow Station fait de votre ordinateur sous Linux une Station : son écran, son son et ses manettes sur vos autres appareils, avec les apps KyberNow Link. Elle fonctionne sous GNOME et KDE Plasma, sous Wayland. Tout se télécharge sur la page des versions : https://github.com/MonsieurNours/kybernow/releases

## Ubuntu, Debian, Fedora

Prenez le paquet de votre système et de votre processeur (`amd64`/`x86_64` pour un PC, `arm64`/`aarch64` pour une machine ARM), puis :

```sh
sudo apt install ./kybernow-station_<version>_amd64.deb     # Ubuntu, Debian
sudo dnf install ./kybernow-station-<version>.x86_64.rpm     # Fedora
```

La Station démarre à votre prochaine ouverture de session.

## Bazzite, Fedora Silverblue ou Kinoite, SteamOS

Sur ces systèmes, le dossier du système ne se modifie pas : la Station s'installe dans votre dossier personnel, sans redémarrer, et reste en place quand le système se met à jour. Elle se met ensuite à jour toute seule, depuis Admin, sans mot de passe.

1. **Sur SteamOS** : passez au mode Bureau (bouton Steam › Alimentation › Passer au bureau). La Station fonctionne dans le mode Bureau, pas dans le mode Jeu.
2. **Téléchargez** `KyberNow-Station-Linux-x86_64.tar.xz` (ou `…-aarch64.tar.xz` pour une machine ARM).
3. **Installez** : ouvrez un terminal (Konsole) dans le dossier du téléchargement et tapez

   ```sh
   tar -xf KyberNow-Station-Linux-x86_64.tar.xz
   ./kybernow-station-*/install.sh
   ```

4. **Si votre mot de passe est demandé** : c'est pour que le clavier, la souris et les manettes de vos appareils puissent piloter l'ordinateur (une règle du système, posée une fois). Sur SteamOS, si vous n'avez jamais choisi de mot de passe, tapez d'abord `passwd`. Sur la plupart des machines avec Steam, ce n'est pas demandé : Steam l'a déjà fait.
5. **C'est prêt** : l'installation donne l'adresse d'Admin et l'endroit du mot de passe de votre premier compte. Vos appareils trouvent la Station tout seuls sur le réseau.

Pour que la Station soit là après un redémarrage, votre session doit s'ouvrir toute seule. Sur SteamOS et Bazzite Deck, qui démarrent en mode Jeu, choisissez de démarrer en mode Bureau :

```sh
steamos-session-select plasma-persistent
```

### Retirer la Station

```sh
~/.local/lib/kybernow-station/current/bin/kybernow-station-home uninstall
```

Vos réglages, comptes et appairages restent, pour une prochaine installation ; ajoutez `--purge` pour les retirer aussi.
