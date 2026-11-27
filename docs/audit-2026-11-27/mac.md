# Audit du 2026-11-27 : app Mac (`apps/macos`) et code Apple partagé (`apps/apple`)

État relu : `main` à `1fb289c6` (0.11.7). Lecture de code seulement : rien n'a été compilé ni lancé, aucun appareil. Zone couverte : `KyberNow`, `KyberKit` (aussi compilé pour iOS et tvOS par chemin, voir `apps/ios/project.yml` et `apps/tvos/project.yml`), notre code PyroWave autour de l'upstream, scripts, tests, `kyber-patches` du Mac, `project.yml`, et la passerelle Rust `apps/apple/src`.

Cinq relectures ciblées ont été menées en parallèle : session et fenêtres, image et son, entrées et PlayStation, interface et modèle, passerelle Rust et projet. Chaque constat gardé ici a ensuite été relu dans le code. **Vérifié** veut dire que toutes les lignes en jeu ont été lues et que le défaut est certain. **Plausible** veut dire que le chemin de code existe mais que l'effet dépend du moment ou du matériel. Un symbole n'est dit mort qu'après une recherche dans tout le dépôt hors `third_party`, iOS et tvOS compris.

Du précédent audit (`docs/archives/audit-2026-10-20.md`), sont réglés : décodage asynchrone, garde d'une image en vol (désactivée par défaut), jeux de paramètres indexés par (type, id), 4:4:4 en AV1 coupé par le cœur, sonde des capacités hors du fil principal, `userdata` de session retenu (`passRetained`), relâchement au changement d'app. **Reste ouvert** : les triples copies AV1 (MAC-29).

## Synthèse

| ID | Catégorie | Constat | Fichier | Effort | Confiance |
|---|---|---|---|---|---|
| MAC-01 | bug critique | Rust peut rappeler un objet Swift déjà libéré (minuteries de 2 s et 30 s, `passUnretained`) | `KyberKit/SessionEngine.swift:1352`, `InputCore.swift:112, 252, 547`, `PlayStationCore.swift:262` | M–L | plausible |
| MAC-02 | bug critique | PyroWave : tampon de commandes jamais validé, la session se fige pour de bon | `KyberKit/PyroWaveDecoder.swift:279` | S | vérifié |
| MAC-03 | bug critique | L'empreinte du Vendor du Mac ignore les correctifs Station posés avant (kynput, kynet…) | `scripts/build-kyclient.sh:57` | S | vérifié |
| MAC-04 | bug | Les ordinateurs d'un compte partent dans iCloud à chaque lancement | `KyberNow/KyberNowApp.swift:116` | S | vérifié |
| MAC-05 | bug | Bandeau PyroWave et voyant de la caméra invisibles : la couche SwiftUI reste cachée | `KyberNow/SessionController.swift:774` | S | vérifié |
| MAC-06 | bug | Mode immersif : crochet clavier posé même app au second plan, parité appui/relâchement fragile, ⊞ collé | `KyberNow/Immersive.swift:40, 74, 124-139` | S | plausible |
| MAC-07 | bug | Une session finie peut revivre (`phaseChanged` sans garde après `end()`) | `KyberNow/SessionController.swift:1766-1813` | S | plausible |
| MAC-08 | bug | Contrôleur libéré avant la sortie du plein écran : fenêtre orpheline, accueil jamais revenu | `KyberNow/SessionController.swift:2289, 2327` | S | plausible |
| MAC-09 | bug | Clic droit, boutons latéraux et molette passent dans la bande réservée de la barre | `KyberKit/StreamView.swift:537, 556, 564` | S | vérifié |
| MAC-10 | bug | Rien n'est relâché à la veille, au verrouillage ni au changement d'utilisateur | `KyberNow`, `KyberKit` | S | plausible |
| MAC-11 | bug | L'avis « … veut emprunter l'ordinateur » ne prend pas le clic | `KyberNow/SessionChrome.swift:23` | S | vérifié |
| MAC-12 | bug | Multi-écrans : le mode de souris ne s'applique qu'à l'écran principal | `KyberNow/SessionController.swift:2259, 2268` | S | vérifié |
| MAC-13 | bug | Un mot de passe changé n'atteint jamais iCloud (Apple TV, iPhone) | `KyberNow/CloudPublisher.swift:151`, `Model.swift:900` | S | vérifié |
| MAC-14 | bug | Trousseau : effacé avant d'écrire, statut ignoré ; lu sur le fil principal à chaque sondage | `KyberNow/Model.swift:1153`, `Accounts.swift:128` | S | vérifié |
| MAC-15 | bug | Caméra : un arrêt pendant l'ouverture la laisse allumée | `KyberKit/SessionEngine.swift:870-891` | S | plausible |
| MAC-16 | bug | iOS/tvOS (KyberKit partagé) : le son ne reprend pas après une interruption | `KyberKit/AudioPlayerCore.swift:222` | M | vérifié |
| MAC-17 | bug | PlayStation au clavier : pas de relâchement à `didResignActive`, Verr. Maj en bascule, ⌘W ferme le jeu | `KyberNow/PlayStationSession.swift:483`, `KyberKit/PlayStationInput.swift:108-153` | S | vérifié / plausible |
| MAC-18 | bug | Décodeur : garde de `pts` remise à zéro au seul passage en PyroWave ; erreurs d'une ancienne génération qui forcent l'attente d'image clé | `KyberKit/VideoDecoder.swift:100, 228, 262` | S | plausible |
| MAC-19 | bug | Sondage : résultats écrits pour un ordinateur retiré ; `refreshAll` lancé plusieurs fois à la fois | `KyberNow/Model.swift:943, 983`, `MenuBar.swift:352` | S | vérifié |
| MAC-20 | bug | « Même empreinte ? » : trois règles différentes (Swift, cœur, comparaisons directes) | `KyberNow/Model.swift:856`, `apps/core/src/account.rs:835` | M | vérifié (écart) |
| MAC-21 | bug | Découverte : `NWBrowser` sans `stateUpdateHandler`, jamais relancé après un échec | `KyberNow/Discovery.swift:49` | S | plausible |
| MAC-22 | bug | Courses mineures entre fils : `RelativeMouse`, `vsync`/`latency` du rendu, `firstPTS` de la caméra | `KyberKit/RelativeMouse.swift:17-70`, `VideoRenderer.swift:131-134`, `CameraCapture.swift:217, 388` | S | vérifié |
| MAC-23 | bug | Passerelle : `unwrap()` sur des verrous dans des `extern "C"` (arrêt brutal) ; route de relais illisible prise pour une connexion directe | `apps/apple/src/lib.rs:116`, `input.rs:604`, `relay.rs:27`, `account.rs:261` | S | vérifié |
| MAC-24 | bug | Petits écarts : rappel identique coupé trop tôt, touche Menu du PC absente, `padConnected` PlayStation différent du cœur, image PyroWave incomplète affichée malgré les commentaires | voir section | S | vérifié / plausible |
| MAC-25 | optimisation | Fin de session : capture GPU attendue, JPEG encodé et écrit sur le fil principal | `KyberNow/SessionController.swift:2108`, `VideoRenderer.swift:1009`, `HostImagery.swift:67` | S | vérifié |
| MAC-26 | optimisation | Fil temps réel du son : `DispatchQueue.main.async` (allocation) une fois par seconde, et deux sauts | `KyberKit/AudioPlayerCore.swift:189` | S | vérifié |
| MAC-27 | optimisation | Fil principal bloqué jusqu'à 1 s par `renderQueue.sync` pendant que le rendu attend une surface | `KyberKit/VideoRenderer.swift:439, 450, 646` | M | plausible |
| MAC-28 | optimisation | Sondage : un `ConsoleClient` neuf par ordinateur toutes les 8 s ; relais global de `objectWillChange` ; UserDefaults écrits à chaque état | `KyberNow/Model.swift:954`, `KyberNowApp.swift:685`, `Notifications.swift:106` | M | plausible |
| MAC-29 | optimisation | AV1 : chaque OBU copiée trois fois par image (déjà signalé, toujours là) | `KyberKit/AV1.swift:43-50`, `VideoDecoder.swift:134, 525` | M | vérifié |
| MAC-30 | optimisation | Allocations par image ou par geste : tableaux de NAL, `Data` de la caméra, couche du pointeur | `VideoDecoder.swift:154, 566`, `CameraCapture.swift:374`, `StreamView.swift:234` | S | vérifié |
| MAC-31 | factorisation | La liste des correctifs clients est recopiée dans 8 scripts, sous un dossier au nom trompeur | `apps/macos/kyber-patches`, `scripts/build-kyclient.sh:100` et 7 autres | M | vérifié |
| MAC-32 | factorisation | `HostImagery` ×3, `MarkGeometry`/`Mark` ×3, `CloudHost` ×2 entre Mac, iOS et tvOS | `KyberNow/HostImagery.swift`, `Theme.swift:356`, `CloudPublisher.swift:26` | M | vérifié |
| MAC-33 | factorisation | Clavier et souris PlayStation recopiés Mac/iPad ; `PadKey` ×2 ; `Notifier.sentence` = `ShareText.sentence` | `PlayStationInput.swift`, `apps/ios/KyberKitIOS/PlayStationKeyMouse.swift`, `InputCore.swift:463`, `Notifications.swift:173` | M | vérifié |
| MAC-34 | factorisation | Codes d'erreur recopiés à la main entre cœur, passerelle et Swift | `apps/apple/src/invite.rs:22`, `lib.rs:195`, `ConsoleClientCore.swift:449` | S | vérifié |
| MAC-35 | code mort | 15 fonctions `kn_*` exportées que nulle app n'appelle | `apps/apple/src/*.rs` | S | vérifié |
| MAC-36 | code mort | Types, fonctions et propriétés Swift sans appelant | voir section | S | vérifié |
| MAC-37 | code mort | Une vingtaine de chaînes de traduction mortes ; `check-strings.py` saute les entrées périmées | `KyberKit/Localizable.xcstrings`, `KyberNow/Localizable.xcstrings`, `scripts/check-strings.py:47` | S | vérifié |
| MAC-38 | code mort | Icônes : le générateur supprimerait 5 icônes utilisées ; 16 jeux d'icônes livrés sans usage | `scripts/phosphor-icons.mjs:12`, `Assets.xcassets/Icons` | S | vérifié |
| MAC-39 | doc | Tests `fullscreen` et `screen` qui ne compilent plus, erreurs masquées par `grep error \|\| true` | `tests/fullscreen/run.sh:16`, `tests/screen/run.sh:10`, `tests/video/run.sh:61` | M | vérifié |
| MAC-40 | doc | Commentaires et docs périmés : `build-apple-mobile.sh`, `Info.plist` en 1.0, `register` dans `SessionEngine`, fonctions mortes listées dans `docs/coeur-apple.md` | voir section | S | vérifié |

