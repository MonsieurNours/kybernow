# Reprise après une perte d'image

[English](reprise-apres-perte.en.md) · Français

Quand une image se perd en route (paquets perdus au-delà de la correction
d'erreurs), le décodeur de l'app n'a plus la référence des images suivantes.
Jusqu'ici, l'app demandait une image clé (`POST /force_idr`) : une image
complète, cinq à dix fois plus lourde qu'une autre, qui arrive au moment où
le réseau a le moins de place, et une chute de qualité le temps que le débit
s'en remette. Désormais, la Station répare la perte **sans image clé** quand
son encodeur le sait : l'image suivante ne s'appuie que sur des images que
l'app a bien reçues.

## Ce qui se passe

1. **La Station numérote chaque image** (HEVC et H.264) : un petit SEI
   « user data unregistered » (UUID `KyberNow:reprise`, une trentaine
   d'octets) devant la première tranche, avec un numéro qui augmente de un à
   chaque image. Kyber ne dit pas à l'app qu'une image s'est perdue (son
   récepteur ne fait que compter les paquets manquants) : le cœur des apps
   (`apps/core/src/media.rs`) voit le trou au numéro sauté et le donne au
   décodeur comme une perte (`Packet::Hole`).
2. **Le décodeur de l'app écarte la suite** (elle s'appuie sur l'image
   perdue) et le cœur demande la reprise : `POST /recover {"last_pts": N}`,
   N étant le pts de la dernière image reçue avant la perte.
3. **La Station répare à l'image suivante**, du meilleur moyen que son
   encodeur connaît :
   - **invalidation** (NVENC) : les images plus récentes que N sont retirées
     des références, l'image suivante s'appuie sur N ou une image d'avant,
     encore dans la mémoire du décodeur (4 images au moins) ;
   - **référence longue** (NVENC, VideoToolbox) : la Station garde des images
     de référence longue (NVENC : deux, une marquée toutes les 250 ms ;
     VideoToolbox : celles qu'il choisit), et l'image suivante s'appuie sur la
     plus récente qui ne dépasse pas N ;
   - sinon, **une image clé**, comme avant.
4. **L'image qui répare le dit** : son SEI porte aussi la borne, le pts de
   l'image la plus récente dont elle dépend. Le décodeur qui a reçu jusque-là
   reprend dessus sans être refait ; un autre (une autre app sur une image
   partagée, ou qui a perdu plus) attend, et redemande.
5. Rien ne vient en 300 ms : le cœur demande une image clé, puis relance le
   flux à 1,5 s, comme avant.

**Avec le canal de retour** ([canal-de-retour.md](canal-de-retour.md),
correctifs 0050 à 0052), la Station n'attend pas l'app : le message `Lost`
nomme les images perdues, et kycontroller (correctif 0061) demande aussitôt
la réparation jusqu'à l'image d'avant la première perdue (l'encodeur prend
la plus récente qu'il a envoyée jusque-là). La demande de l'app (étape 2)
reste le filet de sécurité, pour une app ou une Station sans canal de
retour, et une session à plusieurs écrans.

Plusieurs demandes avant l'image suivante (plusieurs apps sur une image
partagée) : la plus ancienne l'emporte, elle répare tout le monde. La même
perte signalée deux fois (canal de retour, puis l'app) : la seconde demande,
que la réparation déjà partie satisfait, est ignorée pendant 200 ms.

Limite : une perte n'est vue qu'à l'arrivée de l'image suivante (numéro
sauté, ou `Lost` qui part au paquet suivant). Sur un écran immobile, la
dernière image perdue attend la prochaine image de la Station.

## Encodeurs

| Encodeur | Invalidation | Référence longue | Sinon |
|---|---|---|---|
| NVENC (Station Windows, Linux) | oui, si la carte le sait (`NV_ENC_CAPS_SUPPORT_REF_PIC_INVALIDATION`) | 2, si la carte en garde | image clé |
| VideoToolbox (Station Mac) | non | oui (`EnableLTR`, macOS 12 et suivants) | image clé |
| AMF, QSV, VA-API, x264 | non | non | image clé |
| Moteur libavconv | — | — | image clé |

Seul le **HEVC** est réparé sans image clé : en H.264, les décodeurs ne
prennent pas tous le saut de `frame_num` des images écartées. Le H.264 est
tout de même numéroté (pertes vues tout de suite, image clé demandée sans
attendre que le décodeur se plaigne). L'AV1 et PyroWave ne changent pas.

Réglage : `[kyavserver] loss_repair = false` coupe la réparation sans image
clé (les images restent numérotées) ; vrai par défaut.

## Apps

Le cœur (`media.rs`) lit les numéros et la borne pour toutes les apps, et
`ResumeGate` dit à chaque décodeur s'il peut reprendre : il faut qu'il ait
reçu, sans rien perdre avant, l'image de la borne. Le décodeur n'est jamais
refait pour une reprise, et une image écartée n'est jamais montrée.

| App | Décodeur |
|---|---|
| Mac, iPhone, iPad, Apple TV | VideoToolbox (`VideoDecoder.swift`, KyberKit) |
| Windows | D3D11VA par FFmpeg (`apps/windows/src/video.rs`) |
| Android | MediaCodec (`apps/android/jni/src/video.rs`) |
| Linux, Raspberry Pi | VA-API ou V4L2 par FFmpeg (`apps/linux/src/device/video.rs`) |
| Téléviseurs LG | NDL (`apps/webos/src/device/media.rs`) |

Le client web n'est pas concerné : il garde l'image clé (une image perdue
s'y voit à l'erreur de WebCodecs).

Une Station sans le correctif répond 404 à `/recover` : l'app demande alors
une image clé, comme avant, et ne redemande plus `/recover` à cette Station.
Ses images ne sont pas numérotées : rien ne change pour elle.

## Comment c'est fait

| Partie | Fichiers |
|---|---|
| FFmpeg | `server/kyber-patches/0060-kymedia-ffmpeg-reprise.patch` : `0060-avcodec-reprise-sans-image-cle.patch` posé dans le wrap après 0017 (`nvenc.c`, `nvenc.h`, `nvenc_h264.c`, `nvenc_hevc.c`, `videotoolboxenc.c`). Options `ref_recovery` (à l'ouverture), `ref_recover` (pour l'image suivante), `ref_method` et `ref_bound` (lues par l'appelant). |
| txproto | `0060-txproto-reprise.patch` : `kn_recovery.h` (nouveau), commande `recover` (`last_pts`), `tx_encoder_recover`, numéro et borne dans le SEI. |
| kyavservice | `0060-kyavservice-reprise.patch` : `Command::VideoRecover { last_pts }`, `ref_recovery` donné à NVENC et VideoToolbox en HEVC, `[kyavserver] loss_repair` ; moteur libavconv : image clé. |
| kycontroller | `0060-kycontroller-reprise.patch` : `POST /recover`, même portée que `/force_idr` ; un invité du partage répare sur les images qu'il reçoit ; demandes regroupées par flux partagé. `0061-kycontroller-reprise-canal-de-retour.patch` (après 0052, qu'il demande) : les `FeedbackEvent::Lost` réparés aussitôt, session à un flux d'image seulement. |
| kyclient (apps) | `apps/macos/kyber-patches/0060-kyclient-reprise.patch` : `Client::recover(last_pts)`, repli sur `/force_idr` (404). |
| Cœur | `apps/core/src/media.rs` (`image_mark`, `StreamLoss`, `ResumeGate`), `session.rs` (`repair`). |

## À essayer sur le matériel

Rien de tout cela n'a encore tourné face à une vraie Station (écrit,
compilé, tests du cœur seulement) :

