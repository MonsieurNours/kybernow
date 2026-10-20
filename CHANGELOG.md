# Versions de KyberNow

[English](CHANGELOG.en.md) · Français

Chaque section devient les notes de la version publiée sur GitHub : la
console de l'ordinateur et l'app Mac les affichent avant d'installer.

## 0.2.6

- Mises à jour de l'ordinateur sans risque de tout laisser arrêté : la nouvelle version est préparée à côté de l'ancienne puis mise en place d'un coup ; si la console ne répond pas dans la minute, l'ancienne version revient seule et la page Mises à jour dit pourquoi. La mise à jour à la main vérifie aussi l'empreinte du paquet, et « Applications installées » affiche la bonne version.
- Ordinateur partagé : le dossier des données de KyberNow (`C:\ProgramData\KyberNow`) n'est plus accessible qu'aux administrateurs. Un autre compte de l'ordinateur ne peut plus y lire l'empreinte du mot de passe ni glisser un fichier dans une mise à jour.
- Installateur : sur un portable sans mode « Performances élevées », l'installation ne s'arrête plus à 96 % : le mode est recréé, ou laissé tel quel avec un avertissement. L'option est décochée si l'ordinateur y est déjà.

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
