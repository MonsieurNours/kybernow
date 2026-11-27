# Corrections de l'audit du 27/11 — app Windows (`apps/windows`)

Branche `claude/fix-windows-app-632af9`, vague 1. À la demande de la coordination, tous les constats du rapport ont été traités, sauf WINAPP-49 à -54 (vague 2, remontée dans le cœur) ; ceux qui demandent de toucher au cœur ou à d'autres apps sont décrits plus bas. Rapport d'origine : [`../windows-app.md`](../windows-app.md). Aucun PC Windows n'était disponible : **rien n'a été essayé sur Windows**.

## Constats corrigés

| ID | Correction |
|---|---|
| WINAPP-01 | Carte graphique perdue (`DXGI_ERROR_DEVICE_REMOVED`/`RESET`, ou `GetDeviceRemovedReason`) : `DrawError::Lost` distingué du format refusé (plus de repli sur le processeur) ; la carte de la session est remplaçable (`gpu::SessionGpu`), le premier fil de l'image la recrée, chaque écran refait son rendu, rouvre son décodeur et demande une image clé (15 s d'essais au plus). |
| WINAPP-02 | Installateur : adresses ouvertes par `ShellExecuteW` après `account::openable_url` (plus de `cmd /C start`). |
| WINAPP-03 | « Installer la mise à jour » refusée aussi pendant une connexion qui démarre ou une partie PlayStation (`can_quit`, commun avec « Quitter »), revérifiée après le téléchargement. |
| WINAPP-04, -05, -57, -44 | `files.rs` : un seul dossier, une seule écriture atomique (temporaire propre à l'appel, `sync_all`, renommage), fichier illisible mis de côté ; `app.json` modifié et écrit sous son verrou (`update_prefs`) ; `computers.json`, `accounts.json`, `friends.json`, `playstation.json`, `synced.json` y passent ; `synced.json` réécrit seulement s'il change ; `lending::credential_of` au lieu du `format!` recopié. |
| WINAPP-07 | Réglages synchronisés envoyés sur un seul fil, dans l'ordre des changements (`send_queue`), au lieu d'un fil par envoi. |
| WINAPP-06 | Mot de passe posé avant d'ajouter l'ordinateur (et l'ordinateur emprunté, `lending::id_for`) : rien n'est gardé s'il est refusé. |
| WINAPP-08 | `SessionHost::start_as` rend l'erreur du fil refusé au lieu de paniquer sous le verrou `sessions` ; et les 472 `lock().unwrap()` de l'app deviennent `locked()` (`sync::Locked`, essai), qui reprend un verrou empoisonné : une panique sous un verrou ne fige plus toute l'app. |
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
| WINAPP-28 | Barre : profil, souris et « Garder le son » refusés reviennent à l'ancien choix, avec l'avis ; réglages de l'app : langue et synchronisation reviennent aussi ; retrait d'un ordinateur ou d'un ami en échec dit ; `save_settings`, `Fond` et `Connexion` notent leurs rejets. |
| WINAPP-31, -58 | `lib/Sheet.svelte` : fond commun des dix feuilles et confirmations (clic dehors, Échap pour la feuille du dessus, reste de la page inerte sauf la barre de titre, focus posé puis rendu) ; `lib/menukeys.ts` : ↑ ↓, Début, Fin, Échap dans les sept menus de la barre et celui du compte de l'accueil. |
| WINAPP-29 | Démarrage de l'accueil : lectures en parallèle (`allSettled`), `take_action` et l'avis des Stations même si l'une échoue. |
| WINAPP-30 | Feuille d'ajout fermée pendant `ps_handoff_receive` / `pair_start` : l'attente est annulée côté app. |
| WINAPP-33 | WebView2 téléchargé par le client HTTPS de l'installateur (`account::download`, redirections en HTTPS seulement, essai), lancé seulement avec une signature valide de « Microsoft Corporation » (`sys::verify_signature`, `WinVerifyTrust`). |
| WINAPP-32 | Installateur, dossier échangé fichier par fichier : fichiers absents de la nouvelle version retirés (`payload::stale_files`, essai). |
| WINAPP-34 | `SetDefaultDllDirectories` en tête des deux `main` (System32 pour le lanceur, plus le dossier de l'interface pour elle). |
| WINAPP-35 | `uninstall.exe` : entrée « Security directory » remise à zéro quand elle désigne au-delà de la copie (`payload::strip_security_dir`, essai). |
| WINAPP-36 | `mock.ts` chargé par import dynamique en développement seulement, textes à part (`mock.{fr,en}.json`) : `api-*.js` passe de 169 à 119 Ko, plus de faux comptes ni de prénom dans `ui/dist`. |
| WINAPP-37 | Images de l'accueil servies par le protocole `knimage` (`http://knimage.localhost/<id>?v=<n>`) : l'événement `computers` ne porte plus les JPEG en base64. |
| WINAPP-38 | Liste de la barre des tâches refaite seulement quand elle change, sur un seul fil. |
| WINAPP-39 | Plus de verrou `APPS` à chaque message (cache par fil, `RwLock`) ; `WM_INPUT` déjà en file lus d'un coup, un seul envoi. |
| WINAPP-40 | Tampons des images repris d'une image à l'autre (`BufferPool`), tampon des paquets FFmpeg réutilisé (`av_buffer_make_writable`, `av_buffer_realloc`), vues des textures et historique du stylet sans allocation. |
| WINAPP-41 | Vignette pour l'écran principal seulement ; `stop_video` arrête tous les fils puis les attend ensemble (2 s en tout). |
| WINAPP-42 | `displays_info` et `computer_wired` redemandés seulement quand leur clé change. |
| WINAPP-43 | Commandes PlayStation `async`, `playstation.json` écrit hors du verrou. |
| WINAPP-45 | `computers.json` et `accounts.json` préparés sous le verrou, écrits après, le plus récent gagne (`files::write_versioned`, essai). |
| WINAPP-46 | Relevé des ordinateurs en parallèle, six à la fois, un seul envoi à l'interface. |
| WINAPP-48 | Barre cachée : mesures seulement notées ; largeur et rectangle relus seulement quand sa taille change (`ResizeObserver`). |
| WINAPP-59 (app Windows) | L'interface de l'installateur prend la marque, l'interrupteur et les polices de celle de l'app (alias `$appui`, `dedupe` de Svelte) ; plus de copies dans `apps/windows`. |
| WINAPP-60 | `Bar.svelte` de 2 925 à 2 450 lignes : `bar/SharePanel.svelte`, `LendPanel.svelte`, `CodeTop.svelte`, `RelayLine.svelte`, `PsBar.svelte`, styles des panneaux dans `bar/panels.css`. |
| WINAPP-61 | Rubrique Station : `stationRefusal` et `stationRefusalText` d'`api.ts`. |
| WINAPP-63 | `lend_refresh`, `playstation::status_of`, `errors.video_thread`, `playstation::playing` en double retirés. |
| WINAPP-65 | `allow(dead_code)` de module retirés de `decoder.rs` et `pyrowave.rs` ; `Decoder::fourcc` et `pyrowave::Decoder::config` supprimés. |
| WINAPP-66 | `Win32_Devices_Properties` retirée (x64 et ARM64 compilent) ; `Win32_System_Kernel` gardée (sections critiques, filtre d'exceptions). |
| WINAPP-68 | CSP minimale dans `tauri.conf.json` (IPC de Tauri, images `data:` et `knimage`), vérifiée sur les pages construites servies avec cet en-tête. |
| WINAPP-64 | Clés mortes retirées (`home.welcome_title`, `home.welcome_lead`, `camera.indicator`, `pyrowave.banner_text` de l'interface ; la rubrique `mock`). |
| WINAPP-67 | `build.sh`, `Cargo.toml` et README : PyroWave allumé par défaut, `--sans-pyrowave`. |
| TRANS-25 | `bar.mouse_shortcut` remise (« (Ctrl+Alt+Maj+M) »). |
| TRANS-27 (app Windows) | `ui/scripts/check-i18n.mjs` : clés et paramètres alignés (interface, Rust de l'app, cœur, imitation), chaque `t('…')` de l'interface et `t("…")` du Rust de l'app existe, aucune clé de l'interface inutilisée ; `setup/ui/scripts/check-i18n.mjs` : clés employées et paramètres. |
| TRANS-40 | Toutes les fenêtres d'une session réduites : débit plafonné à 20 Mbit/s (`Session::set_bitrate_cap`, valeur de l'app Mac), rendu à la réouverture. |

## Constats laissés

| ID | Raison |
|---|---|
| WINAPP-49 à -54 | Remontée dans le cœur : vague 2, comme demandé. |
| WINAPP-11 (reste) | Demander au cœur de repasser en HEVC quand PyroWave ne s'ouvre pas : le cœur n'a pas cette entrée (zone cœur). L'app retient l'échec et ne le répète plus. |
| WINAPP-55 | La mémoire des envois synchronisés dans `kybernow_core::account::settings` : zone cœur (l'ordre des envois est corrigé côté app, WINAPP-07). |
| WINAPP-56 (reste) | `MicUi` dans `kybernow_core::micro`, textes du catalogue du cœur : zone cœur. Le module WASAPI commun est fait. |
| WINAPP-59 (reste) | Partager ces composants avec `server/web` et `server/installer` (dossier `shared/ui/`) : autres zones. |
| WINAPP-62 | `primary` et `session_hdr` à déplacer dans `kybernow_core::screens` : zone cœur. |
| Hors zone | `server/installer/src/bin/ui.rs:49` a le même `cmd /C start` que WINAPP-02 (autre session). TRANS-40 voulait la valeur dans le cœur (`session::background_bitrate`) : la constante `HIDDEN_BITRATE` de `window.rs` y montera. |

## Vérifié, et comment

- **Compilation croisée x64** : `cargo check --target x86_64-pc-windows-gnu -p kybernow-windows --features kyber-pyrowave` (tests compris) sans avertissement de l'app, depuis le worktree, contre les sources corrigées de Kyber de l'arbre `arbres-windows-app` ; puis `apps/windows/scripts/build.sh` complet (release) dans cet arbre au commit f4d90782 (tout le code de la branche) : `kybernow.exe`, ses DLL et le zip produits sans erreur.
- **Compilation croisée ARM64** : `cargo check --tests --target aarch64-pc-windows-gnullvm -p kybernow-windows --features kyber-pyrowave` sans erreur ni avertissement de l'app (pas de construction complète ARM64).
- **Édition de liens** : `apps/windows/scripts/link-check.sh` (x64 et `--arch arm64`) : vérifiée.
- **Installateur** : `cargo check --target x86_64-pc-windows-gnu --features ui --bins` ; `cargo test --lib` (14 essais, dont `adresses_ouvrables`, `signature_hors_de_la_copie`, `fichiers_retires`, `telechargement_adresses`).
- **Essais Rust de l'app sur le Mac** : `cargo test -p kybernow-windows --features kyber-pyrowave` : 131 essais (nouveaux : `files::*`, `mouse::*`, `sync::verrou_empoisonne_repris`, `lending::liste_des_ordinateurs_empruntes` étendu). Les essais des modules Windows seuls (`camera::echecs_d_ouverture`) compilent pour Windows mais ne tournent que sur un PC.
- **Interface** : `npm run check` (vérificateur de traductions, svelte-check, tsc) pour `ui/` et `setup/ui/` : 0 erreur, 0 avertissement ; `npm run build` : plus de `mockTauri` ni de faux comptes dans `ui/dist`. Découpe de la barre et feuilles vérifiées dans l'imitation (`npm run dev`, navigateur) : styles calculés de chaque élément comparés avant et après (panneaux Partager et Prêter dans six états, barre d'une partie avec et sans manette), identiques ; créer un code, accepter une demande, couper le son d'une partie, Échap et page inerte d'une feuille essayés. `setup/ui` se construit à l'identique sans les modules de `ui/`.
- **Pas essayé sur Windows** : tout le reste (perte de carte, crochet clavier, WASAPI, caméra, protocole `knimage`, installateur).

## À essayer sur un PC

1. Mise à jour du pilote graphique ou TDR forcé (`dxcap -forcetdr`) en pleine session, un et deux écrans : l'image revient seule.
2. Clavier : passage d'un ordinateur à l'autre en tapant vite, Alt+Tab et ⊞ en immersif ; Ctrl+Alt+Suppr puis Annuler ; Win+L ; Impr. écran ; souris 4-8 kHz en jeu ; Windows dans Parallels en mode jeu.
3. Son : choisir « Casque » dans le menu son de Windows en session ; micro de communication changé ; casque Bluetooth qui se reconnecte.
4. Caméra : webcam 720p avec « 1080p », caméra refusée par la confidentialité, caméra prise par une autre app, obturateur fermé à la fin de session.
5. Accueil : aperçus et dernière image (protocole `knimage`), clic droit sur la barre (pas de menu), fenêtre réduite (débit à 20 Mbit/s au journal).
6. Installateur : lien de la page du compte avec `&` ; mise à jour avec le dossier verrouillé (explorateur ouvert dessus) ; PC sans WebView2 (téléchargement et signature).
7. Interface dans WebView2 : CSP (rien de bloqué), feuilles (Échap, focus, barre de titre active), menus au clavier, panneaux de la barre.
8. Réglages synchronisés changés vite d'un appareil à l'autre.

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
- L'installateur ouvre les liens de façon plus sûre, retire les fichiers d'anciennes versions et installe WebView2 sans PowerShell, après avoir vérifié qu'il vient de Microsoft.
- Les fenêtres et les menus de l'app se ferment avec Échap et se parcourent au clavier.
- Vos réglages synchronisés arrivent sur vos autres appareils dans le bon ordre, même changés coup sur coup.

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
- The installer opens links more safely, removes files from older versions and installs WebView2 without PowerShell, after checking it comes from Microsoft.
- The app's windows and menus close with Esc and can be browsed with the keyboard.
- Your synced settings reach your other devices in the right order, even when changed in quick succession.
