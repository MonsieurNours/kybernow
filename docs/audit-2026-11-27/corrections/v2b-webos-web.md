# Vague 2, phase 2b : app LG webOS et client web

Branche `claude/v2b-webos-web-84bb06`, partie de `main` à `d0d5e315`. Plan : [`../vague-2.md`](../vague-2.md). Guide : [`../migration-coeur-2b.md`](../migration-coeur-2b.md), sections « Téléviseurs LG » et « Client web ». Règles d'interface : `docs/link-ui.md` de la branche des docs (`claude/v2-docs-0db24c`, pas encore dans `main`) et `docs/relais.md` (« Dépôt d'un journal »), avec les précisions de la session du relais (`claude/v2b-relais-b213a0`, commit `13469bea` : lignes « Appareil : » et « Journal écrit le », vecteurs d'essai KNJ1). Zone : `apps/webos` (hors correctifs Kyber), `server/web` (hors `node_modules` et `dist`), `apps/Cargo.lock`, et `docs/pyrowave-client-web*.md` (la doc du décodeur du client web).

## App LG webOS

### Fait

| Constat | Correction |
|---|---|
| Guide, LG | L'interface dépend de **`kybernow-rules`** (non facultatif) : `parse_address` (port exigé ; une IPv4 entre crochets n'est plus une adresse, la copie d'avant la prenait), `admin_url` (une seule règle, avec ou sans le cœur), `ADDRESS_EXAMPLE` (l'exemple de l'ajout passe de `…:24811` à `192.168.1.20:9443`, le port d'Admin), `fingerprint::short` (« — » sans chiffre reste à l'interface), `format_code` et `recording_clock`, `relay::label` et `DEFAULT_RELAY`, `MAX_GUESTS`, `RELAY_LINK_WAIT_SECS` (l'assertion de parité de `device/session.rs` disparaît). |
| Guide, débits | Débit à la main : Auto puis 20 à 80 par 10 et 95 (`manual_mbps(LgTv)`, essai `bitrates`) ; un débit gardé hors des bornes est ramené en session (`clamp_manual_mbps`) ; mise à l'échelle par `profile::pixel_rate_scale` du cœur au lieu de sa copie. `cap_mbps` (80 sur l'Ethernet à 100 Mbit/s) reste à l'app. |
| TRANS-40 | `SDL_APP_WILLENTERBACKGROUND` : `Session::set_background(true, LgTv)` (image figée, 2 Mbit/s), rendu au retour au premier plan avant l'image clé. |
| Guide, device | Annonce mDNS lue par `discovery::found` (adresse préférée, empreinte mise en forme, annonce sans Kyber écartée) ; le fil d'image exécute les étapes de **`frames::FrameFlow`** sur NDL (format jamais annoncé ouvert d'office : `Shared` garde le format demandé ; image clé redemandée au repos toutes les 100 ms ; reprise sans image clé) ; mires, rechargement de NDL, déchargement par le téléviseur et refus de chargement rouvrent le décodeur à l'image clé suivante, comme avant. |
| Guide, `ffi::guard` | `ffi_guard!` autour des rappels C : NDL (`on_load`), chaîne Dolby (`on_event`), Luna (`on_reply`, 0), crochet du lecteur (`kybernow_starfish_*`), son de SDL (`fill` : du silence après une panique). |
| WEBOS-27 | `device/dl.rs` : `dlopen` et `dlsym` une fois pour NDL, la chaîne Dolby et Luna (`sym<T>` était écrit deux fois, Luna le refaisait à la main). |
| WEBOS-26 | Noms des mois et des jours, heure locale (`i18n::month`, `weekday`, `local_time`) : une copie au lieu de trois. |
| link-ui, « Touches » | Les dix entrées dans l'ordre de link-ui, chacune avec son indication en petit dessous. Les polices du téléviseur n'ont **ni ⊞ ni ⌘ ⇥ ⌃ ⌥ ⇧** : le menu face à une Station Mac s'affichait avec des signes absents ; noms en toutes lettres (« Touche Windows seule », « Cmd + Tab », « Option + Cmd + Échap »…), essai `shortcuts_are_drawable` qui vérifie chaque signe dans les polices. |
| link-ui, « Envoyer le journal » | Réglages de l'app › **À propos** (en dernier : version, versions proposées, « Envoyer le journal » avec l'aide des téléviseurs). Le journal (`kybernow.log.1` puis `kybernow.log`, en-tête « KyberNow Link {version} », « Appareil : LG {modèle} · webOS {version} », « Journal écrit le … UTC ») est compressé (2 Mio au plus, sinon l'en-tête et la fin), chiffré en AES-256-GCM (`KNJ1`, clé et nonce au hasard), déposé sur le relais du compte, sinon le relais public, TLS vérifié par `kybernow_tunnel::tls::client_config` (empreinte épinglée comprise) ; écran : préparation, envoi, QR code (`…/journal/<id>#k=<clé>`), relu toutes les 2 s, ouvert, expiré (« Nouveau QR code »), refus du relais (« Réessayer » pour l'injoignable) ; « Terminé » retire un dépôt pas ouvert. |
| TRANS-32 | Typographie française à l'affichage (`canvas::french_spacing`) : l'espace avant « : ; ! ? » devient une espace fine insécable pour tout texte montré (app, cœur, Admin). Les polices n'ont pas U+202F : le canevas la dessine (demi-espace) et le paragraphe ne coupe qu'aux espaces ordinaires (avant, `split_whitespace` coupait aussi aux insécables). |
| TRANS-31 | « Avec un code » (au lieu de « Par code »), “Back to home”, “ON THE NETWORK” (carte 3 de l'accueil, link-ui), “Confirm” pour « Valider », exemple “relay.example.org:9443” en anglais, vrais pluriels de « {n} compte(s) et {n} console(s) reçus ». |
| TRANS-34 | Déjà en quatre groupes depuis la vague 1 ; passe maintenant par `rules::fingerprint::short`. |

### Laissé, et pourquoi

- **`welcome.rs` par `#[path]`** : `WelcomeState` et ses règles sont dans le cœur (`apps/core/src/welcome.rs`), pas dans `kybernow-rules` ; l'interface, qui se compile sans le cœur, ne peut que l'inclure. Pour le retirer, le cœur doit déplacer l'accueil dans la caisse (le guide le prévoit après la 2b) ; ses deux inclusions internes de `rules/src` restent donc aussi.
- **`model::next_encoder_max`** : une fonction du cœur (`AutoResult::next_encoder_max`), absente de `kybernow-rules` ; la copie et son essai de parité restent.
- **`hdr.rs` → `rules::hdr`** : le `hdr.rs` de LG lit les métadonnées HDR10 du flux (SEI, OBU) pour NDL ; il n'a ni courbe ni EETF (le téléviseur fait lui-même le rendu) : rien à remplacer.
- **WEBOS-27, découpage NAL** : il reste écrit dans le cœur (`media.rs`, `playstation/refs.rs`) et dans `apps/webos/src/hdr.rs` ; le cœur (figé) n'a pas de module `nal` public. À faire dans le cœur.
- **WEBOS-26, interface commune avec Linux** (`station_offer.rs`, blocs partage et prêt, `canvas.rs`) : il faudrait une caisse d'interface partagée par `apps/linux` et `apps/webos`, hors de ma zone ; seules les copies internes à LG sont retirées.
- **Dépôt du journal** : écrit dans l'app, comme le pont Apple (`apps/apple/src/journal.rs`) et Android le font chacun ; le cœur n'expose ni le dépôt ni l'agent HTTPS du relais (`relay_agent` est `pub(crate)`). À remonter dans le cœur (`journal::deposit`, `state`, `discard`) pour une seule copie.
- **`on_pyrowave_fallback`, `MicUi`, `recording_ui` (pastille, infobulles)** : sans objet sur LG (pas de PyroWave, pas de micro, menus en tuiles sans pastille ni infobulle).
- **TRANS-39, LG** : « Ce lien de partage est illisible » sans objet (le téléviseur n'ouvre pas de lien) ; « Le partage n'est pas disponible pour le moment… » (réponse 503) n'existe pas dans le catalogue du cœur, dont LG montre les textes de refus : à ajouter au cœur.
- **Captures** : les 20 captures suivies par git ne sont pas refaites (elles diffèrent déjà pour d'autres raisons) ; `scripts/preview.sh` produit les nouvelles (`80-…` à `84-…`, `17-session-touches`).

### Vérifié, et comment

- `cargo test --no-default-features` dans mon arbre `webos` (`third_party/arbres-v2b-webos-web`, supprimé à la fin) : 113 essais passés, dont `log_send::tests` (vecteur d'essai KNJ1 du relais : même dépôt octet pour octet ; aller-retour chiffrement et décompression ; journal trop gros : en-tête, coupure dite, fin du journal, dans la limite ; étapes de l'écran), `shortcuts_are_drawable`, `bitrates`, `address` (IPv4 entre crochets refusée), `canvas::tests` (espace fine).
- Compilation croisée `armv7-unknown-linux-gnueabi` (`apps/webos/scripts/build.sh --check`) dans le même arbre, après chaque étape : passe, aucun avertissement venant de webOS.
- Captures regardées (`--preview`) : menu Touches à dix entrées, écran du QR code.

### Reste à essayer sur la LG G2

- Image : une session longue avec pertes (reprise sans image clé, image clé redemandée au repos), un format jamais annoncé, les mires du réglage du HDR en session, une sortie de veille (débit rendu, décodeur rechargé), l'app mise en arrière-plan en session (2 Mbit/s côté Station).
- Découverte mDNS d'une Station (nom, empreinte) ; ajout par adresse `192.168.1.20:9443` et en IPv6.
- Menu Touches face à un PC et à une Station Mac (lisibilité des noms).
- « Envoyer le journal » face au relais publié (avec le dépôt des journaux) et un relais épinglé : QR code scanné, page du téléphone, « Journal ouvert sur le téléphone. », expiration, « Terminé » ; relais d'avant (404).
- Espace fine : lisibilité dans les écrans (Réglages, avis).

## Client web

### Fait

| Constat | Correction |
|---|---|
| KYB-29 | `pyrowave/bitstream.ts` lit l'en-tête `KNPW` v1 (16 octets, Station 0.11.7) et v2 (48 octets : drapeaux, mastering display, MaxCLL, MaxFALL ; un champ sans son bit est absent) ; une version plus récente se lit par ses 16 premiers octets, comme `Config::parse` du cœur ; contrôles du cœur (octets 5 à 7, côtés, 4:2:0 pair, aussi pour l'en-tête de séquence) ; un paquet `KNPW` arrivé dans le flux n'est jamais décodé en image. Métadonnées écrites au journal de la page. |
| (trouvé) | La courbe vers le SDR du décodeur PyroWave (WebGPU) n'était pas celle des autres apps (par canal, genou à 0,8) : `rules::hdr::sdr_curve` (genou 0,75, 4 fois le blanc ramené à 1, sur la plus grande composante), constantes tirées de TypeScript et essai croisé. |
| WEB-21 | `Session.svelte` passe de 3 770 à 808 lignes : la session sans interface (`livesession.svelte.ts` ; ce nom parce que le disque du Mac confond `session.svelte.ts` et `Session.svelte`), l'invité (`guest.svelte.ts`, règles sans état dans `guestrules.ts`), l'ami qui emprunte (`borrowing.svelte.ts`), l'hôte (`hosting.svelte.ts`), l'enregistrement, la souris et le son (`controls.svelte.ts`), les mesures (`metrics.svelte.ts`, `latency.ts`, qui remplace la fenêtre de mesure d'`engine.ts`), la barre (`bar.svelte.ts`, `overflow.ts`, `SessionBar.svelte`, `TouchBar.svelte`, `SessionOverlay.svelte`), les touches (`keysmenu.ts`), la toile (`stage.ts`), les statistiques (`SessionStats.svelte`, `FramePath.svelte`), la capture du micro chargée à la demande (`miccapture.ts`). |
| WEB-22 | `copyText` dans `lib/format.ts`, pris par tous les panneaux (prêt, partage, code, invitations, comptes d'amis, Appareils) ; `Users.svelte` par `list()` (`users.accounts.and` retiré). |
| link-ui, barre | Ordre de l'hôte, de l'invité et de l'ami ; profil et souris en menus ▾ ; pastille « Presse-papiers » ; « Par le relais » de l'hôte ; son « Stéréo » / « Son coupé » ; « 60 i/s » avant la première latence ; infobulles avec leur raccourci (Ctrl+Alt+Maj + D, F, M, R, S, V, qui marchent) ; barre étroite et menu « Plus » dans l'ordre de link-ui ; ami : « Se déconnecter » discret avant « Rendre l'ordinateur ». |
| link-ui, « Touches » | Les dix entrées dans leur ordre, avec leur indication ; les cinq équivalents face à une Station Mac. |
| TRANS-38 | Statistiques comme le panneau de Windows : titre, « de la capture à votre écran », croix, trajet d'une image en six étapes (d'après les dates que le moteur donne pour chaque image), Latence et Débit sur 30 s avec « 1 image », « Débit visé », « Pertes · 30 s », Chemin, Réseau, Image, Mesures, Son ; « — » pour ce que le moteur web ne mesure pas (débit reçu, paquets perdus, images clés, aller-retour, son). |
| TRANS-39 | La puce « 2 manettes » (absente de link-ui) retirée ; « Prêter » et « Rendre » à l'icône de clé. |
| link-ui, PyroWave | PyroWave qui ne s'ouvre pas dans la page (décodeur refusé, ou images PyroWave sans aucune décodée) : image dans le format d'Auto pour la session, avis « Image en {format} : PyroWave n'a pas pu démarrer sur cet appareil. » (`fallback.ts`, `pyrowaveFallback`), au lieu du repli H.264 et de son avis sur le HEVC. |
| TRANS-40, web | Onglet masqué : 20 Mbit/s demandés par `Client.set_bitrate` du moteur (appelé seulement s'il existe ; présent dans le moteur d'une Station de novembre), rendus au retour ; « Débit visé » dit « 20 Mbit/s · fenêtre cachée ». Hôte et ami seulement (Kyber refuse le débit d'un invité) ; l'image se fige d'elle-même (le navigateur ne dessine plus un onglet masqué). |
| WEB-25 | Selon link-ui (branche des docs) : ni micro ni stylet dans le client web pour l'instant. La sonde du micro ne part plus sans le moteur (les Réglages la lançaient toujours), sa capture est un morceau chargé à la demande, et l'état gardé prêt suit « Envoi… » de link-ui. |
| link-ui, journal | « Télécharger le journal » dans la rubrique « Ce navigateur » des Réglages (l'ancienne « Cette app », renommée comme link-ui) : `kybernow-journal.txt`, avec la version de la Station, le navigateur, le système, le moment (lignes « Appareil : » et « Journal écrit le », comme les autres apps), puis les messages de la console gardés dès l'ouverture de la page (`link/journal.ts`, 2 Mio au plus). |
| STWIN-17 | Admin › Appareils › Hors de chez vous, sous « Admin à distance » : « Clés d'Admin à distance » (liste « {appareil} · ajoutée le {date} · utilisée le {date} », « (autre relais) », « Retirer », « Aucune clé… » ; « Ajouter une clé » : QR code du lien, « Ouvrir sur cet appareil », « Terminé »), sur l'API d'Admin 0.11.8 de la branche du relais (`remote_keys`, `POST`/`DELETE /api/relay/keys`) ; rien face à un Admin 0.11.7 (pas de `remote_keys`) ; ajout grisé par le relais ou sans chiffrement de bout en bout prêt. Textes de `docs/comptes.md` de cette branche (`lib/RemoteKeys.svelte`). |
| TRANS-30 | « Le code a six chiffres. » (le bouton « Rejoindre » attend déjà un prénom, un refus de la Station ne peut plus venir que du code) ; apostrophes droites dans les deux catalogues (et deux messages du journal). |
| TRANS-31 | Exemple de relais traduit (“relay.example.org:9443”) ; infobulle du profil « Jeu, Bureau ou Max » (le client web n'a pas de profil Auto). |
| TRANS-32 | Espace fine insécable avant « : ; ! ? » dans tout le catalogue français (137 endroits), et `i18n-check.mjs` refuse désormais une espace ordinaire à cet endroit et toute apostrophe typographique. |
| TRANS-29 | `i18n-check.mjs` signale les textes inutilisés depuis la vague 1 : ceux que le découpage a rendus inutiles sont retirés (`session.metrics.*`, `session.bar.metrics`, `exitFullscreen`, `session.sound.on`, `muted`, `session.gamepads.*`, `micro.shortcut`, `users.accounts.and`). |
| Guide, web | Copies du client web gardées et alignées : empreinte courte au format `7C41 E09B …`, bornes du débit du navigateur (10 à 150, 1 000 en PyroWave, l'exception de link-ui), profils, `PYROWAVE_MBPS` 800 et 1 200, débit de l'onglet masqué 20 Mbit/s (celui des ordinateurs). |

### Laissé, et pourquoi

- **Demander l'en-tête KNPW v2** : le moteur wasm a `kyclient::set_pyrowave_header` (correctif `0025-kyctlweb`), mais `0025-kyberweb` ne l'exporte pas en JavaScript (seul `fallback_codec` l'est) : le client web reçoit toujours la v1. Zone des correctifs Kyber : exporter `setPyrowaveHeader(v)` ; il suffira alors d'appeler `m.setPyrowaveHeader?.(2)` dans `loadEngine()` (`engine.ts`), ce qui reste compatible avec un moteur 0.11.7.
- **Métadonnées HDR de la v2** : lues et écrites au journal, sans autre usage : la toile du client web est SDR et la courbe vers le SDR (celle de la Station) ne dépend pas de la crête du contenu.
- **Essai qui compare les valeurs du web à `values_json` du cœur** (guide) : il faut un fichier JSON produit par un essai du cœur, figé pour la 2b. À ajouter avec le cœur ; en attendant, les valeurs sont relues à la main (ci-dessus).
- **Micro et stylet du client web** : link-ui les dit « pas encore » ; le moteur web n'envoie pas le micro (zone des correctifs Kyber).
- **Profil Auto, débit adaptatif** : le client web n'a ni l'un ni l'autre ; « Débit visé » dit « fixe ».
- **Captures d'écran et navigateur** : rien n'a été essayé dans un navigateur face à une Station (il faut une Station et son mot de passe d'Admin).
- **Le morceau commun du client web et de la console** se nomme `recording-….js` à la construction (Rollup le nomme d'après un module qu'il contient) : sans effet, à renommer si l'auteur le souhaite (`build.rollupOptions.output.chunkFileNames`).

### Vérifié, et comment

- `server/web` : `npm run check` (svelte-check : 0 erreur, 0 avertissement ; tsc ; `i18n-check.mjs` : 1 442 textes alignés, typographie comprise), `npx vitest run` (15 fichiers, 89 essais, dont `pyrowave/bitstream.test.ts` : v1, v2, versions, refus du cœur ; `rules.test.ts` : courbe vers le SDR ; `journal.test.ts` ; `guestrules`, `overflow`, `latency`, `keysmenu` ; repli PyroWave dans `plan.test.ts`), `npm run build`.
- Compatibilité avec une Station 0.11.7 : mêmes routes ; `set_bitrate` du moteur et `remote_keys` d'Admin utilisés seulement s'ils existent ; en-tête KNPW v1 lu comme avant.

### Reste à essayer, dans un navigateur

- Session complète face à une Station : barre (ordre, menus, « Plus » en fenêtre étroite), Touches, Statistiques et trajet d'une image, raccourcis, onglet masqué (20 Mbit/s côté Station puis retour), invité et ami qui emprunte (rien ne doit avoir changé après le découpage).
- PyroWave refusé par la page (sans WebGPU, ou carte graphique perdue au début) : avis et format d'Auto.
- « Télécharger le journal » (Chrome, Safari, Firefox, iPhone).
- Clés d'Admin à distance face à une Station et un relais 0.11.8 : ajout par le QR code, sur cet appareil, retrait, « (autre relais) », ouverture d'Admin de loin avec la clé.
- Lisibilité de l'espace fine dans Admin et le client web.

## Ce qu'il faudrait au cœur (figé pour la 2b)

- Le dépôt d'un journal (`journal::deposit`, `state`, `discard`, et l'agent HTTPS du relais, aujourd'hui `pub(crate)`) : trois copies en Rust (pont Apple, LG) et une en Kotlin.
- L'accueil (`WelcomeState`) dans `kybernow-rules`, pour retirer les inclusions `#[path]` de LG et de Linux.
- Un module `nal` public (WEBOS-27) et `AutoResult::next_encoder_max` dans la caisse.
- Le texte « Le partage n'est pas disponible pour le moment : réessayez dans un instant. » (réponse 503) dans le catalogue du cœur.

## Notes de version

**FR**

- LG : « Envoyer le journal » dans Réglages › À propos : un QR code à scanner avec votre téléphone, le journal chiffré sur le téléviseur.
- LG : le menu Touches propose les mêmes dix raccourcis que vos autres appareils, avec leur rôle, et s'affiche correctement face à un Mac.
- LG : quand l'app passe en arrière-plan, l'ordinateur réduit son débit ; l'image se rétablit plus vite après une perte.
- LG : les adresses d'Admin et l'empreinte suivent les mêmes règles que vos autres appareils ; la ponctuation française est mieux espacée.
- Navigateur : « Télécharger le journal » dans les Réglages, rubrique « Ce navigateur ».
- Navigateur : la barre de session, le menu Touches et les Statistiques sont ceux des autres apps (trajet d'une image en six étapes, menu « Plus » en fenêtre étroite).
- Navigateur : si PyroWave ne démarre pas, l'image repart dans le format habituel et vous le dit ; onglet masqué, l'ordinateur réduit son débit.
- Admin : les clés d'Admin à distance (« Ajouter une clé » par QR code) protègent l'ouverture d'Admin hors de chez vous.

**EN**

- LG: “Send the log” in Settings › About: a QR code to scan with your phone, the log encrypted on the TV.
- LG: the Keys menu offers the same ten shortcuts as your other devices, with what they do, and shows properly with a Mac.
- LG: when the app goes to the background, the computer lowers its bitrate; the picture recovers faster after a loss.
- LG: Admin addresses and the fingerprint follow the same rules as your other devices; French punctuation is better spaced.
- Browser: “Download the log” in Settings, under “This browser”.
- Browser: the session bar, the Keys menu and Statistics match the other apps (the path of a frame in six steps, a “More” menu in a narrow window).
- Browser: if PyroWave doesn't start, the picture restarts in the usual format and tells you; in a hidden tab, the computer lowers its bitrate.
- Admin: remote Admin keys (“Add a key” with a QR code) protect opening Admin away from home.
