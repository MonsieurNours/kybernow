# Versions et mises à jour

[English](releases.en.md) · Français

Les versions de KyberNow sont publiées sur GitHub : https://github.com/MonsieurNours/kybernow/releases. Le dépôt est public : les mises à jour se téléchargent sans jeton, et le code reste disponible comme l'exige l'AGPL (avec les correctifs de Kyber, voir [kyber-changes.md](kyber-changes.md)).

Un seul numéro de version pour tout KyberNow : KyberNow Station (installateur, icône, services), KyberNow Admin et toutes les apps KyberNow Link changent ensemble. L'espace de travail `apps/` (`apps/Cargo.toml`) donne le numéro du cœur et des apps Windows, Android, webOS et Linux ; les `project.yml`, celui des apps Mac, Apple TV et iPhone/iPad. Les apps Apple TV et iPhone/iPad ne sont pas publiées en fichier : `release.sh` publie leur SDK, que leurs scripts de sideload téléchargent.

## Ce que contient une version

| Fichier | Pour quoi |
|---|---|
| `KyberNow-Station-Setup.exe` | KyberNow Station : première installation sur l'ordinateur Windows qui diffusera son image (installation complète, ou réinstallation quand une mise à jour l'exige) |
| `kybernow-update.zip` | Mise à jour d'une Station déjà installée : surcouche Kyber, Admin (avec le pack VB-CABLE dans `admin/vbcable`, [micro.md](micro.md)), icône, `update.ps1` |
| `KyberNow-Station-Mac.dmg` | Installateur de la Station pour Mac : « Installer KyberNow Station » et « Désinstaller KyberNow Station », signés Developer ID et notarisés (`server/macos/scripts/build-station-app.sh`) |
| `KyberNow-Station-Mac-update.zip` | Mise à jour d'une Station Mac déjà installée : l'app « KyberNow Station » entière, notarisée |
| `KyberNow-Station-Linux-x86_64.deb`, `-arm64.deb`, `-x86_64.rpm`, `-arm64.rpm` | KyberNow Station pour Linux (docs/station-linux.md) : première installation (`sudo apt install ./…deb`, `sudo dnf install ./…rpm`) et mise à jour depuis Admin, par le paquet de son architecture et de son format. Publiés dans chaque version (`KYBERNOW_STATION_LINUX=0` pour les laisser de côté) |
| `KyberNow-Station-Linux-x86_64.tar.xz`, `-aarch64.tar.xz` | KyberNow Station pour les systèmes immuables (Bazzite, Fedora Atomic, SteamOS), installée dans le dossier de l'utilisateur, et sa mise à jour depuis Admin (`kybernow-station-home`). Publiées avec les paquets |
| `update.ps1` | Mise à jour à la main, sans Admin |
| `KyberNow-Link-macOS.zip` | L'app Mac, « KyberNow Link.app » (première installation : la glisser dans Applications) ; archive du flux Sparkle |
| `appcast.xml` | Flux des mises à jour de l'app Mac (Sparkle), signé EdDSA |
| `KyberNow-Link-Setup.exe` | KyberNow Link pour Windows, x64 et ARM64 dans un seul fichier : installateur par utilisateur (première installation et mises à jour de l'app, voir [apps/windows/README.md](../apps/windows/README.md)) |
| `KyberNow-Link-Android.apk` | KyberNow Link pour Android et Android TV (arm64-v8a, armeabi-v7a, x86_64), signé par la clé de publication : première installation et mises à jour de l'app (voir [apps/android/README.md](../apps/android/README.md)) |
| `KyberNow-Link-webOS.ipk` | KyberNow Link pour les téléviseurs LG (webOS 5 et après, armv7), paquet installé en mode développeur (webOS Dev Manager ou `ares-install`, voir [apps/webos/INSTALLATION.md](../apps/webos/INSTALLATION.md)) : première installation et mises à jour |
| `KyberNow-Link-Linux-x86_64.flatpak`, `KyberNow-Link-Linux-aarch64.flatpak` | KyberNow Link pour Linux, le Steam Deck et SteamOS (runtime org.freedesktop.Platform 25.08, tiré de Flathub), paquet Flatpak par architecture installé par `flatpak install --user` (voir [apps/linux/INSTALLATION.md](../apps/linux/INSTALLATION.md)) : première installation et mises à jour |
| `KyberNow-Link-RaspberryPi-arm64.deb` | KyberNow Link pour le Raspberry Pi sous Raspberry Pi OS (Debian 13, arm64 ; Pi 3, 4 et 5, décodeurs de la puce par V4L2), paquet Debian installé par `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`, voir [apps/linux/INSTALLATION.md](../apps/linux/INSTALLATION.md) : première installation et mises à jour |
| `KyberNow-Link-OS.img.xz` | KyberNow Link OS : l'image à flasher sur un Raspberry Pi (Pi 3, 4 et 5, arm64 ; Raspberry Pi OS Lite + Link seul, disposition A/B), avec Raspberry Pi Imager (fichier, ou le dépôt `https://www.kybernow.app/updates/rpi-imager.json`), voir [apps/linux/INSTALLATION.md](../apps/linux/INSTALLATION.md) : première installation ; les mises à jour A/B, l'OS les télécharge lui-même dans chaque version (voir [apps/linux/os/README.md](../apps/linux/os/README.md)) |
| `KyberSDK-tvOS.zip` | SDK de Kyber et cœur KyberNow (`KyberNow.xcframework`) compilés pour l'Apple TV, que télécharge `apps/tvos/scripts/sideload.sh` pour installer l'app sans l'App Store ([SIDELOAD.md](../apps/tvos/SIDELOAD.md)) |
| `KyberSDK-iOS.zip` | Cœur KyberNow, libkynput, libopus et leurs en-têtes compilés pour l'iPhone et l'iPad (`apps/ios/Vendor`, joint aux versions 0.4.0 et suivantes), que télécharge `apps/ios/scripts/sideload.sh` pour installer l'app sans TestFlight ni App Store ([SIDELOAD.md](../apps/ios/SIDELOAD.md)) |
| `KyberNow-Server-Setup.exe`, `KyberNow-Setup.exe`, `KyberNow-Client-Setup.exe`, `KyberNow-macOS.zip` | Anciens noms des installateurs et de l'archive Mac, copies identiques : voir « Anciens noms des installateurs » ci-dessous |
| `kybernow-release.json` | Manifeste de la version : numéro, canal (`stable` ou `beta`), date, version minimale déjà installée pour une mise à jour depuis Admin, réinstallation nécessaire ou non, empreinte SHA-256 et taille de chaque fichier ci-dessus (anciens noms compris) |
| `kybernow-release.json.minisig` | Signature du manifeste (minisign, ed25519), vérifiée par Admin avant toute installation |
| `SHA256SUMS` | Empreintes de tous les fichiers (anciens noms compris) |

### Anciens noms des installateurs

Les installateurs s'appellent `KyberNow-Station-Setup.exe` (la Station) et `KyberNow-Link-Setup.exe` (l'app Windows), l'archive de l'app Mac `KyberNow-Link-macOS.zip`. Leurs anciens noms sont aussi publiés, **à l'identique** (copies faites par `release.sh`, même empreinte) :