Total : 3 bugs critiques, 21 bugs, 6 optimisations, 4 factorisations, 4 constats de code mort, 2 de doc.

---

## Bugs critiques

### MAC-01 — Rust peut rappeler un objet Swift déjà libéré
**Catégorie** bug critique · **Effort** M–L · **Confiance** plausible

**Où.**
- Session : `KyberKit/SessionEngine.swift:1352-1355`, `asyncAfter(.now() + 30) { kn_session_free(s); …release() }`. Les `Sink` vidéo et son (`apps/apple/src/lib.rs:554-606`) gardent le `userdata` brut, et leur `Drop` rappelle Swift avec `KN_PACKET_CLOSED`. `kn_session_wait` existe (`lib.rs:1412`), mais Swift ne l'appelle jamais.
- Entrées : `InputCore.swift:547-553` libère la boîte des rappels 2 s après `kn_input_free`. Or `kn_input_free` (`input.rs:423-429`) ne rend qu'une référence, et Kyber ainsi que le fil des minuteries gardent le pipeline.
- Couche du pointeur : `InputCore.swift:252-266`, `passUnretained(self)` puis `kn_pointer_free` dans `deinit`. `kn_pointer_free` libère la boîte sans retirer `on_changed` (`input.rs:100-115`). Or la couche Rust est aussi clonée dans chaque pipeline (`input.rs:416, 486`). De plus, `setScreens` (`InputCore.swift:585`) remplace `screenPointers` avant que `kn_input_set_screens` ne détourne les paquets du curseur.
- Combinaison de manette : `InputCore.swift:112-120`, `passUnretained(self)` et `deinit { kn_combo_free(core) }`, alors que le fil de la combinaison (`input.rs:803-818`) peut être dans `on_formed`.
- PlayStation : `PlayStationCore.swift:262`, `passUnretained(self)`. `PlayStationPlay` n'a pas de `deinit` qui arrête le jeu.

**Scénario.** Un écran secondaire se ferme pendant qu'un paquet de curseur arrive sur le fil de kynput : `takeUnretainedValue()` porte alors sur un `PointerLayer` libéré (SIGSEGV). Autre cas : Kyber retient un `Sink` plus de 30 s après la fin (Wi-Fi tombé, fil réseau bloqué), et son `Drop` appelle un `SessionEngine` rendu.

**Correctif.** Ajouter un rappel `release_userdata` à `KnPlatform`, `KnInputCallbacks`, `KnPointerCallbacks`, `KnComboCallbacks` et `KnPsCallbacks`, appelé par le `Drop` du dernier détenteur côté Rust. Swift passe alors en `passRetained` et les minuteries disparaissent. `AutoTestCore.swift:210-237` (`on_done`) montre déjà le bon modèle. En attendant : `kn_pointer_free` remet `on_changed` à vide avant de libérer, et `setScreens` appelle le cœur avant de lâcher l'ancien tableau.

### MAC-02 — PyroWave : tampon de commandes jamais validé, session figée
**Catégorie** bug critique · **Effort** S · **Confiance** vérifié (déclencheur rare)

**Où.** `KyberKit/PyroWaveDecoder.swift:279-283` :
```swift
let decoded = pyrowave_decoder_decode_gpu_buffer(decoder, …commands…, &outputs)
guard decoded == PYROWAVE_SUCCESS, let pass = commands.makeComputeCommandEncoder() else { note(…); return }
```

**Scénario.** Quand `decode_gpu_buffer` réussit, l'upstream note `commands` comme consommateur de l'emplacement d'envoi (`upstream/pyrowave_decoder.mm:404`, `slot->consumer = cmd`). Si `makeComputeCommandEncoder()` rend nil (appareil perdu, mémoire), le tampon n'est jamais validé. Quatre images plus tard, `acquire_upload_slot` attend avec `waitUntilCompleted` (`:55`) un tampon qui ne finira jamais. Cela se passe sur le fil réseau de Kyber, sous `VideoDecoder.lock` : image, son et entrées s'arrêtent pour de bon.

