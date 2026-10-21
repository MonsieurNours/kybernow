# Les apps Apple sur le cœur Rust

[English](coeur-apple.en.md) · Français

Décidé le 21/10/2026 : les apps Mac, Apple TV et iPhone/iPad reposent sur le cœur client en Rust (`apps/core`, crate `kybernow-core`), celui du client Windows. Tout ce qui ne dépend pas de l'affichage y passe : la session (console, Kyber, reconnexion, image clé, écran, latence, profils), puis les entrées, puis le son. Seuls le décodage (VideoToolbox), le rendu (Metal), les vues et les API du système restent en Swift. Un seul code pour quatre apps, et une app iPhone qui part sur le cœur dès le premier jour.

## Jalons

| Jalon | Contenu | Validé par |
|---|---|---|
| 1 | Le pont C (`apps/apple`) et la session du cœur sur **Mac** : `KyberClient`, `ConsoleClient`, `LatencyTracker` et la machine à états de `SessionController` remplacés par le cœur ; décodeur, rendu, son et entrées inchangés. **Écrit le 21/10/2026** (branche `claude/coeur-apple-jalon1` : Mac Debug, Apple TV simulateur et client Windows x64 compilés, tests du cœur au vert), à valider | Une session complète du Mac vers l'ordinateur, mêmes latences qu'en 0.3.0 |
| 2 | Les **entrées** en Rust (`apps/core/src/input`) : file de secours, clavier, combinaison manette, pointeur, tout relâcher, presse-papiers ; adoptées par le Mac et par Windows. **Écrit le 21/10/2026** (pont `kn_input_*`, `kn_pointer_*`, `kn_keymap_*` ; `InputCore.swift` ; hook des manettes SDL porté dans le cœur, `gamepad_gate.rs` ; Windows sans son `input.rs`), à valider | Tests hors ligne portés, session Mac et Windows |
| 3 | Le **son** en Rust (`apps/core/src/audio`) : Opus, tampon adaptatif, dérive, mixage ; Core Audio et WASAPI ne font plus que tirer les échantillons. **Écrit le 21/10/2026** (pont `kn_audio_*`, `AudioPlayerCore.swift` ; Windows sans le décodage de son `audio.rs`), à valider | Tests hors ligne portés, écart son-image inchangé |
| 4 | **Apple TV** sur le cœur (compilation nightly du cœur pour tvOS, GameController sur le module d'entrées). **Écrit le 21/10/2026** (`build-apple-mobile.sh tvos`, `KyberNow.xcframework` ; `SessionEngine`, `InputCore`, `AudioPlayerCore` communs au Mac et à l'Apple TV ; combinaison du pont de manettes sur le cœur, `kn_combo_*` ; les versions Swift de la session, de la console, de la latence, des entrées et du son supprimées), à valider | Session sur une Apple TV |
| 5 | **iPhone et iPad**, d'après les maquettes (https://claude.ai/artifact/4ef5m4JSRnzuitJcuCFjMX), sur le cœur dès le départ. **Écrit le 21/10/2026** (`apps/ios` : accueil, ajout, lancement, passage, session tactile, rangée de touches et menu Touches, mesures, réglages ; cœur compilé par `build-apple-mobile.sh ios` ; écran externe sur iPhone et iPad, Stage Manager ; simulateur seulement, voir apps/ios/README.md), à essayer sur un appareil | TestFlight interne |

Le client Windows adopte chaque module dans le même jalon que le Mac : aucun module n'existe en deux versions.

## Ce qui va où (jalon 1)

```
Swift (KyberKit, KyberNow)                 Rust (kybernow-core, par kybernow-apple)
──────────────────────────                 ────────────────────────────────────────
VideoPipeline, VideoDecoder, VideoRenderer  session : préparation, connexion, choix de
AudioPlayer (jusqu'au jalon 3)              l'écran, démarrage et relance de l'image,
InputPipeline, PacketQueue, Keymap,         reconnexion, écran virtuel redemandé, image
PadCombo, ReleaseAll, PointerLayer,         clé (300 ms, 1,5 s), repli sans image (débit,
ClipboardBridge (jusqu'au jalon 2)          4:2:0), taille de l'écran de l'ordinateur,
StreamView, MetalSurface, Passage,          politique de mode (« Remettre »), adaptation
SessionChrome, SessionStats, HomeView…      à l'écran de l'appareil, signal de vie, fin
SessionController : la fenêtre, la barre,   console : info, aperçu, appairage, empreinte
les capsules, le plein écran, l'écran du    latence : trajet d'une image, pertes, trous,
Mac, le Trousseau, la dernière image        mesures rejetées, médiane et p95 de la session
                                            profils et débit, langue, mises à jour
```

