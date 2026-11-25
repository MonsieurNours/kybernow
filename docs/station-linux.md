# KyberNow Station pour Linux — cadrage

[English](station-linux.en.md) · Français

Cadré le 25/11/2026. Ce document fixe les choix, le découpage et l'état des jalons, comme [station-macos.md](station-macos.md) pour le Mac.

## Le but

Un ordinateur sous Linux devient une Station comme un PC Windows ou un Mac : mêmes fonctions, même Admin, et les apps KyberNow Link s'y connectent sans savoir (presque) qu'il s'agit d'un Linux. Pour le jeu (Steam, Proton, Lutris, Heroic) comme pour le bureau à distance, avec les profils Jeu, Bureau et Max.

## Choix (validés par l'auteur le 25/11/2026)

| Sujet | Choix |
|---|---|
| Bureaux | GNOME et KDE Plasma 6 sous Wayland, au même niveau. Sous X11 et les bureaux wlroots (Sway, Hyprland) : les écrans tels qu'ils sont, sans écran virtuel |
| Distributions | Ubuntu 24.04, Debian 13, Fedora 42 et plus récentes. Systèmes immuables (Bazzite, SteamOS) dans un second temps |
| Paquets | Un `.deb` et un `.rpm` tirés du même assemblage, avec la règle udev et les mises à jour signées. Pas de Flatpak (uinput, services et capture sans bac à sable n'y passent pas) |
| Architectures | x86_64 et arm64 |
| Cartes et encodeurs | NVENC (NVIDIA), VA-API (AMD, Intel), x264 en secours, choisis selon la carte à l'installation ; images sans copie vers l'encodeur (DMA-BUF) ; PyroWave en Vulkan |
| Entrées | uinput (X11 et Wayland), pas libei ; manettes Xbox par uinput, DualSense par uhid |
| Verrouillage | Comme sur Mac : l'écran verrouillé de la session est diffusé ; après un redémarrage, il faut que la session s'ouvre toute seule (ouverture automatique) |
| Services | Kyber, Admin (et plus tard l'icône) en services systemd de l'utilisateur, dans sa session graphique ; un service root à part pour appliquer les mises à jour |
| Son | Une sortie PipeWire « KyberNow » de 2, 6 ou 8 voies, choisie pendant la session, puis la sortie d'origine rendue (comme la sortie KyberNow du Mac) |
| Moteur | Le serveur Linux de Kyber 0.29 (kycontroller, kyavserver avec le moteur libavconv, kynputserver), avec nos correctifs communs et des correctifs propres à Linux. Pas de moteur à nous |

## Ce qui existe déjà

- **Kyber sait servir depuis Linux** : capture par NvFBC (son choix par défaut, NVIDIA seulement), DRM/KMS, X11, wlroots et GStreamer (moteur libavconv) ; encodage NVENC, VA-API, x264 ; clavier, souris et manettes par uinput (`kynput/src/linux`). Ce que nous ajoutons, c'est ce qui fait une Station : Admin, écrans virtuels, son, paquet.
- **Admin tourne déjà hors de Windows** (Mac de développement et Station Mac) : les modules propres au système sont derrière des `cfg`, avec une version simulée ; la Station Linux y ajoute ses versions réelles, comme le Mac.
- **Les apps lisent déjà ce que la Station sait faire** (`GET /api/stream/info` : `os`, `hdr`, `pyrowave`, `gamepads`, écran virtuel et son) : une Station Linux s'y décrit, et les apps s'adaptent sans changement ([link-ui.md](link-ui.md), « Face à une Station Linux »).

## Comment c'est fait

### Écrans (GNOME)

GNOME ne laisse aucun programme lire l'écran directement : tout passe par Mutter, son compositeur. La Station a son propre agent, `kybernow-display` (`server/linux/display-agent`, Rust), lancé par Admin dès son démarrage, qui parle le même protocole que l'agent du Mac (lignes JSON : `create`, `remove`, `isolate`, `restore`, `state`, `set_mode`, `displays`…). Admin le pilote par `display/linux.rs`.

- **Écrans virtuels** : `org.gnome.Mutter.ScreenCast`, `RecordVirtual`, une session de capture par écran (Mutter ne démarre pas un flux ajouté après `Start`), pointeur hors de l'image (chaque appareil dessine le sien). Mutter crée l'écran au branchement du premier lecteur sur le flux PipeWire, à la taille que ce lecteur demande : c'est la capture de Kyber qui fixe la définition de l'appareil. Écran « de plateforme » (`is-platform`) : GNOME y met sa barre et ses fenêtres comme sur un vrai écran.
- **Écrans de l'ordinateur** : la même API, `RecordMonitor`, sans le dialogue du partage d'écran (la Station n'est pas dans un bac à sable). Capturés seulement quand il n'y a pas d'écran virtuel (partage depuis le PC, session sans écran virtuel).
- **Disposition** : `org.gnome.Mutter.DisplayConfig`, `ApplyMonitorsConfig` en mode temporaire (jamais enregistré). Pendant la session, seuls les écrans virtuels restent sur le bureau, le principal en premier et principal, comme sous Windows et sur Mac ; à l'échelle 2 dès 2 048 pixels de large (comme le Retina du Mac). Les places suivent le mode de disposition de GNOME : en pixels en disposition « physique » (`layout-mode` 2), ramenées à l'échelle en disposition « logique ». Les écrans virtuels n'apparaissent qu'avec le premier flux : la disposition attend qu'ils soient là (un fil de l'agent, toutes les 250 ms). À la fin, la disposition d'avant est remise. Prêt de l'ordinateur : les écrans de l'ordinateur restent, à droite.
- **Ce que Kyber lit** : un fichier TOML de sources GStreamer (`~/.local/share/kybernow/station/kyber-screens.toml`), tenu à jour par l'agent : identifiant de l'écran, flux PipeWire (`pipewiresrc target-object=<numéro de série>` : le numéro de série du nœud, lu par `pw-dump`, et non l'identifiant que donne Mutter, que PipeWire réutilise), taille, place sur le bureau. Kyber le relit à chaque liste d'écrans et à chaque image, et le surveille pour annoncer ses changements (correctif Kyber Linux 0001) : un écran virtuel créé, retiré ou redimensionné apparaît sans redémarrer Kyber.
- **Identifiants** : `linux:101` pour le premier écran virtuel (101, 102… pour Kyber), `linux:1`, `linux:2`… pour ceux de l'ordinateur.
- **Filet de sécurité** : les écrans virtuels vivent avec la connexion de l'agent au bus de la session ; si l'agent ou Admin s'arrête, GNOME les retire et revient à sa disposition enregistrée.

### Écrans (KDE Plasma)

L'agent choisit son bureau au démarrage : GNOME si Mutter est sur le bus de la session, sinon KDE si KWin lui donne son protocole de capture (`KYBERNOW_DESKTOP=gnome|kde` pour forcer). Sous KDE (`kde.rs`) :

- **Écrans virtuels** : protocole Wayland de KWin `zkde_screencast_unstable_v1` (version 2 et après, Plasma 5.25 et après), `stream_virtual_output` : KWin crée l'écran « KyberNow-1 » (« KyberNow-2 »… ; Plasma 6 l'appelle « Virtual-KyberNow-1 ») à la taille donnée (en points, avec l'échelle : 2 dès 2 048 pixels de large) et son flux PipeWire arrive avec l'événement `created`. KWin fixe la taille à la création : un changement de définition en session refait l'écran.
- **Écrans de l'ordinateur** : `stream_output` sur leur `wl_output` (nommé par la version 4 de `wl_output`), sans le dialogue du portail.
- **Droit au protocole** : KWin ne le donne qu'à un programme déclaré dans un fichier `.desktop` dont `Exec` est l'agent et qui nomme l'interface (`X-KDE-Wayland-Interfaces=zkde_screencast_unstable_v1`). Le paquet pose `/usr/share/applications/fr.leslignesclaires.kybernow.station.display.desktop` ; la Station d'essai, sa copie dans `~/.local/share/applications`. Sans lui, l'agent le dit dans le journal d'Admin.
- **Disposition** : `kscreen-doctor` (activer, placer, échelle, priorité ; les écrans absents de la disposition sont désactivés) ; places en points. La liste est lue dans sa sortie texte (`-o`) : `kscreen-doctor -j` plante sous Plasma 5.27.
- **Écran éteint puis rallumé** : KWin le retire, puis l'annonce sous un nouvel objet `wl_output` ; l'agent suit ces retraits et réessaie la capture un instant à la fin d'une session.
- **Filet de sécurité** : les écrans virtuels vivent avec la connexion Wayland de l'agent ; s'il s'arrête, KWin les retire.

### Clavier, souris, pointeur

kynputserver injecte le clavier, la souris et les manettes par `/dev/uinput`, sous X11 comme sous Wayland. Le paquet pose une règle udev (`60-kybernow-uinput.rules`, `uaccess`) : l'utilisateur assis devant l'ordinateur y a accès, sans être root.

**Pointeur absolu sous Wayland** (correctif Kyber Linux 0002) : sous Wayland, aucun programme ne peut placer le pointeur (XTest ne déplace que celui de Xwayland). kynput branche donc un second appareil uinput, « KyberNow pointeur », à deux axes absolus et aux boutons d'une souris : udev le classe en souris absolue (comme la tablette d'une machine virtuelle) et le compositeur l'étend sur tout son bureau. Ses axes couvrent la boîte autour des écrans que la Station annonce (places écrites par l'agent). Tant qu'il tient le pointeur, les clics et la molette passent par lui (GNOME n'applique pas les clics d'une autre souris dans ce cas) ; un mouvement relatif (pointeur capturé, jeux) rend la main à la souris relative. Sous X11, rien ne change (XTest).

### Son

Admin (`audio/mod.rs`, module `linux`) crée au début de la session une sortie « KyberNow » (`module-null-sink` de PipeWire, par `pactl`) au nombre de voies de l'appareil (2, 6 ou 8, ordre de PulseAudio), en fait la sortie par défaut et y déplace les sons en cours : rien ne sort des haut-parleurs de l'ordinateur. Kyber capture le retour de la sortie par défaut. À la fin : sortie d'origine rendue, sons ramenés, module retiré (et retiré au démarrage s'il restait d'une session interrompue).

### Encodeur

L'installation choisit celui de la carte (`kybernow-station-setup`) : NVIDIA → NVENC ; AMD ou Intel avec un nœud de rendu → VA-API ; sinon x264. Admin propose ces trois-là dans Réglages › Image (« VA-API — carte AMD ou Intel »).

### Textes d'Admin

Admin donne `platform` : `linux` (`GET /api/session`) : l'interface et Admin remplacent chaque texte propre à Windows (menu Démarrer, pilote SudoVDA, haut-parleurs de Steam, session Windows…) par sa version Linux, rangée sous `linux` dans les catalogues (`server/web/src/lib/i18n/`, `server/admin/src/i18n/`), et les textes des réglages de Kyber par `schema.linux.json` ([i18n.md](i18n.md)). Trouvé à la première session depuis le Mac (25/11/2026) : Admin parlait de Windows.

### Services et paquet

- `kybernow-admin.service` et `kybernow-kyber.service`, services systemd de l'utilisateur (`/usr/lib/systemd/user`), rattachés à sa session graphique : ils démarrent à l'ouverture de session et s'arrêtent avec elle. Admin pilote Kyber par `systemctl --user` (`linux.rs`), et redémarre la Station par systemd.
- Premier démarrage : `kybernow-station-setup` (avant Admin) pose les réglages de Kyber pour Linux (moteur libavconv, capture GStreamer, fichier des écrans), un certificat propre à la Station, l'encodeur de la carte, le premier compte (votre nom d'utilisateur, mot de passe tiré au hasard dans `~/.local/share/kybernow/station/admin/mot-de-passe`).
- Données : `~/.local/share/kybernow/station/` (`admin/`, `kyber/`, `kyber-screens.toml`).
- Paquet : `server/linux/scripts/package.sh` → `kybernow-station_<version>_<arch>.deb` et `kybernow-station-<version>.<arch>.rpm` (`/usr/lib/kybernow-station`, services, règle udev). À l'installation, les services sont activés pour tous les utilisateurs (`systemctl --global enable`) et la règle udev est chargée.