**Correctif.** Séparer les deux conditions : après un `decode_gpu_buffer` réussi, toujours appeler `commands.commit()`, même sans passe d'entrelacement.

### MAC-03 — L'empreinte du Vendor ignore les correctifs Station posés avant
**Catégorie** bug critique (publication) · **Effort** S · **Confiance** vérifié

**Où.** `apps/macos/scripts/build-kyclient.sh:57-59` :
```sh
stamp() { { cat "$HERE"/kyber-patches/*.patch "$ROOT"/server/kyber-patches/*-kyclient-*.patch "$HERE/scripts/build-kyclient.sh" … ; } | shasum …
```
**Scénario.** Depuis l'unification, `scripts/kyber-client-patches.sh:43-50` (`kyber_station_patches`) pose d'abord `server/kyber-patches/*-kynput-*`, `*-kynet-*`, `*-kycontroller-*` et `*-kywebplayer-*`, par exemple le stylet 0027, le canal de retour 0050 ou l'enregistrement 0070. Aucun de ces fichiers n'entre dans l'empreinte. Modifier l'un d'eux laisse `Vendor/kyclient.stamp` inchangé : `release.sh` ne recompile pas et une libkynput périmée part dans la version. C'est le même accident que celui évité de justesse en 0.7.0.

**Correctif.** Faire entrer `kyber_station_patches` dans `stamp()`. Mieux : une seule fonction de liste partagée avec les sept autres scripts (MAC-31).

---

## Bugs

### MAC-04 — Les ordinateurs d'un compte partent dans iCloud à chaque lancement
**Effort** S · **Confiance** vérifié

**Où.** `KyberNow/KyberNowApp.swift:116`, `cloud.publish(store.hosts)`. Comparer avec `Model.swift:787`, `didSave?(hosts.filter { $0.account == nil })`, dont le commentaire dit : « Les ordinateurs d'un compte restent sur cet appareil ».

**Scénario.** Au lancement, la liste entière est publiée. Tant que rien ne change ensuite, `save()` ne repasse pas et les ordinateurs du compte restent dans iCloud, avec une adresse et un nom d'utilisateur vides. L'Apple TV et l'iPhone les proposent dans « ordinateurs d'iCloud à ajouter » (`tvos/KyberNowTV/AddHostView.swift:171`, `ios/KyberNowIOS/AddHostPanes.swift:224`), voire les ajoutent tout seuls quand la synchro iCloud est allumée.

**Correctif.** Filtrer dans `CloudPublisher.publish` lui-même, pour que les deux appels suivent la même règle.

### MAC-05 — Bandeau PyroWave et voyant de la caméra invisibles
**Effort** S · **Confiance** vérifié

**Où.** `KyberNow/SessionController.swift:774` :
```swift
let needed = !streaming || barVisible || padMode || hint != nil || statsPanel || screenOfferShown
```
Le bandeau (`SessionChrome.swift:59`, `session.pyrowaveBanner`) et le voyant de la caméra (`SessionChrome.swift:83`, `cameraState == .sending`) ne sont dessinés que dans cette couche. `updateCompositing` est antérieur à ces deux ajouts et n'a pas suivi.

**Scénario.** En image, barre fermée, sans rappel en cours : « PyroWave en Wi-Fi » ou « Le réseau sature » est levé, mais la couche reste cachée, donc ni « Passer en HEVC » ni la croix ne sont visibles. De même, une caméra qui part n'a aucun voyant, alors que `docs/link-ui.md` en fait un indicateur de confidentialité.

**Correctif.** `|| pyrowaveBanner || cameraState == .sending`, puis appeler `updateCompositing()` quand ces deux états changent.

### MAC-06 — Mode immersif : crochet posé au second plan, parité fragile
**Effort** S · **Confiance** plausible (pose du crochet vérifiée)

**Où.**
- `KyberNow/Immersive.swift:74-79` : `let want = anySessionWants() && Self.trusted`, sans condition sur `NSApp.isActive`. Les observateurs (`:40-41`) ne suivent pas `didResignActive`.
- `:124` : un `flagsChanged` sans entrée dans `held` est pris pour un appui. `:139` : `if type != .keyUp { held[code] = … }` enregistre aussi les relâchements de modificateurs.

**Scénario.**
- Tant qu'une session tourne, chaque frappe du Mac, dans toutes les apps, passe par le fil principal de KyberNow (`.cgSessionEventTap`, sur `CFRunLoopGetMain`). Un fil principal occupé (MAC-25, Trousseau) retarde le clavier de tout le système.
- On tient ⌘ ailleurs, on clique dans la fenêtre plein écran et on relâche ⌘ : le relâchement est noté comme un appui. Le ⌘ suivant, même dans une autre app, est avalé et envoyé au PC (Win enfoncé), et son relâchement passe au système : Win reste collé côté PC.

**Correctif.** Retirer le crochet à `didResignActive` (ou exiger `NSApp.isActive` dans `refresh()`). Pour `flagsChanged`, décider appui ou relâchement d'après le bit du modificateur dans `cg.flags`. Vider `held` au passage au second plan.

### MAC-07 — Une session finie peut revivre
**Effort** S · **Confiance** plausible

**Où.** `KyberNow/SessionController.swift:1766-1813`. `.preparing`, `.connecting`, `.starting` et `.streaming` réécrivent `phase` sans regarder si elle vaut déjà `.ending` ou `.ended`. Seul le cas `.ending` a une garde. `end()` (`:2099`) appelle `engine?.stop()` sans débrancher `onPhase`, et le cœur arrête par message (`core/src/session.rs`, `Msg::Stop`).

**Scénario.** « Se déconnecter » pendant la connexion ou une reconnexion : un état déjà en file arrive après `end()`, `phase` repasse à `.connecting` et la capsule réaffiche les étapes. Si `finish()` est passé, `.streaming` relance même `scene.open()` après le délai de révélation, et la fin peut être attribuée à tort à l'ordinateur (`endedByComputer`, `:1838`).

**Correctif.** En tête de `phaseChanged`, si `phase` vaut `.ending` ou `.ended`, ne traiter que `.ended` (bilan, `coreEnded`, `onEnded`) puis sortir.

### MAC-08 — Contrôleur libéré avant la sortie du plein écran
**Effort** S · **Confiance** plausible

**Où.** `finish()` (`SessionController.swift:2289-2297`) appelle `onFinish` (retrait de `SessionManager.all`), puis `closeWindow()` (`:2327`), dont les fermetures sont toutes en `[weak self]`. `linger` (`Sessions.swift:174`) ne retient la session que si `coreStarted && !coreEnded`.

**Scénario.** Session en plein écran, puis échec affiché et « Fermer » (`coreEnded` déjà vrai), ou fin par Admin, ou « Annuler » pendant un emprunt (`engine == nil`). Le contrôleur disparaît avant `windowDidExitFullScreen`, donc `dropWindow` ne passe jamais : la fenêtre reste orpheline et l'accueil caché.

**Correctif.** Garder le contrôleur dans le gestionnaire jusqu'à `onWindowClosed`, ou capturer `self` fortement dans le repli de 3 s de `closeWindow`.

