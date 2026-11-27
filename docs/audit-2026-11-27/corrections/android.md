# Corrections de l'audit du 27/11 — app Android

Branche `claude/fix-android-b192bc`. Rapport d'origine : [`../android.md`](../android.md). Tout est dans `apps/android` (Kotlin, pont JNI `jni/src`, PyroWave C++ `jni/pyrowave`, `scripts`), sauf ce fichier.

## Ce que le testeur doit faire pour nous envoyer son journal

Le testeur aux plantages en mode bureau (2 à 5 min) n'a besoin ni d'`adb` ni d'un ordinateur :

1. Installer la version qui contient ces corrections (la prochaine bêta).
2. Jouer comme d'habitude jusqu'au plantage (ou à la session coupée, ou figée).
3. **Rouvrir l'app** : c'est au lancement suivant qu'Android lui dit pourquoi la précédente s'est arrêtée (plantage, plantage natif, « ne répond pas », manque de mémoire).
4. **Réglages de l'app › À propos › Envoyer le journal**, puis choisir l'e-mail ou la messagerie, et nous l'envoyer.
5. Dire aussi la version de l'app (affichée juste au-dessus, « KyberNow Link 0.11.x ») et s'il a vu « L'application ne répond pas ».

Le fichier `kybernow-journal.txt` contient : l'appareil et la version d'Android, les fins précédentes (avec la trace d'un ANR, et les textes lisibles du rapport de plantage natif à partir d'Android 12), les exceptions Kotlin, le journal natif (`natif.log`, paniques Rust avec fil, emplacement et pile) et les 8000 dernières lignes de logcat de l'app. Pour lire un relevé : voir « Pour trancher dès qu'on a un relevé » dans le rapport d'audit (`session.internal`, SIGSEGV dans `libvulkan`, panique dans un export, ANR).

Avec `adb` : `adb logcat -s KyberNow` suffit aussi (la caméra y écrit désormais).

## Constats corrigés

