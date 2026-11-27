# Vague 1 : corrections iPhone/iPad et Apple TV

Zone : `apps/ios` et `apps/tvos` (KyberNowIOS, KyberKitIOS, KyberNowTV, KyberKitTV, scripts, README et SIDELOAD). Rapport d'audit : [`../ios-tvos.md`](../ios-tvos.md). Branche `claude/fix-ios-tvos-c4d4bf`.

## 1. La webcam de l'iPhone (bug signalé le 27/11)

**Ce qui a été lu.** Les rapports de l'iPhone 18 Pro Max (lecture seule, `devicectl … systemCrashLogs`) : six rapports KyberNow du 26/11 22:16 au 27/11 00:22, tous en 0.11.6, tous le même gel (`SIGKILL`, fil principal dans `kn_pointer_move_by` qui attend le verrou de `kynput::InputRouter`), c'est-à-dire l'interblocage pointeur/kynput corrigé en 0.11.7. Aucun fil de ces rapports n'est dans la caméra (`AVCapture`, `VTCompression`, `CameraCapture`, uplink). `log collect` sur l'appareil demande root : pas de journal système lu.

**Cause : pas tranchée sans un essai.** Le chemin iOS a été relu de bout en bout (bouton → `SessionController.toggleCamera` → `SessionEngine.cameraOn` → `CameraCapture.open` → `kn_camera_start` → `CameraLink::start` → START vers la Station → sondage `kn_camera_poll`). Ce qui est écarté :

- **Info.plist** : `NSCameraUsageDescription` est présente (projet normal ; le sideload part du même `project.yml`). Sans elle, l'app se serait arrêtée à la demande d'autorisation, et aucun rapport de ce type n'existe.
- **Bouton affiché mais inactif** : le bouton n'est montré que si l'ordinateur propose la caméra (`cameraOffered`) ; l'offre est donc arrivée. `toggleCamera` ne fait rien d'autre que `cameraOn`/`cameraOff`.
- **Format et orientation** : la capture tourne l'image (720 × 1280 en portrait alors que 1280 × 720 est annoncé), mais l'outil de la Station (`server/camera/camera.c`) redimensionne et borde toute image décodée : sans conséquence.
- **Fil de l'`AVCaptureSession`** : configurée et démarrée sur sa file à elle, pas sur le fil principal.

Ce qui reste possible, du plus au moins probable :

1. **L'app se figeait** : le soir du 26/11, c'était la 0.11.6, qui gelait au toucher (six rapports en deux heures). La caméra a pu ne jamais être vraiment essayée. **À refaire en 0.11.7.**
2. **Échec muet côté app** (`apps/macos/KyberKit/SessionEngine.swift:873-876`, hors de ma zone) : si `kn_camera_start` rend faux (le cœur n'a pas de flux montant pour la caméra), l'état repasse à `.off` **sans aucun avis**. C'est exactement ce que produit **COEUR-04** : après un redémarrage de la Station en dernier recours, l'offre vient du nouveau `Worker` mais `Session::camera()` vise l'ancien, sans flux montant ; le bouton reste là, le toucher ne fait rien. COEUR-04 est corrigé par la session du cœur ; il faudrait en plus un avis dans `SessionEngine` (session Mac).
3. **« Caméra… » sans fin** : si l'outil `kybernow-camera` de la Station ne répond jamais (ni `started`, ni `failed`), rien n'arrête l'attente. En 0.11.6 et 0.11.7, cet outil Windows est un programme console (TRANS-01) ; les défauts STWIN-19 et 21 (Frame Server) peuvent aussi le bloquer. Côté app, il manque un délai d'attente ; son texte n'existe pas dans `docs/link-ui.md` (à décider en vague 2, puis dans `SessionEngine`).
4. **Autorisation refusée** : la même autorisation sert au QR code de l'ajout d'un ordinateur. Refusée alors, la caméra en session ne montrait qu'un avis de 4 s, sans « Ouvrir les Réglages ». **Corrigé ici** (IOS-28) : avis de 6 s avec le bouton.

**Ce qui est ajouté pour trancher la prochaine fois** : chaque étape de la caméra va au journal de l'app (`fr.leslignesclaires.kybernow`, catégorie `app`, messages « caméra : … » : offre, états `starting`/`sending`/`paused`/`off`, avis, bouton sans offre).

**Essai à faire par l'auteur** (KyberNow Link 0.11.8 ou cette branche sur l'iPhone, Station Windows à jour) :

