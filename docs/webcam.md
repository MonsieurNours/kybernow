# Caméra de l'appareil vers l'ordinateur

[English](webcam.en.md) · Français

La caméra de l'appareil (téléphone, tablette, Mac, PC, webcam USB, iPhone près de l'Apple TV) part vers l'ordinateur, qui la voit comme une webcam ordinaire, « KyberNow Camera » : Teams, Zoom, Discord, OBS et les navigateurs la proposent dans leur liste. Les règles visibles, pour toutes les apps, sont dans [link-ui.md](link-ui.md), « Caméra de l'appareil ». Ici : comment c'est fait et où en sont les essais.

## Choix de l'auteur (26/11/2026)

- Apps : Mac, iPhone et iPad, Android, Windows, Linux (bureau, Steam Deck), Apple TV (Caméra de continuité). Pas le client web (plus tard), ni les téléviseurs LG, ni le Raspberry Pi.
- L'hôte et l'ami qui emprunte l'ordinateur ; jamais les invités d'un partage.
- À la demande, éteinte au début de chaque session ; un voyant tant qu'elle envoie.
- Réglage de l'ordinateur « Définition de la caméra » : 720p (par défaut) ou 1080p, 30 images par seconde.
- Liste des caméras dans le menu, la dernière choisie retenue.
- App en arrière-plan (iPhone, iPad, Android) : pause, l'ordinateur garde la dernière image.
- Station Windows : la caméra virtuelle de Windows 11 (`MFCreateVirtualCamera`) ; sous Windows 10, notre propre filtre DirectShow.
- Station Linux : v4l2loopback, sinon une caméra PipeWire.

## Le chemin

```
app : capture → encodeur matériel (H.264, HEVC) → cœur (camera.rs) → kyclient (flux montant)
  ══ QUIC, flux de données à part ══►
Station : kycontroller (uplink.rs) → kybernow-camera (entrée standard) → décodage → caméra virtuelle
```

### Le flux montant (correctif 0085, commun avec le micro)

Kyber sait déjà ouvrir des « points de données » (`register_data_endpoint` de kyproto) : un flux QUIC fiable dans chaque sens, à part de ceux de l'image, du son et des entrées. Ce qui monte ne passe donc jamais devant la souris ni ne l'attend, et la caméra et le micro ne s'attendent pas l'un l'autre.

- **Au départ de l'image**, l'app dit ce qu'elle sait envoyer : `uplink: ["camera"]` dans la demande `/kymux/start_session` (à côté du `StartRequest` de kyctl-common, comme `hdr` : une app d'avant n'envoie rien). Pour chaque type dont l'outil (« sink ») est à côté de kycontroller (`kybernow-camera`, `kybernow-micro`), et jamais pour un invité d'un partage, la Station prépare un point de données et le rend dans la réponse : `uplink: [{"kind": "camera", "endpoint": 12}]`. Une Station d'avant répond sans `uplink` : l'entrée « Caméra » n'apparaît pas.
- **kyclient** se branche sur chaque point rendu et donne à l'app un `Uplink` (`Client::uplink("camera")`) : `try_send` (jamais d'attente : 24 paquets en attente au plus, au-delà `Full`), `try_recv` pour les réponses. Il se ferme quand l'image s'arrête ou est relancée ; le cœur rallume alors la caméra sur le nouveau flux.
- **Les paquets** (le premier octet dit lequel) :

| Type | Sens | Contenu |
|---|---|---|
| `START` 0x01 | app → ordinateur | JSON : `{"codec": "h264", "width": 1280, "height": 720, "fps": 30, "resolution": "720p", "device": "iPhone de Marie"}`. Aussi pour changer de caméra ou de définition sans éteindre. |
| `MEDIA` 0x02 | app → ordinateur | drapeaux (u8 : bit 0 image clé ; bits 1 et 2, les quarts de tour, sens horaire, qui redressent l'image : Android envoie l'image du capteur telle quelle), pts en µs (u64, gros-boutiste), puis l'image encodée (Annex B, paramètres SPS/PPS/VPS avec chaque image clé) |
| `PAUSE` 0x04 | app → ordinateur | rien : l'ordinateur garde la dernière image |
| `STOP` 0x03 | app → ordinateur | rien : l'outil s'arrête |
| `STATUS` 0x10 | ordinateur → app | une ligne JSON : `{"state": "started"}`, `{"state": "busy", "device": "Pixel 9"}`, `{"state": "failed", "reason": "…"}`, `{"state": "stopped"}`, `{"keyframe": true}` |

