# Migration des apps vers le cœur (phase 2b)

Ce que la phase 2a du cœur (branche `claude/v2-coeur-8452cb`, compte rendu [`corrections/v2-coeur.md`](corrections/v2-coeur.md)) livre, et ce que chaque app remplace par quoi. Les numéros de ligne sont ceux du 27/11 : ils bougent, cherchez le nom. Règle de la vague : une app ne garde plus de copie d'une règle que le cœur porte ; ce qui ne peut pas l'appeler (un shader, une interface en TypeScript ou en Kotlin) le reçoit par le pont ou se vérifie par un essai croisé.

Détail des API : [`apps/core/README.md`](../../apps/core/README.md), section « Règles communes des apps ».

## Ce que le cœur offre

| Besoin | API | Pour les ponts |
|---|---|---|
| Règles pures, sans dépendance | caisse `kybernow-rules` (`apps/core/rules`), aussi `kybernow_core::rules` | Linux et LG peuvent en dépendre **sans** le cœur (interfaces des captures) |
| Panique dans un pont | `kybernow_core::ffi_guard!({ … })`, `ffi_guard!(défaut => { … })`, `ffi_guard!(null => { … })` ; `ffi::guard`, `guard_default`, `guard_null` | une ligne par fonction `extern "C"` / `extern "system"` |
| Empreinte d'Admin : affichage | `console::short_fingerprint(fp)` → `7C41 E09B AABB CCDD` (vide sans chiffre) | `rules::fingerprint::short` |
| Empreinte : « la même » | `console::same_fingerprint(a, b)`, `same_fingerprint_opt` (préfixe, deux-points, espaces, casse ignorés ; vide jamais) | `rules::fingerprint::same` |
| Adresse tapée | `console::parse_address(texte, Some(ADMIN_PORT))` ou `None` pour exiger le port | IPv6 nue = hôte entier |
| Adresse d'Admin | `console::admin_url(hôte, port)` (crochets une seule fois), `ADMIN_PORT` (9443), `ADDRESS_EXAMPLE` (`192.168.1.20:9443`) | |
| Fenêtre cachée, app en arrière-plan | `Session::set_background(bool, Device)` (plafond `background_bitrate(device)` : 20 Mbit/s Mac et PC, 8 iPhone, iPad et Apple TV, 2 Android et LG ; et image figée) | remplace la paire `set_bitrate_cap` + `set_hidden` |
| Bornes du débit à la main | `profile::manual_mbps(Device, codec)` : 5 à 500 en HEVC, AV1, H.264, 2 500 en PyroWave, 95 sur LG, pas de 5 | `values_json` → `manual_mbps` |
| Valeurs communes | `plan::values_json(caps)` : profils (60, 90, 300 ; +1/3 sans 4:4:4 ; +1/2 en H.264), `pyrowave_mbps`, bornes, `background_bitrate` (par appareil), `max_guests`, `max_sessions`, `relay_link_wait_secs`, `station_notice_wait_secs`, `station_notice_before`, `admin_port`, `address_example`, `default_relay` | un appel au lancement |
| Plan d'une session | `plan::plan(&Choices, caps, écran)`, `plan::plan_json(choix, caps, écran)` (réglages, mode, débit de la session, nom « Jeu · HEVC ») | remplace les calculs de profil des interfaces |
| PyroWave : débit d'un profil | `profile::pyrowave_mbps(l, h, fréquence, 4:4:4)` | |
| PyroWave ne s'ouvre pas | `Session::pyrowave_failed()` (repli de l'appareil pour la session, image relancée) | une fois, après avoir retenu l'échec |
| Paquet `KNPW` | `pyrowave::Config::parse`, `is_config`, `from_sequence_header`, `fallback`, `chroma_size`, `label`, `bytes` (v1 et v2) | |
| Flux d'images | `frames::FrameFlow` → `Step` (`Open`, `Configure`, `Decode { head, first }`, `Skip`, `Lost`, `Keyframe`, `Recovered`) ; `fed`, `failed`, `unavailable`, `reset`, `idle` | apps qui décodent en Rust |
| « Définition ramenée » | `pyrowave::Lowered { width, height, refresh, mbps }.text(lang)` | les nombres de `Second::lowered` / `KnSecond` |
| Enregistrement | `RecordingState::ui_json` (seule source des textes), `recording::clock` / `rules::format::recording_clock` | |
| Code de partage ou de prêt | `share::format_code`, `normalize_code` (`rules::format`) | |
| Lien d'invitation | `lend::looks_like_invite` (forme), `lend::parse_invite` (lecture) : forme sans lecture → « Ce lien d'invitation est illisible. » | |
| Relais | `rules::relay::{normalize, label, DEFAULT_RELAY}` (les mêmes que le tunnel et `account::relay_label`) | |
| Stations sur le réseau | `discovery::found(nom complet, port, adresses, txt)` → `Found` ; `discovery::SERVICE` | l'app garde sa bibliothèque mDNS |
| « Envoyer à un appareil » | `playstation::bundle::{Bundle::parse, to_json, match_accounts, BundleStation::matches, valid_psn_id, new_account_id}` | l'app range dans son magasin |
| Réglages synchronisés | `account::AccountSync` (`to_send`, `sent`, `unsent`, `forget`, `fetched`, `last`), `settings::changed`, `SyncedValues::{overlay, remove_sent, without, apply_to}` | `SyncedValues::apply_to` ramène le débit aux bornes |
| Micro | `micro::MicUi` (`lit`, `sent`, `refused`, `shown`, `tap`, `set`, `denied`, `changed` → avis, `label`, `tooltip(lang, raccourci)`, `tile`, `ui_json`) | textes de link-ui dans le catalogue du cœur |
| Écrans | `Screens::session_hdr(écrans)`, `Screens::primary()`, `screens::primary_index` | |
| HDR | `rules::hdr` : `pq_to_nits`, `nits_to_pq`, `eetf` (BT.2390 avec relèvement du noir), `sdr_curve` (celle de la Station, `SDR_SLOPE` = 3,6923077), `yuv_matrix`, `content_peak`, `frame_is_pq`, `Output`, `tone`, `hdr10_for`, `BT2020_TO_709` ; `HdrLuminance::from_display` | shaders : essai croisé |
| Chemin vers une Station | `relay::choose`, `relay::DIRECT_PROBE` (interne au cœur ; rien à faire dans les apps) | |

