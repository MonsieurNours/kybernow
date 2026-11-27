# Enregistrement des sessions

[English](enregistrement.en.md) · Français

L'appareil en session enregistre ce qu'il voit et entend dans un fichier MP4, sur l'ordinateur. Les textes et l'interface des apps sont dans [link-ui.md](link-ui.md), « Enregistrement de la session ».

## Les choix

Cadrés avec l'auteur le 26/11/2026 :

| Question | Choix |
|---|---|
| Où | Sur la Station, à partir de l'image et du son déjà encodés pour la session : **aucun réencodage**, aucune charge en plus pour la carte graphique. Toutes les apps en profitent, navigateur et téléviseurs compris. |
| Qui lance | L'appareil en session de l'hôte, et Admin. Un invité du partage le voit sans pouvoir l'arrêter. Pendant un prêt, pas d'enregistrement : un prêt qui commence l'arrête. |
| Prévenir | Une pastille rouge « Enregistrement · 02:14 » pour tout le monde dans la session, et un avis bref au début (ou en arrivant dans une session enregistrée). Pas de demande d'accord : on prévient. |
| Contenu | L'image de l'écran principal de la session et le son de l'ordinateur. KyberNow ne transmet pas de micro : il n'y en a pas. |
| Où va le fichier | Vidéos › KyberNow de l'utilisateur de l'ordinateur (Films › KyberNow sur Mac, `XDG_VIDEOS_DIR` › KyberNow sous Linux), modifiable dans Admin. Un dossier qui est (ou passe par) un lien ou une jonction est refusé : Kyber y écrit avec les droits du système. |
| Combien de temps | 30 jours par défaut (7, 30, 90 jours ou toujours, dans Admin) ; arrêt sous 2 Go libres. |
| Toujours enregistrer | Non : à la main seulement. |
| Permis | Oui par défaut (« Permettre l'enregistrement des sessions » dans Admin). |

## Comment ça marche

```
app (hôte)  ──POST /api/stream/recording──▶  Admin  ──POST /kybernow/recording──▶  Kyber (kycontroller)
     ▲                                          │                                          │
     └──── signe de vie : « recording » ◀───────┘          image de l'hôte + son ──▶ MP4 ──┘
```

1. **L'app** demande (`POST /api/stream/recording` `{"record": true, "device_id"}`), comme les autres appels de l'hôte. Admin vérifie que c'est bien l'appareil en session, que l'enregistrement est permis, qu'aucun prêt n'est en cours et qu'il reste 2 Go.
2. **Admin** donne à Kyber le dossier et le nom du fichier (`KyberNow 2026-11-26 14-32-05`, à l'heure de l'ordinateur) par sa route réservée (`POST /kybernow/recording`, jeton d'Admin, depuis l'ordinateur seulement).
3. **Kyber** (correctif 0070, contrôleur commun aux trois Stations) lit l'image de l'hôte de l'écran principal et le son partagé, **comme le ferait un invité** du partage : il ne les retient jamais (un disque lent perd des images et repart à l'image clé suivante, la session n'en souffre pas). Un fil à lui écrit un MP4 fragmenté : H.264 (`avc1`) ou HEVC (`hvc1`, 8 ou 10 bits, HDR compris) et Opus, un fragment par seconde, écrit au fur et à mesure. Un fichier coupé net (coupure de courant) se lit jusqu'à sa dernière seconde ; un fichier fermé porte sa durée et un index pour avancer dedans.
4. **Tout le monde** voit l'état dans son signe de vie : l'hôte (`POST /api/stream/heartbeat`) et les invités (`POST /api/share/beat`), champ `recording`. Le temps est compté par la Station (`elapsed`) : tous les appareils montrent le même.

Kyber ne demande rien à l'encodeur, sauf une image clé au début, après une perte, et toutes les 10 secondes (pour pouvoir avancer dans le fichier). Avec le rafraîchissement progressif (le réglage habituel), ces images clés sont les seules : elles sont un peu plus lourdes que les autres, une toutes les 10 s.

**Fin** : l'hôte ou Admin l'arrête ; la session de l'hôte finit (3 s sans son image) ; le disque passe sous 2 Go (Admin vérifie toutes les 2 s) ou l'écriture échoue ; un prêt commence ; le profil passe en PyroWave ou en AV1 ; Admin coupe l'enregistrement. Une image qui change de taille ou de format (un profil choisi en session) continue dans un nouveau fichier, `… (2).mp4`.

**Pas enregistrés** : PyroWave et AV1 (refusés au lancement, avec la raison ; arrêt si la session y passe) ; le curseur, que les apps dessinent elles-mêmes (il n'est pas dans l'image envoyée) ; les autres écrans d'une session à plusieurs écrans.

## Échanges

### L'app et Admin

`POST /api/stream/recording` `{"record": true|false, "device_id": "…"}` (compte de l'appareil, comme le partage) : rend l'état, ou un refus `409 {"error", "code"}` :

| `code` | Quand | Texte (link-ui) |
|---|---|---|
| `disabled` | coupé dans Admin | « L'enregistrement est coupé sur cet ordinateur (Admin › Enregistrements). » |
| `not_in_session` | l'appelant n'est pas l'appareil en session | « Seul l'appareil en session peut enregistrer. » |
| `no_session` | personne en session | « Aucune session à enregistrer. » |
| `lent` | ordinateur prêté | « {prénom} utilise l'ordinateur. » |
| `codec` | PyroWave ou AV1 | « PyroWave et AV1 ne s'enregistrent pas : passez en HEVC ou H.264. » |
| `disk_full` | moins de 2 Go | « Moins de 2 Go libres sur l'ordinateur : impossible d'enregistrer. » |
| `folder` | dossier impossible à créer | « Le dossier des enregistrements est inaccessible (Admin › Enregistrements). » |
| `unavailable` | Kyber trop ancien (404) | « Kyber n'enregistre pas encore sur cet ordinateur : mettez à jour KyberNow Station. » |
| `failed` | autre | « L'enregistrement n'a pas pu commencer. » |

L'état (signe de vie de l'hôte et de l'invité, réponse ci-dessus) :

```json
{"allowed": true, "active": true, "elapsed": 134, "started_at": 1764166325000,
 "last_end": {"reason": "stopped", "at": 1764166000000}}
```

`last_end.reason` : `stopped`, `ended` (fin de la session), `write`, `disk_full`, `codec`, `lent`. Une Station qui n'enregistre pas n'envoie pas `recording` : les apps ne montrent rien.

### Admin

| Route | |
|---|---|
| `GET /api/recording` | réglages (`allowed`, `folder`, `custom_folder`, `default_folder`, `keep_days`), état, appareil qui l'a lancé (`by`), appareil en session (`session`), place libre (`free`), fichiers (`name`, `size`, `modified`, `duration_ms` lu dans le fichier, `in_progress`) |
| `PUT /api/recording/settings` | `{allowed?, folder? ("" : par défaut), keep_days? (7, 30, 90, 0 = toujours)}` |
| `POST /api/recording` | `{record}` : Admin lance ou arrête l'enregistrement de la session en cours |
| `GET /api/recording/files/{nom}` | le fichier, par morceaux (`Range`, pour le lecteur de la page) ; `?download=1` pour l'enregistrer |
| `DELETE /api/recording/files/{nom}` | supprime (pas celui qui s'écrit) |

Réglages d'Admin, `[recording]` : `allowed` (vrai par défaut), `folder` (absent : par défaut), `keep_days` (30). Toutes les heures, et à chaque réglage changé, Admin efface les fichiers plus vieux que la durée choisie : seulement ceux dont le nom commence par « KyberNow » (jamais une autre vidéo du dossier), jamais celui qui s'écrit. Admin redémarré pendant un enregistrement le reprend (il relit Kyber).

Le fil d'Admin dit « {appareil} enregistre la session », « Enregistrement terminé ({durée}) », « Enregistrement arrêté : disque plein ».

### Admin et Kyber (correctif 0070)

`POST /kybernow/recording` `{"record": true, "dir": "…", "name": "…"}` ou `{"record": false}` ; `GET /kybernow/recording` : `{"active", "started_at_ms", "display", "files", "bytes", "last_end": {"reason", "at_ms"}}`. Refus `409 {"code"}` : `no_session`, `codec`, `folder`. Comme les autres routes `/kybernow`, seulement depuis l'ordinateur et avec le jeton d'Admin.

## Selon le système

| | Windows | Mac | Linux |
|---|---|---|---|
| Dossier par défaut | `C:\Users\{compte}\Videos\KyberNow` (le profil de la session ouverte : Admin tourne en service) | `~/Movies/KyberNow` | `XDG_VIDEOS_DIR/KyberNow` (`~/.config/user-dirs.dirs`), sinon `~/Videos/KyberNow` |
| Place libre | `GetDiskFreeSpaceExW` | `statvfs` | `statvfs` |
| Écrit par | Kyber (`kycontroller`), dans le dossier de l'utilisateur | Kyber, avec le compte de l'utilisateur | Kyber, avec le compte de l'utilisateur |

Un dossier Vidéos déplacé sous Windows (OneDrive, autre disque) n'est pas suivi : choisissez-le dans Admin.

## Essais

- **Fait** : l'écrivain MP4 sur de vrais flux (x264 1080p, HEVC VideoToolbox 8 bits 1080p et 10 bits 1440p, Opus) : lus et parcourus par ffprobe/ffmpeg et par AVFoundation (durée, pistes, image décodée), essais unitaires (jeux de paramètres, durées Opus, fragments, durée et index à la fermeture) ; `cargo test` de kycontroller, d'Admin et du cœur.
- **À essayer sur une Station** (Windows, Mac, Linux) : lancer depuis une app, depuis Admin ; pastille chez un invité ; fichier lu dans QuickTime, le lecteur de Windows, VLC, la page d'Admin ; changement de profil en session (`… (2).mp4`) ; passage en PyroWave (arrêt) ; disque presque plein ; prêt qui commence ; Admin redémarré pendant un enregistrement ; dossier par défaut sous Windows (service sous SYSTEM) ; HDR ; 5.1.
