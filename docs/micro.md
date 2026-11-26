# Micro de l'appareil vers l'ordinateur

[English](micro.en.md) · Français

Parler dans un jeu, un appel ou une visio lancés sur l'ordinateur, avec le micro de l'appareil qui s'y connecte. Les textes et l'interface, pour toutes les apps, sont dans [link-ui.md](link-ui.md), « Micro ». Ici : le protocole, ce que fait chaque partie et où en sont les essais.

Choix de l'auteur (26/11/2026) :

- coupé à la première session ; « Micro » dans la barre (ou le menu) de session l'allume ; le choix est retenu pour chaque ordinateur ;
- l'ordinateur reçoit le son dans un micro virtuel : **VB-CABLE** sur Windows (installé avec la Station), l'appareil **« Micro KyberNow »** du pilote KyberNow.driver sur Mac, une **source PipeWire** sur Linux ; pendant l'envoi, il devient le micro par défaut de l'ordinateur ;
- prêt du PC : l'ami parle comme le propriétaire ; invités de KyberNow Share : seulement si l'hôte les laisse parler (coupé par défaut) ;
- réduction de l'écho et du bruit : celle du système de l'appareil, allumée par défaut, réglable ;
- toutes les apps qui ont un micro : Mac, iPhone et iPad, Android, Windows, Linux (bureau, Steam Deck, Raspberry Pi avec un micro USB), client web ; ni l'Apple TV (son micro est réservé à Siri) ni les téléviseurs LG ;
- réglage « Micro » dans les Réglages de l'app (celui du système ou un micro choisi) et indicateur dans la barre de session tant qu'il est envoyé.

Kyber 0.29 n'a aucun son montant : seul un type « micro » existe dans la capture de la Station (txproto), sans usage.

## Le son

Opus, mono, 48 kHz, trames de 10 ms (480 échantillons), mode voix (`OPUS_APPLICATION_VOIP`), 32 kbit/s, correction d'erreurs intégrée d'Opus (FEC, pertes prévues 10 %), sans DTX. Encodé par le cœur pour toutes les apps (`apps/core/src/micro.rs`) : l'app ne fait que capter et donner ses échantillons, à n'importe quelle fréquence (le cœur ramène à 48 kHz).

Le transport est le **flux montant commun** de la caméra et du micro (correctifs 0085, écrits avec la session webcam : [webcam.md](webcam.md), « Le flux montant ») : un point de données kyproto par type, un flux QUIC fiable à lui, à part de l'image, du son et des entrées. Le son descendant n'est pas touché. Un flux fiable attend ce qui se perd : avec des trames de 10 ms, une perte en Wi-Fi retarde d'un aller-retour ; le tampon de la Station (60 ms au plus, le reste jeté) l'absorbe. À mesurer ; si c'est trop, une voie non fiable pour le micro seul (le correctif d'abord écrit ici le faisait sur le protocole audio de Kymux, `UnreliableFec`).

## Protocole (correctif 0085, type `microphone`)