### Systèmes immuables (Bazzite, Fedora Atomic, SteamOS)

Sur Bazzite, les Fedora Atomic (Silverblue, Kinoite…) et SteamOS, `/usr` est en lecture seule et remplacé d'un bloc à chaque mise à jour du système : le `.deb` et le `.rpm` n'y entrent pas tels quels. Trois voies étudiées (jalon 8, 25/11/2026) :

| Voie | Bazzite, Fedora Atomic | SteamOS (mode Bureau) | Verdict |
|---|---|---|---|
| **rpm en couche** (`rpm-ostree install ./kybernow-station-….rpm`) | Possible avec le `.rpm` tel quel, pour tous les comptes ; redémarrage, chaque mise à jour du système recompose l'image (plus lente, et bloquée si une dépendance manque) ; Bazzite déconseille les couches. La Station ne peut pas se mettre à jour seule : `rpm -U` n'existe pas, il faut `rpm-ostree` (root, redémarrage) | Impossible : pacman est fermé, et ce qu'il installe est effacé à la mise à jour suivante | Toléré sur Fedora Atomic, pas proposé |
| **Extension systemd-sysext** (image posée dans `/var/lib/extensions`, fusionnée sur `/usr`) | Possible (root, `extension-release` en `ID=_any`), refaite à chaque mise à jour de la Station | Possible en principe, mais root à chaque mise à jour, service `systemd-sysext` à activer, et `/var` est propre à chaque partition A/B : rien ne garantit que l'image suive une mise à jour de SteamOS | Non : root à chaque fois, et fragile sur SteamOS |
| **Dossier de l'utilisateur** (`~/.local/lib/kybernow-station`) | Oui | Oui | **Retenue** |

