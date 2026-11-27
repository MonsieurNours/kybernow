# Audit du 27/11/2026 — app Windows (`apps/windows`)

Relecture statique de `apps/windows` à l'état de `1fb289c6` (0.11.7) : couche Tauri et état (`src/ui.rs` et `src/ui/*`), rendu et décodage (`render.rs`, `video.rs`, `decoder.rs`, `hdr.rs`, `pyrowave*.rs`, `window.rs`), entrées, son, micro, caméra, presse-papiers, interface Svelte (`ui/src`), installateur `KyberNow-Client-Setup` (`setup/`), scripts et configuration. Rien n'a été compilé ni exécuté ; aucun essai sur Windows. Chaque constat a été relu dans le code avant d'être gardé.

- **vérifié** : le code lu suffit à prouver le défaut ;
- **plausible** : le code est conforme à la description, mais l'effet dépend d'un comportement de Windows, du pilote ou du moment (à confirmer sur la machine).

L'audit du 20/10 (`docs/archives/audit-2026-10-20.md`) précédait l'app Windows : aucun recouvrement. Aucun `TODO`/`FIXME` dans la zone.

## Synthèse

| ID | Catégorie | Constat | Où | Effort | Confiance |
|---|---|---|---|---|---|
| WINAPP-01 | bug critique | Carte graphique perdue : l'image se fige, aucune reprise | `src/render.rs:928` | L | vérifié |
| WINAPP-02 | bug critique | Installateur : `cmd /C start "" url` exécute ce qui suit un `&` | `setup/src/sys.rs:200` | S | vérifié |
| WINAPP-03 | bug | « Installer la mise à jour » coupe une partie PlayStation ou une connexion en cours | `src/ui.rs:991` | S | vérifié |
| WINAPP-04 | bug | `app.json` écrit sans atomicité, relu en silence s'il est abîmé (rallume l'ouverture à la connexion) | `src/ui.rs:951`, `:485` | S | vérifié |
| WINAPP-05 | bug | `save_prefs` écrit une copie hors du verrou : la plus ancienne peut gagner | `src/ui.rs` (15 appels) | S | plausible |
| WINAPP-06 | bug | Ordinateur enregistré même si son mot de passe n'a pas pu être gardé | `src/ui.rs:1533-1562`, `src/ui/lend.rs:288` | S | vérifié |
| WINAPP-07 | bug | Réglages synchronisés : deux envois peuvent partir dans le désordre | `src/ui/synced.rs:36`, `:230` | M | plausible |
| WINAPP-08 | bug | Panique sous le verrou `sessions` → verrou empoisonné, 180 `lock().unwrap()` paniquent ensuite | `src/ui.rs:3440`, `src/host.rs:301` | S | plausible |
| WINAPP-09 | bug | Instantané de l'accueil faux (autre écran ou noir) en multi-écrans | `src/render.rs:973-1004` | S | vérifié |
| WINAPP-10 | bug | `quit` détruit les fenêtres avant d'arrêter les fils de l'image ; HCURSOR perdus | `src/window.rs:1786` | S | vérifié |
| WINAPP-11 | bug | PyroWave : décodeur recréé et erreur journalisée à chaque image tant qu'il échoue | `src/video.rs:609` | S | vérifié |
| WINAPP-12 | bug | Crochet clavier bas niveau : verrous, copies et envoi dans le rappel | `src/window.rs:350-447` | M | plausible |
| WINAPP-13 | bug | Son et micro ne suivent pas le périphérique par défaut de Windows | `src/audio.rs:117`, `src/micro.rs:205` | M | vérifié |
| WINAPP-14 | bug | Ctrl/Alt restent tenus sur l'ordinateur après Ctrl+Alt+Suppr ou Win+L | `src/window.rs:2682` | S | plausible |
| WINAPP-15 | bug | Souris absolue (VM, Bureau à distance, tablette) immobile en mode jeu | `src/window.rs:2099` | S | vérifié |
| WINAPP-16 | bug | Impr. écran n'arrive jamais à l'ordinateur | `src/window.rs:2490-2566` | S | plausible |
| WINAPP-17 | bug | `WM_DPICHANGED` non traité par les fenêtres de session | `src/window.rs:2463` | S | plausible |
| WINAPP-18 | bug | `session_camera_state` synchrone : énumère les webcams sur le fil principal | `src/ui/camera.rs:214` | S | vérifié |
| WINAPP-19 | bug | Caméra : fin de session bloquée par un `ReadSample` figé, verrou `SESSIONS` tenu | `src/ui/camera.rs:100`, `src/camera.rs:137` | M | plausible |
| WINAPP-20 | bug | Caméra : encodeur matériel (MFT) jamais arrêté | `src/camera.rs:194-220`, `:418` | S | plausible |
| WINAPP-21 | bug | Caméra : tout échec à l'ouverture annoncé « utilisée par une autre app » | `src/camera.rs:179-188` | S | vérifié |
| WINAPP-22 | bug | Fuite d'un handle d'événement à chaque échec d'ouverture WASAPI | `src/audio.rs:146`, `src/micro.rs:261` | S | vérifié |
| WINAPP-23 | bug | Presse-papiers : lecture du texte sans borne | `src/pasteboard.rs:219` | S | plausible |
| WINAPP-24 | bug | Filtre d'exceptions : journal (mutex, allocation) pendant un plantage | `src/system.rs:244-257` | S | plausible |
| WINAPP-25 | bug | Micro : deux captures peuvent pousser en même temps | `src/micro.rs:173`, `src/host.rs:570` | S | plausible |
| WINAPP-26 | bug | Réglages d'une console : le clic droit ouvre le menu au bout de 300 ms | `ui/src/ConsoleSettings.svelte:101-122` | S | vérifié |
| WINAPP-27 | bug | Menu contextuel de WebView2 (« Actualiser »…) disponible partout | `ui/src/*.ts` | S | plausible |
| WINAPP-28 | bug | Changements optimistes sans retour arrière, rejets non attrapés | `ui/src/Bar.svelte:716-814` etc. | S-M | vérifié |
| WINAPP-29 | bug | Démarrage de l'accueil : 7 appels en chaîne sans `catch` (le lien `kybernow://` peut se perdre) | `ui/src/App.svelte:273-301` | S | vérifié |
| WINAPP-30 | bug | Feuille d'ajout fermée pendant `ps_handoff_receive`/`pair_start` : l'attente continue côté Rust | `ui/src/AddSheet.svelte:246-286` | S | plausible |
| WINAPP-31 | bug | Échap et focus incohérents dans les feuilles et les menus de la barre | `ui/src/AppSheet.svelte`, `Bar.svelte:1344` | S-M | vérifié |
| WINAPP-32 | bug | Installateur : fichiers retirés de la nouvelle version laissés en place | `setup/src/install.rs:154-216` | S | vérifié |
| WINAPP-33 | bug | Installateur : WebView2 téléchargé par PowerShell, lancé sans vérifier la signature | `setup/src/webview2.rs:33-54` | M | plausible |
| WINAPP-34 | bug | Installateur : ordre de recherche des DLL non protégé | `setup/src/bin/setup.rs:38` | S | plausible |
| WINAPP-35 | bug | `uninstall.exe` garde une entrée de signature qui pointe hors du fichier | `setup/src/payload.rs:181-187` | S | plausible |
| WINAPP-36 | optimisation | `mock.ts` (≈ 36 Ko, faux comptes, prénom de l'auteur) livré et exécuté dans l'app | `ui/src/lib/api.ts:11` | S | vérifié |
| WINAPP-37 | optimisation | L'événement `computers` renvoie tous les aperçus JPEG en base64 à chaque fois | `src/ui.rs:705`, `:526` | M | vérifié |
| WINAPP-38 | optimisation | Liste de la barre des tâches reconstruite, sur un fil neuf, à chaque `emit_computers` | `src/ui.rs:704`, `src/shell.rs:272` | S | vérifié |
| WINAPP-39 | optimisation | Verrou `APPS` à chaque message de fenêtre ; un paquet par `WM_INPUT` | `src/window.rs:2466`, `:2080` | M | vérifié |
| WINAPP-40 | optimisation | Deux copies et des allocations par image vidéo | `src/video.rs:249`, `src/decoder.rs:183` | M | vérifié |
| WINAPP-41 | optimisation | Vignette faite pour tous les écrans ; `stop_video` attend les fils un par un | `src/render.rs:692`, `src/window.rs:1073` | S | vérifié |
| WINAPP-42 | optimisation | Accueil/réglages rappellent `displays_info`/`computer_wired` à chaque événement | `ui/src/Home.svelte:241`, `Settings.svelte:126` | S | vérifié |
| WINAPP-43 | optimisation | Commandes PlayStation synchrones qui écrivent sur disque sous verrou | `src/ui/playstation.rs:113` | S | vérifié |
| WINAPP-44 | optimisation | `synced.json` réécrit à chaque accès, même en lecture | `src/ui/synced.rs:44-56` | S | vérifié |
| WINAPP-45 | optimisation | `computers.json` : `fsync` avec le verrou `store` tenu | `src/store.rs:344`, `src/ui.rs:497` | M | vérifié |
| WINAPP-46 | optimisation | Relevé des ordinateurs un par un (un PC éteint coûte ≈ 4 s) | `src/ui.rs:1207-1217` | M | plausible |
| WINAPP-47 | optimisation | Fils son et micro sans priorité MMCSS | `src/audio.rs:81`, `src/micro.rs:140` | S | plausible |
| WINAPP-48 | optimisation | La barre recalcule sa mise en page à chaque `stats`, même cachée | `ui/src/Bar.svelte:325-346`, `:588` | S | plausible |
| WINAPP-49 | factorisation | Courbes HDR écrites trois fois, avec des formules qui divergent | `src/hdr.rs`, `apps/linux/src/hdr.rs` | M | vérifié |
| WINAPP-50 | factorisation | Paquet de configuration PyroWave (`KNPW`) lu trois fois | `src/pyrowave_config.rs` | S | vérifié |
| WINAPP-51 | factorisation | Machine à états du flux d'images propre à chaque app | `src/video.rs:520-590` | M | vérifié |
| WINAPP-52 | factorisation | Plan de session recalculé en TypeScript | `ui/src/lib/api.ts:801-870` | M | vérifié |
| WINAPP-53 | factorisation | Découverte mDNS codée trois fois, règles différentes | `src/discovery.rs:51-100` | M | vérifié |
| WINAPP-54 | factorisation | Paquet « Envoyer à un appareil » PlayStation défini deux fois | `src/consoles.rs:578-640` | M | vérifié |
| WINAPP-55 | factorisation | Envois en attente des réglages synchronisés écrits deux fois (Windows/Linux) | `src/synced.rs` | L | plausible |
| WINAPP-56 | factorisation | WASAPI dupliqué entre `audio.rs` et `micro.rs` ; état du micro différent de Linux | `src/audio.rs`, `src/micro.rs`, `src/ui/micro.rs` | M | vérifié |
| WINAPP-57 | factorisation | Six chemins `%APPDATA%\KyberNow`, quatre façons d'écrire un fichier | `src/ui.rs:482` etc. | S | vérifié |
| WINAPP-58 | factorisation | Fond + feuille recopiés dans 10 composants Svelte | `ui/src/*Sheet.svelte` | M | vérifié |
| WINAPP-59 | factorisation | Composants et polices recopiés entre les 4 apps Svelte du dépôt | `ui/src/lib/Mark.svelte` etc. | M | vérifié |
| WINAPP-60 | factorisation | `Bar.svelte` monolithique (2 888 lignes) | `ui/src/Bar.svelte` | L | vérifié |
| WINAPP-61 | factorisation | Refus de mise à jour de la Station traité deux fois | `ui/src/Station.svelte:68-82` | S | vérifié |
| WINAPP-62 | factorisation | Petites règles d'écrans recodées hors du cœur | `src/displays.rs:162-209` | S | vérifié |
| WINAPP-63 | code mort | `lend_refresh`, `status_of`, `errors.video_thread`, `playing` en double | `src/ui/lend.rs:318` etc. | S | vérifié |
| WINAPP-64 | code mort | Clés de traduction de l'interface jamais utilisées | `ui/src/lib/i18n/*.json` | S | vérifié |
| WINAPP-65 | code mort | `#![allow(dead_code)]` de module qui cache `pyrowave::Decoder::config` et `Decoder::fourcc` | `src/decoder.rs:6`, `src/pyrowave.rs:18` | S | vérifié |
| WINAPP-66 | code mort | Fonctionnalités `windows` peut-être inutiles | `Cargo.toml` | S | plausible |
| WINAPP-67 | doc | PyroWave présenté comme une option alors qu'il est allumé par défaut | `scripts/build.sh:7-12`, `Cargo.toml:15-18` | S | vérifié |
| WINAPP-68 | doc | `csp: null` avec `withGlobalTauri` : pas de seconde barrière | `tauri.conf.json:9-12` | S | vérifié |

**Décompte** : 2 bugs critiques, 33 bugs, 13 optimisations, 14 factorisations, 4 code mort, 2 doc (68 constats).

---

## Bugs critiques

### WINAPP-01 — Carte graphique perdue : l'image se fige, aucune reprise
- **Catégorie** : bug critique · **Effort** : L · **Confiance** : vérifié
- **Où** : `apps/windows/src/render.rs:928-942` (`present`) ; `render.rs:692-698` ; `video.rs:705-707`.
- **Description** : un `Present` en échec (`DXGI_ERROR_DEVICE_REMOVED`/`DEVICE_RESET`) est journalisé une fois (`present_failed`), puis ignoré : ce drapeau n'est lu nulle part ailleurs, `draw` rend `Ok(())` et l'image est comptée comme montrée, donc le garde-fou « rien depuis 4 s » ne part jamais. Le `Gpu` (un par session, partagé par tous ses écrans, `host.rs:850`) n'est jamais recréé. Pire, un `CreateTexture2D` en échec dans `ensure_planes` devient `Unsupported` et fait basculer la session entière sur le décodage logiciel.
- **Scénario** : mise à jour du pilote, TDR, eGPU débranché, ou certaines sorties de veille en pleine session : tous les écrans restent figés, sans message ni reconnexion.
- **Correctif** : faire remonter l'erreur de `present` ; si `GetDeviceRemovedReason() != S_OK`, arrêter les fils de l'image, recréer le `Gpu` et demander au cœur de relancer le flux (même chemin que la relance « sans image »). Distinguer « device perdu » de « format non pris en charge » dans `ensure_planes`.

### WINAPP-02 — Installateur : `cmd /C start "" url` exécute ce qui suit un `&`
- **Catégorie** : bug critique (sécurité) · **Effort** : S · **Confiance** : vérifié
- **Où** : `apps/windows/setup/src/sys.rs:200-204`, appelé par `setup/src/bin/ui.rs:189-198` (navigation externe et nouvelle fenêtre de la page de compte du relais).
- **Description** : `Command::new(system32("cmd.exe")).args(["/C","start","",url])`. Rust ne met entre guillemets qu'un argument contenant une espace ; il ne protège ni `&`, ni `|`, ni `^` pour `cmd.exe`. Le seul contrôle est `starts_with("https://")`.
- **Scénario** : la page du relais (relais personnalisé saisi dans « Un autre relais… », ou relais compromis) contient un lien `https://x/?a&calc.exe` ; le clic lance `calc.exe`. Sans malveillance : une adresse avec `?a=1&b=2` est coupée et cmd affiche une erreur.
- **Correctif** : `ShellExecuteW(None, "open", url, …)` comme `src/shell.rs:328`, après validation par un parseur d'URL. Même défaut, hors zone, dans `server/installer/src/bin/ui.rs:49` (avec `cmd` cherché dans le PATH).

## Bugs

### WINAPP-03 — « Installer la mise à jour » coupe une partie PlayStation ou une connexion en cours
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/ui.rs:991-993` (`install_update`).
- **Description** : seul `state.sessions` est vérifié avant `app.exit(0)`. `exit_when_idle` (`ui.rs:3129`) vérifie aussi `launching` et `state.ps.playing()`.
- **Scénario** : une partie PS5 est en cours ; on clique « Installer » à l'accueil : l'app quitte et la partie est coupée sans prévenir la console.
- **Correctif** : reprendre la condition d'`exit_when_idle` (une fonction `idle(&state)` commune).

### WINAPP-04 — `app.json` écrit sans atomicité, relu en silence s'il est abîmé
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/ui.rs:951-956` (`save_prefs` : `let _ = std::fs::write(...)`) et `ui.rs:485` (`load_prefs` : `.ok()…unwrap_or_default()`).
- **Description** : écriture directe (fichier tronqué possible à un arrêt brutal), erreur ignorée ; à la lecture, un fichier illisible donne les réglages par défaut sans être mis de côté (contrairement à `computers.json`).
- **Scénario** : coupure de courant pendant l'écriture → au lancement suivant `open_at_login` redevient `None`, donc `open_at_login_default` (`ui.rs:4345`) **rallume l'ouverture à la connexion** que la personne avait coupée ; langue, canal bêta, accueil et avis des Stations reviennent aussi.
- **Correctif** : `store::write_atomic` ; mettre de côté un fichier illisible et le journaliser.

### WINAPP-05 — `save_prefs` écrit une copie hors du verrou
- **Effort** : S · **Confiance** : plausible
- **Où** : les 15 appels à `save_prefs` (par ex. `src/ui.rs:965-970`, `src/ui/welcome.rs:30-38`, `src/ui/micro.rs:231-248`, `src/ui/share.rs:274-280`).
- **Description** : `let prefs = { let mut p = state.prefs.lock().unwrap(); …; p.clone() }; save_prefs(&prefs);` : deux commandes `async` sur deux fils peuvent écrire dans le désordre.
- **Scénario** : `set_language` et `welcome_done` presque simultanés : le fichier garde l'ancienne langue, ou l'accueil revient.
- **Correctif** : une fonction `update_prefs(|p| …)` qui modifie et écrit sous le verrou (fichier minuscule).

### WINAPP-06 — Ordinateur enregistré même si son mot de passe n'a pas pu être gardé
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/ui.rs:1533-1562` (`add` : `s.upsert(…)` puis `if !store::set_password(…) { return Err(…) }`) ; même ordre dans `src/ui/lend.rs:288-291`.
- **Description** : `upsert` écrit `computers.json` avant que le mot de passe soit posé ; en cas d'échec, l'erreur s'affiche mais l'ordinateur reste dans la liste (et pour un hôte déjà connu, `user` est remplacé alors que l'ancien mot de passe reste).
- **Scénario** : `CredWriteW` refusé (stratégie, profil itinérant) : l'ordinateur apparaît au relevé suivant et chaque connexion échoue.
- **Correctif** : poser le mot de passe d'abord (identifiant calculé avant), ou annuler l'ajout en cas d'échec.

### WINAPP-07 — Réglages synchronisés : deux envois peuvent partir dans le désordre
- **Effort** : M · **Confiance** : plausible
- **Où** : `src/ui/synced.rs:36` (`static BUSY`, commenté « dans l'ordre ») et `:230-231`.
- **Description** : chaque `save_settings` lance un fil qui prend `BUSY` ; ni l'ordre de démarrage des fils ni un `Mutex` standard ne garantissent l'ordre d'arrivée.
- **Scénario** : « Bureau » puis « Jeu » en moins d'une seconde ; si l'envoi « Jeu » passe d'abord, le relais finit sur « Bureau », les autres appareils aussi, et ce PC reste sur « Jeu » (l'écart ne se résorbe pas, `sent` ayant noté le `updated_at`).
- **Correctif** : un seul fil d'envoi par compte alimenté par un canal, qui fusionne les changements en attente avant chaque envoi. Corriger le commentaire.

### WINAPP-08 — Panique sous le verrou `sessions` → toute l'app paniquera ensuite
- **Effort** : S · **Confiance** : plausible (chemin vérifié, déclencheur rare)
- **Où** : `src/ui.rs:3440` (`let mut running = state.sessions.lock().unwrap(); let host = SessionHost::start_as(…)`), même schéma dans `src/ui/lend.rs:630` et `src/ui/share.rs:619` ; `src/host.rs:301` (`.spawn(…).expect("fil de la fenêtre de session")`).
- **Description** : pas de `panic = "abort"` ; une panique ici empoisonne `sessions`, et les ≈ 180 `lock().unwrap()` de `ui.rs` (relevé, accueil, menu de l'icône, quitter) paniquent à leur tour.
- **Scénario** : création du fil refusée (mémoire, limite de fils) : l'app ne répond plus du tout, sans boîte d'erreur.
- **Correctif** : `start_as` rend un `Result`, ou le fil est créé avant de prendre le verrou. Plus largement, un utilitaire `lock()` qui récupère un verrou empoisonné (`unwrap_or_else(PoisonError::into_inner)`) pour les états purement de données.

### WINAPP-09 — Instantané de l'accueil faux en multi-écrans
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/render.rs:973-1004` (`snapshot`), appelé après `present` en `:692-697` et `:723-728`.
- **Description** : le verrou du contexte D3D (partagé par tous les écrans de la session) est relâché entre `encode`, `present` et `snapshot` ; `snapshot` ne relie que la cible et le viewport et suppose que shaders, SRV, sampler et `cbuf` sont encore ceux de cet écran. Un autre écran a pu relier les siens, ou faire `ClearState()` (`render.rs:526`, `:792`).
- **Scénario** : session à deux écrans : la vignette de l'accueil montre l'écran secondaire, ou est noire.
- **Correctif** : relier tout le pipeline (VS, PS, SRV, sampler, cbuf, topologie, blend) dans `snapshot` avant `Draw`.

### WINAPP-10 — `quit` détruit les fenêtres avant d'arrêter les fils de l'image ; HCURSOR perdus
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/window.rs:1786-1804` (`quit`) face à `remove_window` (`:2107-2135`, qui fait `video.stop()` puis `DestroyCursor`).
- **Description** : `quit` fait `DestroyWindow` sans arrêter le fil de l'écran (qui présente encore sur une fenêtre détruite : faux « Present : … (carte perdue) » au journal) et ne libère jamais `cursor.current` (HCURSOR de `CreateIconIndirect`).
- **Scénario** : un HCURSOR et sa DIB perdus par écran et par session ; avec beaucoup de reconnexions, les objets USER s'accumulent.
- **Correctif** : une `close_window` commune à `quit` et `remove_window`.

### WINAPP-11 — PyroWave : décodeur recréé à chaque image tant qu'il échoue
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/video.rs:609-611` (`if pyro.is_none() { pyro = open_pyro(…) }`), `open_pyro` en `:443-462`.
- **Description** : aucun délai ni mémoire de l'échec : chaque image refait fence, handle partagé et `sync_create`, puis `log::error!`.
- **Scénario** : `ID3D11Device5` absent ou fence partagée refusée : 60 à 240 lignes d'erreur par seconde, processeur gaspillé, écran noir sans fin (aucun repli annoncé).
- **Correctif** : retenir l'échec jusqu'au prochain changement de format, journaliser une fois, et signaler au cœur pour qu'il repasse en HEVC.

### WINAPP-12 — Crochet clavier bas niveau trop lourd
- **Effort** : M · **Confiance** : plausible
- **Où** : `src/window.rs:350-447` (`ll_keyboard` → `grab_key` → `apps()` (copie sous `APPS`), `foreground_app()`, `grabbed.lock()`, `send_key` → `InputPipeline::key` qui prend `switch` et envoie).
- **Description** : un crochet `WH_KEYBOARD_LL` bloque tout le clavier de Windows tant qu'il n'a pas répondu ; au-delà de `LowLevelHooksTimeout`, Windows le retire sans prévenir.
- **Scénario** : pendant un passage d'un ordinateur à l'autre (`set_active` tient `switch`), la frappe se fige sur tout le PC ; après un dépassement, ⊞ et Alt+Tab reviennent à ce PC pour le reste de la session, sans trace au journal.
- **Correctif** : décider dans le rappel avec des atomiques seulement, puis `PostMessageW` vers la fenêtre de session qui fait l'envoi.

### WINAPP-13 — Son et micro ne suivent pas le périphérique par défaut
- **Effort** : M · **Confiance** : vérifié (code) ; comportement WASAPI connu
- **Où** : `src/audio.rs:117-186`, `src/micro.rs:205-319` ; aucun `IMMNotificationClient` dans `apps/windows/src`. Le commentaire d'`audio.rs:6-7` promet pourtant une réouverture « sortie du système changée ».
- **Description** : la boucle ne rouvre que sur arrêt, changement de voies ou erreur ; un flux ouvert sur un `IMMDevice` continue tant que ce périphérique existe.
- **Scénario** : en session, on choisit « Casque » dans le menu son de Windows : le son reste sur les haut-parleurs. Idem pour le micro de communication.
- **Correctif** : `RegisterEndpointNotificationCallback` (`OnDefaultDeviceChanged` lève un drapeau lu par la boucle), dans le module WASAPI commun (WINAPP-56).

### WINAPP-14 — Ctrl/Alt restent tenus après Ctrl+Alt+Suppr ou Win+L
- **Effort** : S · **Confiance** : plausible
- **Où** : `src/window.rs:2682-2690` : `release_all` seulement sur `WM_KILLFOCUS`.
- **Description** : le passage au bureau sécurisé n'envoie pas forcément `WM_KILLFOCUS` ; les relâchements ont lieu sur le bureau sécurisé et ne reviennent pas.
- **Scénario** : Ctrl+Alt+Suppr puis Annuler : chaque touche arrive sur l'ordinateur avec Ctrl+Alt.
- **Correctif** : `WTSRegisterSessionNotification` → `release_all()` sur verrouillage ; au retour d'activation, comparer les modificateurs tenus à `GetAsyncKeyState`.

### WINAPP-15 — Souris absolue immobile en mode jeu
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/window.rs:2099` (`if (m.usFlags.0 & MOUSE_MOVE_ABSOLUTE.0) == 0 && …`).
- **Description** : en mode relatif, les déplacements Raw Input absolus sont ignorés et `mouse_position` ne fait rien.
- **Scénario** : Windows ARM64 dans Parallels, session Bureau à distance, tablette : dès que le jeu cache le pointeur, la souris ne bouge plus.
- **Correctif** : calculer l'écart entre deux positions absolues (0-65535, `MOUSE_VIRTUAL_DESKTOP`), ou rester en mode bureau pour ces périphériques.

### WINAPP-16 — Impr. écran n'arrive jamais à l'ordinateur
- **Effort** : S · **Confiance** : plausible
- **Où** : `src/window.rs:2490-2566` (`if !pressed && !input.held()…is_held(scancode) { return; }`).
- **Description** : Windows n'envoie que `WM_KEYUP` pour `VK_SNAPSHOT` ; le relâchement est jeté faute d'appui noté, et le crochet ne prend pas cette touche (Windows 11 ouvre l'Outil Capture sur ce PC).
- **Correctif** : prendre `VK_SNAPSHOT` dans le crochet en mode immersif, ou envoyer appui+relâchement sur ce `WM_KEYUP`.

### WINAPP-17 — `WM_DPICHANGED` non traité par les fenêtres de session
- **Effort** : S · **Confiance** : plausible
- **Où** : `src/window.rs:2463-2729` (`screen_proc`) ; processus en per-monitor v2 (`system.rs:282`) ; seul `tray.rs:640` traite le message.
- **Scénario** : fenêtre de session glissée d'un écran à 100 % vers un écran à 150 % : la zone cliente n'a plus la définition exacte de l'ordinateur, l'image devient floue.
- **Correctif** : traiter `WM_DPICHANGED` avec `AdjustWindowRectExForDpi` pour garder la taille cliente.

### WINAPP-18 — `session_camera_state` énumère les webcams sur le fil principal
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/ui/camera.rs:214-216` (`#[tauri::command]` sans `async`) → `snapshot` (`:62-64`) → `camera::cameras()` (`MFEnumDeviceSources`) ; appelée à chaque affichage de la barre (`ui/src/Bar.svelte:516`).
- **Scénario** : pointeur en haut de l'écran pendant une session où la caméra est proposée : barre et accueil gèlent le temps de l'énumération (centaines de ms avec des caméras USB ou virtuelles).
- **Correctif** : `#[tauri::command(async)]` et liste en cache, relue à l'ouverture du choix de caméra.

### WINAPP-19 — Caméra : fin de session bloquée, verrou `SESSIONS` tenu
- **Effort** : M · **Confiance** : plausible
- **Où** : `src/ui/camera.rs:100-107` (`ended` : `SESSIONS.lock()` puis `drop(c.sender)`), `src/camera.rs:137-144` (`Drop` fait `join`), `:343-348` (`ReadSample` synchrone).
- **Scénario** : webcam qui ne rend plus d'image (obturateur, pilote) au moment de fermer : la fin de session et toute l'interface caméra restent figées.
- **Correctif** : sortir le `sender` sous verrou, le lâcher hors verrou ; lecteur asynchrone ou fil détaché après le drapeau `stop`.

### WINAPP-20 — Caméra : encodeur matériel jamais arrêté
- **Effort** : S · **Confiance** : plausible
- **Où** : `src/camera.rs:194-220` (`ActivateObject::<IMFTransform>`), fin de `run` `:418-422` (seulement `END_OF_STREAM`/`FLUSH`).
- **Scénario** : caméra allumée/éteinte plusieurs fois sur une GeForce grand public (sessions NVENC limitées) : l'encodeur matériel finit refusé.
- **Correctif** : garder l'`IMFActivate` et appeler `ShutdownObject()` en fin de `run` et pour chaque candidat rejeté.

### WINAPP-21 — Caméra : tout échec annoncé « utilisée par une autre app »
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/camera.rs:179-188` (tous les `map_err(|_| Problem::InUse)`).
- **Scénario** : webcam 720p avec l'option 1080p : `SetCurrentMediaType` refuse, l'interface dit « La caméra est utilisée par une autre app ».
- **Correctif** : distinguer accès refusé (confidentialité), caméra occupée et type refusé ; réessayer en définition inférieure.

### WINAPP-22 — Fuite d'un handle d'événement à chaque échec WASAPI
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/audio.rs:146-156`, `src/micro.rs:261-270` : chaque `?` après `CreateEventW` sort sans `CloseHandle`, et la boucle réessaie toutes les 500 ms.
- **Scénario** : casque Bluetooth qui se reconnecte en boucle : deux handles perdus par seconde.
- **Correctif** : garde RAII `struct Event(HANDLE)` dans le module commun (WINAPP-56).

### WINAPP-23 — Presse-papiers : lecture du texte sans borne
- **Effort** : S · **Confiance** : plausible
- **Où** : `src/pasteboard.rs:219-222` (`while *p.add(len) != 0 { len += 1; }`).
- **Scénario** : une autre app pose un `CF_UNICODETEXT` sans nul final : lecture hors du bloc, plantage.
- **Correctif** : borner par `GlobalSize(global) / 2`.

### WINAPP-24 — Filtre d'exceptions qui journalise pendant un plantage
- **Effort** : S · **Confiance** : plausible
- **Où** : `src/system.rs:244-257` (`on_crash` → `log::error!` → mutex du fichier, `format!`).
- **Scénario** : violation d'accès dans FFmpeg pendant une écriture du journal sur le même fil : mutex non réentrant, le processus reste figé au lieu de se terminer.
- **Correctif** : `try_lock`, tampon fixe, `WriteFile` direct ; ignorer `EXCEPTION_STACK_OVERFLOW`.

### WINAPP-25 — Micro : deux captures peuvent pousser en même temps
- **Effort** : S · **Confiance** : plausible
- **Où** : `src/micro.rs:173-177` (`Drop` lève un drapeau sans attendre), `src/host.rs:570-595`.
- **Scénario** : Ctrl+Alt+Maj+V deux fois de suite : l'ancien fil (jusqu'à 200 ms) et le nouveau appellent le même `sink`, échantillons mêlés dans Opus.
- **Correctif** : garder le `JoinHandle` et l'attendre dans `Drop`, ou un jeton de génération vérifié par le `sink`.