| Ancien nom | Copie de | Pourquoi il reste |
|---|---|---|
| `KyberNow-Server-Setup.exe` | `KyberNow-Station-Setup.exe` | Admin en 0.3.1 et avant renvoie vers ce nom (« réinstallez avec KyberNow-Server-Setup.exe ») ; liens existants vers `releases/latest/download/` (site, guide, messages) |
| `KyberNow-Setup.exe` | `KyberNow-Station-Setup.exe` | Nom de l'installateur avant la 0.3.0 : liens existants |
| `KyberNow-Client-Setup.exe` | `KyberNow-Link-Setup.exe` | L'app Windows en 0.3.1 et avant le cherche dans le manifeste signé et dans la release pour se mettre à jour ; liens existants |
| `KyberNow-macOS.zip` | `KyberNow-Link-macOS.zip` | Nom de l'archive Mac jusqu'à la 0.4.2 : liens existants (le flux Sparkle, lui, pointe vers l'archive de chaque version) |

Admin et l'app Windows cherchent d'abord le nouveau nom, puis l'ancien s'il manque (manifeste d'une version antérieure à la 0.4.0). Le manifeste signé et `SHA256SUMS` listent les deux noms, avec la même empreinte.

**Pourquoi elles restent.** Sans elles, une app Windows restée en 0.3.1 ou avant ne trouverait plus sa mise à jour, et Admin en 0.3.1 ou avant renverrait vers un fichier absent : il faudrait réinstaller à la main avec le nouveau fichier. Les copies sont listées dans `LEGACY_STATION`, `LEGACY_LINK` et `LEGACY_MAC` de `release.sh` et dans `FILES` de `scripts/sign-manifest.sh` ; les retirer de ces listes les retire des versions suivantes.

**Piège.** Le site est mis en ligne à chaque push sur `main` qui touche `site/` : un lien `releases/latest/download/` vers un nouveau nom de fichier ne mène à rien tant que la version qui contient ce fichier n'est pas publiée. Pour les fichiers renommés (installateurs, archive Mac), `site/assets/main.js` (table `LEGACY`) retombe sur l'ancien nom tant que la dernière version ne publie que lui ; tout autre nouveau nom de fichier attend la publication.

## Mettre à jour

**La Station, en un clic.**
- Admin vérifie la dernière version toutes les 6 heures, sur son **canal** : `stable` (par défaut, versions publiées) ou `beta` (aussi les préversions GitHub), à choisir sur la page Mises à jour (réglage `update_channel` de `kyber-admin.toml`). La page **Mises à jour** l'affiche avec ses notes et un bouton **Installer** ; l'icône de la zone de notification propose « Installer la version X… », qui ouvre cette page.
- **Depuis les apps KyberNow Link** (0.11.3) : quand une Station a une version, l'accueil de l'app la propose dans un bandeau (« Mettre à jour », « Plus tard » pour 7 jours), et le premier lancement après une mise à jour de l'app rassemble dans un avis les Stations à mettre à jour. Juste après sa propre mise à jour, ou face à une Station plus ancienne, l'app demande à Admin de vérifier tout de suite (`POST /api/stream/station/check`, au plus une fois par jour par app et une fois par 10 minutes côté Admin) : sans cela, la Station pouvait ignorer la version jusqu'à 6 heures. Règles et textes : docs/link-ui.md, « Mise à jour des Stations ».
- **Rien ne s'installe sans signature.** Pour une version plus récente, Admin télécharge `kybernow-release.json` et `kybernow-release.json.minisig` et vérifie la signature avec la clé publique compilée dans son programme (voir « Clé de signature des mises à jour »). Puis : le manifeste annonce le même numéro que la version GitHub (`tag_name`), un canal que suit la Station (une version `beta` est refusée en `stable`), une `min_installer_version` inférieure ou égale à la version installée et pas de `requires_setup` (sinon « réinstallez avec KyberNow-Station-Setup.exe »). Tout échec (manifeste absent, signature invalide ou d'une autre clé, canal, version) est affiché sur la page et dans `GET /api/update` (`verify_error`), et le bouton **Installer** n'apparaît pas ; l'icône ne propose une version que si sa signature est bonne.
- L'installation retélécharge et revérifie le manifeste, puis télécharge `kybernow-update.zip` (jamais plus que la taille du manifeste) et vérifie sa taille et son empreinte SHA-256 contre lui. Le numéro de la version publiée doit être de la forme `X.Y.Z` : il sert de nom de dossier, tout autre numéro est refusé. Manifeste, signature et paquet restent dans `C:\ProgramData\KyberNow\update\<version>\` (réservé à SYSTEM et aux Administrateurs).
- Admin lance ensuite **l'applicateur**, `kyber-admin update apply <paquet> --version X`, en SYSTEM et détaché (il survit à l'arrêt d'Admin). C'est une copie d'Admin, `C:\Program Files\KyberNow\.staging\kyber-admin-maj.exe` : elle doit pouvoir renommer le dossier `admin` pendant qu'elle tourne. Rien n'est exécuté depuis un dossier qu'un autre compte pourrait modifier, et aucun script n'est lancé : PowerShell n'intervient pas.
- L'applicateur revérifie le manifeste signé et le paquet (rien ne vient du processus Admin qui l'a lancé), puis extrait le paquet dans `C:\Program Files\KyberNow\.staging\<version>\paquet` (droits hérités de Program Files : lecture seule pour les utilisateurs). Il refuse tout chemin qui sortirait du dossier (`..`, chemin absolu, lecteur, flux NTFS), tout nom qui ne serait pas un fichier ordinaire sous Windows (`CON`, `NUL`, `COM1`…, nom finissant par un point ou une espace, `<>"|?*`) et tout lien. Il refuse aussi une version qui n'est pas plus récente que celle installée (le retour en arrière a son propre chemin). Le paquet doit contenir `kyber`, `admin`, `tray` et un `version.txt` égal à la version attendue. Un échec à ce stade ne touche à rien : Admin tourne toujours et l'affiche.
- Il arrête l'icône, puis le serveur Kyber (`kyber` et ses processus), puis Admin (`kyber-admin` et ses agents), puis encore le serveur Kyber si Admin l'avait relancé entre-temps. Il arrête aussi les aides du micro et de la caméra (`kybernow-micro.exe`, `kybernow-camera.exe`, même liste que l'installateur et `update.ps1` : `server/admin/src/update/windows_common.rs`) et Frame Server (la caméra de Windows 11) ; une bibliothèque de la caméra encore chargée par une app qui filme est mise de côté dans `.staging\verrouilles\` (effacée au redémarrage). La relance automatique de `kyber` et de `kyber-admin` après une panne est suspendue jusqu'au redémarrage, pour que Windows ne les relance pas au milieu des renommages ; Admin la repose de toute façon à chaque démarrage. Une seule installation à la fois : l'applicateur, `update.ps1` et l'installateur prennent le verrou `Global\KyberNowUpdate`. Il prépare chaque nouveau dossier dans `.staging\<version>\` (copie du dossier actuel, réglages et certificat de Kyber compris, puis les fichiers du paquet par-dessus), puis bascule par renommage : `kyber` devient `kyber.prev` et `.staging\<version>\kyber` devient `kyber`, de même pour `admin` et `tray`. Les journaux de Kyber (`kyber\log`) sont déplacés, pas copiés. Il remet ensuite les droits des services (`sc sdset`) et la règle de pare-feu Bonjour.
- **Gel du PC pendant ou juste après la mise à jour.** Le 03/11, juste après la 0.9.3, un gel a laissé `kyber_config.toml` et `kybertest_key.pem` remplis de zéros (des fichiers écrits que Windows n'avait pas encore mis sur le disque) : Kyber ne démarrait plus. Depuis la 0.9.4 : avant chaque mise à jour et chaque retour arrière, l'applicateur sauvegarde dans `C:\ProgramData\KyberNow\backups\` les réglages, le certificat et la clé de Kyber s'ils sont lisibles (`kyber_config-<date>.toml`, `kybertest_cert-<date>.pem`, `kybertest_key-<date>.pem`) ; chaque fichier extrait ou copié est forcé sur le disque (`FlushFileBuffers`) avant la bascule, les renommages sont écrits tout de suite (`MOVEFILE_WRITE_THROUGH`), et tout le volume est forcé sur le disque avant et après la bascule et après un retour. Si un fichier est quand même abîmé, Admin le répare à son démarrage (voir server/README, « Réglages de Kyber abîmés »).
- Il redémarre les services, puis contrôle la santé de la nouvelle version : en 60 s, Admin doit répondre à l'adresse de son réglage `listen` (127.0.0.1 s'il écoute partout, sans proxy) et `GET /api/session` donner la nouvelle version, et le serveur Kyber tourner. Sinon, il arrête tout (l'icône comprise, que la nouvelle version d'Admin a pu relancer), remet chaque dossier où il était (`kyber.prev` redevient `kyber`…), redémarre l'ancienne version et écrit la raison dans le journal et dans `C:\ProgramData\KyberNow\update\resultat.json`. Admin relit ce fichier à son démarrage : la page Mises à jour affiche l'échec, et `GET /api/update` le donne dans `install_error` et `last_failure` (version, raison, retour arrière, journal). Si un dossier résiste au retour arrière, l'applicateur remet les autres, garde `.staging\<version>`, l'écrit (`undo_failed`) et la page le dit, même si c'est la version en échec qui tourne.
- **Coupure pendant la bascule.** Avant le premier renommage, l'applicateur écrit `update\bascule.json` (forcé sur le disque) ; il l'efface quand tous les dossiers sont en place, et le réécrit pendant un retour arrière. Si le courant coupe entre deux renommages, Admin le trouve à son démarrage : la console qui tourne (l'ancienne, ou la nouvelle si son dossier a déjà basculé) finit le travail dans son sens, toute partie absente reprise de `.prev` ou de `.staging`, et la page Mises à jour le dit. Si c'est `admin\` qui manque, l'installateur s'en charge avant de chercher les réglages à garder.
- Quoi qu'il arrive après l'arrêt des services (erreur, renommage impossible parce qu'un fichier est ouvert), l'applicateur les redémarre en sortant : la Station n'est jamais laissée sans serveur Kyber ni Admin.
- Réussie, la mise à jour inscrit la nouvelle version dans `C:\Program Files\KyberNow\kybernow.json` et dans « Applications installées » (`DisplayVersion`), et `resultat.json` dit `"ok": true`. La version précédente reste dans les dossiers `.prev` jusqu'à la mise à jour suivante (environ 300 Mo). Dans `update\`, seuls les dossiers des deux dernières versions sont gardés ; `.staging\<version>` est effacé.
- Admin relance l'icône dans la session de l'utilisateur, avec son compte (fichier `update\relaunch-tray`). La page se met à jour seule.
- **Revenir à la version précédente** : tant que les dossiers `.prev` existent, la page Mises à jour propose ce bouton (`POST /api/update/rollback`, version précédente dans `rollback` de `GET /api/update`). Le même applicateur fait la bascule en sens inverse (`kyber-admin update rollback`) : `kyber.prev` redevient `kyber` et la version retirée part en `kyber.bad`, de même pour `admin` et `tray` ; les réglages de Kyber changés depuis la mise à jour (`kyber_config.toml`) et ses journaux sont gardés, les réglages seulement s'ils sont lisibles (TOML valide, sans octets nuls, table `[kycontroller]`) : sinon ceux de `kyber.prev` restent, et le journal le dit. Le 03/11, la copie du fichier abîmé avait écrasé le bon. Le retour automatique après un échec ne recopie rien : il remet les anciens dossiers tels quels. Même contrôle de santé, avec la version précédente ; s'il échoue, la version qui tournait est remise. Réussi, le retour efface les dossiers `.bad` : il ne reste plus de `.prev`, et la version retirée est de nouveau proposée à l'installation.
- **Pendant une session.** Si un appareil est en session, `POST /api/update/install` (et `POST /api/update/rollback`) répond 409 avec « Un appareil est connecté (…) : l'installation coupera sa session » ; la page le demande avant de continuer, et renvoie alors `{"force": true}`.
- Durée : environ une minute. Une session de streaming en cours est coupée. Journal : `C:\ProgramData\KyberNow\update\<version>\update.log` ; sortie de l'applicateur dans `lancement.log`, à lire si `update.log` n'existe pas.
- Admin suit l'applicateur. Si celui-ci s'arrête alors qu'Admin tourne encore (vérification ou extraction refusée), Admin l'indique : page Mises à jour, fin du journal, et « Installer » redevient possible.
- Les haut-parleurs virtuels de Steam, posés par l'installateur, ne sont pas refaits par l'applicateur ; `update.ps1` (à la main) les refait. Après une bascule réussie, l'applicateur pose les noms affichés de l'installateur (services, « Applications installées », raccourcis du menu Démarrer) : une Station installée en 0.3.1 ou avant prend ainsi les noms KyberNow Station et KyberNow Admin. Un échec à cette étape est seulement écrit dans le journal.
- **Stations en 0.2.5 et avant.** Leur console (le nom d'Admin dans ces versions) ne connaît ni le manifeste ni l'applicateur : elle installe le paquet avec `SHA256SUMS` et le `update.ps1` qu'il contient. C'est une raison de plus pour que chaque version publie `SHA256SUMS` et `update.ps1`, dont le chemin manuel se sert aussi.

**La Station, à la main** (PowerShell administrateur) :

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

Le script télécharge `kybernow-update.zip` et `SHA256SUMS`, vérifie l'empreinte du paquet et ses exigences (`requirements.json` : une version qui exige l'installateur, ou une Station installée avant `min_installer_version`, est refusée avec un message qui renvoie vers `KyberNow-Station-Setup.exe`, avant d'arrêter quoi que ce soit), refuse une version qui n'est pas plus récente que celle installée (sauf `-Force` : `& ([scriptblock]::Create((irm …/update.ps1))) -Force`), puis fait la même bascule avec contrôle de santé. En cas d'échec, il le dit en rouge (« Mise a jour non installee… ») après avoir tout redémarré.

**Chemin manuel : intégrité seulement.** `update.ps1` vérifie que le paquet n'est pas abîmé (`SHA256SUMS`), pas qu'il vient de KyberNow : `SHA256SUMS` est publié au même endroit que le paquet, et vérifier le manifeste minisign demanderait un outil téléchargé lui aussi (`kyber-admin.exe` de la version), ce qui tourne en rond. Le script lui-même n'est pas signé. Il n'apporte donc pas les garanties d'Admin : à réserver au dépannage, sur une version dont vous connaissez la page GitHub. Les `.exe` et `update.ps1` ne sont pas signés Authenticode (voir « Signature Windows (Authenticode) »).

Pour essayer un paquet servi depuis le Mac avant de le publier : `$env:KYBERNOW_UPDATE_BASE = 'http://<adresse-du-Mac>:8000'`, puis la même commande, avec `python3 -m http.server 8000` lancé dans `server/installer/dist` (`package-update.sh` y écrit aussi `SHA256SUMS`).

**Tester la vérification et l'applicateur sans Windows** : `cd server/admin && cargo test`. Les tests couvrent le manifeste (lecture du format publié, comparaison des versions, canal, `min_installer_version`, `requires_setup`), la signature (clé d'essai générée dans le test : signature valide, manifeste modifié d'un octet, autre clé, commentaire de confiance qui ne correspond pas), un manifeste réellement produit par `scripts/sign-manifest.sh` et le binaire `minisign` (`server/admin/src/update/testdata/`, clé d'essai jetable dont seule la clé publique est gardée), l'extraction (`..`, chemin absolu, lecteur, liens), la bascule sur un faux dossier d'installation (réussite, contrôle de santé raté puis retour arrière, paquet falsifié qui ne touche à rien, retour à la version précédente et son échec) et le refus pendant une session. Les services, processus et droits de Windows passent par un trait simulé : ils ne sont vérifiés que sur le PC. Pour régénérer `testdata/` : `KYBERNOW_MINISIGN_KEY=<dossier temporaire>/essai.key KYBERNOW_MINISIGN_ITEM=kybernow-minisign-essai scripts/sign-manifest.sh <dossier> 0.2.7 beta --min-installer 0.2.6` sur un `kybernow-update.zip` de 15 octets, puis supprimer la clé privée et l'élément du Trousseau.

**Tester `update.ps1` sans Windows** : `pwsh -NoProfile -File server/installer/tests/update-bench.ps1` (`brew install powershell`). Le banc simule les services et Admin, et rejoue une mise à jour réussie, Admin muet (retour arrière), un paquet incomplet, un renommage bloqué, et un paquet qui exige l'installateur ou une Station plus récente.

**L'app Windows (KyberNow Link).** Elle cherche au lancement puis toutes les 6 heures la dernière version de son canal (réglages de l'app : stables, ou aussi les bêtas), vérifie le manifeste signé comme Admin et l'installateur (`KyberNow-Link-Setup.exe`, ou `KyberNow-Client-Setup.exe` dans le manifeste d'une version antérieure à la 0.4.0) contre lui (taille, SHA-256), puis le lance en silence (`/S /UPDATE`) : il attend la fermeture de l'app, remplace ses fichiers et la relance. Une version sans installateur Windows n'est pas proposée.

**L'app Android.** Elle cherche au lancement, au retour dans l'app puis toutes les 6 heures la dernière version de son canal qui publie `KyberNow-Link-Android.apk` (réglages de l'app › Mises à jour : stables, ou aussi les bêtas). Le cœur Rust vérifie le manifeste signé comme pour l'app Windows ; l'APK téléchargé doit avoir sa taille et son empreinte SHA-256. Un bouton le télécharge, puis l'app l'installe par PackageInstaller : Android demande une fois d'autoriser l'app à installer des applications (sur la Shield : Réglages › Applications › Sécurité et restrictions › Sources inconnues), puis confirme chaque installation, sauf à partir d'Android 12 où l'app se met à jour elle-même sans question. Android refuse un APK signé par une autre clé que l'app installée : une app compilée par Android Studio (clé de débogage) doit être désinstallée une fois avant d'installer l'APK publié.

**L'app des téléviseurs LG.** Elle cherche au lancement puis toutes les 6 heures la dernière version de son canal qui publie `KyberNow-Link-webOS.ipk` (Réglages › Ce téléviseur › Mises à jour : stables, ou aussi les bêtas), vérifiée par le cœur Rust comme pour Android (manifeste signé, empreinte et taille du paquet). Une app installée en mode développeur ne peut pas s'installer elle-même : elle affiche « Version X disponible » sur l'accueil et dans ses réglages, et le paquet s'installe depuis l'ordinateur (webOS Dev Manager ou `ares-install`). `release.sh` le construit avec `apps/webos/scripts/build.sh` (chaîne webOS téléchargée au premier appel, cible Rust `armv7-unknown-linux-gnueabi`, sources de Kyber).

**L'app Linux.** Elle cherche au lancement puis toutes les 6 heures la dernière version de son canal qui publie le Flatpak de son architecture (`KyberNow-Link-Linux-x86_64.flatpak` ou `-aarch64`), vérifié par le cœur Rust (manifeste signé, empreinte et taille). Un paquet Flatpak installé à la main ne se met pas à jour seul : l'app affiche « Version X disponible » et le nouveau paquet s'installe par-dessus (`flatpak install --user`). `release.sh` construit les deux paquets avec `apps/linux/scripts/release-flatpaks.sh`, dans la VM Parallels « KyberNow Linux » (Ubuntu ARM64, Rosetta pour x86_64, dépôt en dossier partagé), démarrée au besoin ; le même script y construit ensuite le paquet .deb du Raspberry Pi (`apps/linux/scripts/deb.sh`, FFmpeg de Raspberry Pi). Sur le Pi, un paquet installé depuis le fichier se met à jour comme le Flatpak (« Version X disponible », puis `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`) . `apps/linux/scripts/apt-repo.sh` assemble un dépôt apt depuis les paquets .deb (signé si une clé GPG est donnée), à servir par un site statique ; `release.sh` ne l'appelle pas et aucun dépôt n'est publié. **KyberNow Link OS** : `release.sh` construit ensuite l'image dans la même VM (`apps/linux/os/release-image.sh`, pi-gen, long) et écrit le dépôt JSON de Raspberry Pi Imager (`apps/linux/os/imager-json.sh` → `site/updates/rpi-imager.json`, commité avec le flux du site) ; sur le Pi, l'OS cherche `KyberNow-Link-OS.img.xz` dans chaque version (manifeste signé), la télécharge et l'écrit dans l'autre emplacement A/B (Réglages de l'appareil › Mise à jour).

**L'app Mac.**
- Sparkle vérifie chaque jour, et à la demande (menu KyberNow Link › Rechercher des mises à jour…). L'app télécharge l'archive, vérifie sa signature EdDSA avec la clé publique de l'app (`SUPublicEDKey`), la remplace et se relance.
- L'app lit le flux cumulatif du site, `https://www.kybernow.app/updates/appcast.xml`, qui liste toutes les versions. Elle ne propose que les stables, sauf si « Recevoir les versions bêta » est coché dans KyberNow Link › Réglages… : voir « Flux des mises à jour de l'app Mac ».
- La clé privée est dans le Trousseau de ce Mac (élément créé par `generate_keys` de Sparkle). Sauvegardez-la, sans jamais la mettre dans le dépôt :

```bash
apps/macos/build/SourcePackages/artifacts/sparkle/Sparkle/bin/generate_keys -x ~/sparkle-kybernow.key
```

Sans elle, aucune mise à jour de l'app ne pourrait plus être signée. Pour la remettre sur un autre Mac : `generate_keys -f ~/sparkle-kybernow.key`.

**La Station Mac** (docs/station-macos.md, jalon 6) suit le même chemin jusqu'au paquet vérifié, avec `KyberNow-Station-Mac-update.zip` au lieu de `kybernow-update.zip` ; une version dont le manifeste ne liste pas ce paquet est refusée sur Mac (« réinstallez avec `KyberNow-Station-Mac.dmg` » si elle exige l'installateur). L'applicateur (`server/admin/src/update/mac.rs`) est une copie d'Admin lancée par launchd en service à part (`fr.leslignesclaires.kybernow.station.update`) : launchd arrêterait avec Admin tout ce qu'Admin lance. Il extrait l'app (`ditto`), revérifie sa signature (`codesign --verify --deep --strict`, identifiant `fr.leslignesclaires.kybernow.station`, équipe de KyberNow compilée dans Admin, `P6DB682S4K`, vérifiée par une exigence de signature, version attendue), arrête l'icône, Admin et Kyber, met l'app en place par renommage (l'ancienne dans `update/previous`, pour « Revenir à la version précédente »), relance tout et attend qu'Admin réponde dans la nouvelle version en 60 s, sinon remet l'ancienne. Même signature : macOS garde les autorisations, et rien ne demande le mot de passe du Mac ; une sortie son plus récente dans l'app est seulement notée au journal (la poser demanderait ce mot de passe). Journal dans `~/Library/Application Support/KyberNow/Station/admin/update/<version>/update.log`, résultat dans `update/resultat.json` comme sous Windows. La fenêtre « Mettre à jour » de l'icône passe par les routes locales d'Admin (`GET /api/local/update`, `POST /api/local/update/check`, `POST /api/local/update/install`, jeton local et certificat épinglé) et suit la Station jusqu'à son retour.

**La Station Linux** (docs/station-linux.md, « Mises à jour et icône ») suit le même chemin jusqu'au paquet vérifié, avec le paquet de son architecture et de son format (`KyberNow-Station-Linux-{x86_64,arm64}.{deb,rpm}`) ; une version qui ne le publie pas est refusée sous Linux. Admin, qui n'est jamais root, dépose sa demande sur le socket `/run/kybernow-station/update.sock` ; le service root `kybernow-station-update@.service` (`kyber-admin update serve`, `server/admin/src/update/linux.rs`) n'accepte que le compte de la Station (le premier qui a demandé une mise à jour, noté dans `proprietaire`), une demande à la fois (`flock`) et 10 s pour la lire ; il copie les fichiers du compte qui demande, ouverts avec ses droits et sans suivre de lien, dans `/var/lib/kybernow-station/update/<version>/` (0700, fichiers en 0600, effacés si la vérification échoue), y revérifie tout (signature, version plus récente que le paquet installé, canal, empreinte), répond à Admin, puis arrête l'icône, Kyber et Admin de ce compte, installe le paquet (apt ou dpkg, dnf ou rpm), relance les services et attend qu'Admin réponde dans la nouvelle version en 60 s, sinon réinstalle la version qui tournait (son paquet publié, téléchargé et vérifié par Admin avant l'installation, gardé par le service root). « Revenir à la version précédente » réinstalle de même la version gardée. Journal dans `/var/lib/kybernow-station/update/<version>/update.log` (et `journalctl -u 'kybernow-station-update@*'`), résultat dans `resultat.json` à côté, au même format. Publication : `release.sh` construit les quatre paquets avec `server/linux/scripts/release-station.sh` (arm64 dans la VM « KyberNow Linux », x86_64 sur la machine `KYBERNOW_STATION_X86_SSH` si elle est réglée, sinon dans le chroot x86_64 de la même VM par Rosetta, `server/linux/scripts/x86-chroot.sh` ; dépôt au même commit) sauf avec `KYBERNOW_STATION_LINUX=0`, et `sign-manifest.sh` les liste dans le manifeste. Une version publiée avec `KYBERNOW_STATION_LINUX=0` ne les a pas, et une Station Linux ne s'y met pas à jour.

