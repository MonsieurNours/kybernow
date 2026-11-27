# Corrections de l'audit du 27/11 — app Linux et KyberNow Link OS

Branche `claude/fix-linux-app-6637be`. Rapport d'origine : [`../linux-app.md`](../linux-app.md). Tout est dans `apps/linux` (app, Link OS `os/`, scripts), sauf ce fichier et une ligne d'`apps/Cargo.lock` (`ureq`, déjà dans le cœur, ajouté à l'app Linux pour le manifeste de Link OS).

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
| LINAPP-19 (en partie) | Menus déroulants bornés à la fenêtre (gauche, largeur, lignes qui tiennent, libellés raccourcis) ; barre plus large que la fenêtre alignée à droite (« Se déconnecter » visible). |
| LINAPP-21 | Défilement des réglages borné à la fin de la rubrique (mesurée au dessin) ; au clavier, la rubrique suit le focus ; une ligne passée sous la barre de titre n'est ni survolée ni cliquée. |
| LINAPP-27 | `rerun-if-changed` sur l'en-tête de PyroWave, de Vulkan et les `libavcodec.a`/`libavutil.a` du préfixe. |
| LINAPP-37 | Une seule décompression, blocs de 4 Mio (avec LINAPP-01). |
| LINAPP-46 | `vgradient`, `image_height`, `image` et 9 icônes supprimés ; `allow(dead_code)` retirés d'`icons.rs` et `theme.rs`. |
| LINAPP-47 (en partie) | `kn_decoder_flush` supprimée ; les deux sondes V4L2 branchées (LINAPP-17). |
| LINAPP-48 | `lowered_label` supprimée ; `PsSettings::height` et `bitrate` en `#[cfg(test)]`, `allow` retiré. |
| LINAPP-49 | `cec-wake` et `hostname` retirés de `kybernow-os-ctl` ; `x-systemd.growfs` retiré du `fstab`. |
| LINAPP-50 | Dépendance directe `kynput-sys` retirée (le cœur l'apporte). |
| LINAPP-52 | `os/README`, `INSTALLATION` (FR et EN) et le texte de la rubrique Mise à jour dits vrais : retour seul aussi sur un système figé ou sans accueil, réglages gardés, renommage d'`autoboot.txt` non atomique sur FAT, cartes d'avant la 0.11.8 à reflasher une fois. |
| (comme WEBOS-01) | `kybernow.json` illisible jamais réécrit vide : mis de côté sous `kybernow.json.illisible-<date UTC>`, l'app le dit au lancement ; présent mais pas lisible (droits) ou impossible à déplacer : rien n'est écrit de la session. |
| (copie de TRANS-35) | `parse_address` : IPv6 entre crochets avec le port (`[fe80::1]:24811`) ; nue (`fe80::1`), refusée au lieu de devenir l'hôte `fe80:` sur le port 1. |

## Constats laissés, et pourquoi

| Id | Raison |
|---|---|
| LINAPP-40 | Vague 2 (réglages synchronisés branchés sur le cœur), comme demandé. |
| LINAPP-19 (reste) | Regrouper des icônes dans « … » sous une largeur donnée change la barre : à trancher d'abord dans `docs/link-ui.md` (fichier commun). La capture 480 × 270 de `preview.rs` n'est pas ajoutée (les deux essais unitaires couvrent le bornage). |
| LINAPP-47 (reste) | Le bit « HEVC Main 10 » du masque de `decoder.c` : sans lecteur, mais inoffensif ; à retirer avec la refonte des capacités (vague 2). |
| LINAPP-18, 20, 22 à 26, 28, 29, 31 | Hors des 10 priorités ; petits correctifs isolés, à reprendre dans une prochaine vague (aucun ne bloque une mise à jour ni l'entrée). |
| LINAPP-30 | Langue, clavier, fuseau et pays Wi-Fi de l'image : demande un choix d'interface au premier démarrage (link-ui). Le fuseau choisi est au moins gardé d'une mise à jour à l'autre. |
| LINAPP-32 à 36, 38, 39 | Optimisations, hors priorités. |
| LINAPP-41 à 45 | Factorisations : vague 2. |
| LINAPP-51, 53, 54 | Doc et cohérence avec link-ui : vague 2 (link-ui, traductions). |

## Vrai cause hors de la zone

- `docs/link-ui.md` : la barre dans une fenêtre très étroite (LINAPP-19) n'y a pas de règle ; il en faut une avant de regrouper des icônes.
- Les cartes déjà flashées gardent l'ancien `kybernow-os-update` dans leur emplacement en cours : c'est lui qui écrit la mise à jour suivante, et il s'arrête toujours (LINAPP-01). Elles ne passeront à cette version qu'en reflashant la carte une fois (dit dans INSTALLATION et le README de l'OS). Rien à corriger ailleurs, mais à dire dans les notes de version.

## Ce qui est vérifié, et comment

- `cargo test -p kybernow-linux --no-default-features` sur le Mac : 164 essais, tous passent, dont les nouveaux : `ui_owns_input_while_a_panel_is_open`, `settings_scroll_is_bounded_and_follows_focus`, `bar_stays_in_a_narrow_window`, `dropdowns_stay_in_the_window`, `video_rules::*` (file bornée, matrice et plage), `store_file_tests::*` (fichier illisible mis de côté, fichier absent, date UTC), `address` (IPv6).
- Partie appareil (`src/device`, non compilable sur le Mac) : `KYBERNOW_CHECK_ONLY=1 cargo check -p kybernow-linux --features pyrowave` dans mon arbre (`third_party/arbres-fix-linux-app`, correctifs Kyber des apps posés, `libvlc.pc` et `kynput.pc` factices pour les sondes pkg-config) : seule erreur, `WindowHandle::Xlib`, propre à Linux, déjà connue ; cette ligne remplacée le temps d'un essai, tout le Rust de l'app se vérifie, avec deux avertissements d'avant (`lives` inutilisé dans `route_ps`, `PwConfig::bytes`) et aucun code mort laissé par les suppressions. Les tests de `src/device/os.rs` (`etat_ab`) ne tournent que sous Linux.
- Link OS : `bash -n` et `shellcheck` sur `kybernow-os-update`, `kybernow-os-ctl`, `kybernow-os-firstboot`, `make-ab-image.sh`, `build-image.sh`, `00-run.sh`, `01-run-chroot.sh` et `os/tests/os-update.sh` (seuls restent deux avis « info » SC2012 d'avant, sur `ls -t` dans `build-image.sh`).
- `apps/linux/os/tests/os-update.sh` (Mac, sans root) : image factice MBR à deux partitions, compressée en `.xz`, écrite en un passage dans deux fichiers (contenu identique, empreinte et taille du fichier lu), refus d'un emplacement trop petit (sortie 6), d'un `.xz` tronqué, d'une image sans table ; un octet ajouté change l'empreinte ; manifeste lu, fichier absent refusé, comparaison des versions ; `minisign -V -Q` rend bien le commentaire de confiance et refuse un manifeste modifié ; `kybernow-os-update` dans un faux système (montages simulés) : `prepare`, `status`, essai réussi inscrit, `commit` pendant le démarrage de l'écriture sans effet, essai raté noté et affiché par `status`, `rollback` refusé vers lui, `apply` sans image refusé. 24 vérifications, toutes passent.

## Reste à essayer sur un vrai Raspberry Pi 4 ou 5

1. Flasher une image construite avec cette branche, puis publier (ou servir) une version plus récente : téléchargement (image + manifeste + signature dans `/var/lib/kybernow/update`), `apply` de bout en bout avec `minisign` et `python3` de l'image, redémarrage à l'essai, `commit` à l'accueil.
2. Les deux sens : de A vers B (écriture en bloc de p3), puis de B vers A (fichiers de p1 remplacés, `autoboot.txt` gardé).
3. Après la mise à jour : Wi-Fi reconnecté, manette Bluetooth sans réassociation, SSH dans l'état d'avant, nom, mode HDMI.
4. Essai raté : une image dont l'app plante (ou n'atteint pas l'accueil) → retour en 5 minutes au plus (`trial-guard`), message « La mise à jour X n'a pas démarré », pas de « Revenir à » vers elle. Système figé → chien de garde (`bcm2835-wdt`) en 15 s.
5. Une image modifiée (un octet) ou un manifeste d'une autre clé : refus, emplacement laissé incomplet, aucun redémarrage.
6. `sudo -l` du compte `kybernow` : seules les commandes listées ; `sudo kybernow-os-update apply /autre/chemin` refusé.
7. Couleurs PyroWave SDR sur le Pi 5 (comparer à HEVC sur la même mire) ; `EAGAIN` de `h264_v4l2m2m` (journal « refusé par le décodeur ») ; file bornée en forçant le décodage par le processeur en 4K.
8. Menus de session à la manette (Prêter, Écran, Partager) sur le Deck et au salon ; Échap dans un menu de la barre ; mode immersif « Toujours » avec le menu ouvert (Alt+Tab rendu).

## Notes de version

**Français**

- KyberNow Link OS : les mises à jour du Raspberry Pi aboutissent enfin. Votre Wi-Fi, vos manettes associées, SSH, le nom de l'appareil et le mode HDMI sont gardés d'une version à l'autre.
- Si une nouvelle version ne démarre pas, le Pi revient tout seul à la précédente, même s'il se fige, et vous le dit dans Réglages de l'appareil › Mise à jour.
- Chaque mise à jour est vérifiée une seconde fois par le système avant d'être écrite.
- Une carte installée avant cette version doit être réinstallée une fois avec Raspberry Pi Imager pour profiter des mises à jour.
- Linux : les menus de session (Prêter, Écran, Partager) et les menus de la barre répondent au clavier et à la manette ; Échap les referme.
- Linux : couleurs justes en PyroWave, image plus stable quand le décodage peine, réglages qui ne défilent plus dans le vide.
- Linux : si le fichier de vos réglages est abîmé, l'app le met de côté au lieu de l'effacer, et vous prévient.

**English**

- KyberNow Link OS: Raspberry Pi updates now go through. Your Wi-Fi, paired controllers, SSH, the device name and the HDMI mode are kept from one version to the next.
- If a new version does not start, the Pi goes back to the previous one by itself, even if it freezes, and tells you in Device settings › Update.
- Each update is checked a second time by the system before it is written.
- A card installed before this version needs reinstalling once with Raspberry Pi Imager to get updates.
- Linux: session menus (Lend, Display, Share) and bar menus respond to the keyboard and controller; Esc closes them.
- Linux: correct colours with PyroWave, a steadier picture when decoding struggles, and settings that no longer scroll into emptiness.
- Linux: if your settings file is damaged, the app sets it aside instead of erasing it, and lets you know.
