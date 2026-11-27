# Corrections de l'audit du 27/11 — app Windows (`apps/windows`)

Branche `claude/fix-windows-app-632af9`, vague 1. Rapport d'origine : [`../windows-app.md`](../windows-app.md). Aucun PC Windows n'était disponible : **rien n'a été essayé sur Windows**.

## Constats corrigés

| ID | Correction |
|---|---|
| WINAPP-01 | Carte graphique perdue (`DXGI_ERROR_DEVICE_REMOVED`/`RESET`, ou `GetDeviceRemovedReason`) : `DrawError::Lost` distingué du format refusé (plus de repli sur le processeur) ; la carte de la session est remplaçable (`gpu::SessionGpu`), le premier fil de l'image la recrée, chaque écran refait son rendu, rouvre son décodeur et demande une image clé (15 s d'essais au plus). |
| WINAPP-02 | Installateur : adresses ouvertes par `ShellExecuteW` après `account::openable_url` (plus de `cmd /C start`). |
| WINAPP-03 | « Installer la mise à jour » refusée aussi pendant une connexion qui démarre ou une partie PlayStation (`can_quit`, commun avec « Quitter »), revérifiée après le téléchargement. |
| WINAPP-04, -05, -57, -44 | `files.rs` : un seul dossier, une seule écriture atomique (temporaire propre à l'appel, `sync_all`, renommage), fichier illisible mis de côté ; `app.json` modifié et écrit sous son verrou (`update_prefs`) ; `computers.json`, `accounts.json`, `friends.json`, `playstation.json`, `synced.json` y passent ; `synced.json` réécrit seulement s'il change ; `lending::credential_of` au lieu du `format!` recopié. |
| WINAPP-06 | Mot de passe posé avant d'ajouter l'ordinateur (et l'ordinateur emprunté, `lending::id_for`) : rien n'est gardé s'il est refusé. |
| WINAPP-08 | `SessionHost::start_as` rend l'erreur du fil refusé au lieu de paniquer sous le verrou `sessions`. |
| WINAPP-09 | Vignette de l'accueil : tout le pipeline de l'écran relié (`bind_image`) et ses constantes rechargées avant le tracé. |
| WINAPP-10 | `close_window` commune à `quit` et `remove_window` : fil de l'image arrêté avant `DestroyWindow`, HCURSOR libéré. |
| WINAPP-11 | Échec d'ouverture du décodeur PyroWave retenu jusqu'au prochain changement de format (plus un essai et une ligne d'erreur par image). |
| WINAPP-12 | Crochet clavier : il ne fait que décider (lectures en `RwLock`, cache par fil) ; l'envoi part par la fenêtre de contrôle (`WM_APP_HOOK_KEY`), jamais dans le rappel. |
| WINAPP-13, -22, -47, -56 | `wasapi.rs` commun au son et au micro : suivi du périphérique par défaut (relu chaque seconde), événement fermé à toute sortie (`Event`), MMCSS « Pro Audio » (`AudioThread`), boucle de réouverture partagée (`keep_open`). |
| WINAPP-14 | Au retour sur la session, modificateurs tenus sur l'ordinateur mais relâchés sur ce PC : relâchés ; verrouillage de Windows (`WM_WTSSESSION_CHANGE`) : tout est relâché. |
| WINAPP-15 | Souris absolue (VM, Bureau à distance, tablette) suivie en mode jeu (`mouse.rs`, essais). |
| WINAPP-16 | Impr. écran envoyée avec son appui (Windows n'envoie que le relâchement). |
| WINAPP-17 | `WM_DPICHANGED` : la zone cliente garde sa taille (fenêtre, pas en plein écran). |
| WINAPP-18 | `session_camera_state` asynchrone, liste des webcams gardée 10 s. |
| WINAPP-19 | Fin de session sans attendre une webcam figée (1 s au plus, le fil finit seul sans rien envoyer), sender lâché hors du verrou. |
| WINAPP-20 | Webcam et encodeur (et candidats écartés) fermés par `ShutdownObject`. |
| WINAPP-21 | Accès refusé, occupée ou impossible selon l'erreur de Windows ; 720p essayé si le 1080p est refusé. |
| WINAPP-23 | Presse-papiers lu dans les bornes de `GlobalSize`. |
| WINAPP-24 | Plantage noté par `WriteFile` direct, tampon fixe, sans verrou ni allocation ; rien pour un débordement de pile. |
| WINAPP-25 | Micro : une capture lâchée attend la fin d'un envoi en cours et n'en fait plus (`PUSH`). |
| WINAPP-26 | Réglages d'une console : le retrait différé des écouteurs du clic droit est annulé par une nouvelle saisie. |
| WINAPP-27 | Menu contextuel de WebView2 retiré hors des champs de saisie (`lib/nomenu.ts`, quatre fenêtres). |
| WINAPP-28 (en partie) | Barre : profil, souris et « Garder le son » refusés reviennent à l'ancien choix, avec l'avis. |
| WINAPP-29 | Démarrage de l'accueil : lectures en parallèle (`allSettled`), `take_action` et l'avis des Stations même si l'une échoue. |
| WINAPP-30 | Feuille d'ajout fermée pendant `ps_handoff_receive` / `pair_start` : l'attente est annulée côté app. |
| WINAPP-32 | Installateur, dossier échangé fichier par fichier : fichiers absents de la nouvelle version retirés (`payload::stale_files`, essai). |
| WINAPP-34 | `SetDefaultDllDirectories` en tête des deux `main` (System32 pour le lanceur, plus le dossier de l'interface pour elle). |
| WINAPP-35 | `uninstall.exe` : entrée « Security directory » remise à zéro quand elle désigne au-delà de la copie (`payload::strip_security_dir`, essai). |
| WINAPP-36 | `mock.ts` chargé par import dynamique en développement seulement, textes à part (`mock.{fr,en}.json`) : `api-*.js` passe de 169 à 119 Ko, plus de faux comptes ni de prénom dans `ui/dist`. |
| WINAPP-37 | Images de l'accueil servies par le protocole `knimage` (`http://knimage.localhost/<id>?v=<n>`) : l'événement `computers` ne porte plus les JPEG en base64. |
| WINAPP-38 | Liste de la barre des tâches refaite seulement quand elle change, sur un seul fil. |
| WINAPP-39 | Plus de verrou `APPS` à chaque message (cache par fil, `RwLock`) ; `WM_INPUT` déjà en file lus d'un coup, un seul envoi. |
| WINAPP-41 | Vignette pour l'écran principal seulement ; `stop_video` arrête tous les fils puis les attend ensemble (2 s en tout). |
| WINAPP-43 | Commandes PlayStation `async`, `playstation.json` écrit hors du verrou. |
| WINAPP-63 (en partie) | `lend_refresh`, `playstation::status_of`, `errors.video_thread` retirés. |
| WINAPP-64 | Clés mortes retirées (`home.welcome_title`, `home.welcome_lead`, `camera.indicator`, `pyrowave.banner_text` de l'interface ; la rubrique `mock`). |
| WINAPP-67 | `build.sh`, `Cargo.toml` et README : PyroWave allumé par défaut, `--sans-pyrowave`. |
| TRANS-25 | `bar.mouse_shortcut` remise (« (Ctrl+Alt+Maj+M) »). |
| TRANS-27 (app Windows) | `ui/scripts/check-i18n.mjs` : clés et paramètres alignés (interface, Rust de l'app, cœur, imitation), chaque `t('…')` de l'interface et `t("…")` du Rust de l'app existe, aucune clé de l'interface inutilisée ; `setup/ui/scripts/check-i18n.mjs` : clés employées et paramètres. |
| TRANS-40 | Toutes les fenêtres d'une session réduites : débit plafonné à 20 Mbit/s (`Session::set_bitrate_cap`, valeur de l'app Mac), rendu à la réouverture. |

## Constats laissés

| ID | Raison |
|---|---|
| WINAPP-07, -55 | Ordre des envois des réglages synchronisés : la file par compte relève du cœur (`kybernow_core::account::settings`), vague 2. |
| WINAPP-08 (reste) | Utilitaire de verrou qui récupère un verrou empoisonné dans tout `ui.rs` (≈ 180 `lock().unwrap()`) : refonte large, pas faite. |
| WINAPP-11 (reste) | Signaler au cœur de repasser en HEVC quand PyroWave ne s'ouvre pas : le cœur n'a pas cette entrée (zone cœur). |
| WINAPP-28 (reste) | `AppSheet`, `Settings`, `App`, `Station`, `Fond`, `Connexion` : même traitement à faire, de préférence par un utilitaire commun (vague 2, avec WINAPP-58). |
| WINAPP-31 | Échap, focus et `inert` des feuilles : passe par le composant `Sheet` partagé (WINAPP-58), vague 2. |
| WINAPP-33 | WebView2 par le client HTTPS de l'installateur et `WinVerifyTrust` : M, à essayer sur un PC. |
| WINAPP-40 | Pool de tampons par image (M) ; seul le tableau de vues sans allocation est fait (`bind_image`). |
| WINAPP-42, -45, -46, -48 | Optimisations restantes (effets Svelte, `fsync` sous verrou, relevé en parallèle, mise en page de la barre cachée) : pas dans les priorités. |
| WINAPP-49 à -54, -62 | Remontée dans le cœur : vague 2, comme demandé. |
| WINAPP-56 (reste) | `MicUi` dans `kybernow_core::micro` : cœur, vague 2. |
| WINAPP-58 à -61 | Factorisation de l'interface (feuille partagée, composants entre apps, découpe de `Bar.svelte`) : vague 2. |
| WINAPP-63 (reste) | `PsState::playing` / `playing(app)` en double : laissé. |
| WINAPP-65, -66 | `allow(dead_code)` de module et fonctionnalités `windows` : à revoir en compilant sans, vague 2. |
| WINAPP-68 | CSP : à poser avec les adresses `knimage` et les aperçus, à essayer dans WebView2 avant. |
| Hors zone | `server/installer/src/bin/ui.rs:49` a le même `cmd /C start` que WINAPP-02 (autre session). TRANS-40 voulait la valeur dans le cœur (`session::background_bitrate`) : la constante `HIDDEN_BITRATE` de `window.rs` y montera. |

## Vérifié, et comment

- **Compilation croisée x64** : `cargo check --target x86_64-pc-windows-gnu -p kybernow-windows --features kyber-pyrowave` (tests compris) sans avertissement de l'app, depuis le worktree, contre les sources corrigées de Kyber de l'arbre `arbres-windows-app` ; puis `apps/windows/scripts/build.sh` complet (release) dans cet arbre au dernier commit : `kybernow.exe`, ses DLL et le zip produits sans erreur.
- **Compilation croisée ARM64** : `cargo check --target aarch64-pc-windows-gnullvm -p kybernow-windows --features kyber-pyrowave` sans erreur ni avertissement de l'app (pas de construction complète ARM64).
- **Édition de liens** : `apps/windows/scripts/link-check.sh` (x64 et `--arch arm64`) : vérifiée.
- **Installateur** : `cargo check --target x86_64-pc-windows-gnu --features ui --bins` ; `cargo test --lib` (13 essais, dont `adresses_ouvrables`, `signature_hors_de_la_copie`, `fichiers_retires`).
- **Essais Rust de l'app sur le Mac** : `cargo test -p kybernow-windows --features kyber-pyrowave` : 129 essais (nouveaux : `files::*`, `mouse::*`, `lending::liste_des_ordinateurs_empruntes` étendu). Les essais des modules Windows seuls (`camera::echecs_d_ouverture`) compilent pour Windows mais ne tournent que sur un PC.
- **Interface** : `npm run check` (vérificateur de traductions, svelte-check, tsc) pour `ui/` et `setup/ui/` : 0 erreur ; `npm run build` : plus de `mockTauri` ni de faux comptes dans `ui/dist`.
- **Pas essayé sur Windows** : tout le reste (perte de carte, crochet clavier, WASAPI, caméra, protocole `knimage`, installateur).

## À essayer sur un PC

1. Mise à jour du pilote graphique ou TDR forcé (`dxcap -forcetdr`) en pleine session, un et deux écrans : l'image revient seule.
2. Clavier : passage d'un ordinateur à l'autre en tapant vite, Alt+Tab et ⊞ en immersif ; Ctrl+Alt+Suppr puis Annuler ; Win+L ; Impr. écran ; souris 4-8 kHz en jeu ; Windows dans Parallels en mode jeu.
3. Son : choisir « Casque » dans le menu son de Windows en session ; micro de communication changé ; casque Bluetooth qui se reconnecte.
4. Caméra : webcam 720p avec « 1080p », caméra refusée par la confidentialité, caméra prise par une autre app, obturateur fermé à la fin de session.
5. Accueil : aperçus et dernière image (protocole `knimage`), clic droit sur la barre (pas de menu), fenêtre réduite (débit à 20 Mbit/s au journal).
6. Installateur : lien de la page du compte avec `&` ; mise à jour avec le dossier verrouillé (explorateur ouvert dessus).

## Notes de version

**Français**
- KyberNow Link pour Windows reprend l'image tout seul après une mise à jour du pilote graphique ou une sortie de veille, au lieu de rester figé.
- Le son et le micro suivent le casque ou les haut-parleurs que vous choisissez dans Windows pendant une session.
- Le clavier ne peut plus se figer pendant un passage d'un ordinateur à l'autre, et Ctrl ou Alt ne restent plus enfoncés après Ctrl+Alt+Suppr ou le verrouillage.
- La touche Impr. écran arrive bien à l'ordinateur, et la souris marche en mode jeu sous Windows dans une machine virtuelle.
- La caméra dit enfin pourquoi elle ne part pas (accès refusé, déjà utilisée), passe en 720p si votre webcam ne fait pas le 1080p, et n'empêche plus de fermer une session.
- « Installer la mise à jour » ne coupe plus une partie PlayStation en cours.
- Vos réglages ne peuvent plus être perdus par une coupure de courant pendant leur enregistrement.
- L'accueil est plus léger : les images de vos ordinateurs ne sont plus renvoyées à chaque changement.
- Une fenêtre de session réduite consomme moins de réseau.
- L'installateur ouvre les liens de façon plus sûre et retire les fichiers d'anciennes versions.

**English**
- KyberNow Link for Windows now brings the picture back by itself after a graphics driver update or waking from sleep, instead of staying frozen.
- Sound and microphone follow the headphones or speakers you pick in Windows during a session.
- The keyboard can no longer freeze while switching from one computer to another, and Ctrl or Alt no longer stay pressed after Ctrl+Alt+Del or locking.
- The Print Screen key now reaches the computer, and the mouse works in game mode when Windows runs in a virtual machine.
- The camera now tells you why it doesn't start (access denied, already in use), switches to 720p if your webcam can't do 1080p, and no longer holds up closing a session.
- “Install update” no longer cuts a PlayStation game in progress.
- Your settings can no longer be lost by a power cut while they are being saved.
- The home screen is lighter: your computers' pictures are no longer sent again at every change.
- A minimized session window uses less network.
- The installer opens links more safely and removes files from older versions.
