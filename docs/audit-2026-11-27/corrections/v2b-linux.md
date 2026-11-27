# Vague 2 (phase 2b) — app Linux, Steam Deck, salon, KyberNow Link OS

Branche `claude/v2b-linux-7f6081`, d'après [vague-2.md](../vague-2.md) et le guide [migration-coeur-2b.md](../migration-coeur-2b.md) (section Linux), règles de `docs/link-ui.md` de la branche des docs (`claude/v2-docs-0db24c`). Zone : `apps/linux` (app, Link OS `os/`, Flatpak, scripts, essais) et `docs/pyrowave-client-linux*`. Rien poussé, rien fusionné. Commits :

| Commit | Contenu |
|---|---|
| 48006b3e | L'interface sur `kybernow-rules` (adresse, empreinte, codes, relais, HDR, bornes du débit) |
| bb4a05e9 | La partie appareil sur le cœur (flux d'images, PyroWave v2 et repli, arrière-plan, réglages synchronisés, découverte, paquet PlayStation, `ffi_guard!`) |
| 90d922bc | Le micro sur `micro::MicUi` du cœur |
| e5acc3dd | Barre (« Plus », infobulles, souris, Presse-papiers, Touches), caméra, prêt, Statistiques |
| 2c06412d | Erreur sous le nom à l'accueil, « Garder la dernière image » par ordinateur |
| 4abf03ae | « Envoyer le journal » (e-mail au bureau, QR code et dépôt chiffré KNJ1 ailleurs) |
| 3746108d | Link OS : pays et fuseau |
| ee45c8e9 | Petits widgets en un seul endroit, « votre ordinateur », shellcheck |
| a2b4acc4 | Espace fine insécable, « Revenir à l'accueil », exemple de relais en anglais |

## Fait

### Migration vers le cœur (guide, section Linux)

| Id | Correctif |
|---|---|
| Interface → `kybernow-rules` | Dépendance directe (non facultative) : `address::parse_address(texte, None)`, `admin_url`, `host_for_url`, `ADDRESS_EXAMPLE` (l'exemple `192.168.1.20:9443` à la place de `24811`), `fingerprint::short` (« 7C41 E09B AABB CCDD », l'écran de vérification découpe par espace) et `fingerprint::same_opt` partout où deux empreintes se comparaient (`model.rs`, `app.rs`, `device/ps.rs`), `format::format_code`, `recording_clock`, `relay::label`, `DEFAULT_RELAY` (aucun relais : le public, fait par l'app), `limits::MAX_GUESTS`, `RELAY_LINK_WAIT_SECS` (assertion de `device/session.rs` retirée), `hdr` (PQ, EETF, courbe vers le SDR, `TONE_MARGIN`). Copies et essais recopiés retirés. |
| Bornes du débit à la main | Menu « Débit » : les crans entre les bornes de `limits::manual_mbps(Device::Computer, pyrowave)` (10 à 500, jusqu'à 2 500 en PyroWave) ; `app::snap_mbps` ramène un débit synchronisé ou laissé par un changement de format aux bornes puis au cran le plus proche. |
| Shader HDR | Essai croisé `shader_aux_constantes_des_regles` : les constantes de `src/native/hdr_gl.c` (PQ, genou et pente de la courbe vers le SDR, BT.2020 → BT.709) contre `rules::hdr` ; `shade` (référence de `tests/hdr/run.sh`) utilise les fonctions des règles. |
| LINAPP-40 | La mémoire des réglages synchronisés est `account::AccountSync` du cœur (côté appareil : `to_send`, `unsent`, `sent`, `fetched`, `forget`) ; l'interface la garde au même JSON (`SyncMemory`, `Store::account_sync`) ; l'ancienne (`synced`, `sync_pending`) est reprise une fois au chargement. `SyncKeys::merge`, `remove_sent`, `without`, `Push`, `Fetched`, `SyncedEntry`, `PendingSync` retirés ; l'interface garde `apply` (ses `HostSettings`) et les clés changées. Un envoi pendant lequel un nouveau changement arrive le garde (`remove_sent` puis `unsent` après `sent`). |
| LINAPP-41 | `PsSettings` passe au cœur par serde (déjà le cas) ; ses copies `height`, `bitrate` (essais) retirées ; essai `regles_de_l_interface_comme_le_coeur` : « Personnalisé » de l'interface = `PlaySettings::is_custom`. |
| Identifiant PSN | La saisie à la main acceptait des identifiants que le cœur refusait (10 signes, sans `=`, bits de trop) : `ps::psn_id` rend la forme du cœur (`bundle::valid_psn_id`, `PsnId::from_base64`), essai croisé. |
| `pixel_rate_scale` | `device/session.rs` : la mise à l'échelle recopiée → `profile::pixel_rate_scale`. |
| `PwConfig` → `pyrowave::Config` | `kn_pw_sequence_header` (C) retiré : `Config::from_sequence_header`. |
| `frames::FrameFlow` | Remplace la machine de `device/video.rs` (attente d'image clé, porte de reprise, perte dite une fois, image clé toutes les 300 ms, paramètres bout à bout) ; Linux gagne « format jamais annoncé » (`Shared::wanted`, suivi au changement de profil et au repli de PyroWave). `recv_timeout` de 100 ms pour `FrameFlow::idle`. |
| `discovery::found` | `device/jobs.rs` : l'annonce lue par le cœur (IPv4 hors de 169.254/16 d'abord, pas d'IPv6 locale au lien, `kyber=0` écarté, empreinte mise en forme). |
| `playstation::bundle` | `device/ps.rs` : `Bundle::parse`, `match_accounts`, `BundleStation::matches`, `to_json` ; le JSON fait main retiré ; `registered_of` (mort) retiré. |
| Arrière-plan (TRANS-40) | Toutes les fenêtres d'une session réduites ou cachées : `Session::set_background(…, Device::Computer)` (20 Mbit/s, image figée voulue), rendu au retour ; pas sur Link OS. |
| `MicUi` | L'appareil tient le `micro::MicUi` de chaque session et donne sa vue (`ui_json` + `shown`) : puce, infobulle, tuile « Envoi… » / « Envoyé » / « Coupé », avis à l'allumage, refus du système (`denied`). Les textes et la machine de `micro.rs` retirés (restent le raccourci, les libellés des réglages, l'avis du système). |
| `on_pyrowave_fallback` | `Session::pyrowave_failed` une fois par session (plusieurs écrans compris) quand le décodeur Vulkan ne s'ouvre pas ; l'avis du cœur montré ; le format attendu suit le repli. |
| `ffi_guard!` | Les quatre rappels C de l'app : son (`media::fill`, tampon muet sur panique), micro (`mic::captured`), vibrations HD (`ps::fill_haptics`), zones de fenêtre (`hit_test`). |
| KYB-29 | `kyclient::set_pyrowave_header(2)` (fonctionnalité `pyrowave`, dépendance kyclient directe, le même que le cœur) ; `hdr::knpw_metadata` lit mastering display, MaxCLL et MaxFALL de l'en-tête v2 (48 octets au moins, octets en trop ignorés ; v1 : rien) et les donne à l'image HDR. |
| LINAPP-42 (part) | Le triplet des décodeurs matériels en un endroit : `Machine::hw_decoders`, `hw_backend`. |
| LINAPP-45 | `widgets::spinner`, `ease`, `capsule` (bornée à la fenêtre moins 32 points, texte raccourci ; avis, reconnexion, capsule à icône) ; `primary_width`, `ghost_width` au lieu des copies d'`offer.rs`, `settings.rs`, `welcome.rs`. |
| LINAPP-43, 44 | Déjà faits par la phase 2a (`v2-scripts`) : `kyber-client-patches.sh` copié avec les sources du Flatpak, `KYBERNOW_KYBER_HEAD=1` par défaut dans `flatpak.sh`, commit de PyroWave lu dans `apps/macos/PyroWave/COMMIT` (`kyber_pyrowave_commit`). Vérifié, rien à refaire. |
| Vérification sur le Mac | `apps/linux/scripts/verifier-mac.sh [--essais]` : `cargo check` de l'app entière (partie appareil comprise), puis les essais de `src/device` chacun dans son processus (symboles natifs laissés non résolus) ; `WindowHandle::AppKit(0)` hors de Linux pour que le Rust se vérifie tel quel (la ligne qui bloquait la vague 1). |

### Règles de link-ui de la phase 2a

| Id | Correctif |
|---|---|
| LINAPP-19 | Barre étroite : mesures, puis pastilles réduites au nombre, puis « Plus » (trois points, « Plus de commandes ») qui prend une à une Ordinateurs, Touches, souris, Garder le son, micro, caméra, Presse-papiers, Plein écran ; « Plus » les liste avec nom et état et fait ce que fait l'icône ; jamais le profil, HDR, Partager et pastilles, Prêter, enregistrement, relais, Se déconnecter. |
| LINAPP-53 | Infobulles « Profil : Jeu, Bureau, Max ou Auto », « Plein écran (Ctrl+Alt+Maj+F) », « Se déconnecter (Ctrl+Alt+Maj+D) » ; souris en menu « Auto / Souris du bureau / Souris des jeux » (réglage coché et gardé, viseur en souris des jeux ; console PlayStation : la bascule reste) ; pastille « Presse-papiers » ; « Touches » à dix entrées dans l'ordre de link-ui avec leurs indications (« Touche Windows seule » : la police n'a pas ⊞), les cinq du Mac face à une Station Mac. |
| LINAPP-54 | Avertissement du prêt selon `os` de la Station : Windows, Mac, sinon « votre session ». |
| LINAPP-30 | Link OS : « Pays » sous la langue (France / États-Unis proposés), « Fuseau horaire » pour un pays qui en a plusieurs, aide ; Réglages de l'appareil › Langue ; `kybernow-os-ctl locale` (sudoers sans argument) lit `os-locale` de l'app, le vérifie, règle `timedatectl`, `iw reg set` + `cfg80211.ieee80211_regdom=` et `/etc/default/keyboard` + `setupcon` ; la mise à jour garde le fuseau, le clavier et le pays du Wi-Fi. |
| Envoyer le journal | Réglages de l'app › À propos (et Link OS › Avancé). Bureau : `xdg-email --attach`, sinon zenity/kdialog, sinon Téléchargements (Flatpak : `--filesystem=xdg-download`) ; « Préparation du journal… ». Deck en mode Jeu, salon, Link OS : gzip (2 Mio au plus, en-tête et fin), AES-256-GCM (`KNJ1`, nonce, chiffré, étiquette), `POST /v1/journal` au relais du compte sinon public, QR `https://<relais>/journal/<id>#k=<clé>`, état toutes les 2 s, « Terminé » → `DELETE` ; les étapes et refus (expiré, relais muet, relais d'avant, trop d'envois) de link-ui. |
| Caméra | Refus de l'ordinateur par `camera::event_notice` (déjà) ; départ refusé (« La caméra n'a pas pu démarrer sur l'ordinateur. ») ; 10 s sans réponse : éteinte, « L'ordinateur ne répond pas : la caméra est éteinte. ». |
| TRANS-38 | Statistiques de link-ui : titre, raccourci, croix « Fermer les statistiques », latence et « de la capture à votre écran », trajet, « Latence » et « Débit » sur 30 s (repère « 1 image », débit visé), « Pertes · 30 s » (images, paquets, images clés) et la session, Chemin, Réseau (aller-retour), Image (format décodé, matériel ou processeur), Mesures (part mesurée), Son (tampon, sortie), Caméra ; phrase avant la première image ; deux colonnes quand la fenêtre est basse. |
| TRANS-39 | Erreur sous le nom à l'accueil (mot de passe refusé : 401 ; Station réinstallée : certificat changé) ; « Garder la dernière image » réglage de l'ordinateur (rubrique Image), l'ancien réglage d'app coupé repris par chaque ordinateur ; Touches (ci-dessus). |
| Message agnostique | `ui/setup.rs` : « votre ordinateur comme si vous y étiez ». |
| Shellcheck | `tests/pyrowave/run.sh` (SC2046) : un tableau. Tous les scripts de `apps/linux` sans remarque (`-S warning`). |
| TRANS-31 | « Revenir à l'accueil » (forme de link-ui) ; exemple de relais en anglais `relay.example.org:9443`. |
| TRANS-32 | Espace fine insécable avant « : ; ! ? » dans les 303 littéraux français de l'app (hors messages du journal de l'app) ; les polices n'ont pas U+202F : dessinée avec U+00A0 rétrécie (`canvas::NARROW_NBSP`), jamais une coupure de ligne à elle. |
| TRANS-29 | L'app Linux n'a pas de catalogue (paires `tr`) : pas de clé morte ; fonctions mortes retirées en route (`registered_of`, textes de `micro.rs`, copies citées). |

## Laissé, et pourquoi

| Id | Raison |
|---|---|
| `welcome.rs` par `#[path]` | Il reste : `kybernow-rules` n'a pas l'accueil, et le cœur (figé en 2b) doit encore retirer ses deux inclusions internes ou déplacer l'accueil dans la caisse (guide, « Ce qui reste au cœur après la 2b »). |
| LINAPP-42 (fonctions avec webOS) | `join_relays`, `relay_failure_text`, `role_of`, `win_scancode`, les types du test Auto et `app_time` : à mettre dans le cœur ou une petite caisse commune d'interface, hors de ma zone (cœur figé, webOS à une autre session). |
| `plan::plan` | `device::session::settings` passe à `profile::pixel_rate_scale` (le guide le permet) mais pas à `plan::plan` : Linux garde le profil dans les réglages (le « Personnalisé » des écrans du cœur en dépend) et son « Alléger » plafonne le **débit** à 20 Mbit/s quand `plan::Choices::lighten` plafonne la **fréquence** à 30 Hz. Deux règles différentes pour le même réglage : à trancher dans link-ui avant d'y passer. |
| « Affichage des images », « Définition » (TRANS-39) | Linux a « Auto » et « Fréquence variable » en plus de « Synchronisé » et « Immédiat », et les hauteurs 2160p à 720p au lieu de « Au plus 4K » et « Sur mesure » ; changer retirerait la fréquence variable ou demanderait `Choices::max_4k` (voir `plan::plan`) : exception à écrire dans link-ui, ou décision. |
| TRANS-31 (reste) | « Avec un code » (Linux, Windows) et « Vibration d'essai » (Android, Linux, LG) : la forme majoritaire, link-ui ne tranche pas. |
| Relais épinglé | Le dépôt du journal joint le relais par les autorités du web : le client épinglé du cœur (`relay::relay_agent`) est `pub(crate)`. Un relais personnel à certificat épinglé refusera le dépôt (« Le relais ne répond pas »). |

## Vrai cause hors de la zone, API qui manquent au cœur

- `pyrowave::Config` lit la v2 mais ne rend pas ses métadonnées HDR : l'app les lit elle-même (`hdr::knpw_metadata`) ; des champs facultatifs dans `Config` suffiraient.
- `relay::relay_agent` (client HTTP vers le relais, empreinte épinglée) à exposer pour le dépôt du journal.
- Catalogue du cœur : « La caméra n'a pas pu démarrer sur l'ordinateur. » et « L'ordinateur ne répond pas : la caméra est éteinte. » n'y sont pas (`camera::event_notice` ne couvre que les refus de l'ordinateur) ; textes de « Envoyer le journal » non plus. Ils sont dans l'app.
- Catalogue du cœur sans espace fine (TRANS-32 du cœur) : ses textes gardent l'espace ordinaire à côté de ceux de l'app.
- `docs/link-ui.md` dit « prêtez votre PC » (accueil, avantage 3) et « Sur votre Mac, votre PC ou votre téléphone » (PlayStation) : listes de plateformes contraires au message agnostique, à revoir par la session des docs ; l'app les reprend mot pour mot.
- `docs/i18n.md` (pas ma zone) devrait dire que Linux applique l'espace fine dans ses littéraux.

## Vérifié, et comment

- `cargo test -p kybernow-linux --no-default-features` sur le Mac : 177 essais, tous passent (dont `narrow_bar_moves_controls_into_more`, `statistics_panel_is_complete_and_fits`, `send_the_log`, `link_os_country_and_time_zone`, `erreur_sous_le_nom`, `garder_la_derniere_image_par_ordinateur`, `memoire_d_avant_reprise`, `avertissement_selon_le_systeme`, `espace_fine_insecable`, `metadonnees_de_l_en_tete_pyrowave_v2`, `shader_aux_constantes_des_regles`, `pays_fuseau_et_clavier`).
- Partie appareil : `apps/linux/scripts/verifier-mac.sh --essais` (sources de Kyber de mon arbre avec les correctifs des apps posés, `kynput.h` de l'app Mac) : `cargo check` de l'app et de ses essais **sans un avertissement** de l'app ; 42 essais de `src/device` passent sur le Mac (dont `device::journal::tests` : dépôt KNJ1 déchiffré avec la clé de l'adresse, 2 Mio ; `device::accounts::tests::memoire_au_json_du_coeur` ; `device::mic::tests` ; `device::ps::tests` ; `device::video::pyrowave_tests`) ; le 43ᵉ demande la carte graphique (`hdr_wl::tests::le_shader_suit_la_reference`). Les essais multi-fenêtres (X11) sont `#[ignore]`.
- Captures regardées : Statistiques au bureau (1280 × 800, 960 × 600 en deux colonnes), au Deck, au salon ; panneau du journal (QR code) ; écran Langue de Link OS (pays, fuseau) ; menus « Plus » et souris (`bureau-17i`, `17j`).
- Scripts : `bash -n` et `shellcheck -S warning` sans remarque sur tous les scripts de `apps/linux` ; validation de `kybernow-os-ctl locale` essayée sur des fichiers (valeurs abîmées refusées : `../`, minuscules, espaces) ; `os/tests/os-update.sh` : tous les essais passent.
- Pas de VM ni de carte : rien n'a tourné sous Linux.

## Reste à essayer sur appareil

Raspberry Pi (Link OS) :
1. Premier démarrage : pays et fuseau ; `timedatectl` à jour, `iw reg get` au pays, `cfg80211.ieee80211_regdom=` dans `cmdline.txt`, clavier USB AZERTY / QWERTZ / QWERTY selon le pays ; après une mise à jour A/B, les trois gardés.
2. « Envoyer le journal » au salon : QR code, ouverture sur un téléphone (page du relais quand elle existera), « Journal ouvert sur le téléphone. », expiration, « Terminé » qui retire le dépôt.
3. PyroWave sur Pi 5 : décodeur qui ne s'ouvre pas → repli et avis ; en-tête v2 face à une Station 0.11.8 (journal « PyroWave … HDR10 », métadonnées).
4. Décodage V4L2 sur la nouvelle machine du flux : perte, reprise, image clé, `EAGAIN`.

Steam Deck :
1. Mode Jeu : « Envoyer le journal » par QR code ; mode Bureau : la fenêtre d'e-mail (KDE, portail du Flatpak), ou l'enregistrement.
2. Barre au bureau du Deck en fenêtre étroite (« Plus »), menu de la souris, Statistiques en 1280 × 800.
3. Fenêtre réduite : 20 Mbit/s et « fenêtre cachée » dans les Statistiques, retour à la normale.
4. Micro : puce et tuile « Envoi… » puis « Envoyé », avis à l'allumage, micro refusé par le système.

Linux de bureau : réglages synchronisés avec un vrai relais (envoi, changement pendant l'envoi, lecture, reprise de l'ancienne mémoire), découverte Bonjour, envoi PlayStation entre appareils, caméra 10 s sans réponse, erreur sous le nom (mot de passe changé dans Admin, Station réinstallée).

## Notes de version

**Français**

- Linux, Steam Deck : « Envoyer le journal » dans les Réglages de l'app : au bureau, il part dans un e-mail ; sur le Deck en mode Jeu, au salon et sur KyberNow Link OS, par un QR code à scanner avec votre téléphone, chiffré sur l'appareil.
- KyberNow Link OS : au premier démarrage, choisissez votre pays : le Wi-Fi, l'heure et le clavier le suivent.
- Linux : une barre de session qui tient dans les fenêtres étroites (menu « Plus »), la souris en menu, le presse-papiers visible, dix raccourcis dans « Touches ».
- Linux : des Statistiques complètes (pertes sur 30 secondes, réseau, image, son).
- Linux : une fenêtre réduite demande moins de débit à l'ordinateur.
- Linux : PyroWave qui ne démarre pas sur l'appareil passe tout seul à un autre format ; le HDR en PyroWave reçoit la luminosité de l'écran de l'ordinateur.
- Linux : l'accueil dit quand le mot de passe est refusé ou que la Station a été réinstallée.

**English**

- Linux, Steam Deck: “Send the log” in the App settings: on the desktop it goes into an e-mail; on the Deck in Game mode, in the living room and on KyberNow Link OS, through a QR code you scan with your phone, encrypted on the device.
- KyberNow Link OS: at first boot, choose your country: Wi-Fi, time and keyboard follow it.
- Linux: a session bar that fits narrow windows (“More” menu), the mouse as a menu, the clipboard shown, ten shortcuts in “Keys”.
- Linux: complete Statistics (losses over 30 seconds, network, picture, sound).
- Linux: a minimised window asks the computer for less bitrate.
- Linux: PyroWave that won't start on the device switches to another format by itself; HDR with PyroWave gets the computer screen's brightness.
- Linux: the home screen says when the password is refused or the Station was reinstalled.
