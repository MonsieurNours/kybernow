# Phase 2b de l'audit du 27/11 : apps Apple

Zone : `apps/macos`, `apps/apple`, `apps/ios`, `apps/tvos` et leur doc (`docs/coeur-apple*.md`, README des apps, `docs/kyber-changes*.md` pour KYB-49). Plan : [../migration-coeur-2b.md](../migration-coeur-2b.md) (section Apple) et [../vague-2.md](../vague-2.md) ; règles d'interface lues sur la branche des docs (`claude/v2-docs-0db24c`, `docs/link-ui.md`, `docs/relais.md`). Branche `claude/v2b-apple-fdde19`. Le cœur (`apps/core`) n'a pas été touché : ce qui y manque est dit plus bas.

## Fait

### Le pont sur le cœur (`apps/apple`)

| Id | Ce qui change |
|---|---|
| MAC-23 | Les 326 fonctions `kn_*` exportées passent par `kybernow_core::ffi_guard!` : une panique est écrite au journal et rend la valeur de repli de la fonction (`KN_ERROR` pour les codes, nul pour les pointeurs, le défaut du type ; `KN_CAMERA_OFF`, -1 pour un compte de manettes ou un rôle inconnus, `KN_SHARE_UNSUPPORTED`, mode de relais 0). `KnHdrLuminance` a son `Default` (les valeurs du cœur). |
| Migration | Nouveau `src/rules.rs` : `kn_short_fingerprint` et `kn_fingerprint_same` (`console::short_fingerprint`, quatre groupes), `kn_parse_address` (IPv6 entre crochets, port exigé ou par défaut), `kn_admin_url`, `kn_values_json` (`plan::values_json` plus `device_background_bitrate` et `device_manual_mbps` de l'appareil), `kn_plan_json`, `kn_settings_bitrate` et `kn_settings_bitrate_for_pixel_rate`, `kn_clamp_manual_mbps` (500 / 2 500 en PyroWave), `kn_pyrowave_mbps`, `kn_pyrowave_lowered_text`, `kn_looks_like_invite`, `kn_session_set_background` (`Session::set_background` avec le `Device` de la cible : `Computer` sur Mac, `AppleMobile`, `AppleTv`), `kn_session_pyrowave_failed`, et les fonctions HDR de l'essai croisé. Retirés : `kn_session_set_bitrate_cap` et `kn_session_set_hidden`. |
| Migration | `src/micro.rs` : `KnMicUi` sur `micro::MicUi` (puce, tuile « Envoi… », « Envoyé », « Coupé », raison, refus) ; les apps n'ont plus leurs règles ni leurs textes du micro. |
| Migration | `on_pyrowave_fallback(userdata, codec, text)` ajouté à la fin de `KnPlatform` (`Platform::on_pyrowave_fallback`) : l'avis vient une fois par session, quand le décodeur PyroWave ne s'ouvre pas sur l'appareil (`kn_session_pyrowave_failed`). |
| KYB-29 | `kn_pyrowave_set_header(2)` au lancement (`kyclient::set_pyrowave_header`) ; `kn_pyrowave_config` lit les en-têtes `KNPW` 1 et 2 et rend, en version 2, l'écran de mastering et les niveaux de lumière au format des SEI HEVC de CoreVideo (`mastering_sei`, `light_sei`) ; `kn_pyrowave_config_from_sequence_header` (paquet perdu), `kn_pyrowave_is_config` (lu à chaque paquet, sans JSON). |
| link-ui 2a | `src/journal.rs` : dépôt d'un journal (gzip, 2 Mio au plus en gardant la fin, AES-256-GCM sans données associées, `KNJ1`, `POST /v1/journal`, adresse du QR code avec la clé après `#k=`, `state`, `DELETE`) ; codes `KN_JOURNAL_*`. Essai sur les vecteurs `KNJ1` de `docs/relais.md`. |
| KYB-49 | `apps/apple/tests/pyrowave/run.sh` prend l'équipe dans `scripts/identite.sh` ; commentaire de `share.rs:46` à jour ; `docs/kyber-changes(.en).md` : le correctif PyroWave est posé par défaut, `--sans-pyrowave` l'écarte. Les fonctions C mortes de 0001 étaient déjà retirées (v2-kyber), vérifié. |

### Les apps sur ces règles

- **Copies retirées** (`KyberKit/CoreRules.swift`, une seule façade Swift) : empreinte courte et comparaison (accueil, ajout, réglages, partage, iOS : plus de ligne à six groupes), analyse d'adresse et adresse d'Admin (plus de crochets doublés en IPv6 ; `HomeAdmin` retiré), profils, PyroWave, bornes du débit à la main (`bitrateRange(codec)`), débit de session (`CoreBitrate.session`), délais du relais et des avis, nombre d'invités, exemple de relais, reconnaissance d'un lien d'invitation. `PyroWaveRate`, `LinkFormat.fingerprint`, `sameFingerprint`, `minBitrate`/`maxBitrate`/`referencePixelRate` n'existent plus.
- **Arrière-plan** (TRANS-40) : Mac (fenêtre occultée), iPhone et iPad (sauf sur un écran externe), et maintenant l'Apple TV (scène inactive, manettes captées par le système) passent par `setBackground`.
- **Micro** : `KyberKit/MicUI.swift` (Mac, iPhone, iPad) sur `KnMicUi`.
- **PyroWave** : `PyroWaveConfig.swift` (lu par le cœur, attachements MDCV et CLL sur les images HDR, décodeur gardé quand seules les métadonnées changent) ; repli en HEVC une fois si le GPU, le décodeur ou la mémoire manquent.
- **Session commune iPhone, iPad, Apple TV** (IOS-21) : `apps/tvos/KyberNowTV/SessionModel.swift`, classe de base des deux `SessionController` (étapes, avis, partage, prêt, enregistrement, bandeaux PyroWave, fin, démonstrations) ; les deux apps n'y ajoutent que leurs crochets. Le Mac garde son contrôleur (fenêtres, plusieurs ordinateurs) et prend les mêmes règles.
- **Hôte visiteur** (IOS-22) : `Host.visitor(guest:)`, `(watch:)`, `(borrow:)` dans `Model.swift`, une fois pour les trois apps (la souris de l'emprunt ne diverge plus).
- **Fichiers communs** (IOS-23, 24) : `ThemeTokens`, `HostImagery`, `AudioRoute`, `AppPreferences` (iOS et tvOS) ; `LinkFormat.ms/mbps/percent` à la place de `StatsFormat` et de `PyroWaveText.percent`.
- **Parties sur console** (IOS-25) : `KyberKit/PlayStationPart.swift` (mesures et arrêt) ; `setUpscale` filtre par `Upscaling.available` partout.
- **Code mort** (IOS-26, TVOS-22, IOS-27) : fonctions, vues, propriétés et icônes sans appel retirées ; un seul script d'icônes, `apps/macos/scripts/phosphor-icons.mjs <mac|ios|tvos>`, qui ne retire plus d'icônes utilisées (TVOS-20) ; QR codes de l'Apple TV par `Share.qrImage` (TVOS-21).
- **HDR** : `KyberKit/HDRShader.swift`, les fonctions du shader Metal (PQ, courbe vers le SDR de la Station, matrice BT.2020 → BT.709) écrites une fois, avec les constantes du cœur ; `apps/macos/tests/hdr` les compare au cœur.

### Règles d'interface (phase 2a, `docs/link-ui.md`)

- **Envoyer le journal** : Mac (feuille de partage `NSSharingServicePicker`), iPhone et iPad (feuille de partage), Apple TV (QR code, dépôt chiffré, étapes et refus de link-ui, « Terminé » retire le dépôt). En-tête : « Appareil : », système, langue, « Journal écrit le » (lus par la page du téléphone), fins précédentes par MetricKit (Mac, iPhone, iPad).
- **Clavier dans les Réglages de l'app** : iPhone et iPad, section « Clavier » avec « Disposition du clavier » (Auto, AZERTY, QWERTY, QWERTZ ; Auto suit le clavier branché quand iPadOS le dit) et « Clavier Apple ISO », retiré des réglages de l'ordinateur ; Apple TV, « Clavier Apple ISO » dans les Réglages de l'app.
- **Touches** : iPhone et iPad, les dix entrées dans l'ordre de link-ui avec leur indication (la barre du Mac n'a pas ce menu : link-ui le réserve aux apps Windows et Linux, le clavier du Mac envoie tout).
- **Menu « Plus » de la barre du Mac** : commandes passées une à une dans « Plus » quand la barre étroite ne tient pas, dans l'ordre de link-ui.
- **Prêt** : avertissement selon le système de la Station (Windows, Mac, autre) dans les trois apps.
- **Apple TV** (TRANS-39) : réglage « Définition de la caméra », « Scannez ce code avec votre téléphone pour ouvrir la page du compte. » sous le QR code de « Gérer le compte », Définition « Comme cet écran » et « Sur mesure » (avec ses définitions).
- **TRANS-37** : déjà corrigé dans le code (la raison du HDR seulement au focus), vérifié. Capsule « ● Caméra » et textes de la caméra (refus venus du cœur) : déjà conformes sur les trois apps.

### Traductions (TRANS-29, 31, 32, IOS-27)

- Chaînes nouvelles ajoutées aux trois catalogues avec l'anglais de link-ui (journal, clavier, Touches et indications, « Plus », avertissements du prêt, « relay.example.org:9443 » en anglais).
- Clés mortes retirées : « Faire jouer », « Faire regarder », « Vous pouvez jouer. », « Vos ordinateurs, sur cet iPad / cet iPhone / ce téléviseur », textes du micro venus du cœur, « Définition ramenée à … » (venu du cœur), anciennes aides et les quatre erreurs de KyberKit qui restaient ; les textes des démonstrations (`#if DEBUG`) gardés. `check-strings.py --strict` passe.
- Libellés alignés : Apple TV « Revenir à l'accueil », « Vibration d'essai » ; anglais « Username or password refused. », « Controller as mouse », « Remote as mouse », une seule traduction de « Ce n'est pas un lien KyberNow. » (“This isn't a KyberNow link.”), orthographe britannique (“recognises”).
- Espace fine insécable (U+202F) avant `: ; ! ?` dans toutes les valeurs françaises des catalogues du Mac, d'iPhone/iPad et de l'Apple TV (les clés, qui sont le texte du code, gardent l'espace simple).

## Laissé, et pourquoi

- **Exceptions à écrire dans `docs/link-ui.md`** (hors de ma zone) :
  - Apple TV, section Clavier : seulement « Clavier Apple ISO » ; la disposition du clavier (nommée pour iPad et iPhone) n'y sert pas : les parties sur console ne prennent pas le clavier sur l'Apple TV.
  - Apple TV, Définition : pas d'« Au plus 4K » (la TV ne dépasse pas la 4K) ; « Sur mesure » propose des définitions toutes faites (pas de champ de saisie à la télécommande).
  - « Décodage matériel » (fait de l'appareil, Réglages de l'app) : le glossaire de `docs/i18n.md` dit « décodage par la carte graphique », faux sur une puce Apple ; gardé, à écrire comme exception ou à changer dans le glossaire.
  - Journal : l'en-tête est en français dans toutes les langues (comme Android) ; « Trop d'envois » couvre aussi `503 full` du relais.
- **Cœur gelé** (à faire dans `apps/core`, puis le pont les reprend) :
  - le dépôt chiffré du journal est écrit dans le pont Apple (`apps/apple/src/journal.rs`) : à déplacer dans le cœur pour Android, Linux et LG (la session v2b-android a le sien en JNI) ;
  - `pyrowave::Config::parse` ne demande pas 48 octets en version 2 et ne lit pas le mastering ni les niveaux de lumière : le pont le fait ; à mettre dans le cœur (format dans `docs/kyber-changes.md`) ;
  - le rendu EDR (au-delà du SDR de la Station, `kn_pq_to_edr`) n'a pas d'équivalent au cœur : propre aux apps Apple, pas d'essai croisé ;
  - le plafond de 80 Mbit/s en quittant PyroWave reste une constante Swift (`hevcMbps`) : le cœur n'en a pas.
- **Barre du Mac (« Plus »)** : pas vue à l'écran, l'app de l'auteur était ouverte (rien n'a été remplacé) ; le calcul de ce qui tient est fait par `ViewThatFits`.
- **TVOS-23** (test d'interface du menu de session) et le reste de **TVOS-24** (`micro.md`, SIDELOAD) : hors de cette tâche.

## Vérifié, et comment

Tout dans mon arbre (`KYBERNOW_ARBRES=…/third_party/arbres-v2b-apple`, `scripts/arbres.sh preparer apple`, au commit essayé), avec `KYBERNOW_IGNORE_RUNNING=1` : l'app de l'auteur et sa Station Mac sont restées ouvertes, rien de leurs fichiers n'a été remplacé.

- **Construction complète, comme la publication sans signature** : `build-kyclient.sh`, `apps/apple/scripts/build-apple.sh`, `build-kyber-tvos.sh`, `build-apple-mobile.sh tvos`, `apps/core/scripts/build-apple.sh --target aarch64-apple-ios` et `aarch64-apple-ios-sim`, `build-apple-mobile.sh ios` : tout passe. `xcodebuild` Debug de l'app Mac, de l'Apple TV (simulateur) et de l'iPhone/iPad (simulateur) : tout compile. Chaque étape de la migration a aussi été compilée pour les trois apps au fil des commits.
- **`cargo test -p kybernow-apple --features kyber-pyrowave`** : 22 essais passent, dont `vecteur_d_essai_knj1` (le dépôt est celui de `docs/relais.md` octet pour octet, clé du QR code comprise), le journal trop long (en-tête et fin gardés), les règles (empreinte, adresses IPv6, adresse d'Admin, bornes du débit, valeurs par appareil), le micro de l'interface et l'en-tête `KNPW` 2.
- **`apps/macos/tests`** : `rules` (règles du cœur vues de Swift), `hdr` (shader Metal contre le cœur : PQ à 5,83e-5 près en relatif, courbe vers le SDR à 4,52e-8, matrice identique, 2 001 points), `pyrowave` (en-têtes 1 et 2, « écran de mastering sur l'image » en HDR, rien en SDR ; PSNR Y 57,9 dB ; image tronquée sautée), `video` (HEVC, H.264, AV1, reprise), `autotest` (les six extraits du cœur) : tout passe ; `screen` et `fullscreen` compilés (`KYBERNOW_COMPILE_ONLY=1`, essais d'affichage à l'écran pas lancés).
- **`apps/ios/tests`** : `screen-pad` (3 095 vérifications) et `session-rules` passent.
- **XCTest de l'Apple TV** (simulateur Apple TV 4K créé pour l'essai, tvOS 27.0) : 6 sur 7 passent, dont `testSoundPanelBackClosesOnlyThePanel` (son texte cherché avec l'espace fine, a9ab5c54). `testInvitationCodeAddsTheComputer` échoue comme sur le code d'origine (déjà noté à la vague 1, `corrections/ios-tvos.md`) : `typeText` n'arrive pas dans le champ du mot de passe au simulateur.
- **Traductions** : `apps/macos/scripts/check-strings.py --strict` : catalogues complets, aucune entrée périmée. Les chaînes mortes viennent d'une extraction par Xcode (`-exportLocalizations`) pour chaque app, relue à la main (l'extraction se fait en Release : les textes des démonstrations, sous `#if DEBUG`, ont été gardés).
- **Pas essayé** (pas d'appareil ni de Station en session ici) : l'arrière-plan face à une Station (débit ramené), le repli PyroWave sur un appareil sans GPU utilisable, les métadonnées HDR10 de PyroWave sur un vrai écran HDR, « Envoyer le journal » face au relais (dépôt, QR code, ouverture sur un téléphone ; le relais qui prend les dépôts est sur la branche v2b-relais), la feuille de partage du Mac et d'iOS, le menu « Plus » de la barre du Mac à l'écran, la disposition du clavier avec un clavier branché à un iPad.

## Notes de version

**Français**

- Apple TV : « Envoyer le journal » (Réglages de l'app › À propos) : le journal de l'app est chiffré sur l'Apple TV et s'ouvre sur votre téléphone par un QR code. Mac, iPhone et iPad : le même bouton donne le fichier à la feuille de partage.
- iPhone et iPad : nouvelle section « Clavier » dans les Réglages de l'app, avec la disposition du clavier pour les parties sur console.
- iPhone et iPad : le menu Touches propose les dix combinaisons de Windows, avec ce que fait chacune.
- Mac : dans une fenêtre étroite, les commandes de la barre qui ne tiennent plus passent dans un menu « Plus ».
- Apple TV : définition de la caméra, « Comme cet écran » et « Sur mesure » pour la définition, et le débit baisse quand l'app passe en arrière-plan.
- PyroWave : les images HDR portent les informations de luminosité de l'ordinateur ; si l'appareil ne peut pas décoder PyroWave, la session passe en HEVC et vous le dit.
- Textes plus soignés en français et en anglais.

**English**

- Apple TV: “Send the log” (App settings › About): the app's log is encrypted on the Apple TV and opens on your phone through a QR code. Mac, iPhone and iPad: the same button hands the file to the share sheet.
- iPhone and iPad: a new “Keyboard” section in App settings, with the keyboard layout for console games.
- iPhone and iPad: the Keys menu offers Windows' ten key combinations, with what each one does.
- Mac: in a narrow window, the bar's controls that no longer fit move into a “More” menu.
- Apple TV: camera resolution, “Match this display” and “Custom” for the resolution, and the bitrate goes down when the app goes to the background.
- PyroWave: HDR pictures carry the computer's brightness information; if the device can't decode PyroWave, the session switches to HEVC and tells you.
- Polished French and English wording.
