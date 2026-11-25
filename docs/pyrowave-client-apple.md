# PyroWave : l'Apple TV, l'iPhone et l'iPad

[English](pyrowave-client-apple.en.md) · Français

Section client de [pyrowave.md](pyrowave.md) (le contrat et la Station), à y fusionner. Concerne l'app Apple TV (`apps/tvos`) et l'app iPhone et iPad (`apps/ios`). Le décodeur est celui du Mac, dans KyberKit : [pyrowave-client-mac.md](pyrowave-client-mac.md). Règles et textes : [link-ui.md](link-ui.md), « PyroWave ».

## Où PyroWave sera indisponible

Le port Metal demande un GPU de la famille Apple7 ou suivante (A14, M1). Ailleurs, la sonde du lancement répond « indisponible » et « Format vidéo » montre PyroWave inactif, avec « Cette Apple TV ne sait pas décoder PyroWave. », « Cet iPhone ne sait pas décoder PyroWave. » ou « Cet iPad ne sait pas décoder PyroWave. ».

| Appareil | PyroWave |
|---|---|
| Apple TV 4K de 2022 (A15, AppleTV14,1) | disponible |
| Apple TV 4K de 2021 (A12, AppleTV11,1), Apple TV 4K de 2017 (A10X), Apple TV HD (A8) | **indisponible** |
| iPhone 12 et suivants (A14 et après), iPhone SE de 3ᵉ génération (A15) | disponible |
| iPhone XS, XR, 11, SE de 2ᵉ génération (A12, A13) | **indisponible** |
| iPad Air 4 et suivants, iPad de 10ᵉ génération et après, iPad mini 6 et après, iPad Pro M1 et après | disponible |
| iPad de 7ᵉ, 8ᵉ et 9ᵉ génération, iPad mini 5, iPad Air 3, iPad Pro de 2018 et 2020 (A10 à A13, A12X/Z) | **indisponible** |
| Simulateurs (iOS, tvOS) | **indisponible** : le Metal du simulateur n'a pas la famille Apple7 (vu le 25/11/2026 : « GPU non pris en charge ») |

Déduit de la famille du GPU ; seule la réponse du simulateur est vue.

## Construction

- **Décodeur** : les projets Apple TV et iPhone/iPad compilent le port Metal dans KyberKit (`pyrowave_common.mm`, `pyrowave_decoder.mm`, `pyrowave_bitstream.cpp`, ARC, C++17, module `CPyroWave`) et `KyberKit/PyroWaveDecoder.swift`, comme le Mac. Aucun réglage propre à tvOS ou iOS n'a été nécessaire : vérifié le 25/11/2026 pour l'appareil et le simulateur (`appletvos`, `appletvsimulator`, `iphoneos`, `iphonesimulator`, Xcode 26). Le port n'emploie que du stockage partagé et des textures `r8Unorm`/`r16Unorm`, que les GPU Apple des Apple TV et iPhone prennent.
- **Kyber** : le correctif kyclient 0025 (`VideoCodec::PyroWave`, `fallback_codec`) et la fonctionnalité `pyrowave` du cœur ne sont posés qu'avec un drapeau explicite, éteint par défaut, comme `build-native.sh --kyber-pyrowave` d'Android : `apps/tvos/scripts/build-kyber-tvos.sh --kyber-pyrowave`, `apps/apple/scripts/build-apple-mobile.sh tvos|ios --kyber-pyrowave` (fonctionnalité `kyber-pyrowave` de `kybernow-apple`, qui allume `kybernow-core/pyrowave`), `apps/core/scripts/build-apple.sh --kyber-pyrowave`. Sans eux, l'app décode PyroWave et l'annonce au cœur, mais le cœur ne le demande jamais à Kyber. `third_party/` étant partagé par tous les worktrees, le correctif n'y est posé qu'à la demande.

## La sonde et le câble