- **Un seul appareil à la fois** par type sur l'ordinateur : un second reçoit `busy`, avec le nom du premier.
- **kycontroller** lance l'outil au premier `START` (`kybernow-camera --uplink`, à côté de son exécutable, ou dans `KYBERNOW_SINKS_DIR`) et lui passe `START`, `MEDIA` et `PAUSE` sur son entrée standard, chacun précédé de sa longueur (u32, gros-boutiste). Ce que l'outil écrit en JSON sur sa sortie (une ligne par message) revient à l'app en `STATUS`. Son entrée se ferme sur `STOP`, quand l'app part ou quand la session finit : il s'arrête de lui-même (tué au bout de 3 s sinon). Un outil lent ralentit l'écriture, donc le flux, donc la file de l'app, qui lâche des images : rien ne s'accumule.

### Le cœur (`apps/core/src/camera.rs`)

`Session::camera()` rend la `CameraLink` de la session ; l'app l'appelle de n'importe quel fil. `Platform::sends_camera()` (vrai pour une app qui sait capturer) fait demander la caméra à chaque départ de l'image ; `Platform::on_camera_offer(bool)` dit quand l'entrée apparaît ou disparaît.

- `start(CameraSettings)` envoie `START` ; `frame(données, pts_µs, clé)` envoie une image et rend `Sent`, `WantKeyframe` (lâchée : la suivante doit être une image clé) ou `Off` ; `pause()`, `stop()` ; `poll()` rend les réponses (`Started`, `Busy`, `Failed`, `Stopped`, `KeyframeWanted`, `Offer`).
- Après une image lâchée, plus rien ne part avant une image clé (les suivantes en dépendraient), et le débit visé baisse d'un quart (au plus une fois par demi-seconde, jamais sous 300 kbit/s) ; il remonte de 10 % toutes les 5 s sans perte, jusqu'au débit de départ : 1,5 Mbit/s en 720p, 4 Mbit/s en 1080p, la moitié par le relais (`target_bitrate()`, à donner à l'encodeur).
- Image relancée (profil changé, reprise) : la caméra allumée renvoie `START` sur le nouveau flux et repart d'une image clé, sans geste.
- Textes des refus et de la ligne « Caméra » des statistiques : `event_notice`, `device_notice`, `stats_line` (catalogues du cœur, section `camera`).

## L'outil de la Station (`server/camera`)

`kybernow-camera`, un seul programme en C pour les trois systèmes, compilé contre le FFmpeg de Kyber de la Station (`server/camera/build.sh`, comme `kybernow-encoder-check`), posé à côté de kycontroller :

- `--uplink` (lancé par kycontroller) : lit les paquets sur son entrée standard ; un fil décode (libavcodec : H.264 ou HEVC, sans délai, deux fils de tranches) et met l'image à la définition du `START` (720p ou 1080p) en gardant son rapport, centrée sur du gris foncé (libswscale) ; le fil principal donne la dernière image à la sortie 30 fois par seconde, qu'une nouvelle soit arrivée ou non (les logiciels qui filment veulent un rythme régulier). Avant la première image : gris foncé ; en pause : la dernière image ; à la fin : une seconde de gris, puis il s'arrête. Une image qui ne se décode pas (référence perdue) demande une image clé à l'app (`{"keyframe": true}`, une fois par seconde au plus).
- `--probe` : `{"ready": true}` si la caméra virtuelle s'ouvrirait, sinon la raison ; `--install` / `--uninstall` (Windows) ; `--test [720p|1080p]` : 20 s de mire, pour essayer la caméra virtuelle sans app.
- Échec à l'ouverture de la caméra virtuelle : `{"state": "failed", "reason": …}`, que l'app dit « {nom} n'a pas pu ouvrir sa caméra virtuelle. ».

Un `START` de plus (autre caméra, autre définition) relance l'outil (kycontroller arrête l'ancien d'abord). Les quarts de tour de `MEDIA` sont appliqués avant la mise à la taille (plans YUV tournés). **Essayé sur le Mac** (26/11) avec une sortie d'essai qui écrit les images dans un fichier : un flux H.264 480 × 640 de ffmpeg découpé en paquets comme kycontroller les passe, avec un quart de tour : image nette, redressée, bordée de gris en 1280 × 720, 30 images par seconde, fin propre à la fermeture de l'entrée.

## Station Windows

- **L'outil** écrit l'image NV12 dans une mémoire partagée nommée `Global\KyberNowCameraFrame` (`server/camera/windows/kn_shared.h` : une image, un compteur pair quand elle est stable, l'heure de la dernière écriture) : il tourne en SYSTEM, lancé par kycontroller comme kyavserver. Lecture permise à tous, applications à faible intégrité comprises ; écriture à SYSTEM et aux administrateurs. Sans image depuis 2 s, les lecteurs montrent du gris.
- **Windows 11** (build 22000 et après) : `KyberNowCameraSource.dll`, une source Media Foundation (`IMFMediaSourceEx`, `IMFMediaStream2`, `IKsControl` sans propriété), NV12 1280 × 720 ou 1920 × 1080 à 30 images par seconde, une image toutes les 1/30 s quel que soit le rythme des demandes. `kybernow-camera --install` l'inscrit (COM, HKLM, `ThreadingModel` Both) puis crée la caméra par `MFCreateVirtualCamera` (chargée depuis `mfsensorgroup.dll`), durée de vie « système », pour tous les utilisateurs : Windows la garde d'un démarrage à l'autre et le service Frame Server charge la source quand une app filme ; toutes les apps la voient (Caméra de Windows comprise). `--uninstall` la retire (`Remove`). MFPlat est chargée à la demande : sur un Windows N sans le Media Feature Pack, l'outil le dit au lieu de ne pas se lancer.
- **Windows 10** : `KyberNowCameraFilter.dll`, un filtre source DirectShow écrit par nous (sans les classes de base de l'exemple de Microsoft) : une broche de capture YUY2 (1280 × 720 ou 1920 × 1080, 30 images par seconde), `IAMStreamConfig` (les deux tailles), `IKsPropertySet` (catégorie capture), horodatage sur l'horloge du graphe. `--install` l'inscrit dans la catégorie des caméras de DirectShow (`IFilterMapper2`). Zoom, Teams, Discord, OBS, Chrome et Edge le voient ; les apps qui ne lisent que Media Foundation (Caméra de Windows) non.
- Les deux DLL sont liées au runtime MinGW en statique : elles sont chargées hors du dossier de Kyber (Frame Server, l'app qui filme).
- **Installation** : `server/kyber-patches/build.sh` construit les trois fichiers dans la surcouche de Kyber ; l'installateur lance `kybernow-camera.exe --install` après l'installation des fichiers (jamais bloquant : une ligne au journal en cas d'échec), la désinstallation `--uninstall` avant de retirer les fichiers.

## Station Mac

- **L'extension** (`server/macos/camera-extension`, Swift, CMIOExtension) : le périphérique « KyberNow Camera », deux flux : celui que lisent les apps (NV12, 1280 × 720 ou 1920 × 1080, 30 images par seconde) et une entrée (« sink ») où l'outil pose ses images. La dernière image repart 30 fois par seconde, mise au format choisi par l'app par `VTPixelTransferSession` ; sans image depuis 2 s, du gris foncé. Bac à sable, groupe d'apps `P6DB682S4K.fr.leslignesclaires.kybernow.station.camera`, service Mach du même nom (`CMIOExtensionMachServiceName`).
- **L'outil** (`out_macos.c`) trouve le périphérique par son nom, ouvre son flux d'entrée (direction 1, `CMIOStreamCopyBufferQueue`, `CMIODeviceStartStream`) et y met chaque image en `CMSampleBuffer` (`CVPixelBuffer` « 420v » sur IOSurface) ; file pleine : l'image saute.
- **Activation** : l'icône de la barre des menus (`server/macos/menubar/Camera.swift`) demande à macOS d'activer l'extension à chaque lancement (`OSSystemExtensionRequest`, remplacée par celle de la version lancée). La première fois, macOS attend l'accord : une notification dit où le donner (Réglages Système › Général › Ouverture et extensions › Extensions de caméra). L'app doit être dans Applications.
- **Droits d'Apple** : installer une extension système demande `com.apple.developer.system-extension.install`, que seul un profil donne. `build-station-app.sh` ne met l'extension et l'outil dans l'app (avec `station-camera.entitlements` et `embedded.provisionprofile`) que s'il trouve un profil Developer ID de `fr.leslignesclaires.kybernow.station` qui l'autorise (`KYBERNOW_STATION_PROFILE`, sinon les dossiers de profils de Xcode) ; sinon la caméra n'est pas proposée, rien d'autre ne change. **À faire une fois** sur le portail Apple Developer (équipe P6DB682S4K) : l'identifiant `fr.leslignesclaires.kybernow.station` avec la capacité « System Extension », l'identifiant `fr.leslignesclaires.kybernow.station.camera` (aucune capacité), puis un profil Developer ID pour le premier, installé sur le Mac qui construit. Ce droit est ouvert au Developer ID : pas de demande à Apple à attendre, contrairement aux manettes. L'extension n'a pas besoin de profil.

## Station Linux

- **v4l2loopback** d'abord : le périphérique `/dev/videoN` nommé « KyberNow Camera » (sinon le premier de ce pilote), en YUYV, écrit par `write()` ; Zoom, Discord, Chrome, Firefox et OBS le lisent. Le paquet installe `modules-load.d/kybernow-camera.conf` (charge le module au démarrage, s'il est installé), `modprobe.d/kybernow-camera.conf` (`card_label="KyberNow Camera" exclusive_caps=1` : vu comme caméra seulement quand la Station y écrit) et une règle udev (`uaccess` : la session de l'utilisateur y écrit sans être du groupe video). Le module n'est pas dans les noyaux des distributions : paquet `v4l2loopback-dkms` (Ubuntu, Debian ; suggéré par le .deb) ou `akmod-v4l2loopback` (Fedora, RPM Fusion).
- **PipeWire** sinon (outil compilé avec libpipewire-0.3) : un nœud `Video/Source` « KyberNow Camera » (YUY2), qui n'existe que tant que la caméra est envoyée ; vu par les logiciels qui lisent les caméras de PipeWire (navigateurs par le portail Caméra, OBS), pas par Zoom ni Discord.
- `server/linux/scripts/package.sh` construit l'outil dans `runtime/bin`, à côté de kycontroller (rpath vers les bibliothèques du runtime).

## Les apps

| App | Capture et encodage | Interface | État |
|---|---|---|---|
| Mac, iPhone et iPad, Apple TV | `apps/macos/KyberKit/CameraCapture.swift`, commun aux trois : AVFoundation (Mac : caméra intégrée, USB, iPhone par la Caméra de continuité ; iPhone, iPad : avant et arrière, image droite par `RotationCoordinator` ; Apple TV : Caméra de continuité seulement), préréglage 720p ou 1080p, 30 images par seconde, pas en miroir ; H.264 matériel par `VTCompressionSession` (temps réel, profil Main, sans images B, une image clé toutes les 2 s et sur demande, débit du cœur relu chaque seconde, plafond à 1,5 fois sur une seconde), AVCC vers Annex B avec SPS et PPS devant chaque image clé. Interruption (arrière-plan) : `kn_camera_pause`, image clé au retour. `SessionEngine` : `cameraOn`, `cameraOff`, `cameraSetResolution`, `onCameraOffer`, `onCameraState`, `onCameraNotice`. Pont : `apps/apple/src/camera.rs` (`kn_camera_*`), `KnPlatform::sends_camera` et `on_camera_offer`. | Mac : puce et menu de la barre, ⌃⌥⌘C, capsule, statistiques, réglage. iPhone, iPad : bouton et menu de la barre, capsule, tuile des statistiques, réglage. Apple TV : tuile du menu et choix de tvOS (`continuityDevicePicker`), ligne des statistiques, capsule. Réglage gardé par ordinateur (`camera1080`), offre retenue (`cameraOffered`) pour n'afficher le réglage qu'aux ordinateurs qui la proposent. | écrit, types vérifiés (KyberKit pour macOS, iOS et tvOS, les trois apps) ; pas compilé dans Xcode, pas essayé |
| Android | à venir dans cette série | | |
| Windows | à venir | | |
| Linux | à venir | | |

## Essais à faire

1. Station avec ce correctif face à une app qui demande la caméra : « Uplink "camera" ready » puis « Uplink "camera" connected » dans le journal de kycontroller ; sans l'outil à côté : « no sink on this computer ».
2. `kybernow-camera --test` sur chaque Station : la mire dans l'app Caméra de Windows 11, dans Zoom sous Windows 10, dans Photo Booth sur Mac, dans `ffplay /dev/videoN` et un navigateur sous Linux (avec et sans v4l2loopback).
3. Une vraie session : la caméra dans Teams, Zoom, Discord, OBS ; changer de caméra et de définition en session ; app mise en arrière-plan sur iPhone (image figée) puis revenue.
4. Un second appareil qui allume sa caméra pendant que le premier l'envoie : avis « {appareil} envoie déjà sa caméra à {nom}. ».
5. Profil changé en session, caméra allumée : la caméra repart d'elle-même (une image clé, pas de geste).
6. Station d'avant : l'entrée « Caméra » n'apparaît pas, rien d'autre ne change.
7. Windows : installation puis désinstallation (la caméra disparaît de la liste des caméras) ; mise à jour de la Station pendant qu'une app filme (DLL utilisée par Frame Server).
8. Mac : première activation (notification, accord dans les Réglages), puis mise à jour de l'app (extension remplacée).
