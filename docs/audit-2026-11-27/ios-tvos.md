# Audit du 27/11/2026 : app iPhone/iPad et app Apple TV

Périmètre : `apps/ios` (`KyberNowIOS`, `KyberKitIOS`, projet normal et sideload, `tests/screen-pad`) et `apps/tvos` (`KyberNowTV`, `KyberKitTV`, `TopShelf`, `UITests`), état de `main` à `1fb289c6` (0.11.7). Les fichiers de `apps/macos/KyberKit` compilés dans ces deux apps (`AudioPlayerCore`, `CameraCapture`, `SessionEngine`, `InputCore`) ne sont repris ici que lorsque le défaut ne se voit que sur iPhone, iPad ou Apple TV ; le reste relève de l'audit Mac.

Méthode : lecture seule, aucune compilation, aucun appareil. Quatre relectures parallèles (session et flux iOS, session et entrées tvOS, duplication et code mort, micro/caméra/haptique/clavier/docs), puis revérification à la main de chaque constat retenu. **Vérifié** = chemin de code relu de bout en bout ; **plausible** = le code est relu mais l'effet dépend du comportement d'iOS ou de tvOS, à confirmer sur appareil. Rien de ce qui a été réglé par l'audit d'octobre (`docs/archives/audit-2026-10-20.md` : `handlerQueue`, haptique continue, quatre manettes, Siri Remote, combinaison 250 ms) n'est repris ; aucune régression n'y a été trouvée.

Note : `project.sideload.yml` diffère beaucoup de `project.yml` (0.9.2, fichiers absents) mais c'est un fichier **généré** par `scripts/sideload.sh` à partir de `project.yml` et ignoré par git : ce n'est pas un écart. La transformation du script (`sideload.sh:64-83`) est cohérente.

## Synthèse

| Id | Catégorie | Constat | Où | Effort | Confiance |
|---|---|---|---|---|---|
| IOS-01 | bug critique | Chaque session d'ordinateur reste en mémoire (cycle par la rangée du clavier) | `SessionView.swift:104-108, 1380-1390` | S | vérifié |
| IOS-02 | bug | Écran externe branché en session : ni mode adapté, ni HDR de la TV | `SessionController.swift:623, 847-865` | S | vérifié |
| IOS-03 | bug | Aucune reprise après un appel, Siri ou une alarme (son du jeu et micro) | `AudioPlayerCore.swift:130-140, 441-465` | M | vérifié (absence) |
| IOS-04 | bug | Mise en veille jamais empêchée pendant une session (iOS et tvOS) | `SessionController.swift` (les deux apps) | S | vérifié (absence) |
| IOS-05 | bug | Ctrl/Alt/⊞ collants : restent enfoncés au repli du clavier, affichés à tort après `releaseAll` | `SessionController.swift:1353`, `StreamView.swift:259-263` | S | vérifié |
| IOS-06 | bug | Glisser ou clic du Pencil qui reste enfoncé sur le PC | `StreamView.swift:408, 482, 521` | S | vérifié |
| IOS-07 | bug | Clavier physique + clavier de l'écran : chaque frappe part deux fois | `StreamView.swift:550-566`, `KeyboardMouse.swift:104-110` | S | plausible |
| IOS-08 | bug | iPad, souris en mode Bureau : deux sources de position, molette en double | `KeyboardMouse.swift:116-139`, `StreamView.swift:299-304, 469-471` | M | plausible |
| IOS-09 | bug | Pas de `prefersPointerLocked` : Dock et Centre de contrôle en plein jeu à la souris | `StreamView.swift:713-716` | M | plausible |
| IOS-10 | bug | Couper le micro peut laisser la session audio en `.soloAmbient` | `AudioPlayerCore.swift:448, 520-526` | S | plausible |
| IOS-11 | bug | Caméra éteinte pendant son ouverture : elle repart quand même | `SessionEngine.swift:870-881` | S | vérifié |
| IOS-12 | bug | `CameraCapture` : `deliver` hors de sa file, `onProblem(.noEncoder)` à chaque image | `CameraCapture.swift:330-342, 389` | M | plausible |
| IOS-13 | bug | Changement de définition par le PC : `streamMode` et `modeLabel` figés (iOS seul) | `SessionController.swift:1081-1084` | S | vérifié |
| IOS-14 | bug | `PlayStationKeyMouse` sans `handlerQueue` : course ou souris sur le fil principal | `PlayStationKeyMouse.swift:30-37, 73-106` | S | plausible |
| IOS-15 | bug | Presse-papiers lu sur le fil de kynput (alerte « Autoriser à coller ? ») | `InputCore.swift:523-526` | M | plausible |
| IOS-16 | bug | Perte du premier plan pendant une reconnexion : rien n'est relâché | `SessionController.swift:900` | S | vérifié |
| IOS-17 | bug | `connect`/`watch` ne vérifient pas `consoleSession == nil` ; `importCloud` sans garde démo | `AppModel.swift:298, 317, 354` | S | plausible |
| IOS-18 | optimisation | Animations SwiftUI sans fin sous la session (accueil, `Passage`) | `SessionView.swift:40-45, 337`, `KyberNowApp.swift:91-92` | S/M | plausible |
| IOS-19 | optimisation | Pointeur : deux sauts de file et `contents` refait à chaque événement (iOS et tvOS) | `StreamView.swift:120, 151-157` (iOS), `:100-106, 261` (tvOS) | S | vérifié |
| IOS-20 | optimisation | Fin de session : instantané GPU synchrone sur le fil principal | `SessionController.swift:1589` | S | plausible |
| IOS-21 | factorisation | ~600 lignes de logique de session en double iOS ↔ tvOS (et Mac) | `SessionController.swift` (×3) | L | vérifié |
| IOS-22 | factorisation | Hôte « visiteur » construit 9 fois ; l'emprunt diverge (souris) | `SessionController.swift:258-330` (iOS), `222-300` (tvOS) | S | vérifié |
| IOS-23 | factorisation | `Theme`, `HostImagery`, `AudioOutput`/`Preferences` en double ou en triple | `Theme.swift`, `Platform.swift` | M | vérifié |
| IOS-24 | factorisation | `StatsFormat` refait `LinkFormat` déjà compilé dans l'app | `SessionStats.swift:374-404` | S | vérifié |
| IOS-25 | factorisation | `PlayStationSession` : ~55 lignes communes iOS ↔ tvOS ↔ Mac, `setUpscale` divergent | `PlayStationSession.swift` (×3) | M | vérifié |
| IOS-26 | code mort | Fonctions, vue et propriétés iOS sans appel | `AppModel.swift:283`, `SessionController.swift:1257, 1359`… | S | vérifié |
| IOS-27 | code mort | Icônes et clés de traduction inutilisées (iOS et tvOS) | `Assets.xcassets/Icons`, `Localizable.xcstrings` | S | vérifié |
| IOS-28 | doc | Permission micro/caméra refusée : pas d'« Ouvrir les Réglages » en session (`link-ui.md`) | `SessionController.swift:566, 702, 785` | S | vérifié |
| IOS-29 | doc | `tests/screen-pad` n'est lancé nulle part (ni CI ni script) | `apps/ios/tests/screen-pad` | S | vérifié |
| IOS-30 | bug | AZERTY déduit du premier clavier activé, pas du clavier branché | `KeymapIOS.swift:58-60` | S | plausible |
| TVOS-01 | bug | Manette rallumée en session : Home renvoie à l'accueil de tvOS | `GamepadBridge.swift:575-578` | S | vérifié |
| TVOS-02 | bug | Pendant « Reconnexion à … », Retour ne fait rien et le menu ne s'ouvre pas | `SessionView.swift:18, 35`, `SessionController.swift:1040` | S | vérifié |
| TVOS-03 | bug | Fil principal en `queue.sync` sur la file des entrées (structure du gel 0.11.7) | `GamepadBridge.swift:165, 262, 328, 350, 381, 400, 471`, `KeyboardMouse.swift:48, 77` | M | plausible |
| TVOS-04 | bug | Création des moteurs Core Haptics sur la file des entrées | `GamepadBridge.swift:573, 1514, 1554` | S | vérifié |
| TVOS-05 | bug | Manette en souris : le pointeur continue de filer quand l'app perd la main | `GamepadBridge.swift:522-536, 1188` | S | plausible |
| TVOS-06 | bug | Partie sur console : manette arrivée en cours de partie, moteurs « hors session » | `GamepadBridge.swift:574` | S | vérifié |
| TVOS-07 | bug | Vibrations : pas d'arrêt en arrière-plan, pas de reprise après `stoppedHandler` | `GamepadBridge.swift:522-536, 1330, 1676-1684` | S | plausible |
| TVOS-08 | bug | Vitesse du pointeur manette calée sur l'ancienne définition du PC | `GamepadBridge.swift:235, 330`, `StreamView.swift:162-169` | S | vérifié |
| TVOS-09 | bug | PlayStation : après la fin, le pont reste en mode console jusqu'à « Fermer » | `PlayStationSession.swift:198-208, 302` | S | vérifié |
| TVOS-10 | bug | Panneaux « Son » et « Taper du texte » : Retour ferme tout le menu | `SessionMenu.swift:611-640, 667-689` | S | plausible |
| TVOS-11 | bug | `fail()` ne nettoie pas comme `end()` (`routeObserver`, `padMouse`) | `SessionController.swift:1361-1375` | S | vérifié |
| TVOS-12 | bug | Minuteries non annulées qui effacent des messages récents | `SessionController.swift:974, 1060-1062` | S | vérifié |
| TVOS-13 | bug | Tuile « Caméra » non tenue à jour, appareil choisi ignoré | `SessionController.swift:990, 996`, `SessionMenu.swift:527-528` | S | plausible |
| TVOS-14 | bug | Repli AV1 forcé sans tester le décodeur (iOS teste) | `SessionController.swift:304` | S | plausible |
| TVOS-15 | optimisation | Minuterie 120 Hz de la manette en souris toujours armée en profil Bureau | `GamepadBridge.swift:1166-1177` | S | vérifié |
| TVOS-16 | optimisation | À chaque appui : `String(describing:)` et appel FFI `combo.enabled` | `GamepadBridge.swift:750-754, 1412` | S | vérifié |
| TVOS-17 | optimisation | Statistiques `@Published` : toute la vue de session recalculée chaque seconde | `SessionController.swift:40-41` | M | plausible |
| TVOS-18 | optimisation | Top Shelf réécrit à chaque changement du magasin ; image à URL fixe | `AppModel.swift:166-169, 557-566`, `Platform.swift:207-209` | S | vérifié |
| TVOS-19 | optimisation | Motifs courts : `CHHapticPattern` recréé toutes les 0,26 s | `GamepadBridge.swift:1832` | S | vérifié |
| TVOS-20 | factorisation | `phosphor-icons.mjs` effacerait 14 icônes s'il était relancé ; pas de script iOS | `scripts/phosphor-icons.mjs:13-26` | S | vérifié |
| TVOS-21 | factorisation | Deux générateurs de QR à côté de `Share.qrImage` | `SettingsView.swift:766`, `AddHostView.swift:485-492` | S | vérifié |
| TVOS-22 | code mort | `select(_:)`, `sendCode`, `Theme.bar/gutter/top`, `onScreenHDR`, `MouseMode`, `PadCombo.axes/navigationKey` | voir détail | S | vérifié |
| TVOS-23 | doc | Aucun test d'interface sur le menu de session | `UITests/RemoteNavigationTests.swift` | S | vérifié |
| TVOS-24 | doc | `link-ui.md`, `micro.md` et SIDELOAD en écart avec le code | `docs/link-ui.md:1068`, `docs/micro.md:52`, `SIDELOAD.md:14, 69` | S | vérifié |

