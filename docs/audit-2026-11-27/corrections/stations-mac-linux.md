# Corrections du 27/11 — Stations Mac et Linux

Branche `claude/fix-stations-mac-linux-e1c42f`, d'après [stations-mac-linux.md](../stations-mac-linux.md). Zone : `server/macos` et `server/linux`, sans leurs `kyber-patches` ni `server/admin`.

## Bug signalé : plusieurs icônes dans la barre des menus (Mac)

**Cause**, lue dans le journal du Mac de l'auteur (`log show`, 26/11 à 18:39) : l'app ouverte par Spotlight est lancée par LaunchServices (`gui/501/application.fr.leslignesclaires.kybernow.station…`, « launch job demand »), puis, 22 s plus tard, le LaunchAgent de l'icône (`fr.leslignesclaires.kybernow.station.menubar`, `RunAtLoad`) en lance une seconde (« speculative »). Rien n'empêchait deux copies de tourner. Même chose après une mise à jour : une icône ouverte à la main ne dépend pas du LaunchAgent, survit à son `bootout`, et la nouvelle version en ajoute une. Pas de `KeepAlive` sur l'icône, pas de relance après plantage, pas d'élément de connexion : ce n'était pas en cause.

**Correctif** (`server/macos/menubar/Single.swift`) : l'icône est celle du LaunchAgent (reconnu par `XPC_SERVICE_NAME`). Une copie lancée autrement démarre le LaunchAgent s'il ne tourne pas, ouvre Admin s'il tournait (comme une seconde ouverture), et s'arrête. Un verrou `flock` (`menubar.lock` dans le dossier de la Station, pid et origine dedans) tranche les lancements simultanés ; le LaunchAgent qui trouve le verrou tenu par une copie ouverte à la main lui envoie SIGTERM et attend. Il arrête aussi les copies d'avant ce correctif, qui ne prennent pas le verrou (même identifiant de paquet, même exécutable : les fenêtres de `kybernow-ui` ne sont pas touchées). Sans LaunchAgent installé (essais), la première copie garde l'icône.

## Constats corrigés

