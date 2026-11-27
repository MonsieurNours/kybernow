# Vague 2 de l'audit du 27/11 : docs, règles et site

Branche `claude/v2-docs-0db24c`, partie de `main` (`165d333f`). Zone : `docs/` (sauf `releases*`, `build-notes*`, `kyber-changes*`), les `README*.md`, `PROJECT*`, `ROADMAP*`, `site/`, `brand/`. Pas de code. Plan : [../vague-2.md](../vague-2.md).

## Fait

| Id | Correctif |
|---|---|
| Caméra (mac.md, coeur.md) | `link-ui` : refus de l'ordinateur selon la raison de son aide caméra (`camera.not_installed`, `camera.extension_off`, puis le texte général), départ refusé par l'app, « Caméra… » sans réponse en 10 s ; d'où vient le choix du texte (`camera::event_notice`). |
| « Envoyer le journal » (android.md, décision de l'auteur du 27/11) | `link-ui`, nouvelle section « Réglages de l'app » : À propos dans toutes les apps ; feuille de partage (Mac, iPhone, iPad, Android, fenêtre « Partager » de Windows), e-mail sous Linux au bureau, QR code sur les téléviseurs, Link OS, le Deck en mode Jeu et Linux au salon (journal chiffré sur l'appareil, clé seulement dans le QR code, dépôt sur le relais), téléchargement dans le client web ; textes et étapes à l'écran, page du téléphone. `relais.md` : section « Dépôt d'un journal » (format, routes, usage unique, une heure, limites, exception de script pour cette page). |
| IOS-30 | `link-ui` : « Disposition du clavier » (Auto, AZERTY, QWERTY, QWERTZ) dans Réglages de l'app › Clavier, avec « Clavier Apple ISO » ; règle d'Auto ; renvoi depuis les consoles PlayStation sur iPad. |
| TVOS-24 | `link-ui` : la capsule « ● Caméra » seule ; le micro a son propre signe, jamais une capsule commune ; l'ancien paragraphe « Le micro suit les mêmes règles… » réécrit. `micro.md` : mode `voiceChat` seulement avec « Réduire l'écho et le bruit » (vérifié dans `AudioPlayerCore.swift:505`). |
| LINAPP-19 | `link-ui` : menu « Plus » des barres au pointeur (ordre de passage, ce qui n'y passe jamais, textes). |
| LINAPP-30 | `link-ui` : pays, fuseau, Wi-Fi et clavier de Link OS au premier démarrage. |
| LINAPP-53 | `link-ui` : barre de l'app Linux (mêmes infobulles, menu de la souris, pastille Presse-papiers) ; menu « Touches » à dix entrées pour Windows et Linux. |
| LINAPP-54 | `link-ui` : avertissement du prêt selon le système de la Station. |
| WEB-25 | `link-ui` : stylet et micro « pas encore » dans le client web ; leur suite dans `ROADMAP`. |
| TRANS-47 (link-ui) | Infobulle du profil avec Auto ; réglage du HDR absent des apps Windows et Linux ; ligne « Micro » de la barre en double retirée, la vraie placée avant « Caméra » (ordre de `SessionChrome.swift`). |
| TRANS-22, COEUR-46 | PyroWave allumé par défaut dans `docs/pyrowave-client-{apple,android,linux,pi,windows}` (FR et EN) et dans les README de l'iPhone, de l'Apple TV et d'Android ; `--sans-pyrowave`, `--no-pyrowave`, webOS sans ; deux points « à faire » déjà faits retirés de la doc Windows. |
| TRANS-33 | `i18n.md` : catalogues de l'icône de la Station Mac et du relais, langue de chacun ; `registre.md` et le README amont de PyroWave dans les exceptions. |
| TRANS-43 | `ROADMAP` : HDR, Station Mac, PlayStation et Apple Pencil sortis du prévu, devenus des essais (HDR sur un écran HDR, PS4, vrai relais, stylet) ; essais de la Station Linux ; demandes à Apple et HDR de la Station Linux dans « Station et Admin » ; mesures de l'audit du 20/10 énoncées dans la ligne. |
| TRANS-44 | Site FR et EN : accueil (« Votre ordinateur », FAQ, carte de téléchargement .deb et .rpm), guide (prérequis, « Sous Linux », question de la FAQ, données structurées), Technologie (PyroWave), `llms.txt`, README du site. Textes de présentation sans liste de systèmes (« l'ordinateur à partager »). |
| TRANS-45, COEUR-47 | Déjà faits par la vague 1 (mêmes titres en FR et EN, vérifié). |
| TRANS-46 | `reprise-apres-perte` : vrais messages de FFmpeg, niveau de journal qui les écrit (`txproto_log_level` ou `ffmpeg_log_level` à `debug`, vérifié dans kyavserver), ligne du cœur (`session.rs:4218`). |
| TRANS-47 (chemins) | `PROJECT`, README de Link OS, doc PyroWave Android : chemins complets de `deb.sh`, `apt-repo.sh`, `strings.py`. README du Mac : « Retirer cet ordinateur » sans points de suspension, comme l'app. |
| MAC-35, MAC-40 | `plusieurs-ordinateurs.md` : plus aucune fonction `kn_*` retirée (vérifié : hors README du cœur, toute fonction `kn_*` citée dans les docs existe dans `apps/apple/src` ou `apps/core`). Le README du cœur est laissé à la session v2-coeur, qui le réécrit pour les nouvelles API (il cite encore `kn_console_share_watch`). |
| KYB-49 | README du Mac : l'image clé est demandée par le cœur, `Client::request_keyframe` de kyclient (correctif 0001), pas par la fonction C. |
| STLIN-26 | `PROJECT` : Station Linux publiée en bêta (0.11.7), onze correctifs propres à Linux, Sway et Hyprland, systèmes immuables, pas de HDR. |
| WEB-26 | README de la Station : les huit espaces d'Admin (et dans le guide du site). |
| TRANS-40, 41 (décisions du 27/11) | `link-ui` : « Image qui n'est plus regardée » (20 Mbit/s Mac, Windows, Linux ; 8 iPhone, iPad, Apple TV ; 2 Android et LG, comme `rules::limits` de la branche v2-coeur) ; débit réglé à la main de 5 à 500 Mbit/s, 2 500 en PyroWave, 95 sur LG, client web 10 à 150 et 1 000 en PyroWave. Ligne « Débit » de `link-ui.en.md` remise à jour (l'interrupteur « Débit adaptatif » y manquait). |
| WINAPP-11 (texte) | `link-ui` : avis « Image en {format} : PyroWave n'a pas pu démarrer sur cet appareil. », une fois, repli pour la session seulement. |
| Micro, « Envoi… » | `link-ui` : gardé, précisé (tuile seulement, avant la première réponse de l'ordinateur ; puce allumée sans texte de plus). |
| SIDELOAD | iPhone et Apple TV : KyberNow Station 0.11.8 ou plus récente (décision de l'auteur). |
| En plus | `relais.md` : section « Plus tard » retirée, ses deux lignes sont déjà dans `ROADMAP` (le prévu n'est que là). |
| TRANS-49 | Logo v1 (`brand/` hors `v2`) et 12 captures webOS que rien ne cite retirés, après grep (voir « Vérifié »). |
| En plus | README racine : « Votre ordinateur sur tous vos appareils », Station sur Windows, Mac et Linux, dossiers `server/macos`, `server/linux`, `server/camera`, `server/micro`. |

## Choix tranchés (à relire par l'auteur)

1. **Barre étroite et « Plus »** : la règle existante reste la première étape (format des mesures retiré, puces en icône seule) ; « Plus » ne vient qu'ensuite. À la liste de LINAPP-19 s'ajoutent la pastille Presse-papiers et l'écran de l'ordinateur (Mac) ; ne passent jamais dans « Plus », en plus de la proposition : « HDR », l'enregistrement (la pastille rouge doit rester visible de tous) et « Par le relais ». Valable pour toutes les barres au pointeur, pas seulement Linux.
2. **Menu « Touches »** : les dix entrées de Linux, mais les quatre de `link-ui` en tête, dans l'ordre de `link-ui` (et non celui de Linux), chacune avec son indication comme sous Windows et dans le client web. « Windows + D » et « Windows + L » s'écrivent « ⊞ + D », « ⊞ + L » ; Linux, dont la police n'a pas ⊞, écrit « Windows » (règle déjà posée pour le mode immersif) : « Touche Windows seule ».
3. **Infobulle du profil** : « Profil : Jeu, Bureau, Max ou Auto », comme l'app Mac. LINAPP-53 proposait la version sans Auto, antérieure au profil Auto.
4. **Avertissement du prêt** : pour Mac et Linux, les textes qu'Admin affiche déjà (« votre session du Mac », « votre session ») plutôt que « votre session sur cet ordinateur » ; système inconnu : « votre session ». L'anglais suit `link-ui` (« They see neither… ») : les variantes Mac et Linux d'Admin disent « They don't see… », à aligner (Stations, phase 2b).
5. **« Envoyer le journal »** : partout, sur décision de l'auteur (27/11). Choix faits en l'écrivant : un seul libellé, « Envoyer le journal », partout (Linux au bureau passe par l'e-mail, sinon l'enregistrement, sans libellé à part) ; « Télécharger le journal » dans le client web ; le Deck en mode Jeu et Linux au salon prennent le QR code, comme les téléviseurs ; 2 Mio compressés au plus, en gardant l'en-tête et la fin ; le relais du compte de l'appareil, sinon le public ; la page du téléphone est la seule page du relais avec un script (il faut WebCrypto pour déchiffrer sans que la clé quitte le téléphone) ; « Terminé » retire un dépôt pas encore ouvert ; l'appareil suit l'état du dépôt pour dire « Journal ouvert sur le téléphone. ».
6. **Disposition du clavier** : dans les Réglages de l'app (elle décrit le clavier, pas l'ordinateur), avec « Clavier Apple ISO », que l'iPhone montre aujourd'hui dans la rubrique Commandes des réglages de l'ordinateur ; à déplacer (Apple, phase 2b). Les correspondances d'Auto suivent celles de Link OS (QWERTZ pour l'allemand et le français de Suisse).
7. **Link OS** : le pays proposé est la France pour le français, les États-Unis pour l'anglais. La proposition parlait de l'anglais britannique, que Link OS ne propose pas (« English · Français »).
8. **Textes de la caméra du Mac** (départ refusé, 10 s sans réponse) : repris tels quels, déjà dans le catalogue de KyberKit, bien qu'ils disent « l'ordinateur » plutôt que « {nom} ».
9. **« Envoi… » du micro** : gardé (honnête sur l'attente), mais seulement dans la tuile et seulement avant la première réponse de l'ordinateur ; la puce de la barre reste « Micro » allumée. Le cœur (`MicUi::tile`) devra dire « Envoi… » tant que l'ordinateur n'a pas répondu.
10. **Repli de PyroWave** : vers le format qu'aurait pris Auto (pas toujours HEVC : AV1 ou H.264 selon l'appareil), d'où « {format} » ; même forme que les replis du client web (« Image en H.264 : … »).

## Laissé, et pourquoi

- **Commentaires de scripts** qui disent encore PyroWave « seulement avec --kyber-pyrowave » : `apps/tvos/scripts/build-kyber-tvos.sh:22-25`, `apps/android/scripts/build-native.sh:39-41`, `apps/windows/scripts/build.sh:104-106`, `apps/macos/scripts/build-kyclient.sh:15-16` (le code allume bien PyroWave par défaut). Zone des scripts.
- **KYB-49, le code** : les fonctions C `kyclient_*_packet_sink` et `kyclient_client_request_keyframe` du correctif 0001 de l'app Mac, et leur ligne dans `docs/kyber-changes.md`. KYB-49 n'est dans aucune session de la phase 2a : à confier.
- **README du cœur** (`apps/core/README*.md`) : confié à la session v2-coeur, à sa demande ; il cite encore `kn_console_share_watch` (MAC-35).
- **Commentaire** `apps/apple/src/share.rs:46` qui cite `kn_console_share_watch`, retirée (Apple, phase 2b).
- **Message agnostique** : `apps/linux/src/ui/setup.rs:424` dit « votre ordinateur Windows » (Linux, phase 2b).
- **STLIN-26, reste** : commentaires de `server/camera/linux` (hors zone, déjà signalé par la vague 1).
- **Menu de l'icône** : son tableau dans `link-ui` n'a ni « Micro » ni « Caméra », que la section de la caméra y met ; à compléter avec TRANS-39.
- **TRANS-48** : la vidéo du site, gardée par décision de l'auteur.
- **Empreintes** : les étapes de `release.sh` qui lisent `brand` (Station, Station Linux, apps) changent d'empreinte avec le retrait du logo v1 : une reconstruction de plus à la prochaine publication.

## À appliquer en phase 2b (règles nouvelles de `link-ui`)

- **Toutes les apps** : textes de la caméra (raisons de l'ordinateur par le cœur, départ refusé, 10 s), voyant de la caméra seul, « Envoyer le journal » (feuille de partage, e-mail ou QR code), avertissement du prêt selon le système, infobulle du profil avec Auto, débit de l'image qui n'est plus regardée et bornes du débit à la main, avis du repli de PyroWave, « Envoi… » du micro.
- **Relais** : le dépôt d'un journal (`relais.md`), sa page et sa mention dans `/privacy`.
- **Mac, Windows, Linux, client web** : menu « Plus » ; menu « Touches » à dix entrées (Windows, Linux, client web).
- **Linux** : barre (infobulles, menu de la souris, pastille Presse-papiers), « Enregistrer le journal… », Link OS (pays, fuseau, Wi-Fi, clavier).
- **iPhone, iPad** : « Disposition du clavier » et « Clavier Apple ISO » dans Réglages de l'app › Clavier.
- **Apple TV, Android, téléviseurs** : rien de nouveau hors des textes communs.

## Vérifié, et comment

- Chaque fait écrit relu dans le code : scripts de construction (`KYBER_PYROWAVE`, options), `console.rs` (PyroWave lu comme objet), `host.rs` de Windows (`lowered`), `camera.rs` et catalogues du cœur, `SessionChrome.swift` (ordre de la barre, infobulle), `AudioPlayerCore.swift` (`voiceChat`), `server/web/src/App.svelte` (huit espaces), `server/relay/src/pages.rs`, `server/macos/menubar/Strings.swift`, `session.rs` (`request_keyframe`, `repair`), correctif 0060 et kyavserver (niveaux de journal), `release-fichiers.txt` et la page de la 0.11.7 sur GitHub (paquets de la Station Linux publiés).
- **TRANS-49** : `git grep` des noms de chaque fichier du logo v1 hors de `brand/` (seuls `brand/README` et `brand/preview.html` les citaient) et de `brand/` (tous les scripts lisent `brand/v2`) ; pour chaque capture webOS, `git grep` de son nom : seuls `src/preview.rs` des apps webOS et Linux nomment ces fichiers, qu'ils écrivent dans `apps/target`. Les huit captures citées par le README de webOS restent.
- Liens relatifs de tous les `.md` touchés : aucun cassé ; paires FR/EN : même nombre de titres ; `link-ui.md` et `.en.md` : 1 105 lignes chacun.
- Site : blocs JSON-LD relus par `json.loads`, balises équilibrées sur les six pages ; accueil et guide ouverts dans le navigateur (`site` de `.claude/launch.json`) : la carte de la Station pour Linux tient dans la grille (taille du paquet lue sur la version publiée), la section « Sous Linux » s'affiche.
- `scripts/tests/release-outils.sh` : tous les essais passent.
- Rien à essayer sur appareil pour cette zone ; les règles nouvelles s'essaieront avec la phase 2b.

## Notes de version

**FR**
- Le site présente KyberNow Station pour Linux, en bêta : téléchargement en .deb et .rpm, et un guide d'installation pas à pas, Bazzite et SteamOS compris.

**EN**
- The website now presents KyberNow Station for Linux, in beta: .deb and .rpm downloads, and a step-by-step installation guide, Bazzite and SteamOS included.
