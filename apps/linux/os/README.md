# KyberNow Link OS : l'image à flasher sur un Raspberry Pi

[English](README.en.md) · Français

Une carte microSD (ou un SSD) qui ne démarre que KyberNow Link : le Raspberry Pi devient une console de streaming (au téléviseur, à la manette) ou un poste de bureau à distance (écran, clavier, souris). Cadrage dans [PROJECT.md](../../../PROJECT.md) (« Client Raspberry Pi ») ; maquettes : https://claude.ai/artifact/6crVeg3QyrA5vs8TRhWRft (rangée « KyberNow Link OS »). Installation : [../INSTALLATION.md](../INSTALLATION.md) (« KyberNow Link OS »).

**État (23/10/2026)** : écrit, l'image se construit dans la VM « KyberNow Linux » ; **rien n'a démarré sur un Raspberry Pi** (aucune carte disponible) : le démarrage, Plymouth, SDL sur KMS/DRM, le son ALSA, le Bluetooth, le CEC, le tryboot A/B et l'agrandissement de la partition de données restent à vérifier sur le matériel.

## Ce que fait l'image

- **Base** : Raspberry Pi OS Lite (Debian 13 « trixie », arm64), construite par [pi-gen](https://github.com/RPi-Distro/pi-gen) (stage0 à stage2) puis notre stage `stage-kybernow` ; pas de bureau, pas de session interactive (compte `kybernow` sans mot de passe ; SSH éteint, par clé seulement quand on l'allume dans Réglages de l'appareil › Avancé).
- **Link seul, plein écran** : `kybernow-link.service` lance l'app (le paquet .deb de `scripts/deb.sh`, FFmpeg de Raspberry Pi, décodeur HEVC V4L2) sur **KMS/DRM** (`SDL_VIDEODRIVER=kmsdrm`, sans compositeur), le son par **ALSA** sur la prise HDMI (`/etc/asound.conf`), `KYBERNOW_OS=1`, les données dans `/var/lib/kybernow` (partition à part). Elle redémarre si elle s'arrête.
- **Démarrage « console »** : firmware sans arc-en-ciel (`disable_splash=1`), noyau silencieux, **Plymouth** dessine l'image fixe du symbole éteint sur l'encre (`mark.png`, rendue par `kybernow --splash` : la première image de l'allumage, aux pixels près), puis le service lui demande de rester affiché (`plymouth quit --retain-splash`) le temps que l'app reprenne la même image et joue l'allumage de la charte **avec son** (`src/device/chime.rs` : crépitements, nappe, accord la majeur 9, carillon, résolution ; synthèse, à remplacer un jour par une bande composée). S'il y a à attendre (réseau, Bluetooth), la couture se remplit sous le nom.
- **Premier démarrage** (`src/ui/setup.rs`, écrans de la maquette) : **Manette** (Bluetooth à l'écoute par `bluetoothctl`, agent sans code, association et connexion de toute manette qui se présente ; USB, télécommande CEC ou clavier marchent aussi ; écran bilingue, anglais puis français), **Langue**, **Réseau** (Ethernet reconnu, sinon la liste des Wi-Fi par `nmcli` et le mot de passe au pavé), **Usage** (Salon ou Bureau : l'interface de Link), **Ordinateur** (l'ajout par code, l'écran habituel).
- **Réglages de l'appareil** (`src/ui/device.rs`, ☰ sur l'accueil) : Usage, Réseau, Manettes (associer, tester), Écran (mode HDMI au prochain démarrage, HDMI-CEC), Son, **Mise à jour**, Langue, Avancé (SSH, redémarrer, éteindre, version). **Start tenu trois secondes** bascule Salon ⇄ Bureau, comme Steam et Big Picture ; le bouton Guide (ou PS) sur l'accueil met en veille (⏻ aussi) : écran éteint, Bluetooth allumé, le téléviseur mis en veille par CEC ; une touche ou un bouton réveille.
- **HDMI-CEC** (`cec-ctl`, v4l-utils) : au lancement l'appareil s'annonce « KyberNow Link » au téléviseur ; la veille et le réveil passent par CEC ; la télécommande conduit l'interface (les touches CEC arrivent au noyau comme un clavier : flèches, OK, retour). Désactivable dans Écran.
- **Mise à jour A/B** : voir plus bas.

## Disposition de la carte (A/B)

`make-ab-image.sh` refait l'image de pi-gen (deux partitions) en six :

| Partition | Rôle |
|---|---|
| p1 `BOOTA` (FAT32, 256 Mo) | démarrage A, et `autoboot.txt` (le firmware le lit toujours sur la partition 1) |
| p2 `rootA` (ext4) | le système A (celui de l'image) : ce que la base occupe, plus 15 % et 128 Mo |
| p3 `BOOTB` (FAT32, 256 Mo) | démarrage B (vide : la première mise à jour l'écrit) |
| p4 | étendue |
| p5 `rootB` (ext4) | le système B (vide) |
| p6 `data` (ext4, 256 Mo) | `/var/lib/kybernow` : ordinateurs, réglages, journal, images de mise à jour ; **agrandie au premier démarrage** jusqu'au bout de la carte (`kybernow-os-firstboot`) |

L'identifiant de disque est fixe (`0x4b79624e`) : les PARTUUID aussi (`4b79624e-01` … `-06`), écrits dans `cmdline.txt` et `/etc/fstab` ; `/etc/kybernow-os/layout` dit la disposition, `/etc/kybernow-os/release` la version.

**autoboot.txt** : `[all] tryboot_a_b=1 boot_partition=1` / `[tryboot] boot_partition=3`. La mise à jour (`kybernow-os-update`, root par sudo) : l'app télécharge `KyberNow-Link-OS.img.xz` de la version publiée (vérifiée par le cœur : manifeste signé, taille, SHA-256) dans la partition de données, puis `apply` écrit les partitions 1 et 2 de l'image (son emplacement A) dans l'emplacement libre **en flux** (`xz -dc | dd`, sans décompresser l'image sur la carte), réécrit `cmdline.txt` et `fstab` du nouvel emplacement avec ses PARTUUID, pose `autoboot.txt` (défaut = en cours, essai = nouveau) et redémarre avec `reboot "0 tryboot"`. Au démarrage suivant, si l'app arrive à l'accueil, elle appelle `commit` : l'essai devient le défaut ; sinon le drapeau tryboot ne dure qu'un démarrage et le firmware revient au défaut tout seul. `rollback` repasse à l'autre emplacement s'il a un système. Le tryboot A/B demande le firmware des **Pi 4, 400, CM4, 5, 500 et CM5** ; un Pi 3 démarre l'emplacement A et ne se met pas à jour en A/B (l'app le dit : réinstaller la carte).

## Construire

Sur un Linux arm64 avec sudo sans mot de passe (la VM « KyberNow Linux », Ubuntu 24.04 ; ou un Pi) : `apt install quilt parted debootstrap zerofree zip dosfstools libarchive-tools libcap2-bin rsync xz-utils bc gpg pigz xxd arch-test` (le `depends` de pi-gen), puis :

```sh
apps/linux/scripts/deb.sh            # le paquet (FFmpeg de Raspberry Pi), dans apps/linux/dist
apps/linux/os/build-image.sh         # → apps/linux/dist/KyberNow-Link-OS.img.xz (long : une demi-heure)

# Depuis le Mac, dans la VM (ce que fait release.sh après les Flatpak et le .deb) :
apps/linux/os/release-image.sh [--check] [DOSSIER]   # + KyberNow-Link-OS.img.xz.info (tailles, empreintes)
apps/linux/os/imager-json.sh VERSION INFO site/updates/rpi-imager.json   # le dépôt pour Raspberry Pi Imager
```

`build-image.sh` clone pi-gen (branche `arm64`) dans `~/kybernow-os/pi-gen`, y copie `stage-kybernow` et le .deb, écrit `config` (hôte `kybernow-link`, compte `kybernow`, locale `fr_FR.UTF-8` par défaut, `STAGE_LIST="stage0 stage1 stage2 stage-kybernow"`), lance `build.sh`, puis `make-ab-image.sh` sur l'image produite, et `xz`. Le dossier de travail est hors du dossier partagé (pi-gen monte des systèmes de fichiers).

```
apps/linux/os/
├── build-image.sh               pi-gen + notre stage + A/B + xz
├── make-ab-image.sh             la disposition A/B et la partition de données (root)
├── release-image.sh             depuis le Mac, dans la VM ; .info pour Imager
├── imager-json.sh               site/updates/rpi-imager.json
└── stage-kybernow/
    ├── prerun.sh, EXPORT_IMAGE
    ├── 00-packages/00-packages  plymouth, bluez, network-manager, alsa, v4l-utils, parted…
    └── 01-link/
        ├── 00-run.sh            fichiers dans le rootfs, config.txt, cmdline.txt
        ├── 01-run-chroot.sh     le .deb, le compte, mark.png, Plymouth, les services
        └── files/
            ├── systemd/         kybernow-link.service, kybernow-os-firstboot.service
            ├── kybernow-os-update   mise à jour A/B (status, apply, commit, rollback)
            ├── kybernow-os-ctl      ssh, reboot, poweroff, standby, cec-name, cec-wake, hdmi, hostname
            ├── kybernow-os-firstboot   partition de données agrandie
            ├── plymouth/        kybernow.plymouth, kybernow.script (mark.png rendue à la construction)
            └── etc/             sudoers, asound.conf, bluetooth (JustWorks)
```

## Côté app

`src/device/os.rs` (`KYBERNOW_OS=1` ou `/etc/kybernow-os/release`) : `kybernow-os-update` et `kybernow-os-ctl` par sudo, `nmcli`, `bluetoothctl` tenu ouvert (`BtListener`), ce qui se prépare au lancement (`readiness`). `src/device/chime.rs` : le son de l'allumage. `src/ui/setup.rs`, `src/ui/device.rs` : les écrans. L'app en OS cherche `KyberNow-Link-OS.img.xz` dans les versions publiées (`jobs::asset`) ; « Automatique » télécharge dès la publication, l'installation attend votre « Redémarrer sur la X » (ou la veille, à venir). Captures : `kybernow --preview` produit `os-00-demarrage-chargement.png`, `os-01-manette.png` … `os-11-reglages-appareil.png` ; `kybernow --splash mark.png 1920 1080` rend l'image de Plymouth.

## À vérifier sur une carte

Dans l'ordre : le démarrage de l'image (firmware, Plymouth, service, SDL kmsdrm comme utilisateur `kybernow`), la continuité Plymouth → app, le son ALSA (stéréo, puis 5.1 PCM), le Bluetooth (association sans code d'une Xbox et d'une DualSense), `nmcli`, le CEC (nom, veille, télécommande en touches), la mise à jour A/B de bout en bout (apply, tryboot, commit, rollback, et un essai qui ne démarre pas), l'agrandissement de la partition de données, le mode HDMI par `cmdline.txt`.
