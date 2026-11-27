# KyberNow Station pour Linux — cadrage

[English](station-linux.en.md) · Français

Cadré le 25/11/2026. Ce document fixe les choix, le découpage et l'état des jalons, comme [station-macos.md](station-macos.md) pour le Mac.

## Le but

Un ordinateur sous Linux devient une Station comme un PC Windows ou un Mac : mêmes fonctions, même Admin, et les apps KyberNow Link s'y connectent sans savoir (presque) qu'il s'agit d'un Linux. Pour le jeu (Steam, Proton, Lutris, Heroic) comme pour le bureau à distance, avec les profils Jeu, Bureau et Max.

## Choix (validés par l'auteur le 25/11/2026)

| Sujet | Choix |
|---|---|
| Bureaux | GNOME et KDE Plasma 6 sous Wayland, au même niveau ; Sway et Hyprland (wlroots) avec leurs écrans sans tête (ajoutés le 26/11). Sous X11 et les autres bureaux : les écrans tels qu'ils sont, sans écran virtuel (X11 n'en crée pas sans prise factice ni EDID forcé par root) |
| Distributions | Ubuntu 24.04, Debian 13, Fedora 42 et plus récentes. Systèmes immuables (Bazzite, SteamOS) dans un second temps |
| Paquets | Un `.deb` et un `.rpm` tirés du même assemblage, avec la règle udev et les mises à jour signées. Pas de Flatpak (uinput, services et capture sans bac à sable n'y passent pas) |
| Architectures | x86_64 et arm64 |
| Cartes et encodeurs | NVENC (NVIDIA), VA-API (AMD, Intel), x264 en secours, choisis selon la carte à l'installation ; images sans copie vers l'encodeur (DMA-BUF) ; PyroWave en Vulkan |
| Entrées | uinput (X11 et Wayland), pas libei ; manettes Xbox par uinput, DualSense par uhid |
| Verrouillage | Comme sur Mac : l'écran verrouillé de la session est diffusé ; après un redémarrage, il faut que la session s'ouvre toute seule (ouverture automatique) |
| Services | Kyber, Admin et l'icône en services systemd de l'utilisateur, dans sa session graphique ; un service root à part pour appliquer les mises à jour |
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
- **Disposition** : `org.gnome.Mutter.DisplayConfig`, `ApplyMonitorsConfig` en mode temporaire (jamais enregistré). Pendant la session, seuls les écrans virtuels restent sur le bureau, le principal en premier et principal, comme sous Windows et sur Mac ; à l'échelle 2 dès 2 048 pixels de large (comme le Retina du Mac). Les places suivent le mode de disposition de GNOME : en pixels en disposition « physique » (`layout-mode` 2), ramenées à l'échelle en disposition « logique ». Les écrans virtuels n'apparaissent qu'avec le premier flux : la disposition attend qu'ils soient là (un fil de l'agent, toutes les 250 ms). Mutter refuse une place négative : un écran de l'appareil à gauche ou au-dessus du principal (fréquent sur Mac) fait glisser toute la disposition pour qu'elle commence en (0, 0) ; avant ce correctif (26/11), GNOME refusait toute la disposition et gardait l'écran de l'ordinateur en principal, avec sa barre. À la fin, la disposition d'avant est remise. Prêt de l'ordinateur : les écrans de l'ordinateur restent, à droite.
- **Ce que Kyber lit** : un fichier TOML de sources GStreamer (`~/.local/share/kybernow/station/kyber-screens.toml`), tenu à jour par l'agent : identifiant de l'écran, flux PipeWire (`pipewiresrc target-object=<numéro de série>` : le numéro de série du nœud, lu par `pw-dump`, et non l'identifiant que donne Mutter, que PipeWire réutilise), taille, place sur le bureau. Kyber le relit à chaque liste d'écrans, au démarrage d'un flux et toutes les 250 ms, et le surveille pour annoncer ses changements (correctif Kyber Linux 0001) : un écran virtuel créé, retiré ou redimensionné apparaît sans redémarrer Kyber.
- **Identifiants** : `linux:101` pour le premier écran virtuel (101, 102… pour Kyber), `linux:1`, `linux:2`… pour ceux de l'ordinateur.
- **Filet de sécurité** : les écrans virtuels vivent avec la connexion de l'agent au bus de la session ; si l'agent ou Admin s'arrête, GNOME les retire et revient à sa disposition enregistrée.

### Écrans (KDE Plasma)

L'agent choisit son bureau au démarrage : GNOME si Mutter est sur le bus de la session, sinon KDE si KWin lui donne son protocole de capture (`KYBERNOW_DESKTOP=gnome|kde|sway|hyprland` pour forcer ; Sway et Hyprland passent devant, d'après `SWAYSOCK`, `HYPRLAND_INSTANCE_SIGNATURE` ou `XDG_CURRENT_DESKTOP`). Sous KDE (`kde.rs`) :

