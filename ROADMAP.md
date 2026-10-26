# KyberNow — prévu

[English](ROADMAP.en.md) · Français

Ce qui est prévu ou à essayer, et seulement ici : les autres documents décrivent ce qui existe ([docs/i18n.md](docs/i18n.md#documentation)). Une ligne sort de ce fichier quand elle est faite ; ce qu'elle a apporté va alors dans la doc de l'app concernée et dans `CHANGELOG.md`.

## Essais à faire

- **Steam Deck** : une session sur le Deck (l'app Linux n'est essayée en session que dans la VM).
- **Raspberry Pi** : l'app (.deb) et KyberNow Link OS sur une vraie carte : firmware (tryboot), Plymouth, décodeur V4L2, sortie HDMI (dont le 1080p à 120 Hz sur Pi 5), son, Bluetooth, CEC.
- **iPad** : écran externe et Stage Manager.
- **Android** : 1080p à 120 Hz sur la Shield, mise à jour de bout en bout depuis une version publiée, une tablette, une console portable, DeX et ChromeOS ; vibrations de la DualSense et de la manette Shield sur la Shield ; pertes de son sur la Shield en 4K 60 (Ethernet à 100 Mbit/s) à revoir en gigabit ou à débit plus bas.
- **Téléviseurs LG** : latence sur la G2 en 4K à 120 Hz, 1440p et 1080p, à 30, 60 et 120 i/s ; écart son-image et latence ajoutée du 5.1 Dolby ; Magic Remote en souris : mouvements envoyés quand le pointeur du téléviseur est caché.
- **Station** : le mode « Pour toute connexion » (écran virtuel créé en cours de flux) ; sur un vrai PC Windows, le redémarrage et la mise à jour de la Station depuis une app, le dernier recours de la connexion, les cartes réseau listées pour le réveil et un vrai réveil depuis la veille et après un arrêt complet.
- **Icône près de l'horloge** : sur Mac, les notifications du système (et leur demande d'autorisation), l'inscription à l'ouverture de session et un vrai lancement à l'ouverture de session, la fenêtre fermée en plein écran pendant une session ; sur un vrai PC Windows, le haut du menu en session et l'icône allumée, les échelles 100 % et 200 %, les bannières de notification, « ne répond toujours pas » après 2 min de réveil.
- **Multi-écrans** : sur un PC Windows et sur un Linux à plusieurs écrans (validé sur Mac le 26/10/2026) ; une session à plusieurs écrans juste après l'installation ou le redémarrage de la Station.
- **Mesures de l'audit** décrites dans [docs/archives/mesures.md](docs/archives/mesures.md) : latence d'affichage de l'app Mac, droits Windows, Wi-Fi saturé, modes de l'écran virtuel SudoVDA.

## KyberNow Station et Admin

- **Écran virtuel sans contournement** : un pilote qui gère vraiment plusieurs écrans (SudoVDA corrigé et recompilé, ou Kidd), pour retirer l'agrandissement du pointeur et la capture des écrans secondaires par Windows Graphics Capture (voir `server/README.md`).
- **HDR** : capture en FP16 et conversion HDR10 dans Kyber (étude chiffrée : [docs/archives/propositions-2026-10-22.md](docs/archives/propositions-2026-10-22.md)).
- **Écran virtuel Kidd** quand Kyber le publiera, derrière l'interface déjà en place.
- **Pilote audio signé KyberNow** (certificat EV + Partner Center), si les haut-parleurs de Steam ne suffisent plus.
- **Pilote d'écran virtuel attesté** (EV + Partner Center), pour ne plus ajouter le certificat de SudoVDA aux autorités racines.
- **Signature Authenticode** de l'installateur, de l'app Windows et du paquet de mise à jour manuelle (SignPath visé ; préalable : binaires Windows construits en CI).
- **Réveil par le réseau sur iPhone, iPad et Apple TV** : demander à Apple l'autorisation multicast (`com.apple.developer.networking.multicast`), pour envoyer le paquet en diffusion et pas seulement en direct.
- **API du contrôleur** : inventorier ce que `kycontroller` expose en HTTP (état de session, métriques).

## KyberNow Link

- **Plusieurs ordinateurs à la fois** (en cours) : une fenêtre de session par ordinateur dans les apps Mac, Windows et Linux (interface de bureau) ; clavier, souris, manettes, presse-papiers et son suivent la fenêtre active (« Garder le son » pour en entendre une autre) ; un seul lecteur de manettes par app, dans le cœur. Contrat : [docs/plusieurs-ordinateurs.md](docs/plusieurs-ordinateurs.md).
- **Android** : mise en page des pliables ; écran externe en pupitre (image sur l'écran branché, appareil en pupitre) ; Play Store en test interne.
- **Android TV + Station** : envoi des ordinateurs du téléphone au téléviseur (code affiché au téléviseur, relais par Admin).
- **iPhone et iPad** : Apple Pencil ; TestFlight pour testeurs internes (profil App Store, archive Release, `xcodebuild -exportArchive`), puis l'App Store.
- **Apple TV** : TestFlight, puis l'App Store.
- **Téléviseurs LG** : dépôt Homebrew Channel, pour des mises à jour installées depuis le téléviseur ; vibrations et gâchettes adaptatives de la DualSense par le service Bluetooth de webOS.
- **Linux** : import DMA-BUF sans copie des images VA-API (4K à 120 Hz) ; décodage NVDEC pour les cartes NVIDIA ; dépôt Flatpak ou Flathub, pour des mises à jour installées seules.
- **Raspberry Pi** : publier le dépôt apt (clé GPG, hébergement, appel dans `release.sh`) ; voie sans copie (images SAND posées sur un plan DRM).
- **KyberNow Link OS** : installer la mise à jour pendant la veille ; une bande sonore composée pour l'allumage, à la place de la synthèse de `chime.rs`.

## KyberNow Share

Tout ce qui fait entrer quelqu'un d'autre sur votre ordinateur. Nom choisi le 26/10/2026 ; logo signé SHARE, comme STATION, ADMIN et LINK.

- **Session partagée** : un spectateur (image et son) ou un 2e joueur (ses manettes deviennent la manette suivante de Windows ; clavier et souris seulement si l'hôte les donne), invité par un code, dans l'app Link ou le navigateur. Kyber accepte déjà plusieurs appareils sur la même image (`multi_client`) ; Admin passe d'un seul appareil en session à des participants qui ont chacun un rôle.
- **Droits imposés par la Station** : correctif Kyber pour refuser les entrées d'un spectateur et ne garder que les manettes d'un 2e joueur (aujourd'hui, seul l'appareil décide de ne rien envoyer).
- **Encodage à part pour un invité à distance** : l'hôte garde sa qualité, l'invité reçoit une image à la mesure de sa connexion (correctif Kyber : un flux en propre à côté du flux partagé, pour que l'appareil le plus lent ne ralentisse plus les autres).
- **Relais** : relais de niveau 4 (TCP et UDP) qui ne voit pas l'image (chiffrement de bout en bout, certificat de Kyber épinglé par son empreinte) ; c'est la Station qui ouvre la connexion, rien à ouvrir sur la box. Relais public tenu par Les Lignes Claires par défaut, remplaçable par le sien (image Docker). Ensuite, accès hors de chez soi sans VPN par le même relais, débit plafonné sur le relais public. Client web : WebTransport par le relais (pas dans Safari).
- **Prêt de l'ordinateur**, à tour de rôle (Windows n'a qu'une session ouverte à l'écran) : compte Windows dédié, sans droits ni accès à vos fichiers ; bureau remplacé par un lanceur qui relance le programme quand il se ferme, sans Explorateur ; liste d'apps choisie par l'hôte, le service de la Station ferme le reste. L'ami joue avec son propre compte Steam (vos jeux : Familles Steam). Trois accès : accès d'ami activable (reprise avec préavis), code pour une séance, créneaux horaires. Point délicat : ouvrir la session du compte dédié sans taper son mot de passe.
- **Piste** : prêter l'ordinateur pendant que vous l'utilisez, par une machine virtuelle qui partage la carte graphique (Hyper-V) ; demande Windows Pro et une 2e licence, se heurte aux anti-triche, encodeur à vérifier.

## Manettes Sony en mode « Fidèle »

L'ordinateur voit une DualShock 4 (pilote ViGEmBus) : boutons, sticks, gâchettes et vibrations passent ; le pavé tactile n'est qu'un bouton.

- **Barre lumineuse** : renvoyer à l'app la couleur que le jeu donne à la DualShock 4 virtuelle (le pilote la reçoit déjà), pour éclairer la vraie manette.
- **Gyroscope et pavé tactile** : étendre le protocole des manettes de Kyber (correctifs côté apps et côté Station, à proposer à Kyber) et le rapport étendu de la DualShock 4 virtuelle, puis chaque app.
- Pas de gâchettes adaptatives : Windows ne voit pas une DualSense, et aucun pilote libre n'en crée une virtuelle.

## Publication, CI, site, marque

- **Copies des anciens noms** (`KyberNow-Server-Setup.exe`, `KyberNow-Setup.exe`, `KyberNow-Client-Setup.exe`, `KyberNow-macOS.zip`) : les retirer quand les installations en 0.3.1 et avant auront disparu.
- **CI** : compiler l'app Mac (avec un `KyberSDK-macOS.zip`) et les binaires Windows.
- **Kyber** : remonter le correctif de `strip` (cargo-c, éditeur de liens d'Xcode 27).
- **Site** : retirer les marques « bêta » à la 1.0 ; revoir les cartes Apple TV, iPhone et iPad quand ces apps passeront par TestFlight ou l'App Store.
- **Marque** : valider le nom KyberNow (et Station / Admin / Link / Share) avec l'équipe de Kyber ; dessiner le logo signé SHARE dans `brand/v2/` ; l'identifiant `fr.leslignesclaires.kybernow` ne change pas.
