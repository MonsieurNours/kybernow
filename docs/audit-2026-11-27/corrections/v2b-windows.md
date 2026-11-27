# Vague 2 (phase 2b) — App Windows et son installateur

Branche `claude/v2b-windows-f85d84`, d'après [vague-2.md](../vague-2.md) et le guide [migration-coeur-2b.md](../migration-coeur-2b.md), section Windows. Zone : `apps/windows` en entier, installateur `setup/` compris (et les lignes Windows de `docs/hdr*` et `docs/pyrowave-client-windows*`). Règles de link-ui : celles de la branche `claude/v2-docs-0db24c`. Rien poussé, rien fusionné. **Rien essayé sur Windows** (aucun PC).

## Fait

### Migration vers le cœur

| Id | Correctif |
|---|---|
| WINAPP-49 | `src/hdr.rs` sur `kybernow_core::rules::hdr` : PQ, `Output`, `frame_is_pq`, `content_peak`, `yuv_matrix`, `tone`, `hdr10_for` du cœur ; luminosités par `HdrLuminance::from_display`. L'app garde DXGI (`outputs`, `sdr_whites`, `sdr_white_nits`) et le blanc du trait du bord haut. Le shader passe dans `src/render.hlsl` (inclus par `render.rs`) avec l'EETF de BT.2390 **et son relèvement du noir** (`dst_lo_pq`, nouveau dans le cbuffer) et la courbe vers le SDR **de la Station** (pente 3,6923077 au lieu du Reinhard étendu). Essai croisé `shader_comme_le_coeur` : lignes du shader, constantes PQ, matrice BT.2020 → BT.709, valeurs à 1e-4 sur une grille contre `rules::hdr`. |
| WINAPP-50 | `src/pyrowave_config.rs` retiré : `kybernow_core::pyrowave::Config` (version 1, et tête de la version 2). |
| KYB-29 | L'app demande l'en-tête `KNPW` version 2 (`kyclient::set_pyrowave_header(2)` au lancement, avec `kyber-pyrowave` ; `kyclient` ajouté en dépendance facultative) et en lit les métadonnées HDR10 (`hdr::pyrowave_hdr` : drapeaux, luminances du mastering, MaxCLL, MaxFALL ; 48 octets au moins, octets en trop ignorés) ; `draw_pyrowave` les donne au rendu (crête du flux). Version 1 toujours lue (essais `knpw_version_1_toujours_lu`, `knpw_version_2_metadonnees_hdr`). |
| WINAPP-51 | Fil de l'image (`src/video.rs`) sur `frames::FrameFlow` : `Open`, `Configure`, `Decode`, `Skip`, `Lost` (`keyframe_lost`, ou la console), `Keyframe` (`request_keyframe`), `Recovered` ; `fed`, `failed`, `unavailable`, `reset` après une carte perdue ou un passage au processeur, `dropped` quand la file déborde, `idle` toutes les 100 ms, `set_wanted` à chaque image. Disparus : `waiting_key`, `send_config`, `codec_announced`, `last_wanted`, la fermeture `keyframe`, le `ResumeGate` de l'app. |
| WINAPP-11 | Décodeur PyroWave qui ne s'ouvre pas : échec retenu (aucun nouvel essai avant un autre format), images écartées (`unavailable`), `Session::pyrowave_failed()` ; l'avis « Image en HEVC : PyroWave n'a pas pu démarrer sur cet appareil. » par `Platform::on_pyrowave_fallback` (le cœur ne l'appelle qu'une fois). |
| WINAPP-52, TRANS-42 | `src/plan.rs` retiré : `kybernow_core::plan` (ligne de commande, accueil, session, réglages synchronisés). `displays_info` rend `plan` (Personnalisé, valeurs du profil, mode, débit de la session, nom) ; l'interface ne calcule plus rien (`isCustom`, `profileBase`, `fit`, `sessionPlan`, `PYROWAVE_MBPS*`, `MAX_SESSIONS`, `DEFAULT_RELAY` retirés). Commande `common_values` (`plan::values_json`), lue une fois par fenêtre (`values` dans `api.ts`) : nombre de sessions, attente du lien du relais (`relay.svelte.ts`), port d'Admin (fenêtre d'ajout), relais public. |
| bornes | Curseur du débit : de 5 à 500 Mbit/s par pas de 5, jusqu'à 2 500 en PyroWave (`manual_mbps` de `values_json`, au lieu de 500 et 1 500 en dur). |
| TRANS-40 | Fenêtres toutes réduites : `Session::set_background(…, Device::Computer)` (20 Mbit/s, image dite figée) à la place de `HIDDEN_BITRATE` et `set_bitrate_cap`. |
| WINAPP-53 | `src/discovery.rs` sur `discovery::found` (l'app garde mdns-sd) ; « déjà à l'accueil » décidé en Rust (`fresh_found` : même adresse, ou même empreinte d'Admin par `same_fingerprint`), plus en TypeScript (`sameFp` retiré de l'accueil et du premier lancement). |
| WINAPP-54 | `consoles.rs` sur `playstation::bundle` : `Bundle::parse` à la réception, `to_json` à l'envoi, `match_accounts` pour les comptes, `BundleStation::matches` pour les Stations, `new_account_id`, `valid_psn_id` ; les types et la lecture tolérante de l'app retirés. |
| WINAPP-55 | `synced.rs` : `AccountSync` du cœur (même `synced.json`, essai `fichier_comme_avant`), `settings::changed` (l'app ajoute le HDR et le mode immersif), `overlay` retiré. |
| WINAPP-56 | Puce « Micro » sur `micro::MicUi` (`ui_json` : état, textes, infobulle ; `tap`, `set`, `denied`, `changed` et son avis) ; `MicSlot`, `BarMic` et `state_code` retirés, textes du micro retirés du catalogue de l'interface (raccourci dans le catalogue Rust). |
| WINAPP-62 | `displays::session_hdr` retiré (`Screens::session_hdr`) ; `primary` sur les `Monitor` gardé là où la liste du cœur n'existe pas encore, même règle que `Screens::primary` (vérifié par l'essai). |
| TRANS-34 | Empreinte courte donnée par Rust (`fingerprint_short` des ordinateurs et du premier contact, `console::short_fingerprint`) ; `fingerprintGroups` retiré ; `same_fingerprint` de `accounts.rs` et la comparaison d'`inviting.rs` remplacées par celle du cœur. |
| TRANS-35 | `split_host` (ligne de commande) et le découpage de `parse_link_text` remplacés par `console::parse_address` ; `ADMIN_PORT` au lieu de 9443 ; clé `errors.bad_port` devenue inutile, retirée. |
| autres copies | Adresse d'Admin des Statistiques par `host_for_url` (donnée par Rust) ; code PlayStation montré « 482 913 » par `share::format_code` (`psGroupCode` retiré) ; libellé du relais (`relay_label`, plus `split('#')`) ; « Définition ramenée » par `Lowered::text` (clé `pyrowave.lowered` retirée) ; horloge de l'enregistrement par `recording::clock` (commande `recording_clock`, `recordingClock` retiré) ; lien d'invitation par `lend::looks_like_invite` (`link::is_borrow` retiré : le lien web d'invitation est reconnu, illisible il se dit illisible). |
| ffi | `ffi_guard!` autour des procédures de fenêtre (session, contrôle, presse-papiers, icône, console PlayStation et ses aides), du crochet clavier, des rappels de FFmpeg (`get_format`, verrou D3D11VA) et de l'énumération des écrans ; `ffi::guard` autour du distributeur des commandes de l'interface (les commandes synchrones tournent sur le fil de WebView2). |

### Installateur

| Id | Correctif |
|---|---|
| REL-09 | PKCE S256 dans `setup/src/account.rs` : vérificateur de 32 octets en base64url, défi dans l'adresse de la page du relais, `code_verifier` dans `POST /v1/devices/token`, gardé avec l'`etat` dans `bin/ui.rs` ; essai `pkce` (vecteur de la RFC 7636, base64url de la RFC 4648, forme, adresse, corps). Un relais 0.11.7 ignore les deux. |

### Règles de link-ui (phase 2a)

| Règle | Fait |
|---|---|
| « Envoyer le journal » | Réglages de l'app › « À propos » (en dernier) : « Version » (« KyberNow Link 0.11.8 ») et « Envoyer le journal », « Préparation du journal… » ; `src/journal.rs` écrit `kybernow-journal.txt` (appareil, Windows, version, fin du lancement précédent quand son journal dit un plantage ou une panique, puis les deux journaux) et ouvre la fenêtre « Partager » de Windows (`IDataTransferManagerInterop`, fichier passé par `AgileReference`). La ligne de version du haut et le champ `arch` d'`app_info` sont retirés. |
| Caméra | Sans réponse de l'ordinateur 10 s après l'allumage : éteinte, « L'ordinateur ne répond pas : la caméra est éteinte. » ; session sans lien caméra : « La caméra n'a pas pu démarrer sur l'ordinateur. » ; refus de l'ordinateur : textes du cœur (`event_notice`). TRANS-39 : accès refusé par Windows, l'avis se choisit (« Ouvrir les Réglages », `ms-settings:privacy-webcam`), la notification y mène aussi. |
| Touches | Dix entrées dans l'ordre de link-ui avec leur indication (⊞ + D, ⊞ + L, Alt + F4, Échap, Gestionnaire des tâches, F11 ajoutés, côté barre et côté fenêtre). |
| Infobulles | « Profil : Jeu, Bureau, Max ou Auto » ; libellés accessibles « Souris : … » et « Qualité de l'image : … » traduits (plus de « : » écrit dans le code). |
| Bornes du débit | Voir plus haut. |
| Menu « Plus » | Barre étroite qui ne tient toujours pas : « Ordinateurs », « Touches », la souris, « Garder le son », le micro, la caméra, « Presse-papiers », « Plein écran » passent un à un dans « Plus » (trois points, « Plus de commandes »), à la place de la première retirée ; même menu ou même bascule. Essayé dans l'imitation (`bar.html?largeur=520`). |
| Avertissement du prêt | Selon le système de la Station (`system` de la barre) : « votre session Windows », « du Mac », ou « votre session ». |
| TRANS-38 | Statistiques : les cinq lignes du bas toujours, « — » quand une mesure manque (le reste y était déjà). |
| TRANS-39 | Erreur sous le nom à l'accueil : « Mot de passe refusé : mettez-le à jour dans les réglages. » (401, 403), « La Station a été réinstallée : vérifiez son empreinte dans les réglages. » (certificat changé) ; l'ordinateur compte comme en ligne, comme sur Mac. |
| Réglage du HDR | Absent de l'app Windows (link-ui) : les README disent que l'outil d'étalonnage de Windows en tient lieu. |
| Image qui n'est plus regardée, repli de PyroWave | Voir TRANS-40 et WINAPP-11. |

### Autres

- **`build.sh`** : le commentaire « PyroWave seulement avec --kyber-pyrowave » avait déjà été corrigé en phase 2a (TRANS-22) ; il dit maintenant aussi la demande de l'en-tête v2.
- **TRANS-29** : les clés mortes de l'app (`home.welcome_title`, `home.welcome_lead`, `camera.indicator`, `errors.video_thread`) étaient déjà retirées (vague 1) ; `app.title` de l'installateur est employée (`document.title`), gardée.
- **TRANS-31** : « Revenir à l'accueil » (forme de link-ui) ; `relay.example.org:9443` en anglais (app et installateur, où l'exemple était écrit en dur). « Avec un code » gardé (pas tranché dans link-ui, le même que Linux).
- **TRANS-32** : espaces fines avant « : ; ! ? » dans les catalogues français (36 dans l'interface, 7 côté Rust, 7 dans l'imitation, 11 dans l'installateur) ; `check-i18n.mjs` de l'app et de l'installateur refuse désormais une espace simple ou insécable à cet endroit.

## Laissé, et pourquoi

- **Capsule « ● Caméra » barre fermée et signe du micro près de l'indicateur de la session** : la barre de Windows est une fenêtre à part, cachée barre fermée ; il faut une petite fenêtre transparente, sans clic, sur chaque écran de la session (comme les aides des parties PlayStation). Pas faite à l'aveugle sans PC pour la régler. À confier.
- **Le cœur n'a pas tout** (figé pour la 2b) :
  - textes « L'ordinateur ne répond pas : la caméra est éteinte. » et « La caméra n'a pas pu démarrer sur l'ordinateur. » : dans le catalogue de l'app, faute de clés dans celui du cœur (`camera.no_answer`, `camera.start_refused` à y ajouter) ;
  - métadonnées HDR10 du paquet `KNPW` v2 : lues par l'app (`hdr::pyrowave_hdr`), `pyrowave::Config` n'en dit rien (`Config::hdr10()` à ajouter) ;
  - `kyclient::set_pyrowave_header(2)` appelé par l'app : le cœur devrait le faire pour toutes les apps qui lisent la v2 ;
  - système de la Station : seul `keymap::station_is_mac` existe ; « Windows » se reconnaît dans l'app (`station_system`) ;
  - erreur sous le nom (401, certificat changé) : décidée par l'app d'après `ConsoleError`, comme le Mac.
- **Écarts de docs hors zone** : `docs/link-ui.en.md` montre encore `relais.exemple.org:9443` en anglais, et « recognizes » (américain) ; l'agent de l'installateur a relevé des mots de l'étape « Votre compte KyberNow » qui diffèrent de link-ui (« Se connecter » au lieu de « Connecter un compte », « Autre relais… » au lieu de « Un autre relais… », « Adresse du relais » au lieu de « Relais ») : à trancher dans link-ui avant de les changer.
- **Gardés en TypeScript, permis** : le groupement en direct de la saisie du code (`JoinSheet`, même règle « trois et trois ») ; le compte à rebours du préavis du prêt (pas dans le guide). L'imitation de navigateur (`mock.ts`, jamais livrée) imite le plan, l'empreinte et l'horloge pour travailler l'interface.
- **Filtre des plantages** (`on_crash`) sans `ffi_guard!` : il n'alloue rien et ne panique pas, la garde du cœur alloue pour son journal.

## Vérifié, et comment

- `npm --prefix apps/windows/ui run check` et `npm --prefix apps/windows/setup/ui run check` : sans erreur (catalogues alignés, clés employées, espaces fines ; svelte-check, tsc).
- `cargo test -p kybernow-windows --features kyber-pyrowave` sur le Mac (`PKG_CONFIG_PATH` vers `rootfs-arm64-macos` de l'arbre) : 113 passent (HDR et shader, `KNPW` v1 et v2, journal, réglages synchronisés, écrans, consoles, liens…). `cargo test` de l'installateur : 15 passent (PKCE compris), un ignoré comme avant.
- `cargo check --target x86_64-pc-windows-gnu --features kyber-pyrowave` après chaque changement (environnement de `build.sh`, sources de Kyber de mon arbre) ; une erreur ajoutée exprès est bien vue.
- `link-check.sh` x64 et ARM64 : édition de liens vérifiée.
- Construction complète de `kybernow.exe` (x64, release, `build.sh` au commit 701ba9d8) dans `third_party/arbres-v2b-windows/windows-app` : réussie, sans avertissement de l'app (exécutable de 69 Mo, `kynput.dll`, `libpyrowave-shared-1.dll`, archive). La première, interrompue par le redémarrage du Mac, a été refaite depuis un arbre remis au dernier commit ; l'arbre est supprimé ensuite.
- Interface : menu « Plus » et « Touches » ouverts dans l'imitation (`npm run dev`, `bar.html?largeur=520`).

## À essayer sur un PC

PyroWave (décodeur qui ne s'ouvre pas : avis, image en HEVC ; en-tête v2 face à une Station 0.11.8, crête dans le journal), HDR (courbe vers le SDR et relèvement du noir sur un écran peu lumineux), fenêtre réduite (20 Mbit/s, « fenêtre cachée » dans les Statistiques), « Envoyer le journal » (fenêtre « Partager »), caméra refusée par la confidentialité (« Ouvrir les Réglages ») et sans réponse en 10 s, menu « Plus » dans une session aux deux tiers de l'écran, les dix touches, l'erreur sous le nom (mot de passe changé dans Admin, Station réinstallée), la connexion d'un compte depuis l'installateur face à un relais 0.11.8 (PKCE) et 0.11.7.

## Notes de version

**FR**
- L'app Windows suit les règles communes de toutes les apps : plan de session, courbes HDR (celles de la Station), flux d'images, découverte, réglages synchronisés et micro viennent du même cœur.
- PyroWave : si cet appareil ne peut pas le décoder, l'image repart aussitôt en HEVC, avec un avis. Avec une Station 0.11.8, l'image HDR en PyroWave tient compte de l'écran d'origine.
- Débit réglé à la main jusqu'à 500 Mbit/s, et 2 500 en PyroWave.
- « Envoyer le journal », dans les réglages de l'app : le journal part par la fenêtre « Partager » de Windows, vers qui vous voulez.
- Barre de session : un menu « Plus » quand elle est trop étroite, et dix raccourcis dans « Touches ».
- L'accueil dit quand un mot de passe est refusé ou qu'une Station a été réinstallée.
- L'installateur connecte votre compte avec PKCE : un code de retour intercepté par une autre app ne peut plus être échangé.

**EN**
- The Windows app follows the rules shared by every app: session plan, HDR curves (the Station's), picture flow, discovery, synced settings and microphone come from the same core.
- PyroWave: if this device can't decode it, the picture restarts in HEVC straight away, with a notice. With a 0.11.8 Station, an HDR picture in PyroWave takes the source screen into account.
- Bitrate set by hand up to 500 Mbit/s, and 2,500 with PyroWave.
- “Send the log”, in the app settings: the log goes through Windows' Share window, to whoever you choose.
- Session bar: a “More” menu when it's too narrow, and ten shortcuts in “Keys”.
- The home screen says when a password is refused or a Station was reinstalled.
- The installer now signs in your account with PKCE: a return code intercepted by another app can no longer be exchanged.