### WINAPP-26 — Réglages d'une console : le clic droit ouvre le menu au bout de 300 ms
- **Effort** : S · **Confiance** : vérifié
- **Où** : `ui/src/ConsoleSettings.svelte:101-122`.
- **Description** : `listenFor` commence par `stopCatch()`, qui programme le retrait de `contextmenu`/`auxclick` 300 ms plus tard ; ce retrait enlève les écouteurs que `listenFor` vient de remettre.
- **Scénario** : on clique sur une touche à remplacer, puis on fait un clic droit après 0,3 s : le bouton est bien noté, mais le menu de WebView2 s'ouvre par-dessus.
- **Correctif** : garder la référence du `setTimeout` et l'annuler dans `listenFor`.

### WINAPP-27 — Menu contextuel de WebView2 disponible partout
- **Effort** : S · **Confiance** : plausible
- **Où** : `ui/src/main.ts`, `bar.ts`, `fond.ts`, `connexion.ts` : aucun blocage de `contextmenu`, rien côté Rust.
- **Scénario** : clic droit → « Actualiser » sur la barre : la page se recharge et perd son état (feuille ouverte, `seen`).
- **Correctif** : bloquer `contextmenu` hors champs de saisie, ou désactiver les menus par défaut de WebView2.

### WINAPP-28 — Changements optimistes sans retour arrière, rejets non attrapés
- **Effort** : S-M · **Confiance** : vérifié (absence de `catch`) ; effet plausible
- **Où** : `ui/src/Bar.svelte:726` (`pickMouse` : `mouseSetting = m` puis `await invoke('session_mouse')`), `:814` (`toggleKeepSound`), `:716` (`pickProfile`), `:687` ; `AppSheet.svelte:105`, `:119` ; `Settings.svelte:97` (`.catch(() => {})` sur `save_settings`) ; `App.svelte:522` ; `Station.svelte:107` ; démarrage de `Fond.svelte:18-20`, `Connexion.svelte:69-90`.
- **Scénario** : la session refuse le changement de souris (`errors.session_not_ready`) : la barre coche « Jeu », la session reste en « Bureau », aucune erreur visible.
- **Correctif** : un utilitaire `apply(optimistic, call, rollback)` qui remet l'ancienne valeur et affiche l'erreur ; un `invoke` qui journalise les rejets.