## Clé de signature des mises à jour (minisign)

Chaque version publie un manifeste, `kybernow-release.json`, signé avec une clé ed25519 (minisign), que vérifient Admin et le cœur Rust des apps qui se mettent à jour seules ou signalent leurs versions (Windows, Android, téléviseurs LG, Linux, KyberNow Link OS) :

```json
{
  "version": "0.2.6",
  "channel": "stable",
  "published": "2026-10-21T08:00:00Z",
  "min_installer_version": "0.2.0",
  "requires_setup": false,
  "files": {
    "kybernow-update.zip": { "sha256": "…", "size": 123456789 },
    "…": {}
  }
}
```

`files` liste ceux des fichiers publiés (`scripts/release-fichiers.txt`, la liste que lit aussi `release.sh`) qui sont présents (`KyberNow-Station-Setup.exe`, `kybernow-update.zip`, `update.ps1`, `KyberNow-Link-macOS.zip`, `KyberNow-macOS.zip`, `appcast.xml`, `KyberSDK-tvOS.zip`, `KyberSDK-iOS.zip`, `KyberNow-Link-Setup.exe`, `KyberNow-Link-Android.apk`, `KyberNow-Link-webOS.ipk`, `KyberNow-Link-Linux-x86_64.flatpak`, `KyberNow-Link-Linux-aarch64.flatpak`, `KyberNow-Link-RaspberryPi-arm64.deb`, `KyberNow-Link-OS.img.xz`, et les anciens noms des installateurs `KyberNow-Server-Setup.exe`, `KyberNow-Setup.exe`, `KyberNow-Client-Setup.exe`, voir « Anciens noms des installateurs »). La signature détachée, `kybernow-release.json.minisig`, porte le commentaire de confiance `kybernow <version> <canal>`. La clé publique est dans le dépôt, `server/admin/src/update/kybernow-release.pub` (identifiant `D63B24FFFC7E79D1`), et compilée dans Admin et dans le cœur des apps (`apps/core/src/update.rs`) : une empreinte SHA-256 seule protège d'un téléchargement abîmé, pas d'une version falsifiée par qui aurait la main sur le compte GitHub ; la signature, si.