### MAC-09 — Clic droit, boutons latéraux et molette dans la bande réservée
**Effort** S · **Confiance** vérifié

**Où.** `KyberKit/StreamView.swift:537-540` : `sendPosition(event); mouseButton(.right, pressed: true)`. `sendPosition` s'arrête dans la bande, mais le clic part quand même. `otherMouseDown` (`:556`) et `scrollWheel` (`:564`) ne testent pas `inReservedBand`, alors que `mouseDown` le fait (`:526`).

**Scénario.** Barre ouverte, un clic droit ou un défilement dans les 150 pt du haut fait un clic droit ou un défilement sur le PC, à la dernière position connue.

**Correctif.** `if !relative.active, inReservedBand(event) { return }` sur ces appuis et sur la molette. Laisser passer les relâchements.

### MAC-10 — Rien n'est relâché à la veille, au verrouillage ni au changement d'utilisateur
**Effort** S · **Confiance** plausible

**Où.** Aucun observateur de `NSWorkspace.willSleepNotification`, `screensDidSleepNotification` ni `sessionDidResignActiveNotification` dans `apps/macos/KyberNow` ou `KyberKit`. Le relâchement repose seulement sur le focus de la fenêtre et de l'app.

**Scénario.** On ferme le capot avec une touche ou un bouton tenu, sans que l'app perde le focus avant la veille : la touche reste enfoncée sur le PC jusqu'au retour.

**Correctif.** Observer ces trois notifications et appeler `releaseAll()` sur chaque vue, plus `input.releaseAll()` pour PlayStation.

### MAC-11 — L'avis d'emprunt ne prend pas le clic
**Effort** S · **Confiance** vérifié

**Où.** `KyberNow/SessionChrome.swift:23`, `if case .request? = session.hint, …`, qui ne traite que `.request`. Or `.lendRequest` est un `Button` qui publie aussi `HintRectKey` (`:102-108`).

**Scénario.** « Léa veut emprunter l'ordinateur » : le clic tombe dans `hitTest` qui rend nil, et part donc vers le PC au lieu d'ouvrir le panneau de prêt.

**Correctif.** Accepter `.lendRequest` dans le même test.

### MAC-12 — Multi-écrans : le mode de souris ne suit que l'écran principal
**Effort** S · **Confiance** vérifié

**Où.**
- `SessionController.swift:2272` : `streamView.mouseMode = choice.streamMode`, sur la seule vue principale.
- `:2260` : `toggleMouseCapture` lit `streamView.isRelative`.

**Scénario.** Session sur deux écrans, « Jeu » choisi dans la barre : l'écran secondaire reste en souris du bureau. Pire, ⌃⌥⌘M avec la souris capturée sur le secondaire repasse en « Jeu » au lieu de la libérer.

**Correctif.** Appliquer le mode à toutes les vues d'écran, et décider de la bascule d'après `mouseCaptured`.

### MAC-13 — Un mot de passe changé n'atteint jamais iCloud
**Effort** S · **Confiance** vérifié

**Où.**
- `CloudPublisher.swift:151` : `for host in hosts where !copied.contains(host.id)`.
- `Model.swift:900` : `updatePassword` ne fait que `setPassword` (pas de `save()`, donc pas de `didSave`).
- `Model.swift:840-841` : dans `addInvited`, `update(host)` (qui publie) passe avant `setPassword`.

**Scénario.** « Mot de passe refusé », puis nouveau mot de passe sur le Mac : l'Apple TV et l'iPhone gardent l'ancien jusqu'au prochain lancement. Même chose quand on réinvite un ordinateur déjà connu : l'ancien mot de passe est copié et marqué comme fait.

**Correctif.** Dans `setPassword`, retirer l'id de `copied` et republier. Dans `addInvited`, ranger le mot de passe avant `update`.

### MAC-14 — Trousseau : écriture qui peut perdre le secret, lectures sur le fil principal
**Effort** S · **Confiance** vérifié (latence plausible)

**Où.** `Model.swift:1153-1159` (et le même schéma dans `Accounts.swift:128-139`) : `SecItemDelete` puis `SecItemAdd`, dont le statut est ignoré. `console(for:)` (`Model.swift:954`) lit `Keychain.get` pour chaque ordinateur à chaque sondage (8 s), sur l'acteur principal, dans le trousseau ancien (pas de `kSecUseDataProtectionKeychain`).

**Scénario.** Si `SecItemAdd` échoue (trousseau verrouillé, ACL changée après une nouvelle signature), le mot de passe ou la session du compte est déjà effacé. Et une demande d'ACL bloquerait l'interface à chaque sondage.

**Correctif.** Faire `SecItemUpdate`, puis `SecItemAdd` si l'élément manque, comme le fait déjà `CloudKeychain.set`, et journaliser le statut. Garder les mots de passe en mémoire après la première lecture.

### MAC-15 — Caméra : un arrêt pendant l'ouverture la laisse allumée
**Effort** S · **Confiance** plausible

**Où.** `KyberKit/SessionEngine.swift:870-878` : après `await camera.open(…)`, `kn_camera_start` et `startCameraPolling()` sont appelés sans vérifier que la caméra est toujours voulue. `cameraOff()` (`:886`) a pu passer pendant l'attente, en particulier pendant la demande d'accès du premier usage.

**Scénario.** On allume puis on éteint vite la caméra, ou la session se termine pendant la demande d'accès. La capture repart et la caméra est envoyée, alors que l'interface la dit éteinte.

**Correctif.** Un numéro de demande incrémenté par `cameraOn` et `cameraOff`, revérifié après l'`await` (sinon `camera.close()`).

### MAC-16 — iOS et tvOS : le son ne reprend pas après une interruption
**Effort** M · **Confiance** vérifié (absence de code), effet plausible

**Où.** `KyberKit/AudioPlayerCore.swift:222-225` : `#else private func watchOutputDevice() {}`. Aucun fichier de KyberKit, `apps/ios` ou `apps/tvos` n'observe `AVAudioSession.interruptionNotification`. Les observateurs de `routeChangeNotification` ne mettent à jour que l'interface.

**Scénario.** Appel, Siri ou alarme pendant une session sur iPhone : RemoteIO reste arrêté jusqu'à la fin de la session. Passage sur AirPods : la latence de sortie (`kn_audio_set_output_latency_ms`) reste l'ancienne.

**Correctif.** À la fin d'une interruption et à chaque changement de route, relancer l'unité et relire la latence sur `outputQueue` (`stopOutput`/`startOutput` existent). C'est du code partagé : à signaler aux audits iOS et tvOS.

### MAC-17 — PlayStation au clavier : touches tenues et raccourcis de l'app
**Effort** S · **Confiance** vérifié (le 3ᵉ point est plausible)

**Où.**
- `KyberNow/PlayStationSession.swift:483-489` : `didResignActiveNotification` n'appelle que `updateChrome()`. `StreamView` relâche tout à ce moment-là, parce que Spotlight ne prend pas toujours le focus de la fenêtre.
- `KyberKit/PlayStationInput.swift:139-153` : `let down = event.modifierFlags.contains(flag) && …`. Verr. Maj n'envoie qu'un `flagsChanged` par bascule, or `KeyCatcher` permet de l'affecter à un bouton (`PlayStationSettings.swift:556`).
- `PlayStationInput.swift:108-120` : `return super.performKeyEquivalent(with:)` pour tout ce qui n'est pas ⌃⌥⌘.