- **Départ de l'image** : le cœur demande `uplink: ["camera", "microphone"]` (le micro dès que l'app en a un, même coupé, et aussi pour un invité) ; la Station rend un point de données pour le micro quand `kybernow-micro` est à côté de kycontroller. Pas de flux : « Mettez à jour la Station pour envoyer le micro. » pour l'hôte, « L'hôte ne laisse pas les invités parler. » pour un invité ou l'ami qui emprunte.
- **Allumer** : `START` + `{"codec": "opus", "rate": 48000, "channels": 1, "frame": 480, "bitrate": 32000, "device": "…"}` ; kycontroller lance `kybernow-micro --uplink` et répond `{"state": "started"}`. Puis un `MEDIA` par trame : drapeaux (0), heure de capture en µs, paquet Opus. **Couper** : `PAUSE` (l'aide joue du silence tout de suite, reste prête). Image relancée : `START` repart sur le nouveau flux.
- **Réponses** (`STATUS`, JSON) : `started` → « Micro » allumé ; `failed` avec `reason` `no_device` (écrit par l'aide quand le micro virtuel manque) → « L'ordinateur n'a pas de micro virtuel : voyez Admin › Image et son. » ; autre refus → « Cet ordinateur ne reçoit pas le micro. »
- **Droit et nombre** : 0085 refuse les invités et n'accepte qu'un appareil à la fois par type. Pour le micro, les choix de l'auteur demandent l'ami qui emprunte et les invités que l'hôte laisse parler (`kn_mic` dans leur jeton, déjà posé par Admin ; `POST /kybernow/role` avec `mic` en session), et plusieurs micros à la fois (le système les mélange). Proposé à la session principale le 26/11 : un correctif à nous posé après 0085 (`0081-kycontroller-micro-droits`), pas encore écrit.

## L'aide de la Station : `kybernow-micro`

`server/micro` (Rust), installée à côté de kycontroller (`KYBERNOW_MICRO` pour une autre). Une par session qui envoie son micro ; plusieurs (l'hôte et ses invités) se mélangent dans le micro virtuel, le système fait le mélange.

- **Décodage** : libopus ; tampon de gigue de 20 ms visé, 60 ms au plus (au-delà, le plus ancien est jeté) ; une trame perdue est refaite par la correction d'erreurs du paquet suivant s'il est là, sinon masquée par Opus (PLC) ; au silence quand rien n'arrive depuis 60 ms.
- **Micro par défaut** : au premier paquet, si le micro par défaut de l'ordinateur n'est pas déjà le micro virtuel, elle le note et met le micro virtuel par défaut ; en partant, elle remet le précédent s'il n'a pas changé entre-temps. Une seconde aide (un invité) trouve le micro virtuel déjà par défaut et ne touche à rien.
- **Windows** : le son est joué en WASAPI (partagé, 48 kHz) dans « CABLE Input » de VB-CABLE ; les apps de l'ordinateur l'entendent dans « CABLE Output ». `--check` : la sortie « CABLE Input » existe et est active.
- **Mac** : joué en Core Audio dans l'entrée cachée « KyberNow Micro (entrée) » du pilote KyberNow.driver ; les apps l'entendent dans **« Micro KyberNow »**, un appareil d'entrée seul (le pilote garde l'anneau entre les deux, comme sa sortie et son retour pour le son de la session).
- **Linux** : au besoin, l'aide crée par `pactl` une sortie nulle cachée (`kybernow_micro_entree`) et une source qui reprend son retour, **« Micro KyberNow »** (`kybernow_micro`, `module-remap-source`), puis y joue par `pacat` (PipeWire, par pipewire-pulse). `--check` : `pactl info` répond.

## VB-CABLE (Windows)

Le pilote de VB-Audio Software, signé et éprouvé ; on ne peut pas écrire de pilote de son à nous sans la signature de Microsoft (en attente, voir la mémoire « Authenticode »). VB-CABLE est un **donationware** : VB-Audio permet de le livrer tel quel avec une autre app, installation silencieuse comprise, à condition que l'utilisateur puisse savoir que c'est un produit de VB-Audio et le soutenir. D'où, partout où on le montre (installateur, Admin › Image et son, cette page) : « VB-CABLE, de VB-Audio Software, est offert en donationware : vb-cable.com. » Le pack de VB-Audio (`VBCABLE_Driver_Pack*.zip`, extrait) se place dans `third_party/payload/vbcable/` avant de construire l'installateur, qui l'installe s'il manque (`VBCABLE_Setup_x64.exe -i -h`, après `kybernow-micro --check`) ; sans lui, l'étape est sautée. Ne jamais renommer ses appareils.

## Les apps

Chaque app capte le micro à sa façon et donne ses échantillons au cœur (`Session::push_microphone`, flottants mono, n'importe quelle fréquence). Le cœur encode, envoie, et dit à l'app l'état (`Platform::on_microphone`) : `Off`, `On`, `Refused(raison)`, `Waiting` (proposé, réponse pas encore venue).

| App | Capture | Écho et bruit |
|---|---|---|
| Mac | AVAudioEngine (nœud d'entrée) | traitement voix d'Apple (`setVoiceProcessingEnabled`) |
| iPhone, iPad | AVAudioEngine, session `playAndRecord` en mode `voiceChat` tant que le micro est allumé | traitement voix d'Apple |
| Android | `AudioRecord`, source `VOICE_COMMUNICATION` | `AcousticEchoCanceler` et `NoiseSuppressor` quand l'appareil les a |
| Windows | WASAPI, catégorie « Communications » | traitement du système quand le pilote du micro en a |
| Linux, Deck, Pi | SDL2 (capture), rôle « communication » | module d'écho de PipeWire s'il est chargé ; sinon rien |
| Client web | `getUserMedia`, Opus par WebCodecs | `echoCancellation`, `noiseSuppression` du navigateur |

Réglages de l'app : « Micro » (« Celui du système » ou un micro de l'appareil) et « Réduire l'écho et le bruit » (allumé). Réglages de l'ordinateur › Son : « Micro », le même interrupteur que celui de la session, retenu pour cet ordinateur.

## Où en est-on

Écrit le 26/11/2026, branche `claude/micro-5db32e` :

- correctifs 0080 (kycontroller, kyclient), compilés et testés (`cargo test -p kycontroller uplink`, `cargo test -p kyclient --features kymux uplink`) ;
- voir les sections suivantes au fil des commits ; ce qui n'est pas écrit est dans « Reste à faire ».

### Reste à faire

- Essayer de bout en bout : aucune session n'a encore envoyé de micro.
