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
| `MEDIA` 0x02 | app → ordinateur | drapeaux (u8 : 1 image clé), pts en µs (u64, gros-boutiste), puis l'image encodée (Annex B, paramètres SPS/PPS/VPS avec chaque image clé) |
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

## Station Windows

À venir dans cette série : outil `kybernow-camera.exe` (décodage H.264/HEVC par FFmpeg, image NV12 dans une mémoire partagée nommée) ; Windows 11 : source de média COM inscrite par l'installateur, caméra créée par `MFCreateVirtualCamera` (durée de vie « système », pour tous les utilisateurs) et lue par Frame Server ; Windows 10 : filtre source DirectShow écrit par nous, lu dans le processus de l'app qui filme.

## Station Mac

À venir : extension caméra CoreMediaIO dans l'app Station, nourrie par son flux « sink ».

## Station Linux

À venir : v4l2loopback (`/dev/videoN`, « KyberNow Camera »), sinon un nœud PipeWire `Video/Source`.

## Les apps

À venir : capture et encodage matériel dans chaque app.

## Essais à faire

1. Station avec ce correctif face à une app qui demande la caméra : « Uplink "camera" ready » puis « Uplink "camera" connected » dans le journal de kycontroller ; sans l'outil à côté : « no sink on this computer ».
2. Un second appareil qui allume sa caméra pendant que le premier l'envoie : avis « {appareil} envoie déjà sa caméra à {nom}. ».
3. Profil changé en session, caméra allumée : la caméra repart d'elle-même (une image clé, pas de geste).
4. Station d'avant : l'entrée « Caméra » n'apparaît pas, rien d'autre ne change.
