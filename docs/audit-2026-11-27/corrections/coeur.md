# Corrections de l'audit du 27/11 : cœur Rust (`apps/core`)

Branche `claude/fix-coeur-19e46f`. Rapport de l'audit : [../coeur.md](../coeur.md).

## Corrigés

- **COEUR-01** — Son PlayStation : index des trames déroulé sur 64 bits (clé fixe d'une trame, même rangée juste avant le passage par zéro), saut de plus de 32 trames ramené à un seul trou. Plus de boucle sans fin ni de mémoire qui grossit toutes les ≈ 11 min.
- **COEUR-02** — `now_micros` : `timespec` en `c_long` (32 bits sur webOS armv7 et Android armeabi-v7a), calcul saturé.
- **COEUR-03** — `OpusHead` : refusé au-delà de 8 voies, en famille 0 au-delà de 2 voies, et avec des flux incohérents (aucun flux, plus de flux couplés que de flux).
- **COEUR-04** — `renew` garde le micro et la caméra que tiennent `Session` et l'app, leur flux montant remis à zéro.
- **COEUR-05** — Correction d'erreurs PlayStation refusée au-delà de 256 unités (image perdue au lieu d'une panique ou d'une image fausse).
- **COEUR-06** — Message BIG : le morceau final (drapeau de fin) part toujours.
- **COEUR-07** — Hexadécimal et `%xx` lus octet par octet (`playstation::hex_decode`, `hex_pair`, communs aux trois fichiers) ; `%xx` final décodé, signe `+` refusé.
- **COEUR-08** — Forme du pointeur : `checked_mul`, côté plafonné à 1 024, point actif ramené dans la forme.
- **COEUR-09** — `PointerLayer::set_host_size` et `move_by` ignorent une taille inférieure à 1 ou NaN.
- **Paniques à la frontière C** — nouveau module `ffi` : crochet de panique qui écrit message, endroit, fil et pile par `log::error!` (donc dans le journal de la plateforme) avant le crochet précédent, posé par le cœur lui-même (session, chaîne d'entrées, moteur de son, partie PlayStation) ; `ffi::guard` / `guard_default` (`catch_unwind`) pour le corps des fonctions `extern "C"`. Le seul rappel C du cœur (`media.rs`, paquets de Kyber) est protégé.
- **COEUR-10** — Filtre des manettes (Mac) : le repos et la valeur déjà partie passent toujours (réessais de la file de secours).
- **COEUR-11** — Prise de la manette : la file des axes passe aussi au repos ; à la fermeture de la barre, les axes tenus repartent d'après le lecteur de manettes du cœur.
- **COEUR-12** — `Drop` de `Worker` : signal de vie et premier plan arrêtés ; après une panique (et seulement là), la fin est dite à Admin sur un fil à part.
- **COEUR-13** — `spawn_prepare` : les trois préparations en cours de session sont suivies ; `release` diffère la fin dite à Admin tant que l'une d'elles est en vol, comme pour un appel abandonné de la mise en route.
- **COEUR-15** — Rejoindre un partage : un accord qui arrive quand personne ne l'attend (relais retenu, autre Station, échéance passée) fait repartir l'invité (`late_leave`) ; `rx` lâché avant `relay_join`.
- **COEUR-16** — Plus de voies en sortie : mono en G et D, stéréo en G et D (plus au centre), 5.1 vers 7.1 avec le LFE à sa place.
- **COEUR-19** — Console sans empreinte connue : la première empreinte vue est exigée des connexions suivantes ; la dernière vue reste dite en cas de refus.
- **COEUR-20** — Transfert PlayStation : une seconde d'attente après chaque code faux, écoute arrêtée après 10 codes faux (`Expired`), confirmations HMAC comparées en temps constant (`verify_slice`).
- **COEUR-24** — Canal de contrôle PlayStation dans un `Arc` : cloné sous le verrou, employé hors de lui ; arrêté (fil joint) hors du verrou.
- **COEUR-30** — Rappel du son : `try_lock` sur `current` et `order` (silence et nouvel essai au rappel suivant), tampons réservés une fois pour 8 voies, ancien flux rendu au producteur pour être libéré hors du fil temps réel.
- **COEUR-31** — `Microphone::push` : `try_lock` partout (échantillons jetés si un réglage tient un verrou), encodeur créé à l'allumage, plus de journal, paquet encodé en place (une allocation par trame : le paquet que garde le flux montant), tampon de lissage du rééchantillonnage réutilisé.
- **COEUR-49** — `.github/workflows/coeur.yml` (nouveau fichier, `ci.yml` intouché) et `apps/core/scripts/ci-essais.sh` : sources de Kyber 0.29 clonées depuis gitlab.com/kyber (sous-modules lus par cargo seulement), correctifs des apps posés par `scripts/kyber-client-patches.sh`, libkynput par cargo-c, libvlc factice (voir plus bas), puis `cargo test -p kybernow-core --features pyrowave --lib`.
- **COEUR-14** — Pendant l'attente de la Station : HDR (choix et écrans), écrans et mode de l'appareil gardés pour la tentative suivante (l'écran de Kyber choisi ne vaut que pour la connexion close).
- **COEUR-17** — Forme du pointeur dédoublonnée par couche de l'appareil, plus par écran du PC.
- **COEUR-18** — Texte tapé ou collé : chaque caractère attend son tour tant que la file avance (perdu seulement après 2 s sans aucun progrès), 4 096 caractères au plus par appel, 8 192 en attente, une ligne de journal par rafale de pertes.
- **COEUR-21** — Raison d'un appel à Admin (jeton absent, tunnel fermé, jeton refusé) rangée pour le fil et la console de l'appel (`CallNotes`, par fil), plus dans la console partagée.
- **COEUR-22** — Le fil du premier plan revérifie `alive` après l'appel HTTP.
- **COEUR-23** — `next_second` ramené à maintenant après un appel bloquant : une seule « seconde ».
- **COEUR-25** — Un seul tunnel par Station et par relais : celui d'un autre fil, ouvert pendant ce temps, est gardé, le doublon fermé.
- **COEUR-26** — Lecteur SDL : numéro de génération ; un ancien fil qui se ferme ne touche plus à l'état du suivant.
- **COEUR-27** — Recherche PlayStation : `ConnectionReset` / `ConnectionRefused` (ICMP sous Windows) ignorés.
- **COEUR-28** — `TargetBitrate::mbps` en u64.
- **COEUR-29** — `impl Drop for Session` : la session lâchée sans `stop()` s'arrête.
- **COEUR-32** — PlayStation : GMAC vérifié sur place (plus de copie du paquet), chiffrement bloc par bloc sans flux de clé alloué, AES-GCM gardé tant que la clé GMAC ne change pas, unités d'image prises sans copie (`Assembler::push` par valeur), image assemblée réservée d'un coup.
- **COEUR-33** — Files bornées : réception → flux (4 096 paquets, l'excédent compté perdu et signalé à la console par l'assemblage), flux → partie (256 événements : une image en trop est jetée, une image clé demandée, `VideoLost`).
- **COEUR-34** — Jeton d'accès renouvelé en avance sur un fil à part (`kybernow-jeton`) pendant que le courant sert encore ; une demande bloquante seulement sans jeton valable, une à la fois, jamais sous le verrou du cache.
- **COEUR-35** — Un agent HTTPS gardé par relais (`relay::relay_agent`), pour les comptes et la recherche de code.
- **COEUR-36** (en partie) — Forme du pointeur comparée par mémoire au lieu d'une empreinte SipHash de tous les pixels ; le décodage reste sur le fil de kynput (voir « Laissés »).
- **COEUR-40** — `profile::pixel_rate_scale` et `pyrowave_fallback` partagés avec le test Auto ; une seule liaison libopus (`audio/opus.rs`) ; paquets du flux montant communs au micro et à la caméra ; décodage d'adresse et chiffres hexadécimaux dans `src/url.rs` (comptes, partage, PSN, PlayStation) ; `sign_up_url` par `account::relay_label` (relais épinglé).
- **COEUR-42** — `AudioEngine::set_order` retiré (et sa course).
- **COEUR-43** — Retirés : `Session::set_name`, `RecordingState::label/tooltip/tile/menu_item/guest_label/guest_tooltip` (`ui_json` reste la seule source), `InputPipeline::core_reads_gamepads`, `relay::open_tunnels`, `ManualClock::starting_at`, `Ring::capacity`. En `#[cfg(test)]` : `drain_second`, `CameraLink::is_sending`, `CameraResolution::from_label`, `Console::station_access`, `share::check_codec`, `pyrowave::floor_mbps`, `LendCodeView::relay_connecting`, `PadButton/PadAxis::from_kynput`, `keymap::is_modifier`. Constantes branchées : `clipboard::MAX_TEXT` (texte coupé entre deux caractères), `settings::MAX_BYTES` (réglages trop grands refusés sans appel), `lend::COME_BACK` (journal).
- **COEUR-44** — 29 clés du catalogue retirées en FR et EN (panneau du test Auto, textes du micro tenus par les interfaces) ; `jerror!` retirée.
- **COEUR-45** — `CtrlEvent::Other` (et sa copie de chaque charge), `LoginPinRequested`, `Login` retirés (journal à la place) ; les constantes de protocole (`LOGIN_PIN_REP`, `GO_HOME`, `PAD_INFO`) restent, elles documentent les codes.
- **COEUR-46** — PyroWave dit allumé par défaut (`Cargo.toml`, `build-apple.sh`, README FR et EN).
- **COEUR-47** — README FR et EN : modules manquants (autotest, caméra, pyrowave, station_offer, welcome, url, ffi), section caméra, origine des correctifs (`*-kyclient-*` de `server/kyber-patches`), API mortes retirées, en anglais le test Auto, l'accueil et le côté hôte hors de chez soi.
- **COEUR-48** — Commentaires corrigés (relances faute d'image, `keyframe_lost`, `kyber_endpoint`, Takion écrit, durée de `join_anywhere`, fil de `navigate`, README Takion), `None.or_else` simplifié, règle « aucun rappel n'envoie d'entrée de façon synchrone » écrite sur `InputPlatform`.
- **TRANS-04** — `build-apple.sh` : `cargo +nightly-$KYBER_NIGHTLY` (ou le `nightly` installé s'il est exactement de cette date), plus jamais un autre nightly.
- **CI 32 bits** — `ci-essais.sh` accepte `CI_TARGET=i686-unknown-linux-gnu` (essais lancés en 32 bits sur le runner x86_64) ; second job `essais-32-bits` dans `coeur.yml`.
- **En plus (webcam)** — `CameraLink` : un START qui ne part pas (file pleine, autre caméra choisie pendant que des images sont en route) est renvoyé avant l'image suivante ; aucune image ne part avant lui.

## Laissés

- **`catch_unwind` dans les ponts eux-mêmes** (`apps/apple/src`, ≈ 380 fonctions `extern "C"` ; `apps/android/jni`, fonctions `extern "system"` ; rappels C de `apps/windows`, `apps/webos`, `apps/linux`) : hors de la zone du cœur (sessions Mac, Android, Windows, webOS, Linux). Le cœur fournit `ffi::guard` et pose déjà le crochet de panique : une panique laisse donc désormais sa trace dans le journal, même si elle arrête l'app. Mode d'emploi pour ces sessions : note `coeur-paniques-ponts.md` du dossier partagé.
- **COEUR-37 à 39** (factorisations de `session.rs`, lecture des réponses HTTP, choix réseau/relais) : vague 2, qui partira de cette branche. `spawn_prepare` (COEUR-13) fait déjà la partie « fils de préparation » de COEUR-37.
- **COEUR-41** (règles du cœur refaites dans les apps : textes de l'enregistrement, `format_code`, lien d'invitation sous Android, « Définition ramenée ») : transverse, la correction est dans les apps et les ponts (vague 2). Côté cœur, les textes de l'enregistrement en double sont retirés (COEUR-43) : `ui_json` est la seule source.
- **COEUR-36, la suite** : sortir le décodage de l'image du pointeur du fil de kynput (comme le presse-papiers) demanderait que chaque plateforme appelle `poll` assez tôt pour ne pas retarder la forme ; avec le correctif serveur 0006 (positions sans image), le gain restant est faible. Seule l'empreinte coûteuse est retirée.
- **COEUR-33, côté partie** : la file flux → partie jette les images en trop ; la file des paquets de son n'est bornée que par la première (réception).
- **docs partagés de PyroWave** (`docs/pyrowave-client-*.md`), scripts Android et iOS, `apps/windows/Cargo.toml` (COEUR-46) : hors zone, ils disent encore « éteint par défaut ».
- **Commentaire Swift faux** (« `kn_session_push_microphone` ne bloque pas », `apps/macos/KyberKit/AudioPlayerCore.swift:411-415`) : il devient vrai avec COEUR-31, mais le fichier est hors zone.
- **Ligne de tête de `ci.yml`** (« le cœur Rust … ne [l']est pas non plus ») : à mettre à jour par la session Outillage, qui tient `ci.yml` ; le cœur a son propre workflow.
- **CI 32 bits, premier passage** : le job `essais-32-bits` (i686) n'a jamais tourné (pas de Linux x86_64 ici, rien de poussé) ; il est en `continue-on-error` tant qu'un premier passage ne l'a pas confirmé. Une vraie cible armv7 en CI demanderait libkynput, libopus et la libvlc factice croisées, et qemu pour lancer les essais : pas fait ; la compilation pour armv7 est vérifiée sur ce Mac (ci-dessous).

## Ce qui est vérifié, et comment

- Arbre à moi (`KYBERNOW_ARBRES=…/third_party/arbres-coeur scripts/arbres.sh preparer apple`), correctifs des apps posés comme `build-apple.sh` (PyroWave compris), Rust 1.89 : `cargo test -p kybernow-core --features pyrowave --lib` — **472 réussis, 0 échec, 4 ignorés** à la fin de la seconde passe (444 au départ de la branche, 464 à la fin de la première).
- Un essai par correctif : `perte_sur_0xffff`, `saut_d_index_borne` (01) ; `horloge_croissante` (02) ; `en_tete_opus_incoherent` (03) ; `micro_et_camera_gardes_apres_le_redemarrage` (04) ; `au_dela_de_256_unites` (05) ; `big_toujours_termine` (06) ; `hexadecimal_sur_des_octets`, `decodage_sans_panique` (07) ; `pointeur_trop_grand_refuse` (08) ; `taille_nulle_ignoree` (09) ; `panique_rattrapee`, `rappel_c_protege` (ffi) ; `libre_tout_passe_et_axes_immobiles_ecartes` étendu (10) ; `stick_tenu_pendant_la_barre` (11) ; `panique_du_fil_arrete_les_fils_de_fond` (12) ; `fin_apres_les_preparations_en_vol` (13) ; `second_accord_fait_repartir_l_invite` et le cas « réseau lent » de `join_anywhere` (15) ; `mixage_descendant` mis à jour (16) ; `premier_certificat_exige_ensuite` (19) ; `trop_de_codes_faux_arretent_l_ecoute` (20) ; `flux_remplace_libere_hors_du_fil_temps_reel` (30) ; `capture_sans_verrou_bloquant` (31) ; `start_pas_perdu_file_pleine` (caméra). Seconde passe : `reglages_gardes_pendant_l_attente_de_la_station` (14) ; `pointeur_dedoublonne` étendu (17) ; `long_texte_attend_son_tour` (18) ; `raison_d_un_appel_gardee_pour_son_fil` (21) ; `mbps_sans_debordement` (28) ; `session_lachee_s_arrete` (29) ; `chiffrement_sans_flux_alloue` (32) ; `jeton_d_acces_garde_quatre_minutes` mis à jour (34) ; `decodage_commun`, `sign_up_url` épinglé (40) ; `texte_trop_long_coupe_proprement`, réglages trop grands (43) ; texte de l'enregistrement par `ui_json` (43). Sans essai propre, vérifiés par la compilation et les essais existants : COEUR-22, 23, 25, 26 (`cargo check --features sdl,pyrowave`), 27, 33, 35, 24.
- Script de la CI essayé sur ce Mac depuis un clone neuf de la branche (dans le dossier temporaire de la session) : clonage de kyber-desktop 0.29.0 et de ses sous-modules depuis GitLab, 17 correctifs posés, libkynput par cargo-c, libvlc factice, essais — **463 réussis, 0 échec, 4 ignorés** (clone au commit du script, avant le correctif de la caméra) ; libopus de Homebrew ; 32 s pour le clonage et les correctifs. Le workflow lui-même (Ubuntu 24.04, paquets apt) n'a pas tourné sur GitHub : pas de push depuis cette session.
- Seconde passe : le script refait tourner dans mon arbre (correctifs déjà posés, libkynput refaite, libvlc factice) : **472 réussis**. Compilation du cœur pour armv7 (32 bits, `long` de 32 bits) sur ce Mac avec la chaîne de webOS : `cargo check -p kybernow-core --features pyrowave --lib --target armv7-unknown-linux-gnueabi`, sans erreur ni avertissement du cœur. `node apps/windows/ui/scripts/check-i18n.mjs` : catalogues du cœur concordants (234 clés). `cargo check --examples` : propre.
- `shellcheck -x` sur les scripts : propre. `cargo clippy -p kybernow-core --features pyrowave --all-targets` : aucun avertissement sur les lignes changées (les 52 qui restent sont antérieurs à la branche).

## La webcam de l'iPhone qui n'a pas marché (côté cœur)

Ce qui, dans le cœur, peut l'expliquer :

1. **COEUR-04, si la Station a redémarré en dernier recours** (pas d'image au départ, « La Station redémarre… ») : la nouvelle tentative créait une `CameraLink` neuve, tandis que l'app gardait l'ancienne (`Session::camera()`). L'app envoyait ses images à un objet sans flux montant (`FrameOutcome::Off`, que l'app Apple ignore), l'ordinateur ne recevait rien, et rien ne le disait. Corrigé.
2. **START perdu** : `CameraLink::start` n'envoyait START qu'une fois, sans réessai si la file du flux montant était pleine ; l'ordinateur ne lance son aide caméra qu'à START (kycontroller), donc les images suivantes partaient pour rien. Peu probable au tout premier START (file vide), plausible en changeant de caméra ou de définition pendant l'envoi. Corrigé (START renvoyé avant l'image suivante).
3. **Relance de l'image** (`stop_streaming` à chaque image clé ratée, adaptation, changement d'écrans) : le flux montant est retiré (`on_camera_offer(false)`) puis rendu au démarrage suivant, et START repart seul (`set_uplink`). Côté cœur c'est correct ; si l'app iOS éteint sa caméra quand l'offre passe à faux, elle ne revient pas d'elle-même : à vérifier côté iOS.
4. **Pas côté cœur, mais à vérifier** : la caméra n'est proposée que si la Station l'annonce (`uplink` dans la réponse de `/kymux/start_session`) ; la Station Mac ne la propose pas sans le profil Developer ID « System Extension » (note de la session webcam) ; sur Windows, la 0.11.7 a livré les aides micro et caméra dans leur ancienne version (TRANS-01, corrigé en 0.11.8). Un seul appareil à la fois par type (« busy ») : une autre app Link qui envoie sa caméra à la même Station la prend.

Pour trancher : le journal de l'iPhone (lignes « caméra » et la réponse STATUS : `started`, `busy`, `failed` + raison) et celui de la Station (lancement de l'aide `kybernow-camera`).

## Notes de version

FR :
- Consoles PlayStation : le son ne se fige plus au bout d'une dizaine de minutes quand un paquet se perd au mauvais moment, et certaines définitions qui échouaient à tous les coups (« pas de réponse de la console ») démarrent.
- Manettes sur Mac : un stick ou une gâchette ne reste plus poussé sur l'ordinateur après une rafale, ni après avoir ouvert puis fermé la barre en le tenant.
- Si l'ordinateur a dû redémarrer sa Station pour vous connecter, votre micro et votre caméra continuent de fonctionner.
- Le son 5.1 ou 7.1 joue la stéréo sur les bonnes enceintes (la droite n'est plus envoyée au centre).
- Partage : rejoindre par le relais ne laisse plus de place fantôme chez l'hôte.
- Plus de coupures du son ni du micro au démarrage de l'image ou en changeant de micro.
- Sécurité : une Station jamais vue garde son certificat pour toute la connexion, et l'envoi de comptes PlayStation entre vos appareils s'arrête après trop de codes faux.
- En cas d'erreur interne, la session se termine proprement sur l'ordinateur, et l'erreur est écrite dans le journal de l'app.
- Un long texte collé ou dicté arrive en entier sur l'ordinateur.
- Les changements d'écran, de HDR ou de définition faits pendant le redémarrage de la Station sont gardés.
- Hors de chez vous, plus d'attente de plusieurs secondes quand le jeton d'accès se renouvelle, et des connexions au relais plus rapides.
- Consoles PlayStation : moins de travail par image (utile sur Raspberry Pi, TV et Android d'entrée de gamme), et plus de retard qui s'accumule quand l'appareil est occupé.

EN:
- PlayStation consoles: sound no longer freezes after about ten minutes when a packet is lost at the wrong moment, and some resolutions that always failed ("the console doesn't answer") now start.
- Controllers on Mac: a stick or trigger no longer stays pushed on the computer after a burst, or after opening and closing the bar while holding it.
- If the computer had to restart its Station to connect you, your microphone and camera keep working.
- 5.1 or 7.1 sound plays stereo on the right speakers (the right channel no longer goes to the centre).
- Sharing: joining through the relay no longer leaves a ghost seat on the host.
- No more sound or microphone dropouts when the picture starts or when you switch microphones.
- Security: a Station seen for the first time keeps its certificate for the whole connection, and sending PlayStation accounts between your devices stops after too many wrong codes.
- If something goes wrong inside the app, the session ends cleanly on the computer and the error is written to the app's log.
- Long pasted or dictated text now reaches the computer in full.
- Screen, HDR or resolution changes made while the Station restarts are kept.
- Away from home, no more waits of several seconds when the access token renews, and faster connections to the relay.
- PlayStation consoles: less work per frame (helps on Raspberry Pi, TVs and entry-level Android), and no more delay building up when the device is busy.