| Id | Correctif |
|---|---|
| AND-01 | Crochet de panique Rust (`jni/src/journal.rs`) vers logcat et vers `files/journal/natif.log` (copie tournante de tout le journal natif, 2 × 1 Mo) ; exceptions Kotlin dans `java.log` ; raison de la fin précédente (`ApplicationExitInfo`, trace d'ANR, textes du tombstone) dans `fins.log` et logcat au lancement ; « Envoyer le journal » dans Réglages › À propos (feuille de partage, FileProvider). |
| AND-02 | PyroWave : swapchain refaite (ou session perdue) avant `vkAcquireNextImageKHR` si elle est nulle ; résultat de la recréation après la présentation lu. |
| AND-03 | PyroWave : clôture remise à zéro seulement après une attente réussie ; sinon `vkDeviceWaitIdle`, remise à zéro, session perdue. |
| AND-04 | `surfaceDestroyed` attend (250 ms au plus) que le fil de l'image ait lâché le décodeur ou la swapchain (`SurfaceGate`, numéro par changement de surface lu avant chaque message, même pour les consoles). |
| AND-05 | Perte du focus (mode bureau) : touches, clics, manette à l'écran, toucher et stylet relâchés ; capture du pointeur oubliée (`onPointerCaptureChanged`) puis redemandée au retour du focus. Console PlayStation : capture oubliée aussi. |
| AND-06 | `configChanges` complets dans les deux activités de session (`colorMode`, `fontScale`, `locale`, `layoutDirection`, `touchscreen`, `fontWeightAdjustment`, `grammaticalGender`, `mcc`, `mnc`). |
| AND-07 | `WifiLock` `WIFI_MODE_FULL_LOW_LATENCY` (`session/LowLatencyWifi.kt`) pris au premier plan et rendu en arrière-plan et à la fin, dans les deux activités ; permission `WAKE_LOCK`. |
| AND-08 | Flux AAudio fermé par un enrobage qui rattrape la panique du `Drop` de ndk 0.9, hors du verrou. |
| AND-09 | « Recevoir d'un autre appareil » : la poignée n'est libérée qu'après le retour de `handoffNext`, et l'attente suivante (« Nouveau code ») ne commence qu'après la fin de la précédente (`playstation/HandoffWait.kt`) ; JSON illisible ignoré. |
| AND-10 | Poignée de session : un numéro dans un registre Rust (`Arc<Handle>`), plus un pointeur. Un appel après `sessionFree` (caméra, micro, réseau, capture de la dernière image) ne trouve plus rien. |
| AND-11 | « Revenir » (emprunt) : `onNewIntent` refait l'activité avec le nouveau prêt, au lieu de la fermer. |
| AND-13 | Souris capturée : historique additionné, fractions gardées (`RelativeMotion`), `requestUnbufferedDispatch`. |
| AND-14 | Souris (boutons, molette, survol) arrêtée dès que quelque chose recouvre l'image (`ui.covering`) ; un bouton relâché pendant l'ouverture part quand même. |
| AND-15 | Sans surface, plus d'images clés demandées ; la surface revenue la redemande. |
| AND-16 | `sessionFree` casse le cycle `Shared` ↔ chaîne d'entrées. |
| AND-17 | Le `finally` d'une demande d'emprunt annulée n'efface plus la nouvelle demande du même ordinateur. |
| AND-18 | Console jointe hors de chez soi rendue si l'on annule pendant la recherche ou le réveil. |
| AND-19 | Recherche des consoles liée au premier plan (`LifecycleStartEffect`), plus pendant les sessions. |
| AND-20 | La découverte vide « Sur le réseau » à l'arrêt et ignore les résolutions arrivées après. |
| AND-21 | Adresse MAC lue par le cœur (`Native.wakeParseMac`, `wake::parse_mac`), refus affiché ; réveil sans paquet parti : on réessaie tout de suite au lieu de 2 min de « Réveil… ». |
| AND-22 | Listes enregistrées (ordinateurs, prêtés, consoles) lues entrée par entrée (`model/JsonLists.kt`). |
| AND-23 = TRANS-26 | Les 11 textes de la caméra rendus à `strings.py` ; `strings.py --check`, appelé par `release-apk.sh --check` (donc par `scripts/release.sh`). |
| AND-24 | Bouton Retour de la souris sans ouvrir la barre ; `MainActivity` recréée ne rejoue plus son lien ; mise à jour au JSON incomplet sans plantage ; mDNS tronqué sans exception ; session sans écran revient à l'accueil. |
| AND-12 | Fenêtre passée sur un autre écran (`DisplayListener`, `onConfigurationChanged`) : mode préféré, fréquence, HDR (valeurs données au cœur), suivi du HDR et surface redonnée avec sa fréquence, pour le nouvel écran. |
| AND-24 (suite) | Écran de l'accueil gardé quand `MainActivity` est recréée (`Route.saved`/`restore`, sans les liens d'invitation ni de partage). |
| AND-25 | Un appareil sans place n'est relu qu'une fois par changement d'appareils (`Gamepads.slotOf`, commun aux sessions et aux parties) ; son mouvement n'est plus avalé. |
| AND-26 | Identifiants des méthodes Kotlin gardés (`Listener::method`, `call_void`) ; pixels du pointeur sans copie de plus ; formes du pointeur en cache (`ShapeCache`, 16) ; souris sans `Pair`, cadre du pointeur et listes de la manette à l'écran réutilisés, inclinaison du stylet lue une fois par appareil ; file d'images bornée à 120 (au-delà : trou, image clé). |
| AND-27 | Manette en souris : la boucle à 125 Hz s'arrête sticks au repos, relancée au mouvement suivant. |
| AND-28 | Clé d'AndroidKeyStore et mots de passe déchiffrés gardés en mémoire (oubliés au retrait ou au changement). |
| AND-29 | `HomeState` et `PsHomeState` immuables (comparés par contenu) ; zoom du fond lu au dessin (`graphicsLayer`), filtre gris mémorisé. |
| AND-31 | Registre commun (`jni/src/registry.rs`) pour sessions, parties PlayStation, consoles jointes et attentes d'envoi (plus aucun pointeur nu rendu à Kotlin pour ces objets) ; `SurfaceSlot` partagé par les deux hôtes ; `give` en un seul endroit ; `SessionSurface` (cadrage de l'image, fréquence) pour les deux activités ; liste « recouvre l'image » unique pour la souris (AND-14). |
| AND-32 | 11 fonctions natives jamais appelées retirées (Kotlin et Rust : `nowMicros`, `autotestCancel`, `shareNormalizeCode`, `shareRoleWithPads`, `shareRoleWithKbm`, `shareQualityLabel`, `shareJoin`, `consoleShareWatch`, `cameraOffered`, `sessionSetMode`, `padSetComboEnabled`, et `remoteState`), et `kn_pw_hdr_output` ; pavé tactile de la DualSense branché vers la console (`PadTouchpad` : `padTouch`, `padTouchClick`). |
| AND-33 | Symboles Kotlin morts retirés (`listFromJson`, `nothingToInstall`, `valueOrNull`, `lastUsed`/`noteUsed`, `borrowedComputer`, `setForTests`, `readable`) ; `ps_touchpad` ; `kn_logo.xml`, `kn_mark.xml` (et leur production dans `brand-drawables.py`) ; couleurs `frost`, `spectre` ; capture suivie et `app/.gitignore` ; dépendances `tv-material`, `lifecycle-viewmodel-compose`, `ui-tooling-preview`. `ps_step_wake` et `ps_hdr_help` sont affichés (AND-34). |
| AND-34 | Étape « Réveil de la console » quand la console vient d'être réveillée ; aide de la puce HDR d'une console ; rangée d'Android TV sans système ni carte graphique ; un nom d'hôte est résolu pour chercher une console, et une IPv6 ou un nom sans console dit l'ordinateur injoignable ; une seule règle IPv4 (`isIpv4`) ; README (arborescence, essais, invitations en anglais) ; étiquette `KyberNow` de la caméra ; commentaire de `jvm.rs`. |
| Hors audit | `InviteUiTest` : vérifie ce que le champ affiche (`EditableText`), pas le texte que Compose donne à l'accessibilité. |
| TRANS-36 (Android) | Adresse d'Admin écrite par le cœur (`Native.adminUrl` → `console::admin_url`) : crochets IPv6 compris, sans ouvrir de connexion. |