Pourquoi le dossier de l'utilisateur : tout ce dont la Station a besoin est déjà propre à l'utilisateur (services systemd `--user`, données dans `~/.local/share/kybernow/station`, son, écrans), le dossier personnel survit à toutes les mises à jour du système, et la mise à jour de la Station se fait sans mot de passe, comme sur le Mac. Ce qui reste, sans root ou avec :

| Pièce | Sans root ? | Où |
|---|---|---|
| Kyber, Admin, l'agent des écrans | Oui | `~/.local/lib/kybernow-station/versions/<version>`, lien `current` (et `previous`) |
| Services `--user` | Oui | `~/.config/systemd/user/kybernow-*.service` : ceux du paquet, `/usr/lib/kybernow-station` réécrit vers `…/current` (une mise à jour ne change que le lien) |
| Droit de l'agent au protocole d'écrans de KWin | Oui | `~/.local/share/applications/fr.leslignesclaires.kybernow.station.display.desktop` (KWin lit les dossiers XDG de l'utilisateur, comme pour la Station d'essai) ; `Exec` est le chemin réel de l'agent, réécrit à chaque changement de version, parce que KWin le compare au programme qui se connecte |
| Son (sortie PipeWire), écrans virtuels (Mutter, KWin), certificat, comptes | Oui | Rien de changé |
| `/dev/uinput` (clavier, souris, manettes) | Souvent déjà ouvert | Les règles udev de Steam (`steam-devices`, présentes sur SteamOS et Bazzite) donnent déjà `uaccess` à `/dev/uinput`. Sinon, la règle de la Station dans `/etc/udev/rules.d` et `uinput` dans `/etc/modules-load.d`, avec sudo, une fois : `/etc` est modifiable et conservé d'une mise à jour à l'autre sur les deux systèmes. Sur SteamOS, il faut d'abord un mot de passe (`passwd`) |
| Greffon PipeWire de GStreamer | Selon le système | Celui du système s'il existe ; sinon celui de la machine qui a compilé, joint à l'archive (`runtime/gst-fallback`, pris par `GST_PLUGIN_PATH` dans un complément du service de Kyber). Sur Fedora Atomic, `rpm-ostree install pipewire-gstreamer` reste possible |
| Démarrage seul après un redémarrage | Réglage du système | Ouverture automatique de la session, comme ailleurs |

