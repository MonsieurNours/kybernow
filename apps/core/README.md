# Cœur de KyberNow Link (`kybernow-core`)

[English](README.en.md) · Français

Ce que les apps KyberNow Link ont en commun, écrit une fois en Rust : le dialogue avec KyberNow Admin, la session Kyber, les profils et le débit, la latence, les mises à jour, le module d'entrées et le moteur de son. Toutes les apps y reposent : Windows, Linux et webOS ([apps/windows](../windows/README.md), [apps/linux](../linux/README.md), [apps/webos](../webos/README.md)) directement, Android ([apps/android](../android/README.md)) par JNI (`apps/android/jni`), Mac, Apple TV, iPhone et iPad ([apps/macos](../macos/README.md), [apps/tvos](../tvos/README.md), [apps/ios](../ios/README.md)) par le pont C `apps/apple` ([docs/coeur-apple.md](../../docs/coeur-apple.md)).

Le cœur ne dépend d'aucun système : rien n'y appelle AppKit, UIKit, Win32 ni Core Audio. Ce que la plateforme fournit passe par des traits (la fenêtre, les décodeurs, la sortie du son et les entrées par `session::Platform` ; l'horloge, le presse-papiers et les rappels vers l'interface par les traits du module d'entrées ; la sortie du son tire elle-même le son du moteur). Les briques de Kyber 0.29 viennent de `third_party/kyber-desktop/kysdk` avec les correctifs KyberNow (`apps/macos/kyber-patches`), voir [apps/windows/README.md](../windows/README.md).

## Organisation du code

| Fichier | Contenu |
|---|---|
| `src/console.rs` | Client d'Admin : session (`prepare`, `heartbeat`, `foreground`, `end`), accueil et ajout (`info`, `probe`, `preview`, `pair_request`, `pair_status`), la Station (`station_restart`, `station_update`, `admin_url`, ci-dessous), certificat épinglé, `Accept-Language` |
| `src/i18n.rs` | Langue (détection, choix), catalogues de textes, notes de version par langue ([docs/i18n.md](../../docs/i18n.md)) |
| `src/session.rs` | Déroulé d'une session sur un fil dédié, dernier recours de la connexion (ci-dessous) ; ce que la plateforme fournit : `Platform` |
| `src/media.rs` | Paquets d'image et de son livrés par Kyber (correctif 0001), en-tête Opus |
| `src/profile.rs` | Profils, format, débit selon les pixels par seconde |
| `src/latency.rs` | Latence de bout en bout, horloge de Kyber |
| `src/update.rs` | Mises à jour (Windows, Android, Linux, webOS) : manifeste signé (minisign), canal, téléchargement vérifié (taille, SHA-256) ; le fichier de l'app est cherché sous son nom actuel puis sous ses anciens noms (Windows : `KyberNow-Link-Setup.exe`, puis `KyberNow-Client-Setup.exe`, seul nom des versions 0.3.1 et antérieures) |
| `src/wake.rs` | Réveil par le réseau (Wake-on-LAN, ci-dessous) : `WakeTarget`, paquet magique, envoi en diffusion et en direct |
| `src/input/` | Module d'entrées partagé (ci-dessous) |
| `src/audio/` | Moteur de son (ci-dessous) |
| `build.rs` | libopus par pkg-config quand il la connaît ; sinon l'exécutable la lie lui-même |

## La Station dans les réglages d'un ordinateur

La rubrique « Station » des réglages d'un ordinateur (Ouvrir Admin, Redémarrer la Station, Mettre à jour la Station) passe par `console.rs` :

- `Info::station` (`GET /api/stream/info`) : `StationInfo` avec la version de la Station, sa mise à jour (`StationUpdate` : `available`, `version`, `installable`, `requires_setup`, `installing`) et `restarting`. `None` avec une Station 0.4.9 ou plus ancienne : l'app n'y propose ni redémarrage ni mise à jour.
- `Console::admin_url()` : `https://adresse:port/` (IPv6 entre crochets), à ouvrir dans le navigateur ; le certificat d'Admin est auto-signé, le navigateur demande de l'accepter la première fois.
- `Console::station_restart(device_id, force)` : redémarre la Station entière (le serveur Kyber et Admin, environ une minute, Admin ne répond plus pendant ce temps). `Console::station_update(device_id, force)` : la même installation qu'« Installer maintenant » dans Admin, rend la version installée ; Admin télécharge et vérifie le paquet avant de répondre (300 s de délai). `device_id` : l'identifiant de l'appareil, qui le nomme dans le fil d'Admin (il ne donne aucun droit : tout appareil appairé peut redémarrer et mettre à jour) ; `force` : même si un appareil est en session, qui est coupée.
- Refus (`StationError`) : `InSession` (un appareil est en session, `device` : son nom ; refaire l'appel avec `force`), `TooSoon` (un redémarrage par minute, ou trop d'essais ratés depuis cette adresse ; `seconds`), `NothingToInstall` (Station à jour, ou version refusée), `RequiresSetup` (la version s'installe avec l'installateur, sur l'ordinateur), `Busy` (installation ou redémarrage déjà en cours), `Unsupported` (Station 0.4.9 ou plus ancienne : elle répond la page d'Admin, ou 404), `Other` (`ConsoleError`). Le message vient d'Admin, dans la langue de l'app. `kind()` et `to_json()` donnent le refus aux ponts (`kind`, `message`, `device`, `seconds`).

## Dernier recours de la connexion : la Station redémarrée

Quand une connexion n'aboutit vraiment pas, la session redémarre elle-même la Station, comme « Redémarrer la Station » des réglages, puis réessaie (`src/session.rs`, `Recovery::restart_station`, vrai par défaut, sans réglage dans les apps).

- **Quand.** Admin a répondu (`prepare` a réussi) et aucune image n'a encore été montrée (`LatencyTracker::any_displayed`), mais l'image n'arrive pas malgré les réparations de la session (image clé redemandée, flux relancé trois fois, repli : débit réduit puis 4:2:0), Kyber refuse l'image (`StreamingStartFailed`), la chaîne d'image échoue trois fois, ou aucun écran utilisable n'apparaît ; ou bien la connexion à Kyber échoue après ses propres essais : réseau, délai, erreur du serveur ou du protocole (`ControlPlaneConnectFailed`), connexion au flux impossible, reconnexion épuisée, connexion fermée par Kyber.
- **Jamais** : Admin injoignable ou qui refuse le compte (`prepare` ou `end` en échec), compte que Kyber ne connaît pas encore (`Unauthorized` : Admin redémarre déjà Kyber pour les comptes en attente quand personne d'autre ne s'en sert), certificat de Kyber différent, erreur de l'appareil, annulation par l'utilisateur, fin demandée par Admin ou par un autre appareil qui prend la main, session établie (une image a été montrée : elle garde ses reprises, reconnexion comprise), `restart_station` faux.
- **Déroulé.** La tentative est close comme une fin de session (`POST /api/stream/end` : écrans et son rétablis ; sans cela, Admin compterait cet appareil comme en session et refuserait). Puis `station_restart(device_id, force = false)` : Admin note dans son fil quel appareil a redémarré la Station, et refuse si un autre appareil est en session (`InSession`), si c'est trop tôt (`TooSoon`), avec une Station trop ancienne (`Unsupported`) ou sur toute autre erreur ; la session échoue alors avec sa raison, comme sans ce recours. Accepté, la plateforme reçoit `Status::StationRestarting` ; le cœur interroge `info` toutes les 2 s jusqu'à ce qu'Admin réponde avec `station.restarting` faux, 90 s au plus. Revenue, la session repart du début : `Preparing`, `prepare` (l'écran virtuel est recréé), Kyber, image, avec les réglages de départ (le repli repart de zéro).
- **Une fois par connexion.** Si l'image ne vient toujours pas : « {raison}, même après un redémarrage de la Station » (`session.after_station_restart`) ; si la Station ne revient pas dans les 90 s : « la Station tarde à revenir après son redémarrage : ouvrez Admin pour en savoir plus » (`session.station_not_back`). L'utilisateur peut annuler pendant l'attente : la session s'arrête sans erreur.
- **Pour les apps.** `Status::StationRestarting` arrive entre l'étape en cours et `Preparing` (ou `Ending`) ; l'écran de connexion affiche « La Station redémarre… » ([docs/link-ui.md](../../docs/link-ui.md), « Connexion »). Une app qui ne le montre pas encore reste sur l'étape « Connexion » : Apple reçoit `KN_STATUS_CONNECTING` puis l'avis `KN_NOTICE_STATION_RESTARTING` ([docs/coeur-apple.md](../../docs/coeur-apple.md)), Android l'état 1 puis l'avis 7 (`SessionListener.NOTICE_STATION_RESTARTING`), l'interface Windows `session-status` à `connecting` puis l'événement `session-station-restarting` (`ui/src/lib/api.ts`), Linux et webOS leur étape « Connexion ». Les ponts lisent `recovery.restart_station` dans le JSON de la session (vrai s'il manque) ; la ligne de commande Windows et les bancs d'essai peuvent le couper.

## Réveil par le réseau (`src/wake.rs`)

Les apps proposent « Réveiller l'ordinateur » quand il est hors ligne, sans rien à saisir : Admin (Station 0.5.0 et suivantes) donne ses cartes réseau dans `Info::wake`, l'app les garde pour chaque ordinateur et le cœur envoie le paquet magique.

- `WakeTarget` : une carte, `mac` (`AA:BB:CC:DD:EE:FF`), `ipv4` (sa dernière adresse), `broadcast` (la diffusion de son sous-réseau), `kind` (`ethernet`, `wifi`), `armed` (Windows la laisse réveiller l'ordinateur ; `None` : inconnu) et `name`. Sérialisé tel quel (serde, champs facultatifs sauf `mac`, champs inconnus ignorés) : les apps Rust le stockent dans leur fichier, les autres gardent le JSON de `wake`. `Info::wake` est vide avec une Station plus ancienne, et la lecture est tolérante (`wake` illisible : liste vide, le reste d'`info` est lu). L'app remplace les cibles gardées par celles d'un `info` réussi quand la liste n'est pas vide. `WakeTarget::from_mac(mac, ipv4)` : une cible d'après une adresse saisie (`3C:7C:…`, `3c-7c-…`, `3c7c.3f12.abcd`).
- `send(&targets)` : le paquet magique (6 × `FF`, puis 16 fois l'adresse MAC, 102 octets) de chaque carte, en UDP avec `SO_BROADCAST`, ports 9 et 7, vers `destinations(&targets)` : la diffusion de chaque sous-réseau (celle d'Admin, sinon le /24 de l'adresse connue), 255.255.255.255, puis la dernière adresse IPv4 de chaque carte, en direct. Un refus sur une adresse est noté au journal et n'arrête rien ; `Ok(n)` dès qu'un paquet est parti, `WakeError::NoTarget` sans adresse MAC lisible, `WakeError::Network` si aucun n'est parti (message dans la langue de l'app). Pas d'attente réseau, mais à appeler hors du fil de l'interface. `targets_from_json` lit pour les ponts une liste de cibles, une cible seule ou l'info entière.
- `parse_mac`, `format_mac`, `magic_packet`, `broadcast(ip, prefix)` : les briques, testées (`cargo test -p kybernow-core wake`) ; `envoi_reel`, ignoré par défaut, envoie vraiment vers une carte fictive (`-- --ignored`).

**Piège Apple.** Sur iPhone, iPad et Apple TV, envoyer vers une adresse de diffusion (le sous-réseau comme 255.255.255.255) demande l'autorisation `com.apple.developer.networking.multicast`, qu'Apple accorde sur demande (formulaire du compte développeur, puis l'autorisation dans les fichiers `.entitlements`). Sans elle, l'envoi en diffusion échoue, souvent avec `EHOSTUNREACH`. L'envoi direct vers la dernière adresse IPv4 passe sans elle, mais ne réveille que si le réseau sait encore joindre cette adresse (cache ARP du routeur ou du commutateur, carte en veille qui répond encore à l'ARP), ce qui n'est pas toujours le cas après un arrêt complet. C'est pourquoi `send` tente toujours l'envoi direct en plus de la diffusion et réussit dès qu'un paquet part : sans l'autorisation, le réveil marche parfois, avec elle, comme ailleurs. Sur Mac, pas de restriction (l'autorisation « Réseau local », déjà demandée pour joindre Admin, suffit).

## Module d'entrées partagé (`src/input/`)

Les entrées de toutes les apps (Mac, Apple TV, iPhone et iPad par `apps/apple` et `InputCore.swift` ; Android par JNI ; Windows, Linux et webOS directement), au-dessus de la chaîne kynput de Kyber (`kynput_rs`, `kynput_sys`) : chaîne et file de secours, clavier, combinaison de la manette, tout relâcher, pointeur de l'ordinateur, presse-papiers (voir « Entrées » dans [apps/macos/README.md](../macos/README.md)).

| Module | Ce qu'il tient |
|---|---|
| `clock.rs` | `Clock` : l'horloge monotone de la plateforme ; `MonotonicClock` (`Instant`), `ManualClock` (tests) |
| `packet.rs` | `OwnedPacket` : un paquet kynput dont on tient la référence (libérée à la fin, clonée par essai d'envoi, cédée à kynput) ; `PacketSink` : l'envoi vers la chaîne, `KynputSink` (le puits d'entrée d'une chaîne kynput) |
| `packet_queue.rs` | `PacketQueue` : envoi à kynput, refus comptés (premier, puis tous les 100, puis le total à la fin), file de secours des paquets qui ne se rattrapent pas (touches, clics, boutons, branchements, retours au repos, texte, FlushKeys) réessayés toutes les 3 ms dans l'ordre jusqu'à succès ou 200 ms ; les paquets « une fois » (position, déplacement, molette, axe en mouvement) ne partent pas tant qu'elle n'est pas vide |
| `keymap.rs` | Usages HID → scancodes (`kynput_keycode_usb_to_kyber`), « ⌘ envoyée comme Ctrl », échange des touches `@#` et `<>` des claviers Apple ISO (la disposition n'est demandée que pour ces deux touches), Verr. maj. et Verr. num. reconnues comme des bascules, réglage ISO des claviers Apple ou inconnus |
| `pad_combo.rs` | `PadCombo` : L1 + R1 + Start + Select tenus 250 ms, touches tenues par manette, manettes connues, les six axes à remettre au repos à la capture (`PadAxis::ALL`), touches de navigation ; `PadButton`, `PadAxis` dans le vocabulaire de kynput, avec leurs valeurs de paquet (`kynput_rs` confond le stick droit et la gâchette droite dans `GamepadButton::to_u32`, on ne passe pas par lui) |
| `release_all.rs` | `ReleaseAll` : ce que l'appareil tient enfoncé (scancodes, boutons, modificateurs dans l'identifiant de la plateforme), tout relâcher (boutons puis touches), oubli, modificateurs réellement tenus renvoyés au retour |
| `pointer_layer.rs` | `PointerLayer` : position, forme (pixels ARGB tels que kynput les livre, `Shape::premultiplied` pour CGImage et Win32), point chaud et visibilité du pointeur de l'ordinateur ; position tenue par l'appareil (Apple TV, `holds_position`) que l'ordinateur reprend au repos des entrées (0,4 s), taille d'écran changée, renvoi après reconnexion ; `MouseMode`, `MouseModeRules` (Auto / Bureau / Jeu, 0,6 s de tolérance au pointeur remontré, recontrôle à 0,7 s) ; échelle et cadre du dessin |
| `clipboard.rs` | `ClipboardBridge` sur le trait `Pasteboard` : texte dans les deux sens (annonce, demande immédiate, réponse, 60 Ko), le presse-papiers de l'appareil regardé deux fois par seconde, notre propre écriture ne repart pas ; là où lire le texte dérange l'utilisateur (`reading_asks_user`, iOS), il n'est lu que sur demande de l'ordinateur et les changements sont annoncés sans lecture (`has_text`) ; fonctions C du correctif `kyber-patches/0002` |
| `gamepad_gate.rs` | Manettes lues par kynput via SDL, sur Mac (Windows lit aussi les siennes par SDL dans kynput, sans ce hook) : hook du routeur qui passe chaque paquet de manette à la combinaison et écarte, quand l'app a pris la manette (barre ouverte), appuis et axes en mouvement (touches de navigation rendues à l'app) ; relâchements et axes au repos passent toujours ; hors prise, axes immobiles au 1/512 écartés. `InputPipeline::install_gamepad_gate`, `set_gamepad_captured` (relâchements et six axes au repos envoyés à la prise) |
| `pipeline.rs` | `InputPipeline` : assemble le tout au-dessus d'une chaîne kynput : clavier, souris absolue et relative, molette, texte Unicode, FlushKeys, manettes produites par l'app (`gamepads_from_app` : Apple TV, iPhone, iPad, Android, Linux, webOS ; branchement, touches, axes filtrés au 1/512 de la course et regroupés par 4 ms, retours au repos réessayés, remise au repos), tout relâcher ; réception du pointeur (forme dédoublonnée, forme entièrement transparente écartée), des vibrations et du presse-papiers |

**Minuteries.** Aucun module ne crée de fil ni de file : chacun expose `poll()` et `next_deadline()`, et `InputPipeline::poll` les appelle tous (réessais, axes, combinaison, presse-papiers, paquets reçus). La plateforme appelle `poll()` au plus tard à l'échéance depuis la file qu'elle veut, ou laisse `InputPipeline::spawn_driver` tenir un fil (`kybernow-entrees`) qui dort jusqu'à l'échéance ou au réveil (`wake()`, tiré par tout ce qui crée une attente). Les tests avancent une `ManualClock` et appellent `poll()` eux-mêmes.

**Fils.** Toutes les méthodes sont appelables de tout fil (état sous verrou). Les rappels de kynput arrivent sur son fil réseau : le pointeur va à `PointerLayer`, qui prévient la vue hors de son verrou ; les vibrations à `InputPlatform::rumble` ; les paquets du presse-papiers sont copiés et traités par `poll()`, hors du verrou du routeur (la réponse y réentrerait).

### Ce que la plateforme fournit

```rust
// Horloge monotone (CLOCK_UPTIME_RAW, QueryPerformanceCounter…).
pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
}

// Presse-papiers texte (NSPasteboard, UIPasteboard, Android, Linux ; None sur tvOS et webOS ; celui de kynput sous Windows).
pub trait Pasteboard: Send + Sync {
    fn change_count(&self) -> u64;
    fn text(&self) -> Option<String>;
    fn set_text(&self, text: &str);
}

// Rappels vers l'interface (toutes les méthodes ont un défaut vide).
pub trait InputPlatform: Send + Sync {
    fn rumble(&self, rumble: GamepadRumble) {}  // fil réseau de kynput
    fn combo_formed(&self, pad: PadId) {}        // fil qui appelle poll()
    fn connected(&self) {}                       // fil de network_connected()
    fn navigate(&self, key: PadKey) {}           // fil de kynput, sous le verrou du routeur
}

// Envoi substitué (tests, bancs d'essai).
pub trait PacketSink: Send + Sync {
    fn consume(&self, packet: OwnedPacket) -> bool;
}
```

Et, en dehors des traits : la **capture des événements** (AppKit, GameController, Win32, Raw Input, SDL, Android) traduite en appels à `InputPipeline` (`key`, `mouse`, `mouse_move`, `mouse_button`, `wheel`, `type_text`, `gamepad_*`, `release_all`, `flush_keys`) ; l'**usage HID** de chaque touche et la **disposition ISO** (`keymap::scancode(hid, command_as_control, || iso)`) ; le **dessin** du pointeur sur le fil principal depuis `PointerLayer::set_on_changed` ; la **règle du mode de souris** (`MouseModeRules { needs_hidden_pointer }`, `true` sur Mac, Windows et Linux, `false` sur Apple TV, iPhone, iPad et Android) ; les manettes SDL du Mac et de Windows restent lues par kynput ; sur Mac, le hook du routeur (`install_gamepad_gate`) les filtre et rend les touches de navigation par `InputPlatform::navigate`.

Création : `InputPipeline::new(Options { display_id, host_size, clipboard, gamepads_from_app }, pointer, platform, clock)` rend un `Arc` ; `kynput()` donne la chaîne à `session::Platform::input_pipeline` ; `start_clipboard()` une fois le réseau relié ; `stop()` en fin de session (FlushKeys direct, total des pertes au journal), `shutdown()` pour arrêter les gestionnaires de kynput.

## Moteur de son (`src/audio/`)

Le son de toutes les apps, sans la sortie : la plateforme (Core Audio, WASAPI, AAudio, SDL) tire le son du moteur depuis son fil temps réel. Mac, Apple TV, iPhone et iPad y passent par `kn_audio_*` (`AudioPlayerCore.swift`), Windows directement (`apps/windows/src/audio.rs`), Android par AAudio (`apps/android/jni/src/audio.rs`), Linux et webOS par SDL (`apps/linux/src/device/media.rs`, `apps/webos/src/device/media.rs` ; en 5.1, webOS donne le son au lecteur du téléviseur).

| Module | Ce qu'il tient |
|---|---|
| `opus.rs` | libopus multistream : `Decoder` (stéréo, 5.1, 7.1 depuis l'en-tête `OpusHead` de `media.rs`), trame de masquage ; un encodeur pour les tests |
| `ring.rs` | Anneau d'échantillons à un producteur (fil réseau) et un consommateur (fil temps réel), sans verrou |
| `mix.rs` | Mixage descendant (G + 0,7 C + 0,7 surround G et arrière G, idem à droite, LFE ignoré ; 7.1 → 5.1 ; mono = moyenne) et `OutputOrder` (Opus, Windows, Core Audio 5.1 A / 7.1 C, ou sur mesure) |
| `engine.rs` | `AudioEngine` : `configure` / `decode` / `conceal` (ou `PacketHandler::packet`) depuis le fil réseau de Kyber, `fill(&mut [f32], frames)` depuis le fil temps réel ; tampon adaptatif ; `AudioStats` une fois par seconde |

Tampon : marge de 8 ms au départ, qui grandit de 5 ms après un tampon vide (60 ms au plus) et redescend d'1 ms toutes les 2 s sans trou (4 ms au moins) ; premier départ à la marge plus un paquet, reprise dès que la marge est là ; un tampon vide est comblé par une trame de masquage d'Opus (silence si le décodeur est occupé par le fil réseau à cet instant) ; dérive rattrapée par un échantillon sauté ou doublé sur 256 (sur 128 au-delà de 6 ms d'écart) jusqu'à ce que le plus bas niveau de la seconde retrouve la marge ; coupe sèche seulement au-delà de 10 ms d'avance. Un tampon plein (sortie arrêtée depuis une seconde) jette ce qui arrive ; la coupe sèche rattrape l'écart au redémarrage.

`fill` ne prend aucun verrou bloquant : l'anneau est sans verrou, le décodeur (trame de masquage) et les mesures sont pris par essai, et un essai manqué donne du silence pour cette demande. Le nombre de voies de la sortie est `out.len() / frames` : la plateforme le change en changeant la taille de son tampon (AirPods branchés sur un flux 5.1), et donne sa latence par `set_output_latency_ms` (tampon de la carte son plus ce qu'elle annonce) pour l'écart son-image des mesures (`take_stats`).

### Ce que la plateforme fournit

- La sortie elle-même : son fil, le nombre de voies qu'elle accepte (ou mixe), son ordre des voies (`AudioEngine::with_order`), sa latence ; sa recréation quand la sortie du système change.
- L'horloge (`Clock`, celle du module d'entrées).
- libopus à l'édition de liens : `build.rs` la demande à pkg-config (`PKG_CONFIG_PATH` vers le rootfs de Kyber sur le Mac) et se contente d'un avertissement sinon (l'app Windows la lie en statique, `apps/windows/build.rs`).

## Tester

Sur le Mac, avec le SDK Kyber compilé pour macOS (`third_party/kyber-desktop/rootfs-arm64-macos`, produit par les scripts de l'app Mac) :

```bash
cd apps && PKG_CONFIG_PATH=$PWD/../third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib cargo test -p kybernow-core
```

Les tests des entrées couvrent la table du clavier, ISO, Verr. maj. ; combinaison brève, tenue, relâchement complet, réglage coupé ; file de secours : accepté, refusé deux fois puis parti dans l'ordre, refusé sans fin, arrêt ; filtrage et regroupement des axes, remise au repos, tout relâcher, texte, pointeur dédoublonné, presse-papiers dans les deux sens ; ceux du son, un la de 440 Hz encodé en Opus, stéréo et 5.1 au rythme réel avec ± 8 ms d'à-coups et une perte sur 97, paquets de 10 ms, dérive de 0,2 % dans les deux sens, sortie arrêtée 200 ms puis coupe sèche, mixage et ordre des voies. Ils avancent une `ManualClock` à la main : ils ne durent qu'une seconde. `cargo test -p kybernow-core input` ou `… audio` pour une moitié. Ceux de la session (`… session`) couvrent le dernier recours avec une fausse console : quand il se déclenche et quand il ne se déclenche pas, une seule fois, refus d'Admin (toujours sans `force`), retour de la Station, délai dépassé, annulation pendant l'attente, et les textes.
