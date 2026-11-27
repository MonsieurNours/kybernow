# Corrections de l'audit du 27/11 — app Linux et KyberNow Link OS

Branche `claude/fix-linux-app-6637be` (deux passes : les 10 priorités, puis tout le reste hors vague 2, à la demande de la coordination). Rapport d'origine : [`../linux-app.md`](../linux-app.md). Tout est dans `apps/linux` (app, Link OS `os/`, scripts), sauf ce fichier, `docs/pyrowave-client-pi.md` et `.en.md` (doc du client Pi), et deux lignes d'`apps/Cargo.lock` (`ureq`, déjà dans le cœur, ajouté à l'app Linux pour le manifeste de Link OS).

## Constats corrigés

| Id | Correctif |
|---|---|
| LINAPP-01 | `apply` ne passe plus par `xz -dc \| head` ni `xz -dc \| dd` : `kybernow-os-image write` (python3, livré dans l'image) lit l'image une seule fois. Plus de SIGPIPE sous `pipefail`. |
| LINAPP-02 | État de root dans `/var/lib/kybernow/.os-state` (root 0700) ; `/var/lib/kybernow` à root, l'app n'a que `config` et `update` (`kybernow-os-update prepare`, au lancement de l'app et au premier démarrage, liens refusés). |
| LINAPP-03 | `apply` recopie dans le nouvel emplacement les réseaux Wi-Fi, `/var/lib/bluetooth`, SSH allumé ou non (`systemctl --root`), nom, `/etc/hosts`, fuseau, et le `video=` de `cmdline.txt`. |
| LINAPP-04, 05, 06 | `SessionUi::ui_owns_input` (menu, Touches, panneaux Partager, Prêter, Écran, menus de la barre) : seule règle pour clavier, manettes, souris des jeux, pointeur du système, stylet et mode immersif ; la souris garde le chemin de l'image pendant un menu de la barre (`panel_open`) ; Échap ferme un menu déroulant. |
| LINAPP-07 | `commit` n'inscrit l'essai que sur le démarrage à l'essai (`tryboot=1`) ; un essai en attente sans `tryboot` est noté `failed` (rendu par `status`, affiché dans Mise à jour, `rollback` refusé vers lui) ; un `commit` pendant le démarrage qui a écrit l'image ne conclut rien (`boot_id`). |
| LINAPP-08 | Chien de garde matériel (`RuntimeWatchdogSec=15s`) ; `kybernow-os-trial.timer` (5 min) → `trial-guard` redémarre un essai pas inscrit ; `StartLimitIntervalSec=0` sur le service de l'app. |
| LINAPP-09 | sudoers limité aux arguments exacts (`apply` sans chemin, `ssh on`/`off`, modes HDMI…). Root copie le manifeste et sa signature chez lui, les vérifie (`minisign`, clé publique dans `/etc/kybernow-os/release.pub`, commentaire de confiance, version plus récente), et compare l'empreinte de ce qu'il a lu et écrit à celle du manifeste. L'app dépose le manifeste et sa signature à côté de l'image. |
| LINAPP-10 | Racines A et B à 3 200 Mo fixes ; `make-ab-image.sh` refuse un système qui ne laisserait pas 512 Mo libres. |
| LINAPP-11 | `flock` dans `kybernow-os-update` (apply, commit, rollback) ; l'app ne lance pas un second `apply` ni un `rollback` pendant une écriture. |
| LINAPP-12 | BT.709 plein (PyroWave SDR) ramené en plage réduite sur le fil du décodeur (`video_rules`), puis montré en BT.709 ; chaque fenêtre repose sa conversion YUV avant de créer, remplir et dessiner sa texture, et la recrée quand elle change. |
| LINAPP-13 | `EAGAIN` : code dédié ; les images prêtes sortent, le paquet repart ; encore refusé, il n'est pas compté (`ResumeGate`) et la suite attend une image clé. |
| LINAPP-14 | `XInitThreads` au lancement, avant `SDL_Init` (et avant la connexion du stylet). |
| LINAPP-15 | File vers le décodeur bornée à 30 paquets (`video_rules::Backlog`) : au-delà, jetés jusqu'à une image clé demandée ; le décodeur l'apprend (`Msg::Dropped`). |
| LINAPP-16 | `software` et `failed` remis à faux à chaque `Msg::Codec`, `failed` aussi après une ouverture réussie. |
| LINAPP-17 | H.264 et HEVC du Pi annoncés seulement si `kn_v4l2_m2m_built` / `kn_v4l2_request_built`. |
| LINAPP-19 (sauf le regroupement) | Menus déroulants bornés à la fenêtre (gauche, largeur, lignes qui tiennent, libellés raccourcis) ; barre plus large que la fenêtre alignée à droite (« Se déconnecter » visible) ; captures `bureau-17g-fenetre-etroite-barre` et `17h-…-profil` en 480 × 270 et essai `narrow_extra_window_keeps_bar_and_menu_inside`. |
| LINAPP-21 | Défilement des réglages borné à la fin de la rubrique (mesurée au dessin) ; au clavier, la rubrique suit le focus ; une ligne passée sous la barre de titre n'est ni survolée ni cliquée. |
| LINAPP-27 | `rerun-if-changed` sur l'en-tête de PyroWave, de Vulkan et les `libavcodec.a`/`libavutil.a` du préfixe. |
| LINAPP-37 | Une seule décompression, blocs de 4 Mio (avec LINAPP-01). |
| LINAPP-46 | `vgradient`, `image_height`, `image` et 9 icônes supprimés ; `allow(dead_code)` retirés d'`icons.rs` et `theme.rs`. |
| LINAPP-47 | `kn_decoder_flush` supprimée ; les deux sondes V4L2 branchées (LINAPP-17) ; bit « HEVC Main 10 » sans lecteur retiré du masque. |
| LINAPP-48 | `lowered_label` supprimée ; `PsSettings::height` et `bitrate` en `#[cfg(test)]`, `allow` retiré. |
| LINAPP-49 | `cec-wake` et `hostname` retirés de `kybernow-os-ctl` ; `x-systemd.growfs` retiré du `fstab`. |
| LINAPP-50 | Dépendance directe `kynput-sys` retirée (le cœur l'apporte). |
| LINAPP-52 | `os/README`, `INSTALLATION` (FR et EN) et le texte de la rubrique Mise à jour dits vrais : retour seul aussi sur un système figé ou sans accueil, réglages gardés, renommage d'`autoboot.txt` non atomique sur FAT, cartes d'avant la 0.11.8 à reflasher une fois. |
| LINAPP-18 | Webcam : `bytesperline` du pilote gardé (YUYV, NV12) ; image MJPEG recopiée avec `AV_INPUT_BUFFER_PADDING_SIZE` octets nuls ; `calloc`, `malloc`, `realloc`, `av_frame_alloc`, `av_packet_alloc`, `av_frame_get_buffer` vérifiés. |
| LINAPP-20 | Cibles `BarSharePill` et `BarShareRequests` (même action que « Partager ») : un anneau à la fois. |
| LINAPP-22 | Un lien `kybernow://` attend la fin du test Auto, de « Hors de chez vous ? » et de l'avis de mise à jour des Stations. |
| LINAPP-23 | Nom du décodeur refait par `snprintf` (« CPU HEVC », plus « CPUHEVC ») et relu après la première image. |
| LINAPP-24 | Stylet X11 : seuls les événements de l'esclave (`deviceid == sourceid`). |
| LINAPP-25 | Instance et place vérifiées avant `SDL_GameControllerOpen` ; refermée sinon. |
| LINAPP-26 | `relative_day` compare les dates locales d'aujourd'hui et d'hier. |
| LINAPP-28 | `FFMPEG_SRC` et `PYROWAVE_SRC` jamais effacés ni remplis : arrêt avec un message s'ils sont incomplets. |
| LINAPP-29 | `JustWorksRepairing = confirm` ; l'agent de l'app répond « yes » seulement pendant l'écran Manette (écoute `bluetoothctl`). |
| LINAPP-31 | Commentaire de `kn_cam_set_bitrate` aligné sur docs/webcam.md (le débit fixé à l'ouverture reste ; la caméra rallumée prend le nouveau). |
| LINAPP-32 | Seule la dernière image matérielle prête est transférée ; paramètres et image copiés une fois dans le paquet de FFmpeg (`kn_decoder_send_parts`, plus de `concat`). La recopie dans `Frame` et le VA-API sans copie restent (voir plus bas). |
| LINAPP-33 | 10 → 8 bits par lignes (sans contrôle de bornes par pixel) ; image réduite de l'accueil faite par le fil du décodeur (`Latest::take_snapshot`). |
| LINAPP-34 | Groupe de 3 fils créé une fois pour la conversion 4:4:4 → BGRA (FFmpeg et PyroWave). |
| LINAPP-35 | Glyphes empruntés au cache ; cache en deux générations ; `text_fit` par dichotomie ; largeur de la barre gardée 100 ms pour le survol ; écrans Manettes, Ajouter, Rejoindre, Compte, Accueil en trois écrans à 30 i/s au lieu de 60. |
| LINAPP-36 | La rubrique ouverte cherchée d'abord (`sections_open_first`) ; manettes lues une fois par tour ; `clone_from` pour les zones de clic. |
| LINAPP-38 | pi-gen figé (étiquette) et stages 0 à 2 repris ; module Flatpak `kybernow-media` (FFmpeg, PyroWave) à part du SDK de Kyber ; crates de cargo dans `/run/ccache` (`--ccache`). |
| LINAPP-39 | `preview.rs` compilé seulement pour les essais et sans `device` ; `--splash` dans `src/splash.rs` ; le code qui ne servait qu'aux captures réservé aux captures. |
| LINAPP-51 | PyroWave demandé par défaut dit partout ; `--sans-pyrowave` dans les usages. |
| LINAPP-53 (textes) | « Via the relay » (link-ui), « Back to home », « Test the controllers », « Choose » partout ; raccourci de la caméra sans `.replace("Maj", …)`. |
| LINAPP-54 (sauf le prêt) | Commentaire de `pyrowave_none.c` ; message `apt install` de `build-image.sh` (table commande:paquet) ; `samples-mac.sh` s'arrête sur une erreur de `swiftc`. |
| TRANS-15 (Linux) | `release-flatpaks.sh` n'amorce le cache de flatpak-builder que si un Flatpak est demandé, dans un dossier temporaire renommé. |
| TRANS-21 (Linux) | pi-gen figé sur `2026-10-06-raspios-trixie-arm64` ; FFmpeg du Pi figé sur `6dbf87ae` ; archives extraites dans `.part` puis renommées, avec un marqueur ; PyroWave cloné dans `.part` ; montage `SRC` dans le `cleanup` de `make-ab-image.sh`. |
| (comme WEBOS-01) | `kybernow.json` illisible jamais réécrit vide : mis de côté sous `kybernow.json.illisible-<date UTC>`, l'app le dit au lancement ; présent mais pas lisible (droits) ou impossible à déplacer : rien n'est écrit de la session. |
| (copie de TRANS-35) | `parse_address` : IPv6 entre crochets avec le port (`[fe80::1]:24811`) ; nue (`fe80::1`), refusée au lieu de devenir l'hôte `fe80:` sur le port 1. |

## Constats laissés, et pourquoi

| Id | Raison |
|---|---|
| LINAPP-40 à 45 | Vague 2 (réglages synchronisés et `PsSettings` sur le cœur, fonctions communes avec webOS, correctifs Kyber en une copie, commit de PyroWave lu dans `COMMIT`, petits widgets). |
| LINAPP-19 (regroupement), 30, 53 (le reste), 54 (le prêt) | Demandent une règle dans `docs/link-ui.md` d'abord : proposées ci-dessous, à implémenter une fois écrites. |
| LINAPP-32 (le reste) | Garder la référence `AVFrame` jusqu'à la texture et le VA-API sans copie (`vaExportSurfaceHandle` → DMA-BUF → EGL) changent tout l'affichage et ne se vérifient que sur une carte (Intel, AMD, Pi) : un chantier à part, décrit dans docs/pyrowave-client-pi.md. |
| LINAPP-35 (le reste) | Le fond de l'accueil mis à l'échelle à chaque image (`image_cover_rounded`) : le garder en cache demande de suivre ses changements (photo, taille) ; à faire avec une mesure sur le Pi. |

## Règles proposées pour docs/link-ui.md (à écrire par la coordination)

**Barre de session dans une fenêtre étroite (LINAPP-19).** Quand la barre ne tient pas dans la fenêtre (marge de 8 points de chaque côté), les icônes de la barre, de la gauche vers la droite et dans cet ordre (Ordinateurs, Touches, Souris, Garder le son, Micro, Caméra, Plein écran), passent une à une dans un menu « Plus » (icône trois points, à la place de la première icône retirée), jusqu'à ce que la barre tienne. Ne passent jamais dans « Plus » : la latence, le profil, « Partager », sa pastille, « Prêter », « Se déconnecter ». Le menu « Plus » liste chaque élément avec son nom et son état (coché quand il est allumé), et ouvre le même menu ou fait la même bascule que l'icône. Les mesures (définition, images par seconde, décodeur) disparaissent avant toute icône.
- FR : « Plus » ; infobulle « Plus de commandes ».
- EN : “More”; tooltip “More controls”.

**Langue, clavier, fuseau et pays du Wi-Fi de KyberNow Link OS (LINAPP-30).** Au premier démarrage, l'écran Langue propose, sous la langue, une ligne « Pays » (liste des pays, celui de la langue choisie par défaut : France pour le français, Royaume-Uni pour l'anglais britannique, États-Unis sinon). Le pays règle le fuseau (le premier fuseau du pays ; une seconde liste « Fuseau horaire » seulement pour les pays qui en ont plusieurs), le domaine radio du Wi-Fi et la disposition d'un clavier USB (AZERTY pour la France et la Belgique, QWERTZ pour la Suisse et l'Allemagne, QWERTY sinon). Tout se change ensuite dans Réglages de l'appareil › Langue. Ces réglages sont gardés d'une mise à jour à l'autre (c'est déjà le cas du fuseau).
- FR : « Pays », « Fuseau horaire », « Le Wi-Fi, l'heure et le clavier suivent le pays. »
- EN : “Country”, “Time zone”, “Wi-Fi, time and keyboard follow the country.”

**Barre de session, écarts de l'app Linux (LINAPP-53).** À trancher dans le tableau de la barre : (1) l'infobulle du profil « Profil : Jeu, Bureau ou Max » / “Profile: Game, Desktop or Max” sur Linux aussi ; (2) « Plein écran (F11) » et « Se déconnecter (Ctrl+Alt+Maj+D) » en infobulle / “Full screen (F11)”, “Disconnect (Ctrl+Alt+Shift+D)” ; (3) la souris en menu « Auto / Souris du bureau / Souris des jeux » / “Auto / Desktop mouse / Game mouse” plutôt qu'une bascule ; (4) la pastille « Presse-papiers » / “Clipboard” ; (5) le menu Touches : les 4 entrées de link-ui sur toutes les apps, ou les 10 de Linux écrites dans link-ui. Proposition : (1), (2) et (3) comme link-ui ; (4) comme link-ui ; (5) écrire les 10 entrées de Linux dans link-ui, dans cet ordre : « Ctrl + Alt + Suppr », « Alt + Tab », « Touche Windows », « Windows + D : bureau », « Alt + F4 », « Échap », « F11 : plein écran », « Gestionnaire des tâches », « Impr. écran », « Windows + L : verrouiller » / “Ctrl + Alt + Del”, “Alt + Tab”, “Windows key”, “Windows + D: desktop”, “Alt + F4”, “Esc”, “F11: full screen”, “Task Manager”, “Print Screen”, “Windows + L: lock” (les 4 de link-ui en tête, inchangées si elles y sont déjà), puis les reprendre dans les autres apps.

**Avertissement du prêt (LINAPP-54).** Selon le système de la Station :
- Windows — FR : « Votre ami joue dans votre session Windows. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance. » EN : “Your friend plays in your Windows session. They see neither your desktop nor your windows, but a game can read your files: lend to friends you trust.”
- Mac et Linux — FR : « Votre ami joue dans votre session sur cet ordinateur. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance. » EN : “Your friend plays in your session on this computer. They see neither your desktop nor your windows, but a game can read your files: lend to friends you trust.”
- Système inconnu (Station d'avant) : la variante Mac et Linux.

## Vrai cause hors de la zone

- `docs/link-ui.md` : quatre règles à écrire (ci-dessus) avant les derniers morceaux de LINAPP-19, 30, 53 et 54.
- Les cartes déjà flashées gardent l'ancien `kybernow-os-update` dans leur emplacement en cours : c'est lui qui écrit la mise à jour suivante, et il s'arrête toujours (LINAPP-01). Elles ne passeront à cette version qu'en reflashant la carte une fois (dit dans INSTALLATION et le README de l'OS). Rien à corriger ailleurs, mais à dire dans les notes de version.

## Ce qui est vérifié, et comment

- `cargo test -p kybernow-linux --no-default-features` sur le Mac : 166 essais, tous passent, dont les nouveaux : `ui_owns_input_while_a_panel_is_open`, `settings_scroll_is_bounded_and_follows_focus`, `bar_stays_in_a_narrow_window`, `dropdowns_stay_in_the_window`, `narrow_extra_window_keeps_bar_and_menu_inside` (480 × 270, FR et EN), `video_rules::*` (file bornée, matrice et plage), `store_file_tests::*` (fichier illisible, fichier absent, date UTC), `address` (IPv6), `hier_au_nouvel_an`, le lien pendant « Hors de chez vous ? » dans `an_invitation_link`, les pastilles de partage dans les essais de la barre.
- Partie appareil (`src/device`, non compilable sur le Mac) : `KYBERNOW_CHECK_ONLY=1 cargo check -p kybernow-linux --features pyrowave` dans mon arbre (`third_party/arbres-fix-linux-app`, correctifs Kyber des apps posés, `libvlc.pc` et `kynput.pc` factices) : seule erreur, `WindowHandle::Xlib`, propre à Linux ; cette ligne remplacée le temps d'un essai, tout le Rust de l'app se vérifie **sans un avertissement** (le code des seules captures réservé aux captures). Les essais de `src/device/os.rs` (`etat_ab`, `questions_de_l_agent`) ne tournent que sous Linux.
- C : `decoder.c` vérifié par `cc -fsyntax-only -Wall` (FFmpeg de Homebrew, en-têtes libva factices) ; le groupe de fils du 4:4:4 essayé dans un petit programme (3 fils, 300 conversions 1280 × 720 identiques, deux appelants en même temps, sans blocage). `camera.c` et `pen.c` (en-têtes Linux) relus seulement.
- Scripts : `bash -n` et `shellcheck -S warning` sans remarque sur tous les scripts de `os/`, `os/stage-kybernow` et `scripts/` (`build.sh`, `sources.sh`, `flatpak.sh`, `release-flatpaks.sh`, `deb.sh`…) ; `python3 -m py_compile` sur `kybernow-os-image` ; le manifeste Flatpak relu par `yaml.safe_load`. Rien n'a tourné dans la VM (interdit pendant les corrections) : le module `kybernow-media`, `--ccache` et la reprise de pi-gen sont à vérifier à la prochaine construction.
- `apps/linux/os/tests/os-update.sh` (Mac, sans root) : image factice MBR à deux partitions, compressée en `.xz`, écrite en un passage dans deux fichiers (contenu identique, empreinte et taille du fichier lu), refus d'un emplacement trop petit (sortie 6), d'un `.xz` tronqué, d'une image sans table ; un octet ajouté change l'empreinte ; manifeste lu, fichier absent refusé, comparaison des versions ; `minisign -V -Q` rend bien le commentaire de confiance et refuse un manifeste modifié ; `kybernow-os-update` dans un faux système (montages simulés) : `prepare`, `status`, essai réussi inscrit, `commit` pendant le démarrage de l'écriture sans effet, essai raté noté et affiché par `status`, `rollback` refusé vers lui, `apply` sans image refusé. 24 vérifications, toutes passent.

## Reste à essayer sur un vrai Raspberry Pi 4 ou 5

1. Flasher une image construite avec cette branche, puis publier (ou servir) une version plus récente : téléchargement (image + manifeste + signature dans `/var/lib/kybernow/update`), `apply` de bout en bout avec `minisign` et `python3` de l'image, redémarrage à l'essai, `commit` à l'accueil.
2. Les deux sens : de A vers B (écriture en bloc de p3), puis de B vers A (fichiers de p1 remplacés, `autoboot.txt` gardé).
3. Après la mise à jour : Wi-Fi reconnecté, manette Bluetooth sans réassociation, SSH dans l'état d'avant, nom, mode HDMI.
4. Essai raté : une image dont l'app plante (ou n'atteint pas l'accueil) → retour en 5 minutes au plus (`trial-guard`), message « La mise à jour X n'a pas démarré », pas de « Revenir à » vers elle. Système figé → chien de garde (`bcm2835-wdt`) en 15 s.
5. Une image modifiée (un octet) ou un manifeste d'une autre clé : refus, emplacement laissé incomplet, aucun redémarrage.
6. `sudo -l` du compte `kybernow` : seules les commandes listées ; `sudo kybernow-os-update apply /autre/chemin` refusé.
7. Couleurs PyroWave SDR sur le Pi 5 (comparer à HEVC sur la même mire) ; `EAGAIN` de `h264_v4l2m2m` (journal « refusé par le décodeur ») ; file bornée en forçant le décodage par le processeur en 4K.
8. Webcam CSI du Pi (lignes alignées) et une webcam MJPEG USB : image droite, pas de lecture hors tampon (ASan si possible).
9. Bluetooth : une manette associée qui se reconnecte (sans question) ; une manette réinitialisée qui demande à se réassocier : refusée hors de l'écran Manette, acceptée dedans.
10. Conversion 4:4:4 et snapshot de l'accueil sur le fil du décodeur : pas d'à-coup toutes les 2 s sur le Pi ; écran Manettes à 30 i/s fluide.
11. Flatpak : deux constructions de suite, la seconde sans refaire FFmpeg ni retélécharger les crates ; pi-gen : seconde image sans debootstrap.
12. Menus de session à la manette (Prêter, Écran, Partager) sur le Deck et au salon ; Échap dans un menu de la barre ; mode immersif « Toujours » avec le menu ouvert (Alt+Tab rendu).

## Notes de version

**Français**

- KyberNow Link OS : les mises à jour du Raspberry Pi aboutissent enfin. Votre Wi-Fi, vos manettes associées, SSH, le nom de l'appareil et le mode HDMI sont gardés d'une version à l'autre.
- Si une nouvelle version ne démarre pas, le Pi revient tout seul à la précédente, même s'il se fige, et vous le dit dans Réglages de l'appareil › Mise à jour.
- Chaque mise à jour est vérifiée une seconde fois par le système avant d'être écrite.
- Une carte installée avant cette version doit être réinstallée une fois avec Raspberry Pi Imager pour profiter des mises à jour.
- Linux : les menus de session (Prêter, Écran, Partager) et les menus de la barre répondent au clavier et à la manette ; Échap les referme.
- Linux : couleurs justes en PyroWave, image plus stable quand le décodage peine, réglages qui ne défilent plus dans le vide.
- Linux : si le fichier de vos réglages est abîmé, l'app le met de côté au lieu de l'effacer, et vous prévient.
- Linux : webcams mieux prises en charge, manettes Bluetooth qui ne se réassocient plus en silence, et une app plus légère pour le Raspberry Pi.

**English**

- KyberNow Link OS: Raspberry Pi updates now go through. Your Wi-Fi, paired controllers, SSH, the device name and the HDMI mode are kept from one version to the next.
- If a new version does not start, the Pi goes back to the previous one by itself, even if it freezes, and tells you in Device settings › Update.
- Each update is checked a second time by the system before it is written.
- A card installed before this version needs reinstalling once with Raspberry Pi Imager to get updates.
- Linux: session menus (Lend, Display, Share) and bar menus respond to the keyboard and controller; Esc closes them.
- Linux: correct colours with PyroWave, a steadier picture when decoding struggles, and settings that no longer scroll into emptiness.
- Linux: if your settings file is damaged, the app sets it aside instead of erasing it, and lets you know.
- Linux: better webcam support, Bluetooth controllers that no longer re-pair silently, and a lighter app on the Raspberry Pi.
