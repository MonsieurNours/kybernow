# Le cœur Rust dans les apps Apple

[English](coeur-apple.en.md) · Français

Les apps Mac, Apple TV, iPhone et iPad reposent sur le cœur client en Rust (`apps/core`, crate `kybernow-core`), celui de l'app Windows. Tout ce qui ne dépend pas de l'affichage y est : la session (dialogue avec Admin, Kyber, reconnexion, image clé, écran, latence, profils), les entrées et le son. Le décodage (VideoToolbox), le rendu (Metal), les vues et les API du système restent en Swift. Un seul code pour quatre apps.

## Ce qui va où

```
Swift (KyberKit, apps)                      Rust (kybernow-core, par kybernow-apple)
──────────────────────                      ────────────────────────────────────────
VideoPipeline, VideoDecoder, VideoRenderer  session : préparation, connexion, choix de
(VideoToolbox, Metal), AV1                  l'écran, démarrage et relance de l'image,
AudioPlayerCore : sortie Core Audio         reconnexion, écran virtuel redemandé, image
InputCore : capture des événements          clé (300 ms, 1,5 s), repli sans image (débit,
vers la chaîne d'entrées du cœur            4:2:0), taille de l'écran de l'ordinateur,
StreamView, MetalSurface, Passage,          politique de mode (« Remettre »), adaptation
SessionChrome, SessionStats, HomeView…      à l'écran de l'appareil, signal de vie, fin
SessionController : la fenêtre, la barre,   console : info, aperçu, appairage, empreinte
les capsules, le plein écran, l'écran du    latence : trajet d'une image, pertes, trous,
Mac, le Trousseau, la dernière image        mesures rejetées, médiane et p95 de la session
SessionEngine, ConsoleClientCore,           entrées : file de secours, clavier, manettes,
CoreBridge, LatencyReporting : le cœur      pointeur, tout relâcher, presse-papiers
vu de Swift                                 son : Opus, tampon adaptatif, dérive, mixage
                                            profils et débit, langue, mises à jour
```

Le cœur ne sait rien de l'affichage : la plateforme lui donne un `Platform` (fenêtre, où envoyer les paquets d'image et de son, la chaîne d'entrées kynput, les rappels d'état). Sur Mac, Apple TV, iPhone et iPad, c'est `SessionEngine.swift` (KyberKit commun) qui le fournit, à travers l'API C.

L'app Windows utilise la même session, le même module d'entrées et le même moteur de son ; son presse-papiers reste celui de kynput. Ce que la session sait pour les apps Apple (trajet d'une image et pertes dans la latence, demande d'image clé répétée toutes les 300 ms puis flux relancé à 1,5 s, repli quand aucune image ne vient en 4 s — débit ramené à 250 Mbit/s, puis 4:2:0, pour cette session seulement —, suivi de la taille de l'écran de l'ordinateur, politique de mode avec « Remettre », adaptation à un nouveau mode de l'écran de l'appareil, choix d'un écran dans la barre, plafond de débit, compte rendu de fin) est désactivable ou sans effet pour Windows, qui garde ses propres veilles d'image clé et de première image.

## Chaque app

KyberKit est un seul module, compilé par les trois projets Xcode, cœur compris : `SessionEngine`, `ConsoleClientCore`, `CoreBridge`, `InputCore`, `AudioPlayerCore` et `LatencyReporting` servent partout. Chaque plateforme ne garde que sa vue de session, sa surface Metal et la capture de ses événements.

- **Mac** (`apps/macos`) : AppKit, `StreamView`, `MetalSurface`, `RelativeMouse`, `KeymapMac` (table kVK → HID, disposition ISO) ; les manettes passent par SDL dans kynput, et la combinaison est suivie dans la chaîne d'entrées du cœur (`gamepad_gate.rs`). Presse-papiers par `NSPasteboard`.
- **Apple TV** (`apps/tvos`) : les fichiers communs de KyberKit sans ceux d'AppKit, plus `KyberKitTV` : vue de session UIKit, surface Metal, `GamepadBridge` et `KeyboardMouse` (GameController).
- **iPhone et iPad** (`apps/ios`) : les mêmes fichiers communs, la surface Metal, `KeyboardMouse` et `GamepadBridge` de l'Apple TV, et la vue de session tactile `KyberKitIOS/StreamView`. Presse-papiers par `UIPasteboard`, lu seulement quand l'ordinateur le demande (`pasteboard_reading_asks_user`). Voir [apps/ios/README.md](../apps/ios/README.md).

Sur Apple TV, iPhone et iPad, `GamepadBridge` suit lui-même la combinaison avant d'envoyer (menu, manette en souris, appui long) : il garde un `PadCombo` tenu par le cœur (`kn_combo_*`, avec son fil pour l'échéance des 250 ms) ; la chaîne d'entrées a sa combinaison désactivée. Le pointeur tenu par l'appareil (télécommande, manette, souris, toucher) est la position du `PointerLayer` du cœur (`holds_position`).