### WINAPP-29 — Démarrage de l'accueil : sept appels en chaîne sans `catch`
- **Effort** : S · **Confiance** : vérifié (structure) ; risque faible aujourd'hui
- **Où** : `ui/src/App.svelte:273-301` (`snapshot` → `lend_friends` → `ps_state` → `account_list` → `welcome_state` → `app_info` → `take_action`).
- **Scénario** : une commande rejette (renommage, panique) : `take_action` (lien `kybernow://` reçu au lancement) et l'avis des Stations ne sont jamais traités.
- **Correctif** : `Promise.allSettled` pour les lectures indépendantes (aussi plus rapide), `take_action` dans un `finally`.

### WINAPP-30 — Feuille d'ajout fermée pendant un appel en cours
- **Effort** : S · **Confiance** : plausible
- **Où** : `ui/src/AddSheet.svelte:246-279` (`ps_handoff_receive { on: true }`), `:286` (`pair_start`) face à `:102` (`pair_cancel`).
- **Description** : `onDestroy(stopReceive)` ne fait rien si `receiving` n'est pas encore vrai ; la réponse arrive ensuite et l'attente côté Rust (fil `ps-reception`, port ouvert) continue sans interface.
- **Correctif** : drapeau `destroyed` ; après l'`await`, annuler aussitôt si la feuille est fermée.

