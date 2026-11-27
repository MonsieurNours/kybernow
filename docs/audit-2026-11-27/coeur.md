# Audit du cœur Rust partagé (`apps/core`) — 27/11/2026

Périmètre : `apps/core` (74 fichiers `.rs`, ≈ 86 000 lignes avec les essais), sur `main` après la 0.11.7 (1fb289c6). Analyse statique seule : lecture, grep, git log, et pour la sémantique de kynput/kyclient lecture ponctuelle de `third_party`. Rien n'a été compilé ni lancé. Six zones ont été lues en parallèle (session ; entrées et manettes ; son, média et débit ; compte, relais, Share et console ; PlayStation ; code mort et docs), puis chaque constat retenu ici a été relu dans le code. L'audit précédent (`docs/archives/audit-2026-10-20.md`) précède le cœur : aucun constat ne le recoupe.

Deux faits transverses rendent certains constats plus graves :

- **Aucun pont ne rattrape les paniques** (`catch_unwind` absent d'`apps/apple`, `apps/android/jni`, `apps/windows`, `apps/linux`, `apps/webos`). Depuis Rust 1.81, une panique qui traverse une fonction `extern "C"` **abandonne le processus**. Toute panique sur une donnée du réseau ou de l'app appelée depuis un pont ferme donc l'app.
- **La CI ne compile ni ne teste le cœur** (`.github/workflows/ci.yml:54-70` : seulement `server/admin`, `server/installer`, `server/tray`). Les nombreux essais de `apps/core` ne tournent qu'à la main.

Confiance : **vérifié** = relu dans le code, le mécanisme est certain ; **plausible** = le code est relu, mais le déclenchement dépend d'un comportement extérieur (système, Station, console) qui n'a pas pu être vérifié.

## Synthèse

| ID | Catégorie | Où | Constat | Effort | Confiance |
|---|---|---|---|---|---|
| COEUR-01 | bug critique | `playstation/audio.rs:90-144` | Boucle sans fin (et mémoire qui grossit) au passage de l'index de trame du son par zéro | S | vérifié |
| COEUR-02 | bug critique | `latency.rs:61-80` | `now_micros` faux sur 32 bits (webOS armv7, Android armeabi-v7a) : `timespec` déclaré en i64 | S | vérifié |
| COEUR-03 | bug critique | `media.rs:372-378` | `OpusHead` famille 0 avec plus de 2 voies : libopus lit au-delà du `Vec` de correspondance | S | vérifié |
| COEUR-04 | bug critique | `session.rs:2389-2413` | Après un redémarrage de la Station, le micro et la caméra de l'app visent des objets orphelins | S | vérifié |
| COEUR-05 | bug | `playstation/fec.rs:51-58`, `frame.rs:19,136` | FEC de Cauchy invalide au-delà de 256 unités (panique en debug, image fausse en release) | S | vérifié |
| COEUR-06 | bug | `playstation/stream.rs:180-191` | Le message BIG peut partir sans morceau final : la console ne répond jamais | S | vérifié |
| COEUR-07 | bug | `playstation/regist.rs:436`, `playstation/session.rs:174`, `playstation/psn.rs:77` | Découpe de `&str` par octets : panique (donc abandon de l'app) sur un texte non ASCII | S | vérifié |
| COEUR-08 | bug | `input/pipeline.rs:1318-1321` | `width * height` d'un pointeur déborde : lecture hors limites côté Swift | S | vérifié |
| COEUR-09 | bug | `input/pointer_layer.rs:346` | `clamp(0, size-1)` panique si la nouvelle taille est inférieure à 1, sous verrou | S | vérifié |
| COEUR-10 | bug | `input/gamepad_gate.rs:107-111` | Mac : un retour au repos d'axe réessayé est avalé par le filtre, le stick reste poussé | S | vérifié |
| COEUR-11 | bug | `input/pipeline.rs:539-553` | Mac : un stick tenu pendant que la barre est ouverte ne repart pas à sa fermeture | S | vérifié |
| COEUR-12 | bug | `session.rs:1427-1432` | Une panique du fil de session laisse tourner signal de vie et premier plan (`alive` reste vrai) | S | vérifié |
| COEUR-13 | bug | `session.rs:4554, 4645, 4734` vs `5288` | Trois préparations en vol non suivies par `release` : la fin peut précéder la préparation | M | vérifié (cœur) |
| COEUR-14 | bug | `session.rs:2370-2384` | Pendant l'attente de la Station : HDR, écrans, mode perdus | S | vérifié |
| COEUR-15 | bug | `share.rs:1705-1770`, `1789-1801` | Rejoindre par le réseau et le relais à la fois : invité fantôme côté Station | S | vérifié (cœur) |
| COEUR-16 | bug | `audio/mix.rs:91-95` | Stéréo vers une sortie 5.1/7.1 : la voie droite part au centre | S | vérifié |
| COEUR-17 | bug | `input/pipeline.rs:1294-1295` | Forme du pointeur périmée (empreinte par écran du PC, couche unique côté appareil) | S | vérifié |
| COEUR-18 | bug | `input/pipeline.rs:1011-1019`, `packet_queue.rs` | Texte long tapé ou collé : tronqué, file sans borne, une erreur de journal par caractère | M | vérifié |
| COEUR-19 | bug (sécurité) | `console.rs:111-118` | Sans empreinte connue, chaque connexion accepte n'importe quel certificat | S | vérifié |
| COEUR-20 | bug (sécurité) | `playstation/handoff.rs:186-224` | Essais de code à 6 chiffres sans limite de débit pendant 5 min | S | vérifié (débit estimé) |
| COEUR-21 | bug | `console.rs:758-763, 893-997` | Erreurs mises de côté dans la `Console` partagée : un fil reprend l'erreur d'un autre | M | plausible |
| COEUR-22 | bug | `session.rs:3612-3630` | Le fil du premier plan rappelle l'app après `Ended` | S | vérifié |
| COEUR-23 | bug | `session.rs:4120-4121` | Rafale de fausses « secondes » après un appel bloquant dans la boucle | S | vérifié |
| COEUR-24 | bug | `playstation/play.rs:237-253, 273-278, 357-362` | Verrou `ctrl` tenu pendant un `join` et des écritures TCP (5 s) | S | vérifié |
| COEUR-25 | bug | `relay.rs:485-502` | Deux tunnels vers la même Station quand deux fils l'ouvrent en même temps | S | vérifié |
| COEUR-26 | bug | `gamepads/sdl.rs:280-302` | Arrêt du fil SDL qui se chevauche avec un redémarrage | S | plausible |
| COEUR-27 | bug | `playstation/discovery.rs:290-308` | Windows : un ICMP « port injoignable » arrête toute la recherche de consoles | S | plausible |
| COEUR-28 | bug | `adaptive.rs:168-170` | `TargetBitrate::mbps` déborde près de `u32::MAX` | S | vérifié |
| COEUR-29 | bug | `session.rs` (`Session`) | Pas de `Drop` : lâcher la session sans `stop()` la laisse tourner | S | vérifié |
| COEUR-30 | optimisation | `audio/engine.rs:102-114, 351, 364` | Rappel temps réel du son : verrous bloquants, allocation et libération de gros tampons | M | vérifié |
| COEUR-31 | optimisation | `micro.rs:410-446` | `Microphone::push` (fil temps réel du Mac) : verrous, allocations, création d'encodeur, journal | M | vérifié |
| COEUR-32 | optimisation | `playstation/takion.rs:504, 566-588`, `gkcrypt.rs:106-142`, `frame.rs:188-209` | Trois à six copies ou allocations par paquet d'image, AES-GCM recréé par paquet | M | vérifié |
| COEUR-33 | optimisation | `playstation/takion.rs:280`, `stream.rs:277` | Canaux sans borne entre réception et décodage : retard qui s'accumule | M | plausible |
| COEUR-34 | optimisation | `account.rs:923-935` | Renouvellement du jeton d'accès au relais sous verrou (jusqu'à ≈ 13 s pour tous les appels) | M | vérifié |
| COEUR-35 | optimisation | `account.rs:304-306`, `relay.rs:534-535` | Nouvel agent TLS et nouvelle poignée de main à chaque appel au relais | S | vérifié |
| COEUR-36 | optimisation | `input/pipeline.rs:1288-1299` | Décodage et empreinte de l'image du pointeur sur le fil kynput, sous le verrou du routeur | M | vérifié |
| COEUR-37 | factorisation | `session.rs:2908-3119, 3130, 3309, 4554-4744` | Client Kyber créé 3 fois, console invitée 2 fois, signes de vie 2 fois, fils de préparation 3 fois | M | vérifié |
| COEUR-38 | factorisation | `console.rs`, `lend.rs`, `invite.rs`, `relay.rs`, `account.rs` | Lecture des réponses HTTP écrite 8 fois, avec écarts (`share_wait` perd `Retry-After`) | M | vérifié |
| COEUR-39 | factorisation | `session.rs:1095-1117`, `lend.rs:980-998`, `share.rs:1492-1523` | « Le réseau en 1,5 s, sinon le relais » en trois versions qui divergent | M | vérifié |
| COEUR-40 | factorisation | `autotest.rs`, `profile.rs`, `micro.rs`, `camera.rs`, `audio/opus.rs`, `playstation/*`, `account.rs`, `share.rs` | Petits doublons internes | S | vérifié |
| COEUR-41 | factorisation | apps Swift, Kotlin, Linux, webOS | Règles du cœur refaites dans les apps (textes de l'enregistrement, `format_code`, lien d'invitation, « Définition ramenée ») | M | vérifié |
| COEUR-42 | code mort | `audio/engine.rs:221` | `AudioEngine::set_order` jamais appelé (et sa course) | S | vérifié |
| COEUR-43 | code mort | `session.rs:1520`, `recording.rs:143-190`, divers | `Session::set_name`, textes de l'enregistrement, API `pub` réservées aux essais | S | vérifié |
| COEUR-44 | code mort | `i18n/fr.json`, `i18n/en.json`, `journal.rs:83` | 29 clés du catalogue du cœur lues par personne ; macro `jerror!` inutilisée | S | vérifié |
| COEUR-45 | code mort | `playstation/session.rs:53-58, 605-634`, `takion.rs:60` | Messages et événements PlayStation jamais lus | S | vérifié |
| COEUR-46 | doc | `Cargo.toml:31-37`, `README*.md:318-321`, `docs/pyrowave-client-*.md`, scripts | PyroWave dit « éteint par défaut », alors qu'il est allumé depuis la 0.11.1 | S | vérifié |
| COEUR-47 | doc | `README.md`, `README.en.md` | Sections absentes en anglais, tableau des modules incomplet, caméra non décrite, origine des correctifs incomplète | S | vérifié |
| COEUR-48 | doc | `session.rs:1744, 1546-1553, 3258-3263, 4314-4317`, `playstation/session.rs:18-19`, `README.md:84`, `share.rs:1579-1580`, `input/pipeline.rs:93-96` | Commentaires périmés ou faux | S | vérifié |
| COEUR-49 | doc / dette | `.github/workflows/ci.yml:54-70` | Le cœur n'est ni compilé ni testé par la CI | S | vérifié |

Décompte : **4 bugs critiques, 25 bugs** (dont 2 de sécurité à faible portée et 4 plausibles), **7 optimisations, 5 factorisations, 4 code mort, 4 doc/dette**.

---

## Bugs critiques

### COEUR-01 — PlayStation : boucle sans fin dans le son au passage de l'index par zéro
- **Catégorie** : bug critique. **Où** : `apps/core/src/playstation/audio.rs:90-95` (`key`), `:97-108` (`store`), `:126-144` (`drain`).
- **Description** : la clé de tri est « l'index déroulé autour de la prochaine trame attendue » :
  ```rust
  Some(n) => (n as u32).wrapping_add(index.wrapping_sub(n) as i16 as i32 as u32) ^ 0x8000_0000,
  ```
  `next` est un `u16`. Quand `next == 0xffff`, une trame 0 déjà arrivée est rangée sous la clé `0x8001_0000`. Une fois `next` revenu à 0, `key(0)` vaut `0x8000_0000` : l'entrée rangée ne correspond plus à aucune clé atteignable. Dans `drain`, elle est toujours « en avance » (`ahead > 0`, `newest >= k + PREFILL - 1`) : chaque tour pousse `AudioOut::Hole` et avance `next`. On refait le tour des 65 536 index sans jamais retomber sur la clé orpheline (à `next == 0xffff`, on cherche `key(0xffff)`, pas `key(0)`), et ainsi de suite. `drain` ne rend jamais la main et `out` grossit jusqu'à manquer de mémoire.
- **Scénario** : l'index revient à zéro toutes les 65 536 trames de 10 ms, soit toutes les ≈ 11 minutes. Il suffit qu'une trame soit perdue ou arrive en désordre juste à ce moment (la trame 0, ou sa copie de secours, rangée pendant que `next == 0xffff`) pour figer le fil `ps-play` : plus de son ni d'image, puis l'app est tuée faute de mémoire. Le test `wraps_around` ne garde jamais de trame d'avance au passage.
- **Correctif** : tenir un compteur étendu sur 32 bits (ou 64) qui ne revient jamais à zéro, et calculer la clé `next_ext + (index - next) as i16`. Ajouter une borne de sécurité dans `drain` (au plus `BUFFER_MAX` trous par appel) et jeter les clés inférieures à `k`. Ajouter l'essai « perte sur 0xffff ».
- **Effort** : S. **Confiance** : vérifié (déroulé à la main sur le code).

### COEUR-02 — Horloge de latence fausse sur les cibles 32 bits
- **Catégorie** : bug critique. **Où** : `apps/core/src/latency.rs:61-80`.
  ```rust
  struct libc_timespec { tv_sec: i64, tv_nsec: i64 }
  ...
  ts.tv_sec * 1_000_000 + ts.tv_nsec / 1000
  ```
- **Description** : deux cibles livrées sont 32 bits : webOS (`apps/webos/scripts/build.sh:31`, `armv7-unknown-linux-gnueabi`, glibc 2.12) et Android `armeabi-v7a` (`apps/android/app/build.gradle.kts:30`). Leur `struct timespec` fait 2 × 32 bits. `clock_gettime` n'écrit que 8 octets : `tv_sec` lu en i64 vaut `sec | (nsec << 32)`, `tv_nsec` reste à 0, et `* 1_000_000` déborde (bouclage silencieux en release). La valeur n'est même pas croissante.
- **Scénario** : sur la TV LG et sur un Android 32 bits, `latency::now_micros()` alimente la latence (`apps/webos/src/device/media.rs:70, 289`, `apps/android/jni/src/video.rs:82, 302`), l'expiration des images dans `drain()` (images comptées perdues à tort), et la valeur exportée à Kotlin (`apps/android/jni/src/exports.rs:85`). La latence affichée est absurde ou vide, et les statistiques d'images perdues sont faussées.
- **Correctif** : déclarer `tv_sec: std::ffi::c_long` (ou `i32` sous `target_pointer_width = "32"`) et `tv_nsec: c_long`, puis `ts.tv_sec as i64 * 1_000_000 + ts.tv_nsec as i64 / 1000`. Plus simple : `std::time::Instant` mesuré depuis une origine fixe (`OnceLock<Instant>`), si l'horloge n'a pas à coïncider avec celle d'un autre processus.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-03 — En-tête Opus incohérent : lecture hors limites dans libopus
- **Catégorie** : bug critique. **Où** : `apps/core/src/media.rs:372-378`, puis `apps/core/src/audio/opus.rs:43-50`.
  ```rust
  if family == 0 {
      return Some(Self { channels, streams: 1, coupled: u8::from(channels > 1),
                         mapping: if channels > 1 { vec![0, 1] } else { vec![0] } });
  }
  ```
- **Description** : en famille 0, `channels` (lu dans le paquet) n'est pas borné à 2. `opus_multistream_decoder_create` lit `channels` octets de correspondance depuis `head.mapping.as_ptr()`. Avec `channels = 6`, il lit 4 octets au-delà du tas, avant de rendre l'erreur : comportement indéfini. En familles 1 et 255, rien ne borne non plus `channels` : 255 voies créent un anneau d'une seconde de ≈ 49 Mo.
- **Scénario** : une Station mal corrigée, un correctif futur de Kyber, ou un flux relayé, envoie un `OpusHead` incohérent. La lecture hors limites passe le plus souvent inaperçue, mais peut planter selon l'allocateur.
- **Correctif** : dans `OpusHead::parse`, refuser `family == 0 && channels > 2`, et `channels > 8` pour toutes les familles.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-04 — Micro et caméra perdus après le redémarrage de la Station en dernier recours
- **Catégorie** : bug critique (fonction perdue en silence). **Où** : `apps/core/src/session.rs:2389-2413` (`renew`), avec `:1416-1417` (`launch`) et `:1454-1473`.
- **Description** : `launch` garde dans `Session` les `Arc` du micro et de la caméra du premier `Worker`. `renew()` crée un `Worker` neuf (`Worker::new` crée un `Microphone` et une `CameraLink` neufs) et reporte `bitrate_cap`, `adaptive`, `hdr_used`, `link_mbps`, `watch_display`… mais **ni `microphone` ni `camera`**. La nouvelle tentative pose l'uplink sur les nouveaux objets, tandis que `Session::set_microphone`, `push_microphone` et `camera()` visent encore les anciens.
- **Scénario** : le PC ne donne pas d'image, la Station est redémarrée, la session repart. Le micro de l'app part vers un objet sans uplink (jeté en silence) ; le nouveau micro reste coupé (son état par défaut) et ignore le choix de l'utilisateur ; la caméra n'envoie plus rien. Rien ne le signale à l'utilisateur.
- **Correctif** : dans `renew()`, `next.microphone = self.microphone.clone(); next.camera = self.camera.clone();` (en remettant leur liaison à zéro si nécessaire). Ajouter un essai qui vérifie que `Session::camera()` est bien celle du `Worker` après `renew`.
- **Effort** : S. **Confiance** : vérifié.

---

## Bugs

### COEUR-05 — PlayStation : correction d'erreurs fausse au-delà de 256 unités
- **Où** : `apps/core/src/playstation/fec.rs:51-58`, `frame.rs:19` (`UNIT_SLOTS_MAX = 512`), `frame.rs:136`.
  ```rust
  out.push(GF.inv((i ^ (m + j)) as u8));
  ```
- **Description** : une matrice de Cauchy sur GF(2⁸) n'est valable que pour `k + m ≤ 256`. `frame.rs` accepte jusqu'à 512 unités ; au-delà de 256, `(m + j) as u8` est tronqué et `i ^ (m + j)` peut valoir 0. En debug, `debug_assert!(a != 0)` fait paniquer le fil `ps-stream`. En release, `inv(0)` rend une valeur arbitraire : soit l'inversion échoue, soit une image fausse est rendue avec `recovered = true` (artefacts jusqu'à l'image clé suivante, et données incohérentes passées à `refs.rs`).
- **Scénario** : une image clé de plus de ≈ 358 Ko en réseau local (≈ 294 Ko par le relais) avec au moins une unité perdue ; `takion.rs:238` parle lui-même d'images clés de « plusieurs centaines de Ko ».
- **Correctif** : `if k + m > 256 { return false; }` dans `fec::decode`, et ne compter sur la FEC dans `frame.rs::start` que si `k + m ≤ 256` (sinon image perdue, comme chiaki-ng).
- **Effort** : S. **Confiance** : vérifié (code) ; la fréquence réelle dépend de la taille des images.

### COEUR-06 — PlayStation : le message BIG peut partir sans morceau final
- **Où** : `apps/core/src/playstation/stream.rs:180-191` (`send_split`).
  ```rust
  while mtu < rest.len() + 26 || (mtu < rest.len() + 25 && !first) {
      let n = if first { mtu - 26 } else { mtu - 25 };
      ...
  }
  if !rest.is_empty() { takion.send_data(1, 1, rest, first)?; }
  ```
- **Description** : la seconde clause est incluse dans la première ; la boucle tourne tant que `rest.len() > mtu - 26`. Pour un morceau suivant, elle coupe `mtu - 25` octets : si `rest.len() == mtu - 25`, tout part dans un morceau « pas le dernier » (drapeau 0) et `rest` est vide, donc aucun morceau avec le drapeau 1 (fin de message) n'est envoyé.
- **Scénario** : avec la MTU de secours (1 454 → `mtu` = 1 404), un BIG d'exactement 1 378 + j × 1 379 octets n'est jamais terminé côté console : « pas de BANG » à chaque essai. La taille ne dépend que du réglage (définition, débit, identifiant de session), donc un utilisateur qui tombe dessus échoue à tous les coups.
- **Correctif** : boucler sur `rest.len() > n` (avec `n` selon `first`) et toujours envoyer le dernier morceau après la boucle, même vide, comme `stream_connection_send_big`.
- **Effort** : S. **Confiance** : vérifié (le comportement exact attendu par la console est déduit de chiaki-ng).

### COEUR-07 — PlayStation : découpe de chaîne non ASCII, panique et abandon de l'app
- **Où** : `apps/core/src/playstation/regist.rs:436` (`parse_hex`), `playstation/session.rs:174` (`hex_decode`), `playstation/psn.rs:77` (`percent_decode`).
  ```rust
  u8::from_str_radix(&text[i..i + 2], 16)
  ```
- **Description** : `&text[i..i+2]` panique si l'indice tombe au milieu d'un caractère UTF-8. `parse_hex` lit des en-têtes passés par `from_utf8_lossy` (un octet invalide devient U+FFFD, 3 octets) ; `percent_decode("%aé")` panique aussi (`psn.rs`, adresse de retour de connexion PSN collée par l'utilisateur). Ces fonctions sont appelées depuis des fonctions `extern "C"` des ponts : la panique abandonne l'app.
- **Correctif** : travailler sur `as_bytes()` et décoder les chiffres hexadécimaux octet par octet (une fonction `hex_pair(b0, b1)` commune aux trois fichiers). Profiter de la passe pour corriger `i + 2 < bytes.len()` en `<=` dans `percent_decode` (un `%xx` final n'est pas décodé).
- **Effort** : S. **Confiance** : vérifié.

### COEUR-08 — Pointeur de l'ordinateur : débordement de `width * height`
- **Où** : `apps/core/src/input/pipeline.rs:1318-1321` (`decode_cursor`).
  ```rust
  if width == 0 || height == 0 || raw.len() < (width * height) as usize { ... }
  let pixels = &raw[..(width * height) as usize];
  ```
- **Description** : `width` et `height` sont des `u32` (kynput-rs `lib.rs:1389-1390`). En release, `65537 × 65537` reboucle à 131 073 : le contrôle passe, et `Shape { width: 65537, height: 65537 }` part à l'app avec 131 073 pixels. Côté Apple, `InputCore.swift:336` fait `Data(bytes:count: width*height*4)` : lecture hors limites et plantage. Android : exception de `Bitmap.createBitmap`. En debug : panique dans un rappel `extern "C"`.
- **Scénario** : une annonce anormale (Station boguée, paquet abîmé non détecté).
- **Correctif** : `width.checked_mul(height)`, plus un plafond raisonnable (256 × 256 par exemple), et vérifier `hot_x < width`, `hot_y < height`.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-09 — `PointerLayer::set_host_size` panique sur une taille inférieure à 1
- **Où** : `apps/core/src/input/pointer_layer.rs:346`.
  ```rust
  (x * size.0 / ow).clamp(0.0, size.0 - 1.0)
  ```
- **Description** : seule l'ancienne taille est contrôlée (`ow > 0.0`). `f64::clamp` panique si `min > max`, donc si `size.0 < 1`. La panique a lieu **sous le verrou `inner`**, qui est alors empoisonné : le `lock().unwrap()` suivant (`remote_cursor_changed`, sur le fil kynput) panique à son tour. Via `kn_pointer_set_host_size` (`extern "C"`), l'app est abandonnée tout de suite.
- **Scénario** : couches qui tiennent une position (Apple TV, webOS) ; webOS appelle `set_host_size((d.width as f64, …))` sans borne (`apps/webos/src/device/session.rs:230-231`), alors que la ligne d'avant prévoit `d.width.max(0)`. Un mode d'écran annoncé à 0 suffit.
- **Correctif** : ignorer (garder l'ancienne taille) si `!(size.0 >= 1.0 && size.1 >= 1.0)` (ce qui écarte aussi NaN) ; même garde dans `attach`.
- **Effort** : S. **Confiance** : vérifié (code) ; le déclenchement dépend de l'annonce d'écran.

### COEUR-10 — Mac : retour au repos d'un axe réessayé, avalé par le filtre des manettes
- **Où** : `apps/core/src/input/gamepad_gate.rs:107-111`.
  ```rust
  if !InputPipeline::axis_moved(last, a.value) { return Filtered::verdict(false); }
  state.last_axis.entry(a.id).or_default().insert(axis, a.value);
  ```
- **Description** : le routeur de kynput appelle le filtre **avant** de transmettre le paquet (`third_party/.../kynput/src/router.rs:109-115` : `hook.on_packet`, puis `forward_pkt`). Si la transmission échoue (file kynput pleine, liaison pas prête), le filtre a déjà noté `last_axis = v`. Le réessai de `PacketQueue` repasse par le filtre : `axis_moved(Some(v), v)` est faux, le filtre rend `process_event: false`, le routeur rend `Ok(())`, et la file croit le paquet parti.
- **Scénario** : Mac avec les manettes lues par le cœur (`apps/apple/src/input.rs:414`, `install_gamepad_gate`). Une rafale remplit la file kynput, on lâche le stick : le 0 est perdu et le personnage continue d'avancer sur le PC. C'est précisément ce que la file de secours devait empêcher.
- **Correctif** : laisser toujours passer `value == 0` (la doc du module le promet déjà pour la manette prise), et ne pas dédoublonner dans le filtre les paquets qui viennent de la file du cœur (`gamepad_axis` dédoublonne déjà).
- **Effort** : S. **Confiance** : vérifié.

### COEUR-11 — Mac : stick tenu pendant que la barre est ouverte, perdu à sa fermeture
- **Où** : `apps/core/src/input/pipeline.rs:539-553` (`set_gamepad_captured`).
- **Description** : à la prise de la manette, le cœur envoie 0 pour chaque axe et le note dans le filtre (`gate.note_axis(id, axis, 0)`), mais ne remet pas `self.axes.last` à 0. Pendant la prise, `gamepad_axis` continue de mettre `axes.last` à jour avec la valeur physique (le filtre jette les paquets). À la fermeture, stick toujours à fond : `axis_moved(Some(32767), 32767)` est faux et SDL n'émet rien pour un stick immobile.
- **Scénario** : on ouvre la barre en tenant la gâchette ou un stick, on la ferme sans lâcher : le PC reste à 0 jusqu'au prochain mouvement.
- **Correctif** : à la prise, passer par `gamepad_axes_rest(id)` (qui remet aussi `axes.last`) ; à la libération, renvoyer l'état réel connu du lecteur (`crate::gamepads::global()`).
- **Effort** : S. **Confiance** : vérifié.

### COEUR-12 — Une panique du fil de session laisse tourner les fils de fond
- **Où** : `apps/core/src/session.rs:1427-1432` ; `alive` n'est mis à faux qu'en `:5242` et `:5269`.
  ```rust
  if std::panic::catch_unwind(AssertUnwindSafe(|| worker.run())).is_err() {
      guard_platform.on_status(&Status::Ended { error: Some(t("session.internal", &[])), report: None });
  }
  ```
- **Description** : après une panique (un des nombreux `.expect("fil …")`, un `lock().unwrap()` empoisonné, une panique de plateforme), l'app reçoit `Ended`, mais `alive` reste vrai : le fil `kybernow-heartbeat` continue d'envoyer le signal de vie à Admin (le PC garde son écran virtuel), et `kybernow-foreground` interroge Admin chaque seconde et appelle `platform.on_game_fullscreen`. Admin n'apprend jamais la fin.
- **Scénario** : sur Mac, `SessionEngine.finish()` libère son `userdata` 30 s après `Ended` (`apps/macos/KyberKit/SessionEngine.swift:1352-1354`) : un changement de premier plan ensuite appellerait Swift sur un objet libéré (plausible).
- **Correctif** : cloner `alive` avant le `spawn` ; dans la branche `Err`, le passer à faux puis tenter un `console.end` borné. Mieux : un garde `Drop` dans `Worker::run` qui fait la même chose quoi qu'il arrive.
- **Effort** : S. **Confiance** : vérifié (mécanisme) ; l'usage après libération est plausible.

### COEUR-13 — Préparations en vol non suivies par la fin de session
- **Où** : `apps/core/src/session.rs:4554-4561` (`readapt`), `:4645-4652` (`replan`), `:4734-4744` (`start_restore_mode`), face à `release` (`:5288-5313`).
- **Description** : ces trois chemins lancent `console.prepare(&req)` sur un fil `kybernow-prepare` dont le `JoinHandle` est jeté. `release()` ne diffère la fin dite à Admin que pour `self.inflight`, qui ne vient que de `step()`. Le commentaire de `release` décrit précisément ce risque (« sinon Admin finirait la préparation après la fin »), mais il n'est couvert que pour la mise en route.
- **Scénario** : l'utilisateur ferme la session pendant une adaptation ou un changement d'écrans (`prepare` jusqu'à 40 s). `stream/end` rend les écrans, puis le `prepare` en vol recrée l'écran virtuel sur le PC, qui reste sans session jusqu'à l'expiration du signal de vie. Côté Admin, `stream_prepare` et `stream_end` sont des gestionnaires concurrents (`server/admin/src/api.rs`) ; aucune sérialisation n'a été trouvée (à confirmer par l'audit d'Admin).
- **Correctif** : une fonction unique `spawn_prepare(req, on_done)` qui range son handle dans `self.inflight` (ou une liste), utilisée par les trois chemins et par `step` ; elle pose aussi `journal::resume(label)` (voir COEUR-37).
- **Effort** : M. **Confiance** : vérifié côté cœur ; l'effet chez Admin est plausible.

### COEUR-14 — Réglages perdus pendant l'attente de la Station
- **Où** : `apps/core/src/session.rs:2370-2384` (`stop_requested`).
  ```rust
  Ok(Msg::Settings(s)) => self.params.settings = s,
  Ok(Msg::BitrateCap(cap)) => self.bitrate_cap = cap,
  Ok(_) => {}
  ```
- **Description** : pendant les 90 s de « La Station redémarre », seuls `Settings` et `BitrateCap` sont gardés ; `SetHdr`, `ScreenHdr`, `SetScreens`, `SetMode`, `PickDisplay` sont jetés. `renew()` reclone ensuite `params` et `screens` d'avant. Le commentaire (« les réglages changés entre-temps sont gardés ») est donc inexact.
- **Scénario** : l'utilisateur coupe le HDR ou branche un écran pendant l'attente : la nouvelle tentative part avec l'ancien état, et la barre ne correspond plus à la session.
- **Correctif** : appliquer aussi ces messages à `params`/`screens`, ou les remettre dans le canal comme le fait `step()`.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-15 — Rejoindre un partage : invité fantôme quand le réseau répond après le relais
- **Où** : `apps/core/src/share.rs:1705-1770` (`join_anywhere_with`), `:1789-1801` (`join_link_with`).
- **Description** : la demande sur le réseau local tourne dans un fil `kybernow-partage` ; passé 1,5 s avec une réponse du relais, la boucle part par `relay_join(route)`. Le fil local continue : si Admin accepte cette demande ensuite, le `Heard::Local(Ok(found))` part dans un canal que personne ne lit, et aucun `leave` n'est envoyé.
- **Scénario** : invité sur le même Wi-Fi que l'hôte, Admin lent (> 1,5 s). La Station reçoit un invité accepté par le réseau, qui ne donnera jamais signe de vie, et une demande par le relais. L'invité fantôme occupe une place pendant le délai de grâce d'Admin (20 s, `server/admin/src/activity.rs:34`) : l'hôte le voit, et la vraie demande peut recevoir « complet ».
- **Correctif** : `drop(rx)` avant `relay_join` ; dans le fil, si `tx.send(Heard::Local(Ok(found)))` échoue, appeler `leave(&found)`. Même chose dans `join_link_with` et pour une réponse arrivée après l'échéance dans `join_with`.
- **Effort** : S. **Confiance** : vérifié côté cœur ; l'effet chez Admin est plausible.

### COEUR-16 — Stéréo vers une sortie 5.1/7.1 : la voie droite part au centre
- **Où** : `apps/core/src/audio/mix.rs:91-95` ; essai qui fige le comportement : `mix.rs:121-123`.
  ```rust
  _ => for (c, d) in dst.iter_mut().enumerate() { *d = if c < ch { src[c] * volume } else { 0.0 } }
  ```
- **Description** : `mix_frame` travaille en ordre Opus/Vorbis (G, C, D, SG, SD, LFE). Recopier la stéréo par indice met D en position 1, c'est-à-dire le centre ; la permutation de sortie (`[0, 2, 1, …]` pour Windows et Core Audio) l'envoie ensuite sur l'enceinte centrale, et la droite reste muette. La mono (1 → n) ne va qu'à gauche.
- **Scénario** : Linux / Link OS ouvre la sortie SDL au nombre de voies demandé (réglage HDMI 5.1) ; si l'ordinateur envoie de la stéréo (bureau, jeu stéréo, ou le temps d'une réouverture), la droite est perdue.
- **Correctif** : en montée de voies, placer G en 0 et D en 2 quand `out >= 3` ; mono : le même signal en G et D. Mettre l'essai à jour.
- **Effort** : S. **Confiance** : vérifié (code) ; l'exposition dépend de ce que la Station envoie.

### COEUR-17 — Forme du pointeur périmée face à un PC à plusieurs écrans
- **Où** : `apps/core/src/input/pipeline.rs:1294-1295`.
  ```rust
  let mut shapes = self.last_shapes.lock().unwrap();
  decode_cursor(cursor, shapes.entry(display_id).or_default())
  ```
- **Description** : l'empreinte de la dernière forme est rangée par écran **du PC**, mais avec une session à un écran (`screens.len() <= 1`), tous les écrans du PC vont à la même couche (`pointer_for`).
- **Scénario** : flèche sur l'écran 1, curseur texte sur l'écran 2 (la couche prend le curseur texte), retour sur l'écran 1 en flèche : l'empreinte de l'écran 1 n'a pas changé, donc `image: None`, et la couche garde le curseur texte jusqu'au prochain vrai changement de forme.
- **Correctif** : ranger l'empreinte par couche de destination (clé `Arc::as_ptr(&layer)`), ou une seule empreinte quand il n'y a qu'une couche.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-18 — Texte long tapé ou collé : tronqué, file sans borne, journal inondé
- **Où** : `apps/core/src/input/pipeline.rs:1011-1019` (`type_text`), `input/packet_queue.rs:148-154, 203-209`.
- **Description** : un paquet « fiable » par caractère. Au-delà des 64 places de kynput, tout part dans la file de secours (`VecDeque` sans borne) ; chaque caractère encore en attente après 200 ms est jeté, avec un `log::error!` par caractère (`count_dropped(final_ = true)`).
- **Scénario** : un collage ou une dictée de quelques milliers de caractères dans le champ texte iOS : texte coupé au milieu, des milliers de lignes d'erreur.
- **Correctif** : une file dédiée au texte vidée depuis `poll` au rythme des acceptations, sans délai de 200 ms ; plafonner la longueur ; un résumé de journal par rafale de pertes.
- **Effort** : M. **Confiance** : vérifié (code) ; le déclenchement dépend de l'usage.

### COEUR-19 — Sans empreinte connue, chaque connexion accepte n'importe quel certificat
- **Catégorie** : bug (sécurité, faible portée). **Où** : `apps/core/src/console.rs:111-118` (`PinVerifier`).
  ```rust
  *self.seen.lock().unwrap() = Some(fp.clone());
  match &self.pinned { Some(p) if !p.eq_ignore_ascii_case(&fp) => Err(..), _ => Ok(..) }
  ```
- **Description** : avec `pinned: None`, le premier contact accepte tout (TOFU voulu), mais **toutes** les connexions suivantes de la même `Console` aussi, et `seen` retient la dernière empreinte vue, pas la première.
- **Scénario** : `lend::reach` (`lend.rs:990-1005`) sonde `info` puis envoie `lend_status(login, password)` sur la même console non épinglée ; un lien web (`parse_invite`) n'a jamais d'empreinte. Si la connexion est rouverte entre les deux appels, un tiers sur le réseau peut s'interposer sur la seconde. La fenêtre est étroite (connexion gardée en général).
- **Correctif** : quand `pinned` vaut `None`, garder la première empreinte vue et refuser toute autre ensuite.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-20 — Transfert PlayStation : essais de code sans limite de débit
- **Catégorie** : bug (sécurité, réseau local). **Où** : `apps/core/src/playstation/handoff.rs:186-224` (`accept_loop`) ; comparaisons HMAC `:271` et `:304`.
- **Description** : après `TRIES` codes faux, un nouveau code est tiré, mais rien ne ralentit les essais. Chaque connexion coûte un échange SPAKE2 de l'ordre de la milliseconde ; pendant les 5 minutes d'écoute, une machine du réseau peut tenter des dizaines de milliers de codes à 6 chiffres (≈ 3 à 10 % de chances de tomber juste selon le débit). Effet secondaire : trois trames invalides font changer le code affiché en boucle. Les deux comparaisons de confirmation (`their != confirm(…)`) ne sont pas en temps constant.
- **Correctif** : attendre ≈ 1 s après chaque échec, arrêter l'écoute après 10 échecs au total ; `Mac::verify_slice` pour les confirmations.
- **Effort** : S. **Confiance** : l'absence de limite est vérifiée ; le débit d'attaque est estimé.

### COEUR-21 — Console : erreurs mises de côté, partagées entre les fils
- **Où** : `apps/core/src/console.rs:758-763` (`account_error`, `token_refused`), `:893-895`, `:939`, `:978-997`.
- **Description** : quand le jeton ou le tunnel manque, la raison est rangée dans un `Mutex<Option<…>>` de la `Console`, puis reprise par le prochain `read()` qui voit une erreur de transport, pas forcément celui de l'appel qui l'a rangée. En session, la même `Arc<Console>` sert à plusieurs fils (signe de vie, premier plan, HDR…). `with_fresh_token` remet aussi `token_refused` à faux pour tous.
- **Scénario** : le fil A n'ouvre pas le tunnel (`StationOffline`) et range l'erreur ; le fil B, simple coupure réseau, la reprend ; A obtient une erreur `Network` générique. Texte faux (« éteint ou en veille » contre « Admin injoignable »), et mauvais chemin si la session décide selon le type d'erreur.
- **Correctif** : faire rendre l'erreur par `request_with` (`Result<ureq::Request, ConsoleError>`) au lieu de la ranger dans la console.
- **Effort** : M. **Confiance** : la course est vérifiée ; l'effet visible est plausible.

### COEUR-22 — Le fil du premier plan rappelle l'app après la fin
- **Où** : `apps/core/src/session.rs:3612-3630`.
- **Description** : `alive` est vérifié avant `console.foreground()` (jusqu'à ≈ 7 s avec un renouvellement de jeton), pas après ; `spawn_heartbeat` revérifie, lui (`:3584`). `platform.on_game_fullscreen` peut donc arriver après `Status::Ended`.
- **Correctif** : `if !alive.load(Ordering::Relaxed) { return; }` après l'appel HTTP.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-23 — Rafale de fausses « secondes » après un appel bloquant
- **Où** : `apps/core/src/session.rs:4120-4121`.
  ```rust
  if now >= next_second { next_second += Duration::from_secs(1); ... }
  ```
- **Description** : `restore_screen()` appelle `prepare()` depuis la boucle d'événements et peut la bloquer jusqu'à 45 s. Au retour, `next_second` a N secondes de retard : la boucle émet une « seconde » à chaque tour (≤ 100 ms), N fois, avec des mesures vides.
- **Scénario** : `on_latency` et `on_second` reçus en rafale ; Statistiques faussées (le débit adaptatif est épargné, l'image étant arrêtée pendant ce temps).
- **Correctif** : `next_second = (next_second + Duration::from_secs(1)).max(now)`.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-24 — PlayStation : verrou `ctrl` tenu pendant un `join` et des écritures réseau
- **Où** : `apps/core/src/playstation/play.rs:237-239`, `:251-253` (`if let Some(mut c) = ctrl.lock().unwrap().take() { c.stop(); }`), `:273-278` (`with_ctrl`), `:357-362` (`goto_bed`).
- **Description** : en édition 2024, le temporaire du `if let` vit pendant tout le bloc `then` : le verrou est tenu pendant `c.stop()`, qui joint le fil `ps-ctrl` (jusqu'à 500 ms). `with_ctrl` et `goto_bed` écrivent sur le TCP sous ce même verrou, avec un délai d'écriture de 5 s. Pas d'interblocage (aucun chemin inverse), mais un appel de l'interface (`kn_ps_keyboard_text`, `goto_bed`) peut figer le fil principal jusqu'à 5 s pendant une reprise ou sur un réseau lent.
- **Correctif** : `let c = ctrl.lock().unwrap().take(); if let Some(mut c) = c { c.stop() }` ; pour les envois, cloner un `Arc<Ctrl>` sous le verrou puis écrire hors du verrou.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-25 — Relais : deux tunnels vers la même Station
- **Où** : `apps/core/src/relay.rs:485-502` (`open_with`).
- **Description** : le cœur lit le cache, relâche le verrou, ouvre le tunnel (`block_on`), puis l'insère. Deux fils qui ouvrent la même route en même temps ouvrent deux connexions ; la seconde écrase la première dans le cache, la première vit à part tant qu'elle est tenue.
- **Scénario** : `relay_twin` (PlayStation) et une console de session, ou deux consoles des ponts créées ensemble : deux connexions QUIC au relais pour la même Station.
- **Correctif** : après le `block_on`, relire le cache et garder l'entrée déjà présente ; ou un verrou d'ouverture par clé.
- **Effort** : S. **Confiance** : vérifié (course) ; fréquence plausible.

### COEUR-26 — Lecteur SDL : arrêt qui se chevauche avec un redémarrage
- **Où** : `apps/core/src/gamepads/sdl.rs:294-302` (`stop_now`) puis `:280-288` (démontage).
- **Description** : `stop_now` met `alive = false` et relâche son verrou ; le démontage a lieu plus tard. Un `acquire()` dans l'intervalle lance un second fil ; si celui-ci démarre avant que l'ancien ait fini, l'ancien remet ensuite `RUMBLES`/`EFFECTS` à `None`, appelle `global().forget_all()` et remet `RUNNING` à faux alors que le nouveau tourne.
- **Scénario** : une relance d'image qui tombe pile vers les 5 s d'`IDLE_STOP` : session sans vibrations ni manettes connues jusqu'au prochain arrêt du lecteur.
- **Correctif** : faire le démontage sous le verrou de démarrage, ou un numéro de génération vérifié avant d'écrire l'état global.
- **Effort** : S. **Confiance** : plausible (dépend de l'ordonnancement).

### COEUR-27 — Windows : un ICMP « port injoignable » arrête la recherche de consoles
- **Où** : `apps/core/src/playstation/discovery.rs:290-308`.
  ```rust
  Err(e) if matches!(e.kind(), WouldBlock | TimedOut) => {}
  Err(e) => return Err(e),
  ```
- **Description** : sous Windows, `recv_from` sur un socket UDP rend `WSAECONNRESET` après un envoi qui a reçu un ICMP « port injoignable ». `search` envoie en direct vers 9302 et 987 pour chaque adresse connue ; une seule adresse qui répond par ICMP (PC qui a repris l'IP d'une console, port PS4 sur une PS5) arrête toute la recherche, et l'app Windows affiche une liste vide (`apps/windows/src/ui/playstation.rs:180, 440, 615` : `unwrap_or_default()`), même si une console avait déjà répondu.
- **Correctif** : traiter `ConnectionReset` et `ConnectionRefused` comme non fatals ; ou couper `SIO_UDP_CONNRESET` sur le socket.
- **Effort** : S. **Confiance** : plausible (comportement Windows connu ; l'ICMP en pratique n'est pas vérifié).

### COEUR-28 — `TargetBitrate::mbps` déborde
- **Où** : `apps/core/src/adaptive.rs:168-170` : `(bitrate + 500_000) / 1_000_000`.
- **Description** : le débit peut valoir `u32::MAX` par `saturating_mul` (`session.rs:1197`, `:1200`, `:5376` : plafond multi-écrans). L'addition déborde : panique en debug, « 0 Mbit/s » affiché en release.
- **Correctif** : `((u64::from(bitrate) + 500_000) / 1_000_000) as u32`.
- **Effort** : S. **Confiance** : vérifié (cas limite : PyroWave multi-écrans au-delà de 4 294 Mbit/s).

### COEUR-29 — `Session` sans `Drop`
- **Où** : `apps/core/src/session.rs` (`struct Session`), branche `Disconnected` de `:4075`, `:2228`, `:2373`.
- **Description** : le `Worker` tient son propre clone de `tx` (`launch`, `:1412`), et `Session` n'implémente pas `Drop`. Lâcher tous les `Arc<Session>` sans `stop()` ne ferme donc pas le canal : la session continue (image, signal de vie) jusqu'à une erreur de Kyber, et la branche `Disconnected` ne peut jamais servir. Les ponts Apple et Android appellent bien `stop`, mais rien ne l'impose.
- **Correctif** : `impl Drop for Session { fn drop(&mut self) { self.stop() } }`.
- **Effort** : S. **Confiance** : vérifié (code) ; une fuite réelle dépend des ponts.

---

## Optimisations

### COEUR-30 — Rappel temps réel du son : verrous bloquants et gros tampons
- **Où** : `apps/core/src/audio/engine.rs:351` (`self.current.lock().unwrap()`), `:364` (`self.order.lock()`), `:114` (`vec![0.0; MAX_FRAMES * channels]`), `:102` (abandon de l'ancien `Arc<Stream>`).
- **Description** : `fill` est appelé par le fil temps réel de l'app (Core Audio, AAudio, WASAPI, SDL). À chaque changement de flux (`configure`, `stop`), il prend `current` et `order` de façon bloquante (`open()` les prend aussi, sous le verrou `producer`), alloue des tampons et libère l'ancien flux (anneau d'une seconde : jusqu'à ≈ 1,5 Mo) dans le rappel. Cela contredit la doc (`audio/mod.rs:19-21`, `engine.rs:7-9` : « sans verrou bloquant »). Un verrou `current` empoisonné ferait paniquer le fil audio à travers `extern "C"`.
- **Gain** : plus de craquement ni de rappel en retard au début du flux et aux passages stéréo ↔ 5.1.
- **Correctif** : `try_lock` sur `current` (silence et nouvel essai au rappel suivant) ; tampon `plc` préalloué pour 8 voies ; garder l'ancien `Arc<Stream>` jusqu'au `configure` suivant ou le rendre au producteur pour qu'il le libère.
- **Effort** : M. **Confiance** : vérifié.

### COEUR-31 — `Microphone::push` sur le fil temps réel du Mac
- **Où** : `apps/core/src/micro.rs:410-446` ; appelant : `apps/macos/KyberKit/AudioPlayerCore.swift:411-415` (« `kn_session_push_microphone` ne bloque pas »).
- **Description** : `push` prend les verrous `link` et `pipe` de façon bloquante, envoie sous `pipe`, crée l'encodeur Opus au premier appel (`:416-424`, avec un `log::warn`), alloue deux `Vec` par trame de 10 ms (`:160`, `:435`), plus un `Vec` par appel au-dessus de 48 kHz (`:205-214`). `set_enabled`, `send_start`, `set_uplink` et `changed` prennent les mêmes verrous depuis d'autres fils ; `set_uplink` libère l'ancien `Uplink` sous verrou. Pas d'interblocage (ordre toujours `pipe` puis `uplink`), mais le commentaire Swift est faux.
- **Gain** : plus d'inversion de priorité ni de trous de capture au (re)démarrage de l'image ou au changement de micro.
- **Correctif** : `try_lock` partout dans `push` (échantillons jetés si un verrou est pris) ; encodeur créé dans `set_enabled` ; paquet encodé directement dans un tampon réutilisé avec l'en-tête en place. Ou une file sans verrou vers un fil d'encodage.
- **Effort** : M. **Confiance** : vérifié.

### COEUR-32 — PlayStation : copies et allocations par paquet d'image
- **Où** : `playstation/takion.rs:504` (`buf.to_vec()` pour le seul GMAC), `gkcrypt.rs:106-116` (`key_stream` alloue un `Vec` par déchiffrement), `gkcrypt.rs:142` (`AesGcm::new` à chaque paquet alors que la clé GMAC ne change que tous les 45 000 octets), `frame.rs:209` (`p.data.clone()` imposé par `push(&AvPacket)`), `frame.rs:188` (`Vec::new()` sans `reserve`), `takion.rs:566, 588` (verrou `remote` pris deux à trois fois par paquet).
- **Gain** : à 100 Mbit/s (≈ 9 000 paquets/s), trois allocations, ≈ 3 copies de 1,4 Ko et une expansion de clé AES de moins par paquet. Utile sur Raspberry Pi, webOS et Android d'entrée de gamme.
- **Correctif** : GMAC sur place (les octets mis à zéro ne sont plus lus ensuite) ; flux de clé dans un tampon réutilisé ; `AesGcm` gardé tant que la clé ne change pas ; `push(AvPacket)` par valeur ; `reserve(k * unit_size)`.
- **Effort** : M. **Confiance** : vérifié (code) ; gain non mesuré.

### COEUR-33 — PlayStation : canaux sans borne entre réception et décodage
- **Où** : `playstation/takion.rs:280`, `stream.rs:277` (`mpsc::channel()`), consommés par `play.rs:549-577` qui appelle `video.packet` de façon synchrone.
- **Description** : si le décodeur ou le son de l'app bloque un moment (app en arrière-plan, décodeur occupé), les paquets s'accumulent sans limite (2 à 12 Mo/s) puis sont décodés avec des secondes de retard. COEUR-01 fait aussi grossir ces files.
- **Correctif** : `sync_channel` borné côté `ps-takion` en jetant les plus anciens, puis demande d'image clé.
- **Effort** : M. **Confiance** : plausible (dépend du comportement de chaque `PacketHandler`).

### COEUR-34 — Jeton d'accès renouvelé sous verrou
- **Où** : `apps/core/src/account.rs:923-935` (`StationAccess::token`), appelé depuis `console.rs:935` à la construction de chaque requête.
  ```rust
  let mut cache = self.cache.lock()...;
  ...
  let fresh = self.fetch()?;   // HTTPS vers le relais : 5 s de connexion + 8 s
  ```
- **Description** : toutes les ≈ 90 s, le premier appel bloque tous les autres appels de la session pendant le renouvellement, et ce temps échappe au délai annoncé de chaque appel (`foreground()` : 2 s, `console.rs:1142`).
- **Gain** : signe de vie jamais retardé de ≈ 13 s par un relais lent (Admin considère l'appareil parti après 20 s).
- **Correctif** : renouveler en avance (≈ 60 s) sur un fil à part en servant le jeton courant jusqu'à son échéance ; un seul renouvellement à la fois.
- **Effort** : M. **Confiance** : verrou vérifié ; l'impact est plausible.

### COEUR-35 — Nouvel agent TLS à chaque appel au relais
- **Où** : `apps/core/src/account.rs:304-306` (`UreqHttp::call`), `relay.rs:534-535` (`lookup`).
- **Description** : chaque appel refait `kybernow_tunnel::tls::client_config` (copie des racines webpki) et un `ureq::Agent` neuf, donc une connexion TCP + TLS complète à chaque fois : toutes les 5 s pendant l'attente du code TV, toutes les 90 s pour le jeton, à chaque liste d'ordinateurs ou de réglages.
- **Correctif** : un `Agent` par `RelayAddr` gardé dans un `OnceLock<Mutex<HashMap<…>>>`.
- **Effort** : S. **Confiance** : vérifié.

### COEUR-36 — Image du pointeur décodée sous le verrou du routeur kynput
- **Où** : `apps/core/src/input/pipeline.rs:1288-1299`, puis `PointerLayer::changed` et le rappel de l'app.
- **Description** : à chaque annonce avec image, `Cursor::from_packet` copie toute l'image, puis le cœur calcule une empreinte SipHash de tous les pixels ; quand la forme change, l'app prémultiplie et crée sa CGImage / son Bitmap. Tout cela tient le verrou du routeur, donc bloque chaque touche ou clic envoyé pendant ce temps. Négligeable avec le correctif serveur 0006 (positions sans image), sensible avec un pointeur animé (sablier).
- **Correctif** : copier le paquet et traiter l'image sur le fil des entrées (comme le presse-papiers) ; au minimum une empreinte moins chère.
- **Effort** : M. **Confiance** : vérifié.

---

## Factorisation

### COEUR-37 — `session.rs` : quatre blocs dupliqués
- **Où et quoi** :
  - Client Kyber créé trois fois à l'identique (`install_default` de rustls, `Client::new`, `set_metrics_callback`, `add_event_sink`, `connect`) : `session.rs:2908-2948`, `:3030-3051`, `:3099-3119`.
  - Console invitée / emprunteuse construite deux fois : `:2986-2995`, `:3064-3074`.
  - Signes de vie du partage et du prêt : même boucle (`sleep`, `alive`, `last_ok`, `GUEST_BEAT_LIMIT`) dans `spawn_share_beat` (`:3309`) et `spawn_lend_beat` (`:3130`).
  - Fils `kybernow-prepare` en trois copies (`:4554`, `:4645`, `:4734`) ; deux ne posent pas `journal::resume(label)`, donc leurs lignes perdent le nom de l'ordinateur (de même pour le fil `kybernow-hdr`, `:2755`).
- **Gain** : moins de risques de dérive ; la factorisation des préparations corrige au passage COEUR-13.
- **Correctif** : `fn connect_kyber(&mut self, config)`, `fn guest_console(…)`, `fn spawn_guest_beat(kind, …)`, `fn spawn_prepare(…)`.
- **Effort** : M. **Confiance** : vérifié.

### COEUR-38 — Lecture des réponses HTTP écrite huit fois
- **Où** : `console.rs:1263-1266` (`station_call`), `:1313-1314`, `:1402-1406` (`share_join_answer`), `:1430-1433` (`share_wait`), `lend.rs:876-881` (`lend_read`), `invite.rs:197-202` (`pair_invite`), `relay.rs:544-547` (`lookup`), `account.rs:316-324`.
- **Description** : le même motif `Status(status, resp) => { Retry-After ; into_json().unwrap_or(Null) ; xxx_refusal(..) }`, plus quatre fois `Err(e) => X::Other(self.read::<Value>(Err(e)).err().unwrap_or_else(…))`. Écart constaté : `share_wait` (`console.rs:1432`) passe `None` comme `retry_after`, alors que `share_join` lit l'en-tête : un 429 sur l'attente perd son délai.
- **Correctif** : une méthode `Console::call_raw(req) -> Result<Reply, ConsoleError>` qui reprend `account::Reply { status, body, retry_after }` ; chaque module garde seulement sa fonction de refus.
- **Effort** : M. **Confiance** : vérifié.

### COEUR-39 — « Le réseau en 1,5 s, sinon le relais » en trois versions
- **Où** : `session.rs:194` et `:1095-1117` (`choose_route`, `DIRECT_PROBE`), `lend.rs:21` et `:980-998` (`reach`, son propre `DIRECT_PROBE`), `share.rs:1492-1523` (`watch_account`) ; `share.rs:1569` définit encore `LOCAL_FIRST` à 1 500 ms.
- **Divergences** : Admin sans jeton (`ConsoleError::Account`) → la session part par le relais, `watch_account` rend l'erreur ; certificat changé → la session et `watch_account` s'arrêtent, `reach` reste sur le réseau ; `KYBERNOW_RELAY` traité dans un ordre différent.
- **Correctif** : une seule fonction `relay::choose(route, mode, probe)` et une seule constante ; décider une fois pour toutes du comportement sur certificat changé.
- **Effort** : M. **Confiance** : vérifié.

### COEUR-40 — Petits doublons internes
- `autotest.rs:498-504` recalcule `profile::pyrowave_fallback(caps)` (`profile.rs:491-499`) ; `autotest.rs:413-416` (`scale`) reprend la règle de `profile.rs:318-319` (puissance 0,75 bornée de 0,4 à 3 autour de la 4K60) : une fonction partagée dans `profile`.
- `micro.rs:245-248` et `camera.rs:35-39` redéfinissent les constantes du flux montant (START, MEDIA, PAUSE, STATUS) avec deux `parse_status` JSON très proches : un petit module `uplink_proto`.
- Deux blocs `extern "C"` libopus : `micro.rs:116-121` et `audio/opus.rs:9-28`.
- `seq16_lt` écrite deux fois (`playstation/frame.rs:63`, `playstation/audio.rs:38`), à côté de `gkcrypt::seq32_lt`.
- Décodage d'adresse en double : `account.rs:445-511` (`callback_query`, `decode`) et `share.rs:1348-1389` (`query_pairs`, `percent_decode`) ; et un troisième `percent_decode` dans `playstation/psn.rs:70` (voir COEUR-07).
- `welcome.rs:103-107` (`sign_up_url`) normalise le relais à la main au lieu d'`account::relay_label` ; avec un relais épinglé (`relais.org:9443#sha256:…`), l'adresse produite serait invalide. Latent : tous les appels actuels passent `""`.
- **Effort** : S chacun. **Confiance** : vérifié.

### COEUR-41 — Règles du cœur refaites dans les apps
- **Textes de l'enregistrement** : le cœur a `RecordingState::label`, `tooltip`, `tile`, `menu_item` (`recording.rs:143-190`) mais toutes les apps passent par `ui_json` et refont le choix : `apps/macos/KyberKit/RecordingCore.swift:64-75`, `apps/linux/src/app.rs:1676-1702`, `apps/webos/src/app.rs:1362-1383`, Kotlin `Recording.kt:78` (qui refait aussi l'horloge, alors que Swift appelle `kn_recording_clock`). Choisir un seul endroit (le plus simple : supprimer les méthodes du cœur, voir COEUR-43, et désigner `ui_json` comme référence dans `docs/link-ui.md`).
- **`format_code`** : `share.rs:1252`, recopié dans `apps/linux/src/app.rs:1235` et `apps/webos/src/app.rs:965`, chacun avec son essai. Cause : l'interface Linux/webOS compile sans le cœur (fonctionnalité `device`). Sortir les petites fonctions pures dans une crate sans dépendances, ou noter la copie dans `docs/link-ui.md`.
- **Lien d'invitation** : le cœur lit `kybernow://borrow` et `https://…/link?borrow=` (`lend.rs:815`, `parse_invite`) ; Swift (`LendCore.swift:158-163`) reconnaît les deux, Kotlin (`Lend.kt:436`, `isInviteLink`) seulement `kybernow://borrow`. Sur Android, un lien web illisible est ignoré sans le message « Ce lien d'invitation est illisible. ». Ajouter `lend::looks_like_invite` au cœur et l'exposer au pont C et en JNI.
- **« Définition ramenée »** : `pyrowave.rs:103` (`Lowered::text`) recopié dans `apps/macos/KyberNow/LinkUI.swift:1563`, utilisé par les Statistiques Mac et iOS, parce que le pont ne donne que les nombres (`apps/apple/src/lib.rs:712-716`). Ajouter le texte à `KnSecond` ou une fonction `kn_pyrowave_lowered_text`.
- **Effort** : M au total. **Confiance** : vérifié des deux côtés.

---

## Code mort

Chaque symbole a été cherché dans tout le dépôt hors `third_party` et `target` (Rust, Swift, Kotlin, C#, TS/Svelte, en-têtes, scripts).

### COEUR-42 — `AudioEngine::set_order` jamais appelé
- **Où** : `apps/core/src/audio/engine.rs:221-226`. Seule occurrence du dépôt. L'ordre des voies est fixé à la construction (`apps/apple/src/audio.rs:40-42`, `apps/android/jni/src/audio.rs:41`, `AudioEngine::with_order`).
- **Remarque** : la fonction contient en plus une course (si `fill` tient le consommateur, `try_lock` échoue, `output_channels` n'est pas remis à 0 et la nouvelle permutation n'est jamais appliquée à nombre de voies égal). Si on la branche un jour (changement de sortie en cours de session), la corriger avec un compteur de génération comparé dans `fill`.
- **Correctif** : la supprimer, ou la brancher (pont C `kn_audio_set_order`, JNI) avec le compteur. **Effort** : S. **Confiance** : vérifié.

### COEUR-43 — API publiques mortes ou réservées aux essais
- `Session::set_name` (`session.rs:1520`) : jamais appelée, alors que `session.rs:616`, `README.md:181` et `README.md:270` (et l'anglais) la présentent comme la source du nom ; le nom vient de l'inscription au focus (`focus.rs:158`).
- `RecordingState::label`, `tooltip`, `tile`, `menu_item` (`recording.rs:143-180`) : seulement les essais ; `guest_label`, `guest_tooltip` (`:182-190`) : nulle part.
- Réservées aux essais, ou nulle part :

| Élément | Où | Usage hors définition |
|---|---|---|
| `LatencyTracker::drain_second` | `latency.rs:549` | essais |
| `Camera::is_sending`, `CameraResolution::from_label` | `camera.rs:282`, `:88` | essais |
| `Console::station_access` | `console.rs:807` | essai |
| `share::check_codec` | `share.rs:1200` | essais (la doc dit « la barre peut s'en servir ») |
| `pyrowave::floor_mbps` | `pyrowave.rs:66` | essais |
| `InputPipeline::core_reads_gamepads` | `input/pipeline.rs:733` | aucun (cité par `README.md:195`) |
| `LendCodeView::relay_connecting` | `lend.rs:716` | essai |
| `PadButton::from_kynput`, `PadAxis::from_kynput` | `input/pad_combo.rs:101`, `:171` | essais |
| `keymap::is_modifier` | `input/keymap.rs:60` | essai |
| `relay::open_tunnels` | `relay.rs:516` | aucun |
| `ManualClock::starting_at` | `input/clock.rs:44` | aucun |
| `Ring::capacity` | `audio/ring.rs:30` | aucun |

- Constantes jamais lues : `account/settings.rs:35` `MAX_BYTES` (le relais applique la limite, le cœur ne vérifie pas avant d'envoyer : la brancher ou l'enlever), `input/clipboard.rs:53` `MAX_TEXT`, `lend.rs:24` `COME_BACK`. Les tables de protocole (`keymap.rs:43-47`, `playstation/endpoint.rs:20`) peuvent rester : elles documentent les codes.
- **Correctif** : supprimer, ou passer en `pub(crate)` / `#[cfg(test)]`. **Effort** : S. **Confiance** : vérifié.

### COEUR-44 — 29 clés i18n du cœur lues par personne ; macro `jerror!` inutilisée
- **Où** : `apps/core/src/i18n/fr.json`, `en.json` (263 clés, les deux catalogues sont identiques en clés).
  - 11 clés copiées du catalogue de l'interface Windows, seul lu par Svelte : `autotest.{title, subtitle, summary, apply, cancel, again, retry, chosen_by, adaptive, adaptive_help, adaptive_pyrowave}`.
  - 12 clés copiées de l'interface Windows et/ou de `server/web`, lues seulement là : `micro.{label, label_off, tooltip_on, tooltip_off, setting_help, app_setting, system_default, echo, echo_help, guests, guests_help, permission_web}`.
  - 6 clés lues nulle part (Mac/iOS prennent `String(localized:)`) : `autotest.phrase`, `micro.permission`, `micro.permission_mac`, `micro.tile_on`, `micro.tile_off`, `micro.tile_waiting`.
  - Les clés construites par `format!` (`autotest.reason.*`, `autotest.step.*`) ont été prises en compte ; aucun pont n'expose `i18n::tr` avec une clé arbitraire.
- `journal.rs:83` : `jerror!` jamais utilisée, masquée par `#[allow(unused_macros)]`.
- `check-i18n.mjs` compare les deux langues, jamais l'usage réel des clés.
- **Correctif** : retirer ces clés et la macro. **Effort** : S. **Confiance** : vérifié.

### COEUR-45 — PlayStation : messages et événements jamais lus
- `playstation/session.rs:53` `LOGIN_PIN_REP`, `:58` `GO_HOME`, `takion.rs:60` `PAD_INFO` : jamais employés.
- `CtrlEvent::LoginPinRequested`, `Login`, `Other` sont émis (`playstation/session.rs:605, 608, 634`) mais jamais lus (`play.rs:530` : `_ => {}`) ; `Other` copie en plus chaque charge utile.
- `Ctrl::server_type` et `prohibited` ne servent qu'à `examples/playstation.rs`.
- **Correctif** : retirer `Other` (ou ne pas copier la charge), garder les constantes de protocole si elles documentent. **Effort** : S. **Confiance** : vérifié.

---

## Doc et dette

### COEUR-46 — PyroWave dit « éteint par défaut » partout
- **Où** : `apps/core/Cargo.toml:31-37` ; `apps/core/README.md:320-321` et `README.en.md:318` (« Fonctionnalité Cargo `pyrowave` (éteinte par défaut) … À allumer (par défaut) quand le correctif sera dans `kyber-desktop` ») ; `apps/core/scripts/build-apple.sh:28-31`, `apps/android/scripts/build-native.sh:38-41`, `apps/apple/scripts/build-apple-mobile.sh:23-29`, `apps/windows/Cargo.toml:15-18` ; `docs/pyrowave-client-apple.md:26, 86`, `docs/pyrowave-client-linux.md:11, 32`, `docs/pyrowave-client-pi.md:88`, `docs/pyrowave-client-windows.md:62`, `docs/pyrowave-client-android.md:14`.
- **Réalité** : les scripts l'allument par défaut depuis f261389a (`KYBER_PYROWAVE=1`), le correctif 0025 est posé par `scripts/kyber-client-patches.sh` ; `docs/build-notes.md:36` le dit correctement (« PyroWave par défaut dans les apps (depuis la 0.11.1) »). webOS ne l'active pas.
- **Correctif** : « allumé par défaut par les scripts (correctif 0025) ; `--sans-pyrowave` pour s'en passer ; webOS sans », en FR et EN. **Effort** : S. **Confiance** : vérifié.

### COEUR-47 — README du cœur incomplets
- `README.en.md` n'a pas : la section « Accueil et compte KyberNow » (`README.md:357-359`, `src/welcome.rs`), le paragraphe « Côté hôte, hors de chez soi » du relais (`README.md:276`), le paragraphe du test `autotest::run(…)` (`README.md:349` ; l'anglais renvoie au français, `README.en.md:345`).
- Le tableau « Organisation du code » (`README.md:9-36`, `README.en.md:9-36`) oublie `autotest.rs`, `camera.rs`, `pyrowave.rs`, `station_offer.rs`, `welcome.rs` ; `camera.rs` (748 lignes) n'est décrit nulle part, alors que le micro a sa section (`README.md:465`).
- `README.md:7` / `README.en.md:7` ne citent que `apps/macos/kyber-patches` comme origine des correctifs Kyber ; `scripts/kyber-client-patches.sh:2-3` pose aussi `server/kyber-patches/*-kyclient-*` (0022 HDR, 0025 PyroWave), dont le cœur dépend.
- `README.md:181`, `:270` (`Session::set_name`) et `:195` (`core_reads_gamepads` présenté comme condition de démarrage du fil SDL) décrivent des API mortes (COEUR-43).
- **Effort** : S. **Confiance** : vérifié.

### COEUR-48 — Commentaires périmés ou faux
- `session.rs:1744` : « Relances faute d'image (jamais remis à zéro dans la session) », contredit depuis 26b937ac (`watch_image` remet `no_image` à 0).
- `session.rs:1546-1553` : la doc de `keyframe_lost` est collée en tête de celle de `set_share_quality` ; `keyframe_lost` (`:1588`) n'en a plus.
- `session.rs:3258-3263` : deux paragraphes redondants pour `kyber_endpoint`.
- `session.rs:4314-4317` : `return None.or_else(|| { let _ = self.tx.send(Msg::Stop); None });` est une tournure obscure pour `let _ = self.tx.send(Msg::Stop); return None;`.
- `playstation/session.rs:18-19` : Takion « n'est pas encore écrit » (il l'est : `takion.rs`).
- `apps/core/README.md:84` (et anglais) : « `send_data` découpe comme chiaki-ng » (la découpe est dans `stream::send_split`) et « `send_feedback` … (plus tard) » (`feedback_loop` l'emploie déjà).
- `share.rs:1579-1580` : « Bloquant (8 s au plus) » pour `join_anywhere` ; après la collecte, l'ouverture du tunnel (`STATION_WAIT` 3 s + connexion) et `share_join` (6 s) portent le total vers ≈ 17 s.
- `input/pipeline.rs:93-96` : `InputPlatform::navigate` « sur le fil de kynput » ; avec les manettes du cœur, c'est le fil `kybernow-manettes` (ou tout fil qui envoie), **sous le verrou de `PacketQueue` et celui du routeur**. Documenter la règle qui a manqué en 26b937ac : « aucun rappel (`OnChanged`, `InputPlatform`, `AppFeedback`) ne doit envoyer d'entrée de façon synchrone » ; aujourd'hui tous les ponts repassent en asynchrone, mais rien ne l'impose.
- `audio/mod.rs:19-21`, `engine.rs:7-9`, `AudioPlayerCore.swift:415` : « sans verrou bloquant » (voir COEUR-30 et COEUR-31).
- **Effort** : S. **Confiance** : vérifié.

### COEUR-49 — Le cœur n'est ni compilé ni testé par la CI
- **Où** : `.github/workflows/ci.yml:54-70` : `cargo test` et `cargo check` pour `server/admin`, `server/installer`, `server/tray` seulement.
- **Description** : aucun `cargo test -p kybernow-core`, ni avec `--features pyrowave` ou `sdl` (que `README.md:505` présente comme la vérification du fil SDL), ni l'exemple `playstation`. Plusieurs constats ci-dessus (COEUR-01, 05, 06, 16) auraient un essai simple à ajouter, qui ne tournerait pas sans CI.
- **Correctif** : un job `cargo test -p kybernow-core` (macOS, avec les correctifs Kyber posés par `scripts/kyber-client-patches.sh`, puis `--features pyrowave`). Une cible 32 bits (`cargo check --target armv7-…`) aurait aussi attrapé COEUR-02 si l'on y ajoutait un essai de croissance de `now_micros`.
- **Effort** : S (M si les correctifs Kyber sont lourds à poser en CI). **Confiance** : vérifié.

---

## Les 10 priorités

1. **COEUR-01** — boucle sans fin du son PlayStation au passage par zéro (gel puis manque de mémoire toutes les ≈ 11 min si une trame manque au mauvais moment).
2. **COEUR-02** — horloge fausse sur webOS et Android 32 bits (latence, pertes et statistiques faussées sur la TV LG).
3. **COEUR-04** — micro et caméra perdus après un redémarrage de la Station (deux lignes dans `renew`).
4. **COEUR-03** — `OpusHead` non borné : lecture hors limites dans libopus.
5. **COEUR-07, 08, 09** — trois paniques sur données extérieures qui abandonnent l'app (aucun pont ne rattrape les paniques) ; à traiter ensemble, puis envisager un `catch_unwind` systématique dans les fonctions `extern "C"` des ponts.
6. **COEUR-06 et COEUR-05** — BIG sans fin de message (échec systématique pour certains réglages) et FEC au-delà de 256 unités.
7. **COEUR-10 et COEUR-11** — sticks et gâchettes qui restent poussés sur le Mac.
8. **COEUR-12 et COEUR-13** — fin de session incomplète (panique du fil, préparations en vol) : la Station garde un écran virtuel sans session.
9. **COEUR-30 et COEUR-31** — verrous et allocations sur les fils temps réel du son et du micro.
10. **COEUR-49** — brancher les essais du cœur dans la CI, pour que les correctifs ci-dessus restent acquis.

Ensuite, en une passe de nettoyage : COEUR-15, 16, 19, 20, 24, puis la factorisation (COEUR-37 à 39) et le code mort et la doc (COEUR-42 à 48).

## Ce qui est sain et n'a pas besoin d'être touché

- **Pas d'autre interblocage du type 26b937ac.** Dans `session.rs`, `focus.rs`, `screens.rs`, le son, la caméra, le relais et la PlayStation, aucun rappel vers l'app ni appel bloquant vers kyclient/kynput n'est fait sous un verrou du cœur. `PointerLayer` envoie désormais hors de son verrou (`Pending`), ses fermetures ne tiennent que des `Weak` (pas de cycle d'`Arc`). Le presse-papiers copie les paquets hors du verrou du routeur puis les traite dans `poll`. L'ordre des verrous est constant (`Focus.inner` → `FocusState.input`/`audio` → `switch` → file → routeur ; `pipe` → `uplink` pour le micro). Seule réserve : la règle n'est pas écrite (COEUR-48).
- **kyclient** : `Events::on_event`, appelé sous le verrou de Kyber, ne fait que pousser dans un canal ; les appels du fil de session à Kyber sont de simples envois de message, et le seul appel bloquant (`disconnect_and_stop`) est isolé et borné à 5 s.
- **Mise en route (`step`)** : bornée, annulable, messages remis dans l'ordre, fin différée après un appel abandonné, préparation plus récente détectée (`last_prepare`).
- **Tampon circulaire du son (`audio/ring.rs`)** : ordre mémoire correct (Release/Acquire), cas plein et vide bornés, compteurs en `wrapping_*`.
- **Décodeur Opus, `media.rs`** : longueurs de décodage bornées, `image_mark`/`sei_mark` sûrs face au réseau, `Drop` correct (hors COEUR-03).
- **Débit adaptatif, PyroWave, autotest** : pas de division par zéro ni de NaN, arithmétique en `saturating_*`, annulation de l'autotest vérifiée entre chaque étape.
- **Mises à jour (`update.rs`)** : signature minisign vérifiée avant toute lecture du manifeste (sans mode hérité), commentaire signé contrôlé, version du manifeste égale à celle de la release, lectures plafonnées, taille et SHA-256 vérifiées avec suppression en cas d'écart.
- **Réseau du compte et du relais** : tous les appels `ureq` ont un délai ; runtime tokio créé une seule fois, aucun `block_on` depuis un fil tokio ; le relais ne fait jamais de premier contact (empreinte d'Admin obligatoire, réponses signées vérifiées) ; jetons et mots de passe masqués dans les `Debug` et absents des journaux et des adresses ; pas d'`unwrap` sur du JSON ou du HTTP hors essais.
- **PlayStation** : lecture des en-têtes Takion (`parse_av`, `parse_message`, `packet_mac`, `check_mac`) bornée ; toutes les allocations venues du réseau plafonnées (canal de contrôle 64 Ko, en-têtes 8 Ko, corps 4 Ko, trames du transfert 1 Mo, unités 512 × 64 Ko) ; positions de clé sans réutilisation de nonce ; fils et sockets arrêtés et joints dans les `Drop` ; DATA remis en ordre avec `seq32_gt`.
- **Entrées** : relâchements toujours transmis (même session inactive), `release_all` hors verrou, lecture des événements SDL conforme à SDL 2.26, propriété des paquets kynput explicite (`OwnedPacket`).
- **Fonctionnalités cargo et dépendances** : `sdl` et `pyrowave` branchées et cohérentes avec les scripts ; les 22 dépendances sont utilisées ; les échantillons `.kns` sont embarqués et testés ; aucun TODO/FIXME dans le cœur.