### À faire dans tous les ponts

1. `ffi_guard!` autour du corps de **chaque** fonction exportée et de chaque rappel C : pont Apple (`apps/apple/src`, 351 fonctions : `lib.rs` 105, `input.rs` 69, `playstation.rs` 54, `share.rs` 29, `account.rs` 25, `autotest.rs` 17, `lend.rs` 13, `audio.rs` 11, `welcome.rs` 9, `camera.rs` 8, `relay.rs` 8, `recording.rs` 2, `invite.rs` 1), JNI (`apps/android/jni/src`, fonctions `extern "system"` ; seul `audio.rs:57` a un `catch_unwind`), rappels C de Windows (`pyrowave.rs`, `decoder.rs`, `gpu.rs`), Linux et LG (`device/*.rs`). Défauts : `()`, `false`, `0`, le code d'erreur de la fonction, `null =>` pour une chaîne ou une poignée, `JObject::null()` (défaut du type).
2. Exposer ce que l'interface ne peut pas appeler elle-même (noms proposés, à garder d'un pont à l'autre) :

| Cœur | Pont C (Apple) | JNI (Android) | Tauri (Windows) |
|---|---|---|---|
| `short_fingerprint` | `kn_short_fingerprint(fp)` | `Native.shortFingerprint(fp)` | champ `fingerprint_short` des écrans, ou commande |
| `same_fingerprint` | `kn_fingerprint_same(a, b)` | `Native.fingerprintSame(a, b)` | commande, ou comparaison faite en Rust |
| `parse_address` | `kn_parse_address(texte, port_par_défaut)` → JSON `{host, port}` ou nul | `Native.parseAddress` | côté Rust (`cli.rs`, `ui.rs`) |
| `admin_url` | `kn_admin_url(hôte, port)` (sans console) | `Native.adminUrl` (existe) | `console::admin_url` |
| `set_background` | `kn_session_set_background(s, bool, appareil)` | `Native.sessionSetBackground(handle, background)` (sans débit ; `Device::Android` posé par le JNI) | `Session::set_background` |
| `values_json` | `kn_values_json()` | `Native.valuesJson()` | au lancement |
| `plan_json` | `kn_plan_json(choix)` | `Native.planJson(choix)` | `displays_info` |
| `pyrowave_failed` | `kn_session_pyrowave_failed(s)` | `Native.sessionPyroWaveFailed(handle)` | `Session::pyrowave_failed` |
| `Lowered::text` | `kn_pyrowave_lowered_text(l, h, fréquence, mbps)` | `Native.pyrowaveLoweredText(…)` | Rust |
| `looks_like_invite` | `kn_looks_like_invite(texte)` | `Native.looksLikeInvite(texte)` | Rust |
| `recording::clock` | `kn_recording_clock` (existe) | `Native.recordingClock(secondes)` | Rust |
| `MicUi::ui_json` | `kn_mic_ui(json d'état, raccourci)` ou tenu par le pont | idem | `micro::MicUi` |
| `pyrowave::Config::parse` | `kn_pyrowave_config(octets)` → JSON | côté Rust du JNI | Rust |
| `discovery::found` | — (Bonjour de Swift : seule l'empreinte passe par `kn_normalize_fingerprint`) | `Native.discoveryFound(json)` si Kotlin résout | Rust |
| `bundle` | `kn_ps_bundle_parse(texte)`, `kn_ps_bundle_match_accounts(…)` | idem | Rust |
| `AccountSync` | tenu par le pont (JSON) | idem | Rust |

## Mac, iPhone, iPad, Apple TV (`apps/macos`, `apps/ios`, `apps/tvos`, pont `apps/apple`)

À remplacer :

- **Empreinte affichée** : `LinkFormat.fingerprint` (`macos/KyberNow/LinkUI.swift:81-89`, appelé par les `SettingsView` Mac, iOS et tvOS, `tvos/AddHostView.swift:332`) et `AddHostFingerprint.groups` (`ios/KyberNowIOS/AddHostView.swift:110-130`, `AddHostPanes.swift:743`) → `kn_short_fingerprint`. Le iOS montrait tous les groupes : link-ui dit les quatre premiers.
- **Même empreinte** (MAC-20) : `HostStore.sameFingerprint` (`Model.swift:832-845`), `AddHostFingerprint.same` (iOS), et les `==` bruts : iOS `AppModel.swift:285, 292, 484`, tvOS `AppModel.swift:347`, Mac `PlayStation.swift:1156` → un seul helper Swift sur `kn_fingerprint_same`. Le cœur fait la même chose dans `account::merge` : un ordinateur ne revient plus deux fois à l'accueil.
- **Arrière-plan** (TRANS-40) : Mac `SessionController.swift:337` (`occludedBitrate`, 20 Mbit/s) et iOS `SessionController.swift:140` (`backgroundBitrate`, 8 Mbit/s) → `kn_session_set_background(true/false, appareil)` : `Computer` sur Mac (20 Mbit/s), `AppleMobile` sur iPhone et iPad (8, comme aujourd'hui), `AppleTv` sur l'Apple TV (8 ; rien aujourd'hui), quand l'app passe en arrière-plan. iOS sur un écran externe (image encore montrée) : pas d'arrière-plan.
- **Profils et bornes** (TRANS-41, 42) : `Model.swift:296-317` (`minBitrate` 5, `maxBitrate` 4294, 60/90/300, `takesPyroWave`) et les bornes répétées (`Model.swift:341, 374`, `SessionController.swift:2184`, `Accounts.swift:847`, `SettingsView.swift:1092-1112` ; iOS `SettingsView.swift:721-752` ; tvOS `SettingsView.swift:482, 486`) → `kn_values_json` (profils, `manual_mbps` : 500 en HEVC, AV1, H.264 et 2 500 en PyroWave, au lieu de 4 294 et 1 500) et `kn_plan_json`.
- **PyroWave** : `KyberKit/PyroWaveSupport.swift:84-96` (`PyroWaveRate` : 800, 1 200, plafond 1 500, puissance 0,75) → `values_json` (plafond à la main : 2 500) et `profile::pyrowave_mbps` (par le pont). `KyberKit/PyroWaveDecoder.swift:33-75` (`PyroWaveConfig`) → `kn_pyrowave_config`. Décodeur Metal qui ne s'ouvre pas → `kn_session_pyrowave_failed`.
- **Adresse d'Admin** (TRANS-36) : `macos/KyberNow/SettingsView.swift:840-846` et `tvos/KyberNowTV/SettingsView.swift:760-764` (crochets ajoutés à la main), iOS `SettingsView.swift:1027` (console jetable), tvOS `AddHostView.swift:500` (sans crochets) → `kn_admin_url(hôte, port)`. Port 9443 en dur (`Model.swift:821`, `Accounts.swift:383`) → `admin_port` de `values_json`.
- **« Définition ramenée »** (COEUR-41) : `LinkUI.swift:1563-1565` (utilisé par iOS `SessionStats.swift:135`, tvOS `SessionMenu.swift:76`) → `kn_pyrowave_lowered_text` avec les nombres de `KnSecond`.
- **Enregistrement** : `KyberKit/RecordingCore.swift:55-75` garde le choix des textes de `ui_json` (référence) et `kn_recording_clock` ; rien d'autre à copier.
- **Invitation** : `KyberKit/LendCore.swift:155-160` (`looksLikeInvite`) → `kn_looks_like_invite`.
- **Constantes** : `ShareCore.swift:244` (`relayLinkWait` 10), `ConsoleClientCore.swift:241, 247` (`noticeWait`, `beforeNotice` « 0.11.2 »), `Share.swift:488` (`guests.count >= 4`) → `values_json`.
- **HDR** : les shaders Metal (PQ, EETF, courbe vers le SDR) vérifiés contre `rules::hdr` (essai croisé : mêmes valeurs à 1e-4 sur une grille).
- **Micro** : l'état de la puce et de la tuile, s'il est refait en Swift → `MicUi` (par le pont), textes du catalogue du cœur.

À retirer ensuite : `LinkFormat.fingerprint`, `AddHostFingerprint`, `sameFingerprint`, `occludedBitrate`, `backgroundBitrate`, les constantes de `Model.swift` et de `PyroWaveSupport.swift`, `HomeAdmin.url` (repli), `LinkUI.lowered`, `looksLikeInvite`, `relayLinkWait`, `noticeWait`, `beforeNotice`, `PyroWaveConfig` ; dans le pont, `kn_session_set_bitrate_cap` et `kn_session_set_hidden` si plus rien ne s'en sert ; les entrées de catalogue Swift devenues inutiles.

## Android (`apps/android`, pont `apps/android/jni`)

Chemins Kotlin sous `app/src/main/java/fr/leslignesclaires/kybernow/`.

- **Empreinte** (TRANS-34) : `ui/add/AddHostScreen.kt:93` (`shortFingerprint`, 8 caractères `7C:41:E0:9B`) et `ui/settings/Settings.kt:761` → `Native.shortFingerprint`. Comparaisons `==` : `ui/AppModel.kt:204`, `playstation/Handoff.kt:91`, `core/Invite.kt:62` → `Native.fingerprintSame`.
- **Arrière-plan** (TRANS-40) : `session/SessionActivity.kt:738, 785` (`sessionSetBackground(handle, true, 2_000_000)`) → `Native.sessionSetBackground(handle, background)` ; le JNI (`exports.rs:441`) appelle `Session::set_background(…, Device::Android)` (2 Mbit/s, comme aujourd'hui) et garde `release_all` de la chaîne d'entrées.
- **Bornes** : `model/SessionMode.kt:83` (`coerceIn(5, 4294)`), `model/SettingsSync.kt:103` (`1..4294`) → `values_json.manual_mbps`.
- **AND-30, une seule source de texte** : `ui/home/LendHome.kt:76-85` (`availabilityLabel`) → `Native.lendAvailabilityLabel` (existe) ; `ui/home/HomeScreen.kt:666-670` (`relayRefusalText`), `session/SessionState.kt:106-110` (`relayText`), `ui/playstation/PsHome.kt:145-148` (codes 31, 32) → `Native.relayErrorText` (existe) ; `session/SessionOverlay.kt:865-872` (`routeLabel`) et les `R.string.route_relay` directs (`SessionOverlay.kt:859, 1041, 1070`, `PsOverlay.kt:100`, `BorrowBar.kt:114`) → `Native.relayPathLabel` (existe). Adresse IPv4 du réveil : `model/JsonLists.kt:22` (`isIpv4`, `Hosts.kt:292`) → la cible du cœur (`Native.wakeParseMac` puis `WakeTarget::from_mac`, qui lit l'IPv4) ; `AddHostScreen.kt:532-538` (`consoleIpv4`, résolution DNS) reste à l'app. Verrou de multidiffusion : un seul assistant Kotlin avec compteur (`model/Discovery.kt:53`, `playstation/PlayStations.kt:108`, `ui/playstation/PsAdd.kt:488`) — Kotlin seulement.
- **Copies des règles** : `core/Welcome.kt:56-80` (`Fallback`, repli en production) → retirer, l'appel natif suffit (en cas d'échec : pas d'accueil, journal) ; `core/StationOffer.kt:37, 75` → `values_json` ; `core/Recording.kt:78-82` (horloge) → `Native.recordingClock` ; textes de `ui_json` : prendre les 11 (Kotlin n'en lit que 7) ; `core/Lend.kt:436` (`isInviteLink`, `kybernow://` seulement) → `Native.looksLikeInvite` (le lien web `…/link?borrow=` illisible dit maintenant « Ce lien d'invitation est illisible. »).
- **Adresse d'Admin** : déjà sur `Native.adminUrl` (vague 1).
- **PyroWave** : `jni/src/pyrowave.rs:13-40` (`PwConfig`, sans contrôle des octets 5 à 7 ni de la parité) → `kybernow_core::pyrowave::Config` (`fallback` remplace `PwConfig::fallback`) ; décodeur Vulkan qui ne s'ouvre pas → `Session::pyrowave_failed`. Flux d'images de `jni/src/video.rs` → `frames::FrameFlow` si la machine y est refaite.
- **Micro** : textes et état de la puce → `MicUi::ui_json` (par le JNI).
- **Dépendances** : `ndk-sys` et `kynput-sys` sont retirés de `jni/Cargo.toml` (TRANS-50) ; vérifier la construction Android (`build-native.sh`).

À retirer ensuite : `shortFingerprint`, `availabilityLabel`, `relayRefusalText`, `relayText`, `routeLabel`, `isIpv4`, `Welcome.Fallback`, `NOTICE_WAIT_MS`, `BEFORE_NOTICE`, `Recording.clock`, `isInviteLink`, `PwConfig`, et les `R.string.relay_*`, `route_*`, `lend_state_*` devenus inutiles (avec le contrôle `strings.py --check`).

## Windows (`apps/windows`)

- **WINAPP-11** : `src/video.rs:497-516, 714-720` (`pyro_failed`) → en plus de retenir l'échec, `Session::pyrowave_failed()` une fois ; `FrameFlow::unavailable()`.
- **WINAPP-49** : `src/hdr.rs:22-243` → `rules::hdr` (la courbe vers le SDR change : celle de la Station et de Linux ; l'EETF prend le relèvement du noir ; `tone` rend aussi `dst_lo_pq`) et `HdrLuminance::from_display` (à la place de `luminance_from`) ; `hdr10_for` → `rules::hdr::hdr10_for` ; garder `sdr_white_nits`, `outputs`, `sdr_whites` (DXGI). Shader `src/render.rs:43-135` : passer à la courbe de la Station et au relèvement du noir, essai croisé contre `rules::hdr`.
- **WINAPP-50** : `src/pyrowave_config.rs` → `kybernow_core::pyrowave::Config` (mêmes méthodes ; `is_config` accepte maintenant 16 octets **ou plus** pour la version 2).
- **WINAPP-51** : `src/video.rs:471-858` → `frames::FrameFlow` : les étapes remplacent `waiting_key`, `send_config`, `codec_announced`, `last_wanted`, `keyframe`, `gate` ; garder la file, le rendu, `newer_waiting`, le repli matériel → processeur (`reset(now)` après), la perte de la carte (`reset`). `Step::Lost` → `Session::keyframe_lost` (au lieu de `request_keyframe` forcé) ; `Step::Keyframe` → `request_keyframe`.
- **WINAPP-52** : `ui/src/lib/api.ts:801-870` (`isCustom`, `profileBase`, `fit`, `sessionPlan`, `PYROWAVE_MBPS*`) → `displays_info` rend le résultat de `plan::plan_json` (mode, débit, réglages, nom) et l'interface ne calcule plus rien ; `src/plan.rs` → `kybernow_core::plan` (le fichier de l'app se retire ; mêmes noms). `api.ts:124-126`, `457-458` (`MAX_SESSIONS`, `Home.svelte:219`) → `values_json`. Curseur du débit (`ui/src/Settings.svelte:382-401`, 5 à 500, 1 500 en PyroWave) → `values_json.manual_mbps` (PyroWave : jusqu'à 2 500).
- **WINAPP-53** : `src/discovery.rs:51-97` → `discovery::found` (l'empreinte arrive maintenant mise en forme).
- **WINAPP-54** : `src/consoles.rs:39-65, 576-823` (`valid_psn_id`, `new_uuid`, `Bundle*`, `pairings_any`, `stations_any`) → `playstation::bundle` ; `merge` garde le rangement dans `Kept` et le trousseau Windows, sur `Bundle::parse` et `match_accounts`, Stations par `BundleStation::matches`.
- **WINAPP-55** : `src/synced.rs` (`overlay`, `AccountSync`) → `account::AccountSync`, `SyncedValues::overlay` ; `changed` → `settings::changed` (le HDR et le mode immersif restent ajoutés par l'app) ; `SyncFile` garde un `AccountSync` du cœur par compte (même JSON).
- **WINAPP-56** : `src/ui/micro.rs:19-151` (`MicSlot`, `BarMic`, `state_code`) → `micro::MicUi` (`ui_json` donne `state`, `reason`, textes) ; garder `micro.permission_windows` dans `denied`.
- **WINAPP-62** : `src/displays.rs:156-166` (`session_hdr`) → `Screens::session_hdr` ; `displays.rs:187-190` (`primary` sur les `Monitor`) → `screens::primary_index` sur les `DeviceScreen` déjà faits, ou garder la recherche sur `Monitor` si la liste n'existe pas encore à cet endroit (règle identique).
- **TRANS-40** : `src/window.rs:2250-2266` (`HIDDEN_BITRATE`, `set_bitrate_cap` seul) → `Session::set_background(…, Device::Computer)` (20 Mbit/s, comme aujourd'hui ; dit aussi l'image figée : la veille « aucune image » attend).
- **TRANS-34** : `api.ts:812-813` (`fingerprintGroups`, `Statistics.svelte:43`, `AddSheet.svelte:530`) → `fingerprint_short` donné par Rust ; `sameFp` (`Home.svelte:186`, `Welcome.svelte:60`), `accounts.rs:42-45` (`same_fingerprint`), `inviting.rs:18-27` → `console::same_fingerprint`.
- **TRANS-35, 42** : `src/cli.rs:181-192` (`split_host`) et `ui.rs:1551-1575` (`parse_link_text`, partie adresse) → `console::parse_address` ; `cli.rs:295` et `AddSheet.svelte:65, 147` (9443) → `ADMIN_PORT`.
- **Codes** : `JoinSheet.svelte:61`, `api.ts:1195` (`psGroupCode`) : le code montré vient de Rust (`format_code`) ; la saisie peut grouper en direct, avec la même règle (trois et trois).
- **Relais** : `Station.svelte:23` (`split('#')`) → `relay_label` donné par Rust.

À retirer ensuite : `src/plan.rs`, `src/pyrowave_config.rs`, les fonctions pures de `src/hdr.rs`, `overlay` et `AccountSync` de `src/synced.rs`, `MicSlot`, `session_hdr`, `HIDDEN_BITRATE`, `split_host`, `Bundle*` de `consoles.rs`, et en TypeScript `isCustom`, `profileBase`, `fit`, `sessionPlan`, `PYROWAVE_MBPS*`, `MAX_SESSIONS`, `fingerprintGroups`, `sameFp`, `psGroupCode` ; `check-i18n.mjs` suivra les clés du micro ajoutées au catalogue du cœur.

## Linux, Steam Deck, Link OS (`apps/linux`)

L'interface se compile sans le cœur (captures) : elle dépend de **`kybernow-rules`** (`kybernow-rules = { path = "../core/rules" }`, non facultatif), et la partie `device/` du cœur.

- **Interface, par `kybernow-rules`** : `parse_address` (`app.rs:7870`, essai 7901-7917) → `rules::address::parse_address(texte, None)` ; `short_fp` (`app.rs:7858-7863`, « 7c3a · 91e0 · … ») → `rules::fingerprint::short` (« 7C3A 91E0 … », comme link-ui) ; `format_code` (`app.rs:1235`) → `rules::format::format_code` ; `recording_clock` (`app.rs:1715-1718`) → `rules::format::recording_clock` ; `relay_label`, `DEFAULT_RELAY` (`model.rs:652-661`) → `rules::relay::{label, DEFAULT_RELAY}` (vide : `DEFAULT_RELAY`, à faire par l'app) ; `RELAY_LINK_WAIT` (`app.rs:1064-1067`, assertion `device/session.rs:681`) → `RELAY_LINK_WAIT_SECS` ; `MAX_GUESTS` (`app.rs:1197-1198`) → `rules::limits::MAX_GUESTS` ; menu `BITRATES` (`app.rs:2663-2665`, 10 à 500) dans `rules::limits::manual_mbps(Device::Computer, …)` (PyroWave : jusqu'à 2 500) ; `admin_url` sans le cœur (`app.rs:7826-7837`) → `rules::address::admin_url` ; exemple 24811 (`ui/pair.rs:217-229`, `app.rs:5134`) → `ADDRESS_EXAMPLE` ; `hdr.rs:18-80` → `rules::hdr` (même courbe, déjà celle de la Station), shader `src/native/hdr_gl.c` vérifié par un essai croisé.
- **`welcome.rs`** : l'inclusion `#[path = "../../core/src/welcome.rs"]` (`main.rs:30`) marche de nouveau (elle cassait la construction depuis COEUR-40) ; elle reste jusqu'à ce que le cœur retire ses deux inclusions internes (après la 2b).
- **Device, par le cœur** : `device/session.rs:819-821` (mise à l'échelle recopiée) → `profile::pixel_rate_scale`, ou mieux `plan::plan` ; `device/video.rs:69-127` (`PwConfig`) → `pyrowave::Config` ; `run`/`frame` (`device/video.rs:384-646`) → `frames::FrameFlow` (Linux gagne le repli « format jamais annoncé ») ; `device/jobs.rs:223-261` → `discovery::found` ; `device/ps.rs:1313-1400` (`bundle`, `merge` à la main) → `playstation::bundle` (identifiant PSN vérifié) ; `recording_ui` (`device/session.rs:559-588`) inchangé (`ui_json`).
- **LINAPP-40** : `synced.rs` → la mémoire des envois passe au cœur (`account::AccountSync`, côté device) ; l'interface garde `apply` sur `HostSettings` et `snap_mbps` (son menu), et un essai compare `SyncKeys` à `SyncedValues` (même JSON : `json_comme_le_coeur`) ; `merge`, `remove_sent`, `without` → `SyncedValues::{overlay, remove_sent, without}` côté device. `PsSettings` (`model.rs:497-555`) a le JSON de `PlaySettings` (essai du cœur `json_des_reglages_comme_les_apps`) : la conversion device passe par serde.
- **Micro** : `micro.rs:48-150` (`MicUi`, textes en dur) → `micro::MicUi::ui_json` donné par le device à l'interface (comme `recording_ui`) ; le sous-titre « Envoi… » n'est pas dans link-ui (le cœur dit « Envoyé » dès qu'il est allumé et pas refusé).
- **TRANS-40** : aucun plafond aujourd'hui → `Session::set_background(…, Device::Computer)` (20 Mbit/s) quand la fenêtre est réduite ou cachée (événements SDL), sauf en Link OS plein écran.
- **WINAPP-49** : rien à changer à la courbe (Linux avait déjà celle de la Station).

À retirer ensuite : `parse_address`, `short_fp`, `format_code`, `recording_clock`, `relay_label`/`DEFAULT_RELAY` du modèle, `RELAY_LINK_WAIT` et son assertion, `MAX_GUESTS`, les fonctions de `hdr.rs`, `PwConfig`, la machine de `device/video.rs`, le JSON fait main du paquet PlayStation, les textes de `micro.rs`, `SyncKeys::merge`, `remove_sent`, `without`.

## Téléviseurs LG (`apps/webos`)

Même principe que Linux : l'interface dépend de `kybernow-rules`.

- **Interface** : `parse_address` (`app.rs:4984`, plus faible que Linux : `[192.168.1.20]:24811` passait) → `rules::address::parse_address(texte, None)` ; `admin_url` local (`app.rs:4975-4977`, jamais le cœur) → `rules::address::admin_url` ; `fingerprint_groups` (`ui/settings.rs:307-315`, « — » si vide) → `rules::fingerprint::short` (le tiret reste à l'interface) ; `format_code` (`app.rs:964-967`) ; `recording_clock` ; `relay_label`, `DEFAULT_RELAY` (`model.rs:519-527`) ; `RELAY_LINK_WAIT` (`app.rs:807`, assertion `device/session.rs:476`) ; `MAX_GUESTS` (`app.rs:928`) ; débits (`app.rs:4615-4619`, 20 à 95) dans `manual_mbps(BitrateDevice::LgTv, …)` ; `cap_mbps` (`app.rs:379-388`, 80 sur Ethernet 100 Mbit/s) reste à l'app (sa prise réseau) ; `hdr.rs` (375 lignes, pas comparé) → `rules::hdr`.
- **Device** : `device/jobs.rs:208-248` → `discovery::found` ; boucle `device/media.rs` (vers `:140`) → `frames::FrameFlow` ; `recording_ui` (`device/session.rs:370-383`) : prendre aussi `pill` et les infobulles si l'interface les montre.
- **TRANS-40** : aucun plafond → `Session::set_background(true, Device::LgTv)` (2 Mbit/s) quand l'app passe en arrière-plan (`SDL_APP_WILLENTERBACKGROUND`), `false` au retour (`device/mod.rs:436-450`, drapeau `FOREGROUND`).
- **Dépendance** : `kynput-sys` est retiré de `webos/Cargo.toml` (TRANS-50).

À retirer ensuite : comme Linux.

## Client web (`server/web`)

Le client web ne lie pas le cœur (TypeScript dans le navigateur). Il garde ses copies, mais **alignées** et notées :

- empreinte courte au format de `rules::fingerprint::short` (`7C41 E09B AABB CCDD`) ;
- bornes du débit à la main propres au navigateur (`plan.ts:57-82` : 10 à 150, 1 000 en PyroWave), justifiées par ses décodeurs (à écrire dans `docs/link-ui.md` par la session des docs, comme l'exception LG) ;
- profils (60, 90, 300, +1/3, +1/2), `PYROWAVE_MBPS` 800 et 1 200, débit en arrière-plan 20 Mbit/s (celui des ordinateurs) si le client en a un ;
- un essai du client compare ses valeurs à celles de `values_json` (fichier JSON produit par un essai du cœur, ou copie vérifiée à la publication).

## Ce qui reste au cœur après la 2b

- Retirer de `apps/core/src/welcome.rs` les deux inclusions `#[path]` de `rules/src` quand Linux et LG dépendent de `kybernow-rules` (marquées « à retirer en 2b ») ; ou déplacer l'accueil dans la caisse.
- KYB-25 : quand `PacketSink` (correctif kyclient) devient non `Copy` avec un `Drop` qui envoie `Closed`, adapter `media.rs` (`packet_sink`, `packet_sink_tracked`, `trampoline`) : la boîte n'est plus libérée par le seul paquet `Closed` de la tâche ; l'ancienne API reste le temps de la 2b (session des correctifs Kyber).
- Ports PlayStation partagés avec le relais et Admin (`playstation::discovery`, `endpoint`) : à porter par `server/relay/tunnel` (zone du relais), puis repris ici.
