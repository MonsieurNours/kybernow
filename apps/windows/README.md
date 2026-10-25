# KyberNow Link pour Windows

[English](README.en.md) · Français

KyberNow Link, l'app de streaming de KyberNow pour Windows 10 (22H2) et 11 : votre ordinateur (KyberNow Station : serveur Kyber 0.29 + KyberNow Admin) en plein écran sur un PC, avec l'image, le son, le clavier, la souris et les manettes. Cible : un portable ou un ordinateur peu puissant qui se connecte à un plus puissant. Décisions : ligne « Client Windows » de [PROJECT.md](../../PROJECT.md) ; maquettes : https://claude.ai/artifact/JkUTEMQmxKjcYksaXRHgBf.

L'app s'ouvre sur son accueil (Tauri 2, interface Svelte dans WebView2) : vos ordinateurs, leur ajout, leurs réglages, le passage vers la session et la barre de session. La session elle-même est une fenêtre Win32 en Direct3D 11 ; la ligne de commande sert aux essais.

L'app repose sur le **cœur client en Rust** ([`apps/core`](../core), crate `kybernow-core`), commun à toutes les apps KyberNow Link (les apps Apple y passent par un pont C : [docs/coeur-apple.md](../../docs/coeur-apple.md)).

## Ce que fait l'app

### Hors session (interface Svelte, `ui/`)

