# Corrections de l'audit du 27/11 : app Mac (`apps/macos`) et code Apple partagé (`apps/apple`)

Branche `claude/fix-mac-c1eede`, 29 commits sur `main` (`a76ec972`) : 14 pour la première passe, 15 pour la suite demandée par la session coordinatrice (« rien de côté »). Rapport d'audit : [mac.md](../mac.md).

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
| MAC-15, IOS-11 | Caméra : un numéro de demande, revérifié après l'ouverture ; éteinte entre-temps, la capture est refermée. |
| MAC-16, IOS-03 (reste) | iPhone, iPad, Apple TV : l'unité de sortie du son est relancée à la fin d'une interruption, à un changement de route et au redémarrage des services audio (latence relue). |
| MAC-20 (côté app) | Une seule règle Swift, `HostStore.sameFingerprint` (avec empreintes absentes), à la place des quatre `==` du Mac. |
| MAC-21 | Découverte : `stateUpdateHandler`, navigateur relancé 3 s après un échec. |
| MAC-22 | Souris relative (`onMove`, `active`), `vsync` et `latency` du rendu sous verrou ; horodatage de la caméra pris sur sa file. |
| MAC-23 | Passerelle : plus de `lock().unwrap()` (verrou empoisonné repris) ; route de relais illisible → route vide refusée comme erreur de relais, jamais une connexion directe (essai Rust). |
| MAC-24 | Rappel reposé : un numéro par rappel. PlayStation : « cliquez pour jouer » suit `kn_gamepads_count` (manettes du lecteur du cœur, −1 tant que SDL ne tourne pas). PyroWave : **tranché**, une image incomplète est sautée (commentaires, `docs/pyrowave.md` et l'essai le disaient ; l'accepter montrait des trous, la suivante est une image clé). |
| MAC-25 | Fin de session : dernière image attendue 250 ms au plus (sinon pas d'image, jamais de gel), lue en une copie ; JPEG et écritures des images sur une file à part ; Trousseau iCloud écrit sur sa propre file. |
| MAC-26 | Mesures du son lues chaque seconde par une minuterie du fil principal, plus sur le fil temps réel ; un seul saut. |
| MAC-27 | `attach(to:)` et `attachVideo` sans `renderQueue.sync` : la couche change à l'image suivante, le fil principal n'attend plus `nextDrawable`. |
| MAC-28 (en partie) | Annonces de mise à jour des Stations écrites dans UserDefaults seulement quand elles changent. |
| MAC-30 (en partie) | Combinaison de manette sans saut en double ; pointeur dessiné d'après l'état reçu, image posée au changement de forme ; tampon Annex B réservé, `CMBlockBuffer` rendu contigu au besoin. |
| MAC-36 (reste) | **Branché** : blanc SDR du calibrage HDR vers le rendu (couche `linear`, écran de la session ; plusieurs écrans : 203 nits), parce que le calibrage existe et que le shader l'attend. **Retirés** : `microphoneState` (jamais lu, l'interface suit `onMicrophone`), `received`/`hole`/`keyframeRequested` du protocole des mesures (les octets et les trous partent déjà des paquets au cœur), le contexte inutile des vibrations HD, la seconde attache identique de `HDRDemo`. |
| MAC-40 (reste) | `CFBundleShortVersionString`/`CFBundleVersion` pris de `MARKETING_VERSION`/`CURRENT_PROJECT_VERSION` (`project.yml`, Info.plist régénéré) ; `embed-kyber.sh` compare le contenu des bibliothèques (empreintes `.sources`) et recopie une bibliothèque changée même plus ancienne ; en-tête de `kn_ps_start` (rappel vidéo obligatoire, `userdata` jusqu'au retour de `kn_ps_stop`). |
| IOS-10 | Couper le micro remet `.playback` quand la catégorie d'avant était `.soloAmbient` ou `.ambient`. |
| IOS-12 | `CameraCapture` : rappels (`send`, `onProblem`…) sous verrou, horodatage capturé sur la file de la capture, encodeur absent dit une fois par ouverture. |
| IOS-15 | Presse-papiers iOS : plus lu sur le fil de kynput ; lu sur le fil principal et gardé pour le contenu en cours (la première demande après une copie ailleurs ne rend rien, la suivante le texte). |
| Webcam (bug de l'auteur) | `kn_camera_start` refusé : avis au lieu d'un retour muet à « éteinte ». « Caméra… » sans `started` ni `failed` de la Station en 10 s : caméra éteinte, avis. |
| PlayStation, fin de partie | `kn_ps_stop` était déjà lancé hors du fil principal sur Mac (`stopPlay`). `PlayStationPlay` retient maintenant son objet pour la partie (`passRetained`) et le rend après le retour de `kn_ps_stop` : plus d'objet libéré pendant qu'un fil de la partie le rappelle. La fenêtre sort du plein écran avant de se fermer. |
| TRANS-04 (Apple) | `build-apple-mobile.sh` (tvOS) s'arrête si le nightly utilisé n'est pas celui de `KYBER_NIGHTLY`. Pas de `+nightly-$KYBER_NIGHTLY` : la toolchain installée s'appelle `nightly` (commit du 2026-10-19), et `nightly-2026-10-19` serait un autre compilateur (celui du commit de la veille), absent de ce Mac ; l'épingler aurait cassé la construction tvOS. Le hachage de `release.sh:512` (`rustup run "nightly-$KYBER_NIGHTLY"`, vide ici) reste à corriger côté outillage. |
| TRANS-06 | `build-apple.sh` pose les correctifs client avant de compiler le cœur. |
| TRANS-18 (Mac) | `vendor-kyber.sh` refuse de réécrire `Vendor` quand l'app est ouverte (sauf dans un worktree avec `KYBERNOW_IGNORE_RUNNING=1`) et prend le verrou du Vendor. |

## Constats laissés

- **Vague 2 (factorisation et remontée dans le cœur)** :
  - MAC-20, partie cœur : la normalisation complète dans `same()` du cœur et `kn_fingerprint_same` ; les comparaisons `==` de l'app iOS (`AppModel.swift`, zone iOS) ;
  - MAC-31, hors zone Apple : la fonction commune dans `scripts/kyber-client-patches.sh` pour les cinq scripts non Apple, et le dossier renommé `apps/kyber-patches` (la liste Apple, `client-patches.sh`, est prête à y migrer) ;
  - MAC-32 à MAC-34 : fichiers partagés Mac/iOS/tvOS (`HostImagery`, marque, `CloudHost`), entrées PlayStation dans le cœur, codes d'erreur nommés ;
  - MAC-23, catch_unwind des ponts : une fois les gardes du cœur fusionnées (session fix-coeur).
- **Optimisations structurelles, à mesurer d'abord** : MAC-28 (un `ConsoleClient` gardé entre deux sondages, invalidations SwiftUI moins larges), MAC-29 (copies des OBU AV1 écrites directement dans le `CMBlockBuffer`), MAC-30 (tableaux de NAL par image). Ce sont des refontes du chemin d'image et du modèle, sans bug derrière : elles méritent un banc avant et après.
- **Hors zone** : `docs/plusieurs-ordinateurs.md` et `apps/core/README.md` citent encore les fonctions retirées (MAC-35) ; `release.sh` hache un nightly absent (TRANS-04) ; catalogues iOS et tvOS avec trois entrées périmées.
- **`BundleRename`** : migration d'avant la 0.4.3, gardée. La retirer est un choix de produit (une installation très ancienne mise à jour directement perdrait le renommage).

## Textes à ajouter à link-ui (caméra)

| Moment | FR | EN |
|---|---|---|
| Départ refusé par le cœur (`kn_camera_start` faux) | « La caméra n'a pas pu démarrer sur l'ordinateur. » | “The camera couldn't start on the computer.” |
| « Caméra… » sans réponse de la Station en 10 s | « L'ordinateur ne répond pas : la caméra est éteinte. » | “The computer isn't answering: the camera is off.” |

Les deux sont déjà dans le catalogue de KyberKit (FR et EN), donc sur Mac, iPhone, iPad et Apple TV.

## Ce qui est vérifié, et comment

- **App** : compilée (Debug, sans signature) avec `xcodebuild` dans un DerivedData du bloc-notes, contre le `Vendor` du worktree (copie). `libkybernow` y a été refaite par `build-apple.sh` avec le mécanisme TRANS-06, dans l'arbre `third_party/arbres-fix-mac/apple`. Ni l'app installée ni le `Vendor` de l'auteur n'ont été touchés. Elle n'a pas été lancée : l'app de l'auteur était ouverte.
- **Rust** : `cargo test -p kybernow-apple --features kyber-pyrowave`, 9 essais passés, dont 3 nouveaux : `userdata_rendu_au_dernier_detenteur` (MAC-01), `couche_liberee_ne_rappelle_plus` et `route_illisible_jamais_directe` (MAC-23).
- **iPhone, iPad, Apple TV** : vérification de types (`swiftc -typecheck`, SDK iphoneos et appletvos) de KyberKit avec les sources de chaque projet, et de l'app iOS entière (fichiers partagés du Mac compris) contre le module KyberKit émis : aucune erreur. Rien de construit dans Xcode pour ces plateformes.
- **Essais Swift** :
  - `tests/rules` (nouveau : garde de fin, modificateurs du mode immersif) ;
  - `tests/scripts` (nouveau : liste des correctifs et empreinte, dont un correctif kynput de la Station qui change l'empreinte) ;
  - `tests/autotest`, `tests/video` (décodage, couleurs, reprise, AV1) et `tests/pyrowave` (dont « tronquée sautée, suivante décodée ») : tous passent ;
  - `tests/fullscreen` et `tests/screen` : compilés seulement (`KYBERNOW_COMPILE_ONLY=1`), parce qu'ils ouvrent des fenêtres à l'écran ;
  - `check-strings.py` : catalogues complets.
- **Relecture à froid** du chemin de fin de session par un sous-agent. Ses quatre points sont corrigés (le 3 par MAC-25, le 2 pour PlayStation par la fin de partie).
- **Pas vérifié sur appareil** : rien de ce qui précède n'a été essayé en session réelle. À essayer sur le Mac : fin de session en plein écran (« Se déconnecter », fin venue de l'ordinateur, échec puis « Fermer »), en une fenêtre et sur plusieurs écrans ; fin d'une partie PlayStation en plein écran ; mode immersif au second plan ; PyroWave, caméra (refus, Station muette 10 s, éteinte pendant la demande d'accès) ; focus de l'accueil au lancement. Sur iPhone : appel pendant une session (le son revient), micro coupé puis bouton silencieux, coller sur le PC après une copie dans une autre app.

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
- Caméra : si l'ordinateur ne la prend pas, l'app vous le dit au lieu de l'éteindre sans rien dire.
- iPhone, iPad et Apple TV : le son revient après un appel, Siri ou une alarme, et quand vous changez d'écouteurs.
- iPhone et iPad : coller sur l'ordinateur ne fige plus l'image pendant l'alerte « Autoriser à coller ? ».

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
- Camera: if the computer doesn't take it, the app tells you instead of turning it off silently.
- iPhone, iPad and Apple TV: the sound comes back after a call, Siri or an alarm, and when you switch headphones.
- iPhone and iPad: pasting on the computer no longer freezes the picture during the “Allow Paste?” alert.