**Archive** : `server/linux/immutable/make-archive.sh` range l'assemblage du paquet (ou un `.deb` déjà fait, lu aussi depuis un Mac) en `KyberNow-Station-Linux-<x86_64|aarch64>.tar.xz` : `install.sh`, `VERSION`, `BUILD` (architecture et glibc de la machine qui a compilé), `bin/` (avec `kybernow-station-home`), `runtime/`, `share/systemd-user`, `share/applications`, `share/udev`. `package.sh` l'appelle après le `.deb`. Les programmes demandent une glibc au moins aussi récente que celle de la machine qui a compilé : l'archive publiée se fait sur Ubuntu 24.04 (glibc 2.39, la plus ancienne des systèmes visés), jamais sur Fedora 45 ; l'installation refuse un système plus ancien en le disant.

**Installation** : `./install.sh` dans le dossier extrait (`bin/kybernow-station-home install`) : contrôle de l'architecture et de la glibc, copie dans `versions/<version>`, lien `current`, services et `.desktop` écrits, programmes manquants signalés (`pactl`, `pw-dump`, GStreamer, `kscreen-doctor` sous KDE), règle udev seulement si `/dev/uinput` (ou `/dev/uhid`, si la règle le nomme) est fermé, services activés et démarrés dans la session graphique. Le premier démarrage est celui du paquet (`kybernow-station-setup`). Si le paquet est aussi installé dans le système, l'installation le signale : les services du dossier de l'utilisateur passent devant.