- **Accueil** (écrans et textes communs : [docs/link-ui.md](../../docs/link-ui.md)) : l'ordinateur choisi en plein cadre, son image (la dernière image de votre dernière session, gardée sur ce PC dans `%LOCALAPPDATA%\KyberNow\images` et désactivable dans ses réglages, « Garder la dernière image » ; sinon un aperçu de son écran demandé à Admin, une fois par minute au plus ; arrivée en balayage à 45°) ou sinon sa **signature** (deux lueurs dont la teinte vient de l'empreinte de son Admin), désaturée hors ligne ; puis, dans l'ordre et rien d'autre : son état (« En ligne », « Hors ligne », « Recherche… »), son nom, son système, une erreur s'il y en a, l'écran que la session demandera (« Écran de 1920 × 1200 à 60 Hz, à la taille de cet écran », ou « Les écrans de l'ordinateur, tels quels » sans écran virtuel ; même calcul que la session, `sessionPlan` dans `ui/src/lib/api.ts` pour `src/plan.rs`), **Se connecter** (hors ligne : **Réveiller l'ordinateur** quand l'app connaît ses cartes réseau, apprises de la Station à chaque relevé, puis « Réveil de l'ordinateur… » et l'ordinateur interrogé toutes les 2 s, 2 min au plus, jusqu'à son retour ; sinon **Réessayer**, qui l'interroge tout de suite), le profil (Jeu, Bureau, Max, et « Personnalisé » quand les réglages s'en écartent) avec sa phrase, et **Réglages** ; en bas vos ordinateurs (nom et état), ceux annoncés sur le réseau (« Sur le réseau », ils s'allument en apparaissant) et l'ajout. Ni carte graphique, ni adresse, ni latence : elles sont dans les réglages de l'ordinateur. Admin est interrogée toutes les 8 s.
- **Ajouter un ordinateur** : le trouver **sur le réseau** (annonce Bonjour `_kybernow._tcp` d'Admin, empreinte comprise ; une annonce marquée `kyber=0`, Admin sans serveur Kyber derrière, n'est pas proposée), **avec le lien** d'Admin (`kybernow://…`, collé) ou **par adresse** (l'empreinte d'Admin est alors affichée, les quatre premiers groupes à comparer à ceux d'Admin) ; puis y avoir accès **avec un code** à six chiffres à accepter dans Admin (Appareils Link : l'appareil reçoit son propre compte, sans rien taper) ou **avec un compte** et son mot de passe, vérifiés auprès d'Admin. Pas de QR code à la caméra.
- **Réglages de l'ordinateur** (« Réglages de Bureau-Salon », « Réglages d'Atelier » : le « de » s'élide devant une voyelle ou un h ; enregistrés à chaque changement) : son nom (modifiable), le **profil** (Jeu, Bureau, Max, Personnalisé : un réglage qui s'écarte du profil, format, texte net, débit ou transport, le rend « Personnalisé »), puis six rubriques dans une colonne à gauche (en rangée au-dessus du contenu sous 1040 px de large) :
  - **Image** : définition (comme cet écran, au plus 4K, ou **sur mesure** : largeur, hauteur et fréquence au choix, jusqu'à 7680 × 4320 (4096 de large en H.264), avec des valeurs courantes), écran virtuel, format vidéo (Auto, HEVC, H.264, AV1 ; ceux que la carte décode), texte net (4:4:4), débit (Auto montre le débit réel de la session, même règle que `src/plan.rs`), affichage des images (synchronisé ou immédiat), garder la dernière image ;
  - **Son** : Auto, stéréo, 5.1, 7.1 ou sur l'ordinateur ;
  - **Commandes** : manette vue par l'ordinateur (Xbox 360 ou fidèle), souris (Auto, du bureau, des jeux), ce que fait le clavier en plein écran, presse-papiers partagé ;
  - **Réseau** : transport de l'image (rapide ou fiable), reconnexion automatique, réveil par le réseau (« Prêt » et l'adresse MAC apprise de la Station, sinon « Pas encore d'adresse : connectez-vous une fois à cet ordinateur. » ; pas de saisie à la main) ;
  - **Statistiques** : la dernière session (médiane en grand, règle de 0 à 20 ms avec les repères 160, 120 et 60 Hz et la bande de la médiane au 95e centile, « 95 % des images sous … », durée et moment, définition, fréquence, format et débit transmis) et cet ordinateur (système, carte graphique, adresse d'Admin, quatre premiers groupes de l'empreinte) ;
  - **Station** : version de KyberNow Station (« À jour » ou « Version … disponible »), **Ouvrir Admin** (son adresse dans le navigateur par défaut), **Redémarrer la Station** et **Mettre à jour la Station** quand une version s'installe depuis un appareil (une version qui exige l'installateur le dit), compte (« Accès propre à cet appareil » pour un ordinateur ajouté avec un code : l'app le retient, et Admin le dit aussi pour ceux ajoutés avant la 0.5.0), **Retirer cet ordinateur**. Un appareil en session : confirmation (« Apple TV du salon est connecté », « Redémarrer quand même »). Pendant l'attente, l'app interroge la Station toutes les 2 s (`poll_now`) jusqu'à son retour, puis le dit (« La Station a redémarré. », « KyberNow Station 0.5.1 est en place. », ou « La Station tarde à revenir… » après 2 min, « La mise à jour n'a pas abouti… » si elle revient sur l'ancienne version) ; une Station d'avant la 0.5.0 (sans `station` dans `info`) : « mettez-la d'abord à jour depuis Admin ».
- **Passage** : l'accueil coupé en deux le long de la couture à 45°, un tube néon allumé pendant la préparation (capsule : préparation de l'écran, connexion, premières images, Annuler ; rien d'autre, ni latence ni chiffre ; quand l'image ne vient pas, le cœur redémarre la Station une fois en dernier recours et la capsule dit « La Station redémarre… » avant de reprendre ses étapes), puis, à la première image montrée (pas dès la connexion), la fenêtre de la session se montre en plein écran et l'accueil se cache, toujours ; sans image au bout de 12 s, le passage reste sur « Premières images » avec Annuler et dit que l'ordinateur n'envoie pas d'image (son journal : Admin, Système › Journaux) ; laissé devant, il montrerait le passage ouvert sur du noir. Si Windows refuse le premier plan à la session (lancement par la barre des tâches, une notification, un lien ou un outil à distance), elle se rattache un instant au fil de premier plan pour le prendre, sinon elle clignote dans la barre des tâches ; une session vers ce PC lui-même est refusée aussi par adresse (une adresse de ses cartes réseau) ; à la fin, il revient, les moitiés se referment et la dernière session s'y inscrit.
- **Barre de session** : fenêtre transparente au-dessus de la session, montrée quand le pointeur touche le haut de l'écran (pas en souris des jeux), cachée quand il la quitte ou n'y entre pas (le clavier revient à la session) : symbole allumé, **statistiques** en direct (latence et sa courbe, définition, images par seconde, décodeur) ; **profil** (menu Jeu, Bureau, Max : enregistré pour cet ordinateur et appliqué tout de suite, nouveau débit sur-le-champ ou flux relancé si le format, le 4:4:4 ou le transport changent, comme sur Mac) ; **menu Touches** (Ctrl + Alt + Suppr, touche ⊞ seule, Alt + Tab, Impr. écran) ; souris, plein écran, **Se déconnecter**. Un menu ouvert fait grandir la fenêtre de la barre vers le bas le temps de son affichage.
- **Mots de passe** dans le gestionnaire d'identifiants de Windows (`KyberNow:<id>`), jamais dans un fichier.
- **Pas sur lui-même** : se connecter à ce PC est refusé (l'écran virtuel éteindrait les écrans où l'app s'affiche).
- **Compte d'appareil pas encore connu de Kyber** : Admin crée le compte d'un appareil accepté par code, mais Kyber ne le lit qu'à son redémarrage, que Admin reporte pendant une session ou un encodage. L'app le dit (« redémarrez le serveur dans Admin ») au lieu d'un simple refus.

### Intégration à Windows

- **Une seule app** : une deuxième ouverture (raccourci, lien, barre des tâches, notification) passe sa demande à l'app déjà ouverte, qui revient devant, puis se ferme (mutex `Local\KyberNow.App`, tube nommé propre à l'utilisateur et à sa session Windows).
- **Lien `kybernow://`** (lien et QR code d'Admin) : inscrit pour l'utilisateur (HKCU) par l'installateur et à chaque lancement ; il ouvre l'ajout avec l'ordinateur et son empreinte déjà vérifiés.
- **Barre des tâches** : au clic droit de l'icône (et dans le menu Démarrer pour l'app épinglée), vos ordinateurs, un clic lance la session (`kybernow.exe --connect <id>`), et « Ajouter un ordinateur ». Autant d'ordinateurs que Windows en montre (son réglage du nombre d'éléments récents, 10 par défaut). La liste suit les ajouts, retraits et changements de nom.
- **Notifications de Windows**, seulement si l'app n'est pas au premier plan : session terminée (par un autre appareil, Admin ou une erreur), avec un bouton qui relance la session (« Reprendre la main », ou « Reprendre » après une erreur) ; nouvelle version prête. Identité des notifications inscrite dans HKCU (`AppUserModelId\LesLignesClaires.KyberNow`, nom affiché « KyberNow Link », icône `kybernow.png`).
- **Réglages de l'app** (roue de la barre de titre) : version, langue (Auto, Français, English), versions proposées (stables, ou aussi les bêtas), recherche et installation d'une mise à jour, ses nouveautés ; **Ce PC** (carte graphique, ce qu'elle décode, écran, son) et **Sur batterie** (alléger : 30 Hz au plus pendant les sessions, pour tous les ordinateurs). Une pastille sur la roue et un bandeau dans la barre de titre (« Version … disponible ») disent qu'une version attend.

### Installation et mises à jour

- **Installateur** `KyberNow-Link-Setup.exe` (`setup/`, « Installation de KyberNow Link » ; l'app en 0.3.1 et avant cherche `KyberNow-Client-Setup.exe` : ce nom est donc encore publié en copie, jusqu'à ce que ces versions aient disparu), dans le style de celui de KyberNow Station (`KyberNow-Station-Setup.exe`) : fenêtre sans bordure en Rust, WebView2 et Svelte, colonne au logo signé de KyberNow Link, sous un grand symbole « now » dont la couture s'allume avec les étapes (Bienvenue, Installation, C'est prêt) ; une page « À propos » (auteur, logiciels libres inclus et leurs licences). Un seul fichier pour **x64 et ARM64** : il porte les deux versions de l'app et pose celle du processeur (il tourne émulé sur un PC ARM et y installe la version ARM64 ; une app x64 émulée le note dans son journal). Pour l'utilisateur seul, sans droits d'administrateur, dans `%LOCALAPPDATA%\Programs\KyberNow`.
  - **Bienvenue** : ce qu'il voit du PC sans rien changer (Windows, processeur, carte graphique, écran, réseau, WebView2, version déjà installée, app ouverte), option « Raccourci sur le Bureau ». Une version installée fait de l'installation une mise à jour (« Mettre à jour »).
  - **Installation** : ferme l'app si elle est ouverte ; pose les fichiers à côté (`KyberNow.new`) puis les échange d'un coup avec l'ancienne version (une mise à jour qui échoue la laisse intacte ; si l'ancien dossier est verrouillé, copie par-dessus) ; installe WebView2 s'il manque (installateur officiel de Microsoft, pour l'utilisateur) ; lien `kybernow://`, notifications et raccourcis posés par l'app elle-même (`kybernow.exe --setup-shell [--desktop]` : `KyberNow Link.lnk` ; un `KyberNow.lnk` laissé par une version d'avant le renommage est retiré) ; « Applications installées » (HKCU, nom affiché « KyberNow Link », clé `…\Uninstall\KyberNow`, celle des versions déjà installées).
  - **C'est prêt** : « Ouvrir KyberNow Link », et les raccourcis de la session.
  - `/S` : silencieux ; `/S /UPDATE` : mise à jour lancée par l'app (attend sa fermeture, la relance avec `--updated`, et la rouvre avec un message si l'installation échoue). L'installateur ne garde jamais le dossier de l'app comme dossier courant (Windows refuserait de le renommer) ; si le dossier reste verrouillé, les fichiers sont échangés un par un, avec retour arrière. Pendant l'installation il tient `Local\KyberNow.Setup` : l'app rouverte entre-temps attend la fin (2 minutes au plus). Sans WebView2, le lanceur propose de l'installer avant d'ouvrir son interface. Programmes de Windows appelés par leur chemin complet (`System32`), pas depuis le dossier de l'installateur. Le pied se lit aussi derrière une signature Authenticode (table des certificats de l'en-tête PE). ARM64 : Windows 11 (Windows 10 sur ARM n'émule pas le x64 du lanceur). Journaux : `%LOCALAPPDATA%\KyberNow\setup.log` (lanceur) et `setup-ui.log` (interface).
  - Pas de signature Authenticode : Windows (SmartScreen) avertit au premier lancement.
- **Désinstallation** (« Applications installées », `uninstall.exe --uninstall`, le lanceur sans son contenu) : ferme l'app, vide la liste de la barre des tâches, propose d'effacer aussi les ordinateurs, réglages et mots de passe (les mots de passe par `kybernow.exe --forget-all`, puis les dossiers de données : `%APPDATA%\KyberNow`, `%LOCALAPPDATA%\KyberNow` et le profil de WebView2), retire raccourcis (`KyberNow Link.lnk`, et `KyberNow.lnk` s'il en reste un), inscriptions et restes d'une mise à jour interrompue (`KyberNow.new`, `KyberNow.old`), puis le dossier. Les options internes de l'installateur écrivent dans `kybernow-setup.log`, pas dans le journal de l'app.
- **Mises à jour**, comme la Station (`server/admin/src/update`) : l'app cherche 15 s après son lancement puis toutes les 6 heures la dernière version publiée sur GitHub, sur son canal ; rien n'est proposé sans le manifeste signé de la version (`kybernow-release.json`, minisign, même clé que la Station) qui doit annoncer la même version et donner l'installateur. L'installateur téléchargé (`KyberNow-Link-Setup.exe`, ou `KyberNow-Client-Setup.exe` quand le manifeste d'une version plus ancienne n'a que lui ; le même pour les deux architectures) doit avoir exactement la taille et l'empreinte SHA-256 du manifeste. « Installer et relancer » le lance en silence (`/S /UPDATE`) et ferme l'app ; il attend sa fermeture, remplace ses fichiers (pas les données) et la relance (`--updated`, « KyberNow Link X est installée »). Pas pendant une session.

### Session

- **Session** (cœur, `apps/core/src/session.rs`, la même que sur Mac, Apple TV, iPhone et iPad) : préparation par Admin (`POST /api/stream/prepare` : écran virtuel à la définition et à la fréquence de l'écran de ce PC, sortie son virtuelle au nombre de voies choisi, en Auto celui de la sortie de ce PC, stéréo s'il n'en a pas : le son vient alors ici dès qu'une sortie apparaît), connexion à Kyber avec le même compte, certificat de Kyber exigé tel que Admin le donne (`kyber_fingerprint`), choix de l'écran (attend 6 s au plus que Kyber n'annonce plus que l'écran virtuel), signal de vie toutes les 5 s (intervalle donné par Admin) avec la latence (Admin peut y demander la fin de la session), premier plan de l'ordinateur interrogé chaque seconde (souris Auto), fin propre (`POST /api/stream/end`, latence de la session). Reprise : reconnexion automatique de Kyber (réglage « Reconnexion automatique », rubrique Réseau), écran redemandé à Admin si Kyber a redémarré, image relancée (3 fois au plus) après une erreur de la chaîne d'image, image clé redemandée dès qu'une image de référence manque (toutes les 300 ms tant qu'elle n'est pas venue). Si le paquet qui annonce le format du flux se perd (transport rapide, avec pertes), le décodeur du format demandé pour la session s'ouvre d'office à la première image ; sans cela, toutes les images seraient jetées sans bruit et l'écran resterait noir. Le journal note le format annoncé, les paramètres, les premières données et la première image clé. Comme sur Mac, une image clé ne se demande qu'après une perte, jamais avant les premières données (l'encodeur commence par une), et un flux sans image 4 s après son démarrage est relancé (ce qui relance aussi le service vidéo de Kyber sur l'ordinateur), trois fois au plus avant d'arrêter la session avec la raison.
- **Admin** : compte Basic, en-tête `x-kyber-admin`, certificat épinglé par son empreinte (`sha256:aa:bb:…`, SHA-256 du certificat). En ligne de commande, sans empreinte donnée (`--fingerprint`), la première vue est affichée et retenue dans `%APPDATA%\KyberNow\consoles.txt` ; un certificat différent ensuite arrête la connexion.
- **Profils** Jeu, Bureau et Max, ramenés à ce que la carte graphique décode, et **débit** selon les pixels par seconde : mêmes règles et mêmes valeurs que l'app Mac (testées dans `apps/core`). Une option de format, de couleurs ou de débit rend le profil « Personnalisé ». En H.264, l'écran demandé est ramené à 4096 pixels de large au plus (limite de NVENC), et le débit d'un profil suit cet écran réduit.
- **Image** : FFmpeg (celui de Kyber, lié en statique) décode par la carte graphique (**D3D11VA**, sans copie hors de la carte ; l'app donne à FFmpeg l'appareil Direct3D et ses interfaces vidéo, que FFmpeg 8.1 ne va pas chercher lui-même : sans elles il plante à la création du décodeur ; sans interfaces vidéo, comme dans une machine virtuelle, décodage par le processeur) ; au lancement, l'app décode pour de bon une petite image de chaque format (H.264, HEVC, HEVC 4:4:4, AV1, dans `src/probe/`) pour savoir ce que la carte sait faire. Si la carte ne décode pas un format, ou le décode dans un format de texture qu'on ne sait pas montrer, l'app décode par le **processeur** (fils par tranches seulement : les fils par image ajoutent une image de retard chacun), sauf l'AV1 : pas d'AV1 par le processeur, il n'est proposé que si la carte le décode. Rendu Direct3D 11 : un shader convertit le YUV (NV12, AYUV 4:4:4 ou trois plans) en RVB selon l'espace et la plage de couleurs du flux, à la taille de la fenêtre en gardant les proportions ; chaîne d'échange « flip », une image en attente au plus ; chaque paquet est décodé mais seule l'image la plus récente est montrée (si d'autres attendent, celle-ci est sautée : le retard ne s'accumule pas derrière la synchro verticale). Synchro verticale par défaut ; `--no-vsync` présente chaque image dès qu'elle est décodée (déchirure permise quand Windows le permet, utile avec un écran G-SYNC ou FreeSync).
- **Son** : le moteur de son du cœur (`kybernow_core::audio::AudioEngine`, le même que sur Mac, Apple TV, iPhone et iPad : libopus, masquage des pertes, tampon adaptatif de 4 à 60 ms qui part à 8 ms, dérive des horloges rattrapée en douceur, voir [apps/core/README.md](../core/README.md)) ; WASAPI en mode partagé piloté par événement tire ses échantillons depuis son fil (`AudioEngine::fill`), aux voies du flux remises dans l'ordre de Windows (5.1, 7.1), et lui donne la latence de la sortie (son tampon plus ce que Windows annonce) ; Windows convertit au format de la sortie. La sortie est rouverte quand le flux change de nombre de voies ou qu'elle tombe en erreur. La barre donne au survol de la latence le son jusqu'à la sortie et les trous comblés dans la seconde.
- **Entrées** : par le module d'entrées du cœur (`kybernow_core::input::InputPipeline`, le même que sur Mac, Apple TV, iPhone et iPad), une chaîne par démarrage de l'image, créée pour l'écran affiché et arrêtée avec elle (FlushKeys, pertes au journal) ; la fenêtre capte les événements de Win32 et de Raw Input et les lui donne. Horloge : `Instant` (QueryPerformanceCounter).
- **Clavier** : les codes de touche de Windows tels quels (jeu 1, `0xE000` pour les touches étendues, comme le client de Kyber), sans les répétitions (l'ordinateur répète lui-même) ; Alt et F10 ne déclenchent pas de menu local. **Raccourcis de Windows** : en plein écran et au premier plan, un crochet clavier de bas niveau envoie à l'ordinateur ce que Windows garderait pour ce PC (touche ⊞ et ses combinaisons, Alt+Tab, Alt+Échap, Ctrl+Échap, touche Menu) ; en fenêtre, ils restent à ce PC. Ctrl+Alt+Suppr reste toujours à Windows : menu Touches de la barre.
- **Souris** : position absolue ramenée aux pixels de l'ordinateur, boutons (dont les deux latéraux), molette. Souris des jeux (relative, déplacements bruts par Raw Input, pointeur caché et retenu dans la fenêtre) : automatique quand Admin voit un jeu en plein écran et que l'ordinateur cache son pointeur, ou forcée par Ctrl+Alt+Maj+M. Règles Auto, Bureau, Jeu du cœur (`MouseModeRules`, comme sur Mac) : un pointeur remontré moins de 0,6 s (menus, clignotement) ne fait pas quitter la souris des jeux, revue 0,7 s après.
- **Pointeur** : celui de l'ordinateur (forme et point chaud, reçus et dédoublonnés par la `PointerLayer` du cœur) devient le pointeur de Windows au-dessus de l'image, **à l'échelle de l'image** (pixels de ce PC par pixel de l'ordinateur, entre 0,5 et 3 : il garde sa taille par rapport à l'image) ; une forme entièrement transparente garde la précédente ; la forme n'est refaite que si elle ou l'échelle change (les paquets de position arrivent jusqu'à 120 fois par seconde). **En mode jeu**, quand l'ordinateur montre son pointeur (menus d'un jeu), celui de ce PC le dessine là où l'ordinateur l'annonce ; caché sinon.
- **Écran redemandé** : quand le signal de vie dit l'écran virtuel parti (Admin redémarré…), il est redemandé et l'image relancée, au plus une fois par 30 s (comme sur Mac). Le profil annoncé par le signal de vie suit les changements faits depuis la barre.
- **Manettes et presse-papiers** : par la configuration client de kynput sous Windows (SDL pour les manettes, vues par l'ordinateur comme des Xbox 360 ; presse-papiers texte et HTML dans les deux sens). Le pont du presse-papiers du cœur (`ClipboardBridge`, texte seul) n'est pas branché sous Windows : kynput y installe déjà le sien, seul destinataire possible des paquets de presse-papiers de sa chaîne.
- **Paquets refusés** : la file d'envoi de kynput tient 64 paquets et refuse sans retour quand elle est pleine ; touches et boutons refusés sont réessayés toutes les 3 ms, dans l'ordre, pendant 200 ms au plus (`PacketQueue` du cœur, sur son fil `kybernow-entrees`), positions, déplacements et molette partent une fois. Le total des pertes va au journal.
- **Rien ne reste enfoncé** : quand la fenêtre perd le clavier, l'app relâche sur l'ordinateur les touches et boutons qu'elle y tient (`release_all`) ; le relâchement d'une touche ou d'un bouton dont l'appui n'est pas parti (raccourci de l'app) ne part pas ; en fin d'image et de session, tout est relâché.
- **Plantages** : notés dans le journal (paniques avec la pile, exceptions natives avec le module et le décalage) ; le journal du lancement précédent est gardé (`kybernow.prev.log`). La sonde de la carte graphique tourne dans un processus à part (`kybernow --probe-json`, journal `kybernow-probe.log`) : s'il plante, l'app décode par le processeur. L'exécutable non allégé donne la ligne en cause (`llvm-addr2line -e kybernow-<version>-windows-<arch>.exe 0x140000000+décalage`) : `build.sh` en garde une copie par version et architecture dans `apps/windows/dist/symbols/` (ignoré par git, sur le Mac qui l'a construit, environ 300 Mo chacune ; un nouveau build de la même version la remplace).
- **Mesures** : la barre de session donne la latence de bout en bout (capture sur l'ordinateur → présentation ici, sur l'horloge de Kyber recalée), la définition, le décodeur et les images par seconde ; la médiane et le 95e centile de la session vont au journal, à Admin et, pour une session d'au moins 3 s, à la rubrique Statistiques. Le titre de la fenêtre de session est « {nom} · KyberNow Link » ; en ligne de commande (sans barre), il porte aussi l'étape et les mesures.
- **Un seul écran** : la session s'affiche en plein écran sur l'écran de ce PC où se trouve sa fenêtre.

| Raccourci | Effet |
|---|---|
| Ctrl+Alt+Maj+D | Se déconnecter (aussi : fermer la fenêtre) |
| Ctrl+Alt+Maj+F | Plein écran ou fenêtre |
| Ctrl+Alt+Maj+M | Souris des jeux ou du bureau |

## Langues

L'app et son installateur existent en français et en anglais ([docs/i18n.md](../../docs/i18n.md)).

- **Détection** : langue d'affichage de Windows (`GetUserPreferredUILanguages`) ; si elle commence par `fr`, français, sinon anglais.
- **Réglage Langue** (réglages de l'app) : Auto (par défaut, celle de Windows), Français, English ; enregistré dans `%APPDATA%\KyberNow\app.json` (`language`) et appliqué tout de suite aux deux fenêtres (accueil et barre de session), aux messages du côté Rust, aux notifications, à la liste de la barre des tâches et au titre de la fenêtre de session. L'installateur n'a pas de réglage : il suit Windows.
- **Admin** : chaque requête porte `Accept-Language: fr` ou `en` (langue de l'app), pour que Admin réponde dans cette langue.
- **Notes de version** : seule la partie de la langue de l'app est montrée (le texte de la release est coupé à la ligne `<!-- en -->` : français avant, anglais après ; tout le texte sans elle).
- **Catalogues** (mêmes clés, le français est la source, une clé absente en anglais retombe sur le français) : interface `ui/src/lib/i18n/{fr,en}.json` (fonction `t()`), côté Rust de l'app `src/i18n/{fr,en}.json` (`tr()`, `src/i18n.rs`), cœur `apps/core/src/i18n/{fr,en}.json` (`apps/core/src/i18n.rs`), installateur `setup/src/i18n/` et `setup/ui/src/lib/i18n/`. `npm --prefix apps/windows/ui run check` compare les clés de l'interface, du côté Rust de l'app et du cœur, `npm --prefix apps/windows/setup/ui run check` celles de l'interface de l'installateur, et un test Rust (`same_keys`) celles de `setup/src/i18n/`. Les journaux et la ligne de commande (essais, dépannage) restent en français.
- **Imitation** dans un navigateur : langue du navigateur, ou `?lang=en` / `?lang=fr` dans l'adresse ; `?vide` (aucun ordinateur), `?installateur` (mise à jour de la Station par l'installateur), `?sansimage` (l'ordinateur n'envoie pas d'image), `?redemarrage` (dernier recours : la Station redémarre pendant la connexion), `?sansreveil` (ordinateur hors ligne sans carte réseau connue), `?echecmaj` (la mise à jour de la Station n'aboutit pas), `/bar.html` pour la barre. Un ordinateur imité par état : en ligne avec une dernière session et une mise à jour de la Station (redémarrage avec confirmation, puis suivi), hors ligne et réveillable, à jour avec une session de la 0.4.x, Station trop ancienne.

## Utilisation

`kybernow.exe` sans argument ouvre l'app. En ligne de commande, une session directe (essais, dépannage) :

```
kybernow <adresse>[:port] [--user <compte>] [--profile jeu|bureau|max] [options]
```

- `<adresse>:port` : l'ordinateur et le port d'Admin (Appareils Link, dans Admin) ; 9443 par défaut.
- Mot de passe : `--password`, la variable `KYBERNOW_PASSWORD`, sinon demandé.
- `--codec h264|hevc|av1`, `--444` / `--420`, `--mbps <débit>`, `--size 1920x1080@60` (sinon l'écran principal de ce PC), `--window`, `--no-vsync`, `--no-hw` (décodage par le processeur), `--no-audio` (son laissé sur l'ordinateur), `--physical` (écrans de l'ordinateur tels quels), `--no-station-restart` (pas de redémarrage de la Station en dernier recours quand l'image ne vient pas).
- `--probe` : affiche ce que la carte graphique décode, l'écran, la sortie son et le système, puis quitte. Ne demande pas d'ordinateur.

## Organisation du code

| Dossier | Contenu |
|---|---|
| `apps/Cargo.toml` | Espace de travail Rust des clients (`core`, `windows`, `apple`, `android/jni`, `webos`, `linux`) ; renvoie les crates de Kyber vers `third_party/kyber-desktop/kysdk` |
| `apps/core/src/console.rs` | Client d'Admin (`info`, `probe`, `preview`, appairage par code, `prepare`, `heartbeat`, `foreground`, `end`), certificat épinglé, `Accept-Language` |
| `apps/core/src/i18n.rs` | Langue (détection, choix), catalogues de textes, notes de version par langue |
| `apps/core/src/session.rs` | Déroulé d'une session sur un fil dédié ; ce que la plateforme fournit : `Platform` |
| `apps/core/src/media.rs` | Paquets d'image et de son livrés par Kyber (correctif 0001), en-tête Opus |
| `apps/core/src/profile.rs` | Profils, format, débit selon les pixels par seconde |
| `apps/core/src/latency.rs` | Latence de bout en bout, horloge de Kyber |
| `apps/core/src/update.rs` | Mises à jour : versions publiées sur GitHub, manifeste signé (minisign), téléchargement vérifié |
| `apps/windows/src/main.rs` | App ou ligne de commande |
| `apps/windows/src/ui.rs` | App Tauri : fenêtres, commandes de l'interface, sondage des ordinateurs, ce PC |
| `apps/windows/src/store.rs` | Ordinateurs et réglages (`computers.json`), mots de passe (gestionnaire d'identifiants) |
| `apps/windows/src/discovery.rs` | Ordinateurs annoncés sur le réseau (mDNS) |
| `apps/windows/src/host.rs` | Une session sur son fil : fenêtre, image, son, entrées, nouvelles pour l'app |
| `apps/windows/src/plan.rs` | Réglages d'une session (profil, retouches, écran, batterie) |
| `apps/windows/src/cli.rs` | Ligne de commande |
| `apps/windows/ui/` | Interface Svelte : accueil, ajout, réglages de l'ordinateur (`Settings.svelte`, rubriques `Statistics.svelte` et `Station.svelte`, règle `lib/FrameRuler.svelte`), réglages de l'app, passage, barre ; `src/lib/mock.ts` imite l'app dans un navigateur (`npm run dev`, développement seulement) |
| `apps/windows/src/window.rs` | Fenêtre Win32 : capture du clavier et de la souris vers le module d'entrées, dessin du pointeur, raccourcis, plein écran |
| `apps/windows/src/video.rs` | Fil de l'image : paquets → décodeur → rendu, images clés |
| `apps/windows/src/decoder.rs` | FFmpeg (D3D11VA ou processeur), sonde des capacités |
| `apps/windows/src/render.rs` | Direct3D 11 : textures, shader YUV → RVB, chaîne d'échange |
| `apps/core/src/input/` | Module d'entrées partagé : chaîne kynput, file d'envoi, pointeur de l'ordinateur, mode de la souris |
| `apps/core/src/audio/` | Moteur de son : libopus, tampon adaptatif, ordre des voies |
| `apps/windows/src/audio.rs` | Sortie WASAPI qui tire le son du moteur du cœur |
| `apps/windows/src/system.rs` | Journal, écran, identité de l'appareil, empreintes d'Admin connues |
| `apps/windows/src/i18n.rs` | Langue de Windows, textes du côté Rust de l'app (`src/i18n/`) |
| `apps/windows/src/shell.rs` | Intégration à Windows (HKCU) : lien `kybernow://`, ordinateurs au clic droit de l'icône de la barre des tâches, notifications |
| `apps/windows/src/instance.rs` | Une seule app par utilisateur : une deuxième ouverture passe ses arguments à la première (tube nommé) ; mutex lu par l'installateur |
| `apps/windows/src/snapshot.rs` | Dernière image d'une session pour l'accueil (JPEG par WIC, `%LOCALAPPDATA%\KyberNow\images`) |
| `apps/windows/src/pc.rs` | Ce PC : écran, son, système, ce que la carte graphique décode (sonde dans un processus à part, `--probe-json`) |
| `apps/windows/src/probe/` | Une image de chaque format (H.264, HEVC, HEVC 4:4:4, AV1) décodée par la sonde de la carte graphique |
| `apps/windows/setup/` | Installateur et désinstalleur : lanceur `KyberNow-Link-Setup.exe` (contenu, installation silencieuse, désinstallation), interface `kybernow-client-setup-ui.exe` (WebView2, Svelte de `setup/ui/`) |

Le cœur ne dépend d'aucun système : une autre plateforme fournit sa fenêtre et ses décodeurs par `Platform`, la capture des entrées (vers `InputPipeline`) et la sortie du son (qui tire le son d'`AudioEngine`).

## Construire

Tout se compile depuis le Mac :

```bash
apps/windows/scripts/build.sh                # x64 (x86_64-pc-windows-gnu, MinGW de Homebrew, comme le serveur)
apps/windows/scripts/build.sh --arch arm64   # ARM64, PC Snapdragon (aarch64-pc-windows-gnullvm, llvm-mingw)
```

Puis l'installateur, une fois les deux dossiers construits : `apps/windows/scripts/build-installer.sh` écrit `apps/windows/dist/KyberNow-Link-Setup.exe` (`[lanceur][x64.tar.zst][arm64.tar.zst][pied de 64 o « KNCLNT01 »]`, voir `setup/src/payload.rs`). Le projet `setup/` a son propre `Cargo.lock`, aux versions de l'installateur du serveur (`wry`, `tao`, `zstd`), et se construit pour x64 seulement. Son interface (`setup/ui`) se regarde dans un navigateur avec une simulation : `npm --prefix apps/windows/setup/ui run dev`, et `?maj`, `?ouverte`, `?arm`, `?echec` dans l'adresse. `scripts/release.sh` construit tout et joint l'installateur à chaque version.

Le dossier à copier sur le PC est `apps/windows/dist/x64/` (ou `arm64/`), aussi en archive `dist/KyberNow-<version>-windows-<arch>.zip` : `kybernow.exe`, `kynput.dll` et `SDL2.dll` (manettes), `WebView2Loader.dll` (interface), `kybernow.png` (icône des notifications), plus les DLL du runtime de la chaîne qu'ils demandent (`libunwind.dll` en ARM64). Le reste vient de Windows 10 et 11 (runtime C universel, Direct3D 11, `d3dcompiler_47.dll`). L'exécutable non allégé de chaque version reste dans `dist/symbols/kybernow-<version>-windows-<arch>.exe` (plantages, voir plus haut).

- **Prérequis x64** : `third_party/` en place, et `server/kyber-patches/build.sh` passé une fois (il produit `third_party/rootfs-win64`, avec FFmpeg et libopus, et `third_party/rootfs-win64-kynput`, avec `kynput.dll`).
- **Prérequis ARM64** : llvm-mingw dans `third_party/toolchains`, la cible Rust `aarch64-pc-windows-gnullvm`, puis `apps/windows/scripts/build-arm64-deps.sh` une fois : libopus, un FFmpeg réduit au décodage (H.264, HEVC, AV1, D3D11VA), SDL2 (sources vérifiées de kynput) et `kynput.dll`, dans `third_party/rootfs-winarm64`. Versions des outils : [docs/build-notes.md](../../docs/build-notes.md).
- **Correctifs de Kyber** : les mêmes que pour les apps Apple (`apps/macos/kyber-patches`), appliqués par le script s'ils ne le sont pas.
- **VLC** : kyclient le lie toujours sous Windows, mais l'app décode elle-même : le script remplace chaque fonction de `libvlc.dll` par une fonction vide (comme sur l'Apple TV) et refuse un exécutable qui dépendrait encore de `libvlc.dll`.
- `--debug` : sans optimisation.
- **Tests du cœur** (sur le Mac) :

```bash
cd apps && PKG_CONFIG_PATH=$PWD/../third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib cargo test -p kybernow-core
```

## Tester

L'interface se travaille dans un navigateur sur le Mac, avec une imitation de l'app : `npm --prefix apps/windows/ui run dev` (ou la configuration `windows-ui` de `.claude/launch.json`).

Copier `dist/x64` sur le PC (ou `dist/arm64` sur un PC Snapdragon, ou dans Windows 11 ARM en machine virtuelle sur le Mac : elle dit si l'app démarre et se connecte, pas la latence ni le décodage par la carte graphique), puis lancer `kybernow.exe` (l'app), ou dans un terminal :

```
kybernow.exe --probe
kybernow.exe 192.168.1.100:9443 --user <compte> --profile bureau
```

L'interface demande le runtime WebView2 d'Edge, présent sur Windows 11 et sur la plupart des Windows 10 (l'installateur le pose au besoin).

Au premier contact, l'empreinte d'Admin s'affiche : la comparer à celle d'Admin (Appareils Link). Le journal est dans `%LOCALAPPDATA%\KyberNow\kybernow.log`.

## Données

| Fichier | Contenu |
|---|---|
| `%APPDATA%\KyberNow\computers.json` | Ordinateurs de l'app, leurs réglages, leur dernière session d'au moins 3 s (médiane, 95e centile, durée, date, définition, fréquence, format, débit ; une session gardée par la 0.4.x n'a que `label`, profil et format en français), `by_code` (ajouté avec un code) et `wake` (ses cartes réseau pour le réveil, du dernier relevé qui en donnait) |
| `%APPDATA%\KyberNow\app.json` | Réglages de l'app : canal des mises à jour (`channel`), langue (`language` : `auto`, `fr`, `en`), sur batterie (`battery` : `alleger`, `garder` ; absent, repris des réglages des ordinateurs, où la 0.4.x le gardait) |
| Gestionnaire d'identifiants, `KyberNow:<id>` | Mot de passe de chaque ordinateur |
| `%APPDATA%\KyberNow\device-id` | Identifiant de cet appareil pour Admin (tiré une fois) |
| `%APPDATA%\KyberNow\consoles.txt` | Empreintes d'Admin retenues par la ligne de commande (`hôte:port sha256:…`) |
| `%LOCALAPPDATA%\KyberNow\images\<id>-last.jpg` | Dernière image de la dernière session de chaque ordinateur (« Garder la dernière image ») |
| `%LOCALAPPDATA%\KyberNow\kybernow.log` | Journal du dernier lancement de l'app (le précédent : `kybernow.prev.log`) |
| `%LOCALAPPDATA%\KyberNow\kybernow-probe.log`, `kybernow-setup.log` | Journaux de la sonde de la carte graphique et des options d'installation de l'app |
| `%LOCALAPPDATA%\KyberNow\setup.log`, `setup-ui.log` | Journaux de l'installateur (lanceur, interface) |
| `%LOCALAPPDATA%\fr.leslignesclaires.kybernow` | Profil de WebView2 de l'app |

## Essais

- **Vérifié** : session de bout en bout sur un PC x64 (~10 ms de latence, 21/10/2026) et dans Windows 11 ARM en machine virtuelle (décodage par le processeur) ; avec les entrées et le son du cœur (0.3.1 et après), confirmé le 24/10/2026.