**Scénario.**
- ⌘Espace en jeu : la touche tenue reste enfoncée sur la console.
- Le bouton affecté à Verr. Maj reste tenu jusqu'à la bascule suivante.
- ⌘ affecté à une action, puis W pressé en jeu : Fichier › Fermer met fin au jeu.

**Correctif.** `input.releaseAll()` à `didResignActive`. Verr. Maj en appui puis relâchement, comme `StreamView` (`:607`), ou refusé par `KeyCatcher`. En jeu, hors clavier de la console, consommer les raccourcis non réservés comme `StreamView` (`:657`).

### MAC-18 — Décodeur : garde de `pts` et générations
**Effort** S · **Confiance** plausible

**Où.**
- `KyberKit/VideoDecoder.swift:100-107` : `lastDelivered` n'est remis à zéro que dans la branche `.pyrowave`. `:228` : `frame.pts <= last` jette l'image.
- `:262-268` : une erreur d'une ancienne génération ne reconstruit pas la session, mais appelle quand même `markLost()`.

**Scénario.**
- PyroWave → HEVC (« Passer en HEVC ») : si le nouvel encodeur repart avec des `pts` plus bas, toutes les images sont jetées sans bruit et sans `onLostReference`, et l'image reste figée. Cela dépend des `pts` de Kyber, non vérifiés.
- Plusieurs erreurs après une veille : chacune force l'attente d'une image clé, même après la reconstruction.

**Correctif.** `lastDelivered = nil` à tout changement effectif de codec. `guard generation == self.generation else { return }` avant `markLost()`.

### MAC-19 — Sondage : écritures pour un ordinateur retiré, tours en double
**Effort** S · **Confiance** vérifié

**Où.**
- `Model.swift:983-984` (`let info = try await console.info()` puis `status[host.id] = …`) et `:1037` ; le même schéma dans `Lend.swift` `refresh`.
- `Model.swift:943` : `refreshAll` sans garde. `MenuBar.swift:352` : `Task { await app.store.refreshAll() }` à chaque ouverture du menu.

**Scénario.** On retire un ordinateur pendant un sondage : `status`, `lends` et la vérification de la Station sont recréés pour un id disparu. Ouvrir le menu plusieurs fois, ou pendant le sondage régulier, lance plusieurs tours complets en même temps (info, prêt, aperçu, présence au relais, qui limite les vérifications par minute).

**Correctif.** `guard self.host(host.id) != nil` après chaque `await`. Garder la `Task` de `refreshAll` en cours et l'attendre au lieu d'en lancer une autre.

