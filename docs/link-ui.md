# KyberNow Link : écrans et textes communs

[English](link-ui.en.md) · Français

Toutes les apps KyberNow Link (Mac, Apple TV, iPhone et iPad, Windows, Android et Android TV, téléviseurs LG, Linux, Steam Deck, Raspberry Pi et KyberNow Link OS, client web `/ecran`) suivent ce document : mêmes écrans, même ordre, mêmes mots. Chaque plateforme garde ses spécificités (télécommande, manette, toucher, fenêtres, QR code au téléviseur), mais une même chose porte le même nom partout. Le ton est celui du site ([ton-du-site.md](ton-du-site.md)) ; les noms et la typographie, ceux de [i18n.md](i18n.md).

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
| État | « En ligne », « Hors ligne », « Recherche… » | “Online”, “Offline”, “Searching…” |
| Nom | le nom de l'ordinateur | |
| Système | « Windows 11 Pro » (tel que la Station le donne) | |
| Écran, écran virtuel actif, à la taille de cet appareil | « Écran de 2560 × 1440 à 120 Hz, à la taille de cet écran » | “2560 × 1440 display at 120 Hz, sized for this screen” |
| Écran, sur mesure | « Écran de 1920 × 1080 à 60 Hz » | “1920 × 1080 display at 60 Hz” |
| Écran, sans écran virtuel | « Les écrans de l'ordinateur, tels quels » | “The computer's own displays, as they are” |
| Bouton principal, en ligne | « Se connecter » | “Connect” |
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
| 1 | « Préparation de l'écran » | “Preparing the display” |
| 2 | « Connexion » | “Connecting” |
| 3 | « Premières images » | “First frames” |
| Échec | « La connexion a échoué », puis la raison en une phrase | “The connection failed”, then the reason |
| Bouton | « Annuler » | “Cancel” |

Si Admin répond mais que l'image n'arrive pas malgré les réparations de la session (image clé, relance, débit réduit, 4:2:0), ou que la connexion au moteur de streaming échoue, l'app redémarre la Station, une fois, sans couper la session d'un autre appareil, attend son retour (1 min 30 au plus) et retente la connexion une fois. L'écran de connexion le dit : « La Station redémarre… » / “The Station is restarting…”, puis reprend ses étapes. Si l'image ne vient toujours pas : « La connexion a échoué », et la raison.

Dès la première image, l'image seule. Pas de capsule d'arrivée, pas de barre qui s'affiche puis se cache, pas de chiffre. Le titre de fenêtre (Windows) est « {nom} · KyberNow Link » (la ligne de commande garde ses mesures dans le titre, faute de barre).

En session : « Se déconnecter » / “Disconnect” (barre, menu, raccourci), « Statistiques » / “Statistics” pour les mesures en direct, « Touches » / “Keys”, « Plein écran » / “Full screen”.

## Réglages de l'ordinateur

Titre : « Réglages de {nom} » / “Settings for {name}”. En tête : « Profil » / “Profile” (Jeu, Bureau, Max, Personnalisé). Puis six rubriques, dans cet ordre (une plateforme peut en fusionner ou en cacher une si elle n'a rien à y mettre, jamais les renommer) :

| Rubrique | Français | English | Contenu |
|---|---|---|---|
| 1 | Image | Picture | définition, fréquence, écran virtuel, format vidéo, texte net (4:4:4), débit, affichage des images, garder la dernière image |
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
- ce qui a été transmis : « 2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s » ;
- aide : « Le délai entre l'ordinateur et cet écran. Sous un repère, l'image arrive avant l'image suivante à cette fréquence. » / “The delay between the computer and this screen. Below a mark, the picture arrives before the next frame at that refresh rate.”
- sans session : « Pas encore de session sur cet ordinateur. » / “No session on this computer yet.”

Données gardées par l'app pour chaque ordinateur, à la fin de chaque session d'au moins 3 s : médiane, 95e centile, durée, date, définition, fréquence, format, débit.

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

## Où sont les textes

Chaque app garde ses catalogues ([i18n.md](i18n.md), « Fichiers ») ; les textes ci-dessus y sont recopiés à l'identique. Un texte propre à une plateforme suit les mêmes règles.
