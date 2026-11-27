# Audit du 2026-11-27 — app Android (`apps/android`)

Périmètre : `apps/android` (Kotlin/Compose, pont JNI Rust `jni/src`, PyroWave C++ `jni/pyrowave`, manifeste, Gradle, scripts, README). Analyse statique seulement : rien compilé ni lancé, aucun appareil. Les chemins Kotlin sont relatifs à `apps/android/app/src/main/java/fr/leslignesclaires/kybernow/` sauf mention contraire. Chaque constat ci-dessous a été relu dans le code après le passage des sous-agents ; les points qu'on ne peut pas trancher sans appareil sont marqués « plausible ».

## Le plantage du testeur (mode bureau, 2 à 5 min) : ce qu'on peut dire

Aucune cause n'est certaine sans journal. Les pistes, par ordre de vraisemblance :

1. **Aucune trace possible aujourd'hui** (AND-01). Une panique Rust dans un export JNI arrête l'app sans un mot dans logcat. Une panique dans le fil de session devient la fin « session.internal », sans cause écrite. C'est la première chose à livrer au testeur.
2. **PyroWave (Vulkan) en fenêtre libre** (AND-02, AND-03, AND-04). Un redimensionnement renvoie `SUBOPTIMAL`/`OUT_OF_DATE` ; si la recréation de la swapchain échoue, l'image suivante appelle `vkAcquireNextImageKHR` sur une swapchain nulle, ce qui donne un SIGSEGV dans le pilote. Ne concerne que PyroWave (Auto en filaire seulement : Ethernet, fréquent sur une tablette en mode bureau).
3. **Perte de focus en mode bureau** (AND-05). Elle n'est pas gérée : capture du pointeur perdue pour de bon, touches et clics restés enfoncés sur l'ordinateur. Pour un testeur, cela ressemble à une session figée.
4. **Recréation de l'activité en pleine session** (AND-06). Un écran externe large gamme ou HDR (`colorMode`), la langue ou la taille de police recréent l'activité et ferment la session ; pour un invité ou un emprunt, l'activité se ferme sans rien dire.
5. **Wi-Fi en économie d'énergie** (AND-07). Aucun WifiLock : rafales de pertes, puis relances sans image, et au bout de trois la session est coupée.
6. **Interblocage du pointeur.** Si le testeur avait une version antérieure à la 0.11.7, l'interblocage `PointerLayer`/kynput corrigé dans le cœur le 27/11 (`26b937ac`) touchait aussi Android : `PointerLayer::new(clock, true)` (`jni/src/session.rs:680`), et `pointerMoveTo` part du fil principal à chaque survol de la souris (`session/SessionActivity.kt:1666`). Ce serait un gel de l'app (ANR « L'application ne répond pas »), pas un plantage. **À demander au testeur : la version exacte et s'il voit « ne répond pas ».**

Pour trancher dès qu'on a un relevé :
- fin « session.internal » dans logcat : panique du cœur, AND-01 la nommera ;
- SIGSEGV dans `libvulkan` ou dans le pilote GPU : PyroWave (AND-02 à AND-04) ;
- `Native thread exited` ou `abort` sans message : panique dans un export JNI (AND-01, AND-08) ;
- ANR : interblocage, ou travail lourd sur le fil principal.

## Synthèse

| Id | Catégorie | Constat | Fichier | Effort | Confiance |
|---|---|---|---|---|---|
| AND-01 | bug critique | Aucun hook de panique, aucun journal exportable | `jni/src/exports.rs:62` | S→M | vérifié |
| AND-02 | bug critique | PyroWave : swapchain nulle après une recréation ratée | `jni/pyrowave/kn_pyrowave.cpp:988` | S | vérifié (plantage plausible) |
| AND-03 | bug | PyroWave : `vkResetFences` sur une clôture encore en vol | `kn_pyrowave.cpp:981` | S | vérifié |
| AND-04 | bug critique | `surfaceDestroyed` rend la main pendant que le décodeur écrit encore | `session/SessionActivity.kt:688`, `jni/src/session.rs:82` | M | plausible |
| AND-05 | bug critique | Perte de focus non gérée (capture du pointeur, touches enfoncées) | `session/SessionActivity.kt:1491` | S | vérifié |
| AND-06 | bug | `configChanges` incomplets : session fermée par une recréation | `AndroidManifest.xml:76,87` | S | plausible |
| AND-07 | bug | Aucun WifiLock | toute l'app | S | vérifié |
| AND-08 | bug | `AudioStream::drop` (ndk 0.9) panique si la fermeture échoue | `jni/src/audio.rs:74,84,121` | S | vérifié |
| AND-09 | bug critique | Mémoire libérée en cours d'usage : « Recevoir d'un autre appareil » | `ui/playstation/PsAdd.kt:491-505` | S | vérifié |
| AND-10 | bug | Mémoire libérée en fin de session (caméra, micro, réseau, minuterie) | `session/SessionActivity.kt:719-739,772` | M | vérifié (fenêtre étroite) |
| AND-11 | bug | « Revenir » (emprunt) ferme la session au lieu de la rouvrir | `session/SessionActivity.kt:1454` | S | vérifié |
| AND-12 | bug | Fenêtre déplacée sur un autre écran : fréquence, mode et HDR figés | `session/SessionActivity.kt:334-445` | M | plausible |
| AND-13 | bug | Souris capturée : historique ignoré, fractions perdues | `session/SessionActivity.kt:1508` | S | vérifié |
| AND-14 | bug | Barre ouverte : les clics et la molette passent aussi à l'ordinateur | `session/SessionActivity.kt:1637,1658` | S | vérifié |
| AND-15 | bug | Sans surface : images clés redemandées sans fin | `jni/src/video.rs:332-342` | S | vérifié |
| AND-16 | bug | Cycle `Shared` ↔ chaîne d'entrées : l'activité peut fuir | `jni/src/session.rs:333-348` | S | vérifié |
| AND-17 | bug | Annuler puis réemprunter : la nouvelle demande perd son état | `ui/AppModel.kt:620-641` | S | vérifié |
| AND-18 | bug | Console jointe hors de chez soi jamais refermée à l'annulation | `playstation/PlayStations.kt:228-251` | S | vérifié |
| AND-19 | bug | Recherche des consoles active pendant les sessions | `ui/home/HomeScreen.kt:297` | S | vérifié |
| AND-20 | bug | La découverte garde les ordinateurs d'un réseau quitté | `model/Discovery.kt:64` | S | plausible |
| AND-21 | bug | Adresse MAC mal validée : réveil promis, rien n'est envoyé | `ui/settings/Settings.kt:714` | S | vérifié |
| AND-22 | bug | Une seule entrée abîmée efface toute la liste des ordinateurs | `model/Hosts.kt:414,430`, `playstation/PsStore.kt:98` | S | plausible |
| AND-23 | bug | `strings.py` a perdu les 11 textes de la caméra | `scripts/strings.py` | S | vérifié |
| AND-24 | bug | Petits bugs : retour de la souris, recréation de l'accueil, MAJ, mDNS | divers | S | plausible |
| AND-25 | optimisation | Énumération des manettes à chaque événement d'un appareil inconnu | `session/Inputs.kt:115,161` | S | vérifié |
| AND-26 | optimisation | Chemin chaud : Bitmap par forme du pointeur, `GetMethodID` par rappel, allocations par événement | `session/SessionState.kt:529`, `jni/src/jvm.rs` | S | vérifié |
| AND-27 | optimisation | Manette en souris : minuterie à 125 Hz même au repos | `session/Inputs.kt:253-271` | S | vérifié |
| AND-28 | optimisation | Keystore sur le fil principal et dans la composition | `ui/home/HomeScreen.kt:271`, `model/Hosts.kt:550` | S | vérifié |
| AND-29 | optimisation | Accueil recomposé en entier (état géant, fond animé) | `ui/home/HomeScreen.kt:245-366` | M | vérifié |
| AND-30 | factorisation | Libellés du cœur ignorés, refaits en Kotlin | `ui/home/LendHome.kt:76`, `core/Native.kt:61,65,200` | S | vérifié |
| AND-31 | factorisation | Doublons : deux activités de session, deux listes « recouvre », poignées JNI | divers | M | vérifié |
| AND-32 | code mort | 17 fonctions natives jamais appelées, plus `kn_pw_hdr_output` | `core/Native.kt`, `core/PsNative.kt` | S | vérifié |
| AND-33 | code mort | Symboles Kotlin, chaînes, ressources, dépendances Gradle | divers | S | vérifié |
| AND-34 | doc | Écarts avec link-ui, README FR/EN, étiquette du journal, commentaire faux | divers | S | vérifié |

---

## AND-01 — Aucun hook de panique, aucun journal exportable (bug critique, diagnostic)

- **Où** : `jni/src/exports.rs:62-70` (`Native.init`, seul `android_logger::init_once`). Aucun `std::panic::set_hook` dans `apps/android/jni` ni dans `apps/core` (seul `apps/windows/src/system.rs:260` en a un). Aucun `Thread.setDefaultUncaughtExceptionHandler`, aucune lecture de `getHistoricalProcessExitReasons`, aucun fichier journal ni bouton « Envoyer le journal ».
- **Description** : sur Android, stderr ne va nulle part.
  - Une panique dans un export `extern "system"` arrête le processus (Rust ≥ 1.81 ; `rust-version = 1.89`) sans aucun message. Il y a 26 `lock().unwrap()` dans `jni/src`, et `.expect("fil de session")` à `core/src/session.rs:1434`.
  - Une panique dans le fil de session est rattrapée (`core/src/session.rs:1427`) et devient la fin `session.internal`, sans cause écrite.
  - Une panique dans `kybernow-image` ou `kybernow-affichage` tue le fil sans bruit : l'image fige, puis les relances sans image coupent la session.
- **Scénario** : le testeur sans `adb` voit l'app disparaître ou la session se couper ; personne ne peut dire pourquoi.
- **Correctif** :
  - **S, avant toute autre chose** : dans `Native.init`, un `set_hook` qui écrit avec `log::error!` le fil, le message, l'emplacement et `Backtrace::force_capture()`. Le modèle existe : `apps/windows/src/system.rs:260-267`.
  - **S** : dans `KyberNowApp.onCreate`, un gestionnaire d'exceptions non attrapées qui journalise, puis passe la main au gestionnaire d'origine.
  - **M** : un journal tournant dans `filesDir` (`kybernow.log` et `.1`) qui reçoit aussi `log` et `Log`, plus la raison de la fin précédente (`getHistoricalProcessExitReasons`, API 30 = minSdk : ANR, et trace native à partir de l'API 31).
  - **M** : une entrée « Envoyer le journal » dans les Réglages (`ACTION_SEND` par le FileProvider déjà déclaré). Règle et texte d'abord dans `docs/link-ui.md`, pour toutes les apps : aucune n'en a aujourd'hui.
- **Effort** : S (hook), M (journal et export). **Confiance** : vérifié.

## AND-02 — PyroWave : swapchain nulle après une recréation ratée (bug critique)

- **Où** : `jni/pyrowave/kn_pyrowave.cpp:988-992` ; `create_swapchain`, `:557-564`.
- **Description** :
  - Après la présentation, `OUT_OF_DATE` ou `SUBOPTIMAL` déclenche `create_swapchain(s)` sans lire son résultat.
  - Or `create_swapchain` détruit l'ancienne swapchain avant de tester `vkCreateSwapchainKHR` (`destroy_swapchain(s)` à la ligne 559, puis `return false`). `s->swapchain` reste donc à `VK_NULL_HANDLE`.
  - L'image suivante appelle `vkAcquireNextImageKHR(device, VK_NULL_HANDLE, …)` (`:869`) : comportement indéfini, en pratique un SIGSEGV dans le pilote. La branche `acquire` (`:870-874`), elle, teste bien ce résultat.
- **Scénario** : en PyroWave, on redimensionne une fenêtre libre, on la réduit ou on la passe sur un autre écran. La surface est en transition, la recréation échoue une fois, et l'app meurt à l'image suivante.
- **Correctif** : `if (!create_swapchain(s)) return KN_PW_LOST;` à la ligne 991. Au début de `frame_locked` : `if (s->swapchain == VK_NULL_HANDLE && !create_swapchain(s)) return KN_PW_LOST;`. Ensuite, `video.rs:304` refait la session à l'image suivante.
- **Effort** : S. **Confiance** : code vérifié, plantage plausible selon le pilote.

## AND-03 — PyroWave : `vkResetFences` sur une clôture encore en vol (bug)

- **Où** : `kn_pyrowave.cpp:981-987`.
- **Description** : `vkResetFences` est appelé même quand `vkWaitForFences` a expiré (1 s). La spécification Vulkan l'interdit tant que la soumission n'est pas finie, et certains pilotes plantent ou perdent le device.
- **Scénario** : un GPU qui bloque une seconde (chauffe, passage en arrière-plan, changement d'écran en mode bureau) fait planter le pilote à l'image suivante.
- **Correctif** : ne remettre la clôture à zéro qu'après une attente réussie ; sinon `vkDeviceWaitIdle`, remise à zéro, puis `KN_PW_LOST`.
- **Effort** : S. **Confiance** : vérifié.

## AND-04 — `surfaceDestroyed` rend la main pendant que le décodeur écrit encore (bug critique)

- **Où** : `session/SessionActivity.kt:688-690` (`sessionSetSurface(handle, null, 0)`) ; `jni/src/session.rs:82-87` (`set_window` : remplace la fenêtre et poste `Msg::Surface`) ; `jni/src/video.rs:236-245`. Même chose dans `PsSessionActivity.kt:320` et `jni/src/playstation.rs:323`.
- **Description** :
  - Android exige que plus personne ne touche la surface après le retour de `surfaceDestroyed`.
  - Ici, `Msg::Surface` passe derrière les images déjà en file. Le décodeur MediaCodec (et son fil `kybernow-affichage`) ou la swapchain Vulkan (`PwSession` garde l'`ANativeWindow`) continuent de rendre sur une BufferQueue abandonnée.
  - MediaCodec s'en tire en général avec une erreur. Vulkan, avec AND-02, peut planter.
- **Scénario** : en mode bureau, on réduit la fenêtre ou on change de bureau virtuel pendant une partie.
- **Correctif** : `set_window(None)` attend un accusé du fil image, envoyé après `self.decoder = None` ou `self.pw = None` (`sync_channel(0)` avec un délai court, environ 250 ms).
- **Effort** : M. **Confiance** : plausible.

## AND-05 — Perte de focus non gérée en mode bureau (bug critique)

- **Où** : `session/SessionActivity.kt`. Ni `onWindowFocusChanged`, ni `onPointerCaptureChanged`, ni `onTopResumedActivityChanged` (grep vide), alors que `PsSessionActivity.kt:620` gère le focus. Voir aussi `updateMouseMode`, `:1491-1501`.
- **Description** :
  - En fenêtres libres (Android 10 et plus), une fenêtre qui perd le focus reste « resumed ». `onPause` (`:694`), seul endroit qui relâche tout, n'est pas appelé.
  - Android retire la capture du pointeur dès la perte du focus. Mais `relative` reste à `true`, et `if (want == relative) return` empêche de la redemander.
  - Le cœur reste en souris relative alors que la souris arrive en absolu par `pointerMoveTo`, et le pointeur dessiné est caché.
- **Scénario** : en jeu avec la souris capturée, Alt+Tab vers une autre fenêtre, puis retour. La souris ne répond plus, Alt ou un clic restent tenus sur l'ordinateur. On croit à une session figée.
- **Correctif** :
  - `onWindowFocusChanged(false)` : `Native.inputReleaseAll`, `ui.pad.releaseAll()`, `haptics.stop()`.
  - `onPointerCaptureChanged(false)` : remettre `relative = false` et appeler `Native.inputSetRelative(handle, false)`.
  - Au retour du focus : `updateMouseMode()`. Reprendre la logique de `PsSessionActivity`.
- **Effort** : S. **Confiance** : vérifié.

## AND-06 — `configChanges` incomplets : session fermée par une recréation (bug)

- **Où** : `AndroidManifest.xml:76` (SessionActivity) et `:87` (PsSessionActivity). Liste actuelle : `orientation|screenSize|screenLayout|smallestScreenSize|keyboard|keyboardHidden|navigation|uiMode|density`.
- **Description** :
  - Manquent `colorMode`, `fontScale`, `locale|layoutDirection`, `touchscreen`, `fontWeightAdjustment` et `grammaticalGender`.
  - L'activité est alors recréée : `onDestroy` libère la session (`:738`) et `onCreate` en ouvre une neuve.
  - Pour un invité ou un emprunt, la remise n'est lisible qu'une fois (`takeGuest`/`takeLoan`) : l'activité fait `finish()` sans un mot (`:200`, `:209`).
- **Scénario** : en mode bureau, on fait glisser la fenêtre sur un moniteur HDR ou large gamme (`colorMode`), ou on branche ou débranche un écran tactile. La session s'arrête.
- **Correctif** : compléter les deux listes ; la taille est déjà suivie par l'écouteur de mise en page (`:313`). `MainActivity` (`:53`) gagnerait à recevoir les mêmes ajouts (voir AND-24).
- **Effort** : S. **Confiance** : plausible.

## AND-07 — Aucun WifiLock (bug)

- **Où** : aucune occurrence de `WifiLock`, `WIFI_MODE_FULL` ni `WAKE_LOCK` dans `apps/` ni `server/`. Le manifeste n'a pas la permission `WAKE_LOCK`.
- **Description** : en économie d'énergie, le Wi-Fi regroupe les réceptions : pics de latence, puis rafales de pertes. `FLAG_KEEP_SCREEN_ON` (`SessionActivity.kt:221`) garde l'écran allumé, pas la radio éveillée.
- **Scénario** : une tablette en Wi-Fi, peu d'entrées pendant un moment. Les pertes s'enchaînent, les images clés sont redemandées, et la veille « aucune image en 4 s » relance l'image, puis coupe la session.
- **Correctif** : ajouter la permission `WAKE_LOCK`. `WifiManager.createWifiLock(WIFI_MODE_FULL_LOW_LATENCY, "kybernow")` (API 29+, donc toujours disponible) avec `setReferenceCounted(false)`, pris dans `onResume` et rendu dans `onPause`/`onDestroy`, dans les deux activités de session.
- **Effort** : S. **Confiance** : vérifié (absence) ; l'effet sur les coupures est plausible.

## AND-08 — `AudioStream::drop` (ndk 0.9) panique si la fermeture échoue (bug)

- **Où** : `~/.cargo/registry/…/ndk-0.9.0/src/audio.rs:1413-1418` (`AudioError::from_result(status).unwrap()` dans `Drop`). Côté pont : `jni/src/audio.rs:74` (`AudioSink::drop`, garde du Mutex tenu), `:84` et `:121` (fil `kybernow-son`).
- **Description** : `AAudioStream_close` peut rendre une erreur quand la sortie vient de disparaître, et le drop panique alors.
  - Dans `AudioSink::drop`, appelé par le fil qui jette le flux (fil de session ou réseau de Kyber) : panique, Mutex empoisonné, fin `session.internal`, ou arrêt de l'app si cela se passe dans un export.
  - Dans `kybernow-son`, seul ce fil meurt, et le son se tait.
- **Scénario** : casque Bluetooth ou sortie HDMI coupés au moment où l'image est relancée.
- **Correctif** : ne jamais laisser l'`AudioStream` de ndk se fermer seul. `std::mem::forget` du flux, puis `ndk_sys::AAudioStream_close` en ignorant le code. Ou `catch_unwind` autour du drop. À signaler aussi en amont.
- **Effort** : S. **Confiance** : vérifié (code) ; fréquence inconnue.

## AND-09 — Mémoire libérée en cours d'usage : « Recevoir d'un autre appareil » (bug critique)

- **Où** : `ui/playstation/PsAdd.kt:491-505` ; `jni/src/playstation.rs:735-738` (`handoffStop` : `drop(Box::from_raw(h))`).
- **Description** :
  - La boucle appelle `PsNative.handoffNext(h, 1000)` sur un fil IO, bloqué jusqu'à 1 s.
  - `onDispose` fait `job.cancel()`, ce qui n'interrompt pas l'appel natif, puis libère aussitôt le `Receiver`.
  - `handoffNext` lit alors une mémoire libérée. En plus, `JSONObject(e)` n'est pas protégé, dans un `MainScope()` créé à la main.
- **Scénario** : en attente d'un envoi, on fait Retour ou « Nouveau code ». L'appel est presque toujours en cours : SIGSEGV probable.
- **Correctif** : un export d'arrêt séparé (`Receiver::stop`). Dans `onDispose`, appeler stop, attendre la fin de la boucle (`cancelAndJoin` dans un scope de l'app), puis libérer. Ou un `Arc` derrière la poignée.
- **Effort** : S. **Confiance** : vérifié.

## AND-10 — Mémoire libérée en fin de session : caméra, micro, réseau, minuterie (bug)

- **Où** : `jni/src/exports.rs:331` (`&*(h as *const Handle)`) et `:378-386` (`sessionFree`, sur le fil `kybernow-fin`). Côté Kotlin :
  - `session/CameraSender.kt:140-143` (`release()` = `quitSafely()` sans `join` ; `cameraFrame(handle(), …)` à la ligne 233) ;
  - `SessionActivity.kt:902-916` (le rappel réseau lit `handle` puis appelle `sessionSetLinkMbps`, sur le fil de ConnectivityManager) ;
  - `MicCapture.stop()` (`join(500)`) ;
  - `SessionActivity.kt:772` : `captureLastImage { Native.sessionStop(h) }`, rappelé jusqu'à 800 ms plus tard avec le `h` capturé. Si la session finit seule pendant ce temps, `finishSession`, puis `onDestroy`, puis la libération passent avant ce rappel.
- **Description** : la poignée est un pointeur nu, et seul le test « 0 » la protège. Un appel natif fait avec une poignée lue juste avant `handle = 0L` lit une mémoire libérée.
- **Scénario** : rare. La caméra ou le micro actifs au moment où l'on quitte, ou une session qui finit pendant la capture de la dernière image.
- **Correctif** :
  - côté Rust (le plus sûr) : un registre `Mutex<HashMap<u64, Arc<Handle>>>`, où la poignée devient un identifiant et `free` la retire ;
  - à défaut : joindre les fils caméra et micro avant `sessionFree`, relire `handle` dans le rappel de `captureLastImage`, et vider `main` (`removeCallbacksAndMessages(null)`) dans `onDestroy`.
- **Effort** : M. **Confiance** : vérifié par lecture ; ce n'est pas le plantage à 2-5 min.

## AND-11 — « Revenir » (emprunt) ferme la session au lieu de la rouvrir (bug)

- **Où** : `session/SessionActivity.kt:1453-1455` ; manifeste `:77` (`singleTask`). Pas d'`onNewIntent`.
- **Description** : `startActivity(SessionActivity)` depuis elle-même arrive en `onNewIntent`, qui n'est pas surchargé, puis `finishSession()` ferme l'activité. Rien ne s'ouvre, et le prêt reste en attente dans l'app.
- **Scénario** : un ami dont l'emprunt vient de finir touche « Revenir » : retour à l'accueil.
- **Correctif** : ouvrir depuis MainActivity (finir d'abord, puis lancer avec `FLAG_ACTIVITY_NEW_TASK`), ou un `onNewIntent` qui relance proprement. Point voisin : un lien `kybernow://` ouvert pendant une session ramène MainActivity, qui est dans la même tâche en `singleTask`.
- **Effort** : S. **Confiance** : vérifié.

## AND-12 — Fenêtre déplacée sur un autre écran : réglages d'affichage figés (bug)

- **Où** : `session/SessionActivity.kt:334-349` (`applyDisplay`), `:413-424` (`watchHdr(display.displayId)`), `:435-445` (`start`).
- **Description** : le mode d'affichage préféré, la fréquence et le suivi HDR ne sont calculés qu'au départ. Après un déplacement de fenêtre (pas de recréation, puisque `density` et `screenSize` sont déclarés), `preferredDisplayModeId` désigne un mode de l'ancien écran, et la fréquence comme le HDR restent faux.
- **Correctif** : un `DisplayListener` ou `onConfigurationChanged` qui compare `display.displayId`, puis refait `applyDisplay`, `watchHdr` et `sessionSetSurface(…, refresh)`.
- **Effort** : M. **Confiance** : plausible.

## AND-13 — Souris capturée : historique ignoré, fractions perdues (bug)

- **Où** : `session/SessionActivity.kt:1508` (`inputMouseMove(h, e.x.roundToInt(), e.y.roundToInt())`).
- **Description** : Android regroupe les déplacements bruts. `getHistoricalX/Y` est ignoré, et l'arrondi jette les fractions, si bien que les petits mouvements lents peuvent s'annuler. Le pointeur est lent et irrégulier avec une souris à 500-1000 Hz.
- **Correctif** : additionner l'historique, garder un reste flottant, et `requestUnbufferedDispatch(e)` sur la vue.
- **Effort** : S. **Confiance** : vérifié (code) ; ampleur de l'effet plausible.

## AND-14 — Barre ouverte : les clics et la molette passent aussi à l'ordinateur (bug)

- **Où** : `session/SessionActivity.kt:1637` (`anythingModal()` = `menuOpen || keysOpen || typing || shareOpen || qualityOpen || lendOpen || giveBackConfirm`) et `:1658`. À comparer avec `SessionOverlay.kt:267` (`covering`, qui ajoute `barOpen`, `keyboardOpen`, `statsOpen`, `watchScreenOpen`, `leaving`, `passageDone`).
- **Description** : `dispatchGenericMotionEvent` envoie les boutons et la molette de la souris tant qu'aucun des sept états n'est vrai. Barre ou statistiques ouvertes, un clic sur un bouton de la barre clique aussi sur l'ordinateur.
- **Correctif** : utiliser `ui.covering` partout (voir aussi AND-31). Penser aussi à relâcher la capture du pointeur quand quelque chose recouvre l'image.
- **Effort** : S. **Confiance** : vérifié.

## AND-15 — Sans surface : images clés redemandées sans fin (bug)

- **Où** : `jni/src/video.rs:332-342` (`frame`) et `:418` (`open` → `host.window()?`).
- **Description** : sans fenêtre (arrière-plan), chaque image non clé demande une image clé, au plus une fois par 300 ms, et chaque image clé échoue dans `open()`. La Station encode environ trois images clés par seconde pour rien, tant que l'app est cachée (débit plafonné à 2 Mbit/s).
- **Correctif** : sans fenêtre, jeter les images sans rien demander. `Msg::Surface` redemande déjà l'image clé au retour (`:240`).
- **Effort** : S. **Confiance** : vérifié.

## AND-16 — Cycle `Shared` ↔ chaîne d'entrées : l'activité peut fuir (bug)

- **Où** : `jni/src/session.rs:333-348` (`InputCallbacks(Arc<Shared>)` et `Board(Arc<Shared>)` dans la chaîne rangée dans `Shared.input`).
- **Description** : le cycle n'est cassé que par `on_stream_stopped`. Si le fil de session panique (fin `session.internal`), `Shared` et sa `GlobalRef` vers `SessionState` (donc l'activité) ne sont jamais libérés.
- **Correctif** : dans `sessionFree`, `h.shared.input.lock().unwrap().take()` (et `video_tx` à `None`).
- **Effort** : S. **Confiance** : vérifié.

## AND-17 — Annuler puis réemprunter : la nouvelle demande perd son état (bug)

- **Où** : `ui/AppModel.kt:620-641`. `Lend.borrow` est `NonCancellable` (`core/Lend.kt:463`).
- **Description** : le `finally` de l'ancien travail s'exécute jusqu'à 30 s après « Annuler » et fait `_borrowing -= id; borrowJobs.remove(id)`, ce qui efface la nouvelle demande du même ordinateur.
- **Correctif** : `finally { if (borrowJobs[id] === job) { … } }`.
- **Effort** : S. **Confiance** : vérifié.

## AND-18 — Console jointe hors de chez soi jamais refermée à l'annulation (bug)

- **Où** : `playstation/PlayStations.kt:228-251`, appelé depuis `MainActivity.kt:453-470` (`lifecycleScope`).
- **Description** : une annulation pendant `searchAt` ou `wake` (jusqu'à 30 s) ne passe pas par `remote.close()`. La poignée native fuit, et le signe de vie continue vers la Station.
- **Correctif** : `catch (e: CancellationException) { remote.close(); throw e }`, ou `try/finally`.
- **Effort** : S. **Confiance** : vérifié.

## AND-19 — Recherche des consoles active pendant les sessions (bug)

- **Où** : `ui/home/HomeScreen.kt:297-300` (`DisposableEffect(Unit) { ps.start(); onDispose { ps.stop() } }`).
- **Description** : la recherche est liée à la composition de l'accueil, qui survit à `onStop`. Pendant une session, l'app diffuse donc toutes les 3 s sur le Wi-Fi en tenant le verrou de multidiffusion, ce qui ajoute du trafic pendant le streaming.
- **Correctif** : `LifecycleStartEffect`, ou `ps.start/stop` dans `AppModel.startPolling/stopPolling`.
- **Effort** : S. **Confiance** : vérifié.

## AND-20 — La découverte garde les ordinateurs d'un réseau quitté (bug)

- **Où** : `model/Discovery.kt:64-73` (`stop()` ne vide pas `_found`) ; utilisé par `model/AccountStore.kt:260` (`lan()`).
- **Description** : de retour au premier plan sur un autre réseau, « Sur le réseau » montre encore le PC de la maison. `connectAccount` saute alors la vérification par le relais.
- **Correctif** : `_found.value = emptyList()` dans `stop()`, et ignorer les résolutions arrivées après l'arrêt.
- **Effort** : S. **Confiance** : plausible.

## AND-21 — Adresse MAC mal validée : réveil promis, rien n'est envoyé (bug)

- **Où** : `ui/settings/Settings.kt:714-715` (`filter { isLetterOrDigit() }` et une longueur de 12) ; `ui/AppModel.kt:814-821` (`wake` journalise un refus mais affiche « Réveil de l'ordinateur… »).
- **Description** : « ZZZZZZZZZZZZ » est accepté, le bouton « Réveiller » apparaît, puis le cœur refuse sans rien dire à l'utilisateur.
- **Correctif** : passer par `wake::parse_mac` du cœur (qui refuse aussi les adresses nulles ou de groupe), afficher l'erreur, et traiter `code != 0`.
- **Effort** : S. **Confiance** : vérifié.

## AND-22 — Une seule entrée abîmée efface toute la liste (bug)

- **Où** : `model/Hosts.kt:414-417` et `:430-433`, `playstation/PsStore.kt:98`.
- **Description** : `runCatching` entoure toute la boucle. Une entrée illisible fait rendre une liste vide, et le `save` suivant écrase les préférences.
- **Correctif** : `mapNotNull { runCatching { parse(it) }.getOrNull() }`.
- **Effort** : S. **Confiance** : plausible (corruption rare).

## AND-23 — `strings.py` a perdu les 11 textes de la caméra (bug d'outillage)

- **Où** : `apps/android/scripts/strings.py`. `grep -c camera_back` renvoie 0, alors que `res/values/strings.xml` et `values-fr` contiennent `camera`, `camera_starting`, `camera_turn_on`, `camera_turn_off`, `camera_indicator`, `camera_front`, `camera_back`, `camera_external`, `camera_resolution`, `camera_resolution_help` et `camera_stats`.
- **Description** : la prochaine régénération efface ces textes, et la compilation casse (`R.string.camera*` dans `CameraSender.kt`, `CameraUi.kt`, `SessionOverlay.kt`, `Settings.kt`). Origine probable : la fusion webcam vers micro.
- **Correctif** : remettre les 11 tuples FR et EN, puis ajouter un mode `--check` (générer à part, comparer) appelé par `scripts/release.sh`.
- **Effort** : S. **Confiance** : vérifié.

## AND-24 — Petits bugs (bug)

- **Bouton Retour de la souris** : Android en fabrique `KEYCODE_BACK`, qui ouvre ou ferme la barre (`SessionActivity.kt:1590-1597`) en plus du bouton envoyé à l'ordinateur. Correctif : ignorer `KEYCODE_BACK` venant de `SOURCE_MOUSE`. Effort S, plausible.
- **`MainActivity` recréée** (langue, `locale` non déclarée) : `handle(intent)` (`:105-107`) est rejoué sans tester `savedInstanceState`, et `route` n'est pas sauvegardée. Résultat : retour à l'accueil, ou réouverture de Rejoindre. Effort S, plausible.
- **Mise à jour de l'app** : `update/Updates.kt:117-131`, `getJSONObject`/`getString`/`getLong` hors de `runCatching`, dans un scope sans gestionnaire d'exceptions. Un champ absent fait planter l'app. Effort S, plausible (le JSON vient du cœur).
- **mDNS** : `model/MdnsLookup.kt:75,79` lit hors du paquet sur une réponse tronquée. Pas de plantage, mais la recherche s'arrête. Effort S, vérifié.
- **`start()` sans écran** : `SessionActivity.kt:435` (`Screen.of(display ?: return)`) n'appelle pas `finish()`, d'où un écran noir. Effort S, vérifié.

## AND-25 — Énumération des manettes à chaque événement d'un appareil inconnu (optimisation)

- **Où** : `session/Inputs.kt:115` (`PadRouter.slot` relance `Gamepads.refresh()`) et `:161` (`return true`).
- **Description** : chaque mouvement `SOURCE_JOYSTICK` d'un appareil sans place (souris qui se dit joystick, télécommande) refait une énumération des périphériques sur le fil principal et publie un `StateFlow`. En plus, l'événement est avalé.
- **Correctif** : une liste des appareils refusés (vidée par `InputDeviceListener`), et rendre `false`.
- **Effort** : S. **Confiance** : vérifié.

## AND-26 — Chemin chaud : pointeur, rappels JNI, allocations par événement (optimisation)

- **Forme du pointeur** : `session/SessionState.kt:529-533` crée un `Bitmap` à chaque changement de forme, sur le fil de kynput (en bureau Windows, à chaque survol d'un lien ou d'un texte). Correctif : un cache indexé par empreinte des pixels.
- **Rappels JNI** : `jni/src/jvm.rs` passe par `call_method`, soit `GetObjectClass` + `GetMethodID` à chaque rappel, y compris `onPointer` à la cadence de la souris (`session.rs:703-738`, qui recopie aussi les pixels en `Vec<i32>`). Correctif : garder les `JMethodID` dans `Listener`.
- **Allocations par événement** : `viewToHost` crée une `Pair` à chaque mouvement de souris (`SessionActivity.kt:1525`). `StylusInput` crée `PenSample`, `Triple` et lit `getMotionRange` à chaque échantillon (`Inputs.kt:571-587`). `PointerView.onDraw` crée un `RectF` (`PointerView.kt:65`). `OnScreenPadView` recrée ses `Path` et `mapOf` à chaque dessin (`:196-199`, `:245-283`).
- **Image** : `video.rs:82` fait `data.to_vec()` par paquet (inévitable avec le canal), et le canal `channel()` n'est pas borné. Un `sync_channel(≈120)` avec `try_send` est plus sûr si le décodeur cale.
- **Effort** : S. **Confiance** : vérifié.

## AND-27 — Manette en souris : minuterie à 125 Hz même au repos (optimisation)

- **Où** : `session/Inputs.kt:253-271` et `:276-282`.
- **Description** : dès le premier mouvement, `tick` se reprogramme toutes les 8 ms tant que `stop()` n'est pas appelé, même sticks au repos. Cela réveille 125 fois par seconde le fil principal (qui porte aussi les entrées et l'overlay) pendant toute la session.
- **Correctif** : arrêter la boucle quand les axes reviennent au repos et que `wheelAcc` vaut 0 ; `axes()` la relance.
- **Effort** : S. **Confiance** : vérifié.

## AND-28 — Keystore sur le fil principal et dans la composition (optimisation)

- **Où** : `ui/home/HomeScreen.kt:271` (`remember(…) { model.friendPassword(…) }`, un déchiffrement AES-GCM par ordinateur prêté à chaque sondage) ; `ui/AppModel.kt:466,476` (`store.password` deux fois par ordinateur toutes les 8 s) ; `model/Hosts.kt:550-561` (`AndroidKeyStore` rechargé à chaque appel).
- **Correctif** : garder la `SecretKey` et un cache des mots de passe en clair en mémoire, invalidé par `setPassword` et `remove`.
- **Effort** : S. **Confiance** : vérifié.

## AND-29 — Accueil recomposé en entier (optimisation)

- **Où** : `ui/home/HomeScreen.kt:245-305` (environ 35 `collectAsState` en tête, un `HomeState` fait de `Map` et `List` instables) et `:356-366` (`Backdrop` lit `animateFloat(...).value` pendant la composition, d'où une recomposition et un `ColorMatrix` neuf à chaque image).
- **Correctif** : lire les états dans les sous-composables, et utiliser `@Immutable` ou des collections immuables. Pour le fond : `Modifier.graphicsLayer { … }` et une matrice mémorisée.
- **Effort** : M (état), S (fond). **Confiance** : vérifié.

## AND-30 — Libellés du cœur ignorés, refaits en Kotlin (factorisation)

- **Où** :
  - `ui/home/LendHome.kt:76-84` (`availabilityLabel`) refait `LendAvailability::label` du cœur, alors que `Native.lendAvailabilityLabel` (`core/Native.kt:200`) n'est jamais appelé ;
  - `relayErrorText` (`:65`) et `relayPathLabel` (`:61`) sont exportés mais Kotlin garde ses propres `R.string.relay_*` et `route_*` (`HomeScreen.kt:655`, `SessionState.kt:107`, `SessionOverlay.kt:867`) ;
  - la validation MAC et deux tests d'IPv4 divergents (`model/Hosts.kt:292`, sans borne 0-255, et `ui/add/AddHostScreen.kt:277`) refont `wake::parse_mac` et les règles du cœur ;
  - trois gestions du verrou de multidiffusion (`Discovery.kt:49`, `PlayStations.kt:108`, `PsAdd.kt:486`).
- **Correctif** : une seule source par texte, celle du cœur (règle « mêmes textes partout »), ou retirer l'export. Un seul assistant pour le verrou de multidiffusion, avec compteur.
- **Effort** : S. **Confiance** : vérifié.

## AND-31 — Doublons (factorisation)

- **Deux listes de ce qui recouvre l'image** : `anythingModal()` (`SessionActivity.kt:1637`) et `SessionUi.covering` (`SessionOverlay.kt:267`). Leur divergence cause AND-14.
- **Deux activités de session** : `SessionActivity` et `PsSessionActivity` recopient `fitSurface`, les rappels de surface, la visibilité de l'overlay, `toggleMenu` et `finishSession`. Un socle commun est à extraire. Effort M.
- **Côté JNI** : `give` (`exports.rs:45`, `update.rs:22`) et `new_string` (`camera.rs:20`) font la même chose. `handle()` existe dans `exports.rs:331` et `playstation.rs:397`. `Host::set_window` (`playstation.rs:323`) recopie `Shared::set_window` (`session.rs:82`). À ranger dans `MediaHost`.
- **Effort** : S à M. **Confiance** : vérifié.

## AND-32 — Fonctions natives jamais appelées (code mort)

- **Constat** : déclarées `external`, exportées en Rust, aucun appel dans `apps/android/app/src` (grep `nom(` hors déclaration) :
  - dans `core/Native.kt` : `nowMicros`, `autotestCancel`, `relayPathLabel`, `relayErrorText`, `lendAvailabilityLabel`, `shareNormalizeCode`, `shareRoleWithPads`, `shareRoleWithKbm`, `shareQualityLabel`, `shareJoin`, `consoleShareWatch`, `cameraOffered`, `sessionSetMode`, `padSetComboEnabled` ;
  - dans `core/PsNative.kt` : `remoteState`, `padTouch`, `padTouchClick`.
  - `kn_pw_hdr_output` (`jni/pyrowave/kn_pyrowave.cpp:1166`, `.h:57`) n'a aucun appelant.
- **Ce qui manque à cause de ces fonctions** :
  - **`autotestCancel`** : « Annuler » du test Auto passe par `autotestFree` (`AppModel.kt:398`). Le chemin `{"cancelled": true}` n'est jamais pris.
  - **`padTouch` et `padTouchClick`** : le pavé tactile de la DualSense n'est jamais envoyé à la console, alors que `docs/playstation.md:31,48` le prévoit. La chaîne `ps_touchpad` est morte elle aussi.
- **Correctif** : pour les libellés, voir AND-30. Pour `autotestCancel`, l'appeler avant `autotestFree`. Pour le pavé tactile, décider : le brancher ou le retirer. Retirer le reste (Kotlin et Rust).
- **Effort** : S (M pour le pavé tactile). **Confiance** : vérifié.

## AND-33 — Symboles, chaînes, ressources et dépendances inutilisés (code mort)

Tous vérifiés par `git grep -w` sur le dépôt hors `third_party`.

- **Kotlin** :
  - `WakeTarget.listFromJson` (`core/Console.kt:141`) ;
  - `nothingToInstall` (`:205`) ;
  - `valueOrNull()` (`:228`) ;
  - `Gamepads.lastUsed` (`model/Gamepads.kt:76`), avec `_lastUsed` écrit à chaque entrée (`:128`) ;
  - `borrowedComputer` (`model/Hosts.kt:442`) ;
  - `PyroWave.setForTests` (`model/PyroWave.kt:37`), qu'aucun test n'appelle ;
  - `Modifier.readable()` (`ui/Theme.kt:288`).
- **Chaînes** :
  - `ps_step_wake` : l'étape « Réveil de la console » de link-ui (l. 944) n'est pas affichée, alors que l'app Linux l'a. C'est un écart à corriger, pas une suppression.
  - `ps_hdr_help` : l'aide de la puce HDR d'une console manque.
  - `ps_touchpad` : voir AND-32.
  - Aucune clé n'existe dans une seule langue.
- **Ressources** :
  - `res/drawable/kn_logo.xml` et `kn_mark.xml`, générés par `scripts/brand-drawables.py` mais jamais référencés ;
  - `colors.xml` : `frost` et `spectre` ;
  - `app/screenshots/tablette-accueil.png`, suivi par git mais cité nulle part : ajouter le dossier au `.gitignore`.
- **Gradle** (`app/build.gradle.kts`) :
  - `libs.tv.material` : aucun import `androidx.tv.material`, et le README dit lui-même qu'aucun écran ne l'emploie ;
  - `libs.androidx.lifecycle.viewmodel.compose` : `by viewModels()` vient d'activity ;
  - `libs.compose.ui.tooling.preview` : aucun `@Preview`.

  À retirer, avec leurs entrées dans `gradle/libs.versions.toml`, et la phrase du README l. 207.
- **Effort** : S. **Confiance** : vérifié (sans compilation).

## AND-34 — Documentation et cohérence (doc)

- **Écarts avec link-ui** :
  - `ps_step_wake` et `ps_hdr_help` (AND-33) ;
  - la rangée Android TV affiche « os · gpu » (`tv/TvChannel.kt:76`), contraire à l'esprit de « rien de technique sur l'accueil » ;
  - `AddHostScreen.kt:276-287` : un nom d'hôte ou une IPv6 sans port renvoie « Console injoignable. » sans aucune recherche.
- **README** :
  - `README.en.md:211` omet les tests d'invitation (`InviteTest`, `InviteUiTest`) que cite la version française, et `PyroWaveTest` n'est cité dans aucune langue ;
  - l'arborescence (`README.md:158-171`, idem en EN) ne cite ni welcome, account, lend, share, playstation, ni la caméra, le micro, l'enregistrement, ni plusieurs modules de `jni/src`.
- **Journal** :
  - `CameraSender.kt:58` journalise sous `KyberNowCamera`, invisible avec `adb logcat -s KyberNow` que conseille le README (l. 153 et 213) ;
  - le commentaire de `jni/src/jvm.rs:3-4` (« en démon : il n'a pas à s'en détacher ») est faux. jni-rs 0.21.1 détache bien à la sortie du fil, par un garde TLS (`vm.rs:461`), et c'est heureux : sans cela, ART arrêterait l'app à la fin de chaque fil de décodage.
- Aucun TODO, FIXME ou HACK dans `apps/android`.
- **Effort** : S. **Confiance** : vérifié.

---

## Mes 10 priorités

1. **AND-01** : hook de panique vers logcat, livré au testeur, avec la consigne `adb logcat -b crash -b main` ou la raison de la fin précédente. Ensuite, journal exportable pour toutes les apps.
2. **AND-02 et AND-03** : PyroWave, swapchain nulle et clôture remise à zéro trop tôt (quelques lignes de C++).
3. **AND-05** : perte du focus en mode bureau (capture, touches enfoncées).
4. **AND-06** : compléter `configChanges` dans les deux activités de session.
5. **AND-07** : WifiLock à faible latence.
6. **AND-04** : `surfaceDestroyed` synchrone (MediaCodec et Vulkan).
7. **AND-09** : mémoire libérée dans « Recevoir d'un autre appareil ».
8. **AND-08** : fermeture AAudio qui panique.
9. **AND-23** : rendre la caméra à `strings.py`, et ajouter `--check` à la release.
10. **AND-10 puis AND-14** : poignée de session sûre entre les fils, puis une seule liste « recouvre l'image ».

## Ce qui est sain

- **`Listener::with`** (`jni/src/jvm.rs`) : un cadre de 16 références locales par rappel, puis `exception_check`, `describe` et `clear`. Pas d'accumulation de références, pas d'exception laissée en attente.
- **Références JNI** : aucun `FindClass` depuis un fil natif, aucun `JNIEnv` ni `jobject` gardé, seulement des `GlobalRef`. Les fils natifs sont détachés à leur sortie par jni-rs.
- **Correspondance Kotlin ↔ Rust** : les fonctions `external` et les exports correspondent un pour un, rappels compris.
- **Cargo** : pas de `panic = "abort"`. Le fil de session est protégé par `catch_unwind` dans le cœur, et les rappels AAudio par `abort_on_panic` de ndk.
- **MediaCodec** : tampon d'entrée vérifié avant la copie, tampon rendu au codec en cas de dépassement, fil de sortie joint avant `stop()`, décodeur refait sur erreur, seule la plus récente image affichée.
- **PyroWave** : bornes de `PwConfig::parse` vérifiées, délais finis sur `acquire` et sur la clôture.
- **Horodatages** : `i64` en µs partout, pas de troncature.
- **Pointeur** : `PointerLayer` envoie à kynput et rappelle hors de son verrou depuis 0.11.7. Le pointeur passe à la vue par des `StateFlow`, sans Compose.
- **Écouteurs** : Gamepads (compté et désenregistré), presse-papiers, HDR, réseau et multidiffusion sont tous rendus. JmDNS est fermé sur son fil.
- **Réseau et singletons** : tout le réseau de `ConsoleClient` est en `Dispatchers.IO`. Aucun `GlobalScope`, aucune activité retenue par un singleton.
- **Rappels des entrées** : `PadRouter`, `PadMouse`, `TouchInput` et `StylusInput` relisent `handle()` à chaque appel.
- **Compose** : toutes les listes paresseuses ont des clés, et l'overlay de session passe en `GONE` hors interface.
- **Langues** : FR et EN alignées clé pour clé.
