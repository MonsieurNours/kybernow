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

**KDE Plasma** : à faire. KWin crée des écrans virtuels pour un programme qui y a droit (`zkde_screencast_unstable_v1`, `stream_virtual_output`) ; la Station devra s'y déclarer. D'ici là, sous KDE, Admin trouve l'agent mais Mutter n'est pas là : pas d'écran virtuel.

### Clavier, souris, pointeur

kynputserver injecte le clavier, la souris et les manettes par `/dev/uinput`, sous X11 comme sous Wayland. Le paquet pose une règle udev (`60-kybernow-uinput.rules`, `uaccess`) : l'utilisateur assis devant l'ordinateur y a accès, sans être root.

**Pointeur absolu sous Wayland** (correctif Kyber Linux 0002) : sous Wayland, aucun programme ne peut placer le pointeur (XTest ne déplace que celui de Xwayland). kynput branche donc un second appareil uinput, « KyberNow pointeur », à deux axes absolus et aux boutons d'une souris : udev le classe en souris absolue (comme la tablette d'une machine virtuelle) et le compositeur l'étend sur tout son bureau. Ses axes couvrent la boîte autour des écrans que la Station annonce (places écrites par l'agent). Tant qu'il tient le pointeur, les clics et la molette passent par lui (GNOME n'applique pas les clics d'une autre souris dans ce cas) ; un mouvement relatif (pointeur capturé, jeux) rend la main à la souris relative. Sous X11, rien ne change (XTest).

### Son

Admin (`audio/mod.rs`, module `linux`) crée au début de la session une sortie « KyberNow » (`module-null-sink` de PipeWire, par `pactl`) au nombre de voies de l'appareil (2, 6 ou 8, ordre de PulseAudio), en fait la sortie par défaut et y déplace les sons en cours : rien ne sort des haut-parleurs de l'ordinateur. Kyber capture le retour de la sortie par défaut. À la fin : sortie d'origine rendue, sons ramenés, module retiré (et retiré au démarrage s'il restait d'une session interrompue).

### Encodeur

L'installation choisit celui de la carte (`kybernow-station-setup`) : NVIDIA → NVENC ; AMD ou Intel avec un nœud de rendu → VA-API ; sinon x264. Admin propose ces trois-là dans Réglages › Image (« VA-API — carte AMD ou Intel »).

### Services et paquet

- `kybernow-admin.service` et `kybernow-kyber.service`, services systemd de l'utilisateur (`/usr/lib/systemd/user`), rattachés à sa session graphique : ils démarrent à l'ouverture de session et s'arrêtent avec elle. Admin pilote Kyber par `systemctl --user` (`linux.rs`), et redémarre la Station par systemd.
- Premier démarrage : `kybernow-station-setup` (avant Admin) pose les réglages de Kyber pour Linux (moteur libavconv, capture GStreamer, fichier des écrans), un certificat propre à la Station, l'encodeur de la carte, le premier compte (votre nom d'utilisateur, mot de passe tiré au hasard dans `~/.local/share/kybernow/station/admin/mot-de-passe`).
- Données : `~/.local/share/kybernow/station/` (`admin/`, `kyber/`, `kyber-screens.toml`).
- Paquet : `server/linux/scripts/package.sh` → `kybernow-station_<version>_<arch>.deb` et `kybernow-station-<version>.<arch>.rpm` (`/usr/lib/kybernow-station`, services, règle udev). À l'installation, les services sont activés pour tous les utilisateurs (`systemctl --global enable`) et la règle udev est chargée.

## Correctifs Kyber propres à Linux

Rangés dans `server/linux/kyber-patches`, posés après les correctifs communs par `server/linux/scripts/build-kyber.sh` (détail dans [kyber-changes.md](kyber-changes.md)) :

