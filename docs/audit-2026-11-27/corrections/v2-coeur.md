# Vague 2, phase 2a : cœur, API communes (`apps/core`)

Branche `claude/v2-coeur-8452cb`. Plan : [`../vague-2.md`](../vague-2.md). Guide pour la phase 2b : [`../migration-coeur-2b.md`](../migration-coeur-2b.md). Zone : `apps/core` (avec la nouvelle caisse `apps/core/rules`, ses catalogues et son README FR/EN), `apps/Cargo.lock`, et les `Cargo.toml` des ponts Android et LG pour TRANS-50. Aucun autre fichier des apps.

## Fait

| Constat | Correction |
|---|---|
| (trouvé) | **Linux et LG ne se construisaient plus** depuis COEUR-40 (vague 1) : `welcome.rs`, inclus tel quel par leurs interfaces (`#[path]`), appelait `crate::account::relay_label`. Vérifié par `cargo check --no-default-features` des deux apps (erreur E0433), réparé : `welcome.rs` reprend `rules/src/relay.rs` et `fingerprint.rs` par chemin (« à retirer en 2b »). La coordination l'avait aussi réparé dans main (`web_host`, une copie) : à la fusion de main, la règle commune est gardée ; le fichier ne dépend toujours que de `crate::i18n`. |
| — | Caisse **`kybernow-rules`** (`apps/core/rules`, aucune dépendance, aussi `kybernow_core::rules`) : les règles pures, que Linux et LG pourront prendre sans le cœur ; essais dans la CI du cœur (`ci-essais.sh`). |
| TRANS-34 | `console::short_fingerprint` (`7C41 E09B AABB CCDD`, `rules::fingerprint::short`). |
| MAC-20 | `console::same_fingerprint`, `same_fingerprint_opt` (préfixe, deux-points, espaces, casse ignorés ; vide jamais) ; la même règle dans `account::merge`, l'empreinte épinglée d'Admin (`PinVerifier`) et l'empreinte d'une route du relais. |
| TRANS-35 | `console::parse_address(texte, port_par_défaut)` : IPv6 entre crochets avec ou sans port, IPv6 nue = hôte entier (jamais `fe80:` port 1), IPv6 vérifiée, IPv4 entre crochets refusée ; les liens (partage, invitation) la prennent. |
| TRANS-36 | `console::admin_url` (crochets une seule fois, aussi pour `Console::admin_url`), `ADMIN_PORT` (9443), `ADDRESS_EXAMPLE` (`192.168.1.20:9443`, le même partout). |
| TRANS-40 | `Session::set_background(bool, Device)` : plafond `background_bitrate(device)` (décision de l'auteur du 27/11 : 20 Mbit/s sur Mac et PC, 8 sur iPhone, iPad et Apple TV, 2 sur Android ; LG avec Android) et image figée ; `rules::limits::Device`. |
| TRANS-41 | Bornes du débit à la main : `rules::limits::manual_mbps`, `clamp_manual_mbps`, `profile::manual_mbps(device, codec)` : 5 à 500 Mbit/s en HEVC, AV1 et H.264 (aucune carte n'encode au-delà), 2 500 en PyroWave (décision de l'auteur du 27/11), 95 sur un téléviseur LG, pas de 5 ; les réglages synchronisés reçus y sont ramenés (`SyncedValues::apply_to`). |
| (fusion) | L'essai du port 443 venu de main (`adresse_de_l_inscription`) montrait qu'une empreinte illisible gardait `:443` dans le libellé : `rules::relay::label` lit l'adresse sans l'empreinte, et `account::relay_label` passe par lui (une seule règle). |
| TRANS-42 | Valeurs communes dans `rules::limits` et `plan::values_json(caps)` (profils sur l'appareil, PyroWave, bornes, débit en arrière-plan, invités, sessions, délais, port et exemple d'Admin, relais public) ; `share::MAX_GUESTS`, `RELAY_LINK_WAIT`, `focus::MAX_SESSIONS`, `station_offer::NOTICE_WAIT_SECS`, `BEFORE_NOTICE`, `profile::PYROWAVE_MBPS*` en viennent ; `profile::pyrowave_mbps` ; relais (`rules::relay`, mêmes formes que le tunnel, essai croisé) ; `welcome.rs` n'écrit plus `share.kybernow.app` en dur. |
| WINAPP-52 | `plan.rs` (venu de l'app Windows, avec ses essais) : `plan`, `plan_json` (réglages, mode, débit de la session, nom), `values_json`. |
| COEUR-37 | `session.rs` : `connect_kyber` (hôte, invité, emprunteur), `guest_console`, `spawn_guest_beat` (signes de vie du partage et du prêt) ; le fil `kybernow-hdr` garde le nom de l'ordinateur au journal (les préparations l'étaient depuis COEUR-13). |
| COEUR-38 | `Reply::read` (statut, corps, `Retry-After`, une fois), `Console::answer`, `Console::failure` : huit copies réunies ; `share_wait` garde maintenant `Retry-After`. |
| COEUR-39 | `relay::choose` et `relay::DIRECT_PROBE` : une règle pour la session, le prêt et « Regarder » ; certificat changé et compte sans jeton s'arrêtent partout ; `KYBERNOW_RELAY=off` ne prend jamais le relais ; `lend::DIRECT_PROBE`, `share::LOCAL_FIRST`, `session::DIRECT_PROBE` retirés (aucune app ne les lisait). |
| COEUR-41 | `share::format_code` et `recording::clock` dans `rules::format` ; `lend::looks_like_invite` ; « Définition ramenée » : `pyrowave::Lowered::text` existait, il ne manque que la fonction du pont (2b) ; textes de l'enregistrement : `ui_json` reste la seule source (décision de COEUR-43). |
| WINAPP-11 | `Session::pyrowave_failed()` : repli de l'appareil pour la session, image relancée (sans nouvelle variante de `Notice`, voir « Laissé »). |
| WINAPP-49 | `rules::hdr` : PQ, `BT2020_TO_709`, EETF de BT.2390 **avec** le relèvement du noir, courbe vers le SDR **de la Station** (correctifs 0022, celle de Linux ; Windows avait un Reinhard étendu), `yuv_matrix`, `content_peak`, `frame_is_pq`, `Output`, `tone`, `hdr10_for` ; `HdrLuminance::from_display`. |
| WINAPP-50 | `pyrowave::Config` : 16 octets au moins, version 1 et suivantes (octets en trop ignorés : prêt pour la v2 de KYB-29), octets 5 à 7 vérifiés, 4:2:0 pair, en-tête de séquence, repli. |
| WINAPP-51 | `frames::FrameFlow` : machine pure du flux d'images (format jamais annoncé ouvert d'office avec les paramètres reçus, paramètres bout à bout puis remplacés, décodeur refait quand ils changent, `av1C`, perte dite une fois, image clé au plus toutes les 300 ms, reprise, PyroWave intra, décodeur indisponible, décodeur refait par l'app). |
| WINAPP-53 | `discovery::found` (`kyber=0`, IPv4 d'abord, jamais d'IPv6 locale au lien, empreinte mise en forme). |
| WINAPP-54 | `playstation::bundle` (paquet tolérant, identifiants PSN vérifiés, comptes rapprochés, Stations par la règle commune des empreintes). |
| WINAPP-55 | `account::AccountSync`, `settings::changed`, `SyncedValues::overlay`. |
| LINAPP-40 | `SyncedValues::remove_sent`, `without` ; `PlaySettings` a exactement le JSON de `PsSettings` de Linux (essai). |
| WINAPP-56 | `micro::MicUi`, `MicTap` ; textes de link-ui (puce, infobulles, tuile) dans le catalogue du cœur, FR et EN. |
| WINAPP-62 | `Screens::session_hdr`, `Screens::primary`. |
| AND-30 | Côté cœur, rien à ajouter : `lendAvailabilityLabel`, `relayErrorText`, `relayPathLabel` existent, et `WakeTarget::from_mac` lit déjà l'IPv4 ; le guide dit quoi remplacer en Kotlin. |
| TRANS-50 | `ndk-sys` et `kynput-sys` retirés du pont Android, `kynput-sys` de l'app LG (aucun usage ; `apps/Cargo.lock` suit). L'app Apple garde le sien (`lib.rs:1159`). |
| TRANS-51 | `regist::PORT`, `takion::PORT`, `playstation::session::PORT` retirés ; l'en-tête `Host` prend `endpoint::PORT_RP`. |
| MAC-35 (reste) | README FR et EN du cœur : plus de fonctions retirées citées (`kn_console_share_watch`, et dans le JNI `shareJoin`, `shareNormalizeCode`, `shareQualityLabel`, `shareRoleWithPads`, `shareRoleWithKbm`, `consoleShareWatch`, absents du pont) ; chaque `kn_…` et `Native.…` cité vérifié contre les ponts. |
| Ponts | `ffi_guard!({ … })`, `ffi_guard!(défaut => { … })`, `ffi_guard!(null => { … })` (nom de la fonction pris tout seul), `ffi::guard_null` ; README FR et EN. |

## Laissé, et pourquoi

- **Avis quand PyroWave ne s'ouvre pas** : une nouvelle variante de `Notice` casserait les `match` exhaustifs des ponts Apple, Linux, LG et Android avant leur migration. Le repli se voit dans la barre (format) et au journal. Si un avis est voulu, son texte est à écrire dans `docs/link-ui.md` (session des docs), puis la variante à ajouter avec la 2b.
- **KYB-25** (`PacketSink` non `Copy`, avec `Drop`) : l'API n'existe pas encore (session des correctifs Kyber). Quand elle arrive, `media.rs` (`packet_sink`, `packet_sink_tracked`, `trampoline`) s'y adapte : la boîte ne sera plus libérée par le seul `Closed` de la tâche ; à fusionner en même temps. Noté dans le guide et dans le dossier partagé.
- **Ports PlayStation partagés avec le relais et Admin** (TRANS-42) : leur place est `server/relay/tunnel` (zone du relais) ; le cœur les reprendra ensuite.
- **`welcome.rs` dans la caisse** : il reste dans le cœur tant que Linux et LG l'incluent par `#[path]` ; ses deux inclusions de `rules/src` sont « à retirer en 2b ».
- **Le sous-titre « Envoi… » du micro** (Linux) n'est pas dans link-ui : `MicUi::tile` dit « Envoyé » dès que le micro est allumé et pas refusé. À trancher dans `docs/link-ui.md` si l'attente doit se dire.
- **Débit en arrière-plan d'un téléviseur LG** : la décision de l'auteur nomme Mac et PC (20), iPhone, iPad, Apple TV (8) et Android (2) ; LG (aucun plafond aujourd'hui) prend 2 Mbit/s, comme Android. À changer dans `rules::limits::background_bitrate` si l'auteur préfère autre chose.
- **Bornes du débit à la main** : 500 Mbit/s en HEVC, AV1 et H.264, 2 500 en PyroWave (décision de l'auteur) ; le Mac allait jusqu'à 4 294. Le client web garde ses bornes de navigateur (10 à 150, 1 000 en PyroWave), exception à écrire dans link-ui par la session des docs.
- **Retraits** : `lend::DIRECT_PROBE`, `share::LOCAL_FIRST`, `session::DIRECT_PROBE`, `regist::PORT`, `takion::PORT`, `playstation::session::PORT` : cherchés dans toutes les apps (Rust des ponts, Swift, Kotlin, TypeScript) avant d'être retirés ; aucun usage. `RecordingState::menu_item`, rétabli dans main, est gardé.
- **Guide en français seulement**, comme les autres fichiers de `docs/audit-2026-11-27`.

## Vérifié, et comment

- Arbre à moi (`KYBERNOW_ARBRES=…/third_party/arbres-v2-coeur scripts/arbres.sh preparer apple`), `apps/core/scripts/ci-essais.sh` (correctifs des apps posés, libkynput, libvlc factice) : `cargo test -p kybernow-core --features pyrowave --lib` : **504 réussis, 0 échec, 4 ignorés** (472 au départ) ; `cargo test -p kybernow-rules` : **15 réussis**. Refait sur l'arbre remis au dernier commit (`preparer apple`), `--locked`.
- Un essai par nouvelle API : `refus_lus_une_fois` (COEUR-38, aurait échoué avant), `un_seul_choix_du_chemin` (COEUR-39), `forme_du_relais_comme_le_tunnel`, `empreinte_et_adresse_communes`, `arriere_plan_un_seul_debit`, `valeurs_communes`, `plan_en_json` et les cinq essais venus de `plan.rs` de Windows, MAC-20 dans `un_ordinateur_une_seule_fois`, bornes dans `reglages_synchronises_appliques`, `memoire_des_envois_synchronises`, `json_des_reglages_comme_les_apps`, `pyrowave_qui_ne_s_ouvre_pas`, `paquet_de_configuration`, cinq essais de `frames`, `annonce_lue_d_une_seule_facon`, `paquet_commun`, `comptes_rapproches`, `puce_et_tuile_communes`, `principal_et_hdr_de_la_session`, `valeurs_hdr_du_systeme`, `macro_des_ponts`, invitations (`lend`) ; dans la caisse : adresses, empreintes, relais, codes, liens, bornes, PQ, EETF, courbe vers le SDR (pente 1 au genou, 4 → 1, 3,6923077), matrice YUV, tons.
- `cargo check -p kybernow-core --features sdl,pyrowave`, `… --examples` : propres. `cargo check -p kybernow-linux --no-default-features` et `-p kybernow-webos --no-default-features` : passent (échouaient avant). Essais de l'interface Linux (`cargo test -p kybernow-linux --no-default-features`) : **170 réussis** (welcome.rs et les règles incluses par chemin compris).
- Pas vérifié ici : la construction Android après TRANS-50 (pont JNI pour `aarch64-linux-android`), les apps elles-mêmes (rien n'y est changé hors des `Cargo.toml`).

## Notes de version

FR :
- Linux, Steam Deck et téléviseurs LG : l'app se construit de nouveau (une règle de l'accueil l'empêchait).
- Rejoindre un partage hors de chez vous : après trop d'essais, l'app dit de nouveau combien de temps attendre.
- Un ordinateur réinstallé (certificat changé) ou un compte déconnecté se signale tout de suite, aussi pour le prêt et « Regarder », au lieu d'essayer le relais.
- Réglages synchronisés : un débit réglé à la main très haut sur un appareil est ramené à ce que les autres acceptent (500 Mbit/s, 2 500 en PyroWave).
- Avec la mise à jour de chaque app (phase 2b) : la même empreinte de l'ordinateur partout (quatre groupes de quatre), les adresses IPv6 acceptées partout, le débit de chaque appareil quand la fenêtre est cachée (20 Mbit/s sur ordinateur, 8 sur iPhone, iPad et Apple TV, 2 sur Android et LG), un retour en HEVC quand PyroWave ne s'ouvre pas sur l'appareil, et le même rendu HDR sur tous les écrans SDR.

EN:
- Linux, Steam Deck and LG TVs: the app builds again (a welcome-screen rule prevented it).
- Joining a share away from home: after too many tries, the app tells you again how long to wait.
- A reinstalled computer (changed certificate) or a signed-out account is reported right away, also for lending and “Watch”, instead of trying the relay.
- Synced settings: a very high bitrate set by hand on one device is brought back to what the others accept (500 Mbit/s, 2,500 with PyroWave).
- With each app's update (phase 2b): the same computer fingerprint everywhere (four groups of four), IPv6 addresses accepted everywhere, each device's bitrate when the window is hidden (20 Mbit/s on computers, 8 on iPhone, iPad and Apple TV, 2 on Android and LG), a switch back to HEVC when PyroWave won't open on the device, and the same HDR rendering on every SDR screen.