Les tests du son, des entrées et du presse-papiers sont ceux du cœur : `cargo test -p kybernow-core` (voir [apps/core/README.md](../apps/core/README.md)).

## Le pont : `apps/apple`, crate `kybernow-apple`

Une API C écrite à la main (préfixe `kn_`), en-tête `kybernow.h` produit par cbindgen au travers de `cargo cinstall` (cargo-c). Swift l'importe par le module `CKyberNow` (`Vendor/KyberNow/include/module.modulemap`).

Règles de l'API :

- **Objets opaques** (`KnConsole`, `KnSession`, `KnInput`, `KnPointer`, `KnAudio`, `KnCombo`) créés et libérés par le cœur ; aucune structure Rust n'est exposée.
- **Résultats composés en JSON** (`char *`, libéré par `kn_string_free`) : les réponses d'Admin, les réglages d'une session. Les mesures de chaque seconde (`KnSecond`) et les paquets (`KnPacket`) sont des structures C plates : chemin chaud, sans copie ni allocation.
- **Rappels** (`KnPlatform`) : des pointeurs de fonction et un `userdata` ; le cœur les appelle depuis ses propres fils (fil de session, fil réseau de Kyber pour les paquets). Swift remonte sur le fil principal ce qui touche l'interface ; les paquets vont directement au décodeur.
- **Paquets** : `video_packet` et `audio_packet` reçoivent `KnPacket` (format, configuration ou image, image clé, pts, données en place, trou avec le nombre d'échantillons manquants) ; le dernier appel d'un flux porte `KN_PACKET_CLOSED`, après quoi le cœur ne rappelle plus pour ce flux : Swift arrête alors son décodeur ou sa sortie du son.
- **Chaîne d'entrées** : `input_pipeline()` rend la chaîne kynput d'un `KnInput` (`kn_input_new`, sur la couche de pointeur `KnPointer` de la vue, `kn_input_kynput`) ; le cœur la passe à Kyber avec la session d'image. Swift ne fait que donner les événements captés (`kn_input_key`, `mouse`, `mouse_button`, `wheel`, `type_text`, `gamepad_*`, modificateurs tenus et resynchronisés) et dessiner le pointeur (`on_changed` de `KnPointerCallbacks`, forme en ARGB prémultiplié) ; les rappels d'interface (combinaison, touches de navigation, entrées reliées, vibrations, presse-papiers) passent par `KnInputCallbacks`.
- **Son** : un `KnAudio` par flux (`kn_audio_new`, ordre des voies de la sortie) reçoit les paquets (`configure`, `decode`, `conceal`) ; le fil temps réel de la sortie tire les échantillons par `kn_audio_fill` et les mesures par `kn_audio_take_stats`.
- **Latence** : le rendu Metal dit au cœur chaque image montrée (`kn_session_frame_displayed`, instants en µs sur `CLOCK_MONOTONIC`, `kn_now_micros`), remplacée, les octets reçus et les trous ; le cœur rend chaque seconde un `KnSecond` (médiane, trajet d'une image, pertes, mesures rejetées, aller-retour, débit) par `on_second`.
- **États** (`on_status`) : préparation, connexion, premières images, image, reconnexion (essai, maximum), fin, terminée (avec la raison si elle n'était pas voulue, et le compte rendu d'Admin : écrans et son rétablis).
- **Avis** (`on_notice`) : ce que la session montre dans une capsule, sans texte (Swift le traduit) : image relancée, mode de l'ordinateur changé ou revenu (avec la taille), repli (niveau), adaptation à l'écran (demandée, faite, refusée), « Remettre » proposé ou retiré.
- **Commandes** : arrêt, image clé perdue ou revenue, nouveaux réglages (profil depuis la barre), plafond de débit (fenêtre occultée), écran choisi, nouveau mode (adaptation à l'écran de l'appareil), remise du mode.
- **Journal** : le cœur écrit par la crate `log` ; `kn_set_log_callback` l'envoie à `KyberLog` (journal unifié, catégorie `core`).

## Compiler et livrer

Le cœur compile la crate Rust `kyclient` des sources de Kyber (`third_party/kyber-desktop/kysdk`), avec les correctifs d'`apps/macos/kyber-patches` (voir [kyber-changes.md](kyber-changes.md)).

**Mac** : bibliothèque dynamique `libkybernow.dylib`, dans `apps/macos/Vendor/Frameworks`, embarquée et signée par `embed-kyber.sh` comme les autres bibliothèques.

```bash
apps/macos/scripts/vendor-kyber.sh      # une fois : SDK Kyber → Vendor/
apps/macos/scripts/build-kyclient.sh    # libkynput et libkyclient avec les correctifs → Vendor/
apps/apple/scripts/build-apple.sh       # kybernow-core + kybernow-apple → Vendor/Frameworks/libkybernow.dylib, Vendor/KyberNow/include/kybernow.h
cd apps/macos && xcodegen generate && xcodebuild -scheme KyberNow -configuration Debug -derivedDataPath build build
```

`build-apple.sh` compile pour `aarch64-apple-darwin` avec le Rust stable du dépôt et refuse de tourner pendant que l'app est ouverte. Le pont lie libkynput et libvlc du SDK Kyber (chaîne d'entrées ; lecteur jamais appelé).

**Apple TV et iPhone/iPad** : bibliothèque statique dans `apps/<plateforme>/Vendor/KyberNow.xcframework` (appareil et simulateur), par `apps/apple/scripts/build-apple-mobile.sh tvos|ios`, déploiement 18.0.

- Apple TV : Rust nightly et `-Zbuild-std` (cible de rang 3). libkynput, libopus et les fonctions vides de VLC viennent du SDK de Kyber pour tvOS (`Vendor/KyberSDK.xcframework`, par `apps/tvos/scripts/build-kyber-tvos.sh`), lié à côté.
- iPhone et iPad : Rust stable. Pas de SDK Kyber à part : libkynput (construite par `apps/core/scripts/build-apple.sh --target aarch64-apple-ios` et `--target aarch64-apple-ios-sim`), libopus et les fonctions vides de VLC vont dans la même bibliothèque que le cœur ; l'en-tête de kynput va dans `Vendor/include` (module `CKyber`).

`apps/core/scripts/build-apple.sh` vérifie que la chaîne de dépendances du cœur compile pour chaque cible Apple. `scripts/release.sh` construit le cœur pour les trois apps ; `Vendor/` de l'Apple TV et de l'iPhone/iPad est publié avec chaque version (`KyberSDK-tvOS.zip`, `KyberSDK-iOS.zip`), que reprennent la CI et les scripts `sideload.sh`. Détails et pièges : [build-notes.md](build-notes.md#le-cœur-rust-des-apps-apple-appsapple).

## État

- Vérifié sur le matériel : session Mac complète, entrées et son compris ; session sur un iPhone 15 Pro Max (22/10/2026).
- Pas encore essayé : l'app Apple TV (installée sur une Apple TV, pas encore lancée) ; l'iPad (absent du profil de provisionnement) ; l'écran externe et Stage Manager ; le client Windows avec ses entrées et son son sur le cœur.
- L'app iPhone et iPad n'est pas encore distribuée : TestFlight interne à venir (profil App Store à créer) ; installation depuis un Mac avec Xcode en attendant ([apps/ios/SIDELOAD.md](../apps/ios/SIDELOAD.md)).