**Mise à jour** (sans root) : Admin télécharge `KyberNow-Station-Linux-<arch>.tar.xz`, le vérifie contre le manifeste signé, puis lance `kybernow-station-home update <archive> <version> --result <…>/resultat.json --log <…>`. Le script se détache d'Admin (unité `kybernow-station-update` par `systemd-run --user`, copie de lui-même hors des dossiers de version), extrait dans `versions/`, contrôle la version, l'architecture et la glibc, arrête les services, bascule `current`, réécrit services et `.desktop`, relance, et attend qu'Admin réponde dans la nouvelle version (`/api/session`, 60 s). Sinon il remet l'autre version. Résultat au même format que l'applicateur du Mac (`version`, `ok`, `rolled_back`, `undo_failed`, `message`, `log`). Deux versions gardées : « Revenir à la version précédente » est `kybernow-station-home rollback`. Le choix du chemin (dossier de l'utilisateur ou paquet) revient à l'applicateur, selon l'endroit d'où tourne Admin ; sur un système ostree avec la Station en couche dans `/usr`, Admin annonce la nouvelle version sans l'installer.

**Retrait** : `kybernow-station-home uninstall` (réglages, comptes et appairages gardés ; `--purge` pour les retirer). La règle udev, si elle a été posée, se retire avec la commande affichée.

**SteamOS** : la Station tourne dans le mode Bureau (KDE Plasma). En mode Jeu, gamescope n'offre ni écran virtuel ni capture à l'agent : la Station n'y diffuse pas. Pour qu'elle soit là après un redémarrage, SteamOS doit démarrer en mode Bureau (`steamos-session-select plasma-persistent`). Même chose pour les images de Bazzite qui démarrent en mode Jeu (Bazzite Deck) ; les images de bureau de Bazzite (KDE, GNOME) sont comme une Fedora.

**Pas essayé** : aucune VM ni machine Bazzite, Fedora Atomic ou SteamOS. Essayé sur le Mac avec des simulacres de `systemctl` et de `curl` : fabrication de l'archive (depuis un assemblage et depuis un `.deb`), installation, mise à jour réussie, mise à jour refusée par le contrôle de santé puis remise de l'ancienne version, archive d'une autre version refusée, retour à la version précédente, retrait. À vérifier sur une vraie machine : le contrôle d'accès de KWin avec le `.desktop` du dossier de l'utilisateur, `steam-devices` et `/dev/uinput`, le greffon GStreamer de secours, la glibc de SteamOS, `graphical-session.target` en mode Bureau et en mode Jeu.

## Correctifs Kyber propres à Linux

Rangés dans `server/linux/kyber-patches`, posés après les correctifs communs par `server/linux/scripts/build-kyber.sh` (détail dans [kyber-changes.md](kyber-changes.md)) :