- **Écrans virtuels** : protocole Wayland de KWin `zkde_screencast_unstable_v1` (version 2 et après, Plasma 5.25 et après), `stream_virtual_output` : KWin crée l'écran « KyberNow-1 » (« KyberNow-2 »… ; Plasma 6 l'appelle « Virtual-KyberNow-1 ») à la taille donnée (en points, avec l'échelle : 2 dès 2 048 pixels de large) et son flux PipeWire arrive avec l'événement `created`. KWin fixe la taille à la création : un changement de définition en session refait l'écran.
- **Écrans de l'ordinateur** : `stream_output` sur leur `wl_output` (nommé par la version 4 de `wl_output`), sans le dialogue du portail.
- **Droit au protocole** : KWin ne le donne qu'à un programme déclaré dans un fichier `.desktop` dont `Exec` est l'agent et qui nomme l'interface (`X-KDE-Wayland-Interfaces=zkde_screencast_unstable_v1`). Le paquet pose `/usr/share/applications/fr.leslignesclaires.kybernow.station.display.desktop` ; la Station d'essai, sa copie dans `~/.local/share/applications`. Sans lui, l'agent le dit dans le journal d'Admin.
- **Disposition** : `kscreen-doctor` (activer, placer, échelle, priorité ; les écrans absents de la disposition sont désactivés) ; places en points. La liste est lue dans sa sortie texte (`-o`) : `kscreen-doctor -j` plante sous Plasma 5.27.
- **Écran éteint puis rallumé** : KWin le retire, puis l'annonce sous un nouvel objet `wl_output` ; l'agent suit ces retraits et réessaie la capture un instant à la fin d'une session.
- **Filet de sécurité** : les écrans virtuels vivent avec la connexion Wayland de l'agent ; s'il s'arrête, KWin les retire.

### Écrans (Sway et Hyprland)

Bureaux wlroots (`wlroots.rs`). Ni PipeWire ni portail : Kyber capture lui-même les sorties du compositeur (son fournisseur wayland, `wlr-screencopy`), et le fichier des écrans les nomme (`source = "wayland:HEADLESS-1"`, correctif Kyber Linux 0013) sous les identifiants de la Station (101, 102…). Le réglage de Kyber ne change pas (GStreamer) : seule la source de chaque écran dit comment le capturer.

- **Écrans virtuels** : écrans sans tête du compositeur. Sway : `swaymsg create_output` (sortie HEADLESS-n), mode par `output … mode --custom`, retrait par `output … unplug`. Hyprland : `hyprctl output create headless KyberNow-n`, `hyprctl output remove`, mode par `hl.monitor` en Lua (`hyprctl eval`, Hyprland configuré en Lua, 0.55 et après, où `keyword` répond « unknown request » ; la règle d'un écran y est reprise telle quelle, d'où `disabled=false` explicite), sinon `hyprctl keyword monitor`. Un changement de définition en session change le mode de la sortie.
- **Écrans de l'ordinateur** : listés tels quels (`wayland:DP-1`), capturés par Kyber sans dialogue.
- **Disposition** : Sway, `output … enable pos … scale …` puis `output … disable` pour les autres, en une commande ; Hyprland, un `hyprctl --batch` des mêmes règles (`hl.monitor` ou `keyword monitor`). Places en points ; pas d'écran « principal » sous wlroots : le premier reçoit le focus.
- **Image d'un écran immobile** : la capture demande une image à chaque trame (`copy`, pas `copy_with_damage`), sinon un bureau qui ne bouge pas n'envoie rien et l'appareil relance le flux en boucle ; avec x264, elle se fait en mémoire partagée (pas de détour par la carte).
- **Filet de sécurité** : ces écrans sont au compositeur et ne partent pas avec l'agent. Leurs noms sont gardés (`wlroots-ecrans`, à côté du fichier des écrans) : au démarrage, l'agent retire ceux d'une session interrompue et rallume les écrans de l'ordinateur (`output * enable`, `hyprctl reload`).
- **Démarrage** : les services de la Station suivent `graphical-session.target`, qu'une session Sway ou Hyprland n'atteint qu'avec un lanceur qui la gère (uwsm, sway-systemd de Fedora) ; sinon, dans la configuration du compositeur, d'abord `exec systemctl --user import-environment WAYLAND_DISPLAY SWAYSOCK HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP` (l'agent reconnaît le bureau par ces variables), puis `exec systemctl --user start kybernow-admin kybernow-kyber kybernow-tray`.

### Clavier, souris, pointeur

kynputserver injecte le clavier, la souris et les manettes par `/dev/uinput`, sous X11 comme sous Wayland. Le paquet pose une règle udev (`60-kybernow-uinput.rules`, `uaccess`) : l'utilisateur assis devant l'ordinateur y a accès, sans être root. C'est un choix (le niveau de `steam-devices`) : tout programme de la session active peut alors créer un clavier virtuel et taper, dans un terminal comme dans une demande de mot de passe (STLIN-27). Pour la même raison, la règle n'ouvre pas les claviers et souris de l'ordinateur (`/dev/input/event*`), voir « Prêt de l'ordinateur ».

**Pointeur absolu sous Wayland** (correctif Kyber Linux 0002) : sous Wayland, aucun programme ne peut placer le pointeur (XTest ne déplace que celui de Xwayland). kynput branche donc un second appareil uinput, « KyberNow pointeur », à deux axes absolus et aux boutons d'une souris : udev le classe en souris absolue (comme la tablette d'une machine virtuelle) et le compositeur l'étend sur tout son bureau. Ses axes couvrent la boîte autour des écrans que la Station annonce (places écrites par l'agent). Tant qu'il tient le pointeur, les clics et la molette passent par lui (GNOME n'applique pas les clics d'une autre souris dans ce cas) ; un mouvement relatif (pointeur capturé, jeux) rend la main à la souris relative. Sous X11, rien ne change (XTest).

### Manettes

Correctif Kyber Linux 0005. Comme sous Windows et sur Mac, une manette PlayStation de l'appareil (Sony : DualShock 3 et 4, DualSense, DualSense Edge) devient une DualSense de l'ordinateur, toute autre une manette Xbox 360 ; le réglage « Toutes les manettes en Xbox 360 » d'Admin (`gamepad_force_xbox360`) les fait toutes Xbox.