| Id | Correctif |
|---|---|
| bug du 27/11 | Une seule icône (ci-dessus) |
| STMAC-01 | Toute commande en argument de `kybernow-display` est ponctuelle ; `lock` est reconnu ; une commande inconnue est une erreur (code 2), jamais le mode agent |
| STMAC-02 | Agent des écrans : SIGPIPE ignoré, réponses par stdio, SIGTERM/SIGINT/SIGHUP rallument les écrans du Mac, propre groupe de processus (`setpgid`) pour survivre à l'arrêt d'Admin par launchd |
| STMAC-03 | Caméra : l'entrée n'accepte que `…kybernow-camera`, identifiant et signature de l'équipe vérifiés sur le processus |
| STMAC-04 | `audio-input` pour l'app, kyavserver et kybernow-micro seulement |
| STMAC-05 | « Accepter » un emprunt depuis une notification demande un Mac déverrouillé |
| STMAC-09 | La sortie d'avant n'est jamais l'une des nôtres ; `restore` choisit la sortie intégrée (ou une autre vraie) si le Mac resterait sur KyberNow |
| STMAC-10 | `isolate` lit tous les champs avant la configuration et l'annule sur erreur |
| STMAC-11 | `create` sur un rang occupé passe par la garde de `remove` (et vide `retina`, `modes`, `scaled` de l'écran retiré : une partie de STMAC-22) |
| STMAC-12 | `display-off.json` écrit de façon atomique |
| STMAC-13 | Notifications gardées quand Admin ne répond pas |
| STMAC-14 | Un relevé de l'icône à la fois (pas d'intervalle plus long menu fermé) |
| STMAC-18 | `--path-format=absolute --git-common-dir` dans `build-station-app.sh` et `run-station.sh` |
| STMAC-25 | `docs/station-macos.md` (filet de l'agent, écrans éteints, applicateur, LaunchAgents seulement, nombre de correctifs, droits), en-tête de `build-kyber.sh`, 44,1 kHz dans le pilote |
| STLIN-02 (unité) | Socket 0660, groupe `kybernow-station` (sysusers), compte qui installe (`sudo`, `pkexec`) ajouté au groupe et noté `SocketUser` (effet immédiat, sans rouvrir la session) ; message sinon |
| STLIN-04 | Agent des écrans Linux : SIGTERM/SIGINT/SIGHUP bloqués puis attendus par un fil, l'agent rend les écrans et s'arrête |
| STLIN-05 (doc) | Limite écrite dans `docs/station-linux.md` : pendant un prêt, clavier et souris de l'ordinateur ne sont pas bloqués |
| STLIN-16 | postrm / `%postun` : rechargements, purge de `/var/lib/kybernow-station` et du propriétaire noté |
| STLIN-17 | Installation dans le dossier de l'utilisateur : `uhid` chargé, déclenché et dans `modules-load.d` |
| STLIN-19 | Chemin fixe `~/.local/share/kybernow/station` dans `kybernow-station-setup`, l'agent des écrans et `kybernow-station-home` |
| STLIN-22 | `release-station.sh` : `|| true`, le message « .deb ou .rpm absent » s'affiche |
| STLIN-23 (b, c) | `build-kyber.sh` Linux : verrou des sources (`scripts/verrou.sh`), empreinte `.kybernow-patches` (`kyber-stamp.sh`) vérifiée par `package.sh` et `run-station.sh` ; en-tête précisé sur la remise à l'origine |
| STLIN-25 | Boucle morte du stylet retirée |
| STLIN-26 (en partie) | `INSTALLATION.md` (noms publiés, Sway et Hyprland), `docs/station-linux.md` (`import-environment` sous Sway/Hyprland, fichier des écrans relu toutes les 250 ms) |
| STLIN-27 | Choix écrit dans `docs/station-linux.md` (Clavier, souris, pointeur) |
| STCOM-05 (en partie) | Les deux `build-kyber.sh` repèrent un correctif posé avec décalage du contexte (essai à blanc `patch -F0`) et le signalent ; `KYBERNOW_PATCH_STRICT=1` en fait un échec |
| TRANS-07 (Mac) | `build-station-app.sh` retire `work/kybernow-camera` avant de le reconstruire : un échec de `clang` arrête la construction |

## Constats laissés

| Id | Raison |
|---|---|
| STMAC-16 | Session des correctifs Kyber (confirmé par la coordination). Mac `0011-kynput-manettes-mac.patch` : dans `report()`, aide morte (`helper = None`) → la relancer après `HELPER_RETRY` et recréer toutes les manettes, comme `plug` le fait pour une aide neuve |
| STLIN-12 | Session des correctifs Kyber. Linux `0002-kynput-pointeur-wayland.patch` : `OnceLock<Option<…>>` → `Mutex<Option<…>>` avec nouvel essai après délai ; au changement d'appareil, relâcher sur l'ancien les boutons qu'il a appuyés |
| STLIN-13 | Session des correctifs Kyber. Linux `0005-kynput-manettes-linux.patch` : `uhid_failed` remis à faux après un délai (nouvel essai), `libc::O_NONBLOCK` au lieu de `0o4000` |
| STLIN-06 | Correctif Kyber Linux 0013 : session des correctifs Kyber (consigne) |
| STLIN-07, STLIN-08, STCOM-03, STCOM-12 | Dans `server/admin` (`update/linux.rs`, `micro.rs`, `display/mod.rs` et i18n) : session Admin |
| STLIN-02 (Admin) | `serve()` qui n'accepte que le propriétaire, et un message clair quand le socket refuse (EACCES) : `server/admin` |
| STLIN-05 (affichage, vrai blocage) | L'afficher avant d'accorder le prêt : Admin ou l'icône (`server/tray`), hors zone. Le vrai blocage (aide root par socket) est un chantier M |
| STLIN-23 (a) | Entrées du cache `station_linux` dans `scripts/release.sh` (fichier commun) : ajouter `apps/macos/kyber-patches` et `kyber_tetes linux kyber-desktop-linux` |
| STCOM-05 (`-F0` par défaut) | Les correctifs Mac **0009** et **0030** ne se posent qu'avec un décalage : `-F0` cassait la construction. À rafraîchir par la session des correctifs, puis `KYBERNOW_PATCH_STRICT=1` par défaut. `server/kyber-patches/apply.sh` (hors zone) n'a pas encore la vérification |
| TRANS-07 (`server/camera/build.sh`, `server/encoder-check/build.sh`) | Hors zone ; côté Mac, le retrait avant construction suffit pour la caméra. `kybernow-encoder-check` est écrit directement dans l'app neuve (pas de reste possible) |
| STMAC-02 (Admin) | `process_group(0)` dans `display/mac.rs` : inutile, l'agent prend son groupe lui-même |
| STLIN-26 (reste) | `PROJECT.md` (fichier commun) et commentaires de `server/camera/linux` (hors zone) |
| STMAC-25 (reste) | Commentaire du correctif Mac 0020 (« 0016 ») : `kyber-patches` |
| STLIN-15, STLIN-14, STMAC-15, STMAC-17, STMAC-19 à 22, STLIN-09 à 11 | Hors des listes demandées pour cette vague |

## Vérifié, et comment

- **Icône** : `server/macos/menubar/build.sh` compile et lance `--check-strings` puis `--check-instance` (décision dans les 8 cas : LaunchAgent seul, LaunchAgent après Spotlight, Spotlight avec ou sans LaunchAgent en marche, essais sans LaunchAgent ; verrou pris, refusé à une seconde ouverture du fichier, titulaire lu, rendu puis repris). Cause lue dans le journal du Mac. Pas lancée en vrai : la Station de l'auteur tourne.
- **Agent des écrans Mac** : `display-agent/build.sh` lance `kybernow-display self-test` (commandes ponctuelles reconnues, `lock` compris ; choix de la sortie rendue dans 7 cas ; sortie d'avant retenue). Commande inconnue : code 2 et erreur. Mode agent lancé à part (`HOME` d'essai) : `ping` répond, son propre groupe de processus (PGID = PID), SIGTERM → arrêt propre, fichier des écrans écrit dans le `HOME` d'essai (celui de l'auteur intact).
- **Caméra** : `main.swift` compile (`swiftc`). Pas essayée.
- **Agent des écrans Linux** : `cargo check` pour `aarch64-unknown-linux-gnu`, `cargo test` sur le Mac (6 essais, dont le nouveau `signal_d_arret_attendu`), clippy sans nouvel avertissement.
- **Scripts** : `bash -n` sur tous les scripts de `server/linux` et `server/macos/scripts` ; `shellcheck` sans nouvel avertissement ; postinst, prerm et postrm extraits de `package.sh` passés à `sh -n` et `shellcheck -s sh`.
- **Correctifs de Kyber pour la Station Mac** : arbre à moi (`third_party/arbres-fix-stations-mac-linux/station-mac`), `KYBER_PATCH_ONLY=1` : tout se pose ; avertissement pour 0009 et 0030.
- **Construction de la Station Mac** : dans le même arbre, `build-kyber.sh` complet (kycontroller, kyavserver, kynputserver, kybernow-pyrowave-check produits par ce build) puis `build-station-app.sh` signé ad hoc (`KYBER_SIGN_IDENTITY=-`, sans trousseau) : app, installateur, désinstalleur, .dmg et zip de mise à jour construits. Droit `audio-input` relu par `codesign -d --entitlements` : présent pour l'app, kyavserver et kybernow-micro, absent des autres. `--check-instance` et `self-test` repassés sur les exécutables de l'app. Pas de caméra dans cette construction (pas de profil sans trousseau) : STMAC-03 et TRANS-07 compilés, pas emballés. Rien d'installé ; la Station de l'auteur n'a pas été touchée.