| Correctif | Ce qu'il change |
|---|---|
| `0001-kyavservice-ecrans-suivis-linux` | Fichier des sources GStreamer (`[kyavserver.gstreamer] sources_file`) relu à chaque liste et à chaque image, surveillé pour annoncer les changements ; identifiant et place de chaque source |
| `0002-kynput-pointeur-wayland` | Pointeur absolu par uinput sous Wayland, clics et molette par lui tant qu'il tient le pointeur |
| `0003-libavgrab-gstreamer-1-24` | Compile avec GStreamer 1.24 (Ubuntu 24.04), qui n'a pas `gst_message_get_details` |
| `0004-kymedia-luac-5-4` | `luac` 5.4 compilé par Kyber quand le système n'en a pas (Fedora 45 : Lua 5.5), avec les sources de Lua |

## Compiler

Sur un Linux de l'architecture voulue (la VM « KyberNow Linux » pour arm64 ; x86_64 sur un PC ou dans la même VM par Rosetta) :

```sh
# Outils (Ubuntu 24.04 ; noms voisins sur Debian et Fedora)
sudo apt install build-essential cmake nasm bison flex pkg-config pipx lua5.4 python3-jinja2 \
  glslang-tools spirv-tools libclang-dev libssl-dev libv4l-dev libcap-dev libvdpau-dev libnuma-dev \
  libva-dev libdrm-dev libgbm-dev libegl-dev libvulkan-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpipewire-0.3-dev libpulse-dev libasound2-dev \
  libxcb1-dev libxcb-shm0-dev libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shape0-dev libxtst-dev \
  libxkbcommon-dev libwayland-dev wayland-protocols libevdev-dev libudev-dev libinput-dev
# Fedora 42 et après (sans libplacebo-devel : Kyber compile la sienne)
sudo dnf install gcc gcc-c++ cmake nasm bison flex pipx python3-jinja2 glslang spirv-tools clang-devel \
  openssl-devel libv4l-devel libcap-devel libvdpau-devel numactl-devel libva-devel libdrm-devel \
  mesa-libgbm-devel mesa-libEGL-devel vulkan-loader-devel gstreamer1-devel gstreamer1-plugins-base-devel \
  pipewire-devel pulseaudio-libs-devel alsa-lib-devel libxcb-devel libXtst-devel libXrandr-devel libxkbcommon-devel \
  wayland-devel wayland-protocols-devel libevdev-devel systemd-devel libinput-devel dbus-devel rustup \
  libXrandr-devel clang20-devel
pipx install meson==1.11.2 && pipx inject meson jinja2
cargo install cargo-c --version 0.10.15 --locked --features vendored-openssl && rustup toolchain install 1.89.0

# Sources de Kyber à part (comme pour le Mac), puis Kyber avec nos correctifs
KYBER_DIR=<copie de kyber-desktop> server/linux/scripts/build-kyber.sh
# Station d'essai dans la session (services systemd de l'utilisateur)
KYBER_DIR=<copie> server/linux/scripts/run-station.sh start
# Paquets .deb et .rpm
KYBER_DIR=<copie> server/linux/scripts/package.sh
```

Pièges rencontrés :

- `luac` : txproto exige un `luac` 5.4. Ubuntu en a un (`lua5.4`) ; Fedora 45 n'a que Lua 5.5. Le correctif Linux 0004 fait compiler à Kyber son propre `luac` avec les sources de Lua (lié à `liblua.so`, il ne se liait pas) quand aucun `luac5.4` n'est installé.
- Vulkan : sans `libXrandr-devel`, le chargeur Vulkan que compile Kyber ne se configure pas, et txproto, compilé alors sans Vulkan, ne compile pas.
- libplacebo : un `libplacebo-devel` du système (Fedora 45 : 7.360) passe devant celle que compile Kyber (7.351) et txproto ne compile plus ; ne pas l'installer.
- libclang : les liaisons Rust de libavconv (bindgen 0.69) lisent mal les en-têtes avec un libclang 21 ou plus récent (Fedora 45 : clang 23, « AVConvHandle is defined multiple times ») ; `build-kyber.sh` prend celui de LLVM 20, 19 ou 18 s'il est installé (`clang20-devel`).
- Code relogeable : le GCC de Fedora ne produit pas de code PIE par défaut, contrairement à Ubuntu ; les objets C++ de libvpl laissaient des relocations en lecture seule et FFmpeg ne liait plus `libavutil.so`. `build-kyber.sh` passe `-fPIC` partout.
- OpenSSL 4 (Fedora 45) : cargo-c s'installe avec l'OpenSSL qu'il embarque (`--features vendored-openssl`).
- libplacebo demande jinja2 dans le Python de meson (`pipx inject`) ; les scripts de Kyber nomment le dossier `x86_64-linux-gnu` en dur (`build-kyber.sh` le remplace par celui de la machine).