- **DualSense par uhid** (`kynput/src/linux/uhid_gamepad.rs`) : `/dev/uhid` crée un vrai appareil HID, une DualSense branchée en USB (054C:0CE6, descripteur relevé sur une CFI-ZCT1W, le même que la Station Mac). Le pilote `hid-playstation` de Linux la prend comme une vraie : manette, capteurs et pavé tactile pour evdev, `hidraw` pour SDL et Steam ; les jeux y voient une DualSense (icônes PlayStation). kynput répond aux trois rapports que le pilote lit à l'ouverture (calibrage, appairage, micrologiciel), avec une adresse MAC par manette (le pilote refuse deux manettes de même adresse). Les vibrations reviennent dans les rapports de sortie de la manette (0x02) et partent à l'appareil, sans durée, comme sur les autres Stations. La lumière, les voyants et les gâchettes adaptatives ne reviennent pas : le protocole de Kyber ne porte que la vibration.
- **Xbox 360 par uinput** (déjà dans Kyber, complété) : vibrations « sans fin » de Linux (durée 0, ou 0xFFFF chez SDL) envoyées sans durée ; tous les retours de vibration lus d'un coup (le fil n'en lisait qu'un par réveil et un effet pouvait attendre le suivant) ; une erreur sur une manette n'arrête plus toutes les autres.
- **Sans uhid** (module absent, droits) : la manette PlayStation devient une Xbox 360, avec un avertissement au journal de kynput. Le paquet doit ouvrir `/dev/uhid` à l'utilisateur comme `/dev/uinput` (règle udev `uaccess`, module `uhid` chargé), et les `hidraw` de ces DualSense pour SDL et Steam.

Journal de kynput : « Plug DualSense gamepad 1 », « DualSense 1 started by the kernel », ou « Plug gamepad 1 (device …) » pour une Xbox.

### Stylet

Correctif Kyber Linux 0006, à la suite du correctif commun 0027 ([stylet.md](stylet.md), « Station Linux ») : la Station annonce le stylet (`PenAvailable`) et le reçoit en tablette graphique uinput, « KyberNow stylet » (`BTN_TOOL_PEN`, `BTN_TOOL_RUBBER`, `BTN_TOUCH`, `BTN_STYLUS`, `ABS_PRESSURE`, `ABS_TILT_X/Y`), créée au premier paquet du stylet. udev la classe en tablette, libinput la donne au compositeur (Wayland : `tablet-v2`, X11 : XInput2) : pression, inclinaison, gomme, bouton et survol dans Krita, GIMP ou Inkscape. Elle couvre la boîte des écrans annoncés, comme le pointeur absolu. La rotation ne passe pas (libinput ne la lit que sur certains stylets Wacom).

### Presse-papiers

Correctif Kyber Linux 0007. Sous Wayland, un programme sans fenêtre ne peut ni lire ni poser le presse-papiers ; le passage par X11 (Xwayland) de Kyber dépend de ce que le compositeur recopie entre X11 et Wayland (KWin ne le fait que quand une fenêtre X11 a le clavier). kynput passe donc par le protocole des gestionnaires de presse-papiers quand le compositeur l'offre : `ext-data-control-v1` (KDE Plasma 6), sinon `zwlr-data-control-unstable-v1` (Plasma 5, KWin jusqu'à 6.4). Texte et HTML, mêmes droits que les autres Stations (copie vers l'appareil, collage depuis l'appareil). GNOME n'offre ni l'un ni l'autre : la Station y reste sur X11, dont Mutter recopie le presse-papiers vers Wayland et retour, et le journal le dit (« Wayland compositor without data-control (GNOME): clipboard through X11 (Xwayland) »). `KYBERNOW_CLIPBOARD=x11` force X11 (dépannage).

### Son

Admin (`audio/mod.rs`, module `linux`) crée au début de la session une sortie « KyberNow » (`module-null-sink` de PipeWire, par `pactl`) au nombre de voies de l'appareil (2, 6 ou 8, ordre de PulseAudio), en fait la sortie par défaut et y déplace les sons en cours : rien ne sort des haut-parleurs de l'ordinateur. Kyber capture le retour de la sortie par défaut. À la fin : sortie d'origine rendue, sons ramenés, module retiré (et retiré au démarrage s'il restait d'une session interrompue).

### Micro de l'appareil

[micro.md](micro.md). L'aide `kybernow-micro` (dans `runtime/bin`, à côté de kycontroller ; libopus du runtime) crée au besoin, par `pactl`, une sortie nulle `kybernow_micro_entree` et la source « Micro KyberNow » (`kybernow_micro`, `module-remap-source` sur son retour), une fois pour la session de l'utilisateur, puis joue le micro reçu par `pacat` (pulseaudio-utils, déjà une dépendance). Pendant l'envoi, la source est le micro par défaut (`pactl set-default-source`) ; le précédent revient à la fin. Compilé (`cargo check --target aarch64-unknown-linux-gnu`) ; pas encore essayé en VM.

### Encodeur

L'installation choisit celui de la carte (`kybernow-station-setup`) : NVIDIA avec son pilote propriétaire (`/proc/driver/nvidia/version` ou `libnvidia-encode.so.1`) → NVENC ; AMD, Intel, ou NVIDIA sous nouveau ou NVK (pas de NVENC), avec un nœud de rendu → VA-API ; sinon x264 (essais : `server/linux/scripts/test-setup.sh`). Admin propose ces trois-là dans Réglages › Image (« VA-API — carte AMD ou Intel »).

### Textes d'Admin

Admin donne `platform` : `linux` (`GET /api/session`) : l'interface et Admin remplacent chaque texte propre à Windows (menu Démarrer, pilote SudoVDA, haut-parleurs de Steam, session Windows…) par sa version Linux, rangée sous `linux` dans les catalogues (`server/web/src/lib/i18n/`, `server/admin/src/i18n/`), et les textes des réglages de Kyber par `schema.linux.json` ([i18n.md](i18n.md)). Trouvé à la première session depuis le Mac (25/11/2026) : Admin parlait de Windows.

### Services et paquet