**Décompte** (54 constats) : 1 bug critique (IOS-01), 31 bugs (17 iOS, 14 tvOS), 8 optimisations, 7 factorisations, 3 constats de code mort (une quarantaine de symboles, icônes et clés), 4 de documentation et tests.

---

## iPhone et iPad

### IOS-01 — Chaque session d'ordinateur reste en mémoire · bug critique · S · vérifié

`SessionView.swift:104-108` : `session.streamView.keyboardAccessory = row.view`. `KeyboardRow.init` (`:1380-1390`) crée une `UIHostingController(rootView: KeyboardRowView(session: session))` et l'attache à sa propre vue par `objc_setAssociatedObject(view, &KeyboardRow.key, host, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)`. `KeyboardRowView` tient `@ObservedObject var session` (référence forte).

Deux cycles : `SessionController → streamView → keyboardAccessory (host.view) → host (objet associé) → rootView → SessionController` ; et `host ↔ host.view` à lui seul. `keyboardAccessory` n'est remis à `nil` nulle part (seule affectation du dépôt : `SessionView.swift:107`).

Scénario : chaque connexion puis déconnexion laisse le contrôleur, la `StreamView`, la surface Metal, le `SessionEngine` (rendus, décodeurs, tampons), le `PointerLayer` et les modèles Share/prêt en mémoire. Après une soirée de sessions, la mémoire de l'app ne fait que monter, jusqu'à l'éviction par iOS (jetsam).

Correctif : garder la `UIHostingController` dans `KeyboardRow` (déjà tenu en `@State`) au lieu de l'objet associé ; remettre `streamView.keyboardAccessory = nil` dans `end()` et `fail()` ; donner à `KeyboardRowView` une référence faible ou un petit modèle dédié. Vérifier ensuite avec le graphe mémoire d'Xcode qu'aucune `SessionController` ne survit.

### IOS-02 — Écran externe branché en session : ni mode adapté, ni HDR de la TV · bug · S · vérifié

`SessionController.swift:623` : `ExternalDisplay.shared.$scene.dropFirst().sink { … useExternal(scene != nil, adapt: true) }`. Un `@Published` émet au `willSet` : quand le bloc tourne, `ExternalDisplay.shared.scene` (donc `.screen`, `ExternalDisplay.swift:27`) vaut encore l'ancienne valeur. Dans `useExternal` (`:847-865`), `updateScreenHDR()` lit `imageScreen` (l'écran de l'appareil) et `display.mode(for:)` rend `nil` (`ExternalDisplay.swift:30-31`), donc `engine?.setMode` n'est jamais appelé.