Le manifeste s'écrit et se signe avec :

```bash
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable            # ou beta
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable --min-installer 0.2.6 --requires-setup
```

`--min-installer X.Y.Z` : version la plus ancienne depuis laquelle Admin peut installer ce paquet (0.2.0 par défaut). `--requires-setup` : cette version ne s'installe qu'avec `KyberNow-Station-Setup.exe` (nouveau pilote, nouveau service…). Le script installe `minisign` avec Homebrew s'il manque, puis vérifie la signature contre la clé publique du dépôt : une clé privée qui ne lui correspond pas est vue avant la publication.

**La clé privée** est dans `~/Library/Application Support/KyberNow/minisign.key`, chiffrée par un mot de passe aléatoire gardé dans le Trousseau (élément `kybernow-minisign`). Le script la crée si elle manque et écrit la clé publique dans le dépôt ; si une clé publique est déjà dans le dépôt, il refuse d'en créer une autre : une nouvelle clé ferait refuser toutes les mises à jour par les Stations déjà installées, qu'il faudrait réinstaller avec `KyberNow-Station-Setup.exe`.

**Sauvegarde hors ligne** (clé USB, coffre de mots de passe), comme la clé Sparkle, sans jamais les mettre dans le dépôt : la clé privée et son mot de passe, l'un sans l'autre ne sert à rien.

```bash
cp ~/Library/Application\ Support/KyberNow/minisign.key /Volumes/<sauvegarde>/kybernow-minisign.key
security find-generic-password -s kybernow-minisign -w     # mot de passe, à recopier dans le coffre
```

**Remise sur un autre Mac** :

```bash
mkdir -p ~/Library/Application\ Support/KyberNow
cp /Volumes/<sauvegarde>/kybernow-minisign.key ~/Library/Application\ Support/KyberNow/minisign.key
chmod 600 ~/Library/Application\ Support/KyberNow/minisign.key
security add-generic-password -a kybernow -s kybernow-minisign -w   # colle le mot de passe à l'invite
```

Puis un essai à blanc sur un dossier où l'on a mis un `update.ps1` quelconque (`scripts/sign-manifest.sh <dossier> 0.0.0 stable`, suivi de `minisign -V -p server/admin/src/update/kybernow-release.pub -m <dossier>/kybernow-release.json`) confirme que la clé correspond au dépôt.

Le mot de passe n'est lisible que Mac déverrouillé, comme le profil de notarisation.

## Clé de signature de l'app Android

Android n'installe une mise à jour que si elle est signée par la même clé que l'app installée : cette clé ne doit jamais changer ni se perdre. `apps/android/scripts/release-apk.sh` (lancé par `release.sh`) la crée au premier appel :

- `~/Library/Application Support/KyberNow/android-release.jks` (PKCS12, alias `kybernow`, RSA 4096, 100 ans), protégée par un mot de passe aléatoire gardé dans le Trousseau (élément `kybernow-android-keystore`) ;
- l'empreinte SHA-256 de son certificat dans le dépôt, `apps/android/scripts/release-cert.sha256`, à commiter : le script refuse ensuite une autre clé, et vérifie avec `apksigner` que chaque APK publié est signé par ce certificat.

Si l'empreinte est dans le dépôt et la clé absente, le script refuse d'en créer une autre : remettez la sauvegarde.

**Sauvegarde hors ligne**, avec la clé minisign, sans jamais la mettre dans le dépôt :

```bash
cp ~/Library/Application\ Support/KyberNow/android-release.jks /Volumes/<sauvegarde>/
security find-generic-password -s kybernow-android-keystore -w     # mot de passe, à recopier dans le coffre
```

**Remise sur un autre Mac** : recopier le fichier au même endroit (`chmod 600`), puis `security add-generic-password -a kybernow -s kybernow-android-keystore -w` (coller le mot de passe), et vérifier avec `apps/android/scripts/release-apk.sh --check`.

## Publier une version

0. Vérifier que la CI est verte sur `main` (onglet Actions du dépôt, voir « Intégration continue ») : elle ne couvre pas l'app Mac, que la répétition compile.
1. Ajouter une section `## X.Y.Z` en haut de `CHANGELOG.md` : ce sont les notes affichées par Admin et par les apps. Ajouter la même section, en anglais, à `CHANGELOG.en.md`. Commiter : le script refuse un arbre modifié. Les deux langues partent avec la version ([docs/i18n.md](i18n.md)) : la release GitHub porte la section française, une ligne `<!-- en -->`, puis l'anglaise (Admin et les apps qui se mettent à jour par le cœur Rust prennent la partie de leur langue) ; dans le flux Sparkle, l'anglais est la description par défaut et le français un lien par langue (`xml:lang="fr"`) vers `site/updates/notes/X.Y.Z/KyberNow-Link-macOS.fr.html` (`scripts/notes-html.py`), commité avec le flux du site.
2. Rien à recompiler à la main : la surcouche Kyber de la Station Windows, le Kyber de la Station Mac, les bibliothèques client des apps et libkynput sont recompilés par la publication, dans ses arbres de construction, quand leurs correctifs ont changé. KyberNow Link peut rester ouvert (les arbres ont leur propre `Vendor/`). Puis, depuis `main` :

```bash
scripts/release.sh X.Y.Z --check        # vérifications seules (une dizaine de secondes)
scripts/release.sh X.Y.Z --repetition   # répétition complète, rien n'est commité ni publié
scripts/release.sh X.Y.Z                # publication
scripts/release-suivi.py X.Y.Z          # dans un autre terminal : grandes étapes cochées et pourcentage
```

**Suivi.** `scripts/release-suivi.py X.Y.Z` redessine toutes les deux secondes les grandes étapes (vérifications, versions, arbres, les quinze constructions (quatorze sans la Station Linux), fichiers, flux, manifeste, publication), cochées au fur et à mesure, avec une barre et un pourcentage (constructions 80 %, avant 5 %, après 15 %) ; il lit le journal `~/Library/Logs/kybernow-release-X.Y.Z.log` (lancer la publication avec `2>&1 | tee` vers ce fichier) et ne pilote rien.