- `kybernow-admin.service` et `kybernow-kyber.service`, services systemd de l'utilisateur (`/usr/lib/systemd/user`), rattachés à sa session graphique : ils démarrent à l'ouverture de session et s'arrêtent avec elle. Admin pilote Kyber par `systemctl --user` (`linux.rs`), et redémarre la Station par systemd.
- Premier démarrage : `kybernow-station-setup` (avant Admin) pose les réglages de Kyber pour Linux (moteur libavconv, capture GStreamer, fichier des écrans), un certificat propre à la Station, l'encodeur de la carte, le premier compte (votre nom d'utilisateur, mot de passe tiré au hasard dans `~/.local/share/kybernow/station/admin/mot-de-passe`).
- Données : `~/.local/share/kybernow/station/` (`admin/`, `kyber/`, `kyber-screens.toml`).
- Paquet : `server/linux/scripts/package.sh` → `kybernow-station_<version>_<arch>.deb` et `kybernow-station-<version>.<arch>.rpm` (`/usr/lib/kybernow-station`, services de l'utilisateur dont l'icône, socket et service root des mises à jour, règle udev). À l'installation, les services sont activés pour tous les utilisateurs (`systemctl --global enable`), sauf les comptes système (`ConditionUser=!@system` : gestionnaire de connexion, services) ; une seule session à la fois porte la Station (deux comptes connectés en même temps se disputeraient ses ports), le socket des mises à jour est ouvert et la règle udev est chargée ; une mise à jour du paquet garde tout cela (les scripts ne désactivent qu'à la désinstallation). Dépendances de l'icône : GTK 3, AppIndicator (Ayatana ou l'ancien) et WebKitGTK 4.1 (écran du prêt) ; recommandés : zenity ou kdialog, notify-send. Les aides propres à un bureau (l'extension AppIndicator de GNOME, `kscreen-doctor` de KDE) ne viennent que sur ce bureau : le .rpm les recommande sous condition (`if gnome-shell`, `if plasma-workspace`), le .deb les suggère (Debian n'a pas de condition) ; sinon l'extension, qui dépend de GNOME Shell, installait tout GNOME et GDM sur une Fedora KDE. Dépendances d'Admin : ethtool, greffons GStreamer « good », hwdata ; recommandés : iw, xprop, rsvg-convert. Pare-feu, comme sous Windows : le paquet déclare le service firewalld `kybernow-station` (8080 TCP et UDP pour le flux, 9443 TCP pour Admin, 5353 UDP pour Bonjour) et le profil ufw « KyberNow Station » ; à l'installation, si firewalld ou ufw tourne, ils sont ouverts (zone par défaut de firewalld), et refermés à la désinstallation, qui arrête aussi la Station dans les sessions ouvertes (`systemctl --user -M user@`). Sans cela, la zone par défaut de Fedora laissait passer le flux et Admin mais pas Bonjour : la Station n'apparaissait pas dans les apps. `uinput` et `uhid` (DualSense) chargés au démarrage (`modules-load.d`), ouverts à l'utilisateur par la règle udev avec le hidraw des DualSense. Le même assemblage donne aussi l'archive des systèmes immuables (`server/linux/immutable/make-archive.sh`). Publication : `server/linux/scripts/release-station.sh` construit les quatre paquets sous leurs noms fixes (arm64 dans la VM ; x86_64 sur une machine x86_64 jointe par ssh, sinon dans la même VM, dans un Ubuntu 24.04 x86_64 par Rosetta : `server/linux/scripts/x86-chroot.sh setup` le prépare une fois, debootstrap en deux temps, paquets de « Compiler », Rust, sources de Kyber copiées à part) ; `scripts/release.sh` les publie dans chaque version (`KYBERNOW_STATION_LINUX=0` pour les laisser de côté) (voir [releases.md](releases.md)).

### Mises à jour et icône

