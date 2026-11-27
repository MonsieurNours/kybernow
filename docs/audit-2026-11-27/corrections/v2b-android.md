# Vague 2 (phase 2b) de l'audit du 27/11 : app Android

Branche `claude/v2b-android-75c5bd`, partie de `main` (`d0d5e315`, fusion de la phase 2a). Zone : `apps/android` (Kotlin, pont JNI `jni/src`, PyroWave C++ `jni/pyrowave`, scripts, `strings.py`), sauf ce fichier. Guide suivi : [`../migration-coeur-2b.md`](../migration-coeur-2b.md), section Android et table « À faire dans tous les ponts » ; règles : `docs/link-ui.md` et `docs/relais.md` de la branche `claude/v2-docs-0db24c`.

## Fait

| Sujet | Correctif |
|---|---|
| `ffi::guard` | Les 197 fonctions `extern "system"` du pont (12 fichiers) dans `kybernow_core::ffi_guard!`. Valeur rendue après une panique choisie là où 0 voudrait dire « réussi » : réveil (1), annulations du partage et du prêt (1), scancode (−1), image de la caméra (`OFF`), plafond de l'encodeur (celui gardé). Le `catch_unwind` d'`audio.rs` reste : il entoure le `Drop` d'un flux AAudio, pas une fonction exportée. |
| Une seule source (AND-30, TRANS-34, 40 à 42) | `core/Core.kt` : la seule entrée vers les règles et textes du cœur (empreinte, adresse, valeurs communes, libellés du prêt, des erreurs et du chemin du relais, lien d'invitation, temps de l'enregistrement, profils, réglages synchronisés, accueil, découverte, « Définition ramenée »). Sans réponse du cœur : une valeur neutre, au journal, jamais une copie. Les essais JVM posent `FakeCore` (réponses écrites d'avance) ; les règles restent essayées par le cœur. |
| Empreinte (TRANS-34) | Quatre groupes du cœur (`Native.shortFingerprint`, `7C41 E09B AABB CCDD`) dans l'ajout, la vérification et les réglages ; comparaisons par `fingerprintSame` (accueil, invitation, « Recevoir d'un autre appareil »). |
| Adresse | `parse_address` du cœur dans « Par adresse » (hôte, `hôte:port`, `[IPv6]:port`, IPv6 nue ; port d'Admin par défaut, le champ « Port » l'emporte) ; `isIpv4` retiré (le cœur ne garde l'adresse du réveil que si c'est une IPv4 ; une IPv4 écrite se lit sans requête au DNS pour chercher une console). Adresse d'Admin : `Native.adminUrl` (vague 1) ; l'ancienne exportation `consoleAdminUrl` est retirée. |
| Arrière-plan (TRANS-40) | `sessionSetBackground(handle, background)` → `Session::set_background(…, Device::Android)` : 2 Mbit/s et image figée, touches relâchées. |
| Débit à la main (TRANS-41) | Paliers jusqu'à 300 Mbit/s, puis en PyroWave 500 à 2 500 (`manual_mbps` de `values_json`) ; `coerceIn(5, 4294)` et `1..4294` retirés ; la dernière session garde le débit dit par le cœur (`Second::target`) au lieu de recalculer la formule des profils. |
| Constantes (TRANS-42) | Délais de la mise à jour des Stations (`NOTICE_WAIT_MS`, `BEFORE_NOTICE`), relais public (`Accounts.DEFAULT_RELAY`), port d'Admin : `values_json`. `relayLabel` : `rules::relay::label`. Copies Kotlin retirées : `Welcome.Fallback`, `SyncedKeys.profileBase`/`known`/`applyStream`/`full` (repli « sans bibliothèque native »), `RecordingState.clock`, `Lend.isInviteLink`, `relayRefusalText`/`relayText`/`routeLabel`/`availabilityLabel` (texte d'Android), `SessionMode.bitrate`. |
| Micro (`MicUi`) | Le pont tient le `micro::MicUi` du cœur ; `onMicrophone(json, notice)` donne son `ui_json` (libellé, infobulle, sous-titre « Envoyé » / « Coupé » / « Envoi… », raison) plus `shown`, et l'avis « Micro envoyé à {nom}. » une fois, après un allumage par l'utilisateur (pas pour le choix retenu au départ, `microphone` des paramètres). Kotlin n'écrit plus aucun de ces textes (7 clés retirées). |
| PyroWave | Paquet `KNPW` lu par `pyrowave::Config` du cœur (`PwConfig` retiré ; contrôles des octets 5 à 7 et de la parité) ; sans paquet, la définition de l'en-tête de séquence (`from_sequence_header`) puis celle de la session. Décodeur ou swapchain qui ne s'ouvre pas : `Session::pyrowave_failed`, une fois par flux ; `onPyroWaveFallback(format, texte)` montre l'avis du cœur une fois, et PyroWave n'est plus proposé jusqu'au prochain lancement (la sonde le redira). « Définition ramenée » des Statistiques : `pyrowave::Lowered::text`. |
| KYB-29 | En-tête `KNPW` version 2 lu (au moins 48 octets si la version vaut 2, octets en plus ignorés, version 1 toujours lue) : mastering display (primaires, blanc, luminances) et MaxCLL/MaxFALL, chacun selon son drapeau. `kn_pw_configure` prend ces métadonnées (`kn_pw_content_hdr`, nul : aucune) et les donne à la swapchain HDR10 (`VkHdrMetadataEXT`), champ par champ, celles de l'écran de l'appareil sinon ; des métadonnées seules changées sont redonnées sans rien recréer. |
| Découverte | L'annonce mDNS (résolue par NsdManager, JmDNS ou le nom) lue par `discovery::found` du cœur : adresse choisie, nom, empreinte sous sa forme normale, `kyber=0` ignoré. |
| « Recevoir d'un autre appareil » | Paquet lu et rapproché par `playstation::bundle` (formes tolérées, identifiants PSN vérifiés, `match_accounts`, `BundleStation::matches`) dans `jni/src/bundle.rs` ; Kotlin range le résultat. Le paquet envoyé passe par `Bundle::to_json`. |
| Verrou de multidiffusion | `model/Multicast.kt` : un seul verrou, tenu tant qu'une recherche en a besoin (ordinateurs, consoles, envoi entre appareils). |
| Enregistrement | Les onze textes de `ui_json` lus (infobulles avec Ctrl + Alt + Maj + R, nom pour les lecteurs d'écran « Enregistrer la session » / « Arrêter l'enregistrement (02:14) ») ; temps écrit par le cœur. |
| « Envoyer le journal » | Téléphone et tablette : feuille de partage, textes de link-ui (déjà alignés). Android TV : QR code, journal compressé puis chiffré sur l'appareil (KNJ1, AES-256-GCM, clé seulement après `#k=`), déposé sur le relais du compte de l'appareil, sinon le relais public (`POST /v1/journal`), suivi toutes les 2 s, « Terminé » retire un dépôt pas encore ouvert, relais épinglé vérifié par l'empreinte du certificat ; étapes et refus de link-ui (`ui/settings/JournalQr.kt`, `model/JournalUpload.kt`). L'adresse du relais est lue par le cœur (`relayParts`). |
| Caméra (link-ui 2a) | Départ refusé par la session : « La caméra n'a pas pu démarrer sur l'ordinateur. » ; « Caméra… » sans réponse en 10 s : « L'ordinateur ne répond pas : la caméra est éteinte. », caméra éteinte ; les refus de l'ordinateur restent ceux du cœur (`camera::event_notice`). |
| Menu « Touches » | Les dix entrées dans l'ordre de link-ui (« Touche ⊞ seule » compris), chacune avec son indication. |
| Raccourcis | Ctrl + Alt + Maj + C (caméra), D (se déconnecter), R (enregistrer), S (statistiques), en plus de M, Q et V. |
| TRANS-38 | Panneau « Statistiques » complet (`session/StatsPanel.kt`) : titre, raccourci, croix « Fermer les statistiques » ; latence en grand « de la capture à votre écran » ; trajet d'une image (six étapes, lieux « Sur l'ordinateur », « Réseau », « Sur cet appareil » ou « Sur ce téléviseur », « — » sans mesure, cascade sur une échelle commune avec le pointillé d'une image, aide à l'appui long ; au téléviseur, par lieu avec les totaux) ; « Latence » et « Débit » sur 30 s avec « 1 image », « Débit visé » ; « Pertes · 30 s » (images, paquets, images clés) et « Session : n images, n paquets » ; « Chemin », « Réseau », « Image », « Mesures », « Son » ; avant la première image, la phrase de link-ui. |
| TRANS-39 | « Ce lien de partage est illisible. » pour un `kybernow://join` que le cœur ne lit pas ; « Le partage n'est pas disponible pour le moment : réessayez dans un instant. » pour un 503 d'Admin (choisi par le code) ; avertissement du prêt selon le système de la Station (Windows, Mac, sinon « votre session »). |
| TRANS-29 | 22 clés mortes retirées de `strings.py` (chemin et erreurs du relais, états du prêt, micro, `session_sound_detail`, `camera_stats`, `pyrowave_lowered`, lignes de statistiques en minuscules). Aucune clé morte : vérifié par un relevé des `R.string` du code. |
| TRANS-31 | « Revenir à l'accueil » (au lieu de « Retour à l'accueil »), « Avec un code » (au lieu de « Par un code »), “recognises”. |
| TRANS-32 | Le catalogue d'Android a déjà l'espace fine (imposée par `strings.py`) ; les textes venus du cœur n'en ont pas (voir « Laissé »). |
| Script | `build-native.sh` : l'aide dit PyroWave allumé par défaut, `--sans-pyrowave` pour s'en passer, `--kyber-pyrowave` gardé sans effet ; même chose dans `jni/Cargo.toml`. |
| Docs | `apps/android/README.md` et `README.en.md` : « Règles et textes du cœur », micro, journal, compilation, état des essais. |