## Constats laissés, et pourquoi

- **AND-30** (libellés du cœur refaits en Kotlin, `lendAvailabilityLabel`, `relayErrorText`, `relayPathLabel`) : demande de choisir la source unique avec le cœur (vague 2) ; les trois exports sont gardés pour cela.
- **Verrou de multidiffusion à compteur** (AND-30) : avec AND-30.
- **« Envoyer le journal » dans `docs/link-ui.md`** : ajouté à la fusion par la session coordinatrice. Texte : « Envoyer le journal » / “Send the log”, aide « Si l'app se ferme ou qu'une session se coupe, ce fichier texte raconte ce qui s'est passé. Joignez-le à votre message : vous choisissez à qui l'envoyer. », pendant la préparation « Préparation du journal… ».
- **Réveil d'une console hors de chez soi** : l'étape « Réveil de la console » ne s'affiche que pour une console réveillée sur le réseau ; `reach` (par une Station) ne dit pas s'il a réveillé la console.
- **TRANS-36 côté Mac et Apple TV** : autres zones.

## Ce qui est vérifié, et comment

- **Essais JVM** : `./gradlew :app:testDebugUnitTest`, 455 essais, tous réussis, dont les nouveaux `JournalTest` (5), `HandoffWaitTest` (4), `JsonListsTest` et `Ipv4Test` (3), `RelativeMotionTest` (3), `ShapeCacheTest`, `RouteTest` (2) et `MdnsLookupTest.truncatedResponsesNeverThrow`. `InviteUiTest`, qui échouait déjà sur `main` (a76ec972 : Compose donne le texte tapé à l'accessibilité, `InputText`), vérifie maintenant ce que le champ affiche.
- **Essais Rust du pont** : `cargo test -p kybernow-android --features kyber-pyrowave --lib` dans l'arbre `android` (17 essais, dont `journal` : dates, rotation, panique écrite dans le fichier ; `gate` : attente, délai, numéros ; `registry` : numéros libérés, appel en cours).
- **Compilation** : `build-native.sh` complet dans mon arbre (`third_party/arbres-fix-android/android`, commit 8edae17c, le dernier avec du code) : `libkybernow.so` des trois cibles et `libkybernow_pyrowave.so` (arm64-v8a, x86_64), sans erreur ni avertissement du pont ; exports nouveaux présents (`Native_init`, `Native_adminUrl`, `Native_wakeParseMac`), exports retirés absents ; `./gradlew :app:assembleDebug` réussi (APK de débogage de 132 Mo, les trois architectures).
- **`strings.py --check`** : à jour après régénération ; refusait bien l'ancien `strings.xml`.
- **Rien sur appareil** (aucun téléphone branché). À essayer sur appareil :
  - « Envoyer le journal » (feuille de partage, fichier lisible, logcat présent, fin précédente après un plantage provoqué) ;
  - mode bureau : Alt+Tab souris capturée, retour, aucune touche tenue ; fenêtre réduite ou redimensionnée en PyroWave ;
  - Wi-Fi faible latence (pertes, `dumpsys wifi | grep -i lock`) ;
  - « Revenir » après la fin d'un emprunt ;
  - « Recevoir d'un autre appareil » : Retour et « Nouveau code » pendant l'attente ;
  - souris capturée à 1000 Hz (mouvements lents) ;
  - adresse MAC invalide refusée ;
  - mode bureau : fenêtre de session glissée sur un autre écran (fréquence, HDR) ;
  - pavé tactile de la DualSense pendant une partie PlayStation (Android le donne-t-il comme `SOURCE_TOUCHPAD` ?) ;
  - « Réveil de la console » dans les étapes après « Réveiller et jouer » ;
  - pointeur en bureau Windows (formes en cache), manette en souris au repos, accueil fluide.

## Notes de version

FR :
- Si l'app se ferme ou qu'une session se coupe, vous pouvez maintenant nous envoyer son journal : Réglages de l'app › À propos › Envoyer le journal.
- En mode bureau, quitter la fenêtre puis y revenir ne laisse plus de touche ni de clic enfoncés sur l'ordinateur, et la souris reprend comme avant.
- Le Wi-Fi reste en faible latence pendant une session : moins de saccades et de coupures sur une tablette en Wi-Fi.
- PyroWave ne fait plus fermer l'app quand vous redimensionnez ou réduisez la fenêtre.
- La souris capturée suit mieux les petits mouvements lents.
- Un clic sur la barre de session ne clique plus aussi sur l'ordinateur.
- « Revenir », après la fin d'un emprunt, rouvre bien la session.
- Une adresse MAC mal tapée est signalée au lieu d'être gardée.
- Corrections de plusieurs plantages rares (fin de session, son, « Recevoir d'un autre appareil »).
- En mode bureau, glisser la fenêtre de session sur un autre écran règle la fréquence et le HDR pour lui.
- Pendant une partie PlayStation, le pavé tactile de la DualSense est envoyé à la console.
- L'accueil, le pointeur et la manette en souris demandent moins au téléphone.

EN:
- If the app closes or a session cuts out, you can now send us its log: App settings › About › Send the log.
- In desktop mode, leaving the window and coming back no longer leaves keys or clicks held down on the computer, and the mouse works as before.
- Wi-Fi stays in low-latency mode during a session: fewer stutters and drop-outs on a tablet on Wi-Fi.
- PyroWave no longer closes the app when you resize or minimise the window.
- The captured mouse follows small, slow movements better.
- A click on the session bar no longer also clicks on the computer.
- “Come back”, after a borrowing ends, now reopens the session.
- A mistyped MAC address is flagged instead of being kept.
- Several rare crashes fixed (end of a session, sound, “Receive from another device”).
- In desktop mode, dragging the session window to another screen sets the refresh rate and HDR for it.
- During a PlayStation game, the DualSense touchpad is sent to the console.
- The home screen, the pointer and the controller-as-mouse ask less of your phone.