### WINAPP-31 — Échap et focus incohérents
- **Effort** : S-M · **Confiance** : vérifié
- **Où** : Échap ne ferme ni `AppSheet`, ni `PairSheet`, ni `StationNotice`, ni la confirmation de `Station.svelte` (alors que `AddSheet`, `JoinSheet`, `LendSheet`, `InviteSheet`, `AutoTest`, `AwayPrompt` le font) ; aucun `inert` derrière les feuilles ; les 7 `role="menu"` de la barre (`Bar.svelte:1344`, `:1375`, `:1394`…) n'ont ni Échap ni flèches ; le menu du compte (`Home.svelte:330`) se ferme seulement au pointeur.
- **Correctif** : passe par le composant `Sheet` partagé (WINAPP-58) ; gestion clavier commune des menus de la barre.

### WINAPP-32 — Installateur : fichiers retirés de la nouvelle version laissés en place
- **Effort** : S · **Confiance** : vérifié
- **Où** : `setup/src/install.rs:154-164`, `:181-216` (`swap_files` : remplace et ajoute seulement quand le dossier est verrouillé).
- **Scénario** : une DLL retirée d'une version reste à côté de `kybernow.exe` et peut encore être chargée.
- **Correctif** : effacer (ou programmer l'effacement) des fichiers absents du nouveau paquet, sauf `uninstall.exe`.

### WINAPP-33 — Installateur : WebView2 par PowerShell, sans vérifier la signature
- **Effort** : M · **Confiance** : plausible
- **Où** : `setup/src/webview2.rs:33-54` (`Invoke-WebRequest`, puis lancement si `exe.exists()`).
- **Scénario** : PC d'entreprise avec PowerShell restreint ou proxy : « téléchargement de WebView2 impossible » ; un fichier partiel peut être lancé.
- **Correctif** : télécharger avec le client HTTPS déjà présent (`account::request`, rustls), vérifier la signature Microsoft (`WinVerifyTrust`) avant de lancer.

### WINAPP-34 — Installateur : ordre de recherche des DLL non protégé
- **Effort** : S · **Confiance** : plausible
- **Où** : `setup/src/bin/setup.rs:38-44`, `setup/src/bin/ui.rs` ; aucun `SetDefaultDllDirectories` dans `setup/`.
- **Scénario** : installateur lancé depuis Téléchargements, où traîne une DLL au nom d'une DLL système chargée à la demande.
- **Correctif** : `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)` en première ligne des deux `main`.

### WINAPP-35 — `uninstall.exe` garde une entrée de signature qui pointe hors du fichier
- **Effort** : S · **Confiance** : plausible (à revoir quand la signature SignPath sera en place)
- **Où** : `setup/src/payload.rs:181-187` (`copy_stub` coupe à `exe_len`).
- **Description** : si l'installateur est signé, l'en-tête PE copié garde l'entrée « Security directory » au-delà de la fin de la copie : signature invalide plutôt qu'absente, plus suspecte pour SmartScreen et les antivirus.
- **Correctif** : remettre à zéro le répertoire 4 dans la copie, ou signer `uninstall.exe` à part.

## Optimisations

### WINAPP-36 — `mock.ts` livré et exécuté dans l'app
- **Effort** : S · **Confiance** : vérifié (chaînes présentes dans `ui/dist`)
- **Où** : `ui/src/lib/api.ts:11-16` (import statique de `./mock`) ; `ui/src/lib/mock.ts` (`Object.assign`, `URLSearchParams`, `t('mock.…')` au chargement du module).
- **Description** : seul l'appel `mockTauri()` est éliminé ; le reste du module (≈ 36 Ko sur 147 Ko du paquet `api-*.js`) est livré et exécuté à l'ouverture des quatre fenêtres, plus la rubrique `mock` des catalogues (≈ 3,5 Ko par langue). On y trouve de fausses adresses e-mail, de faux noms de machines et le prénom de l'auteur (« Matthieu n'a pas répondu. »). Le commentaire « jamais incluse dans l'app » est faux.
- **Correctif** : `if (import.meta.env.DEV && !window.__TAURI__) tauri = (await import('./mock')).mockTauri()` ; sortir la rubrique `mock` des catalogues vers un fichier chargé par le mock.