Le cœur ne sait rien de l'affichage : la plateforme lui donne un `Platform` (fenêtre, où envoyer les paquets d'image et de son, la chaîne d'entrées kynput, les rappels d'état). Sur Mac, c'est `SessionEngine.swift` qui le fournit, à travers l'API C.

## Le pont : `apps/apple`, crate `kybernow-apple`

Une API C écrite à la main (préfixe `kn_`), en-tête `kybernow.h` produit par cbindgen au travers de `cargo cinstall` (cargo-c, comme libkyclient), bibliothèque `libkybernow.dylib` posée dans `apps/macos/Vendor` par `apps/apple/scripts/build-apple.sh`. Swift l'importe par le module `CKyberNow` (`Vendor/KyberNow/include/module.modulemap`).

Règles de l'API :

- **Objets opaques** (`KnConsole`, `KnSession`) créés et libérés par le cœur ; aucune structure Rust n'est exposée.
- **Résultats composés en JSON** (`char *`, libéré par `kn_string_free`) : les réponses de la console, les réglages d'une session. Les mesures de chaque seconde (`KnSecond`) et les paquets (`KnPacket`) sont des structures C plates : chemin chaud, sans copie ni allocation.
- **Rappels** (`KnPlatform`) : des pointeurs de fonction et un `userdata` ; le cœur les appelle depuis ses propres fils (fil de session, fil réseau de Kyber pour les paquets). Swift remonte sur le fil principal ce qui touche l'interface ; les paquets vont directement au décodeur.
- **Paquets** : `video_packet` et `audio_packet` reçoivent `KnPacket` (format, configuration ou image, image clé, pts, données en place, trou avec le nombre d'échantillons manquants) ; le dernier appel d'un flux porte `KN_PACKET_CLOSED`, après quoi le cœur ne rappelle plus : c'est là que Swift rend l'objet retenu (`PacketSinkLease`, inchangé).
- **Chaîne d'entrées** (jalon 2) : `input_pipeline()` rend la chaîne kynput d'un `KnInput` (`kn_input_new`, sur la couche de pointeur `KnPointer` de la vue, `kn_input_kynput`) ; le cœur la passe à Kyber avec la session d'image. Swift ne fait que donner les événements captés (`kn_input_key`, `mouse`, `mouse_button`, `wheel`, `type_text`, `gamepad_*`, modificateurs tenus et resynchronisés) et dessiner le pointeur (`on_changed` de `KnPointerCallbacks`, forme en ARGB prémultiplié) ; les rappels d'interface (combinaison, touches de navigation, entrées reliées, vibrations, presse-papiers) passent par `KnInputCallbacks`.
- **Son** (jalon 3) : un `KnAudio` par flux (`kn_audio_new`, ordre des voies de la sortie) reçoit les paquets (`configure`, `decode`, `conceal`) ; le fil temps réel de la sortie tire les échantillons par `kn_audio_fill` et les mesures par `kn_audio_take_stats`.
- **Latence** : le rendu Metal dit au cœur chaque image montrée (`kn_session_frame_displayed`, instants en µs sur `CLOCK_MONOTONIC`, `kn_now_micros`), remplacée, les octets reçus et les trous ; le cœur rend chaque seconde un `KnSecond` (médiane, trajet d'une image, pertes, mesures rejetées, aller-retour, débit) par `on_second`.
- **États** (`on_status`) : préparation, connexion, premières images, image, reconnexion (essai, maximum), fin, terminée (avec la raison si elle n'était pas voulue, et le compte rendu de la console : écrans et son rétablis).
- **Avis** (`on_notice`) : ce que la session montre dans une capsule, sans texte (Swift le traduit) : image relancée, mode de l'ordinateur changé ou revenu (avec la taille), repli (niveau), adaptation à l'écran (demandée, faite, refusée), « Remettre » proposé ou retiré.
- **Commandes** : arrêt, image clé perdue ou revenue, nouveaux réglages (profil depuis la barre), plafond de débit (fenêtre occultée), écran choisi, nouveau mode (adaptation à l'écran du Mac), remise du mode.
- **Journal** : le cœur écrit par la crate `log` ; `kn_set_log_callback` l'envoie à `KyberLog` (journal unifié, catégorie `core`).

## Ce que le cœur a gagné au jalon 1

Pour égaler l'app Mac, `kybernow-core` porte désormais, en plus de ce qu'il avait pour Windows : le trajet d'une image et les pertes dans la latence (`latency.rs`, port de `LatencyTracker.swift`), la demande d'image clé répétée toutes les 300 ms puis le flux relancé à 1,5 s, le repli quand aucune image ne vient en 4 s (débit ramené à 250 Mbit/s, puis 4:2:0, pour cette session seulement), le suivi de la taille de l'écran de l'ordinateur (images décodées et liste de Kyber), la politique de mode avec « Remettre », l'adaptation à un nouveau mode de l'écran de l'appareil, le choix d'un écran dans la barre, le plafond de débit, et le compte rendu de fin. Tout cela est désactivable ou sans effet pour Windows, qui garde son comportement (ses propres veilles d'image clé et de première image).

## Compiler

```bash
apps/macos/scripts/vendor-kyber.sh      # une fois : SDK Kyber → Vendor/
apps/macos/scripts/build-kyclient.sh    # libkynput avec les correctifs (libkyclient n'est plus liée par l'app)
apps/apple/scripts/build-apple.sh       # kybernow-core + kybernow-apple → Vendor/Frameworks/libkybernow.dylib, Vendor/KyberNow/include/kybernow.h
cd apps/macos && xcodegen generate && xcodebuild -scheme KyberNow -configuration Debug -derivedDataPath build build
```

`build-apple.sh` compile pour `aarch64-apple-darwin` avec le Rust stable du dépôt ; les cibles iOS (stable) et tvOS (nightly, `-Zbuild-std`) viennent du jalon 4 et de `apps/core/scripts/build-apple.sh`. Les tests du cœur : `cargo test -p kybernow-core` (voir `apps/windows/README.md`).

## L'app Apple TV depuis le jalon 4

KyberKit est partagé entre les deux projets Xcode, cœur compris : `SessionEngine`, `ConsoleClientCore`, `CoreBridge`, `InputCore`, `AudioPlayerCore` et `LatencyReporting` servent au Mac et à l'Apple TV (et serviront à iOS) ; chaque plateforme ne garde que sa vue de session, sa surface Metal et la capture de ses événements (AppKit et `KeymapMac` sur Mac, `GamepadBridge` et `KeyboardMouse` de GameController sur Apple TV). Les anciens fichiers Swift (`KyberClient`, `ConsoleClient`, `LatencyTracker`, `AudioPlayer`, `InputPipeline`, `PacketQueue`, `Keymap`, `PadCombo`, `PointerLayer`, `ReleaseAll`, `ClipboardBridge`, `GamepadGate`) et leurs tests hors ligne ont disparu : le cœur a les siens.

Sur tvOS, le cœur est une bibliothèque statique (`Vendor/KyberNow.xcframework`, par `apps/apple/scripts/build-apple-mobile.sh tvos`) liée à côté du SDK de Kyber pour tvOS (`Vendor/KyberSDK.xcframework`), dont l'app ne prend plus que libkynput, libopus et les fonctions vides de VLC. `GamepadBridge` suit lui-même la combinaison avant d'envoyer (menu, manette en souris, appui long) : il garde un `PadCombo`, désormais tenu par le cœur (`kn_combo_*`, avec son fil pour l'échéance des 250 ms) ; la chaîne d'entrées de l'Apple TV a sa combinaison désactivée. Le pointeur tenu par l'Apple TV (télécommande, manette, souris) est la position du `PointerLayer` du cœur (`holds_position`).