## Ce que voient les apps KyberNow Link

Rien de propre à Linux ([link-ui.md](link-ui.md), « Face à une Station Linux ») : un PC, dont le système est le nom de la distribution. Ce qui n'est pas encore là (HDR, PyroWave, stylet, prêt) n'est pas annoncé, et les apps font comme avec une Station qui ne l'a pas.

## Découpage

1. **Kyber sous Linux.** Compilé avec nos correctifs pour x86_64 et arm64 (`build-kyber.sh`), sans le lecteur VLC. **Fait dans la VM arm64 (25/11/2026).**
2. **Admin sous Linux.** Service systemd, Station reconnue, redémarrage, encodeurs : **écrit**. Réveil par le réseau (ethtool), aperçu de l'accueil, nom de la carte graphique, app au début de la session (fichiers `.desktop`, Steam) : **à faire**.
3. **Écrans virtuels.** GNOME par Mutter et KDE Plasma 6 par KWin : **écrits et essayés dans les VM** (voir « Essais »).
4. **Son.** Sortie PipeWire KyberNow : **écrit**.
5. **Entrées.** uinput, règle udev, pointeur absolu sous Wayland : **écrit**. Manettes DualSense par uhid, stylet (tablette uinput, à la suite du correctif commun 0027), presse-papiers sous Wayland : à faire.
6. **Paquet, mises à jour.** `.deb`, `.rpm`, services : **écrit**. Icône près de l'horloge, applicateur des mises à jour (service root), mise à jour depuis Admin : à faire.
7. **Fonctions avancées.** Share et relais (rien de propre au système dans Admin : à essayer), Prêt du PC (mode console dans la session de l'hôte, veille empêchée par `systemd-inhibit`), HDR (écran virtuel de GNOME en BT.2100 : GNOME ne le propose pas encore), PyroWave (Vulkan, import DMA-BUF), débit en direct en VA-API (à ajouter au correctif commun 0026) : à faire.
8. **Systèmes immuables** (Bazzite, Fedora Atomic, SteamOS). Installation dans le dossier de l'utilisateur, sans root sauf la règle udev quand Steam ne l'a pas déjà, mise à jour sans mot de passe (`server/linux/immutable`) : **écrit**, essayé sur le Mac avec des simulacres, jamais sur ces systèmes (voir « Systèmes immuables »).

## Essais

Tout dans la VM Parallels « KyberNow Linux » (Ubuntu 24.04 arm64, GNOME 46 sous Wayland, carte graphique virtuelle virgl, sans VA-API ni NVENC), voir [apps/linux/README.md](../apps/linux/README.md).

**Essayé (25/11/2026)** :

- **Compilation** de Kyber pour arm64 avec les correctifs communs (0001 à 0027) et les trois correctifs Linux ; d'Admin et de l'agent des écrans ; tests d'Admin au vert sous Linux (322) et sur le Mac (326).
- **Station d'essai** (`run-station.sh start`) : Kyber, Admin et l'agent démarrent en services systemd de l'utilisateur ; Admin s'annonce par Bonjour (`kybernow-linux`, `kyber=1`) ; `GET /api/stream/info` donne « Ubuntu 24.04.5 LTS », l'écran virtuel (4 écrans au plus), la sortie son, les manettes, ni HDR ni PyroWave ni prêt. Kyber trouve l'écran de la VM par le fichier des écrans 250 ms après son lancement (correctif 0001).
- **Session sans app**, par l'API d'Admin sur `localhost` avec le compte d'essai (`prepare`, signal de vie, `end`) et un lecteur GStreamer branché comme la capture de Kyber : écran virtuel 1920 × 1080 créé, apparu dans GNOME (« Meta-0 »), seul sur le bureau et principal, l'écran de la VM retiré ; son vers la sortie « KyberNow » en 2 puis 6 voies, sortie par défaut ; à la fin, écran de la VM, disposition et sortie d'origine rendus, sortie KyberNow retirée. **Deux écrans** : 2560 × 1440 à l'échelle 2 en (0, 0) et 1920 × 1080 en (2560, 0), les deux flux reçoivent leurs images, mêmes places dans le fichier de Kyber.
- **Kyber** sur l'écran virtuel (flux RTP de Kyber, en mode un seul client le temps de l'essai) : kyavserver ouvre la source GStreamer « KyberNow », l'encodeur x264 et son contexte RTP ; kycontroller refuse ensuite ce flux de test faute de canal de contrôle, comme prévu sans app.
- **Paquets** : `.deb` (48 Mo) installé par apt : première initialisation, services démarrés, agent pris dans `/usr/lib/kybernow-station`, `/dev/uinput` ouvert à l'utilisateur par la règle udev ; puis désinstallé. `.rpm` construit (79 Mo), pas installé (pas de Fedora sous la main).

