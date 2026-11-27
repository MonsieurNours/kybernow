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
| AND-34 (en partie) | Étiquette `KyberNow` pour la caméra ; commentaire faux de `jvm.rs`. |
| TRANS-36 (Android) | Adresse d'Admin écrite par le cœur (`Native.adminUrl` → `console::admin_url`) : crochets IPv6 compris, sans ouvrir de connexion. |

## Constats laissés, et pourquoi

- **AND-12** (fenêtre passée sur un autre écran : mode, fréquence et HDR figés) : effort M, demande un `DisplayListener` et des essais sur deux écrans ; pas dans les priorités.
- **AND-24, `route` de `MainActivity` non sauvegardée** : un retour à l'accueil après une recréation, sans perte ; à faire avec AND-31.
- **AND-25 à AND-29** (optimisations), **AND-30, AND-31** (factorisation), **AND-32, AND-33** (code mort), **AND-34** (README, link-ui) : vague 2.
- **Poignée des parties PlayStation** (`jni/src/playstation.rs`, `handle()` en pointeur nu) : même risque qu'AND-10, moins exposé (pas de caméra ni de micro) ; même registre à poser en vague 2 (AND-31).
- **« Envoyer le journal » dans `docs/link-ui.md`** : fichier commun, non touché. Texte proposé pour la rubrique « À propos » de toutes les apps : « Envoyer le journal » / « Send the log », aide « Si l'app se ferme ou qu'une session se coupe, ce fichier texte raconte ce qui s'est passé. Joignez-le à votre message : vous choisissez à qui l'envoyer. », bouton pendant la préparation : « Préparation du journal… ». Aucune autre app ne l'a encore.
- **TRANS-36 côté Mac et Apple TV** : autres zones.
- **`scripts/release.sh`** : non modifié ; il appelle déjà `release-apk.sh --check`, qui vérifie maintenant les textes.

## Ce qui est vérifié, et comment

- **Essais JVM** : `./gradlew :app:testDebugUnitTest`, 451 essais, dont les nouveaux `JournalTest` (5), `HandoffWaitTest` (4), `JsonListsTest` (2), `RelativeMotionTest` (3) et `MdnsLookupTest.truncatedResponsesNeverThrow`. Un seul échec, antérieur : `InviteUiTest` (le champ du mot de passe expose son texte dans la sémantique de Compose, `InputText`) échoue de la même façon sur `main` à a76ec972, sans ces corrections ; à regarder à part (test ou masquage du champ).
- **Essais Rust du pont** : `cargo test -p kybernow-android --features kyber-pyrowave --lib` dans l'arbre `android` (16 essais, dont `journal` : dates, rotation, panique écrite dans le fichier ; `gate` : attente, délai, numéros).
- **Compilation** : `build-native.sh --check` pour aarch64-linux-android, puis bibliothèque native des trois cibles et APK de débogage dans mon arbre (`third_party/arbres-fix-android/android`), voir ci-dessous.
- **`strings.py --check`** : à jour après régénération ; refusait bien l'ancien `strings.xml`.
- **Rien sur appareil** (aucun téléphone branché). À essayer sur appareil :
  - « Envoyer le journal » (feuille de partage, fichier lisible, logcat présent, fin précédente après un plantage provoqué) ;
  - mode bureau : Alt+Tab souris capturée, retour, aucune touche tenue ; fenêtre réduite ou redimensionnée en PyroWave ;
  - Wi-Fi faible latence (pertes, `dumpsys wifi | grep -i lock`) ;
  - « Revenir » après la fin d'un emprunt ;
  - « Recevoir d'un autre appareil » : Retour et « Nouveau code » pendant l'attente ;
  - souris capturée à 1000 Hz (mouvements lents) ;
  - adresse MAC invalide refusée.

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