## À essayer sur appareil

- Mac : ouvrir l'app par Spotlight avec l'icône en marche (Admin s'ouvre, pas de seconde icône) ; icône arrêtée par « Quitter » puis app rouverte (une icône, celle du LaunchAgent) ; mise à jour depuis Admin.
- Mac : « Redémarrer » pendant une session, écrans du Mac éteints → ils reviennent ; prêt interrompu par un Admin relancé → session verrouillée.
- Mac : caméra de l'appareil vue dans FaceTime (kybernow-camera admis ; une autre app refusée, ligne « entrée refusée » au journal `fr.leslignesclaires.kybernow.station.camera`). Si macOS refuse `SecCodeCopyGuestWithAttributes` dans le bac à sable, la caméra ne marche plus : c'est le premier point à regarder.
- Mac : son de la session (kyavserver) et micro de l'appareil (kybernow-micro) avec les droits restreints ; Admin, kycontroller et kybernow-display sans le droit.
- Mac : notification d'emprunt sur l'écran verrouillé : « Accepter » demande le déverrouillage.
- Linux : installation du .deb et du .rpm avec `sudo` (groupe créé, `owner.conf`, mise à jour depuis Admin) ; avec un second compte, demande refusée ; `apt purge` / `dnf remove` ; Sway/Hyprland : `systemctl --user stop kybernow-admin` rallume les écrans.

## Notes de version

**FR**
- Station Mac : une seule icône dans la barre des menus, même quand vous ouvrez KyberNow Station depuis Spotlight ou le Finder.
- Station Mac : si la Station s'arrête ou redémarre pendant une session, les écrans du Mac se rallument.
- Station Mac : un prêt interrompu par un redémarrage d'Admin verrouille bien la session.
- Station Mac : le son du Mac revient sur vos haut-parleurs après une session, même après une reconnexion.
- Station Mac : seule la Station peut envoyer des images à « KyberNow Camera ».
- Station Mac : accepter un emprunt depuis une notification demande un Mac déverrouillé.
- Station Linux : seul votre compte peut demander une mise à jour de la Station. Si vous l'avez installée sans `sudo`, ajoutez votre compte au groupe `kybernow-station`.
- Station Linux : sous Sway et Hyprland, les écrans de l'ordinateur se rallument quand la Station s'arrête.
- Station Linux : la désinstallation complète (purge) retire aussi les anciennes versions gardées pour le retour arrière.
- Station Linux (Bazzite, SteamOS) : les manettes PlayStation sont reconnues sans redémarrer.
- Station Linux : pendant un prêt, le clavier et la souris de l'ordinateur ne sont pas encore bloqués ; ne prêtez pas un ordinateur que d'autres peuvent toucher.

**EN**
- Mac Station: a single icon in the menu bar, even when you open KyberNow Station from Spotlight or the Finder.
- Mac Station: if the Station stops or restarts during a session, the Mac's displays come back on.
- Mac Station: a loan interrupted by an Admin restart now locks the session.
- Mac Station: the Mac's sound returns to your speakers after a session, even after reconnecting.
- Mac Station: only the Station can send pictures to “KyberNow Camera”.
- Mac Station: accepting a loan from a notification requires an unlocked Mac.
- Linux Station: only your account can ask the Station to update. If you installed it without `sudo`, add your account to the `kybernow-station` group.
- Linux Station: on Sway and Hyprland, the computer's displays come back on when the Station stops.
- Linux Station: a full uninstall (purge) also removes the older versions kept for rollback.
- Linux Station (Bazzite, SteamOS): PlayStation controllers are recognized without restarting.
- Linux Station: during a loan, the computer's keyboard and mouse are not blocked yet; do not lend a computer that others can touch.
