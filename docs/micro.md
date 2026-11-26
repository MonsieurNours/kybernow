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

Kyber 0.29 n'a aucun son montant : seul un type « micro » existe dans la capture de la Station (txproto), sans usage. Tout ce qui suit est à nous (correctifs 0080).

## Le son

Opus, mono, 48 kHz, trames de 10 ms (480 échantillons), mode voix (`OPUS_APPLICATION_VOIP`), 32 kbit/s, correction d'erreurs intégrée d'Opus (FEC, pertes prévues 10 %), sans DTX. Encodé par le cœur pour toutes les apps (`apps/core/src/micro.rs`) : l'app ne fait que capter et donner ses échantillons, à n'importe quelle fréquence (le cœur ramène à 48 kHz).

Le transport est celui du son de l'ordinateur, dans l'autre sens : un point d'accès audio Kymux de plus sur la connexion de la session, avec le protocole audio de la session (`UnreliableFec`). Le son descendant n'est pas touché ; si la redondance du son descendant change (chantier du transport), le son montant reprendra la même.

## Protocole (correctifs 0080)

**Demande** (`POST /kymux/start_session`, `kyctl-common`) : `uplink: { microphone: <id> }`, l'identifiant du point d'accès audio que l'appareil a enregistré juste avant sur la connexion Kymux. Absent : rien n'est proposé (comme avant). Une Station d'avant ignore le champ.

**Réponse** : `uplink: { microphone: true }` quand l'ordinateur le prend, sinon `microphone: false` et `microphone_refused` :

| Raison | Sens | Ce que dit l'app |
|---|---|---|
| `not_allowed` | invité de KyberNow Share sans le droit | « L'hôte ne laisse pas les invités parler. » |
| `no_device` | pas de micro virtuel sur l'ordinateur (VB-CABLE pas installé, pilote KyberNow.driver absent, PipeWire injoignable) | « L'ordinateur n'a pas de micro virtuel : voyez Admin › Image et son. » |
| `unsupported` | la Station ne peut pas le prendre (aide absente, transport WebSocket) | « Cet ordinateur ne reçoit pas le micro. » |
| `old_station` (dit par kyclient : réponse sans `uplink`) | Station d'avant le micro | « Mettez à jour la Station pour envoyer le micro. » |

Les paquets : un paquet de codec d'abord (`opus`, 480 échantillons par paquet), puis l'en-tête Opus (`OpusHead`, mono, 48 kHz) en paquet de configuration, puis un paquet par trame avec l'heure de capture en microsecondes. **Piège** : le récepteur audio de Kymux ne délivre rien d'un groupe avant son paquet de configuration (le son de l'ordinateur commence de la même façon) ; sans l'en-tête, rien n'arriverait. « L'appareil a coupé son micro » part en paquet de configuration d'un octet (sur le flux fiable de Kymux, il arrive toujours) : l'ordinateur joue du silence tout de suite, sans attendre que son tampon se vide. Rien n'attend : un paquet que la connexion ne peut pas prendre est jeté (10 en file au plus, 100 ms).

**Droit** (kycontroller) : l'hôte toujours ; un invité du partage ou l'ami qui emprunte seulement avec `kn_mic: true` dans son jeton de Kyber. Admin le met pour le prêt, et pour les invités quand l'hôte les laisse parler. En session, `POST /kybernow/role` accepte `mic` (avec la même révision que le rôle) : le droit repris, les paquets sont jetés et le micro virtuel joue du silence ; rendu, ils repassent.

**kyclient** (`uplink.rs`) : `Uplink`, gardé par le cœur pour toute la session (il survit aux relances de l'image) et donné à chaque `StreamingSessionConfig` (`uplink`). `set_offer(true)` : proposé au prochain démarrage de l'image ; `set_codec`, `send(paquet, heure)` (`false` tant que l'ordinateur ne le prend pas), `state()` : `Idle`, `Offered`, `Open`, `Refused(raison)`.

**kycontroller** (`uplink.rs`) : vérifie le droit, demande à l'aide de la Station si le micro virtuel est là (`kybernow-micro --check`, 3 s au plus), se relie au point d'accès de l'appareil, lance l'aide pour la session et lui passe chaque paquet sur son entrée standard, un enregistrement par paquet (longueur sur 2 octets, puis `C` codec, nombre d'échantillons et voies ; `M` heure et paquet ; `S` silence). L'entrée se ferme avec la session ; l'aide est arrêtée une seconde plus tard.

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
