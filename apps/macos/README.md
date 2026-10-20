# KyberNow pour macOS

[English](README.en.md) · Français

Client de streaming de KyberNow : on retrouve son ordinateur (serveur Kyber 0.29 + console KyberNow) en plein écran sur le Mac, avec l'image, le son, le clavier, la souris et les manettes. Identité v2 ([charte](../../brand/v2/README.md)) ; maquettes : [Claude Design](https://claude.ai/artifact/YGA7unwqztvuA26CMtdbv8) (accueil, premier contact, passage, session). Le code vidéo, audio et la détection des capacités n'utilisent pas AppKit : l'app Apple TV ([apps/tvos](../tvos/README.md)) les reprend tels quels, avec le modèle (`Model.swift`) et la découverte Bonjour (`Discovery.swift`). Les quelques différences sont sous `#if os(...)` ; après un changement dans KyberKit, compiler aussi l'app Apple TV.

## Ce que fait l'app

- **Accueil** : l'image de l'ordinateur affiché en plein cadre (zoom très lent, désaturée hors ligne), son nom en grand, son système, sa carte graphique et son adresse, l'écran virtuel qui l'attend, le bouton principal et le profil ; à droite, la latence de la dernière session (médiane, 95e centile, durée) sur la règle de trame ; en bas, les autres ordinateurs avec leur image (un clic les affiche), ceux annoncés sur le réseau (Bonjour, ils s'allument en apparaissant) et l'ajout. En ligne ou non : sondage de la console toutes les 8 s.
- **Image d'un ordinateur** (`HostImagery`), dans cet ordre : la dernière image de sa dernière session, photographiée à la fin (`KyberClient.snapshot`, 1600 pixels de large, JPEG dans `~/Library/Application Support/KyberNow/Images`), désactivable par ordinateur (« Garder la dernière image », qui l'efface) ; sinon un aperçu à jour demandé à sa console (`GET /api/stream/preview`, une fois par minute au plus, arrivée en balayage à 45°) ; sinon sa **signature**, deux lueurs dont la teinte et la place sont tirées de l'empreinte de sa console, avec le symbole immense en filigrane. L'accueil dit laquelle il montre (« Image affichée »).
- **Ajouter un ordinateur** : sur le réseau local (annonce Bonjour `_kybernow._tcp` de la console ; celles marquées `kyber=0`, console sans serveur Kyber derrière, ne sont pas proposées), par adresse et port de la console, ou par QR code (caméra du Mac) ou lien `kybernow://adresse:port?name=…&fp=sha256:…` affichés dans la console, espace Accès. L'empreinte du certificat de la console est vérifiée : automatiquement si le lien ou l'annonce la donne, sinon à l'œil (les quatre premiers groupes). Puis le compte, vérifié auprès de la console et gardé dans le Trousseau. Un lien `kybernow://` ouvert ailleurs ouvre directement l'ajout.
- **Réglages par ordinateur** : profils Jeu, Bureau et Max (ajuster un réglage rend le profil « Personnalisé ») ; écran de l'ordinateur (comme cet écran, au besoin « au plus 4K », taille de la fenêtre, sur mesure, ou ses écrans tels quels) ; format (H.264, HEVC, AV1), 4:4:4, synchro verticale, débit de 5 à 4 294 Mbit/s avec avertissements ; transport (rapide ou fiable), reconnexion ; ⌘ envoyée comme Ctrl, souris des jeux, manette vue par l'ordinateur (Xbox 360 ou fidèle), presse-papiers partagé, son (Auto, stéréo, 5.1, 7.1, aucun) ; carte « Ce Mac ».
- **Débit** : celui d'un profil est calé sur la 4K à 60 Hz et suit le nombre de pixels par seconde de la session (puissance 0,75, entre 0,4 et 3 fois). Par exemple, le profil Max à 300 Mbit/s part à environ 460 Mbit/s en 5K à 60 Hz. Un débit réglé à la main reste tel quel. Les réglages affichent le débit réel.
- **Pointeur** : celui du Mac prend la forme de celui de l'ordinateur, à la taille d'un pointeur du Mac. La case de Windows (32 pixels à 100 %, 48 à 150 %, 64 à 200 %) est ramenée à 32 points, et la pointe suit la même échelle. Un masquage, ou une forme entièrement transparente (lecteurs vidéo), ne le fait jamais disparaître : il garde la dernière forme.
- **Rien ne reste enfoncé** : quand la fenêtre perd le clavier (⌘Tab, ⌘Espace, Spotlight, autre app au premier plan), l'app relâche sur l'ordinateur ce qu'elle y tient (touches, modificateurs, boutons de souris) et oublie son propre état. Pas de `flush_keys` de kynput ici : il relâcherait aussi les manettes, qui, lues par SDL, continuent de parler à l'ordinateur sans la fenêtre et ne renverraient pas leur état (il reste envoyé à la fin de la session). Quand la fenêtre retrouve le clavier, elle renvoie les modificateurs vraiment tenus à cet instant (`NSEvent.modifierFlags`, ⌘ toujours enfoncée après un ⌘Tab) ; un modificateur relâché est relâché sur l'ordinateur quel que soit le côté (gauche ou droit) qu'il tenait. Verrouillage majuscule : envoyé en appui + relâchement, comme une frappe, jamais tenu.
- **Paquets refusés** (`InputPipeline`) : la file d'envoi de kynput tient 64 paquets et jette sans retour, alors que le protocole est en deltas (un relâchement jeté = touche collée jusqu'à la fin de la session). L'app compte les refus (journal `entrées : … refusé`, puis le total à la fin de la session). Les paquets qui ne se rattrapent pas (touches, clics, boutons, branchements et débranchements de manette, retour au repos d'un axe, texte, `flush_keys`) sont réessayés toutes les 3 ms depuis une petite file de secours, dans l'ordre, jusqu'à 200 ms ; les autres (positions, déplacements, molette, axes en mouvement) partent une fois, et pas du tout tant que la file de secours n'est pas vide (ils doubleraient ce qui attend : le retour au repos d'un axe, puis sa nouvelle valeur). Les relâchements que `GamepadGate` injecte à la capture réarment leur autorisation à chaque essai : le hook la consomme avant que la file d'envoi se prononce, et un paquet qu'il écarte compte comme parti. Les manettes lues par SDL ne passent pas par là (kynput les envoie lui-même) : `GamepadGate` écarte seulement les axes qui n'ont pas bougé (au 1/512 de la course), ce qui calme une manette Bluetooth bavarde.
- **Clavier Apple ISO** (`Keymap`) : sur un clavier Apple européen (une touche de plus près de ⇧ gauche), le Mac rapporte la touche sous Échap (`@#` sur un AZERTY, `§±` ailleurs) comme `kVK_ISO_Section`, usage HID 0x64, et celle près de ⇧ (`<>`) comme `kVK_ANSI_Grave`, usage 0x35 : l'inverse de ce qu'attend l'ordinateur (`@` donnait `<`, confirmé le 2026-10-20 sur un MacBook AZERTY ; quirk `hid-apple`, SDL fait le même échange). Les deux usages sont échangés quand le type du clavier de l'événement (`kCGKeyboardEventKeyboardType`, puis `KBGetLayoutType`) est ISO ; un clavier ANSI branché en même temps garde ses touches.
- **Souris : Auto, Bureau ou Jeu** (réglages, barre de session, ⌃⌥⌘M pour basculer entre bureau et jeu).
  - En mode jeu, le pointeur de l'ordinateur, quand il est visible (menus, inventaires), est dessiné par l'app à la place que l'ordinateur annonce (serveur avec `kyber-patches/0006`, 60 positions par seconde). La forme n'est reconstruite que si elle change.
  - En mode jeu, le pointeur du Mac est figé et caché, et seuls les déplacements bruts partent (GCMouse, sans l'accélération de macOS ; déplacements AppKit sans GCMouse). La souris redevient absolue, pointeur replacé au même endroit, à l'ouverture de la barre ou quand la fenêtre perd le focus.
  - Auto (par défaut) passe en mode jeu quand un jeu est en plein écran sur l'ordinateur et cache son pointeur. La console le dit (`GET /api/stream/foreground`, interrogée chaque seconde) : fenêtre au premier plan qui couvre tout l'écran, ou plein écran exclusif DirectX. Les navigateurs, lecteurs vidéo, diaporamas et visioconférences en plein écran n'en font pas partie. Un pointeur remontré moins de 0,6 s (menus, clignotement de Windows) ne fait pas quitter le mode jeu.
  - Le seul masquage du pointeur ne suffit pas à déclencher le mode jeu : sur un ordinateur sans souris branchée, Windows le cache et le remontre toutes les demi-secondes, même sur le bureau.
- **Présentation à la console** : en début de session, l'app donne à la console un identifiant stable (tiré à la première ouverture), le nom du Mac, son système, sa version et le profil ; puis sa latence médiane avec chaque signal de vie, et celle de toute la session à la fin. La console en tire ses appareils connus et son fil d'événements. Si la console demande la fin de la session (« Mettre fin à la session », dans la console ou l'icône de l'ordinateur), l'app l'apprend au signal de vie suivant et se déconnecte comme avec Quitter.
- **Presse-papiers partagé** : texte, dans les deux sens, jusqu'à 60 Ko (`ClipboardBridge`, protocole de kynput, fonctions C de `kyber-patches/0002`). Ce que l'ordinateur annonce est demandé tout de suite, pour que ⌘V marche aussitôt ; ce que le Mac copie est annoncé (vérifié deux fois par seconde) et envoyé à la demande. La console peut l'interdire dans un sens ou dans l'autre.
- **iCloud pour l'Apple TV** (`CloudPublisher`) : à chaque ajout, modification ou retrait d'un ordinateur, l'app publie la liste dans le magasin clé-valeur d'iCloud (clé `hosts` : nom, adresse, port de la console, compte, port du flux, empreinte, et `from` = nom de ce Mac) et copie le mot de passe dans le Trousseau iCloud. L'app Apple TV du même compte les ajoute sans rien taper. Les ordinateurs publiés par un autre Mac restent dans la liste. Réglage « Synchroniser avec iCloud » dans KyberNow › Réglages… (⌘,), actif par défaut ; le couper retire de la liste et du Trousseau iCloud les ordinateurs de ce Mac (l'Apple TV garde ceux qu'elle a déjà). Il faut les droits iCloud, donc un profil de provisionnement, en place en Debug comme en Developer ID (voir « Signature et iCloud »).
- **Mises à jour** : Sparkle, vérification quotidienne et menu KyberNow › Rechercher des mises à jour… ; flux `appcast.xml` signé EdDSA, publié avec chaque version sur GitHub (voir [docs/releases.md](../../docs/releases.md)).
- **Choix de l'écran** : avec l'écran virtuel, l'app attend au plus 6 s, une seule fois, que Kyber n'annonce plus que lui, puis prend l'écran à la taille demandée placé en (0, 0). Les sorties que Windows liste en 0 × 0 (éteintes) sont écartées ; s'il ne reste aucun écran utilisable, la session le dit au lieu de rester sur « Premières images ».
- **Manettes** : celles que gère SDL (Xbox, DualSense, DualShock…). Réglage par ordinateur « Manette vue par l'ordinateur » : **Xbox 360** (par défaut), toutes deviennent des manettes Xbox 360 sur l'ordinateur, celles que tous les jeux reconnaissent ; ou **Fidèle**, une manette Sony reste une DualShock 4 (pavé tactile, barre lumineuse pour les jeux qui s'en servent), les autres restent des Xbox 360. Le choix part avec la session (`gamepad_force_xbox360`) et prime sur l'option « Toutes les manettes en Xbox 360 » de la console, qui ne reste qu'un défaut pour les autres clients.
- **Combinaison de la manette** (L1 + R1 + Start + Select) : la barre s'ouvre et la manette la pilote (croix, A, B) ; même combinaison ou barre fermée, elle revient à l'ordinateur. C'est le contrôleur de session qui prend la manette, pas la couche qui lit les paquets (`GamepadGate`, qui ne fait que signaler la combinaison) : hors session (connexion, reconnexion, fin), la combinaison ne fait rien et la manette reste à l'ordinateur, au lieu de rester muette jusqu'à la combinaison suivante. À l'ouverture de la barre, tout est remis à zéro sur l'ordinateur : touches tenues relâchées (gâchette comprise) et les six axes au repos, pour chaque manette que la couche de paquets a vue passer ; ces relâchements sont les seuls paquets de manette qui partent tant que la barre a la manette. Quand l'image redémarre (changement de profil depuis la barre), la barre garde la manette.
- **Reconnexion** : pendant une reconnexion de Kyber, l'app attend : c'est elle qui relance l'image quand Kyber est revenu. Si Kyber a redémarré sur l'ordinateur, l'écran virtuel a pu partir avec lui (il garde toujours le même numéro, mais la console le retire et rallume les écrans physiques). L'app le redemande donc à la console (`/api/stream/prepare`, sans effet s'il est toujours là), attend au plus 3 s que Kyber n'annonce plus que lui, puis relance l'image dessus. Si l'ordinateur refuse quand même l'image dans les 15 s, elle refait cette demande une fois au lieu de couper la session. Le signal de vie dit aussi si l'écran virtuel est en place (`active`) : s'il a disparu en cours de session, l'app le redemande (au plus une fois toutes les 30 s). Quand l'app relance elle-même l'image (pas d'image clé en 1,5 s), Kyber ferme la connexion des données et se reconnecte sans relancer l'image : l'app la relance alors elle-même, au lieu de rester sur une image figée.
- **Panneaux sur l'image** : le panneau des mesures, qui reste ouvert, a un fond opaque. Un verre dépoli (`ultraThinMaterial`) serait recalculé par macOS à chaque image du flux (160 par seconde en 4K) et fait tomber la cadence. La barre, ouverte brièvement, garde son verre. Démarrer une image arrête toujours la précédente, chaîne d'entrées comprise : deux chaînes se disputaient la manette (SDL n'en sert qu'une), et l'ancienne, débranchée, rejetait tout.
- **Passage et session** : la session se joue dans la fenêtre de l'accueil. L'accueil est photographié et coupé en deux le long de la couture à 45° ; un tube néon s'y allume pendant la préparation (capsule : préparation, connexion, premières images, Annuler), puis les deux moitiés s'écartent sur l'image de l'ordinateur et la fenêtre passe en plein écran (`Passage.swift`, Core Animation). Le temps de la session, un délégué de fenêtre cache la barre des menus et le Dock en plein écran et transmet le reste à celui de SwiftUI. Barre au survol du haut de l'écran, en verre : symbole allumé, latence en direct et sa courbe, définition et format, profil, son, souris, presse-papiers, écran de l'ordinateur (sans écran virtuel), plein écran, Quitter. Fin propre avec vérification que l'ordinateur a retrouvé ses écrans et son son ; les moitiés se referment sur la dernière image, puis l'accueil revient.
- **Charte v2** (`Theme.swift`) : encre et givre, signal Spectre `#9A8CFF` qui ne s'allume en néon que dans les moments actifs (veille au survol, éclair au clic, allumé pendant une connexion ou une mesure en direct, allumage à l'apparition), coupé avec « Réduire la transparence » ; Archivo variable (largeur 125 pour les titres) et Martian Mono ; icônes Phosphor dans `Assets.xcassets/Icons` (`scripts/phosphor-icons.mjs`) ; icône de l'app au format Icon Composer (`KyberNow/AppIcon.icon` : les deux moitiés du symbole en verre « Liquid Glass », le néon de la couture par-dessus, fond d'encre), produite depuis la géométrie de `brand/v2` par `scripts/app-icon.py` et retouchable dans Icon Composer.

| Raccourci | Effet |
|---|---|
| ⌃⌥⌘D | Se déconnecter |
| ⌃⌥⌘F | Plein écran ou fenêtre |
| ⌃⌥⌘M | Souris en mode jeu (déplacements bruts) ou en mode bureau |
| ⌃⌥⌘S | Mesures détaillées (aussi : clic sur les mesures de la barre) |
| ⌘ + touche | Ctrl + touche sur l'ordinateur (réglable) |
| L1 + R1 + START + SELECT | La manette pilote la barre de l'app (croix, A, B) ; même combinaison ou barre fermée : elle revient à l'ordinateur |

⌃⌥⌘ + lettre reste au Mac (menus de l'app) et ne part jamais vers l'ordinateur.

## Image et son

Kyber dessine normalement l'image et joue le son avec VLC. KyberNow les décode lui-même (H.264, HEVC, AV1, Opus) pour la latence et la synchro : `kyber-patches/0001` ajoute à libkyclient une sortie des paquets vers l'app (voir [docs/kyber-changes.md](../../docs/kyber-changes.md)).

**Latence de bout en bout et trajet d'une image** (`LatencyTracker`)
- Mesures de Kyber actives : le serveur horodate chaque image à chaque étape (capture, début et fin d'encodage, envoi), et les pings de Kyber donnent le décalage d'horloge (médiane des 15 derniers).
- Le rendu signale l'instant où chaque image est reçue, décodée et montrée, ramené sur l'horloge `CLOCK_MONOTONIC` de Kyber.
- Latence = affichage − (capture − décalage), médiane de chaque seconde, dans la barre. La médiane de la session va sur l'accueil.
- Étapes, médianes de chaque seconde : capture → encodage, encodage, envoi, réseau (envoi → image complète reçue), décodage, affichage (décodée → à l'écran, synchro comprise).
- Pertes : une image envoyée par l'ordinateur qui n'est ni montrée ni remplacée par une plus récente dans la seconde est perdue (paquets manquants, ou écartée en attendant une image clé). Une image que l'écran écarte au profit d'une plus récente (flux à 160 i/s sur un écran à 120 Hz) compte comme remplacée. S'y ajoutent les trous du flux, les images clés redemandées, et les paquets perdus d'après Kyber (cumuls `NETWORK_LOCAL` / `NETWORK_REMOTE`, toutes les quelques secondes, avec l'aller-retour réseau).

**Image** (`VideoDecoder`, `VideoRenderer`, `VideoPipeline`)
- VideoToolbox, décodeur matériel exigé (logiciel en dernier recours), mode temps réel, une image par appel, sans file d'attente.
- Sortie dans les formats compressés sans perte des puces Apple (`&8v0`, `&8v4`), lus sans copie par Metal (`CVMetalTextureCache`). Conversion YCbCr → RGB dans le shader, selon la matrice annoncée par le flux (709, 601, 2020) et sa plage.
- Affichage : toujours l'image la plus récente. Chaque image part à l'écran dès qu'elle est décodée ; Core Animation montre la plus récente au balayage et écarte les autres (comptées comme remplacées). L'ancienne garde « une image en vol » (l'image N+1 attendait que l'écran ait montré N : un rafraîchissement complet de latence, 8 ms à 120 Hz) reste disponible avec `KN_INFLIGHT_GUARD=1`. Sur un écran à fréquence variable (ProMotion, Adaptive-Sync), l'écran se cale sur l'arrivée des images. Synchro verticale désactivable : latence la plus basse, déchirure possible.
- Reprise après une perte : quand des paquets se perdent au-delà de la correction d'erreurs, VideoToolbox refuse les images qui s'appuient sur la référence perdue. Avec le rafraîchissement progressif de l'encodeur, aucune image clé n'arrive d'elle-même. L'app garde alors la dernière image et cesse d'envoyer au décodeur des images inutilisables. Elle demande une image clé à l'ordinateur (`kyclient_client_request_keyframe`, serveur avec le correctif 0005), relance la demande toutes les 300 ms (minuterie en mode « common », qui tire aussi pendant un menu ouvert) et repart sur l'image clé. Sans réponse en 1,5 s (ordinateur pas à jour), elle relance le flux, qui commence toujours par une image clé. La demande part dans Kyber hors de sa boucle principale, une à la fois et abandonnée au bout d'une seconde : un ordinateur qui ne répond plus ne retarde ni l'arrêt ni la reconnexion.
- Durée de vie des décodeurs : le pipeline vidéo et le lecteur audio donnés à Kyber comme `userdata` sont retenus (`PacketSinkLease`) jusqu'au dernier appel de Kyber (`CLOSED`, correctif `kyber-patches/0001`), ou 30 s après l'arrêt si la bibliothèque ne l'envoie pas. Avant, ils étaient libérés 2 s après l'arrêt alors que Kyber pouvait encore livrer un paquet (plantage possible à la déconnexion).
- Image clé du début perdue : un flux lourd (4K à haute fréquence en 4:4:4) peut perdre sa première image clé entière, sans trou signalé par Kyber. Les images suivantes arrivent sans référence : le décodeur demande alors une image clé comme après une perte, au lieu d'attendre en silence.
- Repli : si aucune image n'est montrée 4 s après le démarrage du flux, ou si les demandes d'image clé restent sans réponse avant la première image, l'app allège le flux pour cette session seulement (débit ramené à 250 Mb/s, puis 4:2:0), le dit en haut de l'écran et le note dans le journal (`flux : …`, `repli …`). Changer de profil dans la barre repart du réglage choisi.
- Mesurer l'affichage : `KN_PRESENT_STATS=1 /Applications/KyberNow.app/Contents/MacOS/KyberNow` écrit chaque seconde dans le journal (catégorie `video`) une ligne `présentation : …` avec les images rendues, celles que l'écran confirme avoir montrées, celles que Core Animation a écartées au profit d'une plus récente (`presentedTime == 0`), et le délai commit → écran (médiane et 95e centile), plus l'état de la synchro verticale et de la garde. `KN_INFLIGHT_GUARD=1` remet la garde « une image en vol » (l'image N+1 attend que l'écran ait montré N) pour comparer. À lire : sans la garde, le délai commit → écran doit tomber vers un demi-intervalle de rafraîchissement (≈ 4 ms à 120 Hz) et des images écartées apparaissent dès que le flux dépasse la fréquence de l'écran, c'est attendu ; avec la garde, le délai vaut un intervalle entier. `MTL_HUD_ENABLED=1` (même commande) affiche le HUD Metal sur l'image : Metal ne dit pas publiquement si la présentation est directe (l'image va à l'écran sans passer par le compositeur) ou composée, seul le HUD l'indique.
- AV1 (puces M3 et suivantes) : l'en-tête de séquence est lu bit à bit pour construire la description `av1C` qu'attend VideoToolbox. Les échantillons sont débarrassés des délimiteurs temporels, et chaque OBU porte sa taille (`AV1.swift`). Mesuré sur M4 Max en 2560 × 1440 : 1 ms de décodage. VLC ne sert plus que de secours (AV1 sur un Mac sans décodeur AV1 matériel, que les réglages ne proposent pas).

**Son** (`AudioPlayer`)
- libopus (multistream : stéréo, 5.1, 7.1), masquage des pertes par Opus.
- Sortie Core Audio (`DefaultOutput` sur Mac, `RemoteIO` sur iOS/tvOS), demandes de 5 ms à la carte son.
- Tampon d'une vingtaine de millisecondes, contre 50 ms fixes chez VLC : sa marge grandit après un trou, redescend d'une milliseconde toutes les dix secondes sans trou, et l'excédent durable est jeté (dérive entre les horloges du PC et du Mac).
- Son et image partent chacun dès qu'ils sont prêts. La barre montre l'écart (son = tampon + latence de la sortie, image = réception → écran), imperceptible sous 45 ms. Un casque Bluetooth ajoute 150 à 250 ms côté son.

**Capacités de la machine** (`DecoderCapabilities`, `DisplayInfo`)
- Au lancement : décodeurs matériels H.264, HEVC et AV1, plus un vrai décodage d'une image HEVC 4:4:4 de 64 × 64 en exigeant le matériel.
- Les profils s'y ramènent : sans 4:4:4 matériel, 4:2:0 avec un tiers de débit en plus ; sans HEVC, H.264 avec moitié plus. Les réglages grisent ce que la machine ne décode pas.
- Écran : les pixels réels de la dalle, pris dans ses modes d'affichage (mode natif), jusqu'à 7680 × 4320, et 4096 de large au plus en H.264 (limite de NVENC). En mode « plus d'espace », macOS dessine plus grand que la dalle puis réduit : un 4K réglé en 3008 × 1692 points dessine 6016 × 3384, et ces pixels en trop ne sont pas demandés à l'ordinateur. Option « Au plus 4K », fréquence maximale, fréquence variable ou non.

Mesures en direct (barre de session) : latence de bout en bout et sa courbe, définition et format. Un clic dessus (ou ⌃⌥⌘S) ouvre les **mesures détaillées** en bas à droite (`SessionStats.swift`) : le trajet d'une image en cascade sur une échelle commune, avec la durée d'une image en pointillé ; la latence (pertes repérées sous la courbe) et le débit sur 30 s ; les pertes sur 30 s et depuis le début ; l'aller-retour réseau, l'image, le son et l'écart son-image. Le panneau reste ouvert et laisse passer clics et mouvements vers l'ordinateur, sauf son bouton de fermeture. En Debug, `KN_STATS_PREVIEW=1` l'affiche rempli de mesures plausibles, sans session.

## Organisation du code

```
apps/macos/
├── project.yml            # projet XcodeGen (cibles KyberKit et KyberNow)
├── KyberKit/              # framework : façade Swift du SDK Kyber
│   ├── KyberClient        # connexion, session, événements, options
│   ├── ConsoleClient      # API de la console (prepare, heartbeat, end, info), empreinte épinglée
│   ├── VideoDecoder / VideoRenderer / VideoPipeline / DecoderCapabilities
│   ├── AudioPlayer
│   ├── LatencyTracker     # latence de bout en bout (mesures de Kyber)
│   ├── StreamView         # vue de session : clavier, souris, curseur
│   ├── MetalSurface       # vue qui porte la couche Metal du rendu
│   ├── RelativeMouse      # souris des jeux (GCMouse)
│   ├── ClipboardBridge    # presse-papiers partagé
│   ├── InputPipeline, Keymap, GamepadGate   # entrées via libkynput
│   └── KyberLog           # journaux vers le journal unifié
├── KyberNow/              # app SwiftUI + AppKit
│   ├── KyberNowApp, Model (ordinateurs, réglages, Trousseau)
│   ├── CloudPublisher     # ordinateurs publiés dans iCloud pour l'Apple TV
│   ├── HomeView, HostImagery, AddHostFlow, SettingsView, AppSettingsView
│   ├── SessionController, SessionChrome, SessionStats, Passage
│   └── Theme, DisplayInfo, Fonts/ (Archivo, Martian Mono, OFL)
├── kyber-patches/         # correctifs KyberNow sur libkyclient et libkynput
├── scripts/               # vendor-kyber.sh, build-kyclient.sh, embed-kyber.sh
├── tests/                 # tests hors ligne (vidéo, son)
└── Vendor/                # SDK Kyber compilé (ignoré par git)
```

Déroulé d'une session : `ConsoleClient.prepare` (écran virtuel à la taille de ce Mac, son au bon nombre de voies) → `KyberClient.connect` → attente que la liste d'écrans se stabilise → `startStreaming` → signal de vie toutes les quelques secondes → `ConsoleClient.end` (l'ordinateur retrouve ses écrans et sa sortie audio).

## Construire

Prérequis : Xcode 27, XcodeGen, Rust 1.89 + `cargo-c`, le client Kyber 0.29 compilé dans `third_party/kyber-desktop` (voir [docs/build-notes.md](../../docs/build-notes.md)).

```bash
apps/macos/scripts/vendor-kyber.sh      # SDK Kyber (en-têtes, bibliothèques, plugins VLC) → Vendor/
apps/macos/scripts/build-kyclient.sh    # libkyclient et libkynput avec les correctifs KyberNow → Vendor/
cd apps/macos && xcodegen generate
xcodebuild -project KyberNow.xcodeproj -scheme KyberNow -configuration Debug -derivedDataPath build build
open build/Build/Products/Debug/KyberNow.app
```

`build-kyclient.sh` est à relancer après chaque `vendor-kyber.sh` : sans lui, l'app reprend des bibliothèques sans les correctifs. Quand `Vendor/` change, il faut quitter l'app avant de recompiler : VLC planterait en chargeant des modules réécrits sous ses pieds (voir [build-notes](../../docs/build-notes.md)). Une recompilation sans changement de `Vendor/` ne touche plus aux bibliothèques de l'app. Signature : certificat « Apple Development: info@leslignesclaires.fr », équipe P6DB682S4K (`project.yml`). Elle reste la même d'une compilation à l'autre, ce qui évite que le Trousseau redemande l'accès au mot de passe ; prévoir un Developer ID pour distribuer.

### Signature et iCloud

La publication pour l'Apple TV exige deux droits, `com.apple.developer.ubiquity-kvstore-identifier` (`$(TeamIdentifierPrefix)fr.leslignesclaires.kybernow`, le magasin clé-valeur de l'app Apple TV) et `keychain-access-groups` (`$(AppIdentifierPrefix)fr.leslignesclaires.kybernow`, Trousseau à protection des données). Ils sont dans `KyberNow/KyberNow.entitlements`, avec la caméra. Sur macOS, ces droits demandent un profil de provisionnement embarqué, sans quoi l'app est tuée au lancement : en Debug, le profil « macOS App Development » `fr.leslignesclaires.kybernow` (ce Mac, certificat info@leslignesclaires.fr) ; en Release, le profil « Developer ID » `P6DB682S4K.fr.leslignesclaires.kybernow` (tous les Mac, valable jusqu'en 2044). Les deux doivent être installés sur le Mac qui compile, sinon Xcode refuse de signer. L'app vérifie aussi les droits à l'exécution (`CloudPublisher.entitled`) : signée sans eux, elle ne publie rien et le dit.

Mise en place (faite le 2026-10-20), à refaire pour un nouveau Mac de compilation ou un profil expiré, sur [developer.apple.com](https://developer.apple.com/account/resources) :

1. Identifiant d'app `fr.leslignesclaires.kybernow` : la capacité iCloud y est déjà (créée pour l'Apple TV, magasin clé-valeur compris) ; rien à cocher de plus, le groupe du Trousseau `P6DB682S4K.fr.leslignesclaires.kybernow` vient de l'identifiant lui-même.
2. Appareils : enregistrer ce Mac (Provisioning UDID dans Informations système › Matériel ; ou laisser Xcode le faire depuis Réglages › Comptes).
3. Profils : « macOS App Development » pour `fr.leslignesclaires.kybernow`, certificat « Apple Development: info@leslignesclaires.fr », ce Mac ; « Developer ID » (Mac) pour le même identifiant, certificat Developer ID Application. Pour qu'Xcode les trouve, copier chaque fichier téléchargé dans `~/Library/Developer/Xcode/UserData/Provisioning Profiles/<UUID>.provisionprofile` (le double-clic ne suffit pas sur macOS).
4. Leurs noms vont dans `project.yml` (`PROVISIONING_PROFILE_SPECIFIER`, par configuration), puis `xcodegen generate`.

Xcode met le profil dans l'app (`Contents/embedded.provisionprofile`) et y ajoute l'identifiant d'app et l'équipe. `scripts/sign-release.sh` re-signe l'app avec les droits que Xcode lui a donnés (et non plus avec le fichier `.entitlements`) : la version Developer ID garde le profil et ses droits jusqu'à la notarisation. L'élément local du Trousseau (service `fr.leslignesclaires.kybernow`) ne change pas : la signature reste la même, l'accès au mot de passe aussi.

## Tester sans ordinateur distant

```bash
apps/macos/tests/video/run.sh   # flux HEVC, H.264 et AV1 générés par ffmpeg : décodage, temps, couleurs rendues, reprise après une perte
apps/macos/tests/audio/run.sh   # Opus stéréo et 5.1 au rythme réel, à-coups et pertes : tampon, trous
apps/macos/tests/clipboard/run.sh   # presse-papiers privé : annonces, demandes, réponses, 60 Ko
apps/macos/tests/screen/run.sh  # fenêtre de 2 × 2,5 s : la surface de session montre vraiment les images (presentedTime), délai commit → écran avec et sans la garde « une image en vol »
```

Mesuré sur M4 Max : 1 à 2 ms de décodage en 2560 × 1440 (4:2:0), 1,6 à 2,7 ms en 4:4:4, couleurs à ±4 niveaux près ; son : 15 à 23 ms de tampon en paquets de 20 ms, 14 à 22 ms en paquets de 10 ms (serveur avec 0004), avec ± 8 ms d'à-coups simulés.

## Journaux

Console.app, sous-système `fr.leslignesclaires.kybernow`, catégories `app`, `kyber` (SDK), `video`, `audio`. En ligne de commande :

```bash
log stream --predicate 'subsystem == "fr.leslignesclaires.kybernow"' --level info
log stream --predicate 'subsystem == "fr.leslignesclaires.kybernow" AND category == "video" AND eventMessage BEGINSWITH "présentation"'   # avec KN_PRESENT_STATS=1
```

## Données

- Ordinateurs et réglages : `UserDefaults` (clé `hosts`), relus avec tolérance (un réglage ajouté garde sa valeur par défaut).
- Images des ordinateurs : `~/Library/Application Support/KyberNow/Images/<id>-last.jpg` (dernière session) et `<id>-preview.jpg` (aperçu de la console), effacées avec l'ordinateur.
- Mots de passe : Trousseau, service `fr.leslignesclaires.kybernow`, un élément par ordinateur.
- iCloud (synchro active, app signée avec les droits) : magasin clé-valeur, clé `hosts` (tableau JSON de `CloudHost`, format partagé avec `apps/tvos/KyberNowTV/CloudSync.swift`) ; mots de passe dans le Trousseau iCloud, service `fr.leslignesclaires.kybernow.cloud`, compte = identifiant de l'ordinateur. Réglages de l'app : `UserDefaults` `icloudSync` (synchro) et `icloudPublished` (ordinateurs publiés par ce Mac).

## Vers iOS et Apple TV

Réutilisables tels quels : `VideoDecoder`, `VideoRenderer`, `VideoPipeline`, `DecoderCapabilities`, `AudioPlayer`, `LatencyTracker`, `ConsoleClient`, `GamepadGate`. À adapter : `ClipboardBridge` (UIPasteboard au lieu de NSPasteboard). À écrire : une vue UIKit qui porte la couche Metal (équivalent de `MetalSurface`), les entrées tactiles et télécommande, et la compilation de libkyclient, libkynput et libopus pour iOS et tvOS (`kysdk/kyctl/build-ios.sh` sert de point de départ). Il n'y a ni VRR ni synchro désactivable sur Apple TV.
