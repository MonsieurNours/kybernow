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
- **Plusieurs ordinateurs à la fois** : deux sessions qui tournent en même temps sur Windows et sur Linux (validé sur Mac le 27/10/2026) ; sous Windows, une fin de session qui en a coupé une autre une fois dans la VM (chaque chemin de fin est maintenant au journal).
- **KyberNow Share et relais** : le partage hors de chez soi et l'accès à son ordinateur par `share.kybernow.app` depuis chaque famille d'apps (radio cellulaire d'un iPhone comprise) ; les images à part d'un invité sur les cartes AMD (AMF) et Intel (QSV) ; le reste des vérifications des correctifs 0019 et 0020 est dans [docs/kyber-changes.md](docs/kyber-changes.md). Les deux sessions à la fois sous Windows et Linux sont sur la ligne « Plusieurs ordinateurs à la fois ».
- **Mesures de l'audit** décrites dans [docs/archives/mesures.md](docs/archives/mesures.md) : latence d'affichage de l'app Mac, droits Windows, Wi-Fi saturé, modes de l'écran virtuel SudoVDA.

## KyberNow Station et Admin

- **H.264 muet sur une carte Intel** : PC-ATELIER (Intel UHD) envoie bien du HEVC mais aucune image en H.264 (« son encodeur ne répond pas »), déjà en 0.5 ; à comprendre côté Station (encodeur de Kyber, QSV).
- **Écran virtuel sans contournement** : un pilote qui gère vraiment plusieurs écrans (SudoVDA corrigé et recompilé, ou Kidd), pour retirer l'agrandissement du pointeur et la capture des écrans secondaires par Windows Graphics Capture (voir `server/README.md`).
- **HDR** : capture en FP16 et conversion HDR10 dans Kyber (étude chiffrée : [docs/archives/propositions-2026-10-22.md](docs/archives/propositions-2026-10-22.md)).
- **Écran virtuel Kidd** quand Kyber le publiera, derrière l'interface déjà en place.
- **Pilote audio signé KyberNow** (certificat EV + Partner Center), si les haut-parleurs de Steam ne suffisent plus.
- **Pilote d'écran virtuel attesté** (EV + Partner Center), pour ne plus ajouter le certificat de SudoVDA aux autorités racines.
- **Signature Authenticode** de l'installateur, de l'app Windows et du paquet de mise à jour manuelle (SignPath visé ; préalable : binaires Windows construits en CI).
- **Réveil par le réseau sur iPhone, iPad et Apple TV** : demander à Apple l'autorisation multicast (`com.apple.developer.networking.multicast`), pour envoyer le paquet en diffusion et pas seulement en direct.
- **API du contrôleur** : inventorier ce que `kycontroller` expose en HTTP (état de session, métriques).

## KyberNow Link

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

- **Image du navigateur en WebTransport par le relais** (sauf Safari) : le relais lirait le nom dans le premier paquet QUIC et ferait suivre sans déchiffrer ; aujourd'hui, par le relais, le navigateur prend l'image en WebSocket.
- **« Auto » qui suit la fenêtre** : la qualité « Auto » d'un invité est mesurée à son arrivée (écran de l'appareil, décodeur) ; la refaire quand sa fenêtre change d'écran.
- **Partage lancé depuis le PC** : le premier invité fixe le format et le débit de l'image partagée.
- **Invité retiré** : il peut revenir avec le même code tant que l'hôte n'a pas demandé « Nouveau code ».
- **Public Suffix List** : inscrire `share.kybernow.app` à la liste, pour que les navigateurs traitent l'adresse de chaque Station (`<identifiant>.share.kybernow.app`) comme un site à part (cookies, stockage).
- **Lien montant de la maison** : le partager équitablement entre plusieurs invités.
- **Prêt de l'ordinateur** (en cours, 0.9.0 ; contrat : [docs/pret.md](docs/pret.md)) : à tour de rôle, en **mode console** dans votre propre session Windows (lanceur plein écran aux couleurs de KyberNow Share, apps choisies, écran du PC couvert, reprise avec 2 min de préavis, session verrouillée à la fin) ; code pour une séance, comptes d'amis, créneaux ; compte Steam au choix à chaque prêt.
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
- **Marque** : valider le nom KyberNow (et Station / Admin / Link / Share) avec l'équipe de Kyber ; l'identifiant `fr.leslignesclaires.kybernow` ne change pas.
