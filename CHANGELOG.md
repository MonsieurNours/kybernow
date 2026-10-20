# Versions de KyberNow

[English](CHANGELOG.en.md) · Français

Chaque section devient les notes de la version publiée sur GitHub : la
console de l'ordinateur et l'app Mac les affichent avant d'installer.

## 0.2.1

- Manette : elle ne répondait plus après une reconnexion de Kyber (par exemple quand l'écran virtuel est recréé). L'app ne garde plus qu'une seule chaîne d'entrées à la fois.
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
