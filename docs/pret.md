# KyberNow Share : prêter l'ordinateur

[English](pret.en.md) · Français

Prêter l'ordinateur, c'est le confier à un ami pendant que vous ne vous en servez pas. Votre ami joue sur votre PC depuis son app KyberNow Link ou son navigateur, chez vous ou de chez lui. Il ne voit ni votre bureau ni vos fenêtres : la Station passe votre session Windows en **mode console**, un lanceur plein écran qui ne montre que les apps que vous avez choisies. À la fin du prêt, tout revient comme avant et la session se verrouille.

Ce document est le contrat commun de la Station (Admin, icône près de l'horloge, Kyber), du cœur Rust, de toutes les apps Link et du client web. Les textes exacts sont dans [link-ui.md](link-ui.md), section « Prêt de l'ordinateur ». Le prêt fait partie de KyberNow Share ([kybernow-share.md](kybernow-share.md)) et passe par le même relais hors de chez soi ([relais.md](relais.md)).

## Ce que voit l'utilisateur

### Ce qu'on prête

- **Votre propre session Windows**, en mode console : pas de compte Windows à part, pas de second Windows. Il faut donc que la session soit **ouverte** : si elle est verrouillée, votre ami lit « L'ordinateur est verrouillé. » et le prêt ne démarre pas. Tant que le prêt est activé, la Station empêche le verrouillage automatique de la session et la mise en veille.
- **Les apps permises** : Steam (en Big Picture) et les apps que vous ajoutez dans Admin, choisies dans le même catalogue que l'app ouverte au début d'une session (menu Démarrer, suggestions, explorateur limité).
- **Le compte Steam, au choix à chaque prêt** : « Son compte Steam » (Steam se déconnecte du vôtre, votre ami se connecte avec le sien ; vos jeux par Familles Steam s'il en fait partie) ou « Votre compte Steam » (il joue avec vos jeux et vos sauvegardes). Avec « Son compte Steam », vos comptes retenus par Steam sont mis de côté pendant le prêt : votre ami ne peut pas choisir le vôtre sans mot de passe. À la fin, Steam revient sur votre compte.
- **Vos fichiers sont protégés au mieux, pas de façon étanche** : Explorateur, la touche Windows, Alt+Tab, le Gestionnaire des tâches, les invites de commande et les apps hors liste sont bloqués, les fenêtres « Ouvrir » et « Enregistrer sous » sont fermées ; mais un jeu ou une app permise peut encore lire votre disque. Admin le dit : à réserver à des amis de confiance.

### Trois façons de prêter

1. **Un code pour une séance** : « Prêter l'ordinateur… » (Admin, icône près de l'horloge, barre de session de vos apps Link). Vous choisissez la durée (« 1 heure », « 2 heures », « 4 heures », « Jusqu'à ce que je reprenne ») et le compte Steam ; la Station donne un code à six chiffres, un lien et un QR code, comme le partage. Le code sert une seule fois ; il vaut 24 heures s'il n'est pas utilisé, et « Annuler le code » l'efface. Votre ami entre avec « Rejoindre avec un code », comme pour un partage.
2. **Les comptes d'amis** : dans Admin, page « Prêt », vous créez un compte par ami (prénom, identifiant, mot de passe, compte Steam proposé). Admin donne un lien d'invitation et son QR code ; votre ami l'ouvre, son app ajoute « Ordinateur de {hôte} » à son accueil, et il emprunte avec son identifiant et son mot de passe (retenus par l'app s'il le veut). Sur votre réseau, « Ajouter un ordinateur » propose aussi « Avec un compte d'ami ».
3. **Les créneaux** : sur le compte d'un ami, des plages de la semaine (« mercredi, de 14 h à 18 h »). Pendant son créneau, votre ami emprunte sans vous demander.

### Quand votre ami arrive

- **Ordinateur libre** (ni session d'un appareil, ni partage, ni personne devant depuis 5 minutes) : le prêt démarre.
- **Ordinateur utilisé** :
  - code ou compte d'ami hors créneau : vous recevez « Léa veut emprunter l'ordinateur » (notification de l'icône, Admin, barre de votre session) avec « Accepter » et « Refuser » ; votre ami attend (« En attente de {hôte}… »). Accepter ferme votre session (ou passe le PC en mode console si vous étiez devant) ;
  - **pendant son créneau, votre ami passe avant vous** : vous êtes prévenu (« Créneau de Léa : l'ordinateur lui est prêté dans 2 min. »), puis votre session se ferme.
- Un code créé depuis votre session vaut accord : votre session se ferme quand votre ami arrive (le panneau le dit).
- Un seul prêt à la fois ; pendant un prêt, ni partage ni session d'un autre appareil.

### Pendant le prêt

- **Votre ami** a une session ordinaire à son écran (écran virtuel à la taille de son appareil, profil Jeu par défaut), avec le clavier, la souris et ses manettes, sans presse-papiers. Sa barre dit « Prêté par {hôte} », le temps restant (« Encore 1 h 20 », ou « Jusqu'à ce que {hôte} reprenne »), et « Rendre l'ordinateur ». Fermer la fenêtre ou perdre le réseau ne rend pas l'ordinateur : il peut revenir pendant 15 minutes, après quoi le prêt s'arrête.
- **Le lanceur** (sur son écran) : logo KyberNow Share, une tuile par app permise, le temps restant, « Rendre l'ordinateur ». À la manette, au clavier ou à la souris. Une app fermée ramène au lanceur.
- **L'écran de votre PC** est couvert : logo KyberNow Share, « Ordinateur prêté à Léa », l'heure de fin s'il y en a une, et « Pour reprendre l'ordinateur, appuyez sur Entrée. ». Le clavier et la souris branchés au PC ne font rien d'autre ; Entrée demande « Reprendre l'ordinateur ? Léa a 2 minutes pour sauvegarder. » (Entrée : reprendre, Échap : annuler).

### Reprendre

- **« Reprendre l'ordinateur »** : Admin, icône près de l'horloge, vos apps Link (l'accueil de cet ordinateur dit « Prêté à Léa »), l'écran du PC (Entrée). Votre ami a **2 minutes** : « {hôte} reprend l'ordinateur dans 2 min : sauvegardez votre partie. » Pendant ce temps, « Reprendre tout de suite » coupe sans attendre.
- **Fin prévue** (durée du code, fin du créneau) : le même préavis de 2 minutes, « Fin du prêt dans 2 min : sauvegardez votre partie. ».
- **Fin** : les apps du prêt se ferment, Steam revient sur votre compte, vos fenêtres reviennent, l'écran virtuel disparaît, et la session Windows **se verrouille**. Votre ami lit « {hôte} a repris l'ordinateur. », « Le prêt est terminé. » ou « Votre créneau est terminé. ».

## Échange avec Admin

Tout en JSON, sur le port d'Admin, certificat épinglé comme pour le partage. Les noms ci-dessous font foi.

**L'ami** (sans appareil appairé ; limité comme le partage : 10 essais par minute et par adresse, et les comptes bloqués 10 minutes après 10 mots de passe refusés) :

- `POST /api/lend/status` — `{"login":"lea","password":"…"}` → `{"state":"free|busy|lent|locked|off","host":"Matthieu","computer":"PC-SALON","slot":{"now":false,"next":{"start":<s Unix>,"end":<s Unix>}}|null}` : l'état de la carte « Ordinateur de {hôte} ». `401 {"error":"bad_credentials"}`.
- `POST /api/lend/borrow` — `{"login","password"}` ou `{"code":"123456"}`, et `{"name":"Léa","device":"<id>","app":"mac|windows|linux|ios|tvos|android|webos|web","screen":{"width":2560,"height":1440,"refresh":120},"codecs":["hevc","h264"]}` →
  - `200 {"lend":"<id>","secret":"<jeton>","host":"Matthieu","computer":"PC-SALON","kyber":{"port":…,"fingerprint":"sha256:…","token":"<JWT>","expires":120},"display":{"id":…,"width":2560,"height":1440,"refresh":120},"until":<s Unix>|null,"steam":"friend|host","beat":5}` une fois le mode console prêt (30 s au plus) ;
  - `202 {"request":"<id>","secret":"<jeton>","host":"Matthieu","computer":"PC-SALON"}` : il faut l'accord de l'hôte (`host` sert à « En attente de {hôte}… », même venu par un code), l'app appelle `POST /api/lend/wait` (en-tête `x-kybernow-lend: <secret de la demande>`, `{"request":"<id>"}`, attente longue de 30 s, à répéter ; `202` : toujours en attente) → la réponse `200` ci-dessus, `403 {"error":"refused"}`, ou `410 {"error":"expired"}` (2 min sans réponse) ;
  - refus : `401 bad_credentials`, `404 no_code`, `409 lent` (un prêt en cours), `423 locked` (session Windows verrouillée), `503 off` (prêt coupé, ou pas de session Windows ouverte), `429 too_many` (`retry_after`).
- `POST /api/lend/beat` — en-tête `x-kybernow-lend: <secret>`, `{"lend":"<id>"}`, toutes les `beat` secondes → `{"token":"<JWT>","expires":120,"until":…|null,"notice":null|{"reason":"taken_back|time_up|slot_end|slot_start","at":<s Unix>},"end":null|"taken_back|time_up|slot_end|left|stopped"}`. Sans signe de vie pendant 20 s, son image s'arrête ; le prêt continue 15 minutes, et le même appareil (ou le même compte, ou le même code) y revient par `borrow`. Un prêt que la Station ne connaît plus (secret refusé, prêt fini depuis plus de 15 min) : `401`, `403`, `404` ou `410`, que le cœur lit comme la fin `stopped`.
- `POST /api/lend/leave` — même en-tête : « Rendre l'ordinateur » (`{"lend":"<id>"}`) ; avec l'en-tête et l'identifiant d'une demande en attente (`{"request":"<id>"}`), « Annuler » cette demande.

**L'hôte** (appareil appairé, comme `share/open` ; l'icône par ses routes locales `/api/local/lend…` ; Admin avec sa session) — toutes les routes rendent la **vue du prêt** :

- `GET /api/lend` → `{"enabled":true,"state":"free|preparing|lent|ending","borrower":{"name":"Léa","kind":"code|friend|slot","app":"ios","via":"lan|relay","since":…,"until":…|null,"steam":"friend|host","connected":true}|null,"notice":null|{"reason":…,"at":…},"requests":[{"id":"…","name":"Léa","kind":"code|friend","app":"…","via":"…","since":…}],"code":{"code":"123456","link":"kybernow://join?…","web":"https://…/link/?join=123456","duration":7200|null,"steam":"friend|host","expires":…}|null}` ; la même chose dans la réponse au signe de vie de l'hôte (champ `lend`, absent quand il n'y a rien à dire).
- `POST /api/lend/code` `{"duration":3600|7200|14400|null,"steam":"friend|host"}` ; `DELETE /api/lend/code`.
- `POST /api/lend/requests/{id}` `{"accept":true,"steam":"friend|host"}`.
- `POST /api/lend/take-back` `{"now":false}` (préavis de 2 min) ou `{"now":true}`.

**Réglages** (Admin seulement) : `GET`/`PUT /api/lend/settings` `{"enabled":false,"apps":[<Launch>…],"steam":true}` ; `GET /api/lend/friends`, `POST /api/lend/friends` `{"name","login","password","steam":"friend|host","slots":[{"days":[3],"start":"14:00","end":"18:00"}],"enabled":true}`, `PUT`/`DELETE /api/lend/friends/{id}` ; `GET /api/lend/friends/{id}/invite` → `{"link":"kybernow://borrow?host=…&port=…&fp=…&login=lea[&relay=…&sid=…]","web":"https://…/link/?borrow=lea"}`. Les mots de passe sont gardés hachés (Argon2) et ne sont jamais rendus.

**`info`** : `"lend":{"available":true,"enabled":false}` ; absent pour une Station antérieure.

**Codes** : même espace de six chiffres que le partage (Admin ne donne jamais un code déjà pris). `POST /api/share/join` avec un code de prêt répond `404 {"error":"no_share","lend":true}` : l'app appelle alors `borrow`. Hors de chez soi, le relais tient aussi le code de prêt, et laisse passer `/api/lend/status`, `borrow`, `wait`, `beat` et `leave` (pas les routes de l'hôte, sauf pour un appareil appairé avec « Accès hors de chez vous »).

**Kyber** : l'ami se connecte avec le JWT, rôle `player_kbm` (clavier, souris, manettes, jamais le presse-papiers) et `kn_display` = son écran virtuel (correctif 0019, sans changement). `sub` = `kn-lend-<lend>`.

## Station

- **Mode console** (agent `display-agent console`, dans la session de l'utilisateur, avec son compte ; l'icône près de l'horloge porte le lanceur et le cache de l'écran, en fenêtres `wry`) :
  1. écran virtuel à la taille de l'appareil de l'ami (comme pour une session d'appareil), en écran principal ; les écrans du PC restent allumés et sont couverts par le cache (logo KyberNow Share, page sombre) ;
  2. fenêtres de l'utilisateur réduites (rendues à la fin dans le même état), barre des tâches cachée, curseur gardé sur l'écran virtuel (`ClipCursor`, remis en place) ;
  3. crochets clavier et souris bas niveau : l'entrée **physique** (sans `LLKHF_INJECTED`) est avalée, sauf Entrée et Échap pour le cache ; l'entrée injectée par Kyber passe, sauf Win, Alt+Tab, Alt+Échap, Ctrl+Échap, Ctrl+Maj+Échap ;
  4. surveillance des processus de la session (création d'un processus et passage toutes les 500 ms) : sont permis le lanceur, les apps permises et leurs processus enfants, Steam et ses jeux (`steamapps`), et les programmes de Windows hors d'une liste refusée (Explorateur nouveau, Gestionnaire des tâches, `cmd`, PowerShell, Terminal, Éditeur du Registre, `mmc`, Panneau de configuration, Paramètres) ; un autre processus nouveau est fermé. Les processus de l'utilisateur ouverts avant le prêt ne sont pas touchés ;
  5. fenêtres « Ouvrir » et « Enregistrer sous » des apps permises fermées ;
  6. Steam : « Son compte Steam » → Steam fermé (`steam -shutdown`), `config/loginusers.vdf` et la connexion automatique (`AutoLoginUser`) mis de côté, Steam rouvert en Big Picture sur l'écran de connexion ; à la fin, Steam fermé, ce que l'ami a laissé effacé, les fichiers de l'utilisateur remis, Steam rouvert s'il l'était.
- **Fin** : apps permises fermées (`WM_CLOSE`, puis arrêt après 10 s), Steam remis, fenêtres rendues, barre des tâches, écran virtuel retiré, curseur libéré, crochets retirés, `LockWorkStation`.
- **Session verrouillée** : le prêt ne démarre pas (`423 locked`). Prêt activé : `SetThreadExecutionState` empêche la veille et l'extinction de l'écran qui verrouillerait la session (à vérifier sur l'ordinateur avec « Exiger la connexion »).
- **Ordinateur utilisé** : une session d'appareil, un partage ouvert, ou une entrée physique depuis moins de 5 minutes (`GetLastInputInfo`, hors entrées injectées par Kyber).
- **Créneaux** à l'heure locale du PC ; préavis 2 min avant la fin ; un créneau qui commence pendant qu'on utilise l'ordinateur ne fait rien tant que l'ami n'arrive pas.
- **Gardes** : redémarrage, mise à jour, fin de session et comptes en attente voient le prêt (« L'ordinateur est prêté à Léa : le redémarrage arrêtera le prêt. »).
- **Fil d'Admin** : « Ordinateur prêté à Léa », « Léa a rendu l'ordinateur », « Vous avez repris l'ordinateur ».
- **Admin, page « Prêt »** : interrupteur « Prêter l'ordinateur » (coupé au départ) et l'avertissement « Votre ami joue dans votre session Windows… » ; « Apps permises » (Steam, puis « Ajouter une app » depuis le catalogue) ; « Comptes d'amis » (prénom, identifiant, mot de passe, compte Steam proposé, créneaux, lien d'invitation et QR code) ; le prêt en cours et le code de séance.
- **Icône près de l'horloge** : « Prêter l'ordinateur… » (panneau du code), « Reprendre l'ordinateur » pendant un prêt, la notification des demandes (« Accepter », « Refuser »).

## Cœur Rust

- **Emprunter** : `lend::status`, `lend::borrow` (code ou compte), `lend::wait`, et une session d'emprunt `SessionRole::Borrower` : elle part de la réponse de `borrow` (pas de `prepare` : l'écran virtuel est déjà là), se connecte à Kyber avec le jeton renouvelé par `beat`, a le clavier, la souris et les manettes (`InputRights` du rôle `player_kbm`, sans presse-papiers), n'envoie ni signe de vie d'hôte, ni `foreground`, ni changement d'écran ou de profil de la Station, ne redémarre jamais la Station ; se reconnecte seule pendant les 15 minutes ; part par `lend::leave`. Le préavis et la fin : `Platform::on_lend_notice`, `Platform::on_lend_end`, avec les textes de link-ui. « Rendre l'ordinateur » en session : `Session::give_back` ; fermer la session ne rend rien.
- **Rejoindre avec un code** : quand la Station répond `"lend":true`, `share::join` (et `join_checked`, `join_anywhere`, `join_link`) rend `ShareError::Lend(LendCode { code, target, route })`, la Station et le chemin pris ; l'app emprunte alors avec `lend::borrow(&target, route, …)` et l'écran de l'appareil (que `join` ne connaît pas). `share::Joining` ne change pas, pour les apps qui ne prêtent pas encore (« Ce code sert à emprunter l'ordinateur. »). Ponts : `KN_SHARE_LEND` 29, JNI code 29, avec `{code, target, route}`.
- **Ordinateurs empruntés** : `lend::parse_invite` lit `kybernow://borrow?…` ; l'app garde l'ordinateur à part de ses ordinateurs appairés (identifiant, mot de passe dans son trousseau).
- **Hôte** : `Console::lend_state`, `lend_code`, `lend_cancel_code`, `lend_answer`, `lend_take_back` ; la vue arrive aussi avec le signe de vie de l'hôte (`Platform::on_lend`) ; `Info::lend`.
- Pont C et JNI : les mêmes fonctions (`kn_lend_*`, `kn_console_lend_*` ; `Native.lend*`).

## Apps et client web

Toutes les apps Link et le client web :

- **Ami** : « Rejoindre avec un code » accepte aussi les codes de prêt ; la carte « Ordinateur de {hôte} » sur l'accueil (ajoutée par le lien d'invitation, ou « Ajouter un ordinateur » › « Avec un compte d'ami »), son état et « Emprunter » ; l'attente de l'accord ; la session d'emprunt et sa barre ; le préavis et la fin.
- **Hôte** : dans la barre de session, « Prêter l'ordinateur… » (durée, compte Steam, code, lien, QR code) et les demandes ; sur l'accueil de cet ordinateur, « Prêté à Léa » et « Reprendre l'ordinateur ».
- Téléviseurs et Apple TV : la saisie du code et de l'identifiant au pavé à l'écran, comme l'appairage.
- **Client web** : l'ami par `/link/?join=<code>` ou `/link/?borrow=<identifiant>` ; l'hôte dans Admin (page « Prêt »).

## Hors de ce premier pas

- Réveiller l'ordinateur endormi pour un ami hors de chez vous.
- Plusieurs amis à la fois, ou prêter pendant que vous jouez (machine virtuelle).
- Un compte Windows à part, étanche : écarté (trop lourd), le mode console est la voie choisie.