## Laissé, et pourquoi

- **En-tête `KNPW` v2 jamais demandé** : la Station n'envoie la version 2 qu'à une app qui demande `pyrowave_header: 2` (`StartRequestVideo`, `set_pyrowave_header` de kyclient), et le cœur ne le demande pas (aucune API, cœur figé pour la 2b). L'app Android lit la v2 dès qu'elle arrivera. À faire dans le cœur : demander l'en-tête 2 quand l'app sait le lire (option de `SessionParams` ou des capacités), pour toutes les apps de la 2b.
- **Espace fine des textes du cœur (TRANS-32)** : le catalogue du cœur a des espaces simples avant `: ; ! ?` ; Android les montre tels quels (avant, ses propres copies avaient l'espace fine). À corriger dans le cœur (`i18n::tr` en français, ou son catalogue), pas dans chaque app.
- **Textes absents du cœur** : « La caméra n'a pas pu démarrer sur l'ordinateur. », « L'ordinateur ne répond pas : la caméra est éteinte. » (dans le catalogue de KyberKit sur Apple) et « Le partage n'est pas disponible pour le moment… » (client web seulement) sont dans `strings.py` ; le cœur devrait les porter (`camera`, `share`), comme le reste.
- **Système de la Station** : le cœur n'a que `station_is_mac` ; l'avertissement du prêt reconnaît Windows au début de `os` (« Windows… »). Une règle `station_system(os)` dans le cœur servirait toutes les apps.
- **`AccountSync`** (table du guide : « tenu par le pont ») : `SettingsSync` garde encore en Kotlin sa mémoire des envois (le plus récent, relais muet renvoyé) ; la section Android du guide ne le demande pas, et le faire tenir par le pont demande un objet par compte avec sa persistance. À reprendre avec LINAPP-40 / WINAPP-55 comme modèle. `SyncedKeys.changed` reste aussi (il traduit les réglages d'Android, pas ceux du cœur).
- **`plan_json`** : le mode et le format de la session sont encore choisis en Kotlin (`SessionMode.pick`, `codec`) ; la section Android du guide ne le demande pas.
- **`frames::FrameFlow`** dans `jni/src/video.rs` : la machine n'a pas été refaite (le guide : « si la machine y est refaite »).
- **Barre au toucher** : l'infobulle du profil (« Jeu, Bureau, Max ou Auto »), le menu de la souris et le menu « Plus » sont ceux des barres au pointeur ; la barre d'Android montre chaque profil et bascule la souris d'un toucher. Les raccourcis M (menu) et Q (quitter) d'Android diffèrent de link-ui (M souris, D se déconnecter) : gardés, D ajouté.
- **Libellés** : « Vibration d'essai » (Android, Linux, LG) contre « Tester la vibration » (tvOS) et l'exemple `relais.exemple.org:9443` en anglais : link-ui ne tranche pas le premier et garde le second tel quel.
- **Journal par QR code** (choix de la copie, à relire) : un relais plein (503) dit « Trop d'envois : réessayez dans une heure. » (link-ui n'a pas de texte pour ce cas) ; un 413 dit le texte du relais muet (impossible : l'app reste sous 2 Mio) ; `natif.log` et logcat ne sont pas filtrés (ni mot de passe ni jeton n'y sont écrits par l'app).

## Vérifié, et comment

- **Essais JVM** : `./gradlew :app:testDebugUnitTest` (sans `libkybernow.so`, `-x checkNativeLibrary`) : 465 essais, tous réussis, dont les nouveaux `JournalUploadTest` (7), `JournalQrUiTest` (5), `StationSystemTest`, `MulticastTest`, `JoinRefusalTest`, `WelcomeTest` réécrit (mémoire de l'accueil sur un faux cœur) et `MicTest` (JSON de `MicUi`) ; `RelayUiTest` sur le nouveau panneau des statistiques.
- **Essais Rust du pont** : `cargo test -p kybernow-android --features kyber-pyrowave --lib` dans mon arbre (rootfs `arm64-macos` pour `libvlc.pc`) : 20 essais réussis, dont `version_1_lue_par_le_coeur`, `version_2_et_ses_metadonnees` (drapeaux, unités, octets en plus, v2 trop courte refusée), `repli_sans_paquet` et `paquet_rapproche_de_l_appareil`.
- **Compilation** : `build-native.sh --check --target aarch64-linux-android` après chaque partie du pont, sans avertissement du pont ; `build-native.sh` complet (trois cibles, `libkybernow_pyrowave.so` pour arm64-v8a et x86_64 avec le nouveau `kn_pw_configure`) puis `./gradlew :app:assembleDebug` dans mon arbre (`third_party/arbres-v2b-android/android`, commit `c13c8e6c`, le dernier avec du code) : APK de débogage de 193 Mo, trois architectures ; 208 fonctions JNI exportées par chaque `libkybernow.so` (les nouvelles présentes, `consoleAdminUrl` absente), aucun avertissement du pont (armeabi-v7a compris).
- **`strings.py --check`** : à jour (836 textes).
- **Rien sur appareil** (aucun appareil branché, ni Station). À essayer sur appareil :
  - Statistiques : panneau complet au téléphone (cascade, aide à l'appui long) et au téléviseur (par lieu) ;
  - micro : « Envoi… » puis « Envoyé », avis « Micro envoyé à {nom}. » une seule fois, raison d'un refus en avis au toucher, rien chez un invité que l'hôte ne laisse pas parler ;
  - PyroWave qui ne s'ouvre pas (appareil sans Vulkan 1.3 qui passe la sonde, ou surface refusée) : l'image repart en HEVC, avis une fois ;
  - PyroWave HDR face à une Station qui envoie l'en-tête v2 (quand le cœur le demandera) : métadonnées dans `dumpsys SurfaceFlinger` ;
  - arrière-plan : débit à 2 Mbit/s (Statistiques « fenêtre cachée ») et retour ;
  - « Par adresse » : `192.168.1.20` seul (port d'Admin par défaut), `[fe80::…]:9443`, une console par son IPv4 ;
  - découverte : ordinateurs du réseau avec leur nom et leur empreinte ;
  - Android TV : « Envoyer le journal » par QR code jusqu'au téléphone, contre un relais qui a les routes `/v1/journal` ;
  - « Recevoir d'un autre appareil » depuis un Mac (comptes, associations, Stations reliées) ;
  - caméra sans réponse de l'ordinateur (10 s) ; menu « Touches » ; raccourcis C, D, R, S ; lien `kybernow://join` abîmé ; prêt face à une Station Mac et Linux (avertissement).

## Notes de version

**FR**
- Statistiques complètes pendant la session : le trajet d'une image étape par étape, la latence et le débit des 30 dernières secondes, les pertes, le chemin et le son.
- Sur Android TV, « Envoyer le journal » affiche un QR code : scannez-le avec votre téléphone pour enregistrer ou partager le journal, chiffré sur le téléviseur et illisible par le relais.
- L'empreinte de l'ordinateur s'affiche en entier (quatre groupes), comme dans Admin.
- Si PyroWave ne peut pas démarrer sur l'appareil, l'image repart aussitôt dans un autre format, et un avis le dit.
- Avec PyroWave, le débit réglé à la main va jusqu'à 2 500 Mbit/s.
- Le menu « Touches » a dix raccourcis, chacun avec son usage, et le clavier a les raccourcis Ctrl + Alt + Maj + C, D, R et S.
- L'adresse d'un ordinateur se tape avec ou sans son port, en IPv6 aussi.
- La caméra s'éteint d'elle-même, avec un avis, si l'ordinateur ne répond pas.
- Un lien de partage abîmé est signalé, et un partage momentanément indisponible le dit.

**EN**
- Full statistics during a session: the path of a frame step by step, latency and bitrate over the last 30 seconds, losses, the path and the sound.
- On Android TV, “Send the log” shows a QR code: scan it with your phone to save or share the log, encrypted on the TV and unreadable by the relay.
- The computer's fingerprint is shown in full (four groups), as in Admin.
- If PyroWave can't start on the device, the picture switches to another format at once, with a notice.
- With PyroWave, the bitrate set by hand goes up to 2,500 Mbit/s.
- The “Keys” menu has ten shortcuts, each with what it does, and the keyboard has the Ctrl + Alt + Shift + C, D, R and S shortcuts.
- A computer's address can be typed with or without its port, IPv6 included.
- The camera turns itself off, with a notice, if the computer doesn't answer.
- A damaged share link is flagged, and a share that is briefly unavailable says so.