Scénario : session à la taille de l'iPhone, branchement d'un téléviseur 4K. L'image passe sur la TV mais reste à la définition de l'iPhone ; iPhone HDR et TV SDR : la Station continue d'envoyer du HDR. Le chemin de démo (`:1386`) fait déjà `.receive(on: DispatchQueue.main)`, ce qui évite le piège.

Correctif : `.receive(on: DispatchQueue.main)` avant `.sink`, ou passer `scene?.screen` en paramètre de `useExternal`.

### IOS-03 — Aucune reprise après un appel, Siri ou une alarme · bug · M · vérifié (absence), effet plausible

`grep interruptionNotification|mediaServicesWereReset` sur `apps/ios`, `apps/tvos`, `apps/macos` : aucune occurrence. La sortie active la session (`AudioPlayerCore.swift:130-140`) puis ne la surveille plus ; `MicrophoneCapture` n'écoute que `.AVAudioEngineConfigurationChange`.

Scénario : appel entrant en pleine session sur iPhone, raccrochage : l'unité de sortie et l'`AVAudioEngine` du micro restent arrêtées, plus de son du jeu, et la tuile « Micro » dit toujours « Envoyé ». Même chose pour Siri sur Apple TV, côté sortie.

Correctif : observer `AVAudioSession.interruptionNotification` (à `.ended` : `setActive(true)`, `AudioOutputUnitStart`, relance de `MicrophoneCapture.start()` si le micro était allumé) et `mediaServicesWereResetNotification` (reconstruire les unités). Code partagé : à coordonner avec l'audit Mac.

### IOS-04 — Mise en veille jamais empêchée pendant une session (iOS et tvOS) · bug · S · vérifié (absence), effet plausible

`isIdleTimerDisabled` n'apparaît nulle part dans les apps Apple. L'image passe par Metal ou `AVSampleBufferDisplayLayer`, pas par `AVPlayer` : rien ne retient la veille.

Scénario : mode « Regarder », invité spectateur, ou partie où l'on ne touche plus l'écran (manette Bluetooth sur iPhone, vidéo plein écran sur le PC) : l'iPhone se verrouille, l'Apple TV lance l'économiseur puis se met en veille ; la session tombe.

Correctif : `UIApplication.shared.isIdleTimerDisabled = true` à l'entrée en `.streaming` (et dans `PlayStationSession`), `false` dans `end()`, `fail()`, `finish()`, dans les deux apps.

### IOS-05 — Touches collantes : enfoncées au repli du clavier, affichées à tort après `releaseAll` · bug · S · vérifié

`SessionController.swift:1353` : `func keyboardHidden() { keyboardShown = false }`, rien de relâché. `StreamView.releaseAll()` (`:259-263`) relâche tout sur le PC mais laisse `sticky` (`:582`) et la rangée inchangés ; `releaseStickyModifiers()` (`:618`) est privé.

Scénarios : (1) Ctrl collant allumé, toucher l'image pour replier le clavier : Ctrl reste enfoncé sur le PC, le clic suivant devient Ctrl+clic. (2) Ctrl allumé, passage en arrière-plan puis retour : la rangée dit « Ctrl tenue · la prochaine touche part avec elle », mais le cœur a relâché Ctrl ; « c » part seul.

Correctif : rendre `releaseStickyModifiers()` public et l'appeler dans `keyboardHidden()` ; dans `releaseAll()` et `detach()`, vider `sticky` et appeler `onStickyChanged`.

### IOS-06 — Glisser ou clic du Pencil qui reste enfoncé sur le PC · bug · S · vérifié

`held(_:)` (`StreamView.swift:408`) commence par `guard !touchesSuspended, let input else { return }`, y compris pour `.ended` ; `pencil(_:contact:)` (`:521`) a la même garde plus `!isRelative`. `pencilMouseDown` (`:482`) n'est remis à zéro ni dans `releaseAll()` ni dans `detach()`.

Scénarios : glisser au maintien, puis la barre s'ouvre (deuxième doigt sur la languette, combinaison manette : `touchesSuspended = open && !external`, `SessionController.swift:1254`) ; le doigt se lève, le relâchement ne part jamais, la fenêtre du PC reste collée au curseur. Pencil face à une Station sans stylet : le jeu passe en relatif pendant un trait, le clic reste tenu ; après `releaseAll`, `pencilMouseDown` reste vrai et le trait suivant part sans appui.

Correctif : traiter toujours les fins de geste (relâcher si `dragging` ou `pencilMouseDown`) avant les gardes ; relâcher le glisser dans un `didSet` de `touchesSuspended` ; remettre `pencilMouseDown = false` dans `releaseAll` et `detach`.

### IOS-07 — Clavier physique et clavier de l'écran : frappe en double · bug · S · plausible

La vue est `UIKeyInput` (`StreamView.swift:550-566` : `insertText` → `input.type(text)`, `"\n"` → `tap(0x28)`) et `KeyboardMouse` reçoit les mêmes touches par `GCKeyboard` (`KeyboardMouse.swift:104-110`). Quand la vue est premier répondeur, UIKit livre aussi les frappes du clavier physique en `insertText`. Brancher un clavier physique cache le clavier de l'écran (`keyboardShown` passe à faux par `keyboardWillHide`) mais la vue reste premier répondeur.

Scénario : iPad + Magic Keyboard, bouton « Clavier » touché pour avoir la rangée Échap/Tab : « a » donne « aa » sur le PC, Entrée part deux fois.

