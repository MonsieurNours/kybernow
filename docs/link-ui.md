# KyberNow Link : écrans et textes communs

[English](link-ui.en.md) · Français

Toutes les apps KyberNow Link (Mac, Apple TV, iPhone et iPad, Windows, Android et Android TV, téléviseurs LG, Linux, Steam Deck, Raspberry Pi et KyberNow Link OS, client web `/link`) suivent ce document : mêmes écrans, même ordre, mêmes mots. Chaque plateforme garde ses spécificités (télécommande, manette, toucher, fenêtres, QR code au téléviseur), mais une même chose porte le même nom partout. Le ton est celui du site ([ton-du-site.md](ton-du-site.md)) ; les noms et la typographie, ceux de [i18n.md](i18n.md).

## Principes

- On vouvoie. Phrases courtes, avec un verbe ; un bouton est un verbe et son objet (« Redémarrer la Station »).
- Rien de technique sur l'accueil : ni carte graphique, ni adresse, ni décodeur, ni latence. Les chiffres vont dans **Statistiques**, les réglages dans les **Réglages de l'ordinateur**.
- Rien par-dessus l'image quand elle arrive : la connexion montre son avancement, puis l'image seule. Les mesures s'affichent à la demande (barre ou menu de session).
- Les points de suspension ne servent qu'aux états en cours (« Connexion… ») et, sur Mac, aux commandes qui ouvrent une fenêtre (convention d'Apple).
- Honnête : on dit combien de temps ça prend et ce qui sera coupé.
- « de » s'élide devant un nom qui commence par une voyelle ou un h : « Réglages d'Atelier », « Réglages d'Hugo-PC », mais « Réglages de Bureau-Salon ».

## Accueil

Pour l'ordinateur choisi, dans cet ordre et rien d'autre :

| Élément | Français | English |
|---|---|---|
| État | « En ligne », « Hors ligne », « Recherche… » ; « Hors de chez vous » quand l'ordinateur n'est joignable que par le relais ([relais.md](relais.md)) | “Online”, “Offline”, “Searching…”; “Away from home” |
| Nom | le nom de l'ordinateur | |
| Système | « Windows 11 Pro » (tel que la Station le donne) | |
| Écran, écran virtuel actif, à la taille de cet appareil | « Écran de 2560 × 1440 à 120 Hz, à la taille de cet écran » | “2560 × 1440 display at 120 Hz, sized for this screen” |
| Écran, plusieurs écrans (Mac, Windows, Linux) | « 3 écrans, à la taille de ceux de cet appareil » | “3 displays, sized for this device's screens” |
| Écran, sur mesure | « Écran de 1920 × 1080 à 60 Hz » | “1920 × 1080 display at 60 Hz” |
| Écran, sans écran virtuel | « Les écrans de l'ordinateur, tels quels » | “The computer's own displays, as they are” |
| Bouton principal, en ligne | « Se connecter » | “Connect” |
| Bouton principal, ordinateur déjà en session (Mac, Windows) | « Afficher la session » : sa fenêtre passe devant | “Show session” |
| Bouton principal, hors ligne | « Réveiller l'ordinateur » quand l'app connaît son adresse MAC, sinon « Réessayer » ; pendant le réveil : « Réveil de l'ordinateur… » | “Wake the computer” when the app knows its MAC address, otherwise “Try again”; while waking: “Waking the computer…” |
| Profils | « Jeu », « Bureau », « Max », et « Personnalisé » quand les réglages s'en écartent | “Game”, “Desktop”, “Max”, “Custom” |
| Phrase du profil | Jeu : « La réactivité d'abord, pour jouer. » Bureau : « Un texte net, pour travailler. » Max : « La meilleure image que cet appareil affiche. » Personnalisé : « Vos propres réglages. » | Game: “Responsiveness first, for gaming.” Desktop: “Sharp text, for work.” Max: “The best picture this device can show.” Custom: “Your own settings.” |
| Accès aux réglages | « Réglages » (icône d'engrenage ; libellé accessible « Réglages de {nom} ») | “Settings” (“Settings for {name}”) |
| Apps de salon seulement (Apple TV, Android TV, téléviseurs LG, Linux au salon et Link OS) | « 2 manettes », « 1 manette », « Aucune manette » | “2 controllers”, “1 controller”, “No controller” |

L'écran affiché est celui que la session demandera (même calcul que la session : définition, fréquence, limite de 4096 de large en H.264 sur Windows) ; « tels quels » quand le réglage « Écran virtuel » est coupé ou que la Station n'en a pas.

Erreurs sous le nom, quand il y en a : « Mot de passe refusé : mettez-le à jour dans les réglages. » / “Password refused: update it in the settings.” ; « La Station a été réinstallée : vérifiez son empreinte dans les réglages. » / “The Station was reinstalled: check its fingerprint in the settings.”

Les autres ordinateurs (rangée ou liste) : le nom et l'état (« En ligne », « Hors ligne »), rien d'autre. Un ordinateur annoncé sur le réseau et pas encore ajouté : « Sur le réseau » / “On your network”. Ajout : « Ajouter un ordinateur » / “Add a computer”.

En haut : « Réglages de l'app » / “App settings” (Link OS : « Réglages de l'appareil » / “Device settings”), et la mise à jour de l'app quand il y en a une : « Version {v} disponible » / “Version {v} available”.

## Connexion

L'écran de connexion montre les étapes, rien d'autre :

| Étape | Français | English |
|---|---|---|
| 1 | « Préparation de l'écran » ; avec plusieurs écrans : « Préparation des écrans » | “Preparing the display”; with several displays: “Preparing the displays” |
| 2 | « Connexion » | “Connecting” |
| 3 | « Premières images » | “First frames” |
| Échec | « La connexion a échoué », puis la raison en une phrase | “The connection failed”, then the reason |
| Bouton | « Annuler » | “Cancel” |

Si Admin répond mais que l'image n'arrive pas malgré les réparations de la session (image clé, relance, débit réduit, 4:2:0), ou que la connexion au moteur de streaming échoue, l'app redémarre la Station, une fois, sans couper la session d'un autre appareil, attend son retour (1 min 30 au plus) et retente la connexion une fois. L'écran de connexion le dit : « La Station redémarre… » / “The Station is restarting…”, puis reprend ses étapes. Si l'image ne vient toujours pas : « La connexion a échoué », et la raison.

Dès la première image, l'image seule. Pas de capsule d'arrivée, pas de barre qui s'affiche puis se cache, pas de chiffre. Le titre de fenêtre (Windows) est « {nom} · KyberNow Link » (la ligne de commande garde ses mesures dans le titre, faute de barre).

**Plusieurs écrans** (apps Mac, Windows et Linux, voir [multi-ecrans.md](multi-ecrans.md)) : l'écran de connexion s'affiche sur l'écran principal de l'appareil ; les autres montrent son fond (sa couleur, sans la photo de l'accueil), sans texte ; chaque écran passe à son image dès sa première image. Si la connexion échoue, le message est sur l'écran principal et les autres fenêtres se ferment. Un écran que l'ordinateur n'a pas pu créer garde ce fond, avec une seule ligne : « L'ordinateur n'a pas pu créer cet écran. » / “The computer could not create this display.” Quand les écrans de l'appareil changent en session, ou qu'un profil choisi en session change leur nombre, la connexion reprend à sa première étape sur l'écran principal ; si l'ordinateur refuse alors les nouveaux écrans, la session se termine avec la raison, comme un échec de la connexion. Un écran créé mais resté sans image (jamais annoncé, ou son encodeur ne s'est pas ouvert) affiche la même ligne qu'un écran non créé. Avec une Station qui ne gère qu'un écran, les fenêtres des autres écrans ne s'ouvrent pas, sans message. Une seule barre de session, sur l'écran où se trouve le pointeur ; la fenêtre où elle s'ouvre devient la fenêtre active ; ses commandes valent pour toute la session. Le réglage « Écrans » changé dans les Réglages pendant une session vaut pour la session suivante ; le profil choisi dans la barre ou le menu de l'icône s'applique tout de suite. « Plein écran » n'est pas proposé (barre, menu, raccourci) : les fenêtres restent en plein écran ; pour travailler dans une fenêtre, on choisit « Cet écran ». Toutes les fenêtres portent le même titre.

**Plusieurs ordinateurs** (apps Mac, Windows et Linux en mode Bureau, voir [plusieurs-ordinateurs.md](plusieurs-ordinateurs.md)) : pendant une session, la barre a un menu « Ordinateurs » / “Computers”, présenté comme les autres menus de la barre de l'app (icône d'un ordinateur ; seule là où la barre montre ses menus en icônes, comme « Touches » sur Windows et Linux ; avec son libellé sur Mac, icône seule dans une barre étroite) et absent quand vous n'avez pas d'autre ordinateur : vos autres ordinateurs, dans l'ordre de l'accueil, avec leur point de couleur et leur état comme dans le menu de l'icône (« {nom} — En ligne »…) ; un clic sur un ordinateur en ligne ouvre sa session, sur un ordinateur hors ligne le réveille (« Réveiller l'ordinateur », comme l'accueil) ; ceux qui sont déjà en session sont cochés et proposent « Afficher » / “Show”. La première session s'ouvre comme d'habitude ; chaque session suivante s'ouvre dans une fenêtre par-dessus, sur l'écran de la fenêtre active, centrée, aux deux tiers de cet écran, avec un seul écran, et sa connexion s'affiche dans cette fenêtre. Chaque fenêtre porte le nom de son ordinateur. La session de la fenêtre au premier plan reçoit le clavier, la souris, les manettes, le presse-papiers et le son ; les autres gardent leur image, sans son. Avec au moins deux sessions ouvertes, la pastille du son de la barre devient un bouton, libellé accessible « Garder le son » / “Keep sound” (icône du haut-parleur, allumée quand la session reste audible même sans être active ; éteint par défaut, pour la session en cours) ; avec une seule session, elle reste comme avant. Raccourci « Ordinateur suivant » / “Next computer” : ⌃⌥⌘O sur Mac, Ctrl+Alt+Maj+O sur Windows et Linux, vers la fenêtre de l'ordinateur suivant dans l'ordre d'ouverture. « Se connecter » sur un ordinateur déjà en session fait passer sa fenêtre devant. Quatre ordinateurs au plus : au-delà, « Se connecter » est inactif, aide « Quatre ordinateurs au plus en même temps. » / “Four computers at most at the same time.” Le clic qui fait passer une fenêtre de session au premier plan va aussi à son ordinateur, comme avec une seule session. Une session qui s'arrête sans « Se déconnecter » (un autre appareil, Admin, une erreur) alors que d'autres continuent donne toujours sa notification, même app au premier plan et même si c'était la session active, sauf quand son message reste affiché dans sa fenêtre (échec d'une connexion, avec « Fermer ») : pas de doublon (Linux, sans notifications : le message reste dans sa fenêtre jusqu'à « Fermer »). Si la connexion d'une session suivante échoue, le message reste dans sa fenêtre avec un seul bouton, « Fermer » / “Close”, qui ferme cette fenêtre (au lieu de « Revenir à l'accueil » : les autres sessions continuent). Fermer une fenêtre ou « Se déconnecter » ne termine que sa session ; quitter l'app les termine toutes.

