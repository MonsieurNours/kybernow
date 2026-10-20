# Versions de KyberNow

[English](CHANGELOG.en.md) · Français

Chaque section devient les notes de la version publiée sur GitHub : la
console de l'ordinateur et l'app Mac les affichent avant d'installer.

## 0.2.6

- Apps Mac et Apple TV, image : le 4:4:4 n'est plus proposé en AV1 (les décodeurs Apple ne lisent que le 4:2:0 : le flux ne s'ouvrait pas) ; le shader Metal n'est plus compilé à chaque flux et la sonde des décodeurs ne bloque plus le lancement ; deux jeux de paramètres qui alternent ne refont plus le décodeur à chaque image ; matrice de couleur et cache de textures allégés ; sur Mac, une fenêtre occultée ou réduite arrête le rendu et baisse le débit demandé, puis les reprend.
- Apps Mac et Apple TV : la latence de bout en bout ne jette plus les images dont les mesures du serveur arrivent après l'affichage (médiane optimiste sans synchro verticale) ; le panneau des mesures montre la part des images mesurées et les mesures rejetées ; les compteurs repartent de zéro après une reconnexion.
- Apps Mac et Apple TV, son : le son voyage en non fiable avec correction d'erreurs (un paquet perdu est masqué par Opus au lieu d'être retransmis : plus de retard qui s'accumule sur Wi-Fi) ; un tampon vide est comblé par une trame de masquage plutôt que par du silence, la lecture reprend dès que la marge est là et la marge redescend cinq fois plus vite ; la dérive des horloges est rattrapée par un échantillon sauté ou doublé sur 256 au lieu d'une coupe sèche (clics) ; AirPods ou écran HDMI choisis en cours de session : la sortie suit, sa latence est relue ; 5.1 ou 7.1 vers un casque : mixage descendant au lieu du centre dans l'oreille droite ; Bluetooth et AirPlay ne font plus taire la sortie (4 096 images par demande) ; voies latérales et arrières du 7.1 étiquetées comme Core Audio l'attend.
- Apps Mac et Apple TV : le décodage de l'image est asynchrone. Le fil réseau de Kyber n'attend plus le décodeur (1 à 3 ms par image, 20 à 60 ms en pic, pendant lesquels ni les paquets ni le son n'étaient lus). Une session de décodage perdue après la veille ou un changement de carte graphique est recréée tout de suite, sans attendre la relance du flux (2 à 3 s).
- App Mac : en plein écran, l'image peut aller directement à l'écran sans passer par le compositeur de macOS (un rafraîchissement de moins) : la couche Metal est la couche de la surface de session, et la barre, les rappels, le panneau des mesures et le passage sortent de la composition quand ils n'ont rien à montrer. À vérifier avec `MTL_HUD_ENABLED=1`.
- App Mac : l'image part à l'écran dès qu'elle est décodée. L'image suivante n'attend plus que l'écran ait montré la précédente (un rafraîchissement complet de latence, 8 ms à 120 Hz, 17 ms à 60 Hz) ; Core Animation montre la plus récente au balayage. L'ancien comportement reste disponible avec `KN_INFLIGHT_GUARD=1`.
- App Mac : mesures de présentation de l'image. Lancée avec `KN_PRESENT_STATS=1`, l'app écrit chaque seconde dans le journal les images rendues, montrées et écartées par l'écran, et le délai commit → écran (médiane, 95e centile) ; `KN_INFLIGHT_GUARD=1` remet la garde « une image en vol » pour comparer. Le test d'affichage hors ligne compare les deux.
- Console : un second appareil qui se connecte prend la main avec préavis. Le premier est prévenu à son signal de vie suivant et se déconnecte proprement, puis l'écran virtuel est recréé à la taille du second ; avant, les deux appareils se volaient l'écran en boucle et la déconnexion du premier coupait l'écran et le son du second. La session est maintenant reconnue par l'appareil et non plus par le compte.
- Console : cinq mots de passe faux depuis une même adresse font attendre 30 s, puis le double à chaque essai ; les vérifications de mot de passe ne se font plus que deux à la fois.
- Console : à l'arrêt du service (mise à jour, extinction), les écrans et le son sont rendus à l'ordinateur tout de suite ; un réglage illisible n'empêche plus la console de démarrer ; un écran ou un son qui n'a pas pu être rendu est réessayé au démarrage suivant ; un agent d'affichage qui ne répond pas est arrêté ; deux changements simultanés de `kyber_config.toml` ne s'écrasent plus.
- Apps Mac et Apple TV : plus de plantage possible à la déconnexion ou au changement de profil. Les décodeurs n'étaient libérés qu'à délai fixe alors que Kyber pouvait encore leur livrer un paquet ; Kyber signale maintenant son dernier appel. La demande d'image clé ne retient plus la boucle de Kyber quand l'ordinateur ne répond pas (Quitter ne gèle plus), et sa relance tire aussi pendant un menu ouvert.
- Moteur de capture libavconv : le pointeur suit de nouveau la souris sur les écrans à plus de 60 Hz (il restait figé tant qu'elle bougeait), et garde sa forme quand le service d'entrées de Kyber redémarre.
- Images clés redemandées par les apps après une perte : une par session toutes les 150 ms au plus, pour qu'un réseau qui perd beaucoup de paquets n'alourdisse pas l'image de tous les écrans partagés.
- Moteur de capture libavconv : réglage `audio_layout = "stereo"` (section `[kyavserver]`) pour envoyer le son en stéréo même quand Windows sort du 5.1 ou du 7.1 ; le client web de Kyber, qui ne lit que la stéréo, reste sinon muet. Le son annonce aussi sa taille de paquet, sans laquelle un client Kyber en son non fiable s'arrêtait.
- Mises à jour de l'ordinateur sans risque de tout laisser arrêté : la nouvelle version est préparée à côté de l'ancienne puis mise en place d'un coup ; si la console ne répond pas dans la minute, l'ancienne version revient seule et la page Mises à jour dit pourquoi. La mise à jour à la main vérifie aussi l'empreinte du paquet, et « Applications installées » affiche la bonne version.
- Ordinateur partagé : le dossier des données de KyberNow (`C:\ProgramData\KyberNow`) n'est plus accessible qu'aux administrateurs. Un autre compte de l'ordinateur ne peut plus y lire l'empreinte du mot de passe ni glisser un fichier dans une mise à jour.
- Installateur : sur un portable sans mode « Performances élevées », l'installation ne s'arrête plus à 96 % : le mode est recréé, ou laissé tel quel avec un avertissement. L'option est décochée si l'ordinateur y est déjà.
- Installateur : Alt+F4 ne ferme plus la fenêtre en pleine installation, et KyberNow apparaît dans « Applications installées » dès les fichiers copiés : une installation interrompue se désinstalle normalement.
- Installateur : relancé sur un ordinateur où KyberNow est déjà installé, il propose de conserver les réglages (comptes, appareils appairés, ports, certificat, réglages de la console) et ne remplace que les programmes et les pilotes. « Repartir de zéro » reste possible, avec un avertissement : les appareils seront à appairer de nouveau.
- Désinstallation plus complète : le pilote d'écran virtuel quitte aussi le magasin de pilotes de Windows, le mode d'alimentation et la catégorie du réseau d'avant KyberNow sont remis, et les réglages peuvent être gardés pour plus tard.
- Installateur : l'interface s'extrait sous Program Files et non plus dans le dossier temporaire ; la fin de l'installation vérifie que le serveur et la console répondent ; une installation officielle de Kyber dont la commande de désinstallation a des arguments est bien retirée ; les comptes standard ne peuvent plus suspendre les services.
- App Apple TV : chaque redémarrage de l'image (changement de profil, repli) laissait derrière lui des observateurs du clavier et de la souris Bluetooth ; ils sont maintenant retirés.
- Apps Mac et Apple TV : réglage par ordinateur « Manette vue par l'ordinateur » : Xbox 360 (par défaut, reconnue par tous les jeux) ou fidèle (une manette Sony reste une DualShock 4). Il prime sur l'option « Toutes les manettes en Xbox 360 » de la console, qui ne reste qu'un défaut pour les autres clients.
- App Mac : sur un clavier Apple européen (ISO), la touche `@#` donnait `<` sur l'ordinateur et `<>` donnait `@` : les deux retrouvent leur place. App Apple TV : réglage « Clavier Apple ISO » pour les claviers Apple Bluetooth.
- Apps Mac et Apple TV : un relâchement refusé par la file d'envoi des entrées (64 paquets) n'est plus perdu en silence : il est réessayé jusqu'à 200 ms, et les refus sont comptés dans le journal. Sur l'Apple TV, un stick n'envoie plus que l'axe qui a bougé, par tranche de 4 ms : une DualSense en Bluetooth envoyait jusqu'à 1 500 paquets par seconde.
- App Apple TV : quand l'app passe en arrière-plan (bouton TV, autre app, veille), plus rien ne reste enfoncé sur l'ordinateur ; au retour, les sticks et gâchettes reprennent leur état réel.
- Apps Mac et Apple TV : à l'ouverture de la barre ou du menu, la manette est remise à zéro sur l'ordinateur (touches relâchées, sticks et gâchettes au repos). Une gâchette tenue ne reste plus enfoncée dans le jeu pendant le menu ; sur l'Apple TV, la manette en souris relâche aussi ses clics et ses touches, et un stick encore tenu à la fermeture du menu reprend aussitôt.
- Apps Mac et Apple TV : la combinaison L1 + R1 + Start + Select pressée pendant une reconnexion ou avant la première image ne rend plus la manette muette jusqu'à la combinaison suivante. La manette n'est prise que quand la barre (Mac) ou le menu (Apple TV) s'ouvre vraiment, et un redémarrage de l'image ne la rend plus au jeu pendant que le menu est ouvert.
- App Mac : après ⌘Tab, ⌘Espace ou un passage dans une autre app, la touche ⌘ (envoyée comme Ctrl) ne reste plus enfoncée sur l'ordinateur. Tout est relâché quand la fenêtre perd le clavier, et les modificateurs vraiment tenus sont renvoyés quand elle le retrouve.
- Client web : un clic dans la barre de session (Quitter, Son, Touches…) ou sur un bouton tactile ne clique plus aussi sur l'ordinateur, sous la barre.
- Client web sur téléphone et tablette : le texte tapé au clavier de l'appareil arrive juste sur un ordinateur en AZERTY, avec majuscules, accents et emoji, et un clavier branché à l'iPad ne double plus les lettres. Sur un appareil tactile, l'image passe donc par WebSocket, comme sur iPhone et iPad.
- Client web : quitter la fenêtre (Alt+Tab, ⌘Tab, autre onglet) ou rendre la souris relâche sur l'ordinateur les touches et les boutons tenus : plus de Maj, d'Alt ou de clic collés.
- Client web : la session tient dans un onglet masqué (la console ne la croit plus terminée au bout de 30 s), et une connexion à la console expirée est annoncée clairement au lieu d'une reconnexion qui échoue.
- Client web : la page ne bascule plus à tort de WebTransport vers WebSocket pendant la préparation de l'écran, et ferme l'ancienne connexion avant d'ouvrir la nouvelle (plus d'entrées en double). « Kyber ne répond pas » et « l'image n'arrive pas » ne sont plus confondus.
- Client web : la molette marche sous Firefox, et le défilement à deux doigts en tactile avance par crans entiers. Sous Firefox, l'image passe par WebSocket, en H.264 par défaut.
- Client web : quand le navigateur bloque le son (Safari, iPhone), une puce « Toucher pour le son » le rétablit, au lieu d'une session muette.
- Client web : quand l'écran de l'ordinateur change (taille, écran retiré) ou que sa capture tombe en erreur, l'image reprend toute seule, sans recharger la page (trois essais pour une erreur). La barre montre les manettes branchées, et les messages de fin disent ce qui s'est vraiment passé.
- Client web en tactile : clavier ouvert, l'image garde toute la hauteur au-dessus de la rangée de touches (elle tombait au quart de l'écran), et sur un iPad avec clavier les doigts marchent aussi. Fermer l'onglet en pleine session demande confirmation.