| Correctif | Ce qu'il change |
|---|---|
| `0001-kyavservice-ecrans-suivis-linux` | Fichier des sources GStreamer (`[kyavserver.gstreamer] sources_file`) relu à chaque liste et à chaque image, surveillé pour annoncer les changements ; identifiant et place de chaque source |
| `0002-kynput-pointeur-wayland` | Pointeur absolu par uinput sous Wayland, clics et molette par lui tant qu'il tient le pointeur |
| `0003-libavgrab-gstreamer-1-24` | Compile avec GStreamer 1.24 (Ubuntu 24.04), qui n'a pas `gst_message_get_details` |

## Compiler

Sur un Linux de l'architecture voulue (la VM « KyberNow Linux » pour arm64 ; x86_64 sur un PC ou dans la même VM par Rosetta) :

```sh
# Outils (Ubuntu 24.04 ; noms voisins sur Debian et Fedora)
sudo apt install build-essential cmake nasm bison flex pkg-config pipx lua5.4 python3-jinja2 \
  glslang-tools spirv-tools libclang-dev libssl-dev libv4l-dev libcap-dev libvdpau-dev libnuma-dev \
  libva-dev libdrm-dev libgbm-dev libegl-dev libvulkan-dev libplacebo-dev libshaderc-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpipewire-0.3-dev libpulse-dev libasound2-dev \
  libxcb1-dev libxcb-shm0-dev libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shape0-dev libxtst-dev \
  libxkbcommon-dev libwayland-dev wayland-protocols libevdev-dev libudev-dev libinput-dev
pipx install meson==1.11.2 && pipx inject meson jinja2
cargo install cargo-c --version 0.10.15 && rustup toolchain install 1.89.0

# Sources de Kyber à part (comme pour le Mac), puis Kyber avec nos correctifs
KYBER_DIR=<copie de kyber-desktop> server/linux/scripts/build-kyber.sh
# Station d'essai dans la session (services systemd de l'utilisateur)
KYBER_DIR=<copie> server/linux/scripts/run-station.sh start
# Paquets .deb et .rpm
KYBER_DIR=<copie> server/linux/scripts/package.sh
```

Pièges rencontrés : `luac` du sous-projet Lua de Kyber ne se lie pas sous Linux (il est compilé seulement si aucun `luac` n'est installé : d'où `lua5.4`) ; libplacebo demande jinja2 dans le Python de meson (`pipx inject`) ; les scripts de Kyber nomment le dossier `x86_64-linux-gnu` en dur (`build-kyber.sh` le remplace par celui de la machine).

## Ce que voient les apps KyberNow Link

Rien de propre à Linux ([link-ui.md](link-ui.md), « Face à une Station Linux ») : un PC, dont le système est le nom de la distribution. Ce qui n'est pas encore là (HDR, PyroWave, stylet, prêt) n'est pas annoncé, et les apps font comme avec une Station qui ne l'a pas.

## Découpage