- **Sonde** : `DecoderCapabilities.prepare()` au lancement des deux apps, hors du fil principal (`KyberNowTVApp`, `KyberNowApp`) : device PyroWave, décodeur 128 × 128, image de test décodée et relue (`PyroWaveDecoder.probe()`, voir le client Mac). Le résultat part au cœur dans les capacités du décodeur (`pyrowave`, `SessionEngine`), et dans le résumé des décodeurs des Réglages de l'app (« …, PyroWave »).
- **Câble** : `NetworkLink` (KyberKit, `PyroWaveSupport.swift`), un `NWPathMonitor` démarré au lancement : relié par câble quand le chemin par défaut passe par une interface `.wiredEthernet` (Ethernet de l'Apple TV ; adaptateur USB-C ou Lightning de l'iPhone et de l'iPad). Envoyé au cœur dans les réglages de la session (`SessionEngine.Settings.wired`, `StreamSettings.wired`) : « Auto » prend PyroWave seulement par câble, hors relais, si la Station le propose et que l'appareil le décode (`profile::pyrowave_format`). Les réglages lisent le lien de leur côté (Apple TV : `watchLink` ; iPhone, iPad : `SettingsLink`), pour leurs aides.
- **Annonce de la Station** : `HostInfo.pyrowave` (`PyroWaveOffer`, `pyrowave` de `GET /api/stream/info`, objet `{available, reason}` ou booléen ; absent : Station d'avant PyroWave, rien ne change dans l'app).

## Interface

Textes de link-ui, dans les catalogues des apps (`Localizable.xcstrings` de l'Apple TV, de l'iPhone et du Mac pour les textes communs de `PyroWaveText`, `apps/macos/KyberNow/LinkUI.swift`).

- **« Auto » sur les apps Apple** : un profil (Jeu, Bureau, Max), c'est le cœur qui choisit le format (règle du cœur). « Format vidéo » ne montre « Auto » et « PyroWave » que si la Station connaît PyroWave (ou s'il est déjà choisi) ; sans cela, la liste reste celle d'avant. « Auto » est coché quand un profil est actif ; le choisir depuis « Personnalisé » reprend le profil Jeu. Choisir un format rend le profil « Personnalisé ». PyroWave choisi : le débit qu'« Auto » prendrait par câble pour la définition de la session (`PyroWaveRate`, comme `PYROWAVE_MBPS` du cœur : 800 Mbit/s en 4K60, ≈ 280 en 1080p60 ; 1 200 en 4:4:4) ; le quitter ramène le débit à 80 Mbit/s au plus.
- **Apple TV, Réglages › Image › Format vidéo** (maquette Apple TV) : quand PyroWave est montré, la rangée devient une liste au focus à droite (Auto, HEVC, H.264, AV1, PyroWave ; coche sur le choix), à lire de loin ; à gauche, l'aide (raison d'un PyroWave inactif, PyroWave choisi, Auto par câble ou sans câble) et, PyroWave choisi en Wi-Fi ou hors de chez vous, l'avertissement ambre sous l'aide. « Texte net (4:4:4) » : « Avec PyroWave, presque sans coût : seul le débit monte. ». Le débit n'avertit plus au-delà de 150 Mbit/s en PyroWave (jauge jusqu'à 1 500).
- **Apple TV, session** : bandeau du début (PyroWave choisi, image reçue en PyroWave, sans câble ou par le relais, une fois par session, 8 s) : « PyroWave en Wi-Fi » ou « PyroWave hors de chez vous », « L'image peut saccader. », et « Passer en HEVC » derrière le glyphe Retour : la télécommande n'a pas de focus pendant le jeu, le bouton est dans le menu. Menu de session : la tuile ambre « PyroWave en Wi-Fi » / « PyroWave hors de chez vous », détail « Passer en HEVC », qui relance l'image en HEVC pour cette session seulement (le choix de l'ordinateur reste ; un profil choisi ensuite reprend la main). Statistiques en rangée : le format dit « PyroWave », et « Images perdues 0,8 % » (part des images des 30 dernières secondes) remplace le compte des pertes quand l'image est en PyroWave.
- **iPhone, iPad, Réglages › Image › Format vidéo** : Auto, HEVC, H.264, AV1, PyroWave (inactif, barré, avec sa raison dans l'aide) ; l'avertissement ambre sous le choix (`SettingsWarning`, `choice(…, warning:)`) ; aide de « Texte net (4:4:4) » ; en PyroWave, le débit n'est plus en ambre au-delà du confort du Wi-Fi (l'avertissement du format suffit).
- **iPhone, iPad, session** : bandeau du début, comme une capsule de la session : titre, « L'image peut saccader. », « Passer en HEVC » (pas pour un invité ni un ami) et la croix ; 8 s. Barre : la puce ambre « PyroWave en Wi-Fi » / « PyroWave hors de chez vous » à côté des statistiques, qui les ouvre. Statistiques (feuille de l'iPhone, panneau de l'iPad) : le format « PyroWave » et la tuile « Images perdues » (part des images des 30 dernières secondes), en plus des pertes.
- **Format reçu** : `SessionEngine.receivedCodec` (dernier paquet de codec), lu chaque seconde : la puce, la tuile, le bandeau et le format des statistiques suivent ce que la Station envoie vraiment (PyroWave pris par « Auto », ou son repli `fallback_codec`).
- **Définition ramenée** : le cœur le dit (`KN_NOTICE_RESOLUTION_LOWERED`, `SessionEngine.Notice.resolutionLowered` dans KyberKit) ; l'Apple TV et l'iPhone/iPad montrent son texte en avis quelques secondes (« Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. »). Pas encore dans leurs Statistiques (`SessionEngine.lowered` le donne, comme sur le Mac), ni le débit du lien donné au cœur (`LinkRate`, `SessionEngine.setLinkMbps`, branchés sur le Mac seulement). Écrit sans être compilé pour l'iPhone et l'Apple TV (pas de bibliothèques de l'appareil dans le worktree du Mac).

Démonstration (Debug, simulateur) : `-demo YES` avec `-demoPyroWave wifi|relay` (session en PyroWave : bandeau, puce ou tuile, « Images perdues » ; Apple TV `-screen passage|menu`, iPhone `-screen bar`), `-demoPyroWaveOffer on|off` (la Tour annonce PyroWave), `-demoCodec pyrowave` (PyroWave choisi pour la Tour), `-demoPyroWaveDecodes YES` (le simulateur compté comme décodant PyroWave), `-demoLink Wi-Fi|Ethernet` (Apple TV) ou `wifi|ethernet` (iPhone, iPad) pour les aides.

## Code commun avec le Mac

Les changements de KyberKit et de `apps/macos/KyberNow` servent aux trois apps : `VideoCodec.pyrowave`, `HostInfo.pyrowave`, `SessionEngine.Settings.wired` et `receivedCodec`, `PyroWaveSupport.swift` (annonce, lien, choix, débit, durées), `PyroWaveText` (textes), `ConnectionSettings.decodes444(.pyrowave)`. L'app Mac compile avec (vérifié le 25/11/2026) mais n'a pas encore l'interface de PyroWave ni `wired` (elle envoie `false`) : c'est le prochain pas du client Mac.

## Essai hors ligne (`apps/apple/tests/pyrowave`)

L'essai du Mac (`apps/macos/tests/pyrowave`) adapté aux appareils : une petite app à part, **PyroWaveBench** (`fr.leslignesclaires.kybernow.pyrowavebench`, Apple TV et iPhone/iPad), qui ne remplace pas KyberNow Link installée. Pas de ffmpeg sur l'appareil : la mire est dessinée sur place (rampe de Y qui défile, damier d'un pixel, barres de couleur, BT.709 plage réduite), 60 images encodées par l'encodeur Metal du port à 1,2 bit par pixel à 60 i/s (une image = ses paquets bout à bout, comme un paquet média de la Station), puis décodées par `VideoDecoder` (« pwv1 ») → `PyroWaveDecoder`. Pour 1080p et 4K (et 1440p sur iPhone, iPad), en 4:2:0 et 4:4:4, SDR puis HDR10 (sortie 10 bits) : la sonde, le temps de décodage cadencé à 60 i/s (moyenne, médiane, 95ᵉ centile, maximum), les images par seconde à la chaîne, le PSNR de la première image, une image tronquée sautée sans casser la suivante ; « OK » (code 0) ou « ÉCHEC » (code 1). Le simulateur n'a pas la famille Apple7 : l'essai n'y fait que la sonde (« Sonde : PyroWave indisponible », vu le 25/11/2026).

`run.sh tv|ios <appareil> [définitions]` construit l'app, la signe, l'installe (`devicectl`) et la lance en lisant sa sortie. Signature : Apple TV avec le profil d'équipe générique de tvOS (« tvOS Team Provisioning Profile: * ») quand il couvre l'appareil, sans rien enregistrer ; sinon, et pour l'iPhone et l'iPad, la signature automatique d'Xcode (`-allowProvisioningUpdates`), qui enregistre l'identifiant de l'app d'essai sur le compte développeur la première fois.

**Pas encore mesuré** : le 25/11/2026, PyroWaveBench a été construite, signée et installée sur l'« Apple TV du salon » (A15), mais tvOS a refusé de la lancer, l'Apple TV étant en veille (« System is asleep - foreground app launch forbidden ») ; elle n'a pas été réveillée (le téléviseur se serait allumé). L'iPhone n'a pas de profil générique : l'essai demande l'enregistrement de l'identifiant (ci-dessous).

### Marche à suivre pour l'auteur

Sur le Mac, dans le worktree de la branche (ou après fusion), Xcode 26, `xcodegen` :

1. **Apple TV du salon** (A15) éveillée, à l'accueil : `apps/apple/tests/pyrowave/run.sh tv "Apple TV du salon"`. Attendu : « GPU : Apple A15 GPU · Apple7 : oui », « Sonde : PyroWave disponible (… ms) », puis pour 1920x1080 et 3840x2160, en 4:2:0 et 4:4:4 : « Encodé : 60 images … », deux blocs (SDR, HDR10) avec « cadencé 60 i/s : 60/60 images, moyenne … ms », « à la chaîne : … i/s », « PSNR Y … dB » (40 dB au moins en Y), « image tronquée puis entière : 1 sortie(s) (tronquée sautée, suivante décodée) », puis « OK ». À noter : toute la sortie (le temps de décodage en 4K est le chiffre à retenir). Plus court : `run.sh tv "Apple TV du salon" 3840x2160`. L'app reste sur l'Apple TV (icône « PyroWaveBench ») ; on peut aussi la lancer à la télécommande : les lignes s'affichent à l'écran.
2. **Apple TV de l'atelier** (A12, 2021) : `run.sh tv "Apple TV de l'atelier"` (signature automatique : le profil générique ne la couvre pas). Attendu : « Apple7 : non », « Sonde : PyroWave indisponible », « ÉCHEC » (code 1), rien de plus.
3. **iPhone 18 Pro Max** déverrouillé : `run.sh ios "iPhone 18 Pro Max de Matthieu"` (la première fois, Xcode enregistre `fr.leslignesclaires.kybernow.pyrowavebench` sur l'équipe ; l'iPhone peut demander de faire confiance au développeur dans Réglages › Général › VPN et gestion de l'appareil). Attendu comme l'Apple TV, avec 2560x1440 en plus.
4. **Les apps** (installation : `apps/tvos/SIDELOAD.md`, `apps/ios/SIDELOAD.md`, ou Xcode) : ouvrir l'app, puis Réglages de l'app : la ligne des décodeurs dit « …, PyroWave » sur l'Apple TV du salon et l'iPhone, pas sur l'Apple TV de l'atelier. Journal : `log stream --device … --predicate 'subsystem == "fr.leslignesclaires.kybernow"'` (ou Console.app) : « PyroWave prêt en … ms (…) » ou « PyroWave : GPU non pris en charge », et « lien réseau : Ethernet » ou « sans câble ».
5. **Une session réelle**, une fois la Station en PyroWave et le SDK construit avec `--kyber-pyrowave` (`build-kyber-tvos.sh --kyber-pyrowave`, `build-apple-mobile.sh tvos|ios --kyber-pyrowave`) : Apple TV en Ethernet sur « Auto » (profil Jeu) : l'image arrive en PyroWave (Statistiques du menu : « PyroWave 4:2:0 », « Images perdues »). Puis en Wi-Fi, PyroWave choisi à la main : l'avertissement des réglages, le bandeau du début, la tuile du menu ; « Passer en HEVC » : l'image repart en HEVC. Même chose sur l'iPhone (adaptateur Ethernet USB-C pour « Auto »), avec la puce de la barre.

## État des essais

| Quoi | État |
|---|---|
| Compilation (appareil et simulateur, tvOS et iOS) | faite le 25/11/2026 ; l'app Mac compile aussi |
| Sonde sur le simulateur | « GPU non pris en charge », PyroWave indisponible, l'app démarre (25/11/2026) |
| Interface : réglages, bandeau, tuile ou puce, Statistiques | vue sur les simulateurs Apple TV 4K et iPhone 16 Pro (tvOS 27.0, iOS 27.0), avec les démonstrations (25/11/2026) ; pas vue sur iPad, ni sur un appareil |
| Décodage, temps de décodage, HDR10 sur un vrai GPU | **à mesurer** (marche à suivre ci-dessus) |
| Apple TV de 2021 : sonde indisponible | déduit, à confirmer |
| `wired` par un vrai câble (Apple TV en Ethernet, adaptateur USB-C) | à essayer (le simulateur prend le lien du Mac) |
| Session complète vers la Station | à essayer (correctif 0025 et `--kyber-pyrowave`) |

## Ce qui attend

- Le correctif kyclient de la Station et la fonctionnalité `pyrowave` du cœur allumés pour toutes les apps : alors, retirer le drapeau `--kyber-pyrowave`.
- « Définition ramenée » : avis branché ; ligne des Statistiques et débit du lien à faire (comme sur le Mac).
- L'Apple TV 4K de 2022 n'a qu'un Ethernet Gigabit (et seulement le modèle de 128 Go) : PyroWave en 4K60 (800 Mbit/s par défaut) y est près de la limite ; à surveiller dans les « Images perdues ».