En session : « Se déconnecter » / “Disconnect” (barre, menu, raccourci), « Statistiques » / “Statistics” pour les mesures en direct, « Touches » / “Keys”, « Plein écran » / “Full screen”, « Clavier » / “Keyboard”, « Ordinateurs » / “Computers” et « Garder le son » / “Keep sound” (plusieurs ordinateurs, ci-dessous).

**Barre de session au pointeur** (apps Mac et Windows, app Linux en mode Bureau, client web) : elle s'ouvre quand le pointeur reste un quart de seconde contre le bord haut de l'image (à 2 points au plus), dans une zone centrée de la largeur de la barre, et seulement si l'app est au premier plan, le pointeur sur la fenêtre de la session et la souris en mode bureau. Un passage rapide ne l'ouvre pas ; un pointeur sur un autre écran ou sur une autre app non plus, ni un bouton de la souris tenu (une fenêtre de l'ordinateur glissée vers le haut). En souris des jeux, le pointeur est capturé : la barre vient par les autres moyens de l'app (menu de l'icône, manette). Un trait court (220 points), centré en haut de l'image, signale l'endroit, seulement quand le geste est possible (app au premier plan, souris du bureau, barre fermée) ; le client web le rend aussi touchable. La barre se referme 0,4 s après que le pointeur l'a quittée, jamais pendant qu'un de ses menus est ouvert. Avec plusieurs écrans, elle s'ouvre sur l'écran où le geste a lieu.

Au toucher (iPhone, iPad, Android hors téléviseur) :

- **Clavier** : il s'ouvre depuis la barre ; un toucher sur l'image le replie (ce toucher ne clique pas sur l'ordinateur).
- **Manette à l'écran** / “On-screen controller” : bouton de la barre, retenu pour chaque ordinateur. Deux dispositions fixes, pour les deux pouces en paysage : « Complète » / “Full”, une manette Xbox entière (deux sticks qui s'enfoncent : L3, R3 ; la croix ; A, B, X, Y ; LB, RB, LT et RT progressives ; Affichage, Menu et Guide) ; « Simple » / “Simple”, le stick gauche, la croix, A, B, X, Y et Menu. Un petit bouton sur la manette passe de l'une à l'autre (libellé accessible « Manette simple » / « Manette complète », “Simple controller” / “Full controller”), choix retenu pour chaque ordinateur. L'ordinateur la voit comme une manette Xbox 360, à la première place de joueur libre. Tant qu'elle est affichée, le reste de l'écran ne sert pas de souris. Réglages dans Commandes : « Disposition » / “Layout” (Complète, Simple), « Taille » / “Size” (Petite, Moyenne, Grande / Small, Medium, Large), « Opacité » / “Opacity” et « Retour haptique » / “Haptic feedback” (Désactivé, Faible, Moyen, Fort / Off, Light, Medium, Strong ; Moyen par défaut). Chaque appui se sent et se voit : le bouton s'allume et s'enfonce ; un clic net à l'appui d'un bouton, de la croix ou d'un stick enfoncé ; pour les gâchettes, un clic léger au début de la course et un clic franc en bout de course ; pour les sticks, un tic quand le stick quitte le centre, un tic très léger à chaque changement de direction (huit directions) et un tic en butée, jamais de vibration continue. Pendant que la manette est affichée, les vibrations du jeu font vibrer l'appareil. Sans moteur haptique (iPad), seul le retour visuel reste.

## Partage (KyberNow Share)

Contrat : [kybernow-share.md](kybernow-share.md), et [relais.md](relais.md) hors de chez soi. Toutes les apps Link et le client web, mêmes textes partout.

**L'hôte, dans sa session** : « Partager » / “Share” dans la barre (présenté comme les autres menus de la barre de l'app, icône de partage). Il ouvre un panneau :

| Élément | Français | English |
|---|---|---|
| Titre | « Partager {nom} » (l'ordinateur) | “Share {name}” |
| Code | « 123 456 » (six chiffres, groupés par trois) | |
| Code, QR code | étiquette « Code » ; QR code du lien des apps (`link`, `kybernow://join…`), libellé accessible « QR code du partage » | “Code”; “QR code for the share” |
| Lien | « Copier le lien » : copie l'adresse du client web (`web`) ; puis « Lien copié. », ou « Copie impossible : sélectionnez le lien à la main. » avec l'adresse à sélectionner ; pas de copie sans presse-papiers utile (Apple TV, téléviseurs, salon) | “Copy link”; “Link copied.”; “Could not copy: select the link by hand.” |
| Aide | « Sur le même réseau : « Rejoindre avec un code » dans KyberNow Link, ou ce lien dans un navigateur. » | “On the same network: “Join with a code” in KyberNow Link, or this link in a browser.” |
| Aide, relais joint (`relay.state` `joined`) | « Sur le même réseau ou ailleurs : « Rejoindre avec un code » dans KyberNow Link, ou ce lien dans un navigateur. Hors de chez vous, vous acceptez chaque invité. » ; « Copier le lien » copie alors le lien du relais | “On the same network or elsewhere: “Join with a code” in KyberNow Link, or this link in a browser. Away from home, you accept each guest.” |
| Lien du relais en préparation (`relay.state` `connecting` : connexion au relais ou code pas encore réservé), sous l'aide du même réseau | « Préparation du lien hors de chez vous… » avec un indicateur d'attente ; « Copier le lien » inactif jusqu'au lien du relais (10 s au plus, puis il copie le lien du réseau) ; le QR code attend aussi | “Getting the away-from-home link ready…” |
| Relais injoignable (`relay.state` `unreachable`), sous l'aide du même réseau | « Pas encore joignable hors de chez vous. » | “Not reachable away from home yet.” |
| Demandes (invités venus par le relais, `pending`), au-dessus des invités, seulement s'il y en a | titre « Demandes » ; « {prénom} demande à rejoindre », dessous « {app} · hors de chez vous » ; boutons « Accepter » et « Refuser » ; « Accepter » inactif quand les quatre places sont prises ; une demande qui arrive pendant la session : avis bref « {prénom} demande à rejoindre. » (le choisir ouvre le panneau) ; sous-titre de la tuile « Partager » des menus à tuiles : « 1 demande · 2 invités » (les demandes d'abord) | “Requests”; “{name} wants to join”, below “{app} · away from home”; “Accept” and “Decline”; notice “{name} wants to join.”; tile “1 request · 2 guests” |
| Liste | titre « Invités » ; aucun : « Personne pour l'instant. » | “Guests”; “Nobody yet.” |
| Un invité | « {prénom} · Spectateur » (aucun droit), « {prénom} · Joueur » (manettes), « {prénom} · Clavier et souris », « {prénom} · Joueur, clavier et souris » ; dessous, son app, sa qualité d'image si elle n'est pas celle de l'hôte (« 1080p · 60 Hz ») et « depuis {durée} » | “{name} · Spectator”, “{name} · Player”, “{name} · Keyboard and mouse”, “{name} · Player, keyboard and mouse”; “for {duration}” |
| Un invité venu par le relais (`via` `relay`) | « hors de chez vous » après son app : « iPhone ou iPad · hors de chez vous · depuis 3 min » | “away from home”: “iPhone or iPad · away from home · for 3 min” |
| App d'un invité (`app`) | Mac, Windows, Linux, « iPhone ou iPad », Apple TV, Android, « Téléviseur LG », « Navigateur » | Mac, Windows, Linux, “iPhone or iPad”, Apple TV, Android, “LG TV”, “Browser” |
| Durée (`since`, secondes Unix de la Station) | « moins d'une minute », « 3 min », « 1 h 12 » ; jamais négative (horloges décalées) | “under a minute”, “3 min”, “1 h 12” |
| Actions sur un invité | deux droits indépendants, chacun donné ou repris sans toucher à l'autre : « Donner les manettes » / « Reprendre les manettes » ; « Donner le clavier et la souris » / « Reprendre le clavier et la souris » ; puis « Retirer » | “Give controllers”; “Take back controllers”; “Give keyboard and mouse”; “Take back keyboard and mouse”; “Remove” |
| Code changé | « Nouveau code » (les invités déjà là restent) | “New code” |
| Fin | « Arrêter le partage » (tout le monde sort) | “Stop sharing” |
| Limite | « Quatre invités au plus. », seulement quand les quatre places sont prises | “Four guests at most.” |
| Station trop ancienne | « Le partage demande KyberNow Station 0.8.0 ou plus récente. » | “Sharing needs KyberNow Station 0.8.0 or later.” |
| Partage coupé sur l'ordinateur | « Le partage est coupé sur cet ordinateur (Admin › Appareils Link). » | “Sharing is turned off on this computer (Admin › Link devices).” |

Partage impossible (Station trop ancienne, partage coupé) : « Partager » estompé ; à la souris, inactif avec la raison en infobulle ; au toucher et à la télécommande, il reste sélectionnable et ouvre le panneau réduit à la raison.

Un partage ouvert se voit dans la barre (pastille « 2 invités » / “2 guests”, à partir d'un invité, jamais « 0 invité » ; elle rouvre le panneau), dans sa partie toujours visible. Les demandes ont leur pastille, à côté de celle des invités : « 1 demande », « 2 demandes » (“1 request”, “2 requests”), seulement quand il y en a ; elle rouvre aussi le panneau. Une demande ne compte pas dans les invités tant qu'elle n'est pas acceptée. Dans les menus à tuiles (Apple TV, téléviseurs, salon, Deck, Android TV), le sous-titre de la tuile « Partager » est le nombre d'invités dès qu'il y en a un, sinon le code (« 123 456 ») quand le partage est ouvert, sinon « Code et QR code » / “Code and QR code”. Fermer la session arrête le partage.

**Le PC lui-même** (KyberNow Station : menu de l'icône près de l'horloge, et accueil d'Admin) : « Partager un écran… » / “Share a screen…”, puis le choix de l'écran (« Écran 1 — 3840 × 2160 », l'écran principal d'abord) ; le même panneau (code, lien, invités, actions), titré « Partage de l'écran 1 » / “Sharing display 1”. Le PC reste utilisable normalement ; aucun écran virtuel, aucun écran éteint.

**Refus de la Station** (Admin, icône, et `error` des routes de l'hôte) : « {nom} est en session : le partage se lance depuis cet appareil. » / “{name} is in a session: share from that device.” ; « Une connexion est en cours sur cet ordinateur : partagez un écran une fois qu'elle est finie. » / “A connection is in progress on this computer: share a screen once it has ended.” ; « Cet écran n'est plus branché. » / “This display is no longer connected.” ; « Cet invité n'est plus dans le partage. » / “This guest is no longer in the share.” ; « Seul l'appareil en session peut partager cet ordinateur. » / “Only the device in a session can share this computer.” ; « Aucun partage en cours. » / “No share in progress.” ; redémarrage ou mise à jour pendant un partage : « Un partage est en cours avec {prénoms} : le redémarrage l'arrêtera » / “A share is in progress with {names}: restarting will stop it” (« l'installation l'arrêtera » / “installing will stop it”).

**Panneau de la Station** (Admin : accueil et Appareils Link ; panneau de l'icône), en plus des textes ci-dessus :

| Élément | Français | English |
|---|---|---|
| Aide de la carte (Admin) | « Montrez un écran de cet ordinateur à quelqu'un sur votre réseau : il le voit, entend le son, et peut jouer si vous le laissez. Vous continuez d'utiliser l'ordinateur normalement. » | “Show one of this computer's screens to someone on your network: they see it, hear the sound, and can play if you let them. You keep using the computer as usual.” |
| Choix de l'écran | « Choisissez l'écran à partager. » ; « Recherche des écrans… » ; « Annuler » | “Choose the screen to share.”; “Looking for screens…”; “Cancel” |
| Partage d'un appareil | « Ouvert depuis la session de {nom}. » | “Opened from {name}'s session.” |
| Liste | « Participants » quand l'hôte y figure ; « {nom} · Hôte » | “Participants”; “{name} · Host” |
| Arrêté | « Partage arrêté. » | “Sharing stopped.” |
| En-tête d'Admin, haut du panneau | « Partage en cours · {n} invités » ; « Partage en cours » | “Sharing · {n} guests”; “Sharing” |
| Icône, ligne d'état | « Partage de l'écran {n} · {n} invités » ; avec des demandes : « … · 1 demande » (aussi en haut de son panneau) | “Sharing display {n} · {n} guests”; “… · 1 request” |
| Icône, demande | une nouvelle demande d'un partage lancé depuis le PC ouvre le panneau sur le partage, une fois par demande (pas pour le partage d'un appareil, dont l'écran est celui de la session) | — |
| Icône, confirmations | « Un partage est en cours avec {prénoms} : arrêter la Station le coupera. Arrêter la Station ? » ; « … : le redémarrage l'arrêtera. Redémarrer la Station ? » ; en quittant : « Un partage est en cours avec {prénoms}. La Station peut rester en marche : le partage continue, et vous pourrez rouvrir l'icône depuis le menu Démarrer. » | “A share is in progress with {names}: stopping the Station will end it. Stop the Station?”; “… restarting will stop it. Restart the Station?”; “A share is in progress with {names}. The Station can keep running: the share goes on, and you can reopen the tray icon from the Start menu.” |
| Icône, échec | « Admin n'a pas pu changer le partage ({erreur}). » | “Admin could not change the share ({error}).” |

**L'invité** :

| Élément | Français | English |
|---|---|---|
| Accueil, à côté de « Ajouter un ordinateur » | « Rejoindre avec un code » | “Join with a code” |
| Écran | titre « Rejoindre un partage » ; aide « Le code s'affiche chez la personne qui partage. » ; « Votre prénom » (proposé : le nom de l'appareil, retenu) ; bouton « Rejoindre » | “Join a share”; “The code is shown by the person sharing.”; “Your name”; “Join” |
| Avec un appareil photo (iPhone, iPad, téléphones et tablettes Android) | en haut de l'écran, le choix « Code » / « QR code » ; un QR code de partage lu dans « Ajouter un ordinateur › QR code » mène aussi ici | “Code” / “QR code” |
| Lien | `kybernow://join…` (et le QR code) ouvre cet écran, code et ordinateur remplis ; rien ne part avant « Rejoindre » ; lien illisible : « Ce lien de partage est illisible. » | “This share link cannot be read.” |
| Code incomplet | « Le code a six chiffres. » | “The code has six digits.” |
| Caméra | refusée : « KyberNow Link n'a pas accès à la caméra. Autorisez-la dans les Réglages, ou tapez le code. » ; absente : « Pas de caméra disponible sur cet appareil. Tapez le code. » | “KyberNow Link can't access the camera. Allow it in Settings, or type the code.”; “No camera available on this device. Type the code.” |
| Recherche | « Recherche du partage… » | “Looking for the share…” |
| Code inconnu | « Aucun partage avec ce code sur le réseau. » | “No share with this code on your network.” |
| Complet | « Ce partage est complet (quatre invités au plus). » | “This share is full (four guests at most).” |
| Trop d'essais (10 par minute) | « Trop d'essais de code : réessayez dans une minute. » | “Too many code attempts: try again in a minute.” |
| Format illisible (seulement si l'appareil ne lit aucun format possible ; sinon il reçoit une image qu'il lit) | « Cet appareil ne lit pas l'image de cet ordinateur ({format}). » | “This device cannot play this computer's picture ({format}).” |
| Partage indisponible (la Station ne peut pas signer le jeton de Kyber, passager) | « Le partage n'est pas disponible pour le moment : réessayez dans un instant. » | “Sharing is not available right now: try again in a moment.” |
| Connexion | les étapes de la connexion, titrées « Invité sur {nom} » | “Guest on {name}” |
| Barre (réduite) | ce qui ne touche que l'appareil (latence et statistiques, son, plein écran s'il existe, « Qualité de l'image ») ; « Spectateur » (aucun droit) ou « Joueur » (un droit au moins) ; « Quitter le partage » ; la manette seulement avec les manettes ; clavier, souris et menu des touches seulement avec le clavier et la souris ; ni profil, ni réglages de l'ordinateur, ni « Ordinateurs », ni presse-papiers | “Spectator” or “Player”; “Leave” |
| Qualité de l'image | à l'écran « Rejoindre » (sous le prénom) et dans la barre : « Qualité de l'image » : « Auto » (proposé), « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz » ; retenue pour la fois suivante ; la changer en cours de session relance l'image de l'invité seulement (étapes « Premières images »), jamais celle de l'hôte. « Auto » : la définition et la fréquence au plus celles de l'écran de l'appareil et de ce que son décodeur suit (et 60 Hz au plus dans un navigateur) ; quand c'est l'image de l'hôte, il la partage. L'hôte passe toujours en premier : l'image d'un invité n'est jamais au-dessus de celle de l'hôte (définition, fréquence, débit, 4:4:4 ; « Comme l'hôte » ou plus : celle de l'hôte), et rien de ce que fait un invité ne change l'écran de l'ordinateur ni l'image de l'hôte. Dans la barre, à la place du profil de l'hôte : une puce (une tuile qui ouvre un panneau dans les menus à tuiles) qui montre la qualité reçue (« 1080p · 60 Hz »), ou le choix tant que la Station ne la connaît pas (« Auto ») ; les cinq choix, la qualité reçue à côté de celui qui est coché. Dans le panneau de l'hôte, sous l'invité : « {app} · {qualité} · depuis {durée} », seulement pour une image à part dont la taille est connue | “Picture quality”: “Auto”, “Same as host”, “4K · 60 Hz”, “1080p · 60 Hz”, “720p · 30 Hz” |
| Image ajustée par la Station (avis bref) | en retard sur l'image de l'hôte, l'invité passe seul sur une image plus légère : « Image allégée pour suivre. » (le même avis quand le décodeur de l'appareil ne suit pas et que l'app repasse d'elle-même en 720p30) ; plus d'image à part possible (deux au plus, une sur une carte Intel ; trois et deux pour un partage lancé depuis le PC ; les invités de même qualité partagent la leur) : « L'ordinateur ne peut pas faire une image de plus : vous recevez {qualité}. » (l'image ouverte la plus proche, plus légère) | “Picture lightened to keep up.”; “The computer can't make another picture: you get {quality}.” |
| Droits changés | « Vous pouvez jouer à la manette. » ; « Les manettes sont reprises. » ; « Vous avez le clavier et la souris. » ; « Le clavier et la souris sont repris. » ; plus aucun droit : « Vous regardez de nouveau. » à la place de l'avis de reprise ; les deux droits changés d'un coup : un seul avis, le droit donné avant le droit repris, les manettes avant le clavier et la souris (avis bref) | “You can play with a controller.”; “The controllers were taken back.”; “You have the keyboard and mouse.”; “The keyboard and mouse were taken back.”; “You are watching again.” |
| Fin | « {hôte} a arrêté le partage. » ; « {hôte} vous a retiré du partage. » ; sinon la raison d'un échec ; jamais « Reprendre la main » ; la reconnexion d'un invité propose « Quitter le partage » (pas « Se déconnecter ») | “{host} stopped sharing.”; “{host} removed you from the share.” |
| Attente, par le relais | « En attente de l'accord de l'hôte… », bouton « Annuler » (quitte la demande) ; le prénom de l'hôte et le nom de l'ordinateur ne sont connus qu'après l'accord : l'écran reste titré « Rejoindre un partage » | “Waiting for the host to accept…”, “Cancel” |
| Refusé | « L'hôte a refusé. » | “The host declined.” |
| Sans réponse (120 s) | « L'hôte n'a pas répondu. » | “The host did not answer.” |
| Relais muet | « Le relais ne répond pas. » | “The relay is not answering.” |
| Trop de demandes (trois attendent l'hôte) | « Trop de demandes en attente : réessayez dans un instant. » | “Too many requests waiting: try again in a moment.” |

{hôte} est le prénom de l'hôte (le nom de l'appareil de l'hôte), ou le nom de l'ordinateur pour un partage lancé depuis le PC. Un invité n'est pas ajouté aux ordinateurs de l'appareil.

**Client web** (navigateur) : le lien du partage (`https://<adresse>:<port>/link/?join=123456`, « Copier le lien » de l'hôte) ouvre « Rejoindre un partage » **sans le mot de passe d'Admin**, avec le code déjà rempli (« 123 456 ») et le prénom proposé (« Chrome sur Mac », retenu pour la fois suivante) ; il faut cliquer « Rejoindre » (jamais de connexion toute seule). « Rejoindre avec un code » est aussi sur l'écran du mot de passe d'Admin et sur l'accueil du client web (à côté de « Réglages »), avec « Annuler » pour revenir ; le code se tape en six chiffres, groupés par trois à mesure. Le navigateur n'a qu'un ordinateur, celui qui sert la page : le code va à lui seul, sans recherche sur le réseau. Le format de l'image se vérifie une fois la Station répondue (le moteur de Kyber ne se charge qu'avec le cookie de l'invité) : le navigateur demande celui de l'hôte s'il le lit, sinon H.264 (image à part) ; il ne sort aussitôt du partage, en le disant, que s'il ne lit ni H.264 ni HEVC. « Qualité de l'image » : à « Rejoindre » et dans la barre (une puce « 1080p · 60 Hz » qui ouvre les cinq choix) ; « Auto » d'après l'écran (`screen × devicePixelRatio`), 60 Hz au plus, et ce que le décodeur du navigateur accepte à cette taille (sinon 3840 × 2160, puis 1920 × 1080, puis 1280 × 720) ; un changement trop rapproché (`429`) est refait après `Retry-After`, sans avis ; si la qualité n'a pas pu changer : « La qualité de l'image n'a pas pu changer : réessayez. ». Une erreur de décodage redemande une image clé au lieu de couper ; si elle revient, l'image repasse une fois en 720p30 (« Image allégée pour suivre. »), puis le texte d'échec habituel. Fin du partage : la carte de fin (« Session terminée », le texte de fin) n'a qu'un bouton, « Rejoindre avec un code » ; pas de « Se reconnecter ». Barre d'un invité : latence et statistiques, son, plein écran, « Spectateur » ou « Joueur », « Quitter le partage » ; avec « le clavier et la souris » : le mode de la souris et le menu « Touches » (en tactile : pavé, toucher direct, clavier) ; la puce des manettes seulement en joueur. Pas de profil, de réglages ni de presse-papiers.

**Fil d'Admin** : « {hôte} partage l'écran {n} » / “{host} is sharing display {n}” ; « {prénom} regarde votre écran » / “{name} is watching your screen” (en arrivant, ou plus aucun droit) ; « {prénom} joue avec vous » / “{name} is playing with you” (un droit donné) ; « {prénom} a quitté le partage » / “{name} left the share” (parti, retiré, ou sans nouvelles) ; « Partage arrêté » / “Sharing stopped” ; par le relais : « {prénom} demande à rejoindre (hors de chez vous) » / “{name} wants to join (away from home)”, « {prénom} : demande refusée » / “{name}: request declined”.

## Prêt de l'ordinateur (KyberNow Share)

Contrat : [pret.md](pret.md). Toutes les apps Link, le client web, Admin et l'icône près de l'horloge, mêmes textes partout. Le lanceur et l'écran du PC portent le logo KyberNow Share.

**L'hôte** : « Prêter l'ordinateur… » / “Lend the computer…” (barre de session des apps Link, menu de l'icône, accueil et page « Prêt » d'Admin). Il ouvre un panneau :

| Élément | Français | English |
|---|---|---|
| Titre | « Prêter {nom} » (l'ordinateur) | “Lend {name}” |
| Durée | « Durée » : « 1 heure », « 2 heures », « 4 heures », « Jusqu'à ce que je reprenne » (proposé : 2 heures) | “Duration”: “1 hour”, “2 hours”, “4 hours”, “Until I take it back” |
| Compte Steam | « Compte Steam » : « Son compte Steam » (proposé), « Votre compte Steam » | “Steam account”: “Their Steam account”, “Your Steam account” |
| Bouton | « Créer un code » | “Create a code” |
| Code, lien, QR code | comme le partage (« 123 456 », « Copier le lien », QR code, lien du relais en préparation) ; dessous « Pour une séance, valable 24 h. » | as for sharing; “For one session, valid for 24 h.” |
| Aide | « Votre ami entre ce code avec « Rejoindre avec un code » dans KyberNow Link, ou ouvre ce lien dans un navigateur. » | “Your friend enters this code with “Join with a code” in KyberNow Link, or opens this link in a browser.” |
| Code créé depuis la session | « Votre session se fermera quand votre ami arrivera. » | “Your session will close when your friend arrives.” |
| Annuler | « Annuler le code » | “Cancel code” |
| Avertissement (une fois, et dans Admin) | « Votre ami joue dans votre session Windows. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance. » | “Your friend plays in your Windows session. They see neither your desktop nor your windows, but a game can read your files: lend to friends you trust.” |
| Prêt coupé dans Admin | « Le prêt est coupé dans Admin (page « Prêt »). » | “Lending is off in Admin (“Lending” page).” |
| Station trop ancienne | « Le prêt demande KyberNow Station 0.9.0 ou plus récente. » | “Lending needs KyberNow Station 0.9.0 or later.” |

**Les demandes** (ordinateur utilisé) : avis « {prénom} veut emprunter l'ordinateur. » / “{name} wants to borrow the computer.” avec « Accepter » / “Accept” et « Refuser » / “Decline” (notification de l'icône, barre de session, Admin) ; dessous « {app} · hors de chez vous » si l'ami passe par le relais. Accepter depuis sa session : « Votre session va se fermer. » / “Your session will close.” Créneau : « Créneau de {prénom} : l'ordinateur lui est prêté dans 2 min. » / “{name}'s time slot: the computer is lent to them in 2 min.”

**Pendant un prêt, chez l'hôte** :

| Élément | Français | English |
|---|---|---|
| État de l'ordinateur (accueil des apps Link, icône, Admin) | « Prêté à {prénom} » ; avec une fin : « Prêté à {prénom} jusqu'à 18 h » | “Lent to {name}”; “Lent to {name} until 6 PM” |
| Bouton principal | « Reprendre l'ordinateur » | “Take back the computer” |
| Préavis en cours | « Reprise dans 1:42 » et « Reprendre tout de suite » | “Taking back in 1:42”, “Take back now” |
| Écran du PC (cache) | logo KyberNow Share ; « Ordinateur prêté à {prénom} » ; « Jusqu'à 18 h » s'il y a une fin ; « Pour reprendre l'ordinateur, appuyez sur Entrée. » | “Computer lent to {name}”; “Until 6 PM”; “To take back the computer, press Enter.” |
| Écran du PC, après Entrée | « Reprendre l'ordinateur ? {prénom} a 2 minutes pour sauvegarder. » ; « Entrée : reprendre · Échap : annuler » | “Take back the computer? {name} has 2 minutes to save.”; “Enter: take back · Esc: cancel” |
| Écran du PC, préavis | « Reprise dans 1:42 · Entrée : tout de suite » | “Taking back in 1:42 · Enter: right now” |

**L'ami** :

| Élément | Français | English |
|---|---|---|
| Carte sur l'accueil | « Ordinateur de {hôte} » ; état « Disponible », « Occupé », « Prêté à quelqu'un d'autre », « Verrouillé », « Prêt coupé », « Hors ligne » ; avec un créneau : « Votre créneau : mercredi, 14 h – 18 h » ou « Votre créneau, jusqu'à 18 h » | “{host}'s computer”; “Available”, “In use”, “Lent to someone else”, “Locked”, “Lending off”, “Offline”; “Your time slot: Wednesday, 2 PM – 6 PM”, “Your time slot, until 6 PM” |
| Bouton principal | « Emprunter » ; occupé : « Demander à {hôte} » | “Borrow”; “Ask {host}” |
| Ajouter | « Ajouter un ordinateur » › « Avec un compte d'ami » : « Identifiant », « Mot de passe », « Retenir le mot de passe » ; lien d'invitation : « {hôte} vous prête son ordinateur. » et « Ajouter » | “Add a computer” › “With a friend account”: “Username”, “Password”, “Remember password”; “{host} is lending you their computer.”, “Add” |
| Attente de l'accord | « En attente de {hôte}… » et « Annuler » | “Waiting for {host}…”, “Cancel” |
| Préparation | étapes de la connexion, avec « Préparation de l'ordinateur » en première étape | “Preparing the computer” |
| Barre | « Prêté par {hôte} » ; « Encore 1 h 20 » ou « Jusqu'à ce que {hôte} reprenne » ; « Rendre l'ordinateur » (confirmation : « Rendre l'ordinateur à {hôte} ? ») ; latence et statistiques, son, plein écran, souris et touches, manettes ; pas de profil de la Station ni de presse-papiers | “Lent by {host}”; “1 h 20 left”, “Until {host} takes it back”; “Give back the computer” (“Give the computer back to {host}?”) |
| Lanceur | logo KyberNow Share ; une tuile par app ; « Encore 1 h 20 » ; « Rendre l'ordinateur » | as in the bar |
| Préavis | « {hôte} reprend l'ordinateur dans 2 min : sauvegardez votre partie. » ; « Fin du prêt dans 2 min : sauvegardez votre partie. » ; « Votre créneau se termine dans 2 min : sauvegardez votre partie. » | “{host} is taking back the computer in 2 min: save your game.”; “The loan ends in 2 min: save your game.”; “Your time slot ends in 2 min: save your game.” |
| Fin | « {hôte} a repris l'ordinateur. » ; « Le prêt est terminé. » ; « Votre créneau est terminé. » ; carte de fin avec « Fermer » | “{host} took back the computer.”; “The loan is over.”; “Your time slot is over.”; “Close” |
| Refus | « {hôte} a refusé. » ; « {hôte} n'a pas répondu. » ; « Identifiant ou mot de passe refusé. » ; « Ce code ne sert plus. » ; « L'ordinateur est déjà prêté. » ; « L'ordinateur est verrouillé. » ; « Le prêt est coupé sur cet ordinateur. » | “{host} declined.”; “{host} didn't answer.”; “Username or password refused.”; “This code is no longer valid.”; “The computer is already lent.”; “The computer is locked.”; “Lending is off on this computer.” |
| Retour possible | fenêtre fermée ou réseau perdu : « Vous pouvez revenir pendant 15 min. » sur la carte de fin, avec « Revenir » | “You can come back for 15 min.”, “Come back” |

**Admin, page « Prêt »** / “Lending” : « Prêter l'ordinateur » / “Lend the computer” (interrupteur) ; « Apps permises » / “Allowed apps” (« Steam », « Ajouter une app » / “Add an app”) ; « Comptes d'amis » / “Friend accounts” (« Ajouter un ami » / “Add a friend”, « Prénom » / “First name”, « Identifiant » / “Username”, « Mot de passe » / “Password”, « Compte Steam proposé » / “Suggested Steam account”, « Créneaux » / “Time slots”, « Ajouter un créneau » / “Add a time slot”, « Lien d'invitation » / “Invitation link”, « Supprimer » / “Delete”) ; « Prêt en cours » / “Current loan”.

**Textes de la Station** (Admin, icône, lanceur, client web ; à reprendre dans les apps quand elles en ont besoin) :

| Élément | Français | English |
|---|---|---|
| Temps restant (lanceur, barre) | « Encore {m} min » ; « Encore {h} h » ; « Encore moins d'une minute » | “{m} min left”; “{h} h left”; “Less than a minute left” |
| Heures | « 18 h », « 18 h 30 » | “6 PM”, “6:30 PM” |
| Lanceur | « Ouverture de {app}… » ; sans app : « Aucune app pour l'instant. » ; confirmation de « Rendre l'ordinateur » : « Annuler » | “Opening {app}…”; “No apps yet.”; “Cancel” |
| Menu de l'icône, pendant un préavis | « Prêté à {prénom} · Reprise dans 1:42 » | “Lent to {name} · Taking back in 1:42” |
| Panneau de l'icône | « Quand votre ami arrive, l'écran de ce PC est couvert et vos fenêtres sont réduites jusqu'à la fin du prêt. » ; QR code : « QR code du prêt » ; erreur : « Admin n'a pas pu changer le prêt ({error}). » | “When your friend arrives, this PC's screen is covered and your windows are minimized until the loan ends.”; “QR code for the loan”; “Admin could not change the loan ({error}).” |
| Accueil d'Admin (carte) | « Confiez l'ordinateur à un ami quand vous ne vous en servez pas : un code pour une séance, avec son lien et son QR code. » | “Hand the computer to a friend while you're not using it: a code for one session, with its link and QR code.” |
| Admin, navigation | « Prêt » ; « {n} demande(s) d'emprunt » ; « Demande d'emprunt » | “Lending”; “{n} borrow request(s)”; “Borrow request” |
| Page « Prêt », introduction | « Prêtez l'ordinateur à un ami quand vous ne vous en servez pas : il joue depuis son appareil, avec les apps que vous choisissez. À la fin, tout revient comme avant et la session Windows se verrouille. » ; sous l'interrupteur : « Tant que le prêt est activé, la session Windows doit rester ouverte : la Station empêche la mise en veille et le verrouillage automatique. » | “Lend the computer to a friend while you're not using it: they play from their own device, with the apps you choose. Afterwards, everything goes back to how it was and the Windows session locks.”; “While lending is on, the Windows session must stay open: the Station keeps the computer from sleeping and locking on its own.” |
| Apps permises | « Ce que votre ami peut ouvrir depuis le lanceur. Le reste est bloqué. » ; Steam : « En Big Picture, avec le compte Steam choisi à chaque prêt. », « Steam n'est pas installé sur cet ordinateur. » ; « Retirer {app} » ; « Aucune autre app pour l'instant. » ; « Votre ami pourra l'ouvrir depuis le lanceur. Les apps et leurs icônes viennent du menu Démarrer de l'ordinateur. » ; « {app} est permise. », « {app} n'est plus permise. » | “What your friend can open from the launcher. Everything else is blocked.”; “In Big Picture, with the Steam account chosen for each loan.”, “Steam isn't installed on this computer.”; “Remove {app}”; “No other app yet.”; “Your friend can open it from the launcher. Apps and their icons come from the computer's Start menu.”; “{app} is allowed.”, “{app} is no longer allowed.” |
| Comptes d'amis | « Un compte par ami : il emprunte avec son identifiant et son mot de passe, sans code. » ; « 3 à 32 caractères : lettres minuscules, chiffres, point, tiret, trait bas. » ; « Laisser vide pour le garder » ; « 6 caractères au moins. » ; « Modifier », « Enregistrer », « Annuler » ; « Supprimer le compte de {prénom} ? Il ne pourra plus emprunter l'ordinateur. » ; « Compte de {prénom} enregistré. », « Compte de {prénom} supprimé. », « Aucun compte d'ami pour l'instant. » ; « dernier emprunt {il y a} », « jamais emprunté » | “One account per friend: they borrow with their username and password, no code needed.”; “3 to 32 characters: lowercase letters, digits, dot, hyphen, underscore.”; “Leave empty to keep it”; “At least 6 characters.”; “Edit”, “Save”, “Cancel”; “Delete {name}'s account? They will no longer be able to borrow the computer.”; “{name}'s account saved.”, “{name}'s account deleted.”, “No friend accounts yet.”; “last borrowed {ago}”, “never borrowed” |
| Créneaux | « Pendant son créneau, votre ami emprunte sans vous demander, et passe avant vous. » ; « Jours », « Début », « Fin » ; « Retirer le créneau » ; « Chaque créneau a au moins un jour, et finit après son début le même jour. » ; « {jours}, {début} – {fin} » ; « Sans créneau : il demande quand l'ordinateur est utilisé » | “During their time slot, your friend borrows without asking you, and goes before you.”; “Days”, “Start”, “End”; “Remove time slot”; “Each time slot has at least one day, and ends after it starts on the same day.”; “{days}, {start} – {end}”; “No time slot: they ask when the computer is in use” |
| Invitation | « Votre ami scanne ce QR code avec KyberNow Link, ou ouvre ce lien dans un navigateur. Il emprunte avec l'identifiant {identifiant} et son mot de passe. » ; « QR code de l'invitation de {prénom} » | “Your friend scans this QR code with KyberNow Link, or opens this link in a browser. They borrow with the username {login} and their password.”; “QR code of {name}'s invitation” |
| Prêt en cours, ami déconnecté | « déconnecté, peut revenir pendant 15 min » | “disconnected, can come back for 15 min” |
| Ami, client web | titre avant de connaître l'hôte : « Emprunter l'ordinateur » ; fin : « Vous avez rendu l'ordinateur. », « Le prêt s'est arrêté. » ; « Trop d'essais : réessayez dans une minute. » | “Borrow the computer”; “You gave back the computer.”, “The loan has stopped.”; “Too many attempts: try again in a minute.” |

**Fil d'Admin** : « Ordinateur prêté à {prénom} » / “Computer lent to {name}” ; « {prénom} a rendu l'ordinateur » / “{name} gave back the computer” ; « Vous avez repris l'ordinateur » / “You took back the computer” ; « {prénom} veut emprunter l'ordinateur » / “{name} wants to borrow the computer” ; « {prénom} : demande refusée » / “{name}: request declined”.

## Hors de chez vous (relais)

Contrat : [relais.md](relais.md). Par le relais, une session ressemble à une session sur le réseau ; seuls ces textes changent.

| Élément | Français | English |
|---|---|---|
| Barre de session, par le relais | puce « Par le relais », dans la partie toujours visible de la barre (une tuile dans les menus à tuiles) | “Via the relay” |
| Client web, Réglages › Réseau et Statistiques | « WebSocket par le relais : hors de chez vous, l'image passe par le relais de KyberNow (un peu plus de latence). » ; « Image par le relais, en WebSocket : un peu plus de latence qu'en WebTransport » | “WebSocket through the relay: away from home, the picture goes through the KyberNow relay (a little more latency).”; “Picture through the relay, over WebSocket: a little more latency than WebTransport” |
| Statistiques, ligne « Chemin » | « Direct », « Par le relais », « Par le relais (TCP) » (repli quand l'UDP ne passe pas) | “Path”: “Direct”, “Via the relay”, “Via the relay (TCP)” |
| Accueil, joignable seulement par le relais | état « Hors de chez vous » (à la place de « En ligne ») | “Away from home” |
| Refus, accès coupé | « {nom} n'est pas joignable hors de chez vous (accès coupé dans Admin). » | “{name} cannot be reached away from home (access turned off in Admin).” |
| Refus, Station absente du relais | « {nom} n'est pas joignable : l'ordinateur est éteint ou en veille. » | “{name} cannot be reached: the computer is off or asleep.” |
| Quand dire ces refus | quand on ouvre l'ordinateur hors de chez soi (au lieu de la connexion), avec son nom ; l'accueil dit seulement l'état | |
| Par le relais, pas d'action lourde | ni redémarrage ni mise à jour de la Station depuis l'app hors de chez soi (Admin les refuse par le relais) ; à la place : « Hors de chez vous, l'app ne peut ni redémarrer ni mettre à jour la Station. » | “Away from home, the app can neither restart nor update the Station.” |
| Invité en attente, partage arrêté | « Ce partage est arrêté. » | “This share has stopped.” |

**Admin** (rubrique des appareils Link) :

| Élément | Français | English |
|---|---|---|
| Section | « Hors de chez vous » | “Away from home” |
| Champ | « Relais » (défaut `share.kybernow.app` ; sans port, c'est le 443 ; un relais installé ailleurs peut porter son port, `relais.exemple.org:9443`), aide « Le relais fait passer le partage et vos appareils par internet, hors de chez vous. Laissez vide pour ne pas en utiliser. », bouton « Enregistrer » (puis « Enregistré. ») | “Relay”, help “The relay carries sharing and your devices over the internet, away from home. Leave it empty to use none.”, “Save” (then “Saved.”) |
| État | « Relais joint » ; « Relais injoignable ({raison}) », ou « Relais injoignable » sans raison ; seulement quand une adresse est réglée ; aussi dans le menu de l'icône, quand un partage ou l'accès demande le relais | “Relay reached”; “Relay unreachable ({reason})”, “Relay unreachable” |
| Interrupteur | « Accès hors de chez vous », aide « Vos appareils appairés ouvrent cet ordinateur par le relais quand ils ne sont pas sur votre réseau. » | “Access away from home”, help “Your paired devices open this computer through the relay when they are not on your network.” |
| Fil | « {appareil} est en session hors de chez vous » ; « Accès hors de chez vous activé », « Accès hors de chez vous coupé » | “{device} is in a session away from home”; “Access away from home turned on”, “Access away from home turned off” |
| Blocage | une adresse (dix essais faux) : dans le fil, « Accès hors de chez vous bloqué 10 min pour {adresse} après trop d'essais. » ; tout l'accès (garde-fou, cent essais faux) : « Accès hors de chez vous bloqué 10 min après trop d'essais. », aussi dans la section | “Access away from home blocked for 10 min for {address} after too many attempts.”; “Access away from home blocked for 10 min after too many attempts.” |

## Réglages de l'ordinateur

Titre : « Réglages de {nom} » / “Settings for {name}”. En tête : « Profil » / “Profile” (Jeu, Bureau, Max, Personnalisé). Puis six rubriques, dans cet ordre (une plateforme peut en fusionner ou en cacher une si elle n'a rien à y mettre, jamais les renommer) :

| Rubrique | Français | English | Contenu |
|---|---|---|---|
| 1 | Image | Picture | définition, écrans, fréquence, écran virtuel, format vidéo, texte net (4:4:4), débit, affichage des images, garder la dernière image |
| 2 | Son | Sound | voies, son sur l'ordinateur, et ce que la plateforme propose (audio spatial, 5.1 Dolby…) |
| 3 | Commandes | Controls | manette vue par l'ordinateur, souris, clavier, toucher, télécommande, vibrations, presse-papiers partagé |
| 4 | Réseau | Network | transport de l'image, reconnexion automatique, réveil par le réseau |
| 5 | Statistiques | Statistics | dernière session, cet ordinateur |
| 6 | Station | Station | Admin, redémarrer, mettre à jour, compte, retirer cet ordinateur |

Ce qui vaut pour toute l'app (langue, iCloud, mises à jour de l'app, test des manettes, ce que l'appareil sait décoder) va dans les **Réglages de l'app**, pas ici.

### Libellés des réglages

| Français | English | Valeurs (FR / EN) |
|---|---|---|
| Définition | Resolution | Comme cet écran / Match this display ; Au plus 4K / 4K at most ; Sur mesure / Custom |
| Écrans | Displays | Auto ; Cet écran / This display ; Tous les écrans / All displays ; aide : « Auto : tous les écrans de cet appareil en Bureau et Max, un seul en Jeu et en Personnalisé. Chaque écran reçoit un écran de l'ordinateur, placé de la même façon. » / “Auto: all of this device's displays in Desktop and Max, one in Game and Custom. Each display gets a display on the computer, laid out the same way.” Inactif, sur « Cet écran », avec une définition sur mesure (aide : « Avec une définition sur mesure, la session prend un seul écran. » / “With a custom resolution, the session takes a single display.”), avec une session dans une fenêtre (même aide que la définition sur mesure, la fenêtre fixant la taille de l'écran) et sans écran virtuel (réglage coupé, ou Station sans écran virtuel utilisable : alors sans aide, la ligne grisée sur « Cet écran » se suffit et l'écran virtuel a la sienne ; avec une définition sur mesure, l'aide de la définition sur mesure reste). Apps Mac, Windows et Linux ([multi-ecrans.md](multi-ecrans.md)) |
| Fréquence | Refresh rate | « 120 Hz » |
| Écran virtuel | Virtual display | aide : « L'ordinateur crée un écran à la taille de cet appareil pendant la session ; les siens s'éteignent, puis reviennent à la fin. » / “The computer creates a display sized for this device during the session; its own displays switch off, then come back at the end.” |
| Format vidéo | Video format | Auto ; HEVC ; H.264 ; AV1 |
| Texte net (4:4:4) | Sharp text (4:4:4) | aide : « Couleurs complètes : le texte reste net, au prix d'un débit plus élevé. » / “Full colour: text stays sharp, at the cost of a higher bitrate.” |
| Débit | Bitrate | Auto ; « 80 Mbit/s » |
| Affichage des images | Frame display | Synchronisé / Synced ; Immédiat / Immediate |
| Garder la dernière image | Keep the last image | |
| Son | Sound | Auto ; Stéréo / Stereo ; 5.1 ; 7.1 ; Sur l'ordinateur / On the computer |
| Manette vue par l'ordinateur | Controller seen by the computer | Xbox 360 ; Fidèle / As is |
| Souris | Mouse | Auto ; Souris du bureau / Desktop mouse ; Souris des jeux / Game mouse |
| Presse-papiers partagé | Shared clipboard | |
| Vibrations | Rumble | |
| Transport de l'image | Picture transport | Rapide / Fast ; Fiable / Reliable ; aide : « Rapide convient au réseau local et au jeu. Fiable renvoie ce qui se perd, au prix d'un peu de délai. » / “Fast suits your home network and gaming. Reliable resends what gets lost, at the cost of a little delay.” |
| Reconnexion automatique | Reconnect automatically | |
| Réveil par le réseau | Wake on LAN | « Prêt » avec l'adresse MAC apprise de la Station, ou « Pas encore d'adresse : connectez-vous une fois à cet ordinateur. » ; aide : « Allume l'ordinateur à distance quand il est en veille. Il faut aussi l'autoriser sur l'ordinateur (Admin › Système). » |

## Statistiques

**Dernière session** / “Last session”, sur le modèle de la carte de l'app Mac (`FrameRuler`) :

- en grand, la médiane : « 8,9 ms » et « médiane » / “median” ;
- une règle en millisecondes, de 0 à 20 ms (plus si la mesure dépasse), avec les repères « 160 Hz », « 120 Hz » et « 60 Hz » à la durée d'une image à ces fréquences (6,25 ; 8,33 ; 16,67 ms), et une bande de la médiane au 95e centile ;
- « 95 % des images sous 14,2 ms » / “95% of frames under 14.2 ms” ;
- la durée et le moment : « 42 min · hier » / “42 min · yesterday” ;
- ce qui a été transmis : « 2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s » ; avec plusieurs écrans, leur nombre d'abord, puis l'écran principal et le débit total : « 3 écrans · 3840 × 2160 · 160 Hz · HEVC · 120 Mbit/s » ; ce nombre est celui des écrans que l'ordinateur a pris (ceux qu'il n'a pas pu créer n'y sont pas), le même dans la barre de session, les statistiques de session et la dernière session ;
- aide : « Le délai entre l'ordinateur et cet écran. Sous un repère, l'image arrive avant l'image suivante à cette fréquence. » / “The delay between the computer and this screen. Below a mark, the picture arrives before the next frame at that refresh rate.”
- sans session : « Pas encore de session sur cet ordinateur. » / “No session on this computer yet.”

Données gardées par l'app pour chaque ordinateur, à la fin de chaque session d'au moins 3 s : médiane, 95e centile, durée, date, nombre d'écrans, définition et fréquence de l'écran principal, format, débit.

**Cet ordinateur** / “This computer” : « Système » / “System”, « Carte graphique » / “Graphics card”, « Adresse » / “Address” (adresse et port d'Admin), « Empreinte » / “Fingerprint” (quatre premiers groupes).

## Station

| Élément | Français | English |
|---|---|---|
| Version | « KyberNow Station 0.5.0 » puis « À jour » ou « Version 0.5.1 disponible » | “KyberNow Station 0.5.0”, “Up to date” or “Version 0.5.1 available” |
| Admin, appareil de bureau ou à main | bouton « Ouvrir Admin » ; aide : « Admin s'ouvre dans votre navigateur et demande son mot de passe. Le navigateur peut signaler le certificat : la Station signe elle-même sa connexion, c'est normal. » | “Open Admin”; “Admin opens in your browser and asks for its password. The browser may warn about the certificate: the Station signs its own connection, this is expected.” |
| Admin, téléviseur et salon | QR code de `https://adresse:port/` ; « Scannez ce code avec votre téléphone pour ouvrir Admin. » et l'adresse en clair | “Scan this code with your phone to open Admin.” |
| Redémarrer | bouton « Redémarrer la Station » ; aide : « Environ 30 secondes. Une session en cours sera coupée. » | “Restart the Station”; “About 30 seconds. A session in progress will be cut off.” |
| Mettre à jour (version installable) | bouton « Mettre à jour la Station » ; aide : « Version {v}. Environ une minute. Une session en cours sera coupée. » | “Update the Station”; “Version {v}. About a minute. A session in progress will be cut off.” |
| Version qui exige l'installateur | « La version {v} s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe. » | “Version {v} installs on the computer, with KyberNow-Station-Setup.exe.” |
| Un appareil est en session | titre « {appareil} est connecté », message « Redémarrer la Station coupera sa session. » (ou « La mise à jour coupera sa session. »), boutons « Redémarrer quand même » / « Mettre à jour quand même » et « Annuler » | “{device} is connected”, “Restarting the Station will end its session.” / “The update will end its session.”, “Restart anyway” / “Update anyway”, “Cancel” |
| Pendant | « La Station redémarre… », « Mise à jour de la Station… » | “The Station is restarting…”, “Updating the Station…” |
| Après | « La Station a redémarré. », « KyberNow Station {v} est en place. » | “The Station has restarted.”, “KyberNow Station {v} is installed.” |
| Trop tôt | « La Station vient de redémarrer. Réessayez dans une minute. » | “The Station has just restarted. Try again in a minute.” |
| La Station tarde à revenir | « La Station tarde à revenir. Ouvrez Admin pour en savoir plus. » | “The Station is taking a while to come back. Open Admin to find out more.” |
| La mise à jour échoue | « La mise à jour n'a pas abouti. Ouvrez Admin pour en savoir plus. » | “The update did not complete. Open Admin to find out more.” |
| Station trop ancienne | « Pour redémarrer ou mettre à jour la Station depuis cet appareil, mettez-la d'abord à jour depuis Admin. » | “To restart or update the Station from this device, first update it from Admin.” |
| Compte | « Compte » : l'identifiant ; appareil ajouté par code : « Accès propre à cet appareil » | “Account”; “This device's own access” |
| Retirer | bouton « Retirer cet ordinateur » ; confirmation « Retirer {nom} de cet appareil ? Vous pourrez l'ajouter à nouveau. », boutons « Retirer » et « Annuler » | “Remove this computer”; “Remove {name} from this device? You can add it again later.”, “Remove”, “Cancel” |

Le redémarrage et la mise à jour passent par Admin (`POST /api/stream/station/restart` et `/update`, voir [server/README.md](../server/README.md)) ; l'app suit ensuite l'état de la Station par `GET /api/stream/info` jusqu'à son retour. Tout appareil ajouté peut le faire.

Le réveil par le réseau : Admin donne dans `GET /api/stream/info` les cartes réseau de l'ordinateur (adresse MAC, adresse IPv4, adresse de diffusion) ; l'app les garde pour chaque ordinateur et le cœur envoie le paquet de réveil. Sur iPhone, iPad et Apple TV, l'envoi en diffusion demande une autorisation d'Apple : l'app envoie aussi le paquet directement à la dernière adresse connue, qui ne réveille pas toujours.

## Icône près de l'horloge (Mac, Windows)

Les apps Mac et Windows ont une icône près de l'horloge : dans la barre des menus sur Mac, dans la zone de notification sur Windows. Elle reste tant que l'app tourne, fenêtre ouverte ou fermée, et donne accès à tout ce qu'il faut pour se connecter vite.

- **Dessin** : le symbole de KyberNow Link seul (état `now` de [brand/v2](../brand/v2/README.md)), d'une seule couleur : image modèle sur Mac (le système la teinte), blanche ou noire selon le thème de la barre des tâches sur Windows. Il ne ressemble pas à l'icône de KyberNow Station (la tuile en couleur et sa pastille) : sur un PC qui a les deux, deux icônes. En session, le symbole s'allume (variante distincte, lisible en petit). Infobulle : « KyberNow Link », en session « KyberNow Link · en session sur {nom} » / “KyberNow Link · in session on {name}”, sur plusieurs ordinateurs « KyberNow Link · en session sur {n} ordinateurs » / “KyberNow Link · in session on {n} computers”.
- **Clic** : sur Mac, un clic ouvre le menu. Sur Windows, clic droit ou gauche ouvre le même menu (au clic gauche, après le délai du double clic de Windows) ; un double clic ouvre la fenêtre.

### Menu

De haut en bas. Pendant une session, d'abord (avec plusieurs ordinateurs, pour la session de la fenêtre active) :

| Élément | Français | English |
|---|---|---|
| En-tête (inactif) | « En session sur {nom} » | “In session on {name}” |
| Mesures (inactif, mis à jour menu ouvert) | « 12,4 ms · 60 i/s · 80 Mbit/s » : la latence comme dans la barre de session, au dixième, les images par seconde et le débit ; « — » pour ce qui n'est pas encore mesuré ; avant la première image, l'étape de la connexion | “12.4 ms · 60 fps · 80 Mbit/s” |
| Plein écran (coché en plein écran ; absent avec plusieurs écrans) | « Plein écran » | “Full screen” |
| Souris | « Souris » ▸ « Auto », « Souris du bureau », « Souris des jeux » (l'ordre des réglages), le mode de l'ordinateur coché ; choisir un mode l'applique tout de suite et le garde pour cet ordinateur | “Mouse” ▸ “Auto”, “Desktop mouse”, “Game mouse” |
| Mesures en direct | « Statistiques » (Mac : le panneau des mesures ; Windows : la barre de session reste affichée jusqu'au passage du pointeur, un second clic la cache) | “Statistics” |
| Fin | « Se déconnecter » | “Disconnect” |

Puis vos ordinateurs, dans l'ordre de l'accueil : « {nom} — En ligne », « {nom} — Hors ligne », « {nom} — Recherche… » (“{name} — Online”, “Offline”, “Searching…”), chacun précédé d'un petit point de couleur : vert en ligne, rouge hors ligne, ambre pendant le réveil et pendant un redémarrage ou une mise à jour de la Station, gris pendant la recherche (couleurs d'état de [brand/v2](../brand/v2/tokens/kybernow.css), `--kn-ok`, `--kn-bad`, `--kn-warn`, `--kn-faint`, en version claire ou sombre selon le menu) ; le texte de l'état reste, la couleur seule ne suffit pas à tout le monde. Ceux qui sont en session sont cochés ; leur sous-menu commence par « Afficher » / “Show”, « Garder le son » / “Keep sound” (à partir de deux sessions ouvertes, coché quand il est allumé) et « Se déconnecter » / “Disconnect”. Chacun ouvre un sous-menu :

| Élément | Français | English |
|---|---|---|
| En ligne (et « Recherche… », comme le bouton de l'accueil) | « Se connecter ({profil}) », le profil de l'ordinateur ; inactif quand quatre ordinateurs sont déjà en session ; sur un ordinateur en session : voir ci-dessus | “Connect ({profile})”; “Disconnect” |
| Hors ligne | « Réveiller l'ordinateur » quand l'app connaît son adresse MAC, sinon « Réessayer » ; pendant le réveil : « Réveil de l'ordinateur… », inactif ; pendant un redémarrage ou une mise à jour de la Station : « Se connecter ({profil}) », inactif (elle revient d'elle-même) | “Wake the computer”, “Try again”, “Waking the computer…” |
| Profil | « Profil » ▸ « Jeu », « Bureau », « Max », celui de l'ordinateur coché ; « Personnalisé » coché et inactif quand les réglages s'en écartent. Choisir un profil l'applique comme sur l'accueil (en session, tout de suite, comme le menu de la barre) | “Profile” ▸ “Game”, “Desktop”, “Max”, “Custom” |
| Réglages | « Réglages » (Mac : « Réglages… ») : la fenêtre s'ouvre sur les réglages de l'ordinateur | “Settings” (Mac: “Settings…”) |
| Admin (en ligne) | « Ouvrir Admin » | “Open Admin” |
| Redémarrer (en ligne, Station 0.5.0 et après) | « Redémarrer la Station » ; pendant : « La Station redémarre… », inactif | “Restart the Station”; “The Station is restarting…” |
| Mettre à jour (version qui s'installe depuis un appareil) | « Mettre à jour la Station ({v}) » ; pendant : « Mise à jour de la Station… », inactif | “Update the Station ({v})”; “Updating the Station…” |

Les groupes du menu sont séparés d'un trait. Pendant une session, « Réglages » et « Ajouter un ordinateur » montrent l'accueil par-dessus la session (Mac et Windows : l'accueil ne sert plus de fenêtre de session).

Redémarrer et mettre à jour suivent la rubrique **Station** : même confirmation quand un appareil est en session (« {appareil} est connecté », « Redémarrer quand même », « Annuler »), même suivi, mêmes textes à la fin.

En bas :

| Élément | Français | English |
|---|---|---|
| Fenêtre | « Ouvrir KyberNow Link » | “Open KyberNow Link” |
| Ajout | « Ajouter un ordinateur » (Mac : « Ajouter un ordinateur… ») | “Add a computer” (Mac: “Add a computer…”) |
| Réglages de l'app | « Réglages de l'app » (Mac : « Réglages de l'app… ») | “App settings” (Mac: “App settings…”) |
| Mise à jour de l'app | Mac : « Rechercher des mises à jour… » ; Windows : « Version {v} disponible » quand une version attend (ouvre les réglages de l'app), sinon « Rechercher des mises à jour » (ouvre les réglages de l'app et lance la recherche) | “Check for updates…”; “Version {v} available”; “Check for updates” |
| Quitter | « Quitter KyberNow Link » ; les sessions en cours se terminent d'abord | “Quit KyberNow Link” |

### Fenêtre et démarrage

- Fermer la fenêtre de l'accueil ne quitte pas l'app : elle continue dans l'icône. On la quitte par « Quitter KyberNow Link » (Mac : aussi ⌘Q et le Dock). Fermer la fenêtre de la session, c'est toujours se déconnecter.
- La première fois que la fenêtre se ferme, une notification le dit, une seule fois : « KyberNow Link est toujours là » / “KyberNow Link is still here”, « Vos ordinateurs vous attendent dans la barre des menus. » / “Your computers are waiting in the menu bar.” (Windows : « … dans la zone de notification, près de l'horloge (sous la flèche ^ si Windows l'a rangée). » / “… in the notification area, next to the clock (under the ^ arrow if Windows tucked it away).” : Windows 11 range d'abord une nouvelle icône sous cette flèche).
- Mac : l'icône du Dock n'est là que tant qu'une fenêtre est ouverte (accueil, réglages, session) ; sans fenêtre, l'app ne vit que dans la barre des menus. Rouvrir l'app (Finder, Launchpad, Spotlight) ouvre l'accueil.
- **Ouvrir à l'ouverture de session** / “Open at login”, dans les Réglages de l'app, coché par défaut (dès le premier lancement de la version qui l'apporte ; une désactivation faite dans le système, Réglages Système ou Gestionnaire des tâches, est respectée). Aide : « KyberNow Link démarre avec votre Mac, sans fenêtre : vos ordinateurs vous attendent dans la barre des menus. » / “KyberNow Link starts with your Mac, without a window: your computers wait for you in the menu bar.” (Windows : « … démarre avec Windows, … dans la zone de notification. » / “… starts with Windows, … in the notification area.”). Lancée ainsi, l'app démarre sans fenêtre, dans l'icône seule.
- Fenêtre fermée, l'app continue de suivre vos ordinateurs (moins souvent qu'avec la fenêtre ouverte, et tout de suite à l'ouverture du menu) : le menu et les notifications sont à jour.

### Notifications

Des notifications du système, seulement quand l'app n'est pas au premier plan (pour la fin d'une session sans « Se déconnecter », aussi quand d'autres sessions continuent, même app au premier plan : voir « Plusieurs ordinateurs ») :

| Quand | Titre | Texte | Bouton |
|---|---|---|---|
| Une version de la Station est prête (une fois par version et par ordinateur) | « Mise à jour de la Station de {nom} » / “Station update for {name}” | « La version {v} est prête. Environ une minute ; une session en cours sera coupée. » / “Version {v} is ready. About a minute; a session in progress will be cut off.” ; version qui exige l'installateur : « La version {v} s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe. » | « Mettre à jour » / “Update” (aucun pour l'installateur) |
| L'ordinateur revient après « Réveiller l'ordinateur » | « {nom} est en ligne » / “{name} is online” | « Vous pouvez vous connecter. » / “You can connect now.” | « Se connecter » / “Connect” |
| Toujours rien au bout de 2 min de réveil | « {nom} ne répond toujours pas » / “{name} is still not responding” | « Vérifiez qu'il est branché au réseau et que le réveil est autorisé sur l'ordinateur (Admin › Système). » / “Check that it is plugged into the network and that waking is allowed on the computer (Admin › System).” | |
| Une session s'arrête sans « Se déconnecter » : un autre appareil ou Admin | « La session sur {nom} est terminée » / “The session on {name} has ended” | « Un autre appareil a pris la main, ou Admin y a mis fin. » / “Another device took over, or Admin ended it.” | « Reprendre la main » / “Take back control” |
| … une erreur | « La session sur {nom} s'est arrêtée » / “The session on {name} stopped” | la raison, en une phrase | « Reprendre » / “Resume” |
| Fin d'un redémarrage ou d'une mise à jour lancés depuis le menu, ou refus | le nom de l'ordinateur | les textes de la rubrique Station (« La Station a redémarré. », « KyberNow Station {v} est en place. », « La Station tarde à revenir… », « La mise à jour n'a pas abouti… », « La Station vient de redémarrer… », Station trop ancienne) | |

Un clic sur la notification ouvre l'app sur ce qu'elle annonce (l'accueil sur l'ordinateur, ou sa rubrique Station).

## Où sont les textes

Chaque app garde ses catalogues ([i18n.md](i18n.md), « Fichiers ») ; les textes ci-dessus y sont recopiés à l'identique. Un texte propre à une plateforme suit les mêmes règles.