- **Mises à jour** (jalon 6), comme sous Windows et sur Mac : même manifeste signé minisign, même vérification par Admin (signature par la clé compilée, version, canal, taille et empreinte du paquet), même page Mises à jour et mêmes boutons dans les apps. Le paquet est celui de l'architecture et du format installés : `KyberNow-Station-Linux-{x86_64,arm64}.{deb,rpm}` (`update/linux.rs`, format lu dans dpkg, sinon rpm).
- Admin n'est jamais root. Il télécharge et vérifie dans `~/.local/share/kybernow/station/admin/update/<version>/`, puis dépose sa demande sur le socket `/run/kybernow-station/update.sock` (`kybernow-station-update.socket`, 0660, groupe `kybernow-station`). Depuis le 27/11 (STLIN-02), seul le propriétaire de la Station le joint : le paquet crée le groupe (`sysusers.d`), y ajoute le compte qui l'installe (`sudo`, `pkexec`) et le note comme propriétaire du socket (`/etc/systemd/system/kybernow-station-update.socket.d/owner.conf`, `SocketUser`), ce qui vaut tout de suite, sans rouvrir la session ; un autre compte local ne peut plus demander de retour arrière ni de bêta. Installé autrement (sans `sudo` ni `pkexec`) : `sudo usermod -aG kybernow-station VOTRE_COMPTE`, puis rouvrir la session. La purge retire `/var/lib/kybernow-station` et le propriétaire noté (le groupe reste). systemd lance alors, pour cette connexion, le service root `kybernow-station-update@.service` (`kyber-admin update serve`), qui ne croit rien de la demande : il lit le compte qui la fait (`SO_PEERCRED`), ne prend les fichiers que dans le dossier de mises à jour de ce compte, sans suivre de lien, les copie dans son dossier `/var/lib/kybernow-station/update/<version>/` et y revérifie tout (signature, version plus récente que le paquet installé, canal suivi par ce compte, `min_installer_version`, taille et empreinte). Il répond à Admin une fois la demande vérifiée, avant d'arrêter quoi que ce soit : un refus s'affiche dans Admin comme ailleurs.
- Puis il arrête l'icône, Kyber et Admin de ce compte (`runuser … systemctl --user`), installe le paquet (`apt-get install` pour un .deb, qui prend les nouvelles dépendances, sinon `dpkg -i` ; `dnf install` pour un .rpm, sinon `rpm -Uvh`), relance les services et attend qu'Admin réponde dans la nouvelle version (60 s, à l'adresse de son réglage `listen`).
- **Retour arrière.** Avant d'installer, Admin télécharge aussi le paquet de la version qui tourne (depuis sa version publiée, vérifié contre son manifeste signé) si le service root ne le garde pas déjà ; le service root le vérifie et le garde. Si Admin ne répond pas dans la nouvelle version, il réinstalle celui-là (`--allow-downgrades`, `rpm --oldpackage`). « Revenir à la version précédente » réinstalle de même la version gardée (`previous`), revérifiée ; seules les deux versions utiles restent dans `/var/lib/kybernow-station/update/`. Une Station installée à la main depuis une version qui ne publiait pas encore de paquets Linux (0.11.0 et avant) n'a pas de retour arrière pour sa première mise à jour : le journal le dit.
- Résultat dans `/var/lib/kybernow-station/update/resultat.json` (lisible par tous, au même format), qu'Admin relit à son démarrage ; journal dans `<version>/update.log` à côté, et `journalctl -u 'kybernow-station-update@*'`.
- **Systèmes immuables.** Un Admin installé dans le dossier de l'utilisateur (`~/.local/lib/kybernow-station/versions/<version>/bin`, voir « Systèmes immuables ») prend l'archive `KyberNow-Station-Linux-{x86_64,aarch64}.tar.xz`, vérifiée de même, et la confie à `kybernow-station-home update <archive> <version> --result … --log …`, à côté de lui : pas de service root ; résultat dans son propre dossier `update/`. « Revenir à la version précédente » : `kybernow-station-home rollback`, vers le lien `previous`. Sur un système ostree (`/run/ostree-booted`) où la Station est en couche sous `/usr` (rpm-ostree), Admin annonce la version sans l'installer : « s'installe avec rpm-ostree ».
- **Icône près de l'horloge** : le même programme que sous Windows (`server/tray`, tao et tray-icon), compilé pour Linux (`src/linux.rs`), en service systemd de l'utilisateur `kybernow-tray.service`. Une icône AppIndicator (StatusNotifierItem) : KDE Plasma l'affiche de lui-même, GNOME avec l'extension AppIndicator (d'office sous Ubuntu, `gnome-shell-extension-appindicator` ailleurs, recommandée par le .rpm et suggérée par le .deb). Mêmes menus et textes (`tray.*`), même état et même pastille (`src/status.rs`, partagé avec Windows) ; un clic ouvre toujours le menu, et les pages du panneau de Windows s'ouvrent dans Admin (partage, prêt, mises à jour). Pas de « Démarrer avec l'ordinateur » : les services démarrent avec la session. « Réinitialiser le mot de passe d'Admin » se fait sans droits particuliers (Admin tourne avec votre compte) et montre le code d'initialisation. Confirmations et messages par zenity ou kdialog ; une demande nouvelle (appareil, invité, ami) donne une notification.
- `kybernow-station-setup` pose `~/.local/share/kybernow/station/kybernow.json` (le port d'Admin), où Admin note l'empreinte de son certificat : l'icône ne parle qu'à ce certificat, comme sous Windows.

### Admin : réveil, aperçu, carte graphique, apps

Admin vit dans la session de l'utilisateur : il lit tout lui-même, sans agent ni droits root (`server/admin/src/linux_*.rs`).

- **Réveil par le réseau** (`linux_net.rs`) : les cartes de `/sys/class/net` qui tiennent à un vrai périphérique (`device`), Ethernet avant Wi-Fi, la carte de la route par défaut (`/proc/net/route`) d'abord ; ni ponts, ni cartes de conteneurs, de machines virtuelles ou de VPN, ni Bluetooth. L'adresse matérielle : celle de la carte, ou l'adresse permanente de `ethtool -P` quand NetworkManager en a tiré une au hasard. Adresses IPv4 par `ip -4 -o addr`. « Réveil permis » : `Wake-on` de `ethtool <carte>` contient `g` (paquet magique) ; non si le noyau interdit à la carte de réveiller l'ordinateur (`device/power/wakeup`) ; inconnu sans `ethtool` ou pour le Wi-Fi. Vitesse de la prise (PyroWave en Auto) : `speed` en Ethernet, `iw dev <carte> link` en Wi-Fi. Nom : celui de la puce dans la base PCI, puis l'interface (« Intel Ethernet Controller I225-V (enp3s0) »).
- **Aperçu de l'accueil** (`linux_screen.rs`) : hors session, l'agent des écrans tient déjà un flux PipeWire par écran de l'ordinateur (écrit dans `kyber-screens.toml`). Admin en lit **une image** sur l'écran placé en (0, 0), par GStreamer (`gst-launch-1.0 pipewiresrc … num-buffers=1 ! videoscale ! jpegenc`), réduite à la largeur demandée : aucun dialogue de partage d'écran, rien à installer de plus que pour Kyber. Gardée 10 s, et jamais faite pendant une session, comme sous Windows.
- **Carte graphique** (`linux_gpu.rs`) : `/sys/class/drm/card<N>` (NVIDIA, AMD, Intel, dans cet ordre, comme sous Windows), nom commercial par la base PCI (`pci.ids` de `hwdata` ou `pciutils` : « AD102 [GeForce RTX 4090] » → « NVIDIA GeForce RTX 4090 »), pilote par sa version (`/sys/module/nvidia/version`) ou « amdgpu (Linux 6.8.0) ». Supervision : NVML pour NVIDIA ; charge, mémoire, température et puissance d'`amdgpu` dans `/sys` ; rien pour Intel (il faut des droits que la Station n'a pas). Une carte de machine virtuelle (virtio) n'est pas montrée.
- **App au début de la session** (`linux_apps.rs`) : le catalogue est fait des fichiers `.desktop` de la session (`XDG_DATA_HOME`, puis `XDG_DATA_DIRS` : paquets, Flatpak, Snap), comme le menu du bureau : sans les apps cachées, réservées à un autre bureau ou du terminal ; nom dans la langue de la session ; icône cherchée dans les thèmes (`hicolor` d'abord) et `pixmaps`, en PNG, une icône SVG convertie par `rsvg-convert` s'il est là. Suggestions : Steam Big Picture (Steam en paquet ou Flatpak), Lutris, Heroic. « Autre… » : vos applications (`~/Applications`), le Bureau et `/opt` (fichiers `.desktop`, AppImage, scripts). Chaque app part dans **son propre service systemd de l'utilisateur** (`systemd-run --user`, `kybernow-app-<empreinte>`) : redémarrer Admin ne la ferme pas, et la fermer à la fin de la session arrête tout ce qu'elle a lancé ; Big Picture se ferme par `steam://close/bigpicture`. Wayland ne laisse pas ramener une fenêtre au premier plan : une app déjà ouverte reste comme elle est. Session verrouillée (`LockedHint` de `loginctl`) : l'app attend le déverrouillage, 2 minutes au plus.
- **Souris Auto** (`linux_screen.rs`) : Wayland ne dit pas quelle fenêtre a la main, mais les jeux de Steam (Proton et la plupart des jeux natifs) sont des fenêtres X11 servies par Xwayland, dont GNOME et KDE restent le gestionnaire : la fenêtre X11 active, son plein écran et son programme se lisent par `xprop` (paquet `x11-utils` ou `xprop`). Une fenêtre `steam_app_…` en plein écran est un jeu ; navigateurs et lecteurs vidéo n'en sont pas. Un jeu en fenêtre Wayland native n'est pas vu : la souris reste celle du bureau.

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
### Prêt de l'ordinateur

Le mode console dans la session de l'hôte, comme sur Mac ([pret.md](pret.md), « Station Linux ») : l'icône `kybernow-tray`, lancée par Admin avec `--console` dans sa propre unité systemd, met le lanceur de l'ami (les apps permises, rien d'autre du bureau) en plein écran sur l'écran virtuel et un cache sur chaque écran de l'ordinateur, prend l'entrée physique (`EVIOCGRAB`) et coupe les raccourcis du bureau (GNOME par `gsettings`, KDE par KGlobalAccel). Admin (`lend_sys.rs`) suit la session par `loginctl`, empêche veille et verrouillage tant que le prêt est activé (`systemd-inhibit`, et `gnome-session-inhibit` sous GNOME), surveille les processus par `/proc` et ferme ce qui n'est pas permis. `lend.available` est vrai dès que l'agent est à côté d'Admin. **Limite connue (STLIN-05)** : le paquet n'ouvre pas les claviers et souris de l'ordinateur à l'utilisateur (`/dev/input/event*`) : sinon, tout programme de la session pourrait lire ce qui est tapé. `EVIOCGRAB` échoue donc, et **pendant un prêt, le clavier et la souris de l'ordinateur ne sont pas bloqués** : les caches restent, mais quelqu'un devant l'ordinateur garde la main sur la session prêtée. Ne prêtez pas un ordinateur Linux que d'autres personnes peuvent toucher pendant le prêt. À venir : une petite aide root, lancée par socket comme les mises à jour, qui prendra l'entrée et rendra le descripteur ; et Admin le dira avant d'accorder le prêt (pas encore).

### PyroWave

Encodeur à part, en Vulkan (la bibliothèque PyroWave est en Vulkan), sur les images DMA-BUF de la capture PipeWire importées sans copie ; détail dans [pyrowave.md](pyrowave.md), « La Station Linux ».

- `server/linux/scripts/pyrowave-linux.sh` (lancé par `build-kyber.sh` avant FFmpeg) compile `libpyrowave-shared.so` et `pyrowave-device-validation` dans le rootfs de Kyber ; correctifs Kyber Linux 0010 (encodeur FFmpeg `libpyrowave_vulkan`), 0011 (kyavserver, moteur libavconv) et 0012 (kycontroller le propose).
- Admin (`pyrowave.rs`) vérifie une fois par lancement, en fond, la carte du nœud de rendu (`/dev/dri/renderD128`, `KYBERNOW_RENDER_NODE` pour un autre) : `pyrowave-device-validation --external --roundtrip --vid … --pid …` (Vulkan 1.3 qui convient, import des DMA-BUF et des descripteurs de synchronisation, aller-retour). `pyrowave.available` est vrai quand il réussit, sinon `reason: "vulkan"`.
- Une capture sans DMA-BUF (machine virtuelle) passe par le processeur pour la conversion en YCbCr : PyroWave marche, plus lentement.

### Débit en direct

Le correctif commun 0026 couvre aussi VA-API (cartes AMD et Intel) : quand l'app change le débit (`/set_bitrate`), les paramètres du débit de l'encodeur (débit, mémoire HRD) suivent et partent avec l'image suivante, sans image clé ; avant, ils n'étaient envoyés qu'avec les images IDR (toutes les 10 s avec le GOP de 600 images). NVENC et VideoToolbox ne changent pas ([debit-automatique.md](debit-automatique.md)). x264 suit déjà un changement de débit de lui-même.

### HDR

La question : GNOME ou KDE peuvent-ils donner à un programme un écran virtuel en BT.2100 (PQ, couleurs BT.2020), comme l'écran virtuel HDR de la Station Windows ? **Pas encore**, ni l'un ni l'autre, et la Station Linux n'annonce donc pas le HDR (`hdr` faux : les apps restent en SDR). Étude du 25/11/2026, d'après les interfaces publiques des deux compositeurs et les rapports de développeurs ; à confirmer dans les VM :

- **GNOME (Mutter 46 à 49)** : le HDR existe pour les écrans physiques qui l'annoncent (EDID), réglé dans Paramètres › Écrans. Un écran virtuel (`org.gnome.Mutter.ScreenCast`, `RecordVirtual`) n'a aucune propriété de couleur ni d'EDID : il reste en sRGB, et son flux PipeWire ne propose que des formats 8 bits, sans colorimétrie. Il faudrait un Mutter modifié (écran virtuel avec un mode de couleur, flux 10 bits annoncé en BT.2100) : hors de notre portée tant que Mutter ne le fait pas lui-même.
- **KDE Plasma 6 (KWin)** : HDR et gestion des couleurs par écran depuis Plasma 6.0, pour les écrans qui l'annoncent. L'écran virtuel de `zkde_screencast_unstable_v1` (`stream_virtual_output`) n'en a pas les capacités (rien à allumer par `kscreen-doctor output.Virtual-KyberNow-1.hdr.enable`), et le greffon de capture de KWin ne propose que des formats DRM 8 bits : même un rendu HDR y serait ramené en SDR.
- **Côté Kyber aussi** : la capture GStreamer de libavgrab ne sait pas lire d'images 10 bits (RGB 10 bits et P010 ne sont pas dans sa table des formats) ni les métadonnées HDR ; il faudrait l'équivalent Linux du correctif commun 0022 (P010 vers HEVC Main 10 en VA-API ou NVENC, métadonnées).
- **Détour écarté** : capturer en KMS un écran physique HDR (kmsgrab) demande des droits root (`CAP_SYS_ADMIN`) et ne fait pas d'écran virtuel à la taille de l'appareil.

À refaire quand Mutter ou KWin donneront un écran virtuel avec une description de couleur (protocole Wayland `wp_color_management_v1` côté compositeur, ou une propriété de `RecordVirtual`) et des flux PipeWire 10 bits. À vérifier dans les VM d'ici là : `kscreen-doctor -o` (lignes HDR et WCG de `Virtual-KyberNow-1`) et les formats du nœud PipeWire de l'écran virtuel (`pw-dump`).

## Correctifs Kyber propres à Linux

Rangés dans `server/linux/kyber-patches`, posés après les correctifs communs par `server/linux/scripts/build-kyber.sh` (détail dans [kyber-changes.md](kyber-changes.md)) :

| Correctif | Ce qu'il change |
|---|---|
| `0001-kyavservice-ecrans-suivis-linux` | Fichier des sources GStreamer (`[kyavserver.gstreamer] sources_file`) relu à chaque liste et à chaque image, surveillé pour annoncer les changements ; identifiant et place de chaque source |
| `0002-kynput-pointeur-wayland` | Pointeur absolu par uinput sous Wayland, clics et molette par lui tant qu'il tient le pointeur |
| `0003-libavgrab-gstreamer-1-24` | Compile avec GStreamer 1.24 (Ubuntu 24.04), qui n'a pas `gst_message_get_details` |
| `0004-kymedia-luac-5-4` | `luac` 5.4 compilé par Kyber quand le système n'en a pas (Fedora 45 : Lua 5.5), avec les sources de Lua |
| `0005-kynput-manettes-linux` | Manette PlayStation de l'appareil en DualSense par uhid (vibrations renvoyées), les autres en Xbox 360 par uinput ; « Toutes les manettes en Xbox 360 » respecté ; vibrations Xbox sans fin et lues d'un coup |
| `0006-kynput-stylet-linux` | Stylet en tablette graphique uinput (pression, inclinaison, gomme, bouton, survol) ; la Station l'annonce (`PenAvailable`) |
| `0007-kynput-presse-papiers-wayland` | Presse-papiers par `ext-data-control-v1` (ou `zwlr-data-control`) sous Wayland quand le compositeur l'offre (KDE) ; sinon X11 par Xwayland (GNOME) |
| `0010-kymedia-ffmpeg-pyrowave-vulkan` | Encodeur FFmpeg `libpyrowave_vulkan` : images DMA-BUF de la capture importées dans Vulkan, PyroWave sur la carte (voir « PyroWave ») |
| `0011-kyavservice-pyrowave-linux` | kyavserver accepte PyroWave sous Linux, avec cet encodeur |
| `0012-kycontroller-pyrowave-linux` | kycontroller propose PyroWave sur une Station Linux |
| `0013-kyavservice-ecrans-wayland` | Écran `source = "wayland:NOM"` capturé par libavgrab (wlr-screencopy) par son nom, une image par trame même immobile, mémoire partagée avec x264 (Sway, Hyprland) |

## Compiler

Sur un Linux de l'architecture voulue (la VM « KyberNow Linux » pour arm64 ; x86_64 sur un PC ou dans la même VM par Rosetta) :

```sh
# Outils (Ubuntu 24.04 ; noms voisins sur Debian et Fedora)
sudo apt install build-essential cmake nasm bison flex pkg-config pipx lua5.4 python3-jinja2 \
  glslang-tools spirv-tools libclang-dev libssl-dev libv4l-dev libcap-dev libvdpau-dev libnuma-dev \
  libva-dev libdrm-dev libgbm-dev libegl-dev libvulkan-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpipewire-0.3-dev libpulse-dev libasound2-dev \
  libxcb1-dev libxcb-shm0-dev libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shape0-dev libxtst-dev \
  libxkbcommon-dev libwayland-dev wayland-protocols libevdev-dev libudev-dev libinput-dev libgtk-3-dev libwebkit2gtk-4.1-dev
# Fedora 42 et après (sans libplacebo-devel : Kyber compile la sienne)
sudo dnf install gcc gcc-c++ cmake nasm bison flex pipx python3-jinja2 glslang spirv-tools clang-devel \
  openssl-devel libv4l-devel libcap-devel libvdpau-devel numactl-devel libva-devel libdrm-devel \
  mesa-libgbm-devel mesa-libEGL-devel vulkan-loader-devel gstreamer1-devel gstreamer1-plugins-base-devel \
  pipewire-devel pulseaudio-libs-devel alsa-lib-devel libxcb-devel libXtst-devel libXrandr-devel libxkbcommon-devel \
  wayland-devel wayland-protocols-devel libevdev-devel systemd-devel libinput-devel dbus-devel rustup gtk3-devel webkit2gtk4.1-devel \
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

Rien de propre à Linux ([link-ui.md](link-ui.md), « Face à une Station Linux ») : un PC, dont le système est le nom de la distribution. Ce qui n'est pas encore là (HDR) n'est pas annoncé, et les apps font comme avec une Station qui ne l'a pas ; PyroWave et le prêt sont annoncés quand la carte graphique convient et quand l'agent du mode console est là.

## Découpage

1. **Kyber sous Linux.** Compilé avec nos correctifs pour x86_64 et arm64 (`build-kyber.sh`), sans le lecteur VLC. **Fait dans la VM arm64 (25/11/2026).**
2. **Admin sous Linux.** Service systemd, Station reconnue, redémarrage, encodeurs : **écrit**. Réveil par le réseau (`/sys`, `ethtool`), aperçu de l'accueil (une image du flux PipeWire de l'écran), carte graphique (`/sys/class/drm`, base PCI, NVML, `amdgpu`), app au début de la session (fichiers `.desktop`, services systemd, Steam Big Picture), souris Auto pour les jeux X11 (Xwayland) : **écrit et testé sur le Mac** (lecture des sources), pas encore essayé dans une VM ni sur une machine réelle.
3. **Écrans virtuels.** GNOME par Mutter et KDE Plasma 6 par KWin : **écrits et essayés dans les VM** (voir « Essais »).
4. **Son.** Sortie PipeWire KyberNow : **écrit**.
5. **Entrées.** uinput, règle udev, pointeur absolu sous Wayland : **écrit**. Manettes DualSense par uhid, stylet (tablette uinput, à la suite du correctif commun 0027), presse-papiers sous Wayland (data-control) : **écrits** (correctifs 0005 à 0007, compilés pour Linux et essais unitaires sur le Mac le 25/11/2026), pas encore essayés sous Linux. Règle udev de `/dev/uhid` et module `uhid` chargé au démarrage : dans le paquet (branche des mises à jour).
6. **Paquet, mises à jour.** `.deb`, `.rpm`, services : **écrit**. Mises à jour depuis Admin et depuis les apps (service root par socket, retour arrière), icône près de l'horloge, paquets publiés : **écrits**, testés sur le Mac (tests d'Admin et de l'icône), pas encore essayés sous Linux (voir « Essais »).
7. **Fonctions avancées.** Share et relais (rien de propre au système dans Admin : à essayer). Prêt du PC (mode console dans la session de l'hôte, veille empêchée), PyroWave (Vulkan, import DMA-BUF, correctifs Linux 0010 à 0012), débit en direct en VA-API (correctif commun 0026) : **écrits** (25/11/2026), pas encore compilés ni essayés sous Linux. HDR : **pas encore possible** (ni GNOME ni KDE ne donnent d'écran virtuel en BT.2100, voir « HDR »).
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

**Suite de la Station Linux (26/11/2026)**, dans les deux VM, depuis `main` (Admin, mises à jour, icône, entrées, prêt, PyroWave, immuables) :

- **Compilation** sous Ubuntu 24.04 et Fedora 45 : Kyber avec tous les correctifs (communs, Linux 0001 à 0012, PyroWave Vulkan), Admin, l'agent des écrans et l'icône ; 352 tests d'Admin et 30 de l'icône sous Linux ; `pyrowave-device-validation` répond sous Fedora.
- **Paquets** : `.deb`, `.rpm` et archive des systèmes immuables construits sur les deux. `.rpm` installé par dnf sous Fedora KDE, `.deb` par apt sous Ubuntu GNOME : première initialisation, Admin, Kyber et l'icône démarrent en services de l'utilisateur ; session par l'API (écran virtuel 1920 × 1080 seul et principal, son vers « KyberNow » en 2 et 6 voies, flux lu comme le lit Kyber, tout rendu à la fin).
- **x86_64** : Kyber, Admin, l'agent et l'icône compilés dans un Ubuntu 24.04 x86_64 par Rosetta (VM arm64, `x86-chroot.sh`) ; `.deb`, `.rpm` et archive construits, programmes en x86-64 qui démarrent (Admin 0.11.2, Kyber 0.29.0), aucune bibliothèque manquante. Pas essayé sur un vrai PC x86_64.
- **Sway** (1.9, sans tête, à côté de GNOME dans la VM Ubuntu) : session par l'API à deux écrans (HEADLESS créés, placés, écran de l'ordinateur éteint, tout rendu à la fin) ; session depuis l'app Mac avec image, bureau immobile compris (après le correctif `copy`). Hyprland 0.57.2 (compilé depuis les sources, imbriqué dans KDE de la VM Fedora) : session par l'API à deux écrans, avec le `.rpm` (écrans créés, placés, écran de l'ordinateur éteint puis rallumé) ; session depuis l'app Mac avec image (26/11).
- **Session depuis l'app Mac** vers la Station installée par le `.rpm` sous Fedora KDE : elle marche, clavier et souris compris (essai de l'auteur) ; image très saccadée, attendu dans la VM (x264 sur le processeur, sans carte graphique ni DMA-BUF).
- Défauts trouvés et corrigés : le pare-feu de Fedora cachait la Station aux apps (Bonjour fermé) ; l'encodeur PyroWave Vulkan ne compilait pas (`F_DUPFD_CLOEXEC` caché par les options de FFmpeg) ; `pyrowave-device-validation` gardait le dossier de compilation dans son chemin de recherche (refusé par rpmbuild) ; `package.sh` s'arrêtait sans `lib64` (Ubuntu) ; l'icône se lie à WebKitGTK (écran du prêt) sans que les paquets le demandent ; l'extension AppIndicator recommandée installait tout GNOME et GDM sur une Fedora KDE ; au premier démarrage d'une installation neuve, `kybernow-station-setup` s'arrêtait avant Admin (fichier de réglages pas encore écrit) ; huit tests d'Admin supposaient « pas Windows = Mac » ; hors Station, le prêt était refusé sous Linux au lieu d'être simulé comme sur le Mac.

**Pas encore essayé** : la mise à jour depuis Admin (socket, service root, apt et dnf, contrôle de santé, retour arrière), l'icône regardée à l'écran (son service tourne sous GNOME et KDE), le prêt avec son agent de console ; une session de bout en bout depuis une app KyberNow Link (image, son, latence), les manettes (Xbox et DualSense), le stylet, le presse-papiers sous Wayland, VA-API et NVENC (aucune vraie carte graphique dans la VM), x86_64, une machine réelle.