1. Sur l'iPhone : Réglages › KyberNow › Caméra doit être allumé (sinon, l'allumer).
2. Sur le Mac : ouvrir Console.app, choisir l'iPhone dans la barre latérale, « Démarrer », filtre `caméra` (et `processus : KyberNow`).
3. Se connecter au PC, ouvrir la barre, « Caméra » › « Allumer la caméra ».
4. Noter ce que dit le journal et ce que montre la barre :
   - « caméra : proposée » puis `starting` puis `sending` et rien côté PC → regarder la Station (« KyberNow Camera » dans l'app Caméra de Windows, journal de `kycontroller`) ;
   - `starting` puis `off` aussitôt, sans avis → cas 2 (flux montant absent dans le cœur) ;
   - `starting` qui ne change plus → cas 3 (l'outil de la Station ne répond pas) ;
   - un avis « … refusé … » → cas 4.

## 2. Constats corrigés

| Id | Correction |
|---|---|
| IOS-01 | La rangée du clavier ne garde plus la session en mémoire : contrôleur tenu par `KeyboardRow`, accessoire du clavier lâché à la fin (`tearDown`) et à la disparition de la vue. |
| IOS-02 | Écran externe branché en session : l'abonnement lit la nouvelle scène (`.receive(on: .main)`), donc mode et HDR de la TV. |
| IOS-03 | (en partie) À la fin d'une interruption (appel, Siri, alarme) : session audio réactivée, micro allumé relancé (iOS) ; session audio réactivée (tvOS). La relance de l'unité de sortie reste à faire dans `AudioPlayerCore` (session Mac). |
| IOS-04 | La veille est retenue pendant toute session d'ordinateur (préparation à reconnexion) et toute partie PlayStation, sur iOS et tvOS (`SleepGuard`, `SessionRules.keepsAwake`). |
| IOS-05 | Clavier de l'écran replié : Ctrl, Alt, ⊞ collants relâchés ; après `releaseAll`/`detach`, la rangée ne les montre plus tenus. |
| IOS-06 | Fin d'un glisser et Pencil levé relâchés même barre ouverte ou en relatif ; glisser relâché quand la barre s'ouvre ; `pencilMouseDown` remis à zéro. |
| IOS-07 | Clavier physique et clavier de l'écran : les appuis d'un clavier physique (GameController) ne passent plus aussi par `insertText`. |
| IOS-08 | iPad, souris en Bureau : ni déplacement ni molette en double (GameController ignoré là où le pointeur d'iPadOS sert ; l'iPhone garde GameController). |
| IOS-13 | Changement de définition par le PC : `streamMode` et `modeLabel` suivent (statistiques et dernière session justes). |
| IOS-14 | `PlayStationKeyMouse` : file attitrée pour le clavier et la souris, `suspended`/`consoleKeyboard` lus sous verrou. |
| IOS-16 | Perte du premier plan pendant la reconnexion ou avant l'image : tout est relâché, débit plafonné, image figée. |
| IOS-17 | `connect`/`watch` refusés pendant une partie PlayStation ; `importCloud` sans effet en démonstration. |
| IOS-28 | Caméra ou micro refusés en session : l'avis porte « Ouvrir les Réglages ». |
| TVOS-01 | Manette rallumée en partie : Home part au jeu (réappliqué à chaque place donnée). |
| TVOS-02 | Pendant « Reconnexion à … » : Retour revient à l'app et ouvre le menu (« Se déconnecter »), la combinaison de la manette aussi. |
| TVOS-03 | Plus aucun `queue.sync` du fil principal vers la file des entrées (pont des manettes, clavier et souris) ; l'arrière-plan passe par une tâche d'arrière-plan. |
| TVOS-04 | Moteurs Core Haptics créés sur la file des vibrations, posés ensuite sur la manette si elle est toujours là. |
| TVOS-05 | Manette en souris : le stick ne pousse plus le pointeur quand l'app perd la main. |
| TVOS-06 | Partie sur console : moteurs « en session » pour une manette arrivée en cours de partie. |
| TVOS-07 | (en partie) Vibrations arrêtées en arrière-plan, plus jouées tant que l'app n'a pas la main. La relance après `stoppedHandler` n'est pas faite. |
| TVOS-08 | Vitesse du pointeur à la manette recalculée quand l'ordinateur change de définition. |
| TVOS-09 | PlayStation : la carte de fin rend Home et les touches à l'app. |
| TVOS-10 | Panneaux « Son » et « Taper du texte » : focus gardé dans le panneau, Retour ne ferme que lui. |
| TVOS-11 | `fail()` nettoie comme `end()` (observateur du son, manette en souris). |
| TVOS-12 | Rappel du menu et avis de repli : minuteries annulées à chaque nouvel affichage. |
| TVOS-14 | Repli AV1 seulement si le décodeur ne le sait pas (comme iOS). |
| TVOS-16 | À chaque appui : plus d'appel au cœur pour la combinaison si rien ne change, plus de nom de touche calculé hors de l'écran de test. |
| TVOS-24 | (en partie) `SIDELOAD.md` tvOS : le SDK est téléchargé si l'un des deux xcframeworks manque. |
| TRANS-37 | Accueil de l'Apple TV : la raison du HDR n'apparaît que puce focalisée ; le bandeau de mise à jour de la Station reprend sa place. |
| TRANS-18 (tvOS) | `build-kyber-tvos.sh` ne retire plus que `include/` et `KyberSDK.xcframework` : le cœur reste dans `Vendor/`. README FR et EN mis à jour. |

## 3. Constats laissés

| Id | Raison |
|---|---|
| IOS-09 | Verrouillage du pointeur : effort M, demande un contrôleur hôte ; à faire avec un iPad et une souris. |
| IOS-10, IOS-11, IOS-12, IOS-15 | Le code est dans `apps/macos/KyberKit` (`AudioPlayerCore`, `SessionEngine`, `CameraCapture`, `InputCore`), tenu par la session Mac. |
| IOS-18, IOS-19, IOS-20 | Optimisations (animations sous la session, sauts de file du pointeur, instantané GPU de fin) : pas des bugs ; IOS-20 touche `VideoRenderer` (Mac). |
| IOS-21, IOS-22 | Vague 2 (fichier de session partagé), comme demandé. |
| IOS-23 à IOS-27, TVOS-20 à 22 | Factorisation et code mort : vague 2. |
| IOS-29 | L'étape de CI (`.github/workflows/ci.yml`) est un fichier commun ; `tests/screen-pad` et le nouveau `tests/session-rules` sont à y ajouter à la fusion. |
| IOS-30 | Demande un réglage de disposition du clavier, donc une règle dans `docs/link-ui.md` d'abord. |
| TVOS-07 (reste) | Relance après `stoppedHandler` : à faire avec une manette et Siri pour voir les raisons d'arrêt réelles. |
| TVOS-13 | Tuile « Caméra » et Caméra de continuité : à reprendre avec un iPhone choisi depuis l'Apple TV. |
| TVOS-15, 17, 18, 19 | Optimisations, sans effet visible. |
| TVOS-23 | Tests d'interface du menu : à écrire au simulateur tvOS, pas fait ici. |
| TVOS-24 (reste) | `docs/link-ui.md` et `docs/micro.md` sont des fichiers communs ; la version minimale de la Station (0.2.2 sur tvOS, 0.3.1 sur iOS) n'a pas pu être tranchée. |

## 4. Vérifications

- **Constructions** : cœur et SDK reconstruits dans un arbre à part (`scripts/arbres.sh preparer apple`, `KYBERNOW_ARBRES=…/arbres-fix-ios-tvos`) : `build-apple.sh` (iOS appareil et simulateur), `build-apple-mobile.sh ios`, `build-kyber-tvos.sh`, `build-apple-mobile.sh tvos`. Puis `xcodegen` et `xcodebuild` Debug pour le **simulateur iOS** et le **simulateur tvOS** : les deux **réussis**, sans nouvel avertissement dans les fichiers touchés.
- **Essais sans simulateur** : `apps/ios/tests/session-rules/run.sh` (nouveau : veille retenue par étape, arrière-plan, menu pendant la reconnexion, plusieurs sessions qui retiennent la veille) **passe** ; `apps/ios/tests/screen-pad/run.sh` **passe** (3 095 vérifications).
- **XCTest (interface, simulateur Apple TV 4K, tvOS 27.2)** : nouveau `testSoundPanelBackClosesOnlyThePanel` (menu de session, panneau « Son », Retour ne ferme que lui, focus rendu à la tuile) **passe** ; `testAddressFormSurvivesKeyboard`, `testAppSettingsKeepFocusInTheList`, `testBottomSectionsReachTheirPanel` **passent**. `testSettingsReachableFromHome` et `testInvitationCodeAddsTheComputer` **échouent, et échouent de la même façon sur le code d'origine** (a76ec972, relancés dans l'arbre) : ils comptent les appuis à droite d'une rangée d'accueil qui a changé depuis (puces « Auto », HDR) ; à remettre à jour à part.
- **Simulateur iOS** : l'app en démonstration (`-demo YES -screen bar`) ouvre la session et sa barre sans plantage.
- **Pas vérifié** (pas d'appareil) : tout ce qui demande un vrai iPhone, iPad ou Apple TV face à une Station (voir ci-dessous).

**À essayer sur appareil** : graphe mémoire d'Xcode après trois sessions (plus aucun `SessionController`) ; iPhone posé sans toucher pendant 5 min en session (pas de verrouillage) ; appel reçu en session puis raccroché (son et micro ; le son du jeu peut encore manquer tant que `AudioPlayerCore` n'est pas corrigé) ; iPad + Magic Keyboard avec la rangée du clavier ouverte (une frappe = une lettre) et souris en Bureau (curseur et molette) ; Ctrl collant puis clavier replié ; glisser au maintien puis ouverture de la barre ; Apple TV : manette éteinte puis rallumée en partie avec « Bouton Guide » sur Jeu, PC débranché pendant la session (Retour pendant « Reconnexion… »), bouton TV pendant une vibration continue ; la webcam (section 1).

## 5. Notes de version

**FR**
- iPhone et iPad : la mémoire ne grimpe plus à chaque session ; l'app restait chargée de toutes les sessions précédentes.
- iPhone, iPad et Apple TV : l'appareil ne se met plus en veille pendant une session ou une partie, même sans toucher l'écran.
- iPhone et iPad : après un appel ou Siri, le son de la session et le micro reprennent.
- iPhone et iPad : Ctrl, Alt et ⊞ ne restent plus enfoncés sur l'ordinateur quand vous repliez le clavier, et un glisser ou un trait du Pencil ne reste plus « collé ».
- iPad : avec un clavier physique, chaque touche ne part plus deux fois ; avec une souris, le curseur ne saute plus et la molette ne défile plus deux fois.
- iPhone et iPad : un écran externe branché en cours de session prend sa définition et son HDR.
- Caméra ou micro refusés : l'avis propose « Ouvrir les Réglages ».
- Apple TV : une manette rallumée en pleine partie envoie bien le bouton Home au jeu.
- Apple TV : pendant « Reconnexion… », le bouton Retour ouvre le menu, d'où vous pouvez vous déconnecter.
- Apple TV : les panneaux « Son » et « Taper du texte » se ferment seuls avec Retour, sans fermer tout le menu.
- Apple TV : le bandeau de mise à jour de la Station s'affiche de nouveau à l'accueil sur un téléviseur HDR.
- Apple TV : les vibrations s'arrêtent quand vous quittez l'app, et le pointeur de la manette suit la définition de l'ordinateur.

**EN**
- iPhone and iPad: memory no longer climbs with each session; the app kept every previous session loaded.
- iPhone, iPad and Apple TV: your device no longer goes to sleep during a session or a game, even when you don't touch the screen.
- iPhone and iPad: after a phone call or Siri, the session's sound and your microphone come back.
- iPhone and iPad: Ctrl, Alt and ⊞ no longer stay pressed on the computer when you fold the keyboard away, and a drag or a Pencil stroke no longer gets “stuck”.
- iPad: with a physical keyboard, each key is no longer sent twice; with a mouse, the cursor no longer jumps and the scroll wheel no longer scrolls twice.
- iPhone and iPad: an external display plugged in during a session gets its own resolution and HDR.
- Camera or microphone denied: the notice now offers “Open Settings”.
- Apple TV: a controller turned back on mid-game sends the Home button to the game again.
- Apple TV: during “Reconnecting…”, the Back button opens the menu, where you can disconnect.
- Apple TV: the “Sound” and “Type text” panels close on their own with Back, without closing the whole menu.
- Apple TV: the Station update banner shows again on the home screen with an HDR TV.
- Apple TV: rumble stops when you leave the app, and the controller pointer follows the computer's resolution.