### WINAPP-37 — L'événement `computers` renvoie tous les aperçus en base64
- **Effort** : M · **Confiance** : vérifié
- **Où** : `src/ui.rs:705`, `:718` (`views()`, `:526-536`) ; aperçus de 1280 px (`:1141-1143`).
- **Description** : `ComputerView.preview` contient l'URL `data:` complète (100 à 300 Ko par ordinateur), recopiée, sérialisée et analysée à chaque `computers`/`sessions`, même si seul `online` a changé ; `emit_computers` est appelé depuis une vingtaine d'endroits (suivi de réveil : toutes les 2 s).
- **Correctif** : un numéro de version d'aperçu dans `computers` et l'image par une commande `preview(id)` appelée quand la version change.

### WINAPP-38 — Liste de la barre des tâches reconstruite à chaque `emit_computers`
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/ui.rs:704` → `src/shell.rs:272-280` (`std::thread::spawn` + COM + `build_jump_list` à chaque appel).
- **Description** : la liste ne dépend que des (identifiant, nom), mais est refaite à chaque aperçu et chaque tour de suivi ; plusieurs fils peuvent faire `BeginList`/`CommitList` en même temps.
- **Correctif** : mémoriser la dernière liste et ne reconstruire que si elle change, sur un fil unique.

### WINAPP-39 — Chemin chaud de la souris : verrous et un paquet par message
- **Effort** : M · **Confiance** : vérifié (coût réel non mesuré)
- **Où** : `src/window.rs:2466` (`app_of` à chaque message : verrou `APPS`, verrou `windows` de chaque session, clones d'`Arc`), `:618-620` ; `:2080-2105` (un `mouse_move` → envoi par `WM_INPUT`).
- **Description** : une souris à 4-8 kHz donne autant de verrous et d'envois par seconde, et ces verrous sont disputés avec le crochet clavier (WINAPP-12).
- **Correctif** : `Arc<ScreenWin>` dans `GWLP_USERDATA` (libéré en `WM_NCDESTROY`) ; cumuler les écarts et envoyer une fois par tour de la file de messages.

### WINAPP-40 — Deux copies et des allocations par image
- **Effort** : M · **Confiance** : vérifié
- **Où** : `src/video.rs:249` (`data.to_vec()`), `src/decoder.rs:183-187` (`av_new_packet` + copie), `src/render.rs:914` (`Vec` de SRV à chaque tracé), `src/window.rs:1967-1975` (deux `Vec` par message de stylet).
- **Correctif** : petit pool de tampons, paquet FFmpeg réutilisé (`av_grow_packet`), tableau fixe pour les SRV.

### WINAPP-41 — Vignette pour tous les écrans ; arrêt des fils un par un
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/render.rs:692-697`, `src/video.rs:746-751` (relecture + JPEG), `src/window.rs:1080-1086` (seul `is_primary` est gardé) ; `stop_video` `:1073-1078` attend chaque fil l'un après l'autre (jusqu'à 2 s chacun).
- **Correctif** : vignette seulement si `shared.is_primary()` ; `stop()` sur tous puis attente de tous.

### WINAPP-42 — Effets Svelte relancés à chaque événement `computers`
- **Effort** : S · **Confiance** : vérifié (sémantique Svelte 5)
- **Où** : `ui/src/Home.svelte:241-262` (`displays_info`), `ui/src/Settings.svelte:126-131` (`computer_wired`), `:163`.
- **Description** : `selected`/`computer` sont des objets neufs à chaque `computers = c` (`App.svelte:184`) ; côté Rust `displays_info` relève les écrans et `system::wired_to` écrit une ligne de journal à chaque appel (`system.rs:147`).
- **Correctif** : ne dépendre que de clés primitives (identifiant, signature des réglages) avec un garde « inchangé ».

### WINAPP-43 — Commandes PlayStation synchrones qui écrivent sous verrou
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/ui/playstation.rs:113-115` (`save` : écriture de `playstation.json` sous le verrou `consoles`), utilisée par `ps_account_manual` (`CredWriteW`), `ps_set_hdr`, `ps_choose_account`, `ps_save_settings`, `ps_save_layout`, `ps_default_layout`, `ps_remove`, toutes sans `async`.
- **Correctif** : passer en `async`, sérialiser sous verrou et écrire après.

### WINAPP-44 — `synced.json` réécrit à chaque accès
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/ui/synced.rs:44-56` (`with_file` sérialise et écrit toujours, sans `sync_all`).
- **Correctif** : version lecture seule, écriture seulement si le contenu change, via `write_atomic` (WINAPP-57).

### WINAPP-45 — `fsync` de `computers.json` avec le verrou `store` tenu
- **Effort** : M · **Confiance** : vérifié
- **Où** : `src/store.rs:344-362` (`write_atomic` + `sync_all`) appelé dans `with_store` (`src/ui.rs:497-500`).
- **Description** : `views()`, `tray_state`, `poll` et toutes les lectures attendent la fin de l'écriture disque.
- **Correctif** : sérialiser sous verrou, écrire hors verrou avec un numéro de version.

### WINAPP-46 — Relevé des ordinateurs un par un
- **Effort** : M · **Confiance** : plausible
- **Où** : `src/ui.rs:1207-1217` (`for c in computers { changed |= refresh(&app, &c); }`).
- **Scénario** : trois ordinateurs éteints sans relais : ≈ 4 s chacun, le tour de 8 s en dure ≈ 20 ; un ordinateur qui revient s'affiche en retard.
- **Correctif** : interroger en parallèle, un seul `emit_computers` à la fin.

### WINAPP-47 — Fils son et micro sans priorité MMCSS
- **Effort** : S · **Confiance** : plausible
- **Où** : `src/audio.rs:81-104`, `src/micro.rs:140-165` ; aucun `AvSetMmThreadCharacteristics` dans `apps/windows/src`.
- **Correctif** : `AvSetMmThreadCharacteristicsW("Pro Audio")` au début des deux fils.

### WINAPP-48 — La barre recalcule sa mise en page même cachée
- **Effort** : S · **Confiance** : plausible
- **Où** : `ui/src/Bar.svelte:325-346` (chaque `stats`, une fois par seconde) → `reportWidth()` (`:588-601`, `getBoundingClientRect` après `tick()`) ; `track` relit le rectangle à chaque mouvement.
- **Correctif** : si la barre est cachée, ne noter que `seen` ; `reportWidth` seulement quand le contenu change ; rectangle en cache (ResizeObserver).

## Factorisation

### WINAPP-49 — Courbes HDR écrites trois fois, formules divergentes
- **Effort** : M · **Confiance** : vérifié
- **Où** : `apps/windows/src/hdr.rs:48-200` + shader `render.rs:40-125` ; `apps/linux/src/hdr.rs:18-80` + shader GL ; `server/admin/src/display/icc.rs:34-50`.
- **Description** : courbe vers le SDR en Reinhard étendu sur Windows, rationnelle sur Linux (pour r = 2 : ≈ 0,964 contre ≈ 0,973) ; BT.2390 sans le relèvement du noir `dst_lo·(1−E)⁴` sur Windows. Le même flux HDR ne rend pas pareil d'une app à l'autre.
- **Correctif** : `kybernow_core::hdr` (PQ, EETF, courbe SDR, `yuv_matrix`, `content_peak`) avec tests ; shaders recopiés mais vérifiés par un test croisé.

### WINAPP-50 — Paquet `KNPW` lu trois fois
- **Effort** : S · **Confiance** : vérifié
- **Où** : `apps/windows/src/pyrowave_config.rs` (longueur ≥ 16), `apps/linux/src/device/video.rs:72-120` (exactement 16), `apps/android/jni/src/pyrowave.rs:13-40` (sans contrôle de parité en 4:2:0).
- **Correctif** : `Config` dans `kybernow_core::pyrowave`.

### WINAPP-51 — Machine à états du flux d'images propre à chaque app
- **Effort** : M · **Confiance** : vérifié (Windows), plausible (effet sur Linux)
- **Où** : `apps/windows/src/video.rs:520-590` face à `apps/linux/src/device/video.rs:328-375`.
- **Description** : Linux n'a pas le repli « format jamais annoncé » ajouté sur Windows contre l'écran noir ; Windows remplace le paquet de configuration, Linux concatène.
- **Correctif** : une machine à états pure dans le cœur qui rend des actions (ouvrir tel décodeur, configurer, sauter, demander une image clé).

### WINAPP-52 — Plan de session recalculé en TypeScript
- **Effort** : M · **Confiance** : vérifié (duplication) ; divergence plausible
- **Où** : `ui/src/lib/api.ts:801-870` (`isCustom`, `profileBase`, `fit`, `sessionPlan`, `PYROWAVE_MBPS*`) face à `src/plan.rs:40-120`.
- **Description** : le plafond de l'encodeur (`encoder_max`) et la taille de fenêtre n'existent pas côté TS ; la règle `max_4k && custom.is_none()` vit dans `ui.rs:2851`. L'accueil et « Débit réel » peuvent annoncer autre chose que la session.
- **Correctif** : `displays_info` (déjà appelé) rend le mode et le débit calculés par Rust ; supprimer le calcul TS.

### WINAPP-53 — Découverte mDNS codée trois fois
- **Effort** : M · **Confiance** : vérifié
- **Où** : `apps/windows/src/discovery.rs:51-100`, `apps/linux/src/device/jobs.rs:233-249`, `apps/webos/src/device/jobs.rs:211`.
- **Description** : Windows écarte `kyber=0` et l'IPv6 locale au lien ; Linux prend la première IPv4 et préfixe `sha256:` une empreinte qui ne l'a pas, Windows non.
- **Correctif** : `kybernow_core::discovery` (filtrage et normalisation de `Found`).

### WINAPP-54 — Paquet PlayStation « Envoyer à un appareil » défini deux fois
- **Effort** : M · **Confiance** : vérifié
- **Où** : `apps/windows/src/consoles.rs:578-640` (structure typée, `valid_psn_id`) ; `apps/linux/src/device/ps.rs:1317-1400` (JSON à la main, sans validation).
- **Correctif** : `Bundle` et la fusion dans `kybernow_core::playstation::handoff`, qui porte déjà le transport.

### WINAPP-55 — Envois en attente des réglages synchronisés écrits deux fois
- **Effort** : L · **Confiance** : plausible
- **Où** : `apps/windows/src/synced.rs` (`AccountSync`) et `apps/linux/src/synced.rs` (`SyncKeys`).
- **Correctif** : la mémoire par compte et par Station dans `kybernow_core::account::settings` ; résout aussi WINAPP-07 à un seul endroit.

### WINAPP-56 — WASAPI dupliqué ; état du micro différent de Linux
- **Effort** : M · **Confiance** : vérifié
- **Où** : `src/audio.rs:117-186` et `src/micro.rs:205-319` (même format, même boucle, même réessai) ; `src/ui/micro.rs:19-35` (`MicSlot`) face à `apps/linux/src/micro.rs:48-99` (`MicUi`, textes en dur côté Linux).
- **Correctif** : un module `wasapi.rs` (ouverture, garde d'événement, boucle, réessai, suivi du périphérique par défaut, MMCSS : règle WINAPP-13, -22, -47) ; `MicUi` dans `kybernow_core::micro` avec les textes du catalogue du cœur (règle « mêmes textes » de `docs/link-ui.md`).

### WINAPP-57 — Six chemins `%APPDATA%\KyberNow`, quatre façons d'écrire
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/ui.rs:482`, `src/store.rs:248`, `src/ui/lend.rs:113`, `src/ui/synced.rs:41`, `src/ui/playstation.rs:86`, `src/shell.rs:128` ; écritures : `store::write_atomic` (fsync), `consoles.rs:261` (repli non atomique), `lending::save_friends`, `with_file` (sans fsync), `save_prefs` (non atomique). Aussi `src/ui/lend.rs:310` réécrit `format!("lend-{id}")` au lieu de `Friend::credential()` (`lending.rs:102`).
- **Correctif** : `app_dir()` et une seule `write_atomic` (règle WINAPP-04 et -44).

### WINAPP-58 — Fond et feuille recopiés dans 10 composants
- **Effort** : M · **Confiance** : vérifié
- **Où** : `.veil`/`.sheet` et `@keyframes` dans `AddSheet:586`, `JoinSheet:242`, `LendSheet:204`, `InviteSheet:113`, `PairSheet:234`, `AppSheet:384`, `AutoTest:157`, `AwayPrompt:35`, `StationNotice:143`, `Station:336`.
- **Correctif** : `lib/Sheet.svelte` (fond, clic dehors, Échap, `role="dialog"`, `inert`, focus initial, animation) ; keyframes dans `app.css`.

### WINAPP-59 — Composants et polices recopiés entre les apps Svelte
- **Effort** : M · **Confiance** : vérifié (diff, md5)
- **Où** : `ui/src/lib/Mark.svelte` = `setup/ui/src/lib/Mark.svelte` ; `ui/src/lib/Switch.svelte` = `server/web/src/lib/Switch.svelte` ≈ `setup/ui/src/lib/Toggle.svelte` ≈ `server/installer/ui/src/lib/Toggle.svelte` ; `Segmented` à 11 lignes près de la version web ; polices Archivo et Martian Mono en 4 copies identiques.
- **Correctif** : un dossier partagé (`shared/ui/`) branché par alias Vite, conforme à « mêmes composants dans toutes les apps ».

### WINAPP-60 — `Bar.svelte` monolithique
- **Effort** : L · **Confiance** : vérifié
- **Où** : `ui/src/Bar.svelte` (2 888 lignes : script jusqu'à `:1229`, panneaux partage `:1768`, prêt `:1858`, stats, console ; style depuis `:1966`).
- **Correctif** : extraire `SharePanel`, `LendPanel`, `PsBar` (le client web en a déjà l'équivalent).

### WINAPP-61 — Refus de mise à jour de la Station traité deux fois
- **Effort** : S · **Confiance** : vérifié
- **Où** : `ui/src/Station.svelte:68-82` (`refusal`) recopie `stationRefusal`/`stationRefusalText` de `ui/src/lib/api.ts:251-271`.
- **Correctif** : appeler les fonctions d'`api.ts`.

### WINAPP-62 — Petites règles d'écrans recodées hors du cœur
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/displays.rs:206-209` (`primary`) refait `kybernow_core::screens::primary_index` (`core/src/screens.rs:404`) ; `displays.rs:162-170` (`session_hdr`) ne travaille que sur des types du cœur.
- **Correctif** : les déplacer dans `kybernow_core::screens`.

## Code mort

### WINAPP-63 — Commande, fonctions et clé inutilisées côté Rust
- **Effort** : S · **Confiance** : vérifié (grep dans tout le dépôt hors `third_party`)
- `lend_refresh` (`src/ui/lend.rs:318`, enregistrée `src/ui.rs:4463`) : appelée seulement par `ui/src/lib/mock.ts:846`.
- `playstation::status_of` (`src/ui/playstation.rs:1216-1219`, sous `#[allow(dead_code)]`) : aucun appel, documentation fausse.
- Clé `errors.video_thread` (`src/i18n/fr.json:25`, `en.json:25`) : aucune utilisation.
- `PsState::playing` (`src/ui/playstation.rs:119`) et `playing(app)` (`:135`) font la même chose.
- Toutes les autres commandes enregistrées sont appelées ; aucune commande appelée n'est absente ; événements émis et écoutés concordent.

### WINAPP-64 — Clés de traduction de l'interface inutilisées
- **Effort** : S · **Confiance** : vérifié (préfixes dynamiques pris en compte)
- **Où** : `ui/src/lib/i18n/fr.json` et `en.json` : `home.welcome_title`, `home.welcome_lead` (`:66-67`), `camera.indicator` (`:865`), `mock.lend_expired`, `mock.lend_no_code` (`:486-488`).
- **Correctif** : les retirer ; ajouter la détection des clés inutilisées à `scripts/check-i18n.mjs`.

### WINAPP-65 — `allow(dead_code)` de module qui cache du code mort
- **Effort** : S · **Confiance** : vérifié
- **Où** : `src/decoder.rs:6` et `src/pyrowave.rs:18` ; `pyrowave::Decoder::config` (`pyrowave.rs:456`) sans appelant ; champ `Decoder::fourcc` (`decoder.rs:86`) jamais lu.
- **Correctif** : réserver `allow(dead_code)` aux liaisons FFI générées, supprimer ce qui ressort.

### WINAPP-66 — Fonctionnalités `windows` peut-être inutiles
- **Effort** : S · **Confiance** : plausible (à confirmer en compilant)
- **Où** : `apps/windows/Cargo.toml` : `Win32_System_Kernel`, `Win32_Devices_Properties` sans usage direct dans `src/` (elles peuvent être exigées par une signature d'API).

## Documentation

### WINAPP-67 — PyroWave présenté comme une option
- **Effort** : S · **Confiance** : vérifié
- **Où** : `scripts/build.sh:7-12` (« `--kyber-pyrowave` : Kyber demande PyroWave ») alors que `KYBER_PYROWAVE` vaut `1` par défaut (`:38`) ; `Cargo.toml:15-18` (« tant qu'ils ne sont pas allumés pour toutes les apps ») ; l'option `--sans-pyrowave` n'est pas documentée.
- **Correctif** : mettre à jour l'en-tête et le commentaire (FR, et README.en si cité).

### WINAPP-68 — `csp: null` avec `withGlobalTauri`
- **Effort** : S · **Confiance** : vérifié (configuration)
- **Où** : `apps/windows/tauri.conf.json:9-12`.
- **Description** : aucun `{@html}` ni `innerHTML` dans `ui/src` (pas de faille connue), mais l'interface affiche des chaînes venues du relais et des Stations (noms, notes de version) et l'IPC entier est exposé à la page : pas de seconde barrière.
- **Correctif** : CSP minimale (`default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'`), à essayer avec les aperçus `data:`.

---

## Dix priorités

1. **WINAPP-02** — ouverture d'adresse par `cmd /C start` dans l'installateur (et `server/installer`) : correctif de quelques lignes, défaut de sécurité.
2. **WINAPP-01** — reprise après carte graphique perdue : seule cause d'image figée sans issue.
3. **WINAPP-03** — mise à jour qui coupe une partie PlayStation.
4. **WINAPP-04 + -05 + -57** — une seule écriture atomique sous verrou pour `app.json` et les autres fichiers (évite de rallumer l'ouverture à la connexion).
5. **WINAPP-12 + -39** — crochet clavier et chemin de la souris sans verrous (risque de clavier figé sur tout le PC).
6. **WINAPP-13 + -22 + -47 + -56** — module WASAPI commun : suivi du périphérique par défaut, handles, MMCSS.
7. **WINAPP-09, -10, -11, -41** — lot S sur le rendu : instantané multi-écrans, `quit`, boucle PyroWave, vignette.
8. **WINAPP-18 + -19 + -21** — caméra : plus de blocage du fil principal ni de la fin de session, messages justes.
9. **WINAPP-36 + -37 + -38** — poids de l'interface : mock hors de l'app, aperçus hors de `computers`, liste de la barre des tâches.
10. **WINAPP-49 à -54** — remonter dans le cœur HDR, `KNPW`, flux d'images, plan de session, mDNS et paquet PlayStation, là où les apps divergent déjà.

## Ce qui est sain

- **Commandes Tauri** : presque toutes celles qui touchent au réseau sont `async` ; `tray_command` travaille sur un fil à part ; aucun verrou tenu pendant un appel réseau (`refresh`, `learn`, `search_once`, `share_watch`) ; ordre de prise des verrous cohérent (`sessions` → `launching` → `cancelled_launch`) ; commandes, événements et interface alignés.
- **Fin de session** : `watch_end` et `QUIT_WITHIN` couvrent une session qui ne répond plus.
- **Fichiers et secrets** : `computers.json` atomique avec `fsync` et mis de côté s'il est illisible ; secrets dans le gestionnaire d'identifiants.
- **D3D** : section critique partagée avec FFmpeg, `Present` et l'attente de la synchro verticale hors verrou (un écran à 60 Hz ne freine pas un écran à 160 Hz) ; `ResizeBuffers` sous `ClearState`/`Flush` ; repli 10 bits → 8 bits ; durées de vie FFmpeg et PyroWave propres.
- **Crochet clavier** : génération et ordre `SeqCst` corrects, `UnhookWindowsHookEx` à la sortie, touches injectées ignorées.
- **COM** : `CoInitializeEx`/`CoUninitialize` appariés sur des fils dédiés ; `CoTaskMemFree`/`PropVariantClear` présents ; presse-papiers sur son propre fil, exclu de l'historique.
- **Interface** : tous les `listen()` et `setInterval` nettoyés ; courses entre réponses gérées par numéro de demande ; enregistrements chaînés dans Settings et ConsoleSettings ; pas de `{@html}` ; catalogues FR et EN identiques ; `prefers-reduced-motion` respecté.
- **Installateur** : pose dans `KyberNow.new` puis renommage unique avec retour arrière ; `unpack_in` sans sortie du dossier ; HKCU et `asInvoker` ; programmes système par chemin complet ; choix x64/ARM64 par `IsWow64Process2`.
- **Tests** : logique pure testée (`hdr.rs`, `displays.rs`, `pyrowave_config.rs`, `glyph.rs`, `follow.rs`) ; décodage du test Auto dans un processus séparé et borné.
- Aucun `TODO`/`FIXME`.
