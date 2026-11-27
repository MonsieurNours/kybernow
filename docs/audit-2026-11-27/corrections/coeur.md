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
- **En plus (webcam)** — `CameraLink` : un START qui ne part pas (file pleine, autre caméra choisie pendant que des images sont en route) est renvoyé avant l'image suivante ; aucune image ne part avant lui.

## Laissés

- **`catch_unwind` dans les ponts eux-mêmes** (`apps/apple/src`, ≈ 380 fonctions `extern "C"` ; `apps/android/jni`, fonctions `extern "system"` ; rappels C de `apps/windows`, `apps/webos`, `apps/linux`) : hors de la zone du cœur (sessions Mac, Android, Windows, webOS, Linux). Le cœur fournit `ffi::guard` et pose déjà le crochet de panique : une panique laisse donc désormais sa trace dans le journal, même si elle arrête l'app. Mode d'emploi pour ces sessions : note `coeur-paniques-ponts.md` du dossier partagé.
- **COEUR-14, 17, 18, 21 à 23, 25 à 29, 32 à 48** : pas demandés dans cette vague (vague 2 : factorisation, code mort, docs). COEUR-46 et 47 (README) et le reste de COEUR-48 (commentaires) n'ont pas été touchés, sauf les lignes des README que les corrections ci-dessus changent.
- **Commentaire Swift faux** (« `kn_session_push_microphone` ne bloque pas », `apps/macos/KyberKit/AudioPlayerCore.swift:411-415`) : il devient vrai avec COEUR-31, mais le fichier est hors zone.
- **Ligne de tête de `ci.yml`** (« le cœur Rust … ne [l']est pas non plus ») : à mettre à jour par la session Outillage, qui tient `ci.yml` ; le cœur a son propre workflow.
- **CI sur une cible 32 bits** (proposée par COEUR-49 pour COEUR-02) : `cargo check --target armv7-…` demande les sources de Kyber et une libkynput croisée ; pas fait. L'essai `horloge_croissante` et `timespec_micros` couvrent le calcul, pas la taille réelle du `long` sur armv7.

## Ce qui est vérifié, et comment

- Arbre à moi (`KYBERNOW_ARBRES=…/third_party/arbres-coeur scripts/arbres.sh preparer apple`), correctifs des apps posés comme `build-apple.sh` (PyroWave compris), Rust 1.89 : `cargo test -p kybernow-core --features pyrowave --lib` — **464 réussis, 0 échec, 4 ignorés** (444 au départ de la branche).
- Un essai par correctif : `perte_sur_0xffff`, `saut_d_index_borne` (01) ; `horloge_croissante` (02) ; `en_tete_opus_incoherent` (03) ; `micro_et_camera_gardes_apres_le_redemarrage` (04) ; `au_dela_de_256_unites` (05) ; `big_toujours_termine` (06) ; `hexadecimal_sur_des_octets`, `decodage_sans_panique` (07) ; `pointeur_trop_grand_refuse` (08) ; `taille_nulle_ignoree` (09) ; `panique_rattrapee`, `rappel_c_protege` (ffi) ; `libre_tout_passe_et_axes_immobiles_ecartes` étendu (10) ; `stick_tenu_pendant_la_barre` (11) ; `panique_du_fil_arrete_les_fils_de_fond` (12) ; `fin_apres_les_preparations_en_vol` (13) ; `second_accord_fait_repartir_l_invite` et le cas « réseau lent » de `join_anywhere` (15) ; `mixage_descendant` mis à jour (16) ; `premier_certificat_exige_ensuite` (19) ; `trop_de_codes_faux_arretent_l_ecoute` (20) ; `flux_remplace_libere_hors_du_fil_temps_reel` (30) ; `capture_sans_verrou_bloquant` (31) ; `start_pas_perdu_file_pleine` (caméra). COEUR-24 : vérifié par la compilation et les essais PlayStation existants (pas d'essai de concurrence).
- Script de la CI essayé sur ce Mac depuis un clone neuf de la branche (dans le dossier temporaire de la session) : clonage de kyber-desktop 0.29.0 et de ses sous-modules depuis GitLab, 17 correctifs posés, libkynput par cargo-c, libvlc factice, essais — **463 réussis, 0 échec, 4 ignorés** (clone au commit du script, avant le correctif de la caméra) ; libopus de Homebrew ; 32 s pour le clonage et les correctifs. Le workflow lui-même (Ubuntu 24.04, paquets apt) n'a pas tourné sur GitHub : pas de push depuis cette session.
- `shellcheck -x` sur le script : propre. `cargo clippy -p kybernow-core --features pyrowave --all-targets` : aucun avertissement sur les lignes changées (les 52 qui restent sont antérieurs à la branche).

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

EN:
- PlayStation consoles: sound no longer freezes after about ten minutes when a packet is lost at the wrong moment, and some resolutions that always failed ("the console doesn't answer") now start.
- Controllers on Mac: a stick or trigger no longer stays pushed on the computer after a burst, or after opening and closing the bar while holding it.
- If the computer had to restart its Station to connect you, your microphone and camera keep working.
- 5.1 or 7.1 sound plays stereo on the right speakers (the right channel no longer goes to the centre).
- Sharing: joining through the relay no longer leaves a ghost seat on the host.
- No more sound or microphone dropouts when the picture starts or when you switch microphones.
- Security: a Station seen for the first time keeps its certificate for the whole connection, and sending PlayStation accounts between your devices stops after too many wrong codes.
- If something goes wrong inside the app, the session ends cleanly on the computer and the error is written to the app's log.