**Ordre.** Les binaires publiés doivent être ceux du commit étiqueté (l'AGPL oblige à publier le code exact des binaires) :
1. vérifications préalables, rapides et toutes affichées d'un coup (tableau plus bas) ; une seule en échec et rien n'est modifié ;
2. alignement des versions (Admin, icône, installateur de la Station, espace de travail `apps/` — cœur et apps Windows, Android, webOS et Linux —, installateur et `tauri.conf.json` de l'app Windows, projets Xcode des apps Mac, Apple TV et iPhone/iPad, fiche AppStream du Flatpak, `Cargo.lock` compris) dans l'arbre `third_party/arbres/versions`, et commit « KyberNow X.Y.Z » ; `main` avance jusqu'à lui (`git merge --ff-only`) ; l'alignement qui toucherait un autre fichier arrête tout ;
3. constructions depuis ce commit, chacune dans son **arbre de construction** ([build-notes.md](build-notes.md), « Arbres de construction »), en cinq voies parallèles :
   - Windows, Android, webOS : surcouche Kyber de la Station Windows (si ses correctifs ont changé), installateur de la Station et paquet de mise à jour, app Windows x64 et ARM64 et son installateur `KyberNow-Link-Setup.exe`, APK Android signé (`apps/android/scripts/release-apk.sh`), paquet webOS ;
   - Apple : app Mac signée Developer ID (bibliothèques client de Kyber et cœur Rust compris), SDK de Kyber et cœur pour l'Apple TV (`KyberSDK-tvOS.zip`) et app Apple TV, cœur et bibliothèques de l'app iPhone/iPad (`KyberSDK-iOS.zip`, les deux apps compilées sans signature pour vérifier), Station Mac (son Kyber si ses correctifs ont changé, app, `.dmg` et paquet de mise à jour, notarisés) ;
   - notarisation de l'app Mac, lancée dès que l'app est signée, agrafage, Gatekeeper ;
   - Flatpak aarch64, puis x86_64 une fois l'image KyberNow Link OS faite (VM « KyberNow Linux ») ;
   - `.deb` du Raspberry Pi puis image KyberNow Link OS (même VM) ;

   un arbre modifié par sa construction (fichier suivi) arrête tout : les binaires ne correspondraient plus au commit ;
4. vérification des fichiers (« Vérifié avant de publier », second tableau) ;
5. flux Sparkle signé, flux cumulatif du site (`site/updates/appcast.xml`) complété par cette version, notes françaises (`site/updates/notes/X.Y.Z/`) et dépôt de Raspberry Pi Imager (`site/updates/rpi-imager.json`, `apps/linux/os/imager-json.sh`), commités par-dessus le commit de version (« Flux des mises à jour : KyberNow X.Y.Z ») ; `SHA256SUMS`, manifeste signé `kybernow-release.json` (`scripts/sign-manifest.sh`, minisign), revérifiés (empreintes, tailles, version, canal, signature avec la clé publique d'Admin) ;
6. release GitHub créée **en brouillon** avec tous les fichiers ;
7. tag `vX.Y.Z` sur le commit de version ; 8. push de `main` (commit de version et commit du flux) et du tag, en une fois (`--atomic`) : le workflow Pages publie le flux ;
9. publication du brouillon ;
10. résumé : durée de chaque étape, refaite ou reprise, total (aussi dans `dist/.journaux-vX.Y.Z/resume.txt`).

Les fichiers sont réunis dans `dist/vX.Y.Z/` (`dist/vX.Y.Z-repetition/` pour une répétition). La version que Windows affiche pour `KyberNow-Station-Setup.exe`, `kybernow-ui.exe` et `kybernow-tray.exe` (Propriétés › Détails, manifestes) suit celle de `Cargo.toml` : leurs `build.rs` produisent les ressources depuis les gabarits `resources/*.rc.in` et `*.manifest.in`.

**Pendant les constructions**, le terminal montre l'état de chaque étape (en attente, en cours avec sa durée, refaite, reprise, échec), mis à jour sur place ; hors d'un terminal (journal, tâche en fond), une ligne par changement et un point toutes les cinq minutes. **Journaux** : chaque étape écrit toute sa sortie dans `dist/.journaux-vX.Y.Z/<étape>.log` (les constructions dans la VM y sont rapatriées, et gardées dans `~/flatpak-<arch>.log`, `~/deb.log`, `~/os-image.log` de la VM). **Un échec** affiche aussitôt la fin de son journal ; les autres voies finissent leur étape en cours puis s'arrêtent (« annulée »), rien n'est publié, et la fin du journal de chaque étape en échec est redonnée à la fin avec le résumé.

**Reprise : empreintes par étape.** Chaque étape a une empreinte de **ses propres entrées** : les objets git, dans le commit construit, des dossiers qu'elle lit (par exemple, pour l'app Windows : `apps/windows`, `apps/core`, `apps/Cargo.toml` et `Cargo.lock`, les correctifs client et de la Station, `server/relay`, `brand`, `scripts/kyber-*.sh`), les versions des outils (Rust stable et nightly, cargo-c, Xcode, xcodegen, MinGW, Node, meson, CMake, Java, NDK, macOS ; Flatpak et Rust de la VM pour Linux), le commit d'origine des sources de Kyber de son arbre, et les dépendances de `third_party/` qu'elle lit (noms, tailles et dates de leurs fichiers : rootfs ARM64 et llvm-mingw pour Windows, chaîne webOS…). Après une réussite, ses fichiers et leur empreinte sont gardés dans `third_party/arbres/.etapes/<étape>/`. À la relance, une étape dont l'empreinte n'a pas changé et dont les fichiers sont intacts est **reprise** sans rien recompiler ; une correction ne refait que les étapes dont elle touche les entrées, et les fichiers publiés viennent tous du commit étiqueté (les entrées sont des fichiers de ce commit). Dans le doute, une entrée est comptée : un changement dans `apps/macos` refait les apps Apple TV et iPhone/iPad, qui en compilent des sources ; un correctif de Kyber refait tout ce qui le pose. Les numéros de version font partie des entrées : une nouvelle version refait tout ; la répétition suivie de la publication de la même version reprend tout sauf les apps Apple (leur numéro de build est la date) et ce qui dépend de la date du jour (fiche AppStream). La Station Mac compte aussi les profils d'approvisionnement d'Apple installés (noms et empreintes) : un nouveau profil, celui du droit des manettes par exemple, la refait. **KyberNow Link OS** dépend de ses fichiers (`apps/linux/os`) et du `.deb` du Raspberry Pi qu'il contient (empreinte de l'étape du `.deb`) : il est refait à chaque nouvelle version (le `.deb` porte la version), repris sinon (relance, répétition puis publication). La notarisation, les flux, les empreintes et la release sont refaits à chaque fois.

**Les entrées en un seul endroit.** Ce qui entre dans l'empreinte de chaque étape est dans `scripts/release-entrees.sh` (chemins du dépôt, sources de Kyber, dépendances de `third_party/`), avec, en plus, la **recette** elle-même (`declare -f construire_<id>` : modifier la construction d'une étape la refait) et la liste de ses fichiers. Le nightly compté est celui que lancent les scripts (`cargo +nightly`), et plus `nightly-<date>`, souvent absent. Kyber pour Mac est recompilé quand `apps/macos/PyroWave` change (il est dans son empreinte, `kyber_station_hash` de `server/kyber-patches/apply.sh`, que `build-kyber.sh` écrit et que `release.sh` compare) : l'encodeur de la Station Mac et le décodeur des apps viennent du même commit. La Station Linux compte les correctifs des apps qu'elle pose (stylet), les licences et les commits des sources de Kyber de la VM et de son chroot x86_64, lus par ssh pendant les vérifications. `scripts/tests/release-outils.sh` relit chaque recette et, de proche en proche, les scripts qu'elle lance et les `project.yml` de xcodegen : un fichier du dépôt qu'ils citent et qui n'est pas dans l'empreinte de l'étape fait échouer l'essai (exceptions motivées dans `scripts/tests/entrees-exceptions.tsv`). Les étapes et leurs libellés sont dans `scripts/release-etapes.txt` (lu aussi par `release-suivi.py`), les fichiers publiés dans `scripts/release-fichiers.txt` (lu par `release.sh`, `sign-manifest.sh` et `release-verifie.py`).

**Une voie qui meurt** (disque plein, `set -e`) sans avoir fini ses étapes : elles passent en échec et les autres voies s'arrêtent, au lieu d'attendre pour toujours une étape qui ne viendra pas. **Ctrl-C** arrête aussi, dans la VM, les commandes lancées depuis l'arbre `linux` avant de rendre les verrous. Dans la VM, le Flatpak x86_64 attend aussi la Station Linux : toutes deux passent par Rosetta. Une erreur dans une fonction de `release.sh` est nommée (« Échec pendant… », `set -E`), et les dossiers temporaires sont retirés à la sortie.

**Durées** (Mac M4 Max, répétitions de la 0.11.1 le 25/11/2026, arbres déjà compilés une fois) : vérifications préalables 7 à 8 s ; tout refaire, caches chauds, environ 10 min (le Flatpak x86_64 sous Rosetta et KyberNow Link OS, à la suite dans la VM, font le chemin le plus long : ~11 min) ; une correction d'un script Linux refait les apps Apple et Linux et reprend le reste : 12 min ; rien de changé : 30 s (tout repris, puis vérifications, flux et manifeste). Pour comparaison, la publication de la 0.11.0, en série : ~11 min hors Linux, ~18 min de plus pour Linux, l'image et la notarisation. Dans la VM, le Flatpak x86_64 attend la fin de pi-gen : lancés ensemble, `rustc --version` sous Rosetta a rendu une sortie vide et la compilation a échoué.

**Relancer après un échec** : corriger, commiter par-dessus, relancer la même commande. Un commit « KyberNow X.Y.Z » en tête (ou juste sous le commit du flux) est réutilisé ; si `HEAD` a déjà ses versions alignées sur X.Y.Z (une correction commitée après le commit de version), c'est `HEAD` qui est construit et étiqueté, sans nouveau commit de version. Un brouillon existant reçoit les nouveaux fichiers (`--clobber`), le tag est posé s'il manque. Seule une version déjà publiée est refusée. Si l'échec est venu après l'étape 5, le commit du flux est en tête et le commit de version juste dessous : il est reconnu, le flux refait ; son commit est remplacé s'il n'a pas encore été poussé, complété par un nouveau sinon.

**Une seule publication à la fois.** La publication (ou la répétition) prend un verrou sur `third_party/arbres/publication` et sur chacun de ses arbres ([build-notes.md](build-notes.md), « Verrous ») ; une autre lancée en même temps, ou une construction à la main dans un de ces arbres, est refusée avec le nom de celle qui le tient.

**Vérifié avant de construire** (une dizaine de secondes ; en répétition, ce qui ne bloque que la publication est une « note ») :