1. **NVENC** (Station Windows, carte NVIDIA, HEVC) : couper le Wi-Fi une
   fraction de seconde, ou perdre des paquets (`clumsy` sous Windows), puis
   lire le journal de kyavserver. Les lignes de FFmpeg sont au niveau
   VERBOSE, écrit seulement avec `txproto_log_level = "debug"` (moteur
   txproto) ou `ffmpeg_log_level = "debug"` (moteur libavconv) dans la
   section `[kyavserver]` de `kyber_config.toml`. Au démarrage du flux :
   « Repairs without keyframe: DPB …, invalidation …, … long-term
   references » (sinon « No invalidation nor long-term reference: losses
   are repaired by IDR ») ; à chaque perte : « Repair up to {pts} at {pts}:
   invalidation » (ou « long-term reference », ou « IDR »). Vérifier que l'image reprend net, sans image
   corrompue, et que le débit ne fait plus de pic. À vérifier aussi : la
   carte accepte-t-elle l'invalidation avec le rafraîchissement progressif
   (le réglage par défaut) ?
2. **VideoToolbox** (Station Mac) : même essai, même niveau de journal.
   VideoToolbox n'écrit qu'une ligne, au démarrage du flux : « Repairs
   without keyframe: long-term references » ; s'il refuse `EnableLTR` en
   HEVC, « No long-term references (…): losses are repaired by keyframes »
   (niveau WARNING, toujours écrit), et tout passe par l'image clé. Rien
   n'est écrit à chaque réparation : le compteur d'images clés de l'app
   (point 3) le dit.
3. **Chaque app** : « image perdue après le pts … : reprise demandée » au
   journal (écrit par le cœur, dans toutes les apps), puis
   le compteur d'images clés qui n'augmente plus à chaque perte
   (Statistiques). Surtout les décodeurs matériels qu'on ne connaît pas de
   l'intérieur : MediaCodec (selon le fabricant), NDL (LG), V4L2 (Raspberry
   Pi).
4. **Image partagée** (KyberNow Share) : deux apps sur la même image, l'une
   qui perd des paquets ; l'autre ne doit rien voir.
5. **Station ancienne** : une app à jour face à une Station sans le
   correctif garde l'image clé (404 une fois, puis `/force_idr`).
6. **Canal de retour** : avec 0050 à 0052, la réparation part sur `Lost`
   (journal de kycontroller : « picture lost from … , repaired », niveau
   debug), avant la demande de l'app, qui doit être ignorée. Écran immobile :
   la Station renvoie-t-elle une image, pour qu'une perte sur la dernière
   soit vue ?
