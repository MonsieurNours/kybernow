# KyberNow — prévu

[English](ROADMAP.en.md) · Français

Ce qui est prévu ou à essayer, et seulement ici : les autres documents décrivent ce qui existe ([docs/i18n.md](docs/i18n.md#documentation)). Une ligne sort de ce fichier quand elle est faite ; ce qu'elle a apporté va alors dans la doc de l'app concernée et dans `CHANGELOG.md`.

## Essais à faire

- **Steam Deck** : une session sur le Deck (l'app Linux n'est essayée en session que dans la VM).
- **Raspberry Pi** : l'app (.deb) et KyberNow Link OS sur une vraie carte : firmware (tryboot), Plymouth, décodeur V4L2, sortie HDMI (dont le 1080p à 120 Hz sur Pi 5), son, Bluetooth, CEC.
- **iPad** : écran externe et Stage Manager.
- **Android** : 1080p à 120 Hz sur la Shield, mise à jour de bout en bout depuis une version publiée, une tablette, une console portable, DeX et ChromeOS ; vibrations de la DualSense et de la manette Shield sur la Shield ; pertes de son sur la Shield en 4K 60 (Ethernet à 100 Mbit/s) à revoir en gigabit ou à débit plus bas.
- **Téléviseurs LG** : latence sur la G2 en 4K à 120 Hz, 1440p et 1080p, à 30, 60 et 120 i/s ; écart son-image et latence ajoutée du 5.1 Dolby ; Magic Remote en souris : mouvements envoyés quand le pointeur du téléviseur est caché.
- **Station** : le mode « Pour toute connexion » (écran virtuel créé en cours de flux).
- **Mesures de l'audit** décrites dans [docs/archives/mesures.md](docs/archives/mesures.md) : latence d'affichage de l'app Mac, droits Windows, Wi-Fi saturé, modes de l'écran virtuel SudoVDA.

## KyberNow Station et Admin

- **HDR** : capture en FP16 et conversion HDR10 dans Kyber (étude chiffrée : [docs/archives/propositions-2026-10-22.md](docs/archives/propositions-2026-10-22.md)).
- **Écran virtuel Kidd** quand Kyber le publiera, derrière l'interface déjà en place.
- **Pilote audio signé KyberNow** (certificat EV + Partner Center), si les haut-parleurs de Steam ne suffisent plus.
- **Pilote d'écran virtuel attesté** (EV + Partner Center), pour ne plus ajouter le certificat de SudoVDA aux autorités racines.
- **Signature Authenticode** de l'installateur, de l'app Windows et du paquet de mise à jour manuelle (SignPath visé ; préalable : binaires Windows construits en CI).
- **Wake-on-LAN** : Admin donne l'adresse MAC de l'ordinateur aux apps, et toutes les apps proposent le réveil (aujourd'hui : Android et Linux, adresse saisie à la main).
- **API du contrôleur** : inventorier ce que `kycontroller` expose en HTTP (état de session, métriques).
- **Question ouverte** : plusieurs utilisateurs simultanés sur le même ordinateur ?

## KyberNow Link

- **Windows** : session sur plusieurs écrans du PC.
- **Android** : manette à l'écran ; mise en page des pliables ; écran externe en pupitre (image sur l'écran branché, appareil en pupitre) ; Play Store en test interne.
- **Android TV + Station** : envoi des ordinateurs du téléphone au téléviseur (code affiché au téléviseur, relais par Admin).
- **iPhone et iPad** : manette virtuelle à l'écran, Apple Pencil ; TestFlight pour testeurs internes (profil App Store, archive Release, `xcodebuild -exportArchive`), puis l'App Store.
- **Apple TV** : TestFlight, puis l'App Store.
- **Téléviseurs LG** : dépôt Homebrew Channel, pour des mises à jour installées depuis le téléviseur ; vibrations et gâchettes adaptatives de la DualSense par le service Bluetooth de webOS.
- **Linux** : import DMA-BUF sans copie des images VA-API (4K à 120 Hz) ; décodage NVDEC pour les cartes NVIDIA ; dépôt Flatpak ou Flathub, pour des mises à jour installées seules.
- **Raspberry Pi** : publier le dépôt apt (clé GPG, hébergement, appel dans `release.sh`) ; voie sans copie (images SAND posées sur un plan DRM).
- **KyberNow Link OS** : installer la mise à jour pendant la veille ; une bande sonore composée pour l'allumage, à la place de la synthèse de `chime.rs`.

## Publication, CI, site, marque

- **Copies des anciens noms** (`KyberNow-Server-Setup.exe`, `KyberNow-Setup.exe`, `KyberNow-Client-Setup.exe`, `KyberNow-macOS.zip`) : les retirer quand les installations en 0.3.1 et avant auront disparu.
- **CI** : compiler l'app Mac (avec un `KyberSDK-macOS.zip`) et les binaires Windows.
- **Kyber** : remonter le correctif de `strip` (cargo-c, éditeur de liens d'Xcode 27).
- **Site** : retirer les marques « bêta » à la 1.0 ; revoir les cartes Apple TV, iPhone et iPad quand ces apps passeront par TestFlight ou l'App Store.
- **Marque** : valider le nom KyberNow (et Station / Admin / Link) avec l'équipe de Kyber ; l'identifiant `fr.leslignesclaires.kybernow` ne change pas.