Correctif : surcharger `pressesBegan/Ended/Cancelled` dans `StreamView` pour absorber les appuis portant un `key` (GameController s'en charge), ou ignorer `insertText`/`deleteBackward` quand `GCKeyboard.coalesced != nil` ; `resignFirstResponder` quand le clavier de l'écran se cache.

### IOS-08 — iPad, souris ou trackpad en mode Bureau : deux sources de position · bug · M · plausible

Hors mode relatif, `KeyboardMouse.swift:116-131` déplace le pointeur avec les deltas bruts de `GCMouse` (`pointer.move(dx:dy:)`), pendant que `StreamView.swift:469-471` (`hovered`) envoie la position absolue du pointeur iPadOS. La molette arrive aussi deux fois : `KeyboardMouse.swift:139` (GCMouse) et `StreamView.swift:299-304` (`allowedScrollTypesMask = .all`).

Scénario : souris Bluetooth ou Magic Keyboard sur iPad, profil Bureau : le curseur du PC saute entre deux positions (accélération iPadOS d'un côté, deltas bruts de l'autre), un cran de molette défile deux fois.

Correctif : sur iOS, ignorer `mouseMovedHandler` et la molette de `GCMouse` tant que `!input.isRelative` (UIKit sert), garder ses boutons 4/5.

### IOS-09 — Pas de verrouillage du pointeur sur iPad · bug · M · vérifié (absence), effet plausible

Aucune occurrence de `prefersPointerLocked` ; `StreamView.swift:713-716` ne fait que cacher le pointeur ; `PlayStationKeyMouse.swift:82-85` lit les deltas `GCMouse`.

Scénario : souris des jeux (relatif) ou partie PlayStation à la souris : le pointeur invisible d'iPadOS atteint un bord, le Dock, le sélecteur d'apps ou le Centre de contrôle s'ouvrent en plein jeu.

Correctif : un contrôleur hôte qui rend `prefersPointerLocked = isRelative || partie console`, avec `setNeedsUpdateOfPrefersPointerLocked()` à chaque bascule (verrouillage effectif en plein écran seulement).

### IOS-10 — Couper le micro peut laisser la session en `.soloAmbient` · bug · S · plausible

`AudioPlayerCore.swift:448` mémorise `previous = (session.category, …)` au premier `start()` du micro ; `stop()` (`:520-526`) y revient. La sortie ne pose `.playback` que si le micro ne tient pas la session (`:134`). Si le micro démarre avant le premier paquet audio (`kn_session_set_microphone` part au départ, `SessionEngine.swift:719`), `previous` vaut la catégorie par défaut d'iOS, `.soloAmbient` (aucun `setCategory` ailleurs dans `apps/ios`).

Scénario : micro retenu allumé pour ce PC, coupé en session : le son du jeu obéit au bouton silencieux et s'arrête au verrouillage.

Correctif : à `stop()`, revenir toujours à `.playback`/`.default` (avec `setPreferredIOBufferDuration` et le multicanal), ou poser `.playback` au lancement de l'app.

### IOS-11 — Caméra éteinte pendant son ouverture : elle repart quand même · bug · S · vérifié

`SessionEngine.swift:870-881` : après `await camera.open(...)`, `kn_camera_start(...)` puis `startCameraPolling()` partent sans vérifier que la caméra est toujours demandée.

Scénario : « Éteindre la caméra » touché pendant l'ouverture ou la demande d'autorisation : `cameraOff()` envoie STOP, puis START part ; le sondage remet l'état à « envoyée », puce rouge allumée, capture fermée, image grise côté Station.

Correctif : un numéro de génération incrémenté par `cameraOn`/`cameraOff`, vérifié après l'`await` ; sinon `camera.close()` et retour. Code partagé (Mac aussi).

### IOS-12 — `CameraCapture` : rappel hors de sa file, avis en rafale · bug · M · plausible

`CameraCapture.swift:342` : le rappel de `VTCompressionSessionEncodeFrame` appelle `self.deliver(encoded)` sur un fil de VideoToolbox, qui lit `firstPTS` (`:389`) écrit par `captureOutput` (`:339`) sur `queue` ; `SessionEngine.cameraOn` (`:854-869`) réécrit `send`, `onProblem`, `onInterrupted` depuis le fil principal pendant la capture (changement avant/arrière). `:330-332` : `guard makeEncoder(...) else { onProblem?(.noEncoder); return }` à chaque image.

Scénarios : changement de caméra en plein envoi → accès concurrent à une fermeture Swift, plantage rare ; encodeur indisponible → 30 avis et 30 `cameraOff()` par seconde sur le fil principal.

Correctif : `deliver` repassé sur `queue` ; fermetures posées une fois ; drapeau `failed` et `stopRunning()` au premier échec d'encodeur.

### IOS-13 — Changement de définition par le PC : `streamMode` figé (iOS seul) · bug · S · vérifié

`SessionController.swift:1081-1084` (cas `.modeChanged`) n'affiche que l'avis. tvOS (`SessionController.swift:966-968`) et le Mac mettent aussi `streamMode` et `modeLabel` à jour. Conséquence iOS : ligne de mode des statistiques (`SessionStats.swift:242`) et `recordSession`/débit de fin (`:1592-1594`) faux après un changement de mode.

Correctif : reprendre les deux lignes de tvOS (et, à terme, IOS-21).

### IOS-14 — `PlayStationKeyMouse` sans file attitrée · bug · S · plausible

`PlayStationKeyMouse.swift:73-91` ne pose aucun `handlerQueue` ; `KeyboardMouse` (`:102, 114`) a pu laisser `handlerQueue = InputQueue.device` sur le même `GCKeyboard`/`GCMouse`. `suspended` et `consoleKeyboard` (`:30-37, 83, 106`) sont lus sans protection.

Scénarios : après une session d'ordinateur, une partie PlayStation : rappels sur la file des entrées pendant que le fil principal écrit `suspended` (course). Sans session avant : chaque mouvement de souris (jusqu'à 1 000 Hz) appelle le cœur sur le fil principal.

Correctif : `handlerQueue` dédié (ou `InputQueue.device`) posé explicitement ; `suspended`/`consoleKeyboard` sous le verrou existant.

### IOS-15 — Presse-papiers lu sur le fil de kynput · bug · M · plausible