Pièges trouvés en route : l'app choisit son écran dans la liste de Kyber dès la réponse de `prepare`, avant que Kyber ait relu le fichier des écrans (« Unknown GStreamer display id 1 », première session depuis le Mac) : sur une Station Linux, `prepare` attend que Kyber liste les écrans créés (2 s au plus, 266 ms mesurés) ; `pipewiresrc path=<n>` attend le numéro de série du nœud (le premier essai tombait sur un autre nœud, « not-negotiated », ou sur rien) ; Admin arrête une session sans signal de vie de l'app (5 s) ; la disposition d'avant, remise juste après le retrait d'un écran virtuel, était refusée (« stale information ») : l'agent réessaie avec l'état suivant ; GNOME de la VM est en disposition « physique ».

**KDE Plasma 6 (25/11/2026)**, dans une seconde VM Parallels « KyberNow Linux KDE » (Fedora 45 KDE arm64, Plasma et KWin 6.8.1, Wayland, connexion automatique ; installée sans fenêtre par Anaconda et un fichier kickstart, ssh avec la clé de l'autre VM) :

- compilation de Kyber, d'Admin et de l'agent sous Fedora 45 (pièges ci-dessus : Lua 5.5, libclang 23, code relogeable, libplacebo, libXrandr, OpenSSL 4, `lib64`) ;
- Station d'essai : Kyber, Admin et l'agent démarrent, l'agent reconnaît KDE ; le contrôle d'accès de KWin donne le protocole grâce au fichier `.desktop` ;
- session par l'API d'Admin (sans app) : écran virtuel 1920 × 1080 créé par KWin, seul et principal, l'écran de la VM éteint ; deux écrans, 2560 × 1440 à l'échelle 2 en (0, 0) et 1920 × 1080 en (1280, 0) en points ; son vers la sortie « KyberNow » ; flux de l'écran virtuel lu par GStreamer comme le lit Kyber ; à la fin, l'écran de la VM revient et Kyber le capture de nouveau ;
- d'abord, sous un KWin 5.27 sans écran dans la VM Ubuntu : capture et contrôle d'accès ; ce KWin-là ne sait pas créer d'écran virtuel.

Défauts trouvés et corrigés en route : une disposition sans écran virtuel désactivait les écrans sous KDE ; un écran virtuel refusé laissait Kyber sans écran ; le nom « Virtual-KyberNow-1 » de Plasma 6 n'était pas reconnu ; la sortie de `kscreen-doctor` de Plasma 6 (une propriété par ligne) n'était pas lue ; un écran rallumé était cherché sous son ancien objet.

**Pas encore essayé** : une session depuis une app sous KDE, une session de bout en bout depuis une app KyberNow Link (image, son, latence), le clavier, le pointeur absolu et les clics sous Wayland, les manettes, VA-API et NVENC (aucune vraie carte graphique dans la VM), x86_64, Fedora, une machine réelle.