## 0.2.5

- App Apple TV : les manettes répondent enfin sur l'ordinateur. Elles s'annonçaient avant que les entrées soient reliées, et l'ordinateur ignorait ensuite leurs boutons ; la télécommande et la manette en souris marchaient, elles.
- Apps Mac et Apple TV : toutes les manettes, PlayStation comprises, deviennent des manettes Xbox 360 sur l'ordinateur.
- App Apple TV : si l'ordinateur n'annonce que des écrans éteints (0 × 0), la session le dit au lieu d'attendre sans fin.

## 0.2.4

- Image plus stable en 4K : l'ordinateur ne jette plus de morceaux d'une grosse image (files d'envoi QUIC et mémoires UDP de 8 Mo au lieu de 1 Mo), ce qui faisait saccader ou figer l'image puis redemander des images clés en chaîne.
- Nouveau moteur de capture de Kyber (libavconv, réglage « Moteur de capture ») pris en charge comme l'historique : écran virtuel vu, mêmes numéros d'écran, pointeur suivi et sans clignotement, son 5.1/7.1 en paquets de 10 ms et horodaté au fil des échantillons.
- App Mac : si l'ordinateur n'annonce que des écrans éteints (0 × 0), la session le dit au lieu de rester sur « Premières images ».

## 0.2.3

- App Apple TV : le pointeur ne disparaît plus quand on le déplace à la télécommande ou à la manette. L'Apple TV envoie maintenant sa position exacte, comme l'app Mac au bureau (Windows ne réaffichait pas son pointeur sur de simples déplacements), et le dessine toujours. La souris Bluetooth reste en mode jeu dans un jeu en plein écran.

## 0.2.2

- Nouvelle app Apple TV (en attendant l'App Store, à installer soi-même : voir https://github.com/MonsieurNours/kybernow/blob/main/apps/tvos/SIDELOAD.md) : votre ordinateur sur le téléviseur, à la télécommande ou à la manette, en 4K à 60 Hz au plus. Jusqu'à quatre manettes (Xbox, DualSense…) avec vibrations, Siri Remote en souris, clavier et souris Bluetooth, son multicanal par l'HDMI, audio spatial dans les AirPods et alerte quand la sortie ajoute du retard, Top Shelf, profils par utilisateur de l'Apple TV.
- App Apple TV : la manette peut servir de souris (menu de session, d'office en profil Bureau) : stick gauche pour le pointeur, A / ✕ clic, X / □ clic droit, B / ○ clic du milieu, stick droit pour défiler, croix pour les flèches, L1 / R1 pour passer d'une fenêtre à l'autre (Alt + Tab), Start pour le menu Démarrer, Select pour Échap.
- Console : appairage par code. L'Apple TV affiche un code à 6 chiffres, on l'accepte dans Accès › Appareils, et elle reçoit son accès sans rien taper à la télécommande.
- Console : l'annonce sur le réseau dit si Kyber est vraiment installé ; les apps ne proposent plus un ordinateur où il ne l'est pas.
- App Mac : vos ordinateurs sont publiés dans iCloud avec leurs accès, pour que l'Apple TV les retrouve toute seule (réglage « Synchroniser avec iCloud », ⌘,).
- Console : une mise à jour qui s'arrête en route ne bloque plus le bouton « Installer » (« une installation est déjà en cours ») ; la page Mises à jour montre la fin du journal de l'installation. Décompression et téléchargement de la mise à jour plus rapides (PowerShell sans barre de progression). Depuis la 0.2.0, la mise à jour lancée par la console pouvait ne jamais démarrer : PowerShell est maintenant lancé avec une console cachée, et sa sortie est gardée.

## 0.2.1

- Toutes les cartes graphiques : l'installation choisit l'encodeur de la carte la plus performante (NVIDIA, sinon AMD, sinon Intel, eGPU compris ; sinon le processeur). AMF et Quick Sync ne sont plus présentés comme expérimentaux, et une mise à jour garde l'encodeur que vous avez choisi.
- Console : l'état en direct de la carte graphique pour AMD et Intel aussi. La page Système montre son nom, son pilote, sa charge et si un flux est en cours ; l'aperçu de l'écran et l'arrêt automatique de l'écran virtuel et du son voient maintenant les flux encodés par AMF et Quick Sync.
- Manette : elle ne répondait plus après une reconnexion de Kyber (par exemple quand l'écran virtuel est recréé). L'app ne garde plus qu'une seule chaîne d'entrées à la fois.
- App Mac : beaucoup moins de paquets perdus en 4K (mémoire de réception UDP de 8 Mo au lieu de 768 Ko) ; l'image ne reste plus figée après la relance du flux qui suit une image clé perdue.
- App Mac : après un redémarrage de Kyber sur l'ordinateur, la session reprend sur l'écran virtuel au lieu de se couper (l'app le redemande à la console). Le panneau des mesures ne fait plus chuter la fluidité de l'image (plus de verre dépoli recalculé à chaque image).
- Mode jeu : le pointeur de l'ordinateur (menus, inventaires) est dessiné par l'app à sa vraie place. L'ordinateur transmet maintenant aussi les déplacements du pointeur.
- AV1 décodé par l'app, choix de l'écran de l'ordinateur dans la barre (sans écran virtuel).
- Console : aperçu de l'écran de l'ordinateur et nom de son système (« Windows 11 Pro 24H2 ») pour l'accueil des apps ; le système est aussi affiché au tableau de bord.
- App Mac : nouvelle identité (charte v2). L'accueil montre l'image de chaque ordinateur (dernière image de sa session, aperçu à jour ou sa signature), la session s'ouvre dans la même fenêtre par un passage en deux moitiés, néon sur les moments importants, nouvelle icône (Icon Composer).
- App Mac : mesures détaillées en session (clic sur les mesures de la barre ou ⌃⌥⌘S) : trajet d'une image étape par étape, latence et débit sur 30 s, pertes.
- App Mac : une image clé perdue au démarrage ne laisse plus l'écran noir ; si l'image ne vient pas, la session allège le flux (débit, puis 4:2:0).
- Console : nouvelle identité et quatre espaces (Vue d'ensemble, Image et son, Accès, Système). L'aperçu de l'écran en grand, qui est connecté et sa latence, « Mettre fin à la session », les appareils connus et ce qui s'est passé aujourd'hui ; journaux par source ; utilisable sur téléphone.
- Icône près de l'horloge : un panneau au clic gauche (état, session en cours, mise à jour, serveur), « Réinitialiser le mot de passe de la console », nouveau symbole.
- Installateur : nouvelle identité, quatre étapes au lieu de six (l'analyse de l'ordinateur tourne pendant la bienvenue).
- App Mac : elle se présente à la console (nom, système, latence) et se déconnecte quand la console met fin à la session.
- Nouveau client web : l'ordinateur s'ouvre depuis un navigateur (Chrome, Edge, Firefox), sans rien installer, à l'adresse de la console suivie de `/ecran` ou par « Ouvrir dans le navigateur » sur la Vue d'ensemble. Même mot de passe que la console ; accueil sur l'aperçu de l'écran, profils Bureau et Jeu, barre de session avec latence, mesures et Ctrl+Alt+Suppr ; pavé tactile, toucher direct et touches spéciales sur tablette et téléphone ; souris Auto, Bureau ou Jeu comme dans l'app ; Safari, iPhone et iPad passent par WebSocket. L'ancien client de Kyber reste disponible pour le dépannage.

## 0.2.0

Première version publiée.

**Ordinateur (serveur et console)**
- Installateur clé en main : Kyber 0.29 corrigé, console d'administration, icône de la zone de notification, écran virtuel, manettes, son à distance.
- Kyber corrigé : son HD et multicanal, son continu et à faible latence (paquets de 10 ms), pointeur stable, image clé à la demande d'une app qui a perdu des paquets.
- Console : appairage des apps (Bonjour, QR code, lien), détection des jeux en plein écran, mises à jour en un clic.

**App Mac**
- Image décodée par l'app (VideoToolbox, H.264, HEVC, AV1) et affichée avec Metal : latence minimale, écrans à fréquence variable suivis, 4:4:4.
- Son décodé par l'app (Opus, stéréo, 5.1, 7.1), avec une vingtaine de millisecondes de tampon.
- Définition réelle de l'écran du Mac (4K, 5K…), profils Jeu, Bureau et Max ramenés à ce que la machine décode.
- Souris Auto, Bureau ou Jeu ; presse-papiers partagé ; manette et barre de session ; latence de bout en bout mesurée.
- Mises à jour en un clic (Sparkle).
