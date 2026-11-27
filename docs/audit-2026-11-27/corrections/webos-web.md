# Corrections de la vague 1 : app LG webOS et client web

Zone : `apps/webos` (hors `kyber-patches` et `sdl-patches`) et `server/web` (hors `node_modules` et `dist`). Branche `claude/fix-webos-web-91bb4c`, partie de `main` à `9803883a`. Rapport d'origine : [`../webos-web.md`](../webos-web.md).

## Constats corrigés

### App LG webOS

- **WEBOS-01** : un `kybernow.json` illisible (tronqué, écrit par une version plus récente) n'est plus réécrit vide. Il est renommé `kybernow.json.bad-<secondes Unix>`, l'app repart d'un fichier neuf et le dit en avis ; s'il ne peut pas être renommé, l'app n'écrit rien. Un profil inconnu se lit comme Jeu au lieu de rendre tout le fichier illisible.
- **WEBOS-02** : jetons de chargement. NDL a un propriétaire par fil d'image (`ndl::owner`, `ensure(owner, …)`, `unload_if`) ; la chaîne Dolby rend un jeton (`esplayer::load` → `unload_if`). La fin d'un flux ne décharge plus celui du flux suivant ; le fil d'image qui se termine met la chaîne Dolby en attente (`esplayer::suspend`), rechargée à la trame suivante si le son continue.
- **WEBOS-03** : `LOAD_COMPLETED` baissé avant chacun des déchargements demandés par l'app : plus de faux « déchargé par le téléviseur ».
- **WEBOS-04** : son Dolby et 5.1 PCM horodatés d'après la durée des trames, recalés sur l'horloge, une trame avec plus de 100 ms d'avance jetée (`src/av_clock.rs`) : le retard ne s'accumule plus.
- **WEBOS-05** : chargement de la chaîne Dolby sur un fil à part ; les trames d'ici là sont sautées, un échec repasse par SDL à la trame suivante.
- **WEBOS-06** : plan Opus stéréo forcé pour les mires et les parties PlayStation.
- **WEBOS-07** : un rechargement Dolby périmé ne baisse plus `RELOADING` (seulement si son époque est la courante).
- **WEBOS-08** : verrou NDL relâché entre deux salves d'amorçage.
- **WEBOS-09** : direction tenue au stick oubliée quand les entrées partent au jeu.
- **WEBOS-10** : `Shared::send` réveille la boucle par un événement SDL (`SDL_RegisterEvents`, `SDL_PushEvent`) : vibrations, combinaison du menu, état et pointeur sans les 50 ms d'attente.
- **WEBOS-11** : Retour sur l'accueil annule d'abord un emprunt en préparation ou en attente ; à la sortie de l'app, les demandes en attente (emprunt, place d'invité) sont retirées, 2 s au plus.
- **WEBOS-12** : OK sur un focus absent de l'écran dessiné le ramène au premier élément ; la confirmation « Forcer » fermée hors des Réglages rend le focus de l'écran ; une confirmation arrivée en jeu reçoit OK et les flèches.
- **WEBOS-14** : retour au premier plan (`SDL_APP_DIDENTERFOREGROUND`) : sondage d'Admin, réseau et comptes relus tout de suite ; en session, décodeur et chaîne Dolby rechargés à l'image clé suivante.
- **WEBOS-15** : le test Auto ne décharge que le décodeur qu'il tient (propriétaire à lui, plus de `ndl::unload` final inconditionnel).
- **WEBOS-13** : la réponse tardive de « Par adresse » n'ouvre plus Ajouter hors de l'écran qui l'attend (`App::probed`).
- **WEBOS-16** : une manette déjà ouverte, ou une cinquième, est refermée.
- **WEBOS-17** : l'aperçu d'un ordinateur retiré est effacé ; un sondage ou un aperçu arrivé après le retrait ne recrée rien.
- **WEBOS-18** : taille de trame lue sur l'encodeur ouvert (`kn_eac3_frame_size(e)`) ; erreurs de l'encodeur distinguées (-1 à -4), comptées et journalisées (la première, puis toutes les 1000).
- **WEBOS-19** : `kybernow.log` tourné au-delà de 5 Mo.
- **WEBOS-20** : Rejoindre, Compte, Manettes et l'attente du code redessinés toutes les 250 ms au lieu de 60 fois par seconde ; QR code gardé ; glyphes empruntés au cache ; offres des Stations calculées une fois par seconde ou après un événement ; la Magic Remote ne redessine qu'au changement de focus.
- **WEBOS-21** : SPS et SEI seuls désémulés ; métadonnées HDR lues une fois par mire.
- **WEBOS-22** : tampons du chemin Dolby réutilisés (PCM remis dans l'ordre, trames E-AC-3), compteurs atomiques.
- **WEBOS-24** : crochet `Feed` et réécriture retirés ; seule la journalisation de `Load` reste (README FR et EN).
- **WEBOS-25** : plus de `allow(dead_code)` dans `sdl.rs` ; 14 déclarations inutilisées, `SignInFail::Cancelled`, `PairAsked::address` et `let _ = (w, h, codec)` retirés. La partie téléviseur compile sans aucun avertissement venant de webOS.
- **WEBOS-29 et TRANS-21 (webOS)** : chaîne figée sur `2026.08-webos` ; archives téléchargées dans un fichier, extraites à part puis mises en place ; FFmpeg vérifié par SHA-256 et compilé en `--cpu=armv7-a` ; relogement interrompu refait (marqueur) sans toucher une chaîne déjà en place ; dossiers temporaires retirés par `trap`. README FR et EN.
- **WEBOS-23** : le générateur de captures (`preview.rs`) n'est plus dans le binaire du téléviseur ; `open_link`, `adjust_for_preview`, `encode_png` suivent.
- **WEBOS-28** : README FR et EN corrigés (horodatage, ~5 s au pire, jetons).
- **TRANS-35 (copie LG)** : `parse_address` juste en IPv6 (`[fe80::1]:24811` ; `fe80::1` sans port n'est plus l'hôte `fe80:` au port 1).
- **TRANS-34 (LG)** : empreinte en quatre groupes de quatre (`7C41 E09B AABB CCDD`), comme les autres apps et Admin, dans Réglages et à l'appairage.

### Client web et console

- **WEB-01** : la perte de la carte graphique en session PyroWave relance l'image avec un avis (« La carte graphique s'est réinitialisée : l'image reprend… ») une fois le nouveau device prêt (2 s au plus) ; le device est remplacé en fond, et les sessions suivantes reprennent PyroWave au lieu de tomber en H.264.
- **WEB-02** : `s.done` géré (`catch` qui ferme le `ImageBitmap`) ; `rb.destroy()` en `finally` dans les mesures.
- **WEB-03** : « Votre prénom » ne se vide plus (chargement de la page Prêt une fois, `untrack`).
- **WEB-04** : réponses périmées jetées après une écriture locale (`Generation` : état, relais, enregistrements) ; journal : une lecture à la fois, réponse d'un fichier quitté jetée.
- **WEB-05** : guet de mise à jour arrêté au démontage de Système.
- **WEB-06** : Admin injoignable au chargement : « La Station ne répond pas. Nouvel essai dans un instant… », nouvel essai espacé (2 à 15 s).
- **WEB-07** : erreur réseau traduite (`NetworkError`, `common.unreachable`) ; relevés de Compte silencieux sur une erreur réseau.
- **WEB-08** : effets des pages en `untrack` ; `langHeaders` lit la langue sans la suivre.
- **WEB-07, suite** : toute erreur du navigateur (`TypeError`, délai, annulation) est dite « La Station ne répond pas… » dans les toasts ; le même avis n'est pas reposé.
- **WEB-09** : appui long et glisser en cours arrêtés à `TouchInput.destroy()`.
- **WEB-10** : démontage de la session complet (bandeau PyroWave, relance prévue par `onPipeline`, sonde du son).
- **WEB-11** : un signal de vie à la fois, abandonné au bout d'une période (Worker et repli de la page).
- **WEB-12** : accueil du client web : réponses arrivées après le démontage jetées (aperçu `blob:` révoqué), relevés par `poll()`.
- **WEB-13** : micro arrêté pendant la demande d'autorisation : `cancelled`, le choix retenu pour l'ordinateur reste.
- **WEB-14** : compte de streaming lu dans `stream_user` (`/api/stream/info`, puis `/api/session`) sans signer de jeton ; repli pour une Station 0.11.7 : un seul jeton par page, réutilisé par la rubrique Station et la session tant qu'il lui reste plus d'une heure (`link/ticket.ts`).
- **WEB-16** : onglets du téléphone qui rétrécissent (ellipse), icônes seules sous 480 px avec le libellé lu par les lecteurs d'écran.
- **WEB-18** : paquet de position de 9 octets réutilisé, rectangle de la toile gardé (`ResizeObserver`, redimensionnement, défilement), métriques réactives seulement barre ou statistiques ouvertes, médianes bornées à 6 h.
- **WEB-27** : plus d'effet de bord dans le `$derived` des invitations, relecture qui ne se recrée plus ; version de Kyber en un seul endroit (`KYBER_VERSION`, ou `kyber_version` si Admin la donne).
- **WEB-15** : code de prêt gardé en empreinte seulement, effacé après 15 min ; comptes d'amis inutilisés depuis 90 jours oubliés ; phrase sous « Retenir le mot de passe ».
- **WEB-17** : sonde PyroWave et décodeur chargés à la demande (`import()`), face à une Station qui le propose, pour l'hôte seulement ; mesures dans `pyrowave/stats.ts`.
- **WEB-19** : `poll()` commun (`lib/poll.ts`) : un tour à la fois, en pause onglet masqué, relevé au retour. Pris par l'état, Vue d'ensemble, Appareils, Image et son, Prêt, Enregistrements, Compte, Système.
- **WEB-20** : un seul `send()` (`lib/http.ts`) pour la console et le client web, `keepalive` compris (fin de session, sortie du partage, fin de prêt, déconnexion).
- **WEB-23** : 22 textes inutilisés retirés ; `i18n-check.mjs` signale désormais tout texte inutilisé.
- **WEB-24** : `controlAsCommand` et la branche `stop` du Worker retirés ; exportations internes rendues locales ; sonde du micro seulement quand le moteur l'envoie.
- **TRANS-30** : « Souris des jeux », « Souris du bureau », « Rétablir le son ».

## Constats laissés

Seule la vague 2 reste :

- **WEBOS-26, WEBOS-27** : interface Rust partagée avec `apps/linux`, découpage NAL et `sym<T>` dans le cœur.
- **WEB-21, WEB-22** : découpage de `Session.svelte`, petits doublons (presse-papiers, listes).
- **WEB-25, `docs/link-ui.md`** et **TRANS-30, reste** (« Le code a six chiffres, et le prénom… », apostrophes typographiques du catalogue) ; **TRANS-34, reste** (`short_fingerprint` du cœur).

Hors de ma zone, signalé : la copie Linux de WEBOS-01 (`apps/linux/src/model.rs`) ; **WEB-26** (`server/README.md`) ; pour WEB-14, Admin doit donner `stream_user` (la valeur de `web_user`) dans `/api/stream/info` ou `/api/session` (note dans le dossier partagé, `webos-web-stream-user.md`) ; pour WEB-27, `kyber_version` dans `/api/status` si Admin veut la donner. Le `-C target-cpu=cortex-a73` de Rust (`scripts/build.sh`) n'a pas été touché : il change toute la compilation du cœur et se mesure sur un téléviseur.

## Vérifié, et comment

- `apps/webos` : `cargo test --no-default-features` dans mon arbre `webos` (`third_party/arbres-fix-webos-web`) : 102 essais passés, dont les nouveaux `app::tests::late_probe_ignored`, `model::tests` (fichier tronqué mis de côté, fichier d'une version plus récente, fichier absent), `av_clock::tests`, `app::tests::back_cancels_a_pending_borrow`, `ok_on_a_focus_not_on_screen`, `address` (IPv6), `ui::settings` (empreinte).
- Compilation croisée `armv7-unknown-linux-gnueabi` (`apps/webos/scripts/build.sh --check`) dans le même arbre : passe, sans aucun avertissement venant de webOS. FFmpeg 7.1.1 configuré et compilé en `--cpu=armv7-a` avec la chaîne du Mac ; `fetch_extract` essayé : bonne somme acceptée, mauvaise refusée, dossier temporaire retiré. Le téléchargement de la chaîne `2026.08-webos` n'a pas été refait (chaîne du Mac déjà en place).
- `server/web` : `npm run check` (svelte-check, tsc, `i18n-check.mjs` avec la détection des textes inutilisés : 1 373 textes alignés), `npm run build` (le décodeur PyroWave dans son propre morceau, `live-*.js`), `npx vitest run` : 49 essais passés, dont les nouveaux `lib/poll.test.ts`, `lib/http.test.ts`, `lib/store.test.ts`, `link/ticket.test.ts`, `link/lend.test.ts`.
- Compatibilité avec une Station 0.11.7 : mêmes routes et mêmes corps ; seuls s'ajoutent l'en-tête de langue sur quelques appels `keepalive` et `x-kyber-admin` sur la déconnexion (déjà là).

## Reste à essayer sur la LG G2

- Un retour de version (`.ipk` plus ancien) : l'avis du fichier mis de côté, le fichier `.bad-…` à côté.
- 5.1 Dolby : chargement en fond (premières trames sautées, puis le son) ; « Image relancée » en Dolby (le son reste en 5.1) ; retard du son stable sur une longue session en Wi-Fi.
- Test Auto annulé puis session aussitôt : l'image reste.
- Sortie de veille du téléviseur en session et sur l'accueil (WEBOS-14).
- Vibrations et combinaison du menu sans manette qui bouge (WEBOS-10).
- Retour sur l'accueil pendant « En attente de {hôte}… » : la demande disparaît chez l'hôte.
- Saisie d'une adresse IPv6 entre crochets ; empreinte en quatre groupes à l'appairage.
- Curseur de Rejoindre et minute du code à 4 dessins par seconde : clignotement correct ; Magic Remote sur les menus.
- 5.1 Dolby avec l'encodeur relu (taille de trame), FFmpeg en `armv7-a` (le son reste juste), journal tourné après des jours d'app ouverte.
- Une construction neuve sur Linux x86_64 avec la chaîne `2026.08-webos`.

Côté navigateur, à essayer : une perte du GPU en session PyroWave (veille du Mac, `chrome://gpucrash`), l'onglet Admin masqué pendant une partie (plus de requêtes), la barre d'onglets d'Admin à 375 px, le toucher (appui long pendant la fin de session).

## Notes de version

**FR**

- LG : si le fichier de l'app ne peut pas être lu (par exemple après le retour à une version plus ancienne), il est mis de côté au lieu d'être effacé, et l'app vous le dit.
- LG : le son 5.1 Dolby reste en 5.1 quand l'image se relance, démarre sans figer la session, et son retard ne s'allonge plus au fil de la partie.
- LG : les vibrations et le menu répondent plus vite ; Retour sur l'accueil annule d'abord une demande d'emprunt en attente ; l'app reprend proprement à la sortie de veille du téléviseur.
- LG : l'empreinte de l'ordinateur s'affiche comme sur vos autres appareils, et les adresses IPv6 sont acceptées.
- Navigateur : avec PyroWave, l'image reprend toute seule si la carte graphique se réinitialise.
- Admin : le champ « Votre prénom » de la page Prêt ne se vide plus pendant la saisie ; Admin vous dit quand la Station ne répond pas et réessaie ; les pages ne sollicitent plus la Station quand l'onglet est caché.
- Navigateur : le code d'un prêt n'est plus gardé en clair, et les comptes d'amis inutilisés depuis 90 jours sont oubliés.
- LG : l'app consomme moins quand elle attend (écrans redessinés à leur rythme), et son journal ne grossit plus sans fin.
- Admin : la barre d'onglets tient sur un petit téléphone, et les messages d'erreur sont toujours dans votre langue.

**EN**

- LG: if the app's file cannot be read (for example after going back to an older version), it is set aside instead of being erased, and the app tells you.
- LG: 5.1 Dolby sound stays in 5.1 when the picture restarts, starts without freezing the session, and its delay no longer grows during play.
- LG: vibrations and the menu respond faster; Back on the home screen first cancels a pending borrow request; the app resumes cleanly when the TV wakes up.
- LG: the computer's fingerprint is shown as on your other devices, and IPv6 addresses are accepted.
- Browser: with PyroWave, the picture comes back on its own if the graphics card resets.
- Admin: the “Your name” field on the Lend page no longer clears while you type; Admin tells you when the Station is not responding and tries again; pages no longer poll the Station while the tab is hidden.
- Browser: a loan code is no longer kept in clear text, and friend accounts unused for 90 days are forgotten.
- LG: the app uses less while it waits (screens redrawn at their own pace), and its log no longer grows without end.
- Admin: the tab bar fits on a small phone, and error messages are always in your language.
