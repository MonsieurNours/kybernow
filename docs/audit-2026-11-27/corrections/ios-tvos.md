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
| IOS-09 | Verrouillage du pointeur d'iPadOS : la vue de session est hébergée par un contrôleur (`PointerLockController`) dont `prefersPointerLocked` suit la souris des jeux (relatif) et les parties sur console, barre fermée et image sur l'appareil ; redemandé à chaque bascule, état réel au journal (« pointeur de l'iPad verrouillé/libre »). Pas d'iPad pour l'essayer : iPadOS lit la préférence depuis le contrôleur racine de SwiftUI, qui doit la transmettre à son enfant (à vérifier). |
| IOS-13 | Changement de définition par le PC : `streamMode` et `modeLabel` suivent (statistiques et dernière session justes). |
| IOS-14 | `PlayStationKeyMouse` : file attitrée pour le clavier et la souris, `suspended`/`consoleKeyboard` lus sous verrou. |
| IOS-16 | Perte du premier plan pendant la reconnexion ou avant l'image : tout est relâché, débit plafonné, image figée. |
| IOS-18 | (en partie) Le passage (deux fonds animés sans fin) quitte l'arbre 1,7 s après l'ouverture, comme sur l'Apple TV. |
| IOS-19 | Pointeur (iOS et tvOS) : un seul saut vers le fil principal en attente, quel que soit le nombre de mouvements ; l'image de la couche refaite seulement quand la forme change. |
| IOS-17 | `connect`/`watch` refusés pendant une partie PlayStation ; `importCloud` sans effet en démonstration. |
| IOS-28 | Caméra ou micro refusés en session : l'avis porte « Ouvrir les Réglages ». |
| TVOS-01 | Manette rallumée en partie : Home part au jeu (réappliqué à chaque place donnée). |
| TVOS-02 | Pendant « Reconnexion à … » : Retour revient à l'app et ouvre le menu (« Se déconnecter »), la combinaison de la manette aussi. |
| TVOS-03 | Plus aucun `queue.sync` du fil principal vers la file des entrées (pont des manettes, clavier et souris) ; l'arrière-plan passe par une tâche d'arrière-plan. |
| TVOS-04 | Moteurs Core Haptics créés sur la file des vibrations, posés ensuite sur la manette si elle est toujours là. |
| TVOS-05 | Manette en souris : le stick ne pousse plus le pointeur quand l'app perd la main. |
| TVOS-06 | Partie sur console : moteurs « en session » pour une manette arrivée en cours de partie. |
| TVOS-07 | Vibrations arrêtées en arrière-plan, plus jouées tant que l'app n'a pas la main ; après un arrêt par le système (`stoppedHandler`), plus de relance des motifs courts sur le moteur arrêté, reprise tout de suite après une panne (`.systemError`), au retour de l'app sinon (`Motors.resume`). |
| TVOS-08 | Vitesse du pointeur à la manette recalculée quand l'ordinateur change de définition. |
| TVOS-09 | PlayStation : la carte de fin rend Home et les touches à l'app. |
| TVOS-10 | Panneaux « Son » et « Taper du texte » : focus gardé dans le panneau, Retour ne ferme que lui. |
| TVOS-11 | `fail()` nettoie comme `end()` (observateur du son, manette en souris). |
| TVOS-12 | Rappel du menu et avis de repli : minuteries annulées à chaque nouvel affichage. |
| TVOS-13 | Tuile « Caméra » : `cameraReady` publié et tenu à jour par les branchements de caméra ; l'appareil choisi dans le choix de tvOS est celui qu'on allume (puis la caméra préférée du système, puis la première). |
| TVOS-15 | Manette en souris : la minuterie à 120 Hz n'est armée que quand un stick sort de sa zone morte, et s'arrête au repos. |
| TVOS-17 | Statistiques de chaque seconde dans `SessionMeter`, observé par le menu seul : menu fermé, la vue de session n'est plus recalculée chaque seconde (`audioStats`, lu par aucune vue, n'est plus publié). |
| TVOS-18 | Top Shelf : rien n'est écrit ni rechargé si les cartes n'ont pas changé ; la date de l'image fait partie de la carte (une nouvelle image republie). Le nom du fichier reste le même (cache de tvOS par adresse : plausible, pas vérifié). |
| TVOS-19 | Motifs courts des vibrations : un motif par palier de 10 %, gardé, au lieu d'un neuf toutes les 0,26 s. |
| TVOS-23 | Tests d'interface du menu de session : panneaux « Son » et « Taper du texte » (Retour ne ferme que le panneau, focus rendu à la tuile) ; les deux tests de l'accueil et de l'invitation, périmés, cherchent maintenant leur bouton au lieu de compter les appuis. |
| TVOS-14 | Repli AV1 seulement si le décodeur ne le sait pas (comme iOS). |
| TVOS-16 | À chaque appui : plus d'appel au cœur pour la combinaison si rien ne change, plus de nom de touche calculé hors de l'écran de test. |
| TVOS-24 | (en partie) `SIDELOAD.md` tvOS : le SDK est téléchargé si l'un des deux xcframeworks manque. |
| TRANS-37 | Accueil de l'Apple TV : la raison du HDR n'apparaît que puce focalisée ; le bandeau de mise à jour de la Station reprend sa place. |
| TRANS-04 (tvOS) | `build-kyber-tvos.sh` compile avec le nightly épinglé (`nightly-$KYBER_NIGHTLY` s'il est installé, sinon `nightly` seulement s'il est de cette date ; un autre arrête le script avec la commande d'installation). README FR et EN : `rustup toolchain install nightly-2026-10-19 …`. `build-apple-mobile.sh` (apps/apple) n'est pas dans ma zone. |
| TRANS-18 (tvOS) | `build-kyber-tvos.sh` ne retire plus que `include/` et `KyberSDK.xcframework` : le cœur reste dans `Vendor/`. README FR et EN mis à jour. |

## 3. Constats laissés

| Id | Raison |
|---|---|
| IOS-10, IOS-11, IOS-12, IOS-15, reste d'IOS-03, avis et délai de la caméra | Le code est dans `apps/macos/KyberKit` (`AudioPlayerCore`, `SessionEngine`, `CameraCapture`, `InputCore`), confié à la session Mac. |
| IOS-18 (reste) | L'accueil reste rendu sous la session (ses animations aussi) : le retirer de l'arbre perdrait son état et relancerait ses `onAppear` à chaque fin de session ; à faire avec le portage des fonds en Core Animation (comme le Mac). |
| IOS-20 | L'instantané passe par `SessionEngine.snapshot` et `VideoRenderer` (Mac) : le sortir du fil principal demande de savoir s'ils le permettent pendant `stop()`. |
| IOS-21, IOS-22 | Vague 2 (fichier de session partagé), comme demandé. |
| IOS-23 à IOS-27, TVOS-20 à 22 | Factorisation et code mort : vague 2. |
| IOS-29 | L'étape de CI (`.github/workflows/ci.yml`) est un fichier commun ; `tests/screen-pad` et le nouveau `tests/session-rules` sont à y ajouter à la fusion. |
| IOS-30 | Demande d'abord une règle dans `docs/link-ui.md` (proposée en section 3 bis). |
| TVOS-24 (reste) | `docs/link-ui.md` et `docs/micro.md` sont des fichiers communs (texte proposé en section 3 bis) ; la version minimale de la Station dans les SIDELOAD aussi. |

## 3 bis. Règles et textes proposés pour `docs/link-ui.md`

**IOS-30 — Disposition du clavier physique (iPad, parties sur console).** Dans « Réglages de l'app › Clavier » (iPad, et iPhone avec un clavier branché), un choix « Disposition du clavier » : **Auto** (par défaut), **AZERTY**, **QWERTY**, **QWERTZ** / *“Keyboard layout”: Auto, AZERTY, QWERTY, QWERTZ*. Aide : « Pour les touches d'une partie sur console (ZQSD ou WASD). Auto suit le clavier physique branché quand iPadOS le dit, sinon la langue du premier clavier. » / *“For a console game's keys (ZQSD or WASD). Auto follows the plugged-in keyboard when iPadOS reports it, otherwise the first keyboard's language.”* Auto lit `GCKeyboard.coalesced` puis le `textInputMode` du premier répondeur, plus `UITextInputMode.activeInputModes.first` ; `fr-CA` reste QWERTY. Même réglage que « Clavier Apple ISO », rangé à côté.

**TVOS-24 — Capsule de la caméra et du micro (`link-ui.md`, « Caméra de l'appareil »).** Remplacer la capsule combinée « ● Caméra · Micro » par : « Barre fermée, la caméra envoyée se signale par la capsule « ● Caméra » / *“● Camera”* en haut à gauche ; le micro allumé, par son icône près de l'onglet (voir « Micro »). Les deux à la fois : les deux signes, chacun à sa place. » — et dans `docs/micro.md` (« mode `voiceChat` ») : « la session audio passe en mode `voiceChat` seulement si « Réduire l'écho et le bruit » est allumé ; sinon `default` » / *“the audio session switches to `voiceChat` mode only when “Reduce echo and noise” is on; otherwise `default`.”* SIDELOAD : une seule version minimale de la Station pour les deux apps, qui ont le même cœur ; proposé : « 0.3.1 ou plus récente » (le 0.2.2 de tvOS date de l'appairage par code). À confirmer par l'auteur : rien dans le code ne fixe cette limite.

## 4. Vérifications

- **Constructions** : cœur et SDK reconstruits dans un arbre à part (`scripts/arbres.sh preparer apple`, `KYBERNOW_ARBRES=…/arbres-fix-ios-tvos`) : `build-apple.sh` (iOS appareil et simulateur), `build-apple-mobile.sh ios`, `build-kyber-tvos.sh`, `build-apple-mobile.sh tvos`. Puis `xcodegen` et `xcodebuild` Debug pour le **simulateur iOS** et le **simulateur tvOS** : les deux **réussis**, sans nouvel avertissement dans les fichiers touchés.
- **Essais sans simulateur** : `apps/ios/tests/session-rules/run.sh` (nouveau : veille retenue par étape, arrière-plan, menu pendant la reconnexion, plusieurs sessions qui retiennent la veille) **passe** ; `apps/ios/tests/screen-pad/run.sh` **passe** (3 095 vérifications).
- **XCTest (interface, simulateur Apple TV 4K, tvOS 27.2)**, après la seconde série : `testSoundPanelBackClosesOnlyThePanel` et `testTypingPanelBackClosesOnlyThePanel` (nouveaux), `testSettingsReachableFromHome` (remis à jour : il cherche « Réglages de … » au lieu de compter quatre appuis), `testAddressFormSurvivesKeyboard`, `testAppSettingsKeepFocusInTheList`, `testBottomSectionsReachTheirPanel` **passent**. `testInvitationCodeAddsTheComputer` **échoue, comme sur le code d'origine** (a76ec972, relancé dans l'arbre) : `typeText` n'arrive pas dans le champ du mot de passe au simulateur, « Ajouter l'ordinateur » reste inactif et le focus tombe sur « Annuler » ; un défaut du test (ou du simulateur), pas de l'app.
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
- iPad : avec une souris ou un trackpad, le pointeur reste dans le jeu (souris des jeux, parties sur console) au lieu d'ouvrir le Dock ou le Centre de contrôle.
- Apple TV : la caméra de l'iPhone choisi dans le menu est bien celle qui s'allume.

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
- iPad: with a mouse or trackpad, the pointer stays in the game (game mouse, console games) instead of opening the Dock or Control Center.
- Apple TV: the iPhone camera you pick in the menu is the one that turns on.