1. **Kyber sous Linux.** Compilé avec nos correctifs pour x86_64 et arm64 (`build-kyber.sh`), sans le lecteur VLC. **Fait dans la VM arm64 (25/11/2026).**
2. **Admin sous Linux.** Service systemd, Station reconnue, redémarrage, encodeurs : **écrit**. Réveil par le réseau (ethtool), aperçu de l'accueil, nom de la carte graphique, app au début de la session (fichiers `.desktop`, Steam) : **à faire**.
3. **Écrans virtuels.** GNOME par Mutter : **écrit et essayé dans la VM** (voir « Essais »). KDE : à faire.
4. **Son.** Sortie PipeWire KyberNow : **écrit**.
5. **Entrées.** uinput, règle udev, pointeur absolu sous Wayland : **écrit**. Manettes DualSense par uhid, stylet (tablette uinput, à la suite du correctif commun 0027), presse-papiers sous Wayland : à faire.
6. **Paquet, mises à jour.** `.deb`, `.rpm`, services : **écrit**. Icône près de l'horloge, applicateur des mises à jour (service root), mise à jour depuis Admin : à faire.
7. **Fonctions avancées.** Share et relais (rien de propre au système dans Admin : à essayer), Prêt du PC (mode console dans la session de l'hôte, veille empêchée par `systemd-inhibit`), HDR (écran virtuel de GNOME en BT.2100 : GNOME ne le propose pas encore), PyroWave (Vulkan, import DMA-BUF), débit en direct en VA-API (à ajouter au correctif commun 0026) : à faire.
8. **KDE Plasma 6, systèmes immuables.** À faire.

## Essais

Tout dans la VM Parallels « KyberNow Linux » (Ubuntu 24.04 arm64, GNOME 46 sous Wayland, carte graphique virtuelle virgl, sans VA-API ni NVENC), voir [apps/linux/README.md](../apps/linux/README.md).

**Essayé (25/11/2026)** :

- **Compilation** de Kyber pour arm64 avec les correctifs communs (0001 à 0027) et les trois correctifs Linux ; d'Admin et de l'agent des écrans ; tests d'Admin au vert sous Linux (322) et sur le Mac (326).
- **Station d'essai** (`run-station.sh start`) : Kyber, Admin et l'agent démarrent en services systemd de l'utilisateur ; Admin s'annonce par Bonjour (`kybernow-linux`, `kyber=1`) ; `GET /api/stream/info` donne « Ubuntu 24.04.5 LTS », l'écran virtuel (4 écrans au plus), la sortie son, les manettes, ni HDR ni PyroWave ni prêt. Kyber trouve l'écran de la VM par le fichier des écrans 250 ms après son lancement (correctif 0001).
- **Session sans app**, par l'API d'Admin sur `localhost` avec le compte d'essai (`prepare`, signal de vie, `end`) et un lecteur GStreamer branché comme la capture de Kyber : écran virtuel 1920 × 1080 créé, apparu dans GNOME (« Meta-0 »), seul sur le bureau et principal, l'écran de la VM retiré ; son vers la sortie « KyberNow » en 2 puis 6 voies, sortie par défaut ; à la fin, écran de la VM, disposition et sortie d'origine rendus, sortie KyberNow retirée. **Deux écrans** : 2560 × 1440 à l'échelle 2 en (0, 0) et 1920 × 1080 en (2560, 0), les deux flux reçoivent leurs images, mêmes places dans le fichier de Kyber.
- **Kyber** sur l'écran virtuel (flux RTP de Kyber, en mode un seul client le temps de l'essai) : kyavserver ouvre la source GStreamer « KyberNow », l'encodeur x264 et son contexte RTP ; kycontroller refuse ensuite ce flux de test faute de canal de contrôle, comme prévu sans app.
- **Paquets** : `.deb` (48 Mo) installé par apt : première initialisation, services démarrés, agent pris dans `/usr/lib/kybernow-station`, `/dev/uinput` ouvert à l'utilisateur par la règle udev ; puis désinstallé. `.rpm` construit (79 Mo), pas installé (pas de Fedora sous la main).

Pièges trouvés en route : l'app choisit son écran dans la liste de Kyber dès la réponse de `prepare`, avant que Kyber ait relu le fichier des écrans (« Unknown GStreamer display id 1 », première session depuis le Mac) : sur une Station Linux, `prepare` attend que Kyber liste les écrans créés (2 s au plus, 266 ms mesurés) ; `pipewiresrc path=<n>` attend le numéro de série du nœud (le premier essai tombait sur un autre nœud, « not-negotiated », ou sur rien) ; Admin arrête une session sans signal de vie de l'app (5 s) ; la disposition d'avant, remise juste après le retrait d'un écran virtuel, était refusée (« stale information ») : l'agent réessaie avec l'état suivant ; GNOME de la VM est en disposition « physique ».

**Pas encore essayé** : une session de bout en bout depuis une app KyberNow Link (image, son, latence), le clavier, le pointeur absolu et les clics sous Wayland, les manettes, VA-API et NVENC (aucune vraie carte graphique dans la VM), x86_64, KDE, Fedora, une machine réelle.
