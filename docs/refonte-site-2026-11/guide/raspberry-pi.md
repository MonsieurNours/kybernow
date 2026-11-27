---
slug: raspberry-pi
titre: Brancher un Raspberry Pi à la télé
title: KyberNow Link sur Raspberry Pi · KyberNow
description: Copiez KyberNow Link OS sur une carte microSD ou installez l'app sur Raspberry Pi OS, et retrouvez votre ordinateur sur la télé.
rubrique: Connecter un appareil
univers: jeu
connexes: [connecter-un-appareil, clavier-et-raccourcis, profils-et-qualite, mettre-a-jour]
---

Copiez l'image sur une carte microSD et branchez le Pi à la télé : il démarre directement sur KyberNow Link. Si vous utilisez déjà Raspberry Pi OS, l'app seule s'y installe aussi.

## Avant de commencer

- Un Raspberry Pi 3, 4 ou 5 (Pi 400, Pi 500 et Compute Module compris).
- Pour KyberNow Link OS, une carte microSD de 8 Go au moins, ou un SSD.
- Une manette, un clavier, ou la télécommande du téléviseur.

Les Pi 4 et 5 décodent l'image avec leur puce, en HEVC, jusqu'à la 4K à 60 Hz ; le Pi 3, en H.264. En HDMI, comptez la 4K à 60 Hz au plus, ou le 1080p à 120 Hz si le téléviseur le permet, sans fréquence variable.

## KyberNow Link OS, sur la carte microSD

Le Pi ne sert plus qu'à KyberNow et démarre comme une console : au téléviseur avec une manette, ou au bureau avec un clavier et une souris.

1. Téléchargez [KyberNow-Link-OS.img.xz](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-OS.img.xz).
2. Ouvrez Raspberry Pi Imager, l'outil officiel pour préparer les cartes, puis Choisir l'OS › Utiliser une image personnalisée, et choisissez le fichier. Ne personnalisez rien dans Imager : tout se règle au premier démarrage.
3. Branchez le Pi au téléviseur en HDMI (sur Pi 4 et 5, la prise HDMI 0, la plus proche de l'alimentation), à la box par un câble Ethernet si possible, puis à l'alimentation. Le symbole de KyberNow s'affiche, puis s'allume avec un son.

> Bon à savoir : vous pouvez aussi coller `https://www.kybernow.app/updates/rpi-imager.json` dans Imager › Réglages › Dépôt personnalisé. « KyberNow Link OS » apparaît alors dans la liste des systèmes.

## Le premier démarrage

1. Mettez votre manette en mode association : sur une manette Xbox, le bouton d'association près du port USB ; sur une DualSense, Create + PS ; sur une Switch Pro, SYNC. Le Pi l'accepte sans code. Une manette USB ou un clavier marchent aussi, comme la télécommande du téléviseur si celui-ci gère HDMI-CEC, la commande des appareils branchés en HDMI.
2. Choisissez la langue.
3. Choisissez le réseau. Ethernet est reconnu tout seul ; sinon, choisissez le Wi-Fi et tapez son mot de passe.
4. Choisissez l'usage : Salon ou Bureau.
5. Choisissez votre ordinateur. Un code à six chiffres s'affiche. Acceptez-le dans Admin, espace Appareils Link (sur un PC, aussi dans le panneau de l'icône près de l'horloge).

## Au quotidien

- ☰ sur l'accueil ouvre les Réglages de l'appareil : réseau, manettes, écran et HDMI-CEC, son, mise à jour, langue et réglages avancés.
- Start tenu trois secondes passe de Salon à Bureau, et inversement.
- Guide (ou PS) met le Pi en veille, et le téléviseur avec lui par HDMI-CEC. Une touche le réveille.
- En session, L1 + R1 + Start + Select ouvrent le menu de session.

## Mettre à jour KyberNow Link OS

Ouvrez Réglages de l'appareil › Mise à jour. La nouvelle version se télécharge, dès sa sortie si « Automatique » est coché. La carte a deux emplacements pour le système : « Redémarrer sur la X » démarre la nouvelle version dans celui qui est libre. Si elle n'arrive pas jusqu'à l'accueil, le Pi revient tout seul à la version d'avant. Vos ordinateurs, vos réglages, le Wi-Fi et les manettes associées restent.

Cette mise à jour demande un Pi 4 ou 5. Sur un Pi 3, copiez la nouvelle image sur la carte.

## L'app seule, sur Raspberry Pi OS

1. Téléchargez [KyberNow-Link-RaspberryPi-arm64.deb](https://github.com/MonsieurNours/kybernow/releases/latest/download/KyberNow-Link-RaspberryPi-arm64.deb), pour Raspberry Pi OS 64 bits (Debian 13).
2. Installez-le d'un double clic dans le gestionnaire de fichiers, ou dans un terminal :

   ```sh
   sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb
   ```

3. KyberNow Link apparaît dans le menu, rubrique Jeux. Ajoutez votre ordinateur : il apparaît tout seul sur le même réseau, et vous acceptez le code dans Admin.
4. Au téléviseur, à la manette, choisissez Réglages de l'app › Interface › Salon pour l'interface lisible à trois mètres : A valide, B revient, Y ouvre les réglages, X ajoute un ordinateur.

Quand une nouvelle version sort, l'app vous le signale : installez le nouveau fichier de la même façon.

> Bon à savoir : sur un Pi 5, l'app lit aussi PyroWave, un format d'image qui se calcule en un quart de milliseconde au plus, au prix d'un débit de 200 Mbit/s et plus : il demande un câble Ethernet. Voir [Profils, format vidéo, débit et HDR](/guide/profils-et-qualite/).

## Si ça ne marche pas

- **Rien ne s'affiche sur la télé.** Sur Pi 4 et 5, branchez le câble HDMI sur la prise HDMI 0, la plus proche de l'alimentation.
- **La manette n'est pas reconnue.** Remettez-la en mode association, ou branchez-la en USB.
- **La télécommande du téléviseur ne fait rien.** Activez HDMI-CEC dans les réglages du téléviseur (chaque marque lui donne son nom).
- **Le Pi revient à la version d'avant après une mise à jour.** La nouvelle version n'a pas atteint l'accueil. Réglages de l'appareil › Mise à jour le dit ; réessayez à la version suivante.
- **L'ordinateur n'apparaît pas.** Il doit être allumé, sur le même réseau, avec KyberNow Station en marche.
