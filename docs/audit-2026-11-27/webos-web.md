# Audit du 27/11/2026 : app LG webOS et client web

Zone : `apps/webos` (avec `kyber-patches` et `sdl-patches`) et `server/web` (console Admin `src/pages` + `src/lib`, client de streaming `src/link`), hors `node_modules` et `dist`. État : `main` à `1fb289c6` (0.11.7).

Méthode : lecture statique seulement (rien compilé, rien lancé, aucun téléviseur ni navigateur). Quatre relectures en parallèle (webOS média, webOS application et interface, client web `link`, console Admin), puis chaque constat retenu a été relu dans le code. **Vérifié** : le chemin de code a été lu de bout en bout. **Plausible** : le code est lu, mais l'effet dépend d'un comportement qu'on ne voit pas d'ici (NDL, lecteur de LG, navigateur, ordre d'arrivée des rappels du cœur). Rien de ce qui suit ne figure dans `docs/archives/audit-2026-10-20.md` comme réglé ; le point « pas d'état injoignable » de la console (WEB-06) y était signalé et reste ouvert.

Aucun `TODO`, `FIXME` ni `XXX` dans les deux zones.

## Synthèse

| Id | Catégorie | Où | Constat | Effort | Confiance |
|---|---|---|---|---|---|
| WEBOS-01 | bug critique | `apps/webos/src/model.rs:645` | Fichier de l'app illisible → remplacé par un fichier vide et réécrit aussitôt (ordinateurs, comptes perdus) | S | vérifié |
| WEBOS-02 | bug | `src/device/media.rs:146`, `:613` | La fin d'un flux décharge la chaîne Dolby / NDL du flux suivant (états globaux sans propriétaire) | M | plausible |
| WEBOS-03 | bug | `src/device/ndl.rs:670`, `:379`, `:417` | Déchargement demandé par l'app pris pour un déchargement du téléviseur | S | plausible |
| WEBOS-04 | bug | `src/device/esplayer.rs:256` | Horodatage Dolby qui ne recule jamais : retard du son cumulatif | M | code vérifié, effet plausible |
| WEBOS-05 | bug | `src/device/media.rs:498` | Chargement Dolby bloquant (jusqu'à ~5 s) dans le rappel des paquets son | M | plausible |
| WEBOS-06 | bug | `src/device/session.rs:696`, `ndl.rs:69` | Plan de son NDL global jamais remis pour les consoles et les mires | S | vérifié |
| WEBOS-07 | bug | `src/device/esplayer.rs:314` | `RELOADING` baissé par un rechargement périmé | S | plausible |
| WEBOS-08 | bug | `src/device/ndl.rs:449` | Verrou NDL tenu jusqu'à 500 ms pendant l'amorçage ; l'interface le lit toutes les 2 s | S | vérifié |
| WEBOS-09 | bug | `src/device/mod.rs:330`, `pads.rs:210` | Stick tenu au moment de « Reprendre » : navigation rejouée toutes les 130 ms pendant le jeu | S | vérifié |
| WEBOS-10 | bug | `src/device/session.rs:72`, `mod.rs:306` | Événements du cœur (vibrations, combinaison, pointeur) en retard jusqu'à 50 ms | S | vérifié |
| WEBOS-11 | bug | `src/app.rs:2818`, `mod.rs:532` | Retour sur l'accueil quitte l'app sans annuler l'emprunt en attente | S | vérifié |
| WEBOS-12 | bug | `src/app.rs:2704`, `mod.rs:591` | OK déclenche un focus qui n'est pas à l'écran ; dialogue en jeu non compté | S | vérifié |
| WEBOS-13 | bug | `src/device/mod.rs:874` | Réponse tardive de « Par adresse » qui ramène sur Ajouter par-dessus une session | S | vérifié |
| WEBOS-14 | bug | `src/device/mod.rs:595`, `sdl.rs:31` | Retour au premier plan non traité (sortie de veille) | M | plausible |
| WEBOS-15 | bug | `src/device/autotest.rs:114` | Test Auto annulé : son `ndl::unload()` final peut décharger la session suivante | S | plausible |
| WEBOS-16 | bug | `src/device/pads.rs:82`, `:91` | Manette déjà ouverte ou 5e manette : jamais refermée | S | vérifié |
| WEBOS-17 | bug | `src/model.rs:851`, `app.rs:4698` | Ordinateur retiré : aperçu laissé sur disque, recréé par une sonde en cours | S | vérifié |
| WEBOS-18 | bug | `src/device/eac3.rs:50`, `src/eac3_shim.c:20`, `:91` | Erreurs E-AC-3 perdues sans trace ; taille de trame 1536 en dur | S | vérifié |
| WEBOS-19 | bug | `src/device/log_file.rs:30` | `kybernow.log` sans limite de taille | S | vérifié |
| WEBOS-20 | optimisation | `src/device/mod.rs:290`, `app.rs:2637`, `ui/canvas.rs:289`, `device/offer.rs:48` | Redessins plein écran à 60 i/s inutiles, copies de glyphes, `offers.tick` à chaque tour | S-M | vérifié |
| WEBOS-21 | optimisation | `src/hdr.rs:131` | Chaque unité NAL désémulée (copiée) avant d'en lire le type | S | vérifié |
| WEBOS-22 | optimisation | `src/device/media.rs:436`, `eac3.rs:45` | Allocations par paquet et `Mutex<u64>` sur le chemin Dolby | S | vérifié |
| WEBOS-23 | code mort | `src/main.rs:16` | Le générateur de captures (`preview.rs`, 3 265 lignes) est livré sur le téléviseur | S | vérifié |
| WEBOS-24 | code mort | `src/starfish_hook.c:72`, `src/device/starfish.rs:27` | Crochet `Feed` qui ne réécrit rien mais intercepte chaque trame avec une ABI devinée | S | vérifié |
| WEBOS-25 | code mort | `src/device/sdl.rs`, `session.rs:697` | Déclarations SDL inutilisées masquées par `allow(dead_code)`, reste `let _ = (w, h, codec)` | S | vérifié |
| WEBOS-26 | factorisation | `src/station_offer.rs`, `src/app.rs`, `ui/canvas.rs`, `i18n.rs` | Interface Rust recopiée entre webOS et `apps/linux` | M-L | vérifié |
| WEBOS-27 | factorisation | `src/hdr.rs:142`, `ndl.rs:230`, `esplayer.rs:74` | Découpage NAL écrit trois fois (cœur, PlayStation, webOS) ; `sym<T>` en double | S | vérifié |
| WEBOS-28 | doc | `apps/webos/README.md:183-184` | « Pas de dérive » et « 2,5 s au plus » faux | S | vérifié |
| WEBOS-29 | dette | `scripts/build.sh:190`, `:196` | FFmpeg téléchargé sans somme de contrôle ; `--cpu=cortex-a73` pour tous les téléviseurs | S | vérifié |
| WEB-01 | bug | `server/web/src/link/pyrowave/live.ts:187` | Après une perte du GPU, PyroWave ne revient jamais ; en session, image figée sans message | S-M | vérifié |
| WEB-02 | bug | `src/link/pyrowave/live.ts:151` | `s.done.then` sans `catch` : rejet non géré et `ImageBitmap` non fermé | S | vérifié |
| WEB-03 | bug | `src/pages/Lend.svelte:60` | Champ « Votre prénom » remis à zéro toutes les 3 s pendant la saisie | S | vérifié |
| WEB-04 | bug | `src/lib/store.svelte.ts:22`, `Devices.svelte:125`, `Recordings.svelte:64`, `System.svelte:324` | Relevés périodiques qui écrasent un état plus récent | M | vérifié |
| WEB-05 | bug | `src/pages/System.svelte:93` | Minuterie de mise à jour jamais arrêtée au démontage ; erreur « délai » après une MAJ réussie | S | vérifié |
| WEB-06 | bug | `src/App.svelte:93` | `/api/session` injoignable au chargement : rejet non géré, roue sans fin | S | vérifié |
| WEB-07 | bug | `src/lib/api.ts:800` | Erreurs réseau brutes du navigateur (en anglais) dans les toasts, toutes les 5 s sur Compte | S | vérifié |
| WEB-08 | bug | `src/pages/Devices.svelte:101`, `ImageSound.svelte:131`, `Overview.svelte:93` | `$effect` qui suivent des lectures synchrones : requêtes en double, aperçu cassé au changement de langue | S | vérifié |
| WEB-09 | bug | `src/link/touch.ts:56` | `TouchInput.destroy()` laisse l'appui long et le glisser en cours | S | vérifié |
| WEB-10 | bug | `src/link/Session.svelte:2066` | Démontage incomplet (bandeau PyroWave, sonde du son, minuterie de `onPipeline`) | S | plausible |
| WEB-11 | bug | `src/link/heartbeat-call.ts:38`, `heartbeat.worker.ts:22` | Battements sans délai ni garde : empilement, jeton d'invité plus ancien qui écrase le récent | S | plausible |
| WEB-12 | bug | `src/link/Home.svelte:74`, `:122` | Aperçu `blob:` perdu au démontage ; relevés en onglet masqué | S | vérifié |
| WEB-13 | bug latent | `src/link/Session.svelte:1769`, `micro.svelte.ts:121` | Démarrage du micro annulé = choix du micro éteint pour cet ordinateur | S | vérifié |
| WEB-14 | sécurité | `src/link/StationPanel.svelte:19` | Un jeton de streaming de 12 h signé juste pour afficher un nom | S | vérifié |
| WEB-15 | sécurité | `src/link/lend.ts:37-83` | Mot de passe d'ami (si demandé) et code de prêt en clair dans `localStorage` | S | vérifié |
| WEB-16 | bug | `src/App.svelte:533` | Barre d'onglets à 8 entrées sur téléphone sans gestion du débordement | S | plausible |
| WEB-17 | optimisation | `src/link/main.ts:7`, `pyrowave/state.svelte.ts:7` | Sonde PyroWave (GPU « haute performance ») à chaque chargement, code importé statiquement | M | vérifié |
| WEB-18 | optimisation | `src/link/inputs.ts:107`, `Session.svelte:1893`, `:2034` | Allocations par mouvement, `getBoundingClientRect` par mouvement, métriques réactives toutes les 500 ms | S | vérifié |
| WEB-19 | optimisation | `src/App.svelte:144` et pages Admin | Relevés Admin (~0,75 requête/s) même onglet masqué | S-M | vérifié |
| WEB-20 | factorisation | `src/lib/api.ts:795`, `src/link/backend.ts:21` | Deux `request()` et cinq `fetch` écrits à la main | S-M | vérifié |
| WEB-21 | factorisation | `src/link/Session.svelte` | 3 708 lignes : invités, prêt, micro, enregistrement, barre mêlés | L | vérifié |
| WEB-22 | factorisation | `src/lib/LendPanel.svelte:109`, `SharePanel.svelte:138`, `pages/Users.svelte:35` | Copie dans le presse-papiers et liste « a, b et c » réécrites | S | vérifié |
| WEB-23 | code mort | `src/lib/i18n/fr.json`, `en.json` | 22 clés de traduction inutilisées ; `i18n-check.mjs` ne le voit pas | S | vérifié |
| WEB-24 | code mort | `src/link/keyswap.ts:17`, `heartbeat.worker.ts:42`, divers | Fonctions et exportations inutilisées, pile micro chargée alors que masquée | S | vérifié |
| WEB-25 | doc | `docs/link-ui.md:146`, `:197` | Stylet et micro annoncés dans le client web, qui ne les a pas | S | vérifié |
| WEB-26 | doc | `server/README.md:22`, `README.en.md:22` | « Cinq espaces » : la console en a huit | S | vérifié |
| WEB-27 | dette | `src/lib/InviteDialog.svelte:44`, `pages/System.svelte:469` | Effet de bord dans un `$derived` ; « Kyber 0.29 » écrit en dur | S | vérifié |

Bilan : 56 constats. **1 bug critique**, 34 bugs (18 webOS, 16 web, dont 2 de sécurité et 1 latent), 6 optimisations, 5 factorisations, 5 code mort, 5 doc/dette.

---

## App LG webOS (`apps/webos`)

Chemins relatifs à `apps/webos/` sauf mention contraire.

### WEBOS-01 — Fichier de l'app remplacé par un fichier vide au lancement (bug critique, vérifié)

`src/model.rs:645` : `std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()`, puis `s.save()` à `:658`, sans condition.

- **Scénario** : une version plus ancienne lit un fichier écrit par une plus récente (retour de la bêta à la stable, ou réinstallation d'un `.ipk` antérieur) ; une variante d'enum inconnue (`Profile` à `:22`, sans `#[serde(other)]`, ou tout autre enum sérialisé) fait échouer la désérialisation. Le fichier est aussitôt réécrit vide : ordinateurs appairés, mots de passe, comptes, amis, emprunts perdus pour de bon. Idem pour un fichier tronqué.
- **Correctif** : en cas d'échec de lecture ou d'analyse, renommer en `kybernow.json.bad` (horodaté), journaliser, ne rien réécrire tant que l'utilisateur n'a rien changé ; `#[serde(other)]` (ou une valeur par défaut) sur les enums persistés ; test de lecture d'un fichier « du futur ».
- **Même code dans `apps/linux/src/model.rs:783`** (hors zone, à signaler à l'audit Linux).
- Effort S.

### WEBOS-02 — La fin d'un flux décharge la chaîne du flux suivant (bug, plausible)

`src/device/media.rs:146-147` : à la sortie de `Feeder::run`, `esplayer::unload()` puis `ndl::unload()` ; `media.rs:613-617` : `AudioSink::drop` décharge aussi la chaîne Dolby. `esplayer` et `ndl` sont des états globaux ; `esplayer::unload` incrémente `EPOCH` (`esplayer.rs:341-342`), ce qui annule tout chargement en cours.

- **Scénario** : en 5.1 Dolby, l'image est relancée (changement de mode, « Image relancée »). Le cœur crée de nouveaux puits avant que les anciens soient libérés (libération au `Closed` de Kyber, asynchrone). Le nouvel `AudioSink` charge la chaîne Dolby, puis l'ancien fil d'image se termine et la décharge : chargement abandonné → stéréo SDL, ou `feed` répond « pas chargée », traité comme fatal par `feed_dolby` (`media.rs:415-419`) → **stéréo pour toute la session**. NDL subit la même chose (rechargé à l'image clé suivante, coupure visible).
- **Correctif** : `load()`/`ensure()` rendent un jeton (génération) ; `unload(jeton)` ne fait rien si ce n'est plus le chargement courant.
- Effort M.

### WEBOS-03 — Déchargement de l'app pris pour un déchargement du téléviseur (bug, plausible)

`ndl::unload` (`src/device/ndl.rs:670-678`), le rechargement de `ensure_any` (`:379-380`) et la réinitialisation (`:417`) appellent `api.unload` sans remettre `LOAD_COMPLETED` à faux. Le rappel `0x17` (`:302-304`) conclut alors « déchargé par le téléviseur » et lève `TV_UNLOADED`. `esplayer` fait l'inverse, correctement (`esplayer.rs:332`, `LOADED` baissé avant).

- **Scénario** (si le rappel arrive avant le chargement suivant) : à chaque changement de format, de taille ou de HDR, `play_any` voit `UNLOADED_BY_TV`, oublie `loaded` sans décharger ; le `Load` suivant est refusé, puis `Quit`/`Init` complet : seconde coupure d'image et avertissement trompeur au journal.
- **Correctif** : `LOAD_COMPLETED.store(false)` avant chacun des trois `api.unload`.
- Effort S.

### WEBOS-04 — Le retard du son Dolby ne peut que croître (bug, code vérifié, effet plausible)

`src/device/esplayer.rs:256` : `pts = max(loaded_at.elapsed(), last_pts + 32 ms)`. Une rafale pousse l'horodatage devant l'horloge et il ne revient jamais ; une dérive d'horloge PC/téléviseur s'accumule pareillement. Le PCM 5.1 espace ses paquets de +1 ms (`ndl.rs:598`) au lieu de leur durée.

- **Scénario** : la rafale qui suit WEBOS-05, ou une gigue Wi-Fi, ajoute un retard du son qui reste jusqu'à la fin de la session.
- **Correctif** : horodater d'après le nombre d'échantillons et recaler (jeter des trames) quand l'avance dépasse ~100 ms ; corriger le README (WEBOS-28).
- Effort M.

### WEBOS-05 — Chargement Dolby bloquant dans le rappel des paquets son (bug, plausible)

`AudioSink::configure` appelle `esplayer::load` (`media.rs:498`) depuis le rappel de Kyber ; `start` attend NDL (`esplayer.rs:184-186`, jusqu'à 2,5 s) puis l'amorçage (`:221-228`, jusqu'à 2,5 s). Les paquets s'accumulent et arrivent en rafale (alimente WEBOS-04) ; si Kyber livre image et son sur le même fil, l'image gèle.

- **Correctif** : charger sur un fil à part, route « Dolby en attente » qui jette les paquets d'ici là.
- Effort M.

### WEBOS-06 — Plan de son NDL jamais remis pour les consoles et les mires (bug, vérifié)

`ndl::set_audio_plane` n'est appelé que par `session::start` (`src/device/session.rs:696`) et par `ndl::bench` (`ndl.rs:713`). Une partie PlayStation (`device/playstation.rs:583-585`) et `ensure_mire` (via `wanted_plane()`, `ndl.rs:73`) gardent le plan de la session précédente.

- **Scénario** : après une session en 5.1 PCM, une partie PlayStation charge NDL avec le plan PCM 6 voies alors que son son passe par SDL ; sur un téléviseur sans multicanal, chargement refusé, Quit/Init.
- **Correctif** : passer le plan en paramètre de `ensure`, ou `set_audio_plane(OpusClock)` dans le chemin console et dans `ensure_mire`.
- Effort S.

### WEBOS-07 — `RELOADING` baissé par un rechargement périmé (bug, plausible)

`src/device/esplayer.rs:314` : le fil de rechargement fait `RELOADING.store(false)` sans vérifier que son époque est encore la courante. Un rechargement A d'une session finie qui se termine pendant le rechargement B de la suivante rouvre `feed`, qui reçoit « pas chargée » → stéréo définitive. Fenêtre étroite.

- **Correctif** : ne baisser le drapeau que si `EPOCH == epoch`.
- Effort S.

### WEBOS-08 — Verrou NDL tenu pendant l'amorçage (bug, vérifié)

`src/device/ndl.rs:449-455` : la boucle d'amorçage (`PRIME_BUDGET` 500 ms, pauses de 2 ms) tourne avec le verrou `NDL` pris. `ndl::multichannel()` (`ndl.rs:628`), appelé par la boucle principale toutes les 2 s dans Réglages › Son (`device/mod.rs:456`), y compris depuis le menu de session, attend d'autant : l'interface fige jusqu'à 0,5 s.

- **Correctif** : relâcher le verrou entre deux salves, ou mettre en cache l'état multicanal dans un atomique.
- Effort S.

### WEBOS-09 — Stick tenu : navigation rejouée pendant le jeu (bug, vérifié)

`src/device/mod.rs:330-333` appelle `pads.repeat_nav()` à chaque tour, en jeu compris ; en jeu, les axes partent à l'ordinateur sans passer par `pads.nav`, seul endroit qui remet `held` à `None` (`pads.rs:210-222`).

- **Scénario** : menu ouvert, stick poussé à gauche, A sur « Reprendre », stick relâché en jeu : `app.nav(Left)` rejoué toutes les 130 ms ; le focus a dérivé à la réouverture du menu.
- **Correctif** : vider `held` quand les entrées partent au jeu, ou n'appeler `repeat_nav` qu'hors jeu.
- Effort S.

### WEBOS-10 — Événements du cœur en retard jusqu'à 50 ms (bug de latence, vérifié)

`Shared::send` (`src/device/session.rs:72-74`) ne fait qu'un `tx.send` ; la boucle dort dans `SDL_WaitEventTimeout(…, 50)` (`mod.rs:306-309`) quand rien n'anime. Vibrations, combinaison du menu, état, forme du pointeur arrivent jusqu'à 50 ms après, tant qu'aucun événement SDL ne réveille la boucle (manette immobile, télécommande posée).

- **Correctif** : un type d'événement SDL (`SDL_RegisterEvents`) poussé par `SDL_PushEvent` depuis `send`, comme `wake_main` de `apps/linux/src/device/video.rs:127`.
- Effort S.

### WEBOS-11 — Retour sur l'accueil quitte sans annuler l'emprunt (bug, vérifié)

`src/app.rs:2818` : sur l'accueil, Retour rend toujours `Action::Quit`, même quand la carte affiche « En attente de {hôte}… ». La sortie (`device/mod.rs:532-542`) arrête la session et les appairages mais n'envoie ni annulation d'emprunt ni départ du partage.

- **Scénario** : l'hôte voit une demande d'un téléviseur parti ; s'il accepte, le prêt est ouvert dans le vide jusqu'au délai de 15 min.
- **Correctif** : sur l'accueil, Retour annule d'abord une demande en attente (comme Annuler de l'écran du code, `app.rs:2852`) ; `Quit` envoie aussi `BorrowCancel`/`JoinCancel`.
- Effort S.

### WEBOS-12 — OK sur un focus invisible ; dialogue en jeu (bug, vérifié)

`Nav::Ok` active `self.focus` sans vérifier qu'il est dans `hits` (`src/app.rs:2704-2706`).

- **Cas 1** : `Dialog::Force` s'ouvre quel que soit l'écran (`app.rs:4666`) ; Annuler remet le focus sur `Target::Setting(Restart/Update)` (`:4623`), absent de l'accueil ; l'OK suivant relance la demande à la Station sans rien d'affiché.
- **Cas 2** : un dialogue arrivé en jeu est dessiné (`ui/mod.rs:75`) mais `in_game` (`device/mod.rs:591`) ne tient pas compte de `app.dialog` : OK et flèches partent à l'ordinateur.
- **Correctif** : si le focus n'est pas dans `hits`, OK le ramène sur le premier élément ; `close_dialog` choisit le focus selon l'écran ; `in_game` exige `dialog.is_none()`.
- Effort S.

### WEBOS-13 — Réponse tardive de « Par adresse » (bug, vérifié)

`Event::Probed` (`src/device/mod.rs:874-882`) fait `app.go(Screen::Add)` sans vérifier l'écran courant ; la sonde peut durer 5 s. Si l'utilisateur est reparti et a ouvert une session, l'écran Ajouter passe par-dessus.

- **Correctif** : ignorer la réponse hors de `Screen::Add` / étape Adresse / `busy`.
- Effort S.

### WEBOS-14 — Retour au premier plan ignoré (bug, plausible)

Seul `SDL_APP_WILLENTERBACKGROUND` est traité (`src/device/mod.rs:595-599`, relâche les entrées). `SDL_APP_DIDENTERFOREGROUND` est déclaré (`sdl.rs:31`) mais jamais lu, alors que SDL-webOS l'émet. Au retour de veille, rien ne relance sondage, réseau, comptes, ni image clé ; si le déchargement NDL par le téléviseur n'est pas notifié, l'image reste figée. Les minuteries sur `Instant` décalent les vérifications de mise à jour.

- **Correctif** : au premier plan, forcer un sondage, vérifier `ndl::is_loaded`, demander une image clé ou relancer la session.
- Effort M. À essayer sur un téléviseur.

### WEBOS-15 — Test Auto annulé et session suivante (bug, plausible)

`src/device/autotest.rs:114` : `ndl::unload()` inconditionnel à la fin du fil. Annulé puis suivi d'une session pendant une mesure réseau encore en cours, il décharge le décodeur de la session (image noire, rechargement).

- **Correctif** : ne décharger que si le test a lui-même chargé (jeton de WEBOS-02).
- Effort S.

### WEBOS-16 — Manettes jamais refermées (bug mineur, vérifié)

`src/device/pads.rs:82-84` (instance déjà connue) et `:91` (`?` quand les 4 places sont prises) sortent sans `SDL_GameControllerClose(ctrl)` ; chaque ouverture SDL est comptée.

- **Correctif** : fermer `ctrl` sur ces deux sorties. Effort S.

### WEBOS-17 — Restes d'un ordinateur retiré (bug mineur, vérifié)

`Store::remove` (`src/model.rs:851-860`) laisse `images/{id}.jpg` ; une sonde en cours recrée `live[id]` (`app.rs:4698`) et réécrit l'aperçu (`device/mod.rs:848-853`).

- **Correctif** : supprimer l'image ; ignorer les événements dont l'id n'est plus dans le fichier. Effort S.

### WEBOS-18 — Encodeur E-AC-3 : erreurs silencieuses, taille en dur (bug, vérifié)

`src/device/eac3.rs:50-55` ignore un retour ≤ 0 ; `src/eac3_shim.c:91-93` rend 0 si le paquet dépasse le tampon : la trame disparaît sans journal. `kn_eac3_frame_size()` rend 1536 en dur (`eac3_shim.c:20-22`) alors que l'encodeur lit `ctx->frame_size` échantillons : une divergence future ferait lire hors du tampon.

- **Correctif** : taille lue sur l'encodeur ouvert ; compter et journaliser une fois les erreurs. Effort S.

### WEBOS-19 — Journal sans limite (bug, vérifié)

`src/device/log_file.rs:30-38` : `kybernow.log` n'est tourné qu'au lancement. Une app laissée des jours ouverte (veille rapide du téléviseur) grossit sans fin sur un stockage réduit.

- **Correctif** : rotation au-delà de ~5 Mo. Effort S.

### WEBOS-20 — Redessins et calculs inutiles (optimisation, vérifié)

- Join, Compte, Manettes se redessinent à 60 i/s (`device/mod.rs:290-293`) : 1920 × 1080 au processeur et ~8 Mo vers la texture par image, pour un curseur qui clignote toutes les 530 ms (`ui/join.rs:24`) ; le QR est recalculé à chaque image (`ui/widgets.rs:310`).
- `pointer_moved` rend toujours `true` (`app.rs:2637-2645`) : chaque mouvement de la Magic Remote redessine tout, même sans changement de focus (`App::pointer` n'est pas lu par le dessin).
- `Canvas::text` clone la couverture de chaque glyphe à chaque dessin (`ui/canvas.rs:289`).
- `offers.tick` à chaque tour de boucle, en session comprise (`device/mod.rs:355`, `device/offer.rs:48-75`), clone un contexte par ordinateur.
- **Correctif** : redessin à la demande (échéances), `pointer_moved` → « focus changé », emprunt de la couverture, `offers.tick` après un sondage ou une fois par seconde. Effort S-M.

### WEBOS-21 — HDR : copie de chaque NAL (optimisation, vérifié)

`src/hdr.rs:131` désémule chaque unité avant de regarder son type : ~1 Mo copié par image clé 4K ; le fil des mires rescanne la même mire toutes les 100 ms (`device/mires.rs:75`). Filtrer les types 33/39 avant `unescape`, mettre en cache par mire. Effort S.

### WEBOS-22 — Allocations du chemin Dolby (optimisation, vérifié)

Un `Vec` par paquet (`media.rs:436`), `drain` + `to_vec` + `Vec<Vec<u8>>` par trame (`eac3.rs:45-56`), compteurs `Mutex<u64>` (`media.rs:373`, `405`, `456`), `route` verrouillé deux fois par paquet. Tampons réutilisés et `AtomicU64`. Effort S.

### WEBOS-23 — Le générateur de captures est livré (code mort, vérifié)

`mod preview;` (`src/main.rs:16`) et la branche `--preview` (`:24-29`) sont compilés sans condition : 3 265 lignes de démonstration dans le binaire du téléviseur. Cela cache d'autre code mort : `App::open_link` (`app.rs:2496`) et `adjust_for_preview` (`app.rs:4500`) ne servent qu'à la prévisualisation (et aux tests).

- **Correctif** : `#[cfg(not(feature = "device"))]` ou une feature `preview` utilisée par `scripts/preview.sh`. Effort S.

### WEBOS-24 — Crochet `Feed` qui ne fait rien (code mort à risque, vérifié)

`kybernow_starfish_rewrite` rend toujours `NULL` et `kybernow_starfish_pts_scale` toujours 1 (`src/device/starfish.rs:27-35`), mais le crochet `Feed` (`src/starfish_hook.c:72-100`, exporté par `build.rs:79`) intercepte chaque trame du lecteur de LG avec une ABI `std::string` devinée ; si `dlsym` échoue, il rend `ret` sans l'avoir construit (`:77-78`).

- **Correctif** : retirer `Feed` et la réécriture, garder la journalisation de `Load` (diagnostic documenté dans le README). Effort S.

### WEBOS-25 — Restes (code mort, vérifié par grep dans tout le dépôt)

- `src/device/session.rs:697` : `let _ = (w, h, codec);`, reste d'une ancienne version.
- `src/device/sdl.rs` (sous `allow(dead_code)`) : `SDL_INIT_JOYSTICK`, `SDL_INIT_HAPTIC`, `SDL_APP_DIDENTERFOREGROUND` (voir WEBOS-14), `SDL_WINDOWEVENT`, `SDL_WINDOWEVENT_FOCUS_GAINED/LOST`, `SC_WEBOS_HOME`, `SC_WEBOS_CURSOR_SHOW`, `SDL_InitSubSystem`, `SDL_GetWindowSize`, `SDL_Start/StopTextInput`, `SDL_GetTicks`, `SDL_NumJoysticks`.
- Retirer l'`allow` pour que le compilateur les signale. Effort S.

### WEBOS-26 — Interface Rust recopiée entre webOS et Linux (factorisation, vérifié par diff)

- `src/station_offer.rs` : 106 lignes différentes sur 474 avec `apps/linux`.
- `duration_label`, `time_left_label`, blocs partage/prêt/emprunt mot pour mot (`apps/webos/src/app.rs:773`, `:1169` contre `apps/linux/src/app.rs:1030`, `:1447`) ; `recording_clock` des deux côtés ; `hour_label`/`weekday_label` (`app.rs:1629-1660`) contre `apps/linux/src/app_time.rs`.
- `ui/canvas.rs` et `i18n.rs` très proches ; `model::next_encoder_max` (`model.rs:90`) refait celle du cœur (avec test de parité).
- Noms de mois/jours FR/EN écrits trois fois dans webOS (`autotest.rs:142`, `ui/settings.rs:363`, `app.rs:1647`).
- **Correctif** : une petite bibliothèque d'interface sans dépendance (textes, durées, dates, canvas) partagée par `#[path]` comme `welcome.rs`, ou un crate ; `next_encoder_max` repris du cœur. Effort M-L.

### WEBOS-27 — NAL et `dlsym` en double (factorisation, vérifié)

Découpage NAL et retrait des octets d'émulation dans `apps/core/src/media.rs:226`, `apps/core/src/playstation/refs.rs:369` et `:392`, `apps/webos/src/hdr.rs:142` et `:166` : un module `nal` du cœur suffit. `sym<T>` identique dans `ndl.rs:230` et `esplayer.rs:74`. Effort S.

### WEBOS-28 — README faux sur la synchro Dolby (doc, vérifié)

`README.md:184` (et `README.en.md`) : « un décalage fixe, pas de dérive (chaque trame est horodatée à son arrivée) » est faux (WEBOS-04) ; `:183` « 2,5 s au plus » alors que le pire cas est ~5 s (WEBOS-05). À corriger avec les correctifs, FR et EN. Effort S.

### WEBOS-29 — Construction (dette, vérifié)

`scripts/build.sh:190` télécharge l'archive FFmpeg sans somme de contrôle ; `:196` compile avec `--cpu=cortex-a73` pour tous les téléviseurs (les plus anciens sont en Cortex-A53, l'option n'élargit pas le jeu d'instructions mais l'ordonnancement est celui de l'A73). Ajouter un `sha256` ; préférer `--cpu=generic-armv7-a` ou le documenter. Effort S.

---

## Client web (`server/web`)

Chemins relatifs à `server/web/` sauf mention contraire.

### WEB-01 — PyroWave ne revient jamais après une perte du GPU (bug, vérifié)

`src/link/pyrowave/live.ts:187-198` : la fermeture `create` garde le premier `gpu` pour toujours ; s'il est perdu, elle lance `pyrowaveGPU()` dont le nouveau device n'est jamais utilisé, et rend `null` : toutes les sessions suivantes de la page tombent en H.264. En session, `push()` rend `false` à chaque image (`live.ts:137-140`) sans rien signaler : **image figée, sans message**.

- **Scénario** : veille du Mac, réinitialisation du pilote, plantage du processus GPU de Chrome pendant une session PyroWave.
- **Correctif** : `let gpu` remplacé par le résultat de `pyrowaveGPU()` ; dans `LiveDecoder`, `gpu.lost` (ou N images sautées de suite) lève une erreur qui relance l'image.
- Effort S-M.

### WEB-02 — Rejet non géré et `ImageBitmap` perdu (bug, vérifié)

`src/link/pyrowave/live.ts:151` : `s.done!.then(...)` sans `catch`. `done` (`decoder.ts:350`) rejette si le device est perdu : un rejet non géré par image en vol, et `bitmap` jamais fermé. Dans `decoder.ts:353-360` (mesures, tests seulement), `rb` n'est pas détruit si `mapAsync` rejette.

- **Correctif** : `.catch(() => bitmap.close())` ; `rb.destroy()` en `finally`. Effort S.

### WEB-03 — « Votre prénom » remis à zéro pendant la saisie (bug, vérifié)

`src/pages/Lend.svelte:60-65` : le `$effect` appelle `loadView()`, qui lit `app.status?.lend` avant son premier `await` : l'effet dépend de `app.status`, remplacé toutes les 3 s par `refreshStatus` (`lib/store.svelte.ts:22-28`). Chaque passage relance `load()` (3 requêtes) qui réécrit `hostName = s.host_name` (`:46`) et recrée la minuterie.

- **Correctif** : `untrack(() => { load(); loadView() })`, comme `System.svelte:454-463`. Effort S.

### WEB-04 — Relevés qui écrasent un état plus récent (bug, vérifié)

Aucun relevé n'a de numéro de génération ni d'`AbortController`, et ils peuvent se chevaucher :

- `src/lib/store.svelte.ts:22-28` contre `setShare`/`setLend` : « Arrêter le partage » → `setShare(null)`, puis un relevé parti avant le clic remet le panneau jusqu'à 3 s.
- `src/pages/Devices.svelte:125-145` : un relevé du relais en vol remet l'interrupteur « Accès hors de chez vous ».
- `src/pages/Recordings.svelte:64-67` : `!busy` testé avant la requête, pas après.
- `src/pages/System.svelte:324-331` (`loadText`) : changement de fichier pendant la lecture → texte de l'ancien fichier affiché.
- **Correctif** : compteur de séquence dans le magasin, incrémenté à chaque écriture locale ; réponses antérieures jetées ; tour sauté si une requête est en cours. Effort M.

### WEB-05 — Guet de mise à jour jamais arrêté (bug, vérifié)

`src/pages/System.svelte:93-119` : `waitForRestart` crée un `setInterval` hors de tout nettoyage. La mise à jour redémarre Admin, les sessions Admin sont en mémoire (`server/admin/src/auth.rs:57`), la page passe à la connexion et System est démonté ; la minuterie continue, reçoit des 401, et au bout de 5 min affiche « délai dépassé » sur l'écran de connexion alors que la mise à jour a réussi.

- **Correctif** : garder la minuterie dans le composant et l'arrêter dans le nettoyage de l'effet. Effort S.

### WEB-06 — Roue sans fin si Admin ne répond pas (bug, vérifié)

`src/App.svelte:93-96` : `loadSession()` sans `catch` ; appelée par `onMount` (`:102`) et `logout` (`:161-164`). Station en redémarrage → rejet non géré, `session` reste `null`, roue sans fin, aucun nouvel essai. Point « pas d'état injoignable » de l'audit d'octobre, toujours ouvert.

- **Correctif** : état « Station injoignable » avec nouvel essai espacé. Effort S.

### WEB-07 — Erreurs réseau brutes dans les toasts (bug, vérifié)

`src/lib/api.ts:800` : `fetch` sans `try` ; l'erreur réseau du navigateur (« Failed to fetch », « Load failed » sous Safari), en anglais, part telle quelle dans `errorText`. `Account.svelte:42-46` recharge toutes les 5 s avec un toast d'erreur à chaque échec : pendant un redémarrage, un toast anglais toutes les 5 s dans une interface française.

- **Correctif** : `ApiError(0, t('common.unreachable'))` dans `request()` ; pages en relevé silencieuses sur le statut 0. Effort S.

### WEB-08 — Effets qui suivent des lectures synchrones (bug, vérifié)

`src/pages/Devices.svelte:101-117` : `loadKyber()` lit `kyber.loaded` avant son `await` ; quand il passe à vrai, tout l'effet repart (appairage, appareils, relais, invitations en double). Chaque appel d'API lit `i18n.lang` via `langHeaders()` (`lib/i18n/index.svelte.ts:138`) : un changement de langue relance les effets qui appellent l'API directement. Idem `ImageSound.svelte:131-137`, `App.svelte:141-146`, `Overview.svelte:93-111` (là, le nettoyage révoque l'URL `blob:` de l'aperçu encore affiché : image cassée jusqu'au prochain relevé).

- **Correctif** : `untrack` autour des appels ; garde « en cours » dans `loadKyber`. Effort S.

### WEB-09 — Toucher : appui long et glisser survivent (bug, vérifié)

`src/link/touch.ts:56-61` : `destroy()` retire les écouteurs mais pas la minuterie d'appui long (`:125`) ; un clic droit peut partir après la fin de session. Un glisser en cours (`dragging`) ne reçoit pas de `mouseup`.

- **Correctif** : `clearTimeout(this.longPress)` et `mouseup` si `dragging`. Effort S.

### WEB-10 — Démontage incomplet de la session (bug mineur, plausible)

`src/link/Session.svelte:2066-2112` : `pyroBannerTimer` (`:1041`) non arrêté ; `engine`/`phase` pas remis, donc la sonde du son (`:1662-1673`) continue d'interroger un moteur arrêté toutes les 200 ms jusqu'à 30 s ; la minuterie de 1 s de `onPipeline` (`:1203`) n'est pas suivie.

- **Correctif** : arrêter la minuterie du bandeau, `engine = null` après `retire`, invalider la sonde. Effort S.

### WEB-11 — Battements sans délai ni garde (bug, plausible)

`src/link/heartbeat-call.ts:38` : `fetch` sans `signal` ; `heartbeat.worker.ts:22-24` ne vérifie pas qu'un battement est en cours. Derrière un relais lent, ils s'empilent toutes les 5 s ; les réponses peuvent arriver dans le désordre et `onGuestBeat` réécrit `ticketToken` avec un jeton plus ancien (`Session.svelte:461`).

- **Correctif** : `AbortSignal.timeout(période)` et un battement à la fois. Effort S.

### WEB-12 — Accueil : aperçu perdu, relevés en onglet masqué (bug, vérifié)

`src/link/Home.svelte:74-81` : un `loadPreview()` en vol au démontage affecte une URL `blob:` jamais révoquée. `:122-131` : capture de l'écran toutes les 30 s et `info`/`status` toutes les 8 s, même onglet masqué (la capture coûte côté Station).

- **Correctif** : drapeau `alive` vérifié après l'`await` ; pas de relevé si `document.hidden`. Effort S.

### WEB-13 — Micro : démarrage annulé = choix éteint (bug latent, vérifié)

`src/link/Session.svelte:1769-1781` et `micro.svelte.ts:121-143` : si la session quitte `live` pendant `getUserMedia` (demande d'autorisation ouverte), l'effet appelle `mic.stop()`, `start` rend `'failed'` et `startMic` fait `setMicOn(hostname, false)` : le choix retenu pour cet ordinateur est perdu. Masqué tant que `engineSendsMicrophone()` rend `false` (`engine.ts:159`).

- **Correctif** : `start` rend `'cancelled'`, sans toucher au choix. Effort S.

### WEB-14 — Jeton de 12 h signé pour afficher un nom (sécurité, vérifié)

`src/link/StationPanel.svelte:19-24` appelle `/api/web/ticket` à chaque ouverture du panneau, uniquement pour `ticket.user` ; chaque appel signe un JWT de streaming de 12 h (`server/admin/src/api.rs:860-871`) jamais utilisé, qui traîne en mémoire.

- **Correctif** : exposer le compte par `/api/session` ou `/api/stream/info`. Effort S.

### WEB-15 — Secrets de l'ami dans `localStorage` (sécurité, vérifié)

`src/link/lend.ts:37-70` garde le mot de passe d'un compte d'ami en clair quand l'ami le demande ; `:80-83` garde la clé `code:{code}` du prêt en cours 15 min. C'est voulu (et l'origine est propre à chaque Station via le relais, `docs/relais.md`), mais rien ne le dit à l'ami ni ne l'efface au bout d'un temps.

- **Correctif** : texte explicite à côté de « Retenir le mot de passe » ; ne garder qu'une empreinte du code ; purge des comptes inutilisés depuis 90 jours. Effort S.

### WEB-16 — Barre d'onglets sur téléphone (bug, plausible)

`src/App.svelte:533-546` : 8 onglets (`:21-30`) sans `min-width: 0` ni gestion du débordement ; à 375 px, ~45 px par onglet pour des libellés d'un seul mot (« Utilisateurs »). Les derniers (Système, Compte) sortent de l'écran.

- **Correctif** : `min-width: 0` + ellipse, libellés masqués sous ~400 px, ou un onglet « Plus ». Effort S. À vérifier à 375 px.

### WEB-17 — Sonde PyroWave à chaque chargement (optimisation, vérifié)

`src/link/main.ts:7` lance `probeAtLoad()` à chaque page, même pour un invité ou un ami (forcés hors PyroWave, `Session.svelte:1029`) et face à une Station qui ne l'offre pas ; le device « haute performance » (`decoder.ts:83`) reste vivant toute la page (GPU discret réveillé sur un portable à deux GPU, 0,3-1,2 s de sonde). `pyrowave/state.svelte.ts:7` importe statiquement `live`, `decoder`, `shaders` (WGSL + image de test en base64) dans le paquet principal.

- **Correctif** : `import()` à la demande, sonde seulement pour l'hôte quand `info.pyrowave.available`. Effort M.

### WEB-18 — Chemin chaud des entrées et métriques (optimisation, vérifié)

- `src/link/inputs.ts:107-117` : deux `Uint8Array` et un `DataView` par mouvement de souris.
- `Session.svelte:1893-1900` : `getElementById` + `getBoundingClientRect` (mise en page forcée) par mouvement, sur le chemin WebSocket (Safari).
- `Session.svelte:2034-2040` : `latency`, `fps`, `spark`, `pyroStats` réaffectés toutes les 500 ms, barre et statistiques fermées ; `secondMedians` (`:2045`) grandit sans limite.
- **Correctif** : tampon de 9 octets réutilisé, rectangle mis en cache par `ResizeObserver`, métriques seulement si `barVisible || metricsOpen`, tableau borné. Effort S.

### WEB-19 — Relevés Admin onglet masqué (optimisation, vérifié)

Aucun `visibilitychange` côté Admin : `App.svelte:144`, `Overview.svelte:99-104`, `Devices.svelte:107-112`, `ImageSound.svelte:135`, `Lend.svelte:64`, `Account.svelte:45`, `System.svelte:349`. Un onglet laissé sur Vue d'ensemble envoie ~0,75 requête/s à la Station, en pleine partie.

- **Correctif** : un `poll(fn, ms)` commun : pause si `document.hidden`, relevé immédiat au retour, tour sauté si le précédent n'a pas répondu (règle aussi une partie de WEB-04). Effort S-M.

### WEB-20 — Deux enveloppes `fetch` (factorisation, vérifié)

`src/lib/api.ts:795-818` et `src/link/backend.ts:21-48` font la même chose (en-têtes, `x-kyber-admin`, JSON, `ApiError`) ; `fetch` écrits à la main dans `backend.ts:345` (`shareQuality`), `:510` (`leaveLend`), `share.ts:266`, `Session.svelte:2019` (doublon de `backend.end` en `keepalive`), `StationPanel.svelte:35`.

- **Correctif** : un seul `request()` exporté avec `{signal, headers, keepalive}` ; WEB-06/07 réglés au même endroit. Effort S-M.

### WEB-21 — `Session.svelte` trop gros (factorisation, vérifié)

3 708 lignes qui mêlent invités (qualité, repli, battements), prêt, micro, enregistrement et barre. Sortir la machine à états des invités et le micro dans des modules `*.svelte.ts`, comme `station.svelte.ts`. Effort L.

### WEB-22 — Petits doublons (factorisation, vérifié)

`src/lib/LendPanel.svelte:109-116` et `SharePanel.svelte:138-145` réécrivent `navigator.clipboard.writeText` au lieu de `copyText` (`lib/lend.ts:150`, à déplacer dans `format.ts`). `src/pages/Users.svelte:35` joint les noms à la main avec `users.accounts.and` au lieu de `list()` (`lib/i18n/index.svelte.ts:131`). Effort S.

### WEB-23 — Clés de traduction inutilisées (code mort, vérifié)

Absentes de tout `server/web/src` (clés dynamiques, pluriels et surcharges `macos.`/`linux.` pris en compte) et de tout le dépôt ; le cœur, le Mac et webOS ont leurs propres catalogues (`apps/core/src/i18n.rs:136` inclut `apps/core/src/i18n/*.json`, pas ceux du web), dans `fr.json` et `en.json` :

- `common.langAutoHint` ;
- `lend.page.intro`, `lend.page.switch`, `lend.page.switchHelp`, `lend.page.current`, et `macos.lend.page.{intro,switchHelp}`, `linux.lend.page.{intro,switchHelp}` (restes de la refonte de la page Prêt, d91085cb) ;
- `lend.apps.steam`, `lend.apps.steamHelp`, `lend.apps.none` ;
- `lend.friends.empty`, `lend.friends.lastUsed` ;
- `recording.on_computer`, `recording.record_menu`, `recording.stop_menu`, `recording.codec`, `recording.disk_full`, `recording.folder`, `recording.lent`, `recording.failed` (Admin envoie ses refus déjà traduits, `server/admin/src/recording.rs:292`).

`scripts/i18n-check.mjs` ne compare que la parité FR/EN et les paramètres : y ajouter la détection des clés inutilisées. Effort S.

### WEB-24 — Fonctions et exportations inutilisées (code mort, vérifié par grep)

- `src/link/keyswap.ts:17` `controlAsCommand()` : jamais appelée (Session et Réglages recalculent la règle) ; seul le champ de préférences porte ce nom.
- Branche `'stop'` du worker (`heartbeat.worker.ts:42-45`) : `Heartbeat` fait toujours `terminate()`.
- Pile micro chargée et `probeMicrophone()` lancée à chaque session (`Session.svelte:1727`) alors que `engine.ts:500-509` sont des bouchons et l'interface masquée : à charger à la demande.
- Exportés mais utilisés dans leur seul fichier : `loadEngine`, `canSendPackets`, `sendPacket`, `swapControlCommand`, `blamesAudio`, `elide`, `lendKey`, `COME_BACK_MS`, `resetPyroWaveStats`, `MIN_REFRESH`, `BITS_PER_PIXEL(_444)` ; `PyroWaveDecoder.lastGpuMs` et `readPlanes` ne servent qu'aux tests et au banc.
- Effort S.

### WEB-25 — `link-ui.md` promet stylet et micro au client web (doc, vérifié)

`docs/link-ui.md:146` range le client web parmi les apps à stylet, alors que `docs/stylet.md:71` dit « à faire » et que `src/link` ne traite pas `pointerType === 'pen'` ; `:197` le range parmi les apps à micro, alors que `docs/micro.md:73` le dit caché (moteur wasm sans flux montant). Marquer « pas encore dans le client web », FR et EN. Effort S.

### WEB-26 — README de la Station : « cinq espaces » (doc, vérifié)

`server/README.md:22` (« cinq espaces ») et son tableau `:24-30`, `server/README.en.md:22-30` : la console en a huit (`App.svelte:21-30`) ; Enregistrements, Compte et Prêt manquent au tableau. FR et EN dans le même commit. Effort S.

### WEB-27 — Dette mineure (vérifié)

- `src/lib/InviteDialog.svelte:44-50` modifie `waitFrom` dans un `$derived.by` (effet de bord) ; l'effet de relevé (`:54-68`) se recrée à chaque tour parce qu'il écrit `shown` dont il dépend.
- `src/pages/System.svelte:469` : « Kyber 0.29 » en dur ; Admin pourrait donner la version.
- Effort S.

---

## Dix priorités

1. **WEBOS-01** : ne plus jamais réécrire un `kybernow.json` illisible (et le même correctif dans `apps/linux`). Perte de données silencieuse au premier retour de version.
2. **WEB-01 + WEB-02** : PyroWave qui se relance après une perte du GPU, au lieu d'une image figée sans message.
3. **WEBOS-02 + WEBOS-03 + WEBOS-15** : un jeton de chargement pour NDL et la chaîne Dolby ; une seule correction couvre les trois déchargements intempestifs.
4. **WEBOS-04 + WEBOS-05 + WEBOS-28** : horodatage Dolby par échantillons et chargement hors du fil des paquets ; README corrigé.
5. **WEB-03 + WEB-08 + WEB-05** : effets Svelte de la console (`untrack`, minuterie nettoyée) ; le champ qui se vide est visible de tout hôte qui prête son PC.
6. **WEBOS-09 + WEBOS-12 + WEBOS-11** : télécommande et focus (stick qui dérive, OK invisible, Retour qui abandonne une demande d'emprunt).
7. **WEB-04 + WEB-19 + WEB-06 + WEB-07** : un `poll()` commun et un `request()` commun (WEB-20) : états périmés, onglet masqué, Station injoignable, erreurs en anglais.
8. **WEBOS-10 + WEBOS-08** : réveil de la boucle par événement SDL, verrou NDL relâché ; latence et figeages de l'interface.
9. **WEBOS-14** : reprise au premier plan (sortie de veille du téléviseur), à essayer sur la G2.
10. **WEBOS-23 + WEB-17 + WEB-23 + WEB-24** : nettoyage (prévisualisation hors du binaire, PyroWave chargé à la demande, clés et fonctions mortes) et vérification des clés inutilisées dans `i18n-check.mjs`.

## Ce qui est sain

- **webOS, appels système** : disposition de `HContext` identique à celle de la SDL webOS, libération du contexte luna correcte ; chemins d'échec de `kn_eac3_open` corrects ; ordre de fermeture de `AudioSink::drop` juste.
- **webOS, fils** : amorçage NDL (`carry_prime`) et rechargements Dolby s'arrêtent sur changement de génération ou d'époque ; mires et session se partagent le décodeur proprement (`MIRES`, `BUSY`, `NOT_LOADED`).
- **webOS, données** : `hdr.rs` borné et testé (primaires correctes en HEVC et AV1) ; fichier de l'app écrit atomiquement en 0600, aucun secret au journal, `Debug` de `KnAccount` masqué ; pas d'`unwrap` atteignable depuis le réseau hors tests ; numéros d'essai qui écartent les réponses tardives (`join`, `borrow`, `watch`, `auto`, `invitation`).
- **webOS, code partagé** : `welcome.rs` repris du cœur par `#[path]`, offre de Station calculée par le cœur, i18n conforme à `docs/i18n.md` ; correctifs SDL et Kyber petits, expliqués, appliqués de façon idempotente avec empreinte ; aucun journal par image.
- **Web, moteur** : `retire`/`allStopped` n'ont jamais deux moteurs à la fois, `stop()` borné, résultats périmés écartés par `generation` ; bascule WebTransport → WebSocket armée à `streaming` ; garde d'entrées qui relâche tout au `blur`, à l'onglet masqué et à la perte du verrou du pointeur ; battements dans un Worker.
- **Web, PyroWave** : tampons de l'analyseur réutilisés, groupes de liaison préparés une fois, trois images en vol au plus, `VideoFrame` et `ImageBitmap` fermés sur le chemin normal, `destroy()` complet.
- **Web, sécurité** : un seul `{@html}` (`Qr.svelte`, SVG de modules, rien de l'utilisateur) ; aucun jeton dans `localStorage` ni dans les URL côté `link` ; `redact()` avant tout journal ; écritures protégées par `x-kyber-admin`, cookies `HttpOnly` + `SameSite` ; noms encodés dans les liens des QR.
- **Web, paquet** : une seule dépendance d'exécution (`qrcode-generator`, utilisée) ; polices variables auto-hébergées (132 Ko) ; toutes les classes d'`app.css` servent ; Admin et `link` partagent déjà `SharePanel`, `LendPanel`, i18n et `format` ; `lib/lend.ts` (hôte) et `link/lend.ts` (ami) ne se recouvrent pas.