`InputCore.swift:523-526` lit `UIPasteboard.general.string` depuis le rappel `Clipboard::answer()` du cœur. Sur iOS, une lecture après une copie dans une autre app affiche « Autoriser à coller ? » et bloque le fil appelant pendant l'alerte : forme et position du pointeur, vibrations retenues, image figée par la désactivation de la scène. C'est la même famille que le gel 0.11.7 (fil de kynput qui attend l'interface).

Correctif : répondre au cœur de façon asynchrone après une lecture sur le fil principal, ou n'offrir le texte qu'au retour au premier plan (`UIPasteboard.hasStrings` d'abord, sans alerte).

### IOS-16 — Perte du premier plan pendant une reconnexion · bug · S · vérifié

`SessionController.swift:900` : `guard …, self.phase == .streaming else { return }`. Verrouillage pendant « Reconnexion… » ou dans les 0,9 s avant l'image : ni `releaseAll`, ni débit plafonné, ni `setOccluded(true)`.

Correctif : agir pour toute phase hors `.ending`, `.ended`, `.failed`.

### IOS-17 — Gardes de lancement plus faibles que sur tvOS · bug · S · plausible

`AppModel.swift:317` (`connect`) et `:354` (`watch`) : `guard session == nil, …` ; tvOS ajoute `consoleSession == nil` (`AppModel.swift:401, 431`). `importCloud` (`:298`) n'a pas le `!store.demo` de tvOS (`:353`). Scénario : lien universel ou Top Shelf/raccourci qui lance une session d'ordinateur pendant une partie PlayStation : deux sessions tiennent manettes et son.

Correctif : aligner sur tvOS.

### IOS-18 — Animations SwiftUI sans fin sous la session · optimisation · S/M · plausible

`Passage` reste dans l'arbre jusqu'à `.ended`, à opacité 0 (`SessionView.swift:40-45, 337`), avec deux `HostBackdrop` dont la dérive est en `repeatForever` (`apps/tvos/KyberNowTV/Backdrops.swift:61-64`, partagé) ; `HomeView` reste rendue sous la session (`KyberNowApp.swift:91-92`) avec ses dérives et pulsations (`HomeParts.swift:85, 325, 482`). Le Mac a documenté ce coût (`apps/macos/KyberNow/HostImagery.swift:155-158` : 50 à 70 % d'un cœur) et l'a contourné par Core Animation.

Correctif : retirer `Passage` après l'ouverture (`@State` posé à 1,2 s) ; ne pas rendre `HomeView` (ou couper ses animations) quand `app.inSession` ; à terme, porter `BackdropLayers` en UIKit.

### IOS-19 — Pointeur : deux sauts de file et `contents` refait à chaque événement · optimisation · S · vérifié

iOS : `PointerLayer(queue: InputQueue.device, …)` (`StreamView.swift:120`), puis `DispatchQueue.main.async` (`:151-157`) ; tvOS : `:100-106`, et `contents` réaffecté à chaque mouvement (`:261`). Chaque déplacement (≈120 Hz) et chaque écho du PC paient deux sauts, un appel Rust `scale`, une `CATransaction` et une nouvelle image de couche.

Correctif : coalescer (un seul bloc en attente vers le fil principal, état lu au dernier moment) ; ne refaire `contents`/échelle/taille que si `change.shape`.

### IOS-20 — Instantané GPU synchrone à la fin · optimisation · S · plausible

`SessionController.swift:1589` : `engine?.snapshot(maxWidth: 1600)` mène à `waitUntilCompleted()` puis `getBytes` (`VideoRenderer.swift:1008-1009`) sur le fil principal à chaque « Se déconnecter ». Correctif : instantané et `saveLast` hors du fil principal.

### IOS-21 — Logique de session en double iOS ↔ tvOS ↔ Mac · factorisation · L · vérifié

≈ 311 lignes strictement identiques (blocs de 6 lignes ou plus), ≈ 600 avec les quasi-identiques, entre `apps/ios/KyberNowIOS/SessionController.swift` (1 645 l.) et `apps/tvos/KyberNowTV/SessionController.swift` (1 376 l.) : les trois `init` invité/regarder/emprunt (iOS 258-330 ↔ tvOS 222-300 ↔ Mac 549-625), `engineSettings` (331 ↔ 301), rappels du moteur (712-775 ↔ 618-678), `routeChanged` (817 ↔ 838), Share (1024-1070 ↔ 792-835), `toggleRecording` (1207 ↔ 890 ↔ Mac 2403), prêt, bandeaux PyroWave/Auto, démos (1456-1580 ↔ 1166-1317).

Les divergences relevées dans ce rapport en sont la conséquence directe : IOS-13 (mode), IOS-16/TVOS-11 (nettoyages différents), TVOS-14 (AV1), puce « Le réseau sature » de PyroWave sur tvOS seulement (tvOS `:354-358`, absente iOS `:380-382` et Mac), bandeau « autre réseau » annulable sur iOS (`:424-432`) mais pas sur tvOS (`:390-398`), `hdrState` calculé de deux façons (iOS `:484-490`, tvOS `:467-470`), `store.update(host)` appelé pour un visiteur sur iOS (`:1597`) mais pas tvOS (`:1335`), Mac `toggleRecording` sans `route:` (`apps/macos/KyberNow/SessionController.swift:2409`, à signaler à l'audit Mac).

Correctif : un fichier partagé (`apps/macos/KyberNow/SessionModel.swift`, ajouté aux trois `project.yml` comme `Share.swift`/`Lend.swift`) portant l'état et les réactions sans interface ; chaque app ne garde que la présentation. Les gardes de phase et le choix du codec de repli peuvent descendre dans `apps/apple`.

### IOS-22 — Hôte « visiteur » construit 9 fois, avec une divergence · factorisation · S · vérifié

`Host(name:address:consolePort:username:"")` + `apply(.jeu)` + `display`/`mouse`/`clipboard = false`/`keepLastImage = false` dans les trois `init` des trois apps. Pour l'emprunt, le Mac pose `host.settings.mouse = .auto` (`apps/macos/KyberNow/SessionController.swift:604`) ; iOS (`:312-316`) et tvOS (`:279-284`) non. Correctif : `Host.visitor(guest:)`, `.visitor(watch:)`, `.visitor(borrow:)` dans `Model.swift`, puis trancher la souris de l'emprunt.

### IOS-23 — `Theme`, `HostImagery`, `Platform` en double ou en triple · factorisation · M · vérifié

- `Theme.swift` iOS et tvOS : ≈ 250 lignes identiques (jetons de couleur strictement identiques, polices, néon, `Mark`, `Wordmark`, `SignalCorner`), ≈ 220 en commun avec le Mac (qui nomme `warnBg` ce que les deux autres appellent `warnFill`).
- `HostImagery` : iOS `Platform.swift:144-210` ↔ tvOS `Platform.swift:178-251` (≈ 66 lignes, ne diffèrent que le dossier et la largeur), plus la version Mac.
- `AudioOutput`/`AudioRoute` : iOS `Platform.swift:61-98` ↔ tvOS `:50-87`, avec divergences (voies réelles en Bluetooth sur iOS, forcées à 2 sur tvOS). `Preferences` : six clés communes.

Correctif : `ThemeTokens.swift` partagé (couleurs, polices, logo), `HostImagery` paramétré, base commune `Preferences` + extensions par plateforme.

### IOS-24 — `StatsFormat` refait `LinkFormat` · factorisation · S · vérifié

`SessionStats.swift:374-404` (`decimal`, `ms`, `mbps`, `percent`, `duration`, 19 appels) double `LinkFormat` (`apps/macos/KyberNow/LinkUI.swift:15-30`, compilé dans l'app iOS) et `PyroWaveText.percent` (`LinkUI.swift:1569`). Correctif : ajouter `ms(unit:)` et `mbps` à `LinkFormat`, supprimer `StatsFormat`.

### IOS-25 — `PlayStationSession` en triple · factorisation · M · vérifié

`startStats` (iOS 322 ↔ tvOS 219 ↔ Mac 464), `goToRest`, `toggleMute`, `close`/`stopPlay` : ≈ 55 lignes iOS ↔ tvOS, ≈ 65 iOS ↔ Mac. Divergence : tvOS filtre `setUpscale` par `Upscaling.available` (`:110, 132`), iOS (`:167`) et Mac non. Correctif : un cœur de partie partagé à côté de `PlayStationCore.swift`.

### IOS-26 — Code mort iOS · code mort · S · vérifié

Grep sur tout le dépôt hors `third_party` et `build` : `AppModel.isLocal(_:)` (`AppModel.swift:283`), `SessionController.toggleBar()` (`:1257`), `sessionMinutes` (`:1359`, seul tvOS s'en sert), `PadControl.isFace/isStick/isTrigger` (`ScreenPadLogic.swift:24-26`, absents aussi du test), `ChoiceButton` (`Theme.swift:415-435`), `ChamferEdge` (`Theme.swift:368`, n'est utilisé que sur tvOS), `@EnvironmentObject discovery` jamais lu dans `AddHostMethodLabel` (`AddHostView.swift:448`), `Theme.top` (`:49`).

### IOS-27 — Icônes et traductions inutilisées (iOS et tvOS) · code mort · S · vérifié

- Clés `stale` dans les deux catalogues : « Faire jouer », « Faire regarder », « Vous pouvez jouer. ». Absentes du code mais pas encore `stale` : iOS « Vos ordinateurs, sur cet iPad », « … sur cet iPhone », « Ajoutez un ordinateur équipé … ou par son QR code. » ; tvOS « Vos ordinateurs, sur ce téléviseur », « iCloud, réseau, code ou adresse », « Ajoutez un ordinateur équipé … ou par un code … ».
- Icônes sans aucun `Icon("…")` : iOS `battery-high/medium/low`, `device-mobile`, `hand-pointing`, `headphones`, `play`, `qr-code`, `vibrate` ; tvOS `battery-high/medium/low`, `copy`, `hand-pointing`, `headphones`, `lock-simple`, `qr-code` (et leurs variantes `-bold`).

Correctif : supprimer, et retirer de la liste du script d'icônes (TVOS-20).

### IOS-28 — Autorisation refusée : pas d'« Ouvrir les Réglages » en session · doc/cohérence · S · vérifié

`docs/link-ui.md:1055` demande l'action « Ouvrir les Réglages » là où le système le permet. iOS n'affiche que `show(notice: problem.notice)` (`SessionController.swift:702`) pour la caméra et `MicText.permission` (`:566, 785`) pour le micro ; `openSettingsURLString` n'existe que dans `AddHostScanner.swift:151`. Correctif : capsule avec action vers `UIApplication.openSettingsURLString`.

### IOS-29 — `tests/screen-pad` lancé nulle part · doc/tests · S · vérifié

Cité seulement par `apps/ios/README.md:115` (et `.en.md`) et des en-têtes de fichiers ; absent de `.github/workflows/ci.yml`, de `scripts/` et de `release.sh`. Les symboles qu'il appelle existent encore. Correctif : une étape `swiftc` sur le runner macOS de la CI.

### IOS-30 — AZERTY déduit du premier clavier activé · bug · S · plausible

`KeymapIOS.swift:58-60` : `UITextInputMode.activeInputModes.first`, avec `"fr"` seul accepté. iPad avec un clavier QWERTY branché mais le français en tête des claviers : noms de touches PlayStation faux ; repli `Locale.preferredLanguages` = `fr` au Québec → AZERTY à tort. Correctif : réglage explicite (comme « Clavier Apple ISO ») ou `textInputMode` du premier répondeur.

---

## Apple TV

Les fichiers `KyberKitTV/GamepadBridge.swift`, `KeyboardMouse.swift`, `MetalSurface.swift` et `ConsolePads.swift` sont aussi compilés dans l'app iOS : TVOS-01, 03 à 08 et 15, 16, 19 valent pour l'iPhone et l'iPad avec manette.

### TVOS-01 — Manette rallumée en session : Home renvoie à l'accueil de tvOS · bug · S · vérifié

`GamepadBridge.swift:575-578` :
```swift
setUpPad(slot)
applyHomeGesture()
if player > 0 {
    seat(slot, player)
```
À l'appel de `applyHomeGesture()`, `slot.active` (`player > 0`, `:202`) est faux : Home reste `.enabled` (`:883`). `seat()` (`:588-611`) ne réapplique rien, et `freed()` n'appelle `applyHomeGesture` que s'il installe une manette en attente.

Scénario : manette Xbox mise en veille puis rallumée en pleine partie, réglage « Bouton Guide » sur Jeu : Home quitte l'app au lieu d'aller au jeu. Vrai aussi en partie PlayStation.

Correctif : appeler `applyHomeGesture()` à la fin de `seat()` (ou après `seat` dans `connected`).

### TVOS-02 — Pendant la reconnexion, Retour ne fait rien · bug · S · vérifié

`SessionView.swift:18` : `streaming` inclut `.reconnecting`, donc `gameOnly` est vrai (`:35`) et `controllerUserInteractionEnabled = false` (`:187`) : Retour part à GameController, `onExitCommand` (`:149-158`) n'est jamais appelé. Côté manette, `menuRequested()` (`SessionController.swift:1040`) commence par `guard phase == .streaming`. L'écran n'affiche qu'un `MenuHint` « Reconnexion à … » (`:129-133`), sans bouton.

Scénario : PC éteint : ni Retour ni la combinaison ne permettent de sortir avant l'épuisement des tentatives.

Correctif : exclure `.reconnecting` de `gameOnly`, ou accepter `.reconnecting` dans `menuRequested` (le menu contient « Se déconnecter »).

### TVOS-03 — Fil principal en `queue.sync` sur la file des entrées · bug · M · plausible

`queue.sync` depuis le fil principal : `GamepadBridge.swift:165, 262` (`appLeft` à chaque `willResignActive`, donc à chaque Centre de contrôle sur iPhone), `328, 350, 381, 400, 471` ; `KeyboardMouse.swift:48, 77`. `StreamView.detach()` (`:171-176`) en enchaîne trois. La file ne revient jamais en synchrone sur le fil principal (pas d'interblocage Swift pur), mais elle exécute des appels FFI (`flushKeys`, `gamepadUnplug`, `ConsolePads.*`, création des moteurs, TVOS-04) : tout appel lent du cœur ou de kynput refige le fil principal, exactement la forme du gel 0.11.7 (8BADF00D).

Correctif : `setCaptured`, `setPointer`, `attach`, `detach` en `async` (file série : l'ordre est gardé) ; `appLeft` en `async` encadré par `beginBackgroundTask` ; un seul `detach` par fin de session.

### TVOS-04 — Moteurs Core Haptics créés sur la file des entrées · bug · S · vérifié

`connected()` (`:573`) : `slot.motors = Motors(controller: c, …)` → `createEngine(withLocality:)` (`:1514, 1554`) en synchrone sur `InputQueue.device` (iOS : `CHHapticEngine()` à `:992`). Le commentaire `:1448-1451` (« Core Haptics a sa propre file ») vaut pour les appels ultérieurs, pas pour la construction, qu'il estime lui-même à des dizaines de ms.

Scénario : deuxième manette allumée en jeu : touches et sticks de J1 et pointeur retenus ; si le menu s'ouvre au même moment (`setCaptured` → `sync`), le fil principal attend aussi.

Correctif : construire `Motors` sur `Motors.queue` puis affecter `slot.motors` par `queue.async`.

### TVOS-05 — Manette en souris : le pointeur file quand l'app perd la main · bug · S · plausible

`appLeft()` (`:522-536`) relâche les clics mais ne remet pas `slot.live` à zéro ni n'arrête `mouseTimer` ; `mouseTick()` (`:1188`) ne teste pas `inBackground` ; au retour, `resync()` sort tout de suite en mode souris (`:500`).

Scénario : profil Bureau, stick poussé, Centre de contrôle ouvert : la minuterie 120 Hz continue de pousser le pointeur (et la molette) sur le PC.

Correctif : sticks et gâchettes de `slot.live` à zéro dans `appLeft`, ou `!inBackground` dans `mouseTick` ; relire l'état réel dans `appBack`.

### TVOS-06 — Partie sur console : moteurs « hors session » pour une manette arrivée en cours · bug · S · vérifié

`:574` : `slot.motors?.setSession(input != nil)` ; en partie console `input` est nil. `attachConsole` (`:388`) et `addScreenPad` (`:993` : `input != nil || console`) font la différence. Effet : après chaque paquet (0, 0), `shutdown()` arrête le moteur ; chaque vibration suivante le redémarre (latence, à-coups).

Correctif : `setSession(input != nil || console)`.

### TVOS-07 — Vibrations : ni arrêt en arrière-plan, ni reprise après `stoppedHandler` · bug · S · plausible

`appLeft()` n'appelle pas `motors?.stop()` ; `rumble()` (`:1330`) et `consoleFeedback()` ne testent pas `inBackground`. Le `stoppedHandler` (`:1676-1684`) remet l'état à zéro sans relance, alors que `resetHandler` relance `set(level)` ; en mode motifs courts, la relance programmée (`:1848`) rappelle `engine.start()` même app suspendue.

Scénarios : bouton TV pendant un grondement continu (durée −1, 5 s par paquet) : la manette vibre sur l'accueil de tvOS ; Siri interrompt un grondement continu : plus rien tant que l'intensité ne change pas.

Correctif : `motors?.stop()` dans `appLeft`, garde `!inBackground` dans `rumble`/`consoleFeedback` ; dans `stoppedHandler`, `generation += 1` et relance de `set(level)` sauf raison `.applicationSuspended`/`.idleTimeout` (relance alors à `didBecomeActive`).

### TVOS-08 — Vitesse du pointeur manette calée sur l'ancienne définition · bug · S · vérifié

`GamepadBridge.hostSize` (`:235`) n'est écrit qu'à `attach` (`:330`) ; `StreamView.setHostSize` (`:162-169`) met à jour la couche de pointeur, pas le pont. `fullSpeed(hostSize)` (`:1199`) et `gain` (`:1303`) restent faux.

Scénario : profil Bureau, PC passé de 1080p à 4K : pointeur deux fois trop lent à la manette et à la Siri Remote.

Correctif : `GamepadBridge.shared.setHostSize(_:)` (en `queue.async`) appelé depuis `StreamView.setHostSize`.

### TVOS-09 — PlayStation : pont en mode console jusqu'à « Fermer » · bug · S · vérifié

`PlayStationSession.swift:198-208` : à `.ended`, `stopPlay()` mais `detachConsole()` seulement dans `close()` (`:302`). `applyHomeGesture` (`console && !captured`) laisse Home désactivé sur la carte de fin ; les touches partent encore vers `ConsolePads`.

Correctif : `GamepadBridge.shared.detachConsole()` dans `.ended`, après `stopPlay()`.

### TVOS-10 — Panneaux « Son » et « Taper du texte » : Retour ferme tout le menu · bug · S · plausible

`SessionMenu.swift:611-640` et `667-689` ont un `onExitCommand` mais ni `.focusSection()` ni focus posé à l'ouverture, contrairement aux panneaux qualité et écran (`:563, 604`). Le focus reste sur la tuile derrière le panneau : Retour remonte à `SessionView` et ferme tout ; les flèches atteignent les tuiles masquées.

Correctif : `.focusSection()` et focus initial, comme `qualityPanelView` ; test d'interface (TVOS-23).

### TVOS-11 — `fail()` ne nettoie pas comme `end()` · bug · S · vérifié

`SessionController.swift:1361-1375` ne retire pas `routeObserver` (retiré par `end()`, `:1346`) et ne remet pas `streamView.padMouse` à faux : `GamepadBridge.padMouse` reste vrai après un échec. Correctif : une fonction de nettoyage unique (iOS garde un seul tableau `observers`, vidé dans les deux cas).

### TVOS-12 — Minuteries non annulées · bug · S · vérifié

`closeMenu()` (`:1060-1062`) cache `menuHint` 4 s plus tard sans annulation ; `fallbackNote` (`:974`) est effacé 6 s après le premier message même si un second est arrivé. Fermer, rouvrir, refermer le menu en moins de 4 s : le rappel disparaît trop tôt. Correctif : `DispatchWorkItem` annulé à chaque affichage, comme `pyrowaveBannerClose`.

### TVOS-13 — Tuile « Caméra » non tenue à jour · bug · S · plausible

`:990` : `var cameraReady: Bool { !CameraCapture.cameras().isEmpty }` refait une `DiscoverySession` à chaque rendu et n'est pas observable ; `SessionMenu.swift:527-528` ignore l'appareil rendu par `.continuityDevicePicker` et `toggleCamera` prend `cameras().first?.id` (`:996`) : si la découverte ne le voit pas encore, avis « n'est plus branchée » avec un nom vide. Correctif : état `@Published` mis à jour par `AVCaptureDevice.wasConnected`/`wasDisconnected` ; après le choix, `AVCaptureDevice.systemPreferredCamera`.

### TVOS-14 — Repli AV1 forcé sans tester le décodeur · bug · S · plausible

tvOS `:304` : `if codec == .av1 { codec = … }` ; iOS `:334` : `if codec == .av1, !DecoderCapabilities.current.av1 { … }`. Une Apple TV qui décoderait l'AV1 en matériel ne l'aurait jamais. Correctif : condition d'iOS, dans le code partagé (IOS-21).

### TVOS-15 — Minuterie 120 Hz toujours armée en profil Bureau · optimisation · S · vérifié

`updateMouseTimer()` (`:1166-1177`) arme une minuterie à 120 Hz dès que `padMouse` et une chaîne d'entrées existent, sticks au repos ou menu ouvert ; `padMouse` est actif d'office en profil Bureau (`SessionController.swift:221`). Correctif : armer quand un stick sort de la zone morte, arrêter au retour au repos.

### TVOS-16 — Travail inutile à chaque appui · optimisation · S · vérifié

`button()` (`:750-754`) : `slot.live.pressed.insert(button.name)` avec `name` = `String(describing:)` (`:1412`, réflexion et allocation) même hors écran de test ; `combo.enabled = …` appelle `kn_combo_set_enabled` (FFI) à chaque appui. Correctif : ne remplir `live` que si `config.monitoring` ; noms statiques ; `combo.enabled` mis à jour seulement quand il change. (Les axes envoyés par paire même si un seul change, `:722-743`, sont un gain mineur : le cœur filtre déjà au 1/512.)

### TVOS-17 — Toute la vue de session recalculée chaque seconde · optimisation · M · plausible

`stats` et `audioStats` sont `@Published` sur `SessionController` (`:40-41`), observé par `SessionView` : chaque seconde, le corps entier et `updateUIViewController` de `SessionStage` repassent, menu fermé. Correctif : un petit `ObservableObject` de mesures, observé par `SessionMenu`/`StatsCard` seulement.

### TVOS-18 — Top Shelf réécrit à chaque changement du magasin · optimisation · S · vérifié

`AppModel.swift:166-169, 557-566` : écriture JSON atomique et `topShelfContentDidChange()` à chaque changement, y compris en session (signe de vie du prêt via `store.setLend`). L'image d'un ordinateur garde la même URL (`<id>-last.jpg`, `Platform.swift:207-209`, `ContentProvider.swift:16-17`) : tvOS peut garder l'ancienne en cache (plausible). Correctif : `TopShelfEntry: Equatable`, ne rien écrire si rien ne change ; empreinte ou date dans le nom de l'image.

### TVOS-19 — Motifs courts recréés toutes les 0,26 s · optimisation · S · vérifié

`:1832` : `let p = try pattern(intensity: level)` à chaque tranche, même à intensité constante (≈ 8 créations/s pour une DualSense à deux moteurs). Correctif : cache de `CHHapticPattern` par palier de 10 %.

### TVOS-20 — Script d'icônes en retard sur le catalogue · factorisation · S · vérifié

`apps/tvos/scripts/phosphor-icons.mjs:13-20` ne liste pas `arrow-up`, `check-circle`, `info`, `key`, `sparkle`, `video-camera`, `video-camera-slash`, présentes dans `Assets.xcassets/Icons` ; le script commence par `rmSync(out, { recursive: true, force: true })` (`:26`) : le relancer effacerait ces 14 jeux d'images. iOS n'a aucun script et son catalogue diverge (`microphone*`, `gear-six`). Correctif : un script unique paramétré par catalogue (Mac, tvOS, iOS), liste à jour, icônes mortes (IOS-27) retirées.

### TVOS-21 — Deux générateurs de QR locaux · factorisation · S · vérifié

`SettingsView.swift:766` et `AddHostView.swift:485-492` (propriété calculée, refaite à chaque `body`) alors que `Share.qrImage(for:)` existe (`apps/macos/KyberKit/ShareCore.swift:871`). Correctif : réutiliser `Share.qrImage`.

### TVOS-22 — Code mort tvOS et entrées partagées · code mort · S · vérifié

Grep sur tout le dépôt hors `third_party`/`build` : `SessionController.select(_:)` (`:1081`, jamais appelé ; s'il était rebranché il appliquerait Auto sans test), `@State sendCode` (`PlayStationAdd.swift:24`), `Theme.bar` (`:22`), `Theme.gutter` (`:49`), `Theme.top` (`:50`), `MetalSurface.onScreenHDR` (`KyberKitTV/MetalSurface.swift:48, 114`, jamais assigné), `StreamView.MouseMode` (`KyberKitTV/StreamView.swift:34`), `PadCombo.axes` (`apps/macos/KyberKit/InputCore.swift:100`) et `PadCombo.navigationKey` (`:141`) ; paramètres `input:` inutilisés de `longPress` (`:812`) et `dropLongPress` (`:853`).

### TVOS-23 — Aucun test d'interface du menu de session · doc/tests · S · vérifié

`UITests/RemoteNavigationTests.swift` ne couvre ni Retour en session, ni les panneaux, ni la carte d'échec, et attend par `sleep()`. Le mode démo `-screen menu` le permet ; un test « menu → Son → Retour ferme le panneau, pas le menu » aurait attrapé TVOS-02 et TVOS-10.

### TVOS-24 — Documentation en écart · doc · S · vérifié

- `docs/link-ui.md:1068` (et `.en.md`) décrit une capsule combinée « ● Caméra · Micro » et dit les textes « à fixer avec le travail du micro » ; la section « Micro » (`:199`) et l'app iOS (`SessionView.swift:537-544`) ont une icône de micro près de l'onglet, aucune app n'a la capsule combinée. Réécrire en renvoyant à « Micro ».
- `docs/micro.md:52` : « mode `voiceChat` tant que le micro est allumé » ; le code (`AudioPlayerCore.swift:449`) ne passe en `voiceChat` que si « Réduire l'écho et le bruit » est allumé.
- Station minimale : `apps/tvos/SIDELOAD.md:14` dit 0.2.2, `apps/ios/SIDELOAD.md:14` dit 0.3.1, pour le même cœur.
- `apps/tvos/SIDELOAD.md:69` (et `.en.md`) : « téléchargé que si `apps/tvos/Vendor` manque » ; `sideload.sh` télécharge si l'un des deux xcframeworks manque.

---

## Les 10 priorités

1. **IOS-01** — fuite de chaque session iOS (S) : la mémoire monte à chaque connexion, jusqu'à l'éviction.
2. **TVOS-01** — Home après une manette rallumée (S) : sortie de l'app en pleine partie, cas courant (manettes qui se mettent en veille).
3. **TVOS-02** — Retour inopérant pendant la reconnexion (S) : l'utilisateur est bloqué.
4. **IOS-04** — veille non empêchée (S, iOS et tvOS) : session qui tombe en spectateur ou à la manette.
5. **IOS-03** — pas de reprise après interruption audio (M) : plus de son après un appel.
6. **TVOS-03 + TVOS-04** — retirer les `queue.sync` du fil principal et la création des moteurs de la file des entrées (M) : ferme la classe de gel 8BADF00D côté Swift.
7. **IOS-05 + IOS-06** — modificateurs, glisser et Pencil collés (S) : Ctrl ou clic tenu sur le PC.
8. **IOS-02** — écran externe (S) : définition et HDR faux sur la TV.
9. **IOS-07 + IOS-08** — clavier et souris doublés sur iPad (S/M, à confirmer sur iPad avec Magic Keyboard).
10. **IOS-21** — sortir la logique de session dans un fichier partagé (L) : prévient le retour des divergences (IOS-13, TVOS-11, TVOS-14, PyroWave, HDR) ; à faire après les correctifs ci-dessus, en commençant par les `init` visiteurs (IOS-22).

## Ce qui est sain

- **Pas d'interblocage Swift pur** : la file des entrées ne revient jamais en synchrone sur le fil principal ; les appels Rust du chemin chaud (`send_reliably`) mettent en file au lieu d'attendre ; `kn_ps_stop` est déjà hors du fil principal.
- **Fermetures** : celles des contrôleurs de session sont en `[weak self]` ; les observateurs de `KeyboardMouse` sont retirés à chaque `attach` ; la minuterie HDR de `MetalSurface` s'arrête hors fenêtre ; la tâche de statistiques PlayStation est annulée.
- **Force-unwraps** : aucun atteignable (`ScreenPad` ne force que A/B/X/Y, `fatalError` seulement dans les `init(coder:)` indisponibles, un `as!` sur la classe de couche de l'aperçu caméra).
- **Acquis d'octobre** : `handlerQueue` des manettes, haptique continue, quatre manettes, Siri Remote avec courbe, combinaison 250 ms, `applyHomeGesture` en `.disabled` : en place, sans régression vue.
- **Sideload** : `project.sideload.yml` généré proprement depuis `project.yml` (identifiants, équipe, droits gratuits/complets, groupe du Top Shelf cohérent).
- **Docs** : README iOS et tvOS FR/EN alignés section par section ; fichiers et scripts cités présents ; `NSMicrophoneUsageDescription`/`NSCameraUsageDescription` présentes et traduites ; Apple TV sans micro conforme à la doc ; ordre « Micro » puis « Caméra » conforme à `link-ui.md`.
- **Vues** : peu de doublon littéral entre iOS et tvOS (0 à 13 lignes par paire, 44 pour `ShareViews`) : les interfaces sont vraiment propres à chaque plateforme ; le gain de factorisation est dans les modèles.
