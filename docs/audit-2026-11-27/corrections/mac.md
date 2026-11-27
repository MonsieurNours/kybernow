# Corrections de l'audit du 27/11 : app Mac (`apps/macos`) et code Apple partagé (`apps/apple`)

Branche `claude/fix-mac-c1eede`, 14 commits sur `main` (`a76ec972`). Rapport d'audit : [mac.md](../mac.md).

## Bugs signalés par l'auteur

**a) « Mon compte » prend le focus à l'ouverture.** Au premier affichage de l'accueil, AppKit donne le clavier à la première vue de la boucle (le bouton « Connecter un compte » de la barre de titre) : anneau visible, Espace ou Entrée l'activaient. `HomeFocus.clear` (`AppLife.swift`) remet la fenêtre en premier répondeur au premier affichage, et chaque fois que l'accueil revient d'une fenêtre cachée. Un champ en cours de frappe garde le focus. Tab mène toujours au premier bouton, comme le veut `docs/link-ui.md`.

**b) L'app figée « comme une simple image » en fin de session.** Pas de rapport de gel ni de plantage dans `~/Library/Logs/DiagnosticReports` pour ce cas. Le journal du 26/11 montre une fin normale (« session : Ending », crochet immersif retiré 0,8 s après, « Ended » 3,2 s après). La cause trouvée dans le code tient en deux défauts :
- **MAC-08, cause principale.** `finish()` retire la session du gestionnaire, puis `closeWindow()` sort du plein écran avec des fermetures en `[weak self]`. Plus rien ne retient alors le contrôleur, dès que le cœur l'a dit fini (`linger` ne le garde que tant que `coreEnded` est faux). Le cas se produit :
  - fin venue de l'ordinateur ;
  - échec affiché puis « Fermer » ;
  - fin d'Admin ;
  - bilan du cœur arrivé avant la fin de l'animation de sortie (3,2 s dans le journal, contre 3 s pour le délai de secours).

  Le contrôleur est alors libéré avant `windowDidExitFullScreen`. La fenêtre de session reste en plein écran, par-dessus tout, avec le passage refermé sur l'**accueil photographié** : une image, sans bouton. L'accueil ne revient jamais (`sessionWindowClosed` n'est pas appelé).

  Correctif : le contrôleur se retient lui-même jusqu'à `dropWindow`. La retenue est rompue en vidant `onExitFullScreen` et par le délai de 3 s.
- **Relecture à froid (sous-agent).** La fin ne dépendait que de la complétion de l'animation Core Animation du passage, qui peut ne jamais venir (fenêtre occultée, autre espace, écran en veille). Ajout d'un secours de 1,5 s (`finish()` ne passe qu'une fois). Les fenêtres des autres écrans (multi-écrans en espaces séparés) étaient retirées encore en plein écran : elles en sortent maintenant avant de se fermer.
- Avec eux : MAC-07 (un état du cœur arrivé après `end()` faisait revivre la session et pouvait relancer `scene.open()`) et MAC-06 (crochet immersif retiré au second plan).

## Constats corrigés

| ID | Correction |
|---|---|
| MAC-01 | `release_userdata` ajouté à la fin de `KnPlatform` et de `KnInputCallbacks`. Le dernier détenteur côté Rust rend l'objet Swift : la plateforme de la session et chaque flux que garde Kyber (`Owner` partagé par les `Sink`). Swift le rend sur le fil principal ; plus de délai de 30 s ni de 2 s. `kn_pointer_free` et `kn_combo_free` coupent leur rappel sous verrou avant de libérer. `setScreens` lâche les anciennes couches après `kn_input_set_screens`. |
| MAC-02 | PyroWave : `commands.commit()` toujours fait après un `decode_gpu_buffer` accepté, même sans passe d'entrelacement. |
| MAC-03, MAC-31 (côté Apple) | Une seule liste, `apps/macos/scripts/client-patches.sh`, sert à la pose et à l'empreinte de `Vendor/kyclient.stamp`, correctifs kynput/kynet/kycontroller/kywebplayer de la Station compris. Elle est utilisée par `build-kyclient.sh`, `build-apple.sh` et `build-apple-mobile.sh`. |
| MAC-04 | `CloudPublisher.publish` filtre lui-même les ordinateurs d'un compte (au lancement aussi). |
| MAC-05 | La couche SwiftUI reste affichée pour le bandeau PyroWave et pour la caméra qui part. |
| MAC-06 | Crochet clavier immersif retiré quand l'app passe au second plan. Appui ou relâchement d'un modificateur décidé d'après ses bits gauche/droite (`SessionRules.modifierPressed`). Un relâchement dont l'appui n'est pas passé par le crochet reste au système. |
| MAC-07 | `phaseChanged` ignore tout sauf `.ended` après `end()`. Le délai d'apparition exige `phase == .starting`. |
| MAC-08 | Voir plus haut. Secours de 1,5 s sur le passage. Fenêtres des autres écrans sorties du plein écran avant fermeture. |
| MAC-09 | Clic droit, autres boutons et molette ignorés dans la bande réservée à la barre ; les relâchements passent. |
| MAC-10 | `StreamView` relâche tout à la veille, à l'extinction des écrans et au changement d'utilisateur. |
| MAC-11 | L'avis « … veut emprunter l'ordinateur » prend le clic (`Hint.takesClick`). |
| MAC-12 | Le mode de la souris s'applique à tous les écrans. ⌃⌥⌘M décide d'après `mouseCaptured`. |
| MAC-13 | Un mot de passe changé est recopié dans le Trousseau iCloud (`passwordChanged`). Une réinvitation range le mot de passe avant de publier. |
| MAC-14 | Trousseau : `SecItemUpdate` puis `SecItemAdd` (`KeychainWrite.upsert`), jamais d'effacement d'abord, statut journalisé. Mots de passe gardés en mémoire après la première lecture (`SecretCache`). |
| MAC-17 | PlayStation : `releaseAll()` à `didResignActive`. Verr. Maj fait appui puis relâchement. Modificateurs lus d'après leurs bits. En jeu, ⌘W, ⌘Q… vont à la manette. |
| MAC-18 | Garde des `pts` remise à zéro à tout changement de codec. Erreur d'une ancienne génération sans effet. |
| MAC-19 | Pas d'écriture pour un ordinateur retiré pendant l'attente (`setStatus`, prêt, ordinateurs empruntés). Un seul tour de `refreshAll` à la fois. |
| MAC-24 (partiel) | Touche Menu d'un clavier de PC (kVK 0x6E → HID 0x65). |
| MAC-35 | 14 fonctions `kn_*` sans appelant retirées (`kn_session_wait` gardée), ainsi que leurs mentions dans `docs/coeur-apple` et le README du Mac. |
| MAC-36 | Swift sans appelant retiré : `StreamOptions`, `options`, `modePolicy`, `resized`, `InfoTip`, `radiusControl`/`radiusSheet`, `hasRelay`, `setLanguage`, `nowMicros`, `register`/`unregister`/`setVolume`/`hdrUsed`, `PadCombo.buttons`/`axes`/`navigationKey`, `occlusionObserver`, `PyroWaveDecoder.available`, `ChainTimes.all`/`sum`. |
| MAC-37 | 28 chaînes mortes retirées (21 de KyberKit, 7 de KyberNow). `check-strings.py` signale les entrées périmées ; avec `--strict`, elles font échouer la vérification. |
| MAC-38 | La liste de `phosphor-icons.mjs` garde `arrow-up`, `microphone(-slash)` et `video-camera(-slash)`. Neuf jeux sans usage retirés (`broadcast`, `camera`, `eye-slash`, `lock-simple`, `magnifying-glass`, `monitor-play`, `plugs-connected`, `qr-code`, `wifi-slash`). |
| MAC-39 | Une compilation ratée arrête l'essai (`compile()` au lieu de `\| grep error \|\| true`, dans les cinq `run.sh`). `fullscreen` et `screen` compilent de nouveau : port PyroWave, `HDRVideoLayer`, bouchon `tests/common/hdr-stub.swift`. `KYBERNOW_COMPILE_ONLY=1` permet de les compiler sans ouvrir de fenêtres. |
| MAC-40 (partiel) | Commentaire PyroWave de `build-apple-mobile.sh` ; `register` → `setScreenViews` dans `SessionEngine`. |
| TRANS-06 | `build-apple.sh` pose les correctifs client avant de compiler le cœur. |
| TRANS-18 (Mac) | `vendor-kyber.sh` refuse de réécrire `Vendor` quand l'app est ouverte (sauf dans un worktree avec `KYBERNOW_IGNORE_RUNNING=1`) et prend le verrou du Vendor. |

## Constats laissés

- **MAC-15** (caméra arrêtée pendant son ouverture), **MAC-16** (son iOS/tvOS après une interruption), **MAC-20** (règle d'empreinte, demande un changement du cœur), **MAC-21** (Bonjour jamais relancé), **MAC-22** (courses mineures), **MAC-23** (`unwrap()` sur verrous et route illisible dans la passerelle), reste de **MAC-24** (`showHint`, `padConnected`, image PyroWave incomplète : c'est un choix à trancher), **MAC-25** à **MAC-30** (optimisations), **MAC-32** à **MAC-34** (factorisations) : hors des priorités demandées, pour la vague 2.
- **MAC-31, partie hors zone** : la fonction commune devrait vivre dans `scripts/kyber-client-patches.sh` (zone outillage), pour les 5 scripts non Apple, et le dossier devrait être renommé `apps/kyber-patches`. La liste Apple (`client-patches.sh`) est prête à y migrer.
- **MAC-36, reste** : `microphoneState` (écrit, jamais lu), le setter `VideoRenderer.sdrWhite`, `inputProcRefCon` de `DualSenseHaptics`, les méthodes jamais appelées du protocole `LatencyReporting`, le double `kCVImageBufferYCbCrMatrixKey` de `HDRDemo`. Ils touchent l'HDR ou les mesures et demandent une décision (brancher ou retirer).
- **MAC-40, reste** : `Info.plist` en 1.0, `rsync --update` d'`embed-kyber.sh`, `docs/plusieurs-ordinateurs.md` et `apps/core/README.md`, qui citent encore les fonctions retirées (hors zone).
- **`BundleRename`** : migration d'avant la 0.4.3, gardée. La retirer est un choix de produit (une installation très ancienne mise à jour directement perdrait le renommage).
- **PlayStation, fin de partie** : `PlayStationPlay` n'a toujours pas de `deinit`, et `kn_ps_stop` attend les fils de la partie (`join`) ; sur le fil principal, cela peut figer l'app un instant. La fenêtre PlayStation est aussi retirée sans sortir du plein écran (`PlayStationSession.close`). Non corrigé, à reprendre.
- **Relecture à froid, point 3** : `end()` fait encore la capture, l'encodage JPEG et la publication iCloud sur le fil principal (MAC-25). Le Trousseau est désormais lu en mémoire, mais `CloudKeychain.set` reste synchrone pour un ordinateur pas encore copié.
- **Hors zone, vu en passant** : les catalogues iOS et tvOS ont encore « Faire jouer », « Faire regarder » et « Vous pouvez jouer. » périmés (signalés par `check-strings.py`).

## Ce qui est vérifié, et comment

- **App** : compilée (Debug, sans signature) avec `xcodebuild` dans un DerivedData du bloc-notes, contre le `Vendor` du worktree (copie). `libkybernow` y a été refaite par `build-apple.sh` avec le mécanisme TRANS-06, dans l'arbre `third_party/arbres-fix-mac/apple`. Ni l'app installée ni le `Vendor` de l'auteur n'ont été touchés. Elle n'a pas été lancée : l'app de l'auteur était ouverte.
- **Rust** : `cargo test -p kybernow-apple --features kyber-pyrowave`, 8 essais passés, dont 2 nouveaux : `userdata_rendu_au_dernier_detenteur` (MAC-01, rendu une fois, après la fermeture du dernier flux) et `couche_liberee_ne_rappelle_plus`.
- **Essais Swift** :
  - `tests/rules` (nouveau : garde de fin, modificateurs du mode immersif) ;
  - `tests/scripts` (nouveau : liste des correctifs et empreinte, dont un correctif kynput de la Station qui change l'empreinte) ;
  - `tests/autotest`, `tests/video` (décodage, couleurs, reprise, AV1) et `tests/pyrowave` : tous passent ;
  - `tests/fullscreen` et `tests/screen` : compilés seulement (`KYBERNOW_COMPILE_ONLY=1`), parce qu'ils ouvrent des fenêtres à l'écran.
- **Relecture à froid** du chemin de fin de session par un sous-agent. Ses points 1, 2 (autres écrans) et 4 sont corrigés ; le point 2 pour PlayStation et le point 3 sont listés plus haut.
- **Pas vérifié sur appareil** : rien de ce qui précède n'a été essayé en session réelle.

## Notes de version

**FR**
- La fin d'une session ne laisse plus l'app figée sur une image : la fenêtre se ferme toujours, même en plein écran, et l'accueil revient.
- À l'ouverture, plus aucun bouton de l'accueil n'est sélectionné d'office ; Tab vous y mène toujours.
- PyroWave : une session ne peut plus se figer pour de bon après une erreur de la carte graphique.
- Le bandeau PyroWave et le voyant de la caméra s'affichent de nouveau pendant la session.
- Mode immersif : il ne touche plus au clavier quand l'app est au second plan, et la touche ⊞ ne reste plus enfoncée sur l'ordinateur.
- Plus de clic droit ni de défilement involontaire sous la barre ; rien ne reste enfoncé quand votre Mac se met en veille ou se verrouille.
- Consoles PlayStation au clavier : plus de touche tenue après ⌘Espace, Verr. Maj fonctionne, et ⌘W ne ferme plus le jeu.
- Plusieurs écrans : le mode de la souris s'applique à tous vos écrans.
- iCloud : les ordinateurs de votre compte ne sont plus envoyés à l'Apple TV et à l'iPhone, et un mot de passe changé y arrive tout de suite.
- Un plantage rare à la déconnexion est corrigé.

**EN**
- Ending a session no longer leaves the app frozen on a picture: the window always closes, even in full screen, and the home screen comes back.
- When the app opens, no home screen button is selected anymore; Tab still takes you there.
- PyroWave: a session can no longer freeze for good after a graphics card error.
- The PyroWave banner and the camera light show again during the session.
- Immersive mode: it no longer touches the keyboard while the app is in the background, and the ⊞ key no longer stays pressed on the computer.
- No more accidental right-click or scrolling under the bar; nothing stays pressed when your Mac goes to sleep or locks.
- PlayStation consoles with the keyboard: no key stays held after ⌘Space, Caps Lock works, and ⌘W no longer closes the game.
- Multiple displays: the mouse mode applies to all your displays.
- iCloud: your account's computers are no longer sent to Apple TV and iPhone, and a changed password reaches them right away.
- A rare crash on disconnection is fixed.