### MAC-20 — « Même empreinte ? » : trois règles
**Effort** M · **Confiance** vérifié (l'écart), effet plausible

**Où.**
- Comparaisons directes avec `==` : `KyberNowApp.swift:284`, `AddHostFlow.swift:186`, `HomeView.swift:627`, `WelcomeViews.swift:227`, et sur iOS `AppModel.swift:285, 292, 482`.
- `HostStore.sameFingerprint` (`Model.swift:856`) retire `sha256:` et les deux-points.
- `same()` du cœur (`apps/core/src/account.rs:835`) ne normalise que la casse et les espaces.

**Scénario.** Une empreinte écrite dans un autre format (invitation, `admin_fp` d'un compte) est « la même » pour `addInvited`, mais pas pour la fusion du cœur. Résultat : le même ordinateur deux fois à l'accueil, ou « Ajouter » proposé pour un ordinateur déjà connu.

**Correctif.** Une seule règle dans le cœur (normalisation complète dans `same`, exposée en `kn_fingerprint_same`), et un seul helper Swift à la place des cinq comparaisons.

### MAC-21 — Découverte : navigateur Bonjour jamais relancé
**Effort** S · **Confiance** plausible

**Où.** `KyberNow/Discovery.swift:49-62` : pas de `stateUpdateHandler`, et `start()` sort tout de suite si `browser != nil`.

**Scénario.** Accès au réseau local refusé puis accordé, ou mDNS réinitialisé : la découverte de l'accueil reste morte jusqu'au prochain lancement, et les ordinateurs d'un compte ne sont plus vus sur le réseau.

**Correctif.** Sur `.failed`, annuler, remettre `browser = nil` et relancer après un délai.

### MAC-22 — Courses mineures entre fils
**Effort** S · **Confiance** vérifié (impact faible)

- `KyberKit/RelativeMouse.swift:17-70` : `active`, `remainder` et `onMove` sont écrits sur le fil principal (`start`, `stop`, `StreamView.attach`) et lus sur la file de GCMouse. Remplacer la fermeture `onMove` pendant qu'elle est appelée est une course sur le compteur de références. Correctif : `queue.sync` pour `onMove` et `remainder`, `active` atomique.
- `KyberKit/VideoRenderer.swift:131-134` : `vsync` et `latency` sont posés depuis le fil principal et lus sur les fils de décodage et de présentation sans verrou. Correctif : les ranger sous `lock` comme les autres propriétés.
- `KyberKit/CameraCapture.swift:217, 339` : `firstPTS` est écrit sur `queue` et lu dans `deliver` (`:388`, fil de VideoToolbox). Correctif : calculer l'horodatage sur `queue`, ou le capturer dans la fermeture d'encodage.

### MAC-23 — Passerelle Rust : arrêts brutaux et route illisible
**Effort** S · **Confiance** vérifié

- **`unwrap()` sur des verrous** : `apps/apple/src/lib.rs:116, 128, 1130, 1240` et `input.rs:604-645, 771, 809-813`. En édition 2024, une panique dans un `extern "C"` arrête le processus, et rien n'utilise `catch_unwind`. Le risque existe seulement si le verrou a été empoisonné par une autre panique. Correctif : `lock().unwrap_or_else(|e| e.into_inner())`.
- **Route de relais illisible** : `relay.rs:27` et `account.rs:261`, `optional(route_json).and_then(|r| serde_json::from_str(&r).ok())`. Une route enregistrée devenue illisible devient une connexion directe sans relais : l'utilisateur voit « injoignable » au lieu d'une erreur de relais. Correctif : rendre une erreur, comme le fait `kn_console_new_account` pour `access_json`.

### MAC-24 — Petits écarts
**Effort** S

- `SessionController.swift:2197-2199` (`showHint`, `guard self?.hint == which`) : deux « L'image a été relancée. » à 2 s d'écart, et la minuterie du premier efface le second au bout de 2 s au lieu de 4. Correctif : un numéro de génération par rappel. *Vérifié.*
- `KyberKit/KeymapMac.swift:17-47` : pas d'entrée pour `kVK_ContextualMenu` (0x6E) vers HID 0x65, donc la touche Menu d'un clavier PC branché au Mac est perdue. *Plausible.*
- `PlayStationSession.swift:90, 490-498` : `padConnected = !GCController.controllers().isEmpty`, alors que le cœur ignore le clavier dès que SDL voit une manette (`apps/core/src/playstation/play.rs:476`). L'invite « cliquez pour jouer » peut contredire ce que la console reçoit. Correctif : exposer le nombre de manettes du cœur. *Plausible.*
- `PyroWaveDecoder.swift:259` : `decode_is_ready(decoder, false) || decode_is_ready(decoder, true)`. Le second test (incomplet permis) rend le premier inutile, donc une image incomplète est affichée, contrairement aux commentaires (`:8-9`, `:235`, « incomplète, sautée »). Correctif : trancher, puis aligner le code, les commentaires et `docs/pyrowave.md`. *Vérifié.*

---

## Optimisations

### MAC-25 — Fin de session : travail lourd sur le fil principal
**Effort** S · **Confiance** vérifié

`end()` (`SessionController.swift:2108-2110`) appelle `engine?.snapshot(maxWidth: 1600)`, qui attend le GPU avec `commands.waitUntilCompleted()` et partage la file de commandes des images en cours (`VideoRenderer.swift:1009-1013`). La capture est ensuite copiée deux fois (`[UInt8]` puis `Data`). Enfin, `store.imagery.saveLast` encode un JPEG de 1600 px et l'écrit (`HostImagery.swift:67-69`), juste avant l'animation de fermeture. Effet : une saccade au début du passage, multipliée par le nombre de sessions à la sortie de l'app (`endAll`).

**Correctif.** Garder la capture sur le fil principal (le passage en a besoin), lire les octets directement dans un `Data(count:)`, et faire l'encodage et l'écriture sur une file utilitaire.

### MAC-26 — Fil temps réel du son
**Effort** S · **Confiance** vérifié

`AudioPlayerCore.swift:189-192` : une fois par seconde, le rappel de Core Audio alloue un bloc et appelle libdispatch (`DispatchQueue.main.async { onStats(stats) }`). Ce saut est d'ailleurs doublé, car `onStats` repasse lui-même par `main.async` (`SessionEngine.swift:1633`).

**Correctif.** Lire les statistiques depuis une minuterie d'une seconde hors du fil temps réel (le moteur les garde déjà), et retirer le saut en double.

### MAC-27 — `renderQueue.sync` depuis le fil principal
**Effort** M · **Confiance** plausible

`VideoRenderer.swift:439` (`attach(to:)`) et `:240` (`attachVideo`, tvOS) attendent la file de rendu. Celle-ci peut être bloquée dans `layer.nextDrawable()` (`:646`) jusqu'à 1 s (`allowsNextDrawableTimeout = true`, `:450`), par exemple pour une fenêtre cachée, sur un autre bureau ou pendant une transition plein écran. `MetalSurface.show` vient du fil principal.

**Correctif.** Échanger la couche sous `lock` et la configurer au rendu suivant, ou rendre `attach` asynchrone.

### MAC-28 — Sondage et invalidations trop larges
**Effort** M · **Confiance** plausible

- `Model.swift:954-968` : un `ConsoleClient` neuf (`kn_console_new`) par ordinateur à chaque sondage, donc une poignée de main TLS et une file neuves toutes les 8 s. Correctif : un client mis en cache par (adresse, port, utilisateur, empreinte, route), invalidé quand l'ordinateur ou son mot de passe change.
- `KyberNowApp.swift:685` : `.onReceive(app.store.objectWillChange) { app.objectWillChange.send() }`, plus d'autres relais dans `AppModel.init`. Chaque état sondé invalide toute la scène, réglages et sessions compris. Correctif : que chaque vue observe le magasin qu'elle lit.
- `Notifications.swift:106` : `checkStationUpdates` écrit dans UserDefaults à chaque changement de `store.$status`. Correctif : n'écrire que si `announced` a changé.

### MAC-29 — AV1 : trois copies de chaque OBU (toujours présent)
**Effort** M · **Confiance** vérifié

`AV1.swift:43-50` construit `headerBytes`, `payload` et `bytes` en `Data`. `VideoDecoder.swift:134-148` fait ensuite `frame.append(obu.bytes)`, puis `:525` copie le tout dans le `CMBlockBuffer`. Une image 4K de 200 à 500 Ko est ainsi copiée environ quatre fois, avec plusieurs allocations, sur le fil réseau de Kyber.

**Correctif.** Pour les OBU qui ont déjà leur champ de taille, calculer des tranches et écrire directement dans le `CMBlockBuffer`. Ne réécrire que les OBU sans taille et l'en-tête de séquence.

### MAC-30 — Allocations par image ou par geste
**Effort** S · **Confiance** vérifié

- `VideoDecoder.swift:154` (`[UnsafeBufferPointer]`) et `:566-587` (`nalUnits` construit un tableau) : deux tableaux par image HEVC ou H.264. Correctif : parcourir les NAL avec une fermeture, ou réutiliser un tampon.
- `CameraCapture.swift:374-383` : le `Data` Annex B grossit plusieurs fois par image. Correctif : `reserveCapacity(length + 64)`. Le même code suppose aussi un `CMBlockBuffer` contigu (`lengthAtOffsetOut: nil`) : vérifier `lengthAtOffset == totalLength`, sinon `CMBlockBufferCopyDataBytes`.
- `StreamView.swift:234-268` : chaque mouvement du curseur de l'ordinateur relit `pointer.state` (appel au cœur et verrou) au lieu de l'état reçu, et repose `contents` même sans changement de forme. Correctif : utiliser l'état passé et ne toucher `contents` que si `change.shape`.
- `InputCore.swift:458` puis `SessionEngine.swift:1411` : la combinaison de manette passe deux fois par le fil principal avant d'ouvrir la barre. Correctif : retirer le saut intérieur.

---

## Factorisation

### MAC-31 — Liste des correctifs clients recopiée dans 8 scripts
**Effort** M · **Confiance** vérifié

`apps/macos/kyber-patches` sert à toutes les apps, y compris un correctif propre à Windows (`0004-kynput-presse-papiers-windows`). La liste `PATCHES=(…macos/kyber-patches… + server *-kyclient-*)` est reconstruite dans :
- `apps/macos/scripts/build-kyclient.sh:100`
- `apps/apple/scripts/build-apple.sh:117`
- `apps/apple/scripts/build-apple-mobile.sh:90`
- `build-kyber-tvos.sh:75`
- `android/build-native.sh:180`
- `windows/build.sh:105`
- `webos/build.sh:144`
- `linux/build.sh:82` (qui ajoute en plus le correctif de webOS)

MAC-03 vient directement de cette dispersion.

**Correctif.** Une fonction `kyber_client_patch_list [extra…]` dans `scripts/kyber-client-patches.sh`, qui serve à la fois à la pose et à l'empreinte, et un dossier renommé `apps/kyber-patches`.

### MAC-32 — Copies entre Mac, iOS et tvOS
**Effort** M · **Confiance** vérifié

- `HostImagery` : `KyberNow/HostImagery.swift:11`, `ios/KyberNowIOS/Platform.swift:144`, `tvos/KyberNowTV/Platform.swift:178`. Même logique ; seuls le type d'image, le dossier et la largeur de l'aperçu changent.
- Dessin de la marque (`MarkGeometry`, `Mark`, `NeonGlow`, `Icon`, `SignalCorner`) : `macos/KyberNow/Theme.swift:275-402`, `ios/KyberNowIOS/Theme.swift:143-228`, `tvos/KyberNowTV/Theme.swift:144-229`. `MarkGeometry` est identique au commentaire près.
- `CloudHost` et la requête du Trousseau iCloud : `CloudPublisher.swift:26` et `tvos/KyberNowTV/CloudSync.swift:18` (compilé aussi pour iOS), avec le service `"fr.leslignesclaires.kybernow.cloud"` écrit deux fois. Un champ ajouté d'un côté serait perdu en silence de l'autre.

**Correctif.** Un fichier partagé par sujet, listé dans les trois `project.yml`, sur le modèle de `Model.swift` ou `LinkUI.swift`, qui le sont déjà.

### MAC-33 — Entrées PlayStation et petits doublons
**Effort** M · **Confiance** vérifié

- `PlayStationInput.swift:108-153` et `apps/ios/KyberKitIOS/PlayStationKeyMouse.swift:93-142` réimplémentent tous deux la détection de ⌃⌥⌘, les touches et boutons tenus et le relâchement. Or le `KeyMouse` du cœur tient déjà `held` et `release_all()`. Les deux copies divergent déjà (Verr. Maj, MAC-17).
- Le même `switch` vers `PadKey` existe dans `InputCore.swift:463-471` et `PlayStationCore.swift:285-293`.
- `Notifier.sentence` (`Notifications.swift:173`) recopie `ShareText.sentence` (`LinkUI.swift:240`), sans la garde de chaîne vide : `""` devient `"."`.

**Correctif.** Les règles des raccourcis et des touches tenues dans le cœur, ou dans un seul helper Swift partagé ; un seul `PadKey(core:)` ; et `ShareText.sentence` partout.

### MAC-34 — Codes d'erreur recopiés à la main
**Effort** S · **Confiance** vérifié

- `apps/apple/src/invite.rs:22-27` (60–65) et `lib.rs:195-197` (30–32) répètent des nombres déjà écrits en dur dans `core/src/invite.rs:116-125` et `core/src/relay.rs:198-212`.
- `ConsoleClientCore.swift:449` teste `case 70...80:`.

**Scénario.** Si le cœur renumérote un code, Swift classe mal l'erreur sans aucun avertissement.

**Correctif.** Un test Rust qui vérifie que chaque `KN_*` vaut son `Error::code()`, et les constantes nommées côté Swift.

---

## Code mort

### MAC-35 — 15 fonctions exportées sans appelant
**Effort** S · **Confiance** vérifié (`git grep -w` hors `third_party` et hors `apps/apple/src` : seules des docs les citent)

- `kn_console_share_watch`
- `kn_audio_set_muted`
- `kn_camera_is_on`, `kn_camera_offered`
- `kn_gamepads_start_sdl`
- `kn_input_is_active`, `kn_input_set_active`
- `kn_screens_primary_index`
- `kn_session_is_active`, `kn_session_set_active`
- `kn_session_is_muted`, `kn_session_set_muted`
- `kn_session_microphone`
- `kn_session_no_image`
- `kn_session_wait`

L'état actif et le son coupé passent désormais par le registre du focus.

**Correctif.** Les retirer, ainsi que leurs mentions dans `docs/coeur-apple.md`, `docs/plusieurs-ordinateurs.md`, `apps/core/README.md` et `apps/macos/README.md` (FR et EN). Exception : `kn_session_wait`, à garder pour corriger MAC-01.

### MAC-36 — Swift sans appelant
**Effort** S · **Confiance** vérifié (Mac, iOS, tvOS)

- `StreamOptions` (`KyberKit/KyberTypes.swift:26`) et `ConnectionSettings.options` / `options(width:height:refresh:)` (`KyberNow/Model.swift:377-391`).
- `ConnectionSettings.modePolicy` (`Model.swift:424`) : la règle vit maintenant dans le cœur.
- `KyberDisplay.resized` (`KyberTypes.swift:15`).
- `InfoTip` (`Theme.swift:828`), `Theme.radiusControl` et `radiusSheet` (`:128`, `:130`).
- `LendTarget.hasRelay` (`LendCore.swift:98`).
- `Core.setLanguage` et `Core.nowMicros` (`CoreBridge.swift:36, 41`).
- Dans `SessionEngine.swift` : `register(_:screen:)` (`:1117`), `unregister(screen:)` (`:1137`), `setVolume` (`:1250`), `hdrUsed` (`:1064`), et `microphoneState` (`:448`), écrit mais jamais lu.
- `PadCombo.buttons`, `PadCombo.axes`, `PadCombo.navigationKey` (`InputCore.swift:98-100, 141`) : la combinaison et la navigation sont dans le cœur.
- `occlusionObserver` (`SessionController.swift:328`) : jamais affecté, seulement retiré.
- `PyroWaveDecoder.available` (`:210`).
- Le setter `VideoRenderer.sdrWhite` (`:203-206`) n'est jamais appelé. En mode `KN_HDR_LAYER=linear`, le shader garde donc 203 nits au lieu du blanc calibré : à brancher sur `HDRCalibrationStore` ou à retirer.
- `DualSenseHaptics.swift:160-170` : `inputProcRefCon` est passé puis jeté (`_ = refCon`).
- `LatencyReporting.swift:79-80` : `ChainTimes.all` et `.sum`. En revanche, `received(bytes:)`, `hole()` et `keyframeRequested()` du protocole (`:85-88`) ne sont jamais appelés par `VideoRenderer` : à retirer du protocole ou à brancher.
- `HDRDemo.swift:139-140` pose deux fois `kCVImageBufferYCbCrMatrixKey`.

### MAC-37 — Chaînes de traduction mortes
**Effort** S · **Confiance** vérifié

- `KyberKit/Localizable.xcstrings` : une vingtaine d'entrées ne sont plus utilisées par aucun Swift, par exemple « session terminée », « configuration vidéo », « identifiant ou mot de passe refusé », « vérification du certificat », « lecteur vidéo » ou « l'ordinateur ne répond plus ». Ces textes d'erreur viennent maintenant du cœur.
- `KyberNow/Localizable.xcstrings` :
  - trois entrées marquées périmées : « Faire jouer », « Faire regarder », « Vous pouvez jouer. » ;
  - quatre clés sans usage : « Ajouter un ordinateur… » (le menu construit `LinkText.addTitle + "…"`), « Ajoutez un ordinateur équipé de KyberNow Station… », l'ancienne aide PSN et « Ouvrez votre ordinateur sur ce Mac ».
- `scripts/check-strings.py:47` saute en silence les entrées `stale`.

**Correctif.** Retirer ces entrées. Faire lister par le script les entrées périmées et, en option, les clés absentes des sources.

### MAC-38 — Icônes
**Effort** S · **Confiance** vérifié

`scripts/phosphor-icons.mjs` vide `Assets.xcassets/Icons` (`rmSync`) puis le régénère depuis `ICONS` (`:12`). Cette liste n'a pas `arrow-up`, `microphone`, `microphone-slash`, `video-camera` ni `video-camera-slash`, qui sont pourtant présentes sur disque et utilisées (`HomeView.swift:771`, `SettingsView.swift:671`, `SessionChrome.swift:956, 986`) : relancer le script les supprimerait. À l'inverse, `broadcast`, `camera`, `eye-slash`, `magnifying-glass`, `monitor-play`, `plugs-connected`, `qr-code` et `wifi-slash` (16 jeux, deux graisses chacun) sont livrées sans usage.

**Correctif.** Corriger `ICONS` dans les deux sens et régénérer une fois.

Autre migration à retirer : `BundleRename` (`KyberNowApp.swift:615-640`) traite les installations d'avant la 0.4.3, et son `waitUntilExit` bloque `App.init`. Elle peut partir maintenant que l'app est en 0.11.

---

## Doc et cohérence

### MAC-39 — Tests du Mac qui ne compilent plus, en silence
**Effort** M · **Confiance** vérifié

`tests/fullscreen/run.sh:16` et `tests/screen/run.sh:10` compilent `MetalSurface.swift`, `VideoDecoder.swift` et `VideoRenderer.swift` sans `PyroWaveDecoder.swift` (que `VideoDecoder` utilise) ni `HDR.swift` (`HDRDisplay`, utilisé par `MetalSurface`). Leur dernière modification date du 26 octobre, avant PyroWave. Partout, `swiftc … 2>&1 | grep error || true` (et `tests/video/run.sh:61-64`, `tests/pyrowave/run.sh:24-25`) masque l'échec, puis le script lance un binaire absent.

**Correctif.** Compléter les sources ou ajouter des bouchons, et arrêter le script à la première erreur de compilation (`set -o pipefail`, sans `|| true`).

### MAC-40 — Commentaires et docs périmés
**Effort** S · **Confiance** vérifié

- `apps/apple/scripts/build-apple-mobile.sh:22-28` dit que PyroWave n'est posé « qu'avec --kyber-pyrowave », alors que `:35` l'allume par défaut (`KYBER_PYROWAVE:-1`).
- `KyberNow/Info.plist` : `CFBundleShortVersionString` vaut « 1.0 » et `CFBundleVersion` « 1 ». L'app construite affiche bien 0.11.x grâce à `MARKETING_VERSION`, mais le fichier trompe le lecteur. Retirer ces deux clés ou utiliser les variables.
- `SessionEngine.swift:403` : « doivent être enregistrées (`register`) », alors que le code passe par `setScreenViews` (voir MAC-36).
- `docs/coeur-apple.md`, `docs/plusieurs-ordinateurs.md` et les README : ils décrivent les fonctions mortes de MAC-35.
- `apps/apple/src/playstation.rs:315` : sans rappel vidéo, la partie refuse de démarrer. C'est voulu, mais pas écrit dans l'en-tête.
- `scripts/embed-kyber.sh:33` copie avec `rsync --update`. Une bibliothèque remplacée par une autre de date égale ou plus ancienne n'est pas recopiée dans l'app (plausible) : comparer le contenu des `.dylib` (`--checksum`).

Écarts avec `docs/link-ui.md` : les contrôles par sondage (menu de la barre des menus, notifications, comptes dans les réglages, déconnexion) concordent. Seuls MAC-05 (le voyant de la caméra et le bandeau PyroWave que la règle demande, absents à l'écran) et MAC-11 (l'avis d'emprunt cliquable comme celui de la demande) s'en écartent.

---

## Mes 10 priorités

1. **MAC-02** : `commit()` systématique après un décodage PyroWave réussi. Une ligne, et un gel définitif évité.
2. **MAC-03 + MAC-31** : l'empreinte du Vendor doit couvrir les correctifs de la Station, avec une seule liste partagée par les 8 scripts. Cela évite une publication avec des bibliothèques périmées.
3. **MAC-05** : bandeau PyroWave et voyant de la caméra de nouveau visibles (confidentialité).
4. **MAC-04 + MAC-13** : iCloud ne reçoit que les ordinateurs ajoutés à la main, et toujours le dernier mot de passe.
5. **MAC-06** : crochet immersif retiré au second plan, et modificateurs décidés d'après leurs bits (fin du ⊞ collé).
6. **MAC-01** : rappel `release_userdata` dans la passerelle, d'abord pour la couche du pointeur et la session. C'est le seul plantage mémoire plausible restant.
7. **MAC-07 + MAC-08** : garde de phase après `end()`, et contrôleur retenu jusqu'à la fermeture de la fenêtre.
8. **MAC-09 + MAC-10 + MAC-17** : entrées propres (bande réservée, veille, PlayStation au second plan).
9. **MAC-14 + MAC-19** : Trousseau mis à jour sans effacer d'abord, sondage sans doublons ni écritures fantômes.
10. **MAC-39 + MAC-35 à MAC-38** : tests qui échouent pour de vrai, puis le ménage (FFI, Swift, chaînes, icônes). Peu de risque et beaucoup de clarté avant la sortie.

## Ce qui est sain

- **Durée de vie de la session.** `passRetained` équilibré, libéré tout de suite si le départ échoue, et `KnPlatform` copiée côté Rust. Les rappels du cœur remontent tous sur le fil principal. Les paquets vont au décodeur sous verrou, sans attente du fil principal. Aucun `DispatchQueue.main.sync` ni sémaphore attendu sur le fil principal.
- **Pas de cycle de rétention trouvé.** `[weak self]` est utilisé partout (moteur, vues, notifications, Combine). `teardown` retire les observateurs et la couche SwiftUI.
- **Souris et curseur.** `NSCursor.hide/unhide` et `CGAssociateMouseAndMouseCursorPosition` sont appariés. Chaque fin passe par `detach()`. `StreamView` relâche à `didResignKey`, `didResignActive` et `resignFirstResponder`, utilise les bits gauche/droite des modificateurs, et resynchronise au retour.
- **L'interblocage iOS n'existe pas sur le Mac.** Le `PointerLayer` du Mac a `holdsPosition: false`, n'appelle jamais `move_by`, et ne tient jamais son verrou pendant un appel au cœur.
- **Lecture des flux sans force-unwrap.** NAL et Annex B, `BitReader` borné, OBU et LEB128 AV1, en-tête de séquence, `PyroWaveConfig`. Aucun `try!` ni `as!` sur des données externes (le seul `as!` est une animation locale).
- **Pipeline vidéo.** Décodage asynchrone avec générations, durée de vie des `CVMetalTexture` tenue jusqu'à la fin du GPU, `CVMetalTextureCacheFlush` chaque seconde, regroupement des images correct sous verrou, deux verrous séparés (`lock`/`out`) sans interblocage. Côté PyroWave : 4 emplacements pour 3 images en vol et un cache de textures par IOSurface.
- **Son.** Sortie par défaut suivie sur sa propre file, arrêt et départ sérialisés.
- **Caméra.** Encodeur vidé avant l'invalidation, observateurs retirés.
- **Minuteries raisonnables.** Caméra à 4 Hz seulement allumée, marge EDR à 2 Hz seulement sur écran HDR, mesures à 1 Hz, barre à 10 Hz seulement ouverte, découverte toutes les 3 s et en pause pendant un jeu.
- **Logique dans le cœur.** Comptes, partage, prêt, accueil, relais, mise à jour des Stations, table de touches, ⌘ en Ctrl, échange ISO, règles de souris, combinaison de manette, état du pointeur, protocole PlayStation. La passerelle `apps/apple` n'est que de la colle JSON et C. Les 61 fonctions qui rendent une chaîne sont libérées côté Swift (`Core.take`) sur tous les chemins, et les pointeurs venus de Swift sont contrôlés.
- **Projet.** `project.yml` et `project.pbxproj` concordent. Les entitlements (caméra, micro, iCloud, groupe de trousseau) sont tous nécessaires, et chaque script est référencé.
- **Code tenu.** Aucun TODO ni FIXME dans la zone. Catalogues FR et EN complets.