| Vérification | Si elle échoue |
|---|---|
| Branche `main`, arbre propre (`git status --porcelain` vide, fichiers ignorés exceptés, ainsi que les schémas partagés des projets Xcode, qu'Xcode ouvert réécrit dans son format), `origin/main` inclus | commiter, `git switch main`, `git pull` |
| Section `## X.Y.Z` dans `CHANGELOG.md` et `CHANGELOG.en.md` | l'ajouter aux deux |
| Ni tag `vX.Y.Z` ailleurs que sur le commit de version, ni release `vX.Y.Z` déjà publiée ; `gh` connecté | `git tag -d vX.Y.Z` s'il reste d'un essai ; `gh auth login` |
| Aucune autre publication en cours, aucun arbre pris par une autre construction | attendre, ou voir qui le tient (message) |
| Disque du Mac : 40 Go libres au moins (note sous 80) | [build-notes.md](build-notes.md), « Place sur le disque » |
| Notarisation : `xcrun notarytool history` répond (profil « kybernow » lisible, Mac déverrouillé) ; un accord d'Apple expiré répond 403 et est nommé comme tel | accepter l'accord sur developer.apple.com (Account › Agreements) et App Store Connect ; ou `xcrun notarytool store-credentials` (plus bas) |
| `scripts/sign-manifest.sh`, flux cumulatif `site/updates/appcast.xml` | sans manifeste signé, Admin refuse la mise à jour ; `scripts/rebuild-appcast.sh` une fois |
| App Android : clé de publication et outils (`release-apk.sh --check`) ; app webOS : cible Rust `armv7-unknown-linux-gnueabi` | « Clé de signature de l'app Android » ; `rustup target add …` |
| Sources et dépendances partagées : `kyber-desktop`, `kyber-desktop-macos`, `kyber-web`, SDL2 de Kyber 0.29, rootfs ARM64, llvm-mingw | [build-notes.md](build-notes.md) |
| App Windows : **édition de liens** d'un petit programme contre les mêmes bibliothèques que `kybernow.exe` (`apps/windows/scripts/link-check.sh`, x64 contre la libavcodec de la Station de l'arbre, ARM64 contre la sienne) : un symbole manquant est nommé en une seconde | voir le symbole : en 0.11.0, l'encodeur PyroWave de la libavcodec du serveur sans sa bibliothèque d'import |
| Surcouche Kyber et Kyber de la Station Mac de leurs arbres : correctifs actuels (empreintes, toutes deux de `kyber_station_hash` dans `server/kyber-patches/apply.sh`, la liste commune aux trois Stations : celle de Kyber pour Mac compte aussi `apps/macos/PyroWave`, d'où vient son encodeur PyroWave ; celle de la surcouche, calculée par `server/kyber-patches/stamp.sh` pour `build.sh` comme pour la publication, compte aussi les sources des aides micro, caméra et essai de l'encodeur : en 0.11.7, les aides corrigées sont parties dans leur ancienne version ; quand elle change, `build.sh` repart des sources d'origine de l'arbre, `arbre_kyber_origine` ; elle se calcule dans l'arbre, au commit de version, qui change les `Cargo.toml` des aides) | rien : signalé (« note »), la publication les recompile |
| VM « KyberNow Linux » : vue au démarrage « headless » (réglée au besoin, jamais de fenêtre qui prendrait le clavier), joignable (démarrée au besoin), outils, Rosetta dans binfmt, 20 Go libres pour pi-gen, après le nettoyage de son dossier de travail (`build-image.sh --clean`, lancé par `release-image.sh --check` : ce qu'une construction ratée a laissé, sauf le système du stage 2) | [apps/linux/README.md](../apps/linux/README.md) ; [apps/linux/os/README.md](../apps/linux/os/README.md), « Place sur le disque » ; un système de fichiers encore monté : la commande `umount` donnée |
| Copie du Flatpak : `apps/linux/scripts/sources.sh` (sources de Kyber d'origine) contient ce que `build.sh` lit, les correctifs client et webOS s'y posent, l'espace de travail réduit se résout (`cargo metadata`) | le fichier ou le correctif nommé |
| Dépendances de cargo téléchargées : `cargo fetch --locked` dans chaque espace de travail (après une fusion, un `Cargo.lock` nouveau) | réseau, ou `Cargo.lock` à mettre à jour |
| Outils : xcodegen, xcodebuild, npm, cargo, zstd, MinGW, zip, ditto, python3, xmllint, rsync, minisign, meson, cmake, llvm-objdump de llvm-mingw | Homebrew |
| Rust nightly (`rustup run nightly`) présent ; une autre date que celle de `scripts/kyber-toolchain.sh` est une « note » | `rustup toolchain install nightly-<date>` |
| Essais des outils de publication (`scripts/tests/release-outils.sh` : verrous, arbres, correctifs, entrées des étapes, fichiers publiés), quelques secondes | l'essai nommé ; une entrée manquante va dans `scripts/release-entrees.sh` |
| Sources de Kyber de chaque arbre au même commit que le `third_party/` du dépôt principal (copiées une fois : Kyber monté là-bas n'y arrivait jamais) | « note » : `scripts/arbres.sh recopier <variante>` |

**Vérifié avant de publier** (`scripts/release-verifie.py`, puis les outils d'Apple et d'Android) :

| Vérification | Sur quoi |
|---|---|
| Présent et de taille plausible (1 Mo au moins, 200 Mo pour l'image ; quelques kio pour `update.ps1`, flux et manifeste) | chaque fichier publié, anciens noms compris |
| Version X.Y.Z lisible dedans | apps Mac et Station Mac (`Info.plist` dans les archives), `kybernow-update.zip` (`version.txt`), installateurs Windows (ressource de version), `.deb` et `.ipk` (champ `Version`), APK (`versionName`, `aapt2`), `appcast.xml` ; champs de `KyberNow-Link-OS.img.xz.info` |
| Signature Developer ID, ticket de notarisation agrafé, Gatekeeper | app Mac, app de la Station Mac (paquet de mise à jour), `.dmg` de la Station Mac |
| Signature de l'APK par la clé de publication (`apksigner`, certificat de `apps/android/scripts/release-cert.sha256` ; ce fichier absent ou illisible fait échouer la vérification, il n'accepte plus n'importe quelle clé) | `KyberNow-Link-Android.apk` |
| Dépendances DLL (`llvm-objdump -p`) : DLL de Windows ou livrées à côté seulement | `apps/windows/dist/x64` et `arm64` de l'arbre ; surcouche Kyber (avec Kyber 0.29) |
| Sous-système Windows (sans fenêtre de terminal) des aides lancées dans la session : `kybernow-micro.exe`, `kybernow-camera.exe` | surcouche Kyber |
| `SHA256SUMS` juste, manifeste cohérent (chaque fichier : empreinte, taille ; version, canal ; tout fichier publié présent, de `scripts/release-fichiers.txt`, dans les deux), signature minisign valide avec la clé publique d'Admin | `dist/vX.Y.Z/` |

**Canaux.** Une version stable est la « dernière version » de GitHub (`releases/latest`), celle que voient Admin et l'app Mac. `--beta` publie une *prerelease*, jamais marquée « dernière » : son manifeste porte `"channel": "beta"` et son flux Sparkle `<sparkle:channel>beta</sparkle:channel>`. Le numéro reste `X.Y.Z` (Admin refuse tout autre format) : une bêta prend le numéro suivant, et la stable qui la suit un numéro plus haut. L'app Mac lit le flux cumulatif du site, qui réunit les deux canaux : elle ne propose une bêta que si l'utilisateur les a demandées (« Flux des mises à jour de l'app Mac », plus bas).

**Répétition (`--repetition`, ou `--dry-run`).** Tout comme une publication — mêmes vérifications, mêmes constructions en parallèle dans les mêmes arbres, reprise des étapes inchangées, notarisation (sauf `--sans-notarisation`), vérification des fichiers, flux, manifeste signé —, sans commit, release, tag ni push, qu'elle affiche (`[répétition, non lancé] …`). Le commit de version est fait dans l'arbre `versions`, sur aucune branche : l'arbre de travail n'est pas touché. Elle construit `HEAD` (des modifications non commitées n'y sont pas). Ce qui ne bloquerait que la publication (branche, arbre modifié, notes de version, tag) est signalé en « note ». Les fichiers vont dans `dist/vX.Y.Z-repetition/`, avec le flux cumulatif dans `appcast-site.xml` et ce qui changerait dans `site/updates/appcast.xml`. À lancer après toute grosse fusion : ce qui casserait une publication casse ici, sans conséquence, et la publication qui suit reprend ses étapes. `--check` s'arrête après les vérifications et affiche le plan.

**Exigences d'une mise à jour.** `--min-installer A.B.C` (version de KyberNow déjà installée en dessous de laquelle le paquet est refusé, 0.2.0 par défaut) et `--requires-setup` (la mise à jour exige l'installateur) sont passés à `package-update.sh`, qui en garde une copie dans le paquet (`requirements.json`, vérifiée par `update.ps1`), et à `scripts/sign-manifest.sh`, dont le manifeste signé fait foi. Quand s'en servir : « Migrations des réglages », plus bas.

**Ce que l'auteur prépare une fois** : `gh auth login` (jeton et droits : « Réglages GitHub qui protègent les publications » ; un jeton *fine-grained* limité au dépôt demande Contents en lecture et écriture, plus Workflows si `main` pousse un changement de `.github/workflows/`), le profil de notarisation et le certificat Developer ID (« Signature et notarisation de l'app Mac »), la clé Sparkle (plus haut) et la clé minisign (en-tête de `scripts/sign-manifest.sh`).

## Flux des mises à jour de l'app Mac

L'app Mac lit `https://www.kybernow.app/updates/appcast.xml` (`SUFeedURL` dans `apps/macos/project.yml`) : le fichier `site/updates/appcast.xml` du dépôt, publié par le workflow Pages à chaque push sur `main` qui touche `site/`.

- **Toutes les versions publiées**, stables et bêtas, la plus récente en premier. Chaque entrée est celle du `appcast.xml` joint à sa release, recopiée telle quelle par `scripts/merge-appcast.py` : lien de téléchargement vers `releases/download/vX.Y.Z/KyberNow-Link-macOS.zip` (`KyberNow-macOS.zip` jusqu'à la 0.4.2), longueur, signature EdDSA, notes. Rien n'est re-signé : l'app vérifie toujours l'archive avec `SUPublicEDKey`.
- **Canaux.** Les entrées d'une bêta portent `<sparkle:channel>beta</sparkle:channel>`, les stables aucun canal. L'app (délégué Sparkle dans `KyberNowApp.swift`) demande le canal `beta` quand « Recevoir les versions bêta » est coché dans KyberNow Link › Réglages… (`UserDefaults`, clé `receiveBetaUpdates`), aucun sinon. Sparkle propose la plus récente des entrées permises : une stable plus récente qu'une bêta installée est proposée normalement, même case cochée ; décocher la case ne fait pas revenir à la stable précédente, l'app attend la stable suivante.
- **Apps Mac en 0.2.6 et avant.** Elles lisent `releases/latest/download/appcast.xml`, que GitHub résout vers la dernière stable. Chaque release garde donc son `appcast.xml` : ces apps y trouvent la dernière stable, qui lit ensuite le flux du site.
- **Délai.** GitHub Pages met en cache environ 10 minutes, et le flux arrive en ligne une minute ou deux après le push, alors que la release est publiée quelques secondes après : l'app ne voit en pratique jamais une entrée dont l'archive est encore en brouillon (sauf échec entre le push et la publication : relancer la même commande publie le brouillon).
- **Reconstruire le flux** à partir des releases existantes (amorçage, ou flux abîmé) : `scripts/rebuild-appcast.sh` liste les releases publiées (`gh release list`, brouillons exclus), reprend l'`appcast.xml` de chacune, marque `beta` celles qui sont des prereleases, et repasse par `generate_appcast` (clé Sparkle du Trousseau) une release qui aurait l'app Mac sans flux. Il écrit `site/updates/appcast.xml` (ou `-o fichier`) et le vérifie avec `xmllint` ; il ne commite rien. Commiter le fichier sur `main` le publie.

```bash
scripts/rebuild-appcast.sh
xmllint --noout site/updates/appcast.xml
git add site/updates/appcast.xml && git commit -m "Flux des mises à jour : reconstruit"
```

## Intégration continue

`.github/workflows/ci.yml` vérifie chaque push et chaque pull request vers `main` (onglet Actions du dépôt) :

| Job | Machine | Ce qu'il vérifie |
|---|---|---|
| Admin, icône, installateur | macOS | `cargo test` d'Admin et de l'installateur de la Station ; `cargo check --target x86_64-pc-windows-gnu` d'Admin, de l'installateur (`--features ui`) et de l'icône, ressources Windows comprises (MinGW par Homebrew) |
| Interfaces Svelte | Linux | `npm ci`, `npm run build`, `npm run check` dans `server/web` et `server/installer/ui` |
| Scripts et update.ps1 | Linux | `shellcheck` de tous les scripts, `update.ps1` en ASCII, banc de `update.ps1` (`server/installer/tests/update-bench.ps1`, services simulés) |
| App iPhone/iPad (règles) | macOS | chaque `apps/ios/tests/*/run.sh` (manette à l'écran et toucher, règles de session ; un essai pas encore là est sauté), sans simulateur |
| App Apple TV | macOS | `xcodegen`, puis `xcodebuild` pour le simulateur, sans signature, avec le `KyberSDK-tvOS.zip` de la dernière release |

`.github/workflows/outillage.yml`, aux mêmes moments :

| Job | Machine | Ce qu'il vérifie |
|---|---|---|
| Outils de publication | macOS | `scripts/tests/release-outils.sh` (aussi lancé par les vérifications préalables de `release.sh`) |
| Relais | Linux | `cargo test --locked --workspace` de `server/relay` et de son tunnel |
| Banc de mesure | Linux | `cargo build --locked` de `tools/banc`, qui recompile `apps/core/src/adaptive.rs` |

`.github/workflows/relay-image.yml` repasse les essais du relais avant de construire l'image Docker : une régression ne part pas vers les relais, qui se mettent à jour d'eux-mêmes chaque nuit. Sa version (lancement à la main) passe par l'environnement du script, jamais dans son texte.

Ce qu'elle ne couvre pas :
- **L'app Mac.** Elle lie le SDK client de Kyber et VLC (`apps/macos/Vendor`, 88 Mo, compilés depuis les sources de Kyber par `build-kyclient.sh`), publiés nulle part.
- **L'app Apple TV quand les correctifs client ou le cœur Rust ont changé** depuis la dernière release (`apps/macos/kyber-patches`, `apps/webos/kyber-patches`, les correctifs de la Station que les apps posent aussi, `*-kyclient-*`, `*-kynput-*`, `*-kynet-*`, `*-kycontroller-*`, `*-kywebplayer-*`, `scripts/kyber-client-patches.sh`, `kyber-toolchain.sh`, `build-kyber-tvos.sh`, `apps/core`, `apps/apple`, `apps/Cargo.*`) : le SDK publié ne correspond plus au code, la compilation est sautée avec un avertissement jusqu'à la release suivante.
- **Les apps KyberNow Link autres que l'Apple TV** (Windows, Android, webOS, Linux, iPhone et iPad) : ni compilées ni testées en CI, elles demandent les sources de Kyber (`third_party/`) ; seules les règles de l'app iPhone/iPad sans interface le sont (`apps/ios/tests`). Le cœur Rust a son propre workflow (`coeur.yml`).
- **Les binaires Windows liés et Kyber** : la CI vérifie le code, elle ne produit ni `KyberNow-Station-Setup.exe` ni la surcouche (correctifs de Kyber, `third_party/` absent), et ne lance rien sous Windows.
- **Rien n'est signé ni publié** : la publication reste `scripts/release.sh`, sur le Mac de l'auteur (notarisation, clés Sparkle et minisign).

## Signature et notarisation de l'app Mac

La version diffusée (configuration Release) est signée « Developer ID Application: Mathieu Requenna (P6DB682S4K) », avec le runtime renforcé et un horodatage sécurisé, sans autorisation de débogage. Autorisations (`apps/macos/KyberNow/KyberNow.entitlements`) : la caméra (QR code d'appairage), et le magasin clé-valeur iCloud et le groupe du Trousseau partagés avec l'app Apple TV (ordinateurs publiés pour l'Apple TV), qui exigent un profil de provisionnement. Les compilations de développement (Debug) gardent le certificat de développement, plus rapide.

- `apps/macos/scripts/embed-kyber.sh` signe les bibliothèques de Kyber et de VLC selon la configuration.
- L'équipe et le certificat sont écrits une fois, dans `scripts/identite.sh` (`KYBER_SIGN_IDENTITY` pour un autre certificat), lu par les scripts de signature de l'app et de la Station Mac.
- `apps/macos/scripts/sign-release.sh`, après `xcodebuild`, re-signe les outils de Sparkle, du plus profond au framework, puis l'app. Xcode copie Sparkle après les phases du projet, avec la signature de Sparkle, que la notarisation refuse.
- `scripts/release.sh` envoie l'app à Apple (`notarytool`, profil `kybernow`), agrafe le ticket (`stapler`) et vérifie avec Gatekeeper avant de publier.

Mise en place, une fois par Mac qui publie :
1. Certificat : Xcode › Réglages › Comptes › équipe P6DB682S4K › Gérer les certificats › + › Developer ID Application.
2. Mot de passe pour app sur https://account.apple.com (Connexion et sécurité › Mots de passe pour app), enregistré pour `notarytool` (à saisir vous-même) :

```bash
xcrun notarytool store-credentials kybernow --apple-id <votre identifiant Apple> --team-id P6DB682S4K
```

Le mot de passe enregistré n'est lisible que Mac déverrouillé : `release.sh` le vérifie avant de compiler et garde l'écran allumé jusqu'à la fin, pour qu'il ne se verrouille pas en route.

La première notarisation d'un compte peut prendre plusieurs heures (vérifications d'Apple) ; ensuite, quelques minutes. Pour retrouver un envoi : `xcrun notarytool history --keychain-profile kybernow`.

## Migrations des réglages

Les réglages d'Admin (`C:\ProgramData\KyberNow\kyber-admin.toml`) portent la version de leur format, `schema` (absent = 0 : fichier écrit par Admin en 0.2.5 ou avant). À chaque démarrage, et avant `init` ou `reset-password`, Admin amène le fichier au format qu'il connaît (`server/admin/src/migrate.rs`) :

- une migration par version (de N à N+1), dans l'ordre, chacune idempotente : la rejouer ne change plus rien ;
- avant d'écrire, l'original est copié dans `backups\kyber-admin.toml.schema-<N>-<horodatage>` ; chaque changement est écrit dans le journal (`kyber-admin.log`) ; les commentaires du fichier sont gardés ;
- un fichier d'une version **plus récente** d'Admin (après un retour arrière de la mise à jour) est laissé tel quel, et les réglages que cette version ne connaît pas sont gardés quand elle réécrit le fichier ;
- une migration impossible (fichier illisible) n'empêche pas Admin de démarrer : le fichier est alors mis de côté (`kyber-admin.toml.invalide-…`).

À la main : `kyber-admin.exe migrate --dry-run` affiche ce qui serait changé, `kyber-admin.exe migrate` le fait.

| Schéma | Depuis | Changement |
|---|---|---|
| 1 | 0.2.6 | `listen` devient une adresse `IP:port` valide : port 9443 ajouté à une adresse seule (`"0.0.0.0"`, qui empêchait Admin de démarrer), `"0.0.0.0:<port>"` pour un port seul (`listen = 9443`, qui rendait tout le fichier illisible : il était mis de côté, mot de passe compris), espaces retirés ; une valeur inutilisable est retirée (adresse par défaut). |

Ajouter une migration : une fonction `to_N` dans `migrate.rs`, ajoutée à `MIGRATIONS`, `CURRENT` passé à N, un test sur un fichier de la version N-1, et une ligne dans ce tableau.

**Mise à jour qui exige l'installateur.** Certaines versions ne peuvent pas s'installer par le paquet de mise à jour : nouveau pilote (écran virtuel, manettes), service à recréer, droits à reposer. Elles se publient avec `scripts/release.sh X.Y.Z --requires-setup` : le manifeste signé porte `"requires_setup": true` et Admin refuse alors « Installer » en renvoyant vers `KyberNow-Station-Setup.exe` (`KyberNow-Server-Setup.exe` pour Admin en 0.3.1 et avant, d'où sa copie). De même, `--min-installer A.B.C` refuse le paquet sur une Station installée avant la version A.B.C (`min_installer_version`). Le paquet en garde une copie (`requirements.json`), que `update.ps1`, sans manifeste, vérifie de la même façon ; pour Admin, le manifeste signé fait foi.

## Signature de l'image du relais

L'image Docker du relais (`ghcr.io/monsieurnours/kybernow-relay`) est construite et poussée par le workflow `relay-image.yml`, sur GitHub, à chaque version publiée. Il la signe avec [cosign](https://docs.sigstore.dev/cosign/) par son empreinte (toutes ses étiquettes avec) quand les secrets `COSIGN_PRIVATE_KEY` et `COSIGN_PASSWORD` sont posés sur le dépôt ; sans eux, l'image part non signée et le workflow le note, comme les vérifications préalables de `release.sh` (« note », jamais un échec). Un relais dont le `.env` nomme une clé publique (`RELAY_COSIGN_KEY`) ne prend une nouvelle image que si sa signature s'y vérifie (`server/relay/update.sh`) ; sinon il garde la précédente.

**Créer la clé, une fois, sur le Mac** (hors ligne ; jamais dans le dépôt) :

```bash
mkdir -p ~/Library/Application\ Support/KyberNow && cd ~/Library/Application\ Support/KyberNow
cosign generate-key-pair          # demande un mot de passe ; écrit cosign.key et cosign.pub
```

Puis, sur le dépôt, les deux secrets que lit le workflow (`gh` les lit sur l'entrée standard, rien ne passe en argument) :

```bash
gh secret set COSIGN_PRIVATE_KEY -R MonsieurNours/kybernow < ~/Library/Application\ Support/KyberNow/cosign.key
gh secret set COSIGN_PASSWORD -R MonsieurNours/kybernow    # tapez le mot de passe de la clé
```

Sauvegardez `cosign.key` et son mot de passe avec les autres clés (« Sauvegarde des clés de signature », plus bas) : sans eux, il faut une nouvelle paire, et poser la nouvelle clé publique sur chaque relais.

**Sur la machine du relais** (à faire par l'auteur) : copier `cosign.pub` dans le dossier du relais (celui de `docker-compose.yml`), puis ajouter `RELAY_COSIGN_KEY=cosign.pub` à son `.env` et relancer `docker compose up -d`. La vérification passe par l'image officielle de cosign (`RELAY_COSIGN_IMAGE` pour en changer). Les images publiées avant la première signature ne sont pas signées : poser la clé seulement après une version signée.

## Clés, compte GitHub et signature Windows

### Sauvegarde des clés de signature

Trois clés privées ne doivent jamais se perdre : une nouvelle clé ferait refuser toutes les mises à jour par ce qui est déjà installé, qu'il faudrait réinstaller à la main.

| Clé | Où elle vit | Clé publique attendue |
|---|---|---|
| Sparkle (app Mac, EdDSA) | Trousseau de session, élément créé par `generate_keys` | `SUPublicEDKey` dans `apps/macos/project.yml` |
| minisign (manifeste de chaque version, vérifié par Admin et par le cœur Rust des apps) | `~/Library/Application Support/KyberNow/minisign.key`, chiffrée par le mot de passe de l'élément `kybernow-minisign` du Trousseau | `server/admin/src/update/kybernow-release.pub` (compilée dans Admin et dans `apps/core/src/update.rs`) |
| Android (APK publié) | `~/Library/Application Support/KyberNow/android-release.jks`, mot de passe dans l'élément `kybernow-android-keystore` du Trousseau | empreinte dans `apps/android/scripts/release-cert.sha256` (voir « Clé de signature de l'app Android ») |

La sauvegarde des clés Sparkle et minisign (faite le 2026-10-21) est une image disque chiffrée en AES-256, `kybernow-cles.dmg`, volume `KyberNow-Cles`. Elle contient :
- `sparkle-kybernow.key`, la clé Sparkle exportée par `generate_keys -x`, en clair à l'intérieur de l'image ;
- `kybernow-minisign.key` et son mot de passe `minisign-motdepasse.txt`, tiré du Trousseau sans passer par l'écran ;
- `kybernow-minisign.pub` ;
- un `LISEZMOI.txt` avec les commandes de remise en place.

Le mot de passe de l'image est noté sur papier, rangé à part. Les codes de secours de la 2FA GitHub peuvent aussi aller dans l'image. L'image doit vivre **hors du Mac qui publie** (disque externe, NAS, idéalement deux lieux). Ouvrez-la une fois par an pour vérifier qu'elle se lit.

Créer ou rouvrir l'image et y écrire, sans rien afficher :

```bash
hdiutil create -size 20m -fs APFS -encryption AES-256 -volname KyberNow-Cles ~/kybernow-cles.dmg
hdiutil attach ~/kybernow-cles.dmg
GK=apps/macos/build/SourcePackages/artifacts/sparkle/Sparkle/bin/generate_keys
$GK -x /Volumes/KyberNow-Cles/sparkle-kybernow.key
cp ~/Library/Application\ Support/KyberNow/minisign.key /Volumes/KyberNow-Cles/kybernow-minisign.key
cp ~/Library/Application\ Support/KyberNow/minisign.pub /Volumes/KyberNow-Cles/kybernow-minisign.pub
security find-generic-password -s kybernow-minisign -w > /Volumes/KyberNow-Cles/minisign-motdepasse.txt
hdiutil detach /Volumes/KyberNow-Cles
```

Vérifier une sauvegarde sans révéler la clé (image ouverte). La première commande recalcule la clé publique Sparkle, qui doit être égale à `SUPublicEDKey`. La seconde signe un fichier d'essai avec la clé et le mot de passe sauvegardés, puis vérifie la signature avec la clé publique du dépôt :

```bash
{ printf '\x30\x2e\x02\x01\x00\x30\x05\x06\x03\x2b\x65\x70\x04\x22\x04\x20'; base64 -d -i /Volumes/KyberNow-Cles/sparkle-kybernow.key; } \
  | openssl pkey -inform DER -pubout -outform DER | tail -c 32 | base64
echo essai > /tmp/essai.txt
tr -d '\n' < /Volumes/KyberNow-Cles/minisign-motdepasse.txt | { read -r PW; printf '%s\n' "$PW"; } \
  | minisign -S -s /Volumes/KyberNow-Cles/kybernow-minisign.key -m /tmp/essai.txt -x /tmp/essai.txt.minisig >/dev/null 2>&1 \
  && minisign -V -q -p server/admin/src/update/kybernow-release.pub -m /tmp/essai.txt -x /tmp/essai.txt.minisig && echo minisign OK
rm -f /tmp/essai.txt /tmp/essai.txt.minisig
```

Remettre les clés sur un autre Mac (image ouverte) :

```bash
$GK -f /Volumes/KyberNow-Cles/sparkle-kybernow.key && $GK -p      # doit afficher SUPublicEDKey
mkdir -p ~/Library/Application\ Support/KyberNow
cp /Volumes/KyberNow-Cles/kybernow-minisign.key ~/Library/Application\ Support/KyberNow/minisign.key
cp /Volumes/KyberNow-Cles/kybernow-minisign.pub ~/Library/Application\ Support/KyberNow/minisign.pub
chmod 600 ~/Library/Application\ Support/KyberNow/minisign.key
printf 'add-generic-password -a kybernow -s kybernow-minisign -l kybernow-minisign -w %s\n' \
  "$(cat /Volumes/KyberNow-Cles/minisign-motdepasse.txt)" | security -i >/dev/null
```

`printf` est interne au shell : le mot de passe ne passe pas dans la liste des processus.

### Réglages GitHub qui protègent les publications

État au 2026-10-21, vérifié avec `gh api` :
- **2FA** active sur le compte `MonsieurNours`, seul collaborateur du dépôt.
- **Releases immuables** (Settings › General › Releases). Une release publiée ne peut plus changer de fichiers ni de tag. `release.sh` crée un brouillon, y dépose tout, puis publie, ce qui reste compatible. Une erreur dans une version publiée se corrige par une nouvelle version : le numéro d'une release supprimée ne peut pas resservir.
- **Règles (Settings › Rules)** :
  - `tags-v-creation` : seul le rôle admin peut créer un tag `v*`. Le `GITHUB_TOKEN` des Actions ne le peut pas.
  - `tags-v-figes` : aucun tag `v*` ne peut être déplacé ni supprimé, sans exception. Si une publication échoue après le push du tag, publiez la version suivante, ou désactivez la règle le temps du nettoyage.
  - `main-legere` : `main` ne peut être ni supprimée ni réécrite (force push). Les pushes directs restent permis.
- **`gh` sur le Mac de publication** : jeton OAuth de `gh` (droits larges, Mac de développement). `release.sh` s'en sert pour les releases ; les `git push` passent par SSH. Un jeton à granularité fine limité au dépôt aurait besoin au minimum de *Contents: Read and write* : GitHub n'a pas de permission plus étroite pour les releases, et ce jeton permettrait donc aussi de pousser. Le droit `admin:public_key` (ajouter une clé SSH au compte) ne sert pas à la publication : `gh auth refresh -h github.com --remove-scopes admin:public_key` le retire.
- **CI** : les workflows utilisent le `GITHUB_TOKEN` fourni à chaque exécution, réduit à `contents: read` (plus `pages: write` et `id-token: write` pour le workflow du site, qu'exige le déploiement sur Pages). Le réglage par défaut du dépôt est aussi la lecture seule. Aucun secret à créer.

Relire les règles :

```bash
gh api repos/MonsieurNours/kybernow/rulesets --jq '.[].name'
gh api repos/MonsieurNours/kybernow/immutable-releases
```

### Signature Windows (Authenticode)

`KyberNow-Station-Setup.exe`, `KyberNow-Link-Setup.exe`, `kyber-admin.exe`, `kybernow-tray.exe` et `update.ps1` ne sont pas signés Authenticode : Windows affiche « Éditeur inconnu » (SmartScreen, UAC). Authenticode ne remplacerait pas minisign : il dit qui a signé ; minisign prouve que c'est la clé de KyberNow, gravée dans Admin, ce qui protège aussi contre un compte GitHub compromis.

Le pilote d'écran virtuel (SudoVDA) est autosigné : l'installateur de la Station ajoute son certificat, `sudovda.cer` (usage « Code Signing »), aux magasins Root et TrustedPublisher (`server/installer/src/install.rs`), et le désinstalleur l'en retire.

**Piège, pour toute signature Authenticode** : elle change les empreintes des fichiers signés. `SHA256SUMS` et le manifeste minisign se calculent après elle, et les copies sous les anciens noms se font depuis les fichiers signés.
