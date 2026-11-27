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
- **WEB-14** : un seul jeton de streaming par page, réutilisé par la rubrique Station et la session tant qu'il lui reste plus d'une heure (`link/ticket.ts`).
- **WEB-15** : code de prêt gardé en empreinte seulement, effacé après 15 min ; comptes d'amis inutilisés depuis 90 jours oubliés ; phrase sous « Retenir le mot de passe ».
- **WEB-17** : sonde PyroWave et décodeur chargés à la demande (`import()`), face à une Station qui le propose, pour l'hôte seulement ; mesures dans `pyrowave/stats.ts`.
- **WEB-19** : `poll()` commun (`lib/poll.ts`) : un tour à la fois, en pause onglet masqué, relevé au retour. Pris par l'état, Vue d'ensemble, Appareils, Image et son, Prêt, Enregistrements, Compte, Système.
- **WEB-20** : un seul `send()` (`lib/http.ts`) pour la console et le client web, `keepalive` compris (fin de session, sortie du partage, fin de prêt, déconnexion).
- **WEB-23** : 22 textes inutilisés retirés ; `i18n-check.mjs` signale désormais tout texte inutilisé.
- **WEB-24** : `controlAsCommand` et la branche `stop` du Worker retirés ; exportations internes rendues locales ; sonde du micro seulement quand le moteur l'envoie.
- **TRANS-30** : « Souris des jeux », « Souris du bureau », « Rétablir le son ».

## Constats laissés

- **WEBOS-13, 16, 17, 18, 19, 20, 21, 22, 24, 25, 26, 27, 29** : hors des priorités de la vague 1 (latence d'interface, code mort à risque, factorisation, construction). WEBOS-26 et 27 relèvent de la vague 2 (vers le cœur).
- **WEBOS-01, copie Linux** (`apps/linux/src/model.rs`) : zone d'une autre session.
- **WEB-09, 10, 11, 12, 13, 16, 18, 21, 22, 27** : hors des priorités. WEB-21 (découpage de `Session.svelte`) et WEB-22 relèvent de la vague 2.
- **WEB-14, vraie cause** : Admin n'expose le compte de streaming que par `/api/web/ticket`, qui signe un jeton de 12 h. Le correctif ici réduit à un jeton par page ; le compte devrait venir de `/api/session` ou `/api/stream/info` (`server/admin`, hors zone). Le client web doit rester compatible avec une Station 0.11.7 : il ne peut pas encore compter sur ce champ.
- **WEB-25, 26** : `docs/link-ui.md` (fichier commun) et `server/README.md` (hors zone).
- **TRANS-30, reste** : le message « Le code a six chiffres, et le prénom ne peut pas être vide. » et les apostrophes typographiques du catalogue (vague 2, traductions).
- **TRANS-34, reste** : la fonction commune `short_fingerprint` du cœur (vague 2) ; Android est dans une autre zone.
- **WEB-07, reste** : les toasts d'erreur des actions (clic) restent affichés, en français ou en anglais désormais.

## Vérifié, et comment

- `apps/webos` : `cargo test --no-default-features` dans mon arbre `webos` (`third_party/arbres-fix-webos-web`) : 101 essais passés, dont les nouveaux `model::tests` (fichier tronqué mis de côté, fichier d'une version plus récente, fichier absent), `av_clock::tests`, `app::tests::back_cancels_a_pending_borrow`, `ok_on_a_focus_not_on_screen`, `address` (IPv6), `ui::settings` (empreinte).
- Compilation croisée `armv7-unknown-linux-gnueabi` (`apps/webos/scripts/build.sh --check`) dans le même arbre : passe, sans avertissement nouveau.
- `server/web` : `npm run check` (svelte-check, tsc, `i18n-check.mjs` avec la détection des textes inutilisés : 1 373 textes alignés), `npm run build` (le décodeur PyroWave dans son propre morceau, `live-*.js`), `npx vitest run` : 46 essais passés, dont les nouveaux `lib/poll.test.ts`, `lib/http.test.ts`, `link/ticket.test.ts`, `link/lend.test.ts`.
- Compatibilité avec une Station 0.11.7 : mêmes routes et mêmes corps ; seuls s'ajoutent l'en-tête de langue sur quelques appels `keepalive` et `x-kyber-admin` sur la déconnexion (déjà là).

## Reste à essayer sur la LG G2

- Un retour de version (`.ipk` plus ancien) : l'avis du fichier mis de côté, le fichier `.bad-…` à côté.
- 5.1 Dolby : chargement en fond (premières trames sautées, puis le son) ; « Image relancée » en Dolby (le son reste en 5.1) ; retard du son stable sur une longue session en Wi-Fi.
- Test Auto annulé puis session aussitôt : l'image reste.
- Sortie de veille du téléviseur en session et sur l'accueil (WEBOS-14).
- Vibrations et combinaison du menu sans manette qui bouge (WEBOS-10).
- Retour sur l'accueil pendant « En attente de {hôte}… » : la demande disparaît chez l'hôte.
- Saisie d'une adresse IPv6 entre crochets ; empreinte en quatre groupes à l'appairage.

Côté navigateur, à essayer : une perte du GPU en session PyroWave (veille du Mac, `chrome://gpucrash`), et l'onglet Admin masqué pendant une partie (plus de requêtes).

## Notes de version

**FR**

- LG : si le fichier de l'app ne peut pas être lu (par exemple après le retour à une version plus ancienne), il est mis de côté au lieu d'être effacé, et l'app vous le dit.
- LG : le son 5.1 Dolby reste en 5.1 quand l'image se relance, démarre sans figer la session, et son retard ne s'allonge plus au fil de la partie.
- LG : les vibrations et le menu répondent plus vite ; Retour sur l'accueil annule d'abord une demande d'emprunt en attente ; l'app reprend proprement à la sortie de veille du téléviseur.
- LG : l'empreinte de l'ordinateur s'affiche comme sur vos autres appareils, et les adresses IPv6 sont acceptées.
- Navigateur : avec PyroWave, l'image reprend toute seule si la carte graphique se réinitialise.
- Admin : le champ « Votre prénom » de la page Prêt ne se vide plus pendant la saisie ; Admin vous dit quand la Station ne répond pas et réessaie ; les pages ne sollicitent plus la Station quand l'onglet est caché.
- Navigateur : le code d'un prêt n'est plus gardé en clair, et les comptes d'amis inutilisés depuis 90 jours sont oubliés.

**EN**

- LG: if the app's file cannot be read (for example after going back to an older version), it is set aside instead of being erased, and the app tells you.
- LG: 5.1 Dolby sound stays in 5.1 when the picture restarts, starts without freezing the session, and its delay no longer grows during play.
- LG: vibrations and the menu respond faster; Back on the home screen first cancels a pending borrow request; the app resumes cleanly when the TV wakes up.
- LG: the computer's fingerprint is shown as on your other devices, and IPv6 addresses are accepted.
- Browser: with PyroWave, the picture comes back on its own if the graphics card resets.
- Admin: the “Your name” field on the Lend page no longer clears while you type; Admin tells you when the Station is not responding and tries again; pages no longer poll the Station while the tab is hidden.
- Browser: a loan code is no longer kept in clear text, and friend accounts unused for 90 days are forgotten.
