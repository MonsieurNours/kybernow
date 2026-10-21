# Versions et mises à jour

[English](releases.en.md) · Français

Les versions de KyberNow sont publiées sur GitHub : https://github.com/MonsieurNours/kybernow/releases. Le dépôt est public : les mises à jour se téléchargent sans jeton, et le code reste disponible comme l'exige l'AGPL (avec les correctifs de Kyber, voir [kyber-changes.md](kyber-changes.md)).

Un seul numéro de version pour tout KyberNow : la console, l'icône, l'installateur, l'app Mac et l'app Apple TV changent ensemble.

## Ce que contient une version

| Fichier | Pour quoi |
|---|---|
| `KyberNow-Server-Setup.exe` | KyberNow Server : première installation sur l'ordinateur Windows à partager (ce nom le distingue de l'installateur de l'app, `KyberNow-Client-Setup.exe`). Une copie sous l'ancien nom, `KyberNow-Setup.exe`, est encore publiée quelques versions pour les liens existants ; le site ne doit être mis en ligne avec les nouveaux liens qu'une fois la version publiée |
| `kybernow-update.zip` | Mise à jour d'un ordinateur déjà installé : surcouche Kyber, console, icône, `update.ps1` |
| `update.ps1` | Mise à jour à la main, sans la console |
| `KyberNow-macOS.zip` | L'app Mac (première installation : la glisser dans Applications) |
| `appcast.xml` | Flux des mises à jour de l'app Mac (Sparkle), signé EdDSA |
| `KyberNow-Client-Setup.exe` | L'app Windows, x64 et ARM64 dans un seul fichier : installateur par utilisateur (première installation et mises à jour de l'app, voir [apps/windows/README.md](../apps/windows/README.md)) |
| `KyberSDK-tvOS.zip` | SDK de Kyber compilé pour l'Apple TV, que télécharge `apps/tvos/scripts/sideload.sh` pour installer l'app sans l'App Store ([SIDELOAD.md](../apps/tvos/SIDELOAD.md)) |
| `kybernow-release.json` | Manifeste de la version : numéro, canal (`stable` ou `beta`), date, version minimale déjà installée pour une mise à jour depuis la console, réinstallation nécessaire ou non, empreinte SHA-256 et taille de chaque fichier ci-dessus |
| `kybernow-release.json.minisig` | Signature du manifeste (minisign, ed25519), vérifiée par la console avant toute installation |
| `SHA256SUMS` | Empreintes de tous les fichiers |

## Mettre à jour

**L'ordinateur, en un clic.**
- La console vérifie la dernière version toutes les 6 heures, sur son **canal** : `stable` (par défaut, versions publiées) ou `beta` (aussi les préversions GitHub), à choisir sur la page Mises à jour (réglage `update_channel` de `kyber-admin.toml`). La page **Mises à jour** l'affiche avec ses notes et un bouton **Installer** ; l'icône de la zone de notification propose « Installer la version X… », qui ouvre cette page.
- **Rien ne s'installe sans signature.** Pour une version plus récente, la console télécharge `kybernow-release.json` et `kybernow-release.json.minisig` et vérifie la signature avec la clé publique compilée en elle (voir « Clé de signature des mises à jour »). Puis : le manifeste annonce le même numéro que la version GitHub (`tag_name`), un canal que suit l'ordinateur (une version `beta` est refusée en `stable`), une `min_installer_version` inférieure ou égale à la version installée et pas de `requires_setup` (sinon « réinstallez avec KyberNow-Server-Setup.exe »). Tout échec (manifeste absent, signature invalide ou d'une autre clé, canal, version) est affiché sur la page et dans `GET /api/update` (`verify_error`), et le bouton **Installer** n'apparaît pas ; l'icône ne propose une version que si sa signature est bonne.
- L'installation retélécharge et revérifie le manifeste, puis télécharge `kybernow-update.zip` (jamais plus que la taille du manifeste) et vérifie sa taille et son empreinte SHA-256 contre lui. Le numéro de la version publiée doit être de la forme `X.Y.Z` : il sert de nom de dossier, tout autre numéro est refusé. Manifeste, signature et paquet restent dans `C:\ProgramData\KyberNow\update\<version>\` (réservé à SYSTEM et aux Administrateurs).
- La console lance ensuite **l'applicateur**, `kyber-admin update apply <paquet> --version X`, en SYSTEM et détaché (il survit à l'arrêt de la console). C'est une copie de la console, `C:\Program Files\KyberNow\.staging\kyber-admin-maj.exe` : elle doit pouvoir renommer le dossier `admin` pendant qu'elle tourne. Rien n'est exécuté depuis un dossier qu'un autre compte pourrait modifier, et aucun script n'est lancé : PowerShell n'intervient plus.
- L'applicateur revérifie le manifeste signé et le paquet (rien ne vient de la console qui l'a lancé), puis extrait le paquet dans `C:\Program Files\KyberNow\.staging\<version>\paquet` (droits hérités de Program Files : lecture seule pour les utilisateurs). Il refuse tout chemin qui sortirait du dossier (`..`, chemin absolu, lecteur, flux NTFS), tout nom qui ne serait pas un fichier ordinaire sous Windows (`CON`, `NUL`, `COM1`…, nom finissant par un point ou une espace, `<>"|?*`) et tout lien. Il refuse aussi une version qui n'est pas plus récente que celle installée (le retour en arrière a son propre chemin). Le paquet doit contenir `kyber`, `admin`, `tray` et un `version.txt` égal à la version attendue. Un échec à ce stade ne touche à rien : la console tourne toujours et l'affiche.
- Il arrête l'icône, puis le serveur (`kyber` et ses processus), puis la console (`kyber-admin` et ses agents), puis encore le serveur si la console l'avait relancé entre-temps. La relance automatique de `kyber` après une panne (posée par l'installateur) est suspendue jusqu'au redémarrage, pour que Windows ne le relance pas au milieu des renommages. Il prépare chaque nouveau dossier dans `.staging\<version>\` (copie du dossier actuel, réglages et certificat de Kyber compris, puis les fichiers du paquet par-dessus), puis bascule par renommage : `kyber` devient `kyber.prev` et `.staging\<version>\kyber` devient `kyber`, de même pour `admin`, `tray` et `webclient` (s'il est dans le paquet ou déjà installé : chaque dossier présent a ainsi son `.prev`). Les journaux de Kyber (`kyber\log`) sont déplacés, pas copiés. Il remet ensuite les droits des services (`sc sdset`) et la règle de pare-feu Bonjour.
- Il redémarre les services, puis contrôle la santé de la nouvelle version : en 60 s, la console doit répondre à l'adresse de son réglage `listen` (127.0.0.1 si elle écoute partout, sans proxy) et `GET /api/session` donner la nouvelle version, et le serveur tourner. Sinon, il arrête tout (l'icône comprise, que la nouvelle console a pu relancer), remet chaque dossier où il était (`kyber.prev` redevient `kyber`…), redémarre l'ancienne version et écrit la raison dans le journal et dans `C:\ProgramData\KyberNow\update\resultat.json`. La console relit ce fichier à son démarrage : la page Mises à jour affiche l'échec, et `GET /api/update` le donne dans `install_error` et `last_failure` (version, raison, retour arrière, journal). Si un dossier résiste au retour arrière, l'applicateur remet les autres, garde `.staging\<version>`, l'écrit (`undo_failed`) et la page le dit, même si c'est la version en échec qui tourne.
- Quoi qu'il arrive après l'arrêt des services (erreur, renommage impossible parce qu'un fichier est ouvert), l'applicateur les redémarre en sortant : l'ordinateur n'est jamais laissé sans serveur ni console.
- Réussie, la mise à jour inscrit la nouvelle version dans `C:\Program Files\KyberNow\kybernow.json` et dans « Applications installées » (`DisplayVersion`), et `resultat.json` dit `"ok": true`. La version précédente reste dans les dossiers `.prev` jusqu'à la mise à jour suivante (environ 300 Mo). Dans `update\`, seuls les dossiers des deux dernières versions sont gardés ; `.staging\<version>` est effacé.
- La console relance l'icône dans la session de l'utilisateur, avec son compte (fichier `update\relaunch-tray`). La page se met à jour seule.
- **Revenir à la version précédente** : tant que les dossiers `.prev` existent, la page Mises à jour propose ce bouton (`POST /api/update/rollback`, version précédente dans `rollback` de `GET /api/update`). Le même applicateur fait la bascule en sens inverse (`kyber-admin update rollback`) : `kyber.prev` redevient `kyber` et la version retirée part en `kyber.bad`, de même pour `admin`, `tray` et `webclient` ; un dossier apporté par la version retirée, sans `.prev` (`webclient`), part aussi ; les réglages de Kyber changés depuis la mise à jour (`kyber_config.toml`) et ses journaux sont gardés. Même contrôle de santé, avec la version précédente ; s'il échoue, la version qui tournait est remise. Réussi, le retour efface les dossiers `.bad` : il ne reste plus de `.prev`, et la version retirée est de nouveau proposée à l'installation.
- **Pendant une session.** Si un appareil est en session, `POST /api/update/install` (et `POST /api/update/rollback`) répond 409 avec « Un appareil est connecté (…) : l'installation coupera sa session » ; la page le demande avant de continuer, et renvoie alors `{"force": true}`.
- Durée : environ une minute. Une session de streaming en cours est coupée. Journal : `C:\ProgramData\KyberNow\update\<version>\update.log` ; sortie de l'applicateur dans `lancement.log`, à lire si `update.log` n'existe pas.
- La console suit l'applicateur. S'il s'arrête alors que la console tourne encore (vérification ou extraction refusée), elle l'indique : page Mises à jour, fin du journal, et « Installer » redevient possible.
- Les haut-parleurs virtuels de Steam et le raccourci du menu Démarrer, posés par l'installateur, ne sont plus refaits à chaque mise à jour ; `update.ps1` (à la main) le fait toujours.
- **Passage à la 0.2.6.** Une console 0.2.5 ou plus ancienne ne connaît ni le manifeste ni l'applicateur : elle met à jour vers la 0.2.6 comme avant, avec `SHA256SUMS` et le `update.ps1` du paquet. Les versions publiées gardent donc `SHA256SUMS` et `update.ps1`. À partir de la 0.2.6, la console n'installe plus qu'une version signée.

**L'ordinateur, à la main** (PowerShell administrateur) :

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

Le script télécharge `kybernow-update.zip` et `SHA256SUMS`, vérifie l'empreinte du paquet, puis fait la même bascule avec contrôle de santé. En cas d'échec, il le dit en rouge (« Mise a jour non installee… ») après avoir tout redémarré.

**Chemin manuel : intégrité seulement, jusqu'à la signature Authenticode.** `update.ps1` vérifie que le paquet n'est pas abîmé (`SHA256SUMS`), pas qu'il vient de KyberNow : `SHA256SUMS` est publié au même endroit que le paquet, et vérifier le manifeste minisign demanderait un outil téléchargé lui aussi (`kyber-admin.exe` de la version), ce qui tourne en rond. Le script lui-même n'est pas signé. Il n'apporte donc pas les garanties de la console : à réserver au dépannage, sur une version dont vous connaissez la page GitHub. La signature Authenticode des `.exe` et de `update.ps1` est envisagée pour combler ce manque.

Pour essayer un paquet servi depuis le Mac avant de le publier : `$env:KYBERNOW_UPDATE_BASE = 'http://<adresse-du-Mac>:8000'`, puis la même commande, avec `python3 -m http.server 8000` lancé dans `server/installer/dist` (`package-update.sh` y écrit aussi `SHA256SUMS`).

**Tester la vérification et l'applicateur sans Windows** : `cd server/admin && cargo test`. Les tests couvrent le manifeste (lecture du format publié, comparaison des versions, canal, `min_installer_version`, `requires_setup`), la signature (clé d'essai générée dans le test : signature valide, manifeste modifié d'un octet, autre clé, commentaire de confiance qui ne correspond pas), un manifeste réellement produit par `scripts/sign-manifest.sh` et le binaire `minisign` (`server/admin/src/update/testdata/`, clé d'essai jetable dont seule la clé publique est gardée), l'extraction (`..`, chemin absolu, lecteur, liens), la bascule sur un faux dossier d'installation (réussite, contrôle de santé raté puis retour arrière, paquet falsifié qui ne touche à rien, retour à la version précédente et son échec) et le refus pendant une session. Les services, processus et droits de Windows passent par un trait simulé : ils ne sont vérifiés que sur le PC. Pour régénérer `testdata/` : `KYBERNOW_MINISIGN_KEY=<dossier temporaire>/essai.key KYBERNOW_MINISIGN_ITEM=kybernow-minisign-essai scripts/sign-manifest.sh <dossier> 0.2.7 beta --min-installer 0.2.6` sur un `kybernow-update.zip` de 15 octets, puis supprimer la clé privée et l'élément du Trousseau.

**Tester `update.ps1` sans Windows** : `pwsh -NoProfile -File server/installer/tests/update-bench.ps1` (`brew install powershell`). Le banc simule les services et la console, et rejoue une mise à jour réussie, une console muette (retour arrière), un paquet incomplet et un renommage bloqué.

**L'app Windows.** Elle cherche au lancement puis toutes les 6 heures la dernière version de son canal (réglages de l'app : stables, ou aussi les bêtas), vérifie le manifeste signé comme la console et l'installateur de son architecture contre lui (taille, SHA-256), puis le lance en silence (`/S /UPDATE`) : il attend la fermeture de l'app, remplace ses fichiers et la relance. Une version sans installateur Windows n'est pas proposée.

**L'app Mac.**
- Sparkle vérifie chaque jour, et à la demande (menu KyberNow › Rechercher des mises à jour…). L'app télécharge l'archive, vérifie sa signature EdDSA avec la clé publique de l'app (`SUPublicEDKey`), la remplace et se relance.
- L'app lit le flux cumulatif du site, `https://www.kybernow.app/updates/appcast.xml`, qui liste toutes les versions. Elle ne propose que les stables, sauf si « Recevoir les versions bêta » est coché dans KyberNow › Réglages… : voir « Flux des mises à jour de l'app Mac ».
- La clé privée est dans le Trousseau de ce Mac (élément créé par `generate_keys` de Sparkle). Sauvegardez-la, sans jamais la mettre dans le dépôt :

```bash
apps/macos/build/SourcePackages/artifacts/sparkle/Sparkle/bin/generate_keys -x ~/sparkle-kybernow.key
```

Sans elle, aucune mise à jour de l'app ne pourrait plus être signée. Pour la remettre sur un autre Mac : `generate_keys -f ~/sparkle-kybernow.key`.

## Clé de signature des mises à jour (minisign)

Chaque version publie un manifeste, `kybernow-release.json`, signé avec une clé ed25519 (minisign) :

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

`files` liste ceux des fichiers publiés qui sont présents (`KyberNow-Server-Setup.exe`, `kybernow-update.zip`, `update.ps1`, `KyberNow-macOS.zip`, `appcast.xml`, `KyberSDK-tvOS.zip`). La signature détachée, `kybernow-release.json.minisig`, porte le commentaire de confiance `kybernow <version> <canal>`. La clé publique est dans le dépôt, `server/admin/src/update/kybernow-release.pub` (identifiant `D63B24FFFC7E79D1`), et compilée dans la console : une empreinte SHA-256 seule protège d'un téléchargement abîmé, pas d'une version falsifiée par qui aurait la main sur le compte GitHub ; la signature, si.

Le manifeste s'écrit et se signe avec :

```bash
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable            # ou beta
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable --min-installer 0.2.6 --requires-setup
```

`--min-installer X.Y.Z` : version la plus ancienne depuis laquelle la console peut installer ce paquet (0.2.0 par défaut). `--requires-setup` : cette version ne s'installe qu'avec `KyberNow-Server-Setup.exe` (nouveau pilote, nouveau service…). Le script installe `minisign` avec Homebrew s'il manque, puis vérifie la signature contre la clé publique du dépôt : une clé privée qui ne lui correspond pas est vue avant la publication.

**La clé privée** est dans `~/Library/Application Support/KyberNow/minisign.key`, chiffrée par un mot de passe aléatoire gardé dans le Trousseau (élément `kybernow-minisign`). Le script la crée si elle manque et écrit la clé publique dans le dépôt ; si une clé publique est déjà dans le dépôt, il refuse d'en créer une autre : une nouvelle clé ferait refuser toutes les mises à jour par les consoles déjà installées, qu'il faudrait réinstaller avec `KyberNow-Server-Setup.exe`.

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

## Publier une version

0. Vérifier que la CI est verte sur `main` (onglet Actions du dépôt, voir « Intégration continue ») : elle ne couvre pas l'app Mac, que `--dry-run` compile.
1. Ajouter une section `## X.Y.Z` en haut de `CHANGELOG.md` : ce sont les notes affichées par la console et par l'app. Ajouter la même section, en anglais, à `CHANGELOG.en.md`. Commiter : le script refuse un arbre modifié. Les deux langues partent avec la version ([docs/i18n.md](i18n.md)) : la release GitHub porte la section française, une ligne `<!-- en -->`, puis l'anglaise (la console et l'app Windows prennent la partie de leur langue) ; dans le flux Sparkle, l'anglais est la description par défaut et le français un lien par langue (`xml:lang="fr"`) vers `site/updates/notes/X.Y.Z/KyberNow-macOS.fr.html` (`scripts/notes-html.py`), commité avec le flux du site.
2. Si les correctifs de Kyber ont changé : `server/kyber-patches/build.sh` (il écrit `third_party/payload/kyber-overlay.stamp`, l'empreinte des correctifs compilés, que le script compare à ceux du dépôt).
3. Quitter la KyberNow compilée en Release si elle est ouverte (une autre copie ne gêne pas), puis, depuis `main` :

```bash
scripts/release.sh X.Y.Z --check     # vérifications seules
scripts/release.sh X.Y.Z --dry-run   # essai complet, rien n'est publié
scripts/release.sh X.Y.Z             # publication
```

**Ordre.** Les binaires publiés doivent être ceux du commit étiqueté (l'AGPL oblige à publier le code exact des binaires) :
1. vérifications préalables, toutes affichées d'un coup ; une seule en échec et rien n'est modifié ;
2. alignement des versions (console, icône, installateur, apps Mac et Apple TV, `Cargo.lock` et projets Xcode compris) et commit « KyberNow X.Y.Z » ; un commit refusé arrête tout ;
3. construction depuis ce commit : installateur, paquet de mise à jour, app Mac signée Developer ID, SDK de Kyber pour l'Apple TV et app Apple TV ; si la construction modifie un fichier suivi, le script s'arrête (les binaires ne correspondraient plus au commit) ;
4. notarisation de l'app Mac, agrafage du ticket, vérification Gatekeeper ;
5. flux Sparkle signé, `SHA256SUMS`, manifeste signé `kybernow-release.json` (`scripts/sign-manifest.sh`, minisign) ; flux cumulatif du site (`site/updates/appcast.xml`) complété par cette version et commité par-dessus le commit de version (« Flux des mises à jour : KyberNow X.Y.Z ») ;
6. release GitHub créée **en brouillon** avec tous les fichiers ;
7. tag `vX.Y.Z` sur le commit de version ; 8. push de `main` (commit de version et commit du flux) et du tag, en une fois (`--atomic`) : le workflow Pages publie le flux ;
9. publication du brouillon.

Les fichiers restent dans `dist/vX.Y.Z/`. La version que Windows affiche pour `KyberNow-Server-Setup.exe`, `kybernow-ui.exe` et `kybernow-tray.exe` (Propriétés › Détails, manifestes) suit celle de `Cargo.toml` : leurs `build.rs` produisent les ressources depuis les gabarits `resources/*.rc.in` et `*.manifest.in`.

**Relancer après un échec** : la même commande. Le commit « KyberNow X.Y.Z » déjà en tête est réutilisé (pas de second commit), un brouillon existant reçoit les nouveaux fichiers (`--clobber`), le tag est posé s'il manque. Seule une version déjà publiée est refusée. Si l'échec est venu après l'étape 5, le commit du flux est en tête et le commit de version juste dessous : il est reconnu, l'app reconstruite, et le flux refait (l'archive a changé, donc sa signature) ; son commit est remplacé s'il n'a pas encore été poussé, complété par un nouveau sinon.

**Vérifié avant de publier** :

| Vérification | Si elle échoue |
|---|---|
| Branche `main`, arbre propre (`git status --porcelain` vide, fichiers ignorés exceptés), `origin/main` inclus | commiter, `git switch main`, `git pull` |
| Section `## X.Y.Z` dans `CHANGELOG.md` et `CHANGELOG.en.md` | l'ajouter aux deux |
| Surcouche Kyber compilée avec les correctifs du dépôt : empreinte de `server/kyber-patches/*.patch` et `build.sh` égale à `kyber-overlay.stamp` ; sans empreinte, surcouche plus récente que tous ces fichiers | `server/kyber-patches/build.sh` |
| SDK de Kyber de l'app Mac (`apps/macos/Vendor`) présent | `apps/macos/scripts/build-kyclient.sh` |
| Ni tag `vX.Y.Z` ailleurs que sur le commit de version, ni release `vX.Y.Z` déjà publiée | `git tag -d vX.Y.Z` s'il reste d'un essai |
| `gh` connecté, profil `notarytool` « kybernow » lisible (Mac déverrouillé) | `gh auth login`, voir plus bas |
| `scripts/sign-manifest.sh` présent | sans manifeste signé, la console refuse la mise à jour |
| Flux cumulatif `site/updates/appcast.xml` présent | `scripts/rebuild-appcast.sh`, une fois, puis commiter |
| Outils : xcodegen, xcodebuild, npm, cargo, zstd, MinGW, zip, python3, xmllint | Homebrew (python3 et xmllint : outils de Xcode) |

**Canaux.** Une version stable est la « dernière version » de GitHub (`releases/latest`), celle que voient la console et l'app Mac. `--beta` publie une *prerelease*, jamais marquée « dernière » : son manifeste porte `"channel": "beta"` et son flux Sparkle `<sparkle:channel>beta</sparkle:channel>`. Le numéro reste `X.Y.Z` (la console refuse tout autre format) : une bêta prend le numéro suivant, et la stable qui la suit un numéro plus haut. L'app Mac lit le flux cumulatif du site, qui réunit les deux canaux : elle ne propose une bêta que si l'utilisateur les a demandées (« Flux des mises à jour de l'app Mac », plus bas).

**Essai (`--dry-run`).** Fait tout sauf le commit, la notarisation, la release, le tag et le push, qu'il affiche (`[essai, non lancé] …`). Il aligne les versions pour construire comme la publication, puis remet les fichiers de version tels qu'il les a trouvés. Il écrit le flux cumulatif dans `dist/vX.Y.Z/appcast-site.xml` et montre ce qui changerait dans `site/updates/appcast.xml`, sans toucher à ce dernier. Il ne recompile pas le SDK de l'Apple TV (`build-kyber-tvos.sh` applique les correctifs dans `third_party/`, partagé avec les autres copies) et reprend `apps/tvos/Vendor`. Les vérifications en échec sont listées au début et à la fin, l'essai continue pour montrer la suite et sort en erreur. `--check` s'arrête après les vérifications et affiche le plan.

**Exigences d'une mise à jour.** `--min-installer A.B.C` (version de KyberNow déjà installée en dessous de laquelle le paquet est refusé, 0.2.0 par défaut) et `--requires-setup` (la mise à jour exige l'installateur) sont passés à `package-update.sh`, qui en garde une copie dans le paquet (`requirements.json`), et à `scripts/sign-manifest.sh`, dont le manifeste signé fait foi. Quand s'en servir : « Migrations des réglages », plus bas.

**Ce que l'auteur prépare une fois** : `gh auth login` avec un jeton limité au dépôt `MonsieurNours/kybernow` (jeton *fine-grained*, permission Contents en lecture et écriture : releases, tags et push de `main` ; plus Workflows si `main` pousse un changement de `.github/workflows/`), le profil de notarisation et le certificat Developer ID (« Signature et notarisation de l'app Mac »), la clé Sparkle (plus haut) et la clé minisign (en-tête de `scripts/sign-manifest.sh`).

## Flux des mises à jour de l'app Mac

L'app Mac (depuis la 0.2.7) lit `https://www.kybernow.app/updates/appcast.xml` (`SUFeedURL` dans `apps/macos/project.yml`) : le fichier `site/updates/appcast.xml` du dépôt, publié par le workflow Pages à chaque push sur `main` qui touche `site/`.

- **Toutes les versions publiées**, stables et bêtas, la plus récente en premier. Chaque entrée est celle du `appcast.xml` joint à sa release, recopiée telle quelle par `scripts/merge-appcast.py` : lien de téléchargement vers `releases/download/vX.Y.Z/KyberNow-macOS.zip`, longueur, signature EdDSA, notes. Rien n'est re-signé : l'app vérifie toujours l'archive avec `SUPublicEDKey`.
- **Canaux.** Les entrées d'une bêta portent `<sparkle:channel>beta</sparkle:channel>`, les stables aucun canal. L'app (délégué Sparkle dans `KyberNowApp.swift`) demande le canal `beta` quand « Recevoir les versions bêta » est coché dans KyberNow › Réglages… (`UserDefaults`, clé `receiveBetaUpdates`), aucun sinon. Sparkle propose la plus récente des entrées permises : une stable plus récente qu'une bêta installée est proposée normalement, même case cochée ; décocher la case ne fait pas revenir à la stable précédente, l'app attend la stable suivante.
- **Les anciennes apps continuent de marcher.** Les versions jusqu'à la 0.2.6 lisent `releases/latest/download/appcast.xml`, que GitHub résout vers la dernière stable : chaque release garde son `appcast.xml`, elles reçoivent donc la 0.2.7 par là, puis lisent le flux du site.
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
| Console, icône, installateur | macOS | `cargo test` de la console et de l'installateur ; `cargo check --target x86_64-pc-windows-gnu` de la console, de l'installateur (`--features ui`) et de l'icône, ressources Windows comprises (MinGW par Homebrew) |
| Interfaces Svelte | Linux | `npm ci`, `npm run build`, `npm run check` dans `server/web` et `server/installer/ui` |
| Scripts et update.ps1 | Linux | `shellcheck` de tous les scripts, `update.ps1` en ASCII, banc de `update.ps1` (`server/installer/tests/update-bench.ps1`, services simulés) |
| App Apple TV | macOS | `xcodegen`, puis `xcodebuild` pour le simulateur, sans signature, avec le `KyberSDK-tvOS.zip` de la dernière release |

Ce qu'elle ne couvre pas :
- **L'app Mac.** Elle lie le SDK client de Kyber et VLC (`apps/macos/Vendor`, 88 Mo, compilés depuis les sources de Kyber par `build-kyclient.sh`), publiés nulle part. Pour la compiler en CI, il faudrait joindre à chaque release un `KyberSDK-macOS.zip` (le contenu de `apps/macos/Vendor`), comme `KyberSDK-tvOS.zip` pour l'Apple TV.
- **L'app Apple TV quand les correctifs client ont changé** depuis la dernière release (`apps/macos/kyber-patches`, `build-kyber-tvos.sh`) : le SDK publié ne correspond plus au code, la compilation est sautée avec un avertissement jusqu'à la release suivante. C'est le cas jusqu'à la 0.2.6 (`KYCLIENT_MEDIA_PACKET_KIND_CLOSED`).
- **Les binaires Windows liés et Kyber** : la CI vérifie le code, elle ne produit ni `KyberNow-Server-Setup.exe` ni la surcouche (correctifs de Kyber, `third_party/` absent), et ne lance rien sous Windows.
- **Rien n'est signé ni publié** : la publication reste `scripts/release.sh`, sur le Mac de l'auteur (notarisation, clés Sparkle et minisign). Une CI qui construit les binaires Windows serait l'étape suivante, pour les faire signer Authenticode par SignPath (programme gratuit pour les projets libres, qui exige une construction en CI).

## Signature et notarisation de l'app Mac

La version diffusée (configuration Release) est signée « Developer ID Application: Mathieu Requenna (P6DB682S4K) », avec le runtime renforcé et un horodatage sécurisé, sans autorisation de débogage. Seule autorisation demandée : la caméra (`KyberNow/KyberNow.entitlements`, pour le QR code). Les compilations de développement (Debug) gardent le certificat de développement, plus rapide.

- `scripts/embed-kyber.sh` signe les bibliothèques de Kyber et de VLC selon la configuration.
- `scripts/sign-release.sh`, après `xcodebuild`, re-signe les outils de Sparkle, du plus profond au framework, puis l'app. Xcode copie Sparkle après les phases du projet, avec la signature de Sparkle, que la notarisation refuse.
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

Les réglages de la console (`C:\ProgramData\KyberNow\kyber-admin.toml`) portent la version de leur format : `schema = 1` depuis la 0.2.6 (absent = 0). À chaque démarrage, et avant `init` ou `reset-password`, la console amène le fichier au format qu'elle connaît (`server/admin/src/migrate.rs`) :

- une migration par version (de N à N+1), dans l'ordre, chacune idempotente : la rejouer ne change plus rien ;
- avant d'écrire, l'original est copié dans `backups\kyber-admin.toml.schema-<N>-<horodatage>` ; chaque changement est écrit dans le journal (`kyber-admin.log`) ; les commentaires du fichier sont gardés ;
- un fichier d'une console **plus récente** (après un retour arrière de la mise à jour) est laissé tel quel, et les réglages que cette console ne connaît pas sont gardés quand elle réécrit le fichier ;
- une migration impossible (fichier illisible) n'empêche pas la console de démarrer : le fichier est alors mis de côté comme avant (`kyber-admin.toml.invalide-…`).

À la main : `kyber-admin.exe migrate --dry-run` affiche ce qui serait changé, `kyber-admin.exe migrate` le fait.

| Schéma | Depuis | Changement |
|---|---|---|
| 1 | 0.2.6 | `listen` devient une adresse `IP:port` valide : port 9443 ajouté à une adresse seule (`"0.0.0.0"`, qui empêchait la console de démarrer), `"0.0.0.0:<port>"` pour un port seul (`listen = 9443`, qui rendait tout le fichier illisible : il était mis de côté, mot de passe compris), espaces retirés ; une valeur inutilisable est retirée (adresse par défaut). |

Ajouter une migration : une fonction `to_N` dans `migrate.rs`, ajoutée à `MIGRATIONS`, `CURRENT` passé à N, un test sur un fichier de la version N-1, et une ligne dans ce tableau.

**Mise à jour qui exige l'installateur.** Certaines versions ne peuvent pas s'installer par le paquet de mise à jour : nouveau pilote (écran virtuel, manettes), service à recréer, droits à reposer. Elles se publient avec `scripts/release.sh X.Y.Z --requires-setup` : le manifeste signé porte `"requires_setup": true` et la console refuse alors « Installer » en renvoyant vers `KyberNow-Server-Setup.exe`. De même, `--min-installer A.B.C` refuse le paquet sur un ordinateur installé avant la version A.B.C (`min_installer_version`). Le paquet en garde une copie (`requirements.json`, pour une future vérification par `update.ps1`, qui n'a pas de manifeste), le manifeste signé fait foi.
## Clés, compte GitHub et signature Windows

### Sauvegarde des clés de signature

Deux clés privées ne doivent jamais se perdre : une nouvelle clé ferait refuser toutes les mises à jour par ce qui est déjà installé, qu'il faudrait réinstaller à la main.

| Clé | Où elle vit | Clé publique attendue |
| Sparkle (app Mac, EdDSA) | Trousseau de session, élément créé par `generate_keys` | `SUPublicEDKey` dans `apps/macos/project.yml` |
| minisign (versions Windows) | `~/Library/Application Support/KyberNow/minisign.key`, chiffrée par le mot de passe de l'élément `kybernow-minisign` du Trousseau | `server/admin/src/update/kybernow-release.pub` |

La sauvegarde (faite le 2026-10-21) est une image disque chiffrée en AES-256, `kybernow-cles.dmg`, volume `KyberNow-Cles`. Elle contient :
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

{ printf '\x30\x2e\x02\x01\x00\x30\x05\x06\x03\x2b\x65\x70\x04\x22\x04\x20'; base64 -d -i /Volumes/KyberNow-Cles/sparkle-kybernow.key; } \
  | openssl pkey -inform DER -pubout -outform DER | tail -c 32 | base64
echo essai > /tmp/essai.txt
tr -d '\n' < /Volumes/KyberNow-Cles/minisign-motdepasse.txt | { read -r PW; printf '%s\n' "$PW"; } \
  | minisign -S -s /Volumes/KyberNow-Cles/kybernow-minisign.key -m /tmp/essai.txt -x /tmp/essai.txt.minisig >/dev/null 2>&1 \
  && minisign -V -q -p server/admin/src/update/kybernow-release.pub -m /tmp/essai.txt -x /tmp/essai.txt.minisig && echo minisign OK
rm -f /tmp/essai.txt /tmp/essai.txt.minisig

Remettre les clés sur un autre Mac (image ouverte) :

$GK -f /Volumes/KyberNow-Cles/sparkle-kybernow.key && $GK -p      # doit afficher SUPublicEDKey
mkdir -p ~/Library/Application\ Support/KyberNow
cp /Volumes/KyberNow-Cles/kybernow-minisign.key ~/Library/Application\ Support/KyberNow/minisign.key
cp /Volumes/KyberNow-Cles/kybernow-minisign.pub ~/Library/Application\ Support/KyberNow/minisign.pub
chmod 600 ~/Library/Application\ Support/KyberNow/minisign.key
printf 'add-generic-password -a kybernow -s kybernow-minisign -l kybernow-minisign -w %s\n' \
  "$(cat /Volumes/KyberNow-Cles/minisign-motdepasse.txt)" | security -i >/dev/null

`printf` est interne au shell : le mot de passe ne passe pas dans la liste des processus.

### Réglages GitHub qui protègent les publications

État au 2026-10-21, vérifié avec `gh api` :
- **2FA** active sur le compte `MonsieurNours`, seul collaborateur du dépôt.
- **Releases immuables** (Settings › General › Releases). Une release publiée ne peut plus changer de fichiers ni de tag. `release.sh` crée un brouillon, y dépose tout, puis publie, ce qui reste compatible. Une erreur dans une version publiée se corrige par une nouvelle version : le numéro d'une release supprimée ne peut pas resservir.
- **Règles (Settings › Rules)** :
  - `tags-v-creation` : seul le rôle admin peut créer un tag `v*`. Le `GITHUB_TOKEN` des Actions ne le peut pas.
  - `tags-v-figes` : aucun tag `v*` ne peut être déplacé ni supprimé, sans exception. Si une publication échoue après le push du tag, publiez la version suivante, ou désactivez la règle le temps du nettoyage.
  - `main-legere` : `main` ne peut être ni supprimée ni réécrite (force push). Les pushes directs restent permis.
- **`gh` sur le Mac de publication** : jeton OAuth de `gh` (droits larges, Mac de développement). `release.sh` s'en sert pour les releases ; les `git push` passent par SSH. Un jeton à granularité fine limité au dépôt aurait besoin au minimum de *Contents: Read and write* : GitHub n'a pas de permission plus étroite pour les releases, et ce jeton permettrait donc aussi de pousser. Retirer le droit inutile `admin:public_key`, qui permet d'ajouter une clé SSH au compte : `gh auth refresh -h github.com --remove-scopes admin:public_key`.
- **CI** : les workflows utilisent le `GITHUB_TOKEN` fourni à chaque exécution, réduit à `contents: read`. Le réglage par défaut du dépôt est aussi la lecture seule. Aucun secret à créer.

Relire les règles :

gh api repos/MonsieurNours/kybernow/rulesets --jq '.[].name'
gh api repos/MonsieurNours/kybernow/immutable-releases

### Signature Windows (Authenticode) : en attente

Aujourd'hui, `KyberNow-Server-Setup.exe`, `kyber-admin.exe`, `kybernow-tray.exe` et `update.ps1` ne sont pas signés : Windows affiche « Éditeur inconnu » (SmartScreen, UAC). Cette signature ne remplacerait pas minisign. Authenticode dit qui a signé ; minisign prouve que c'est la clé de KyberNow, gravée dans la console, ce qui protège aussi contre un compte GitHub compromis.

Décision du 2026-10-21 : **reportée**, à reprendre quand le build Windows sera plus mûr. L'option visée est SignPath Foundation (gratuite).

À savoir avant d'y revenir (vérifié le 2026-10-21) :
- **Nom affiché.** Le SIREN 510 178 577 figure dans SIRENE au nom de « MATTHIEU REQUENNA », sans nom commercial. Aucun certificat ne peut donc porter « Les Lignes Claires » tant que ce nom n'est pas déclaré au registre (guichet unique de l'INPI, à vérifier).
- **Durée maximale** d'un certificat : 460 jours depuis le 2026-03-01.
- **SmartScreen.** Aucun certificat, EV compris, ne donne plus de réputation immédiate. Elle se construit avec les installations et se garde tant que l'identité de signature ne change pas.

| Option | Prix (2026-10) | Éditeur affiché | Signature depuis le Mac | Remarques |
|---|---|---|---|---|
| SignPath Foundation | gratuit | « SignPath Foundation » | non | build Windows obligatoire sur les runners GitHub, approbation manuelle de chaque version |
| Certum Open Source | 69 € (carte + lecteur) ou 49 € (nuage SimplySign) par an | « Open Source Developer, Matthieu Requenna » | oui, `jsign` (carte) | réservé aux personnes physiques |
| Azure Artifact Signing | 9,99 $/mois | nom légal | oui, `jsign` | particuliers : États-Unis et Canada seulement ; EI française non documentée |
| IV / OV (SSL.com, Sectigo, Certum) | IV 129 $/an + jeton ou nuage ; OV ≈ 200–400 $/an | « Matthieu Requenna » | oui, `jsign` | clé sur jeton ou dans le nuage, obligatoire |
| EV | ≈ 330–380 €/an | « Matthieu Requenna » | oui, `jsign` | indispensable pour un pilote attesté |

Ce que SignPath demandera :
- **Construire toute la partie Windows dans GitHub Actions** : console, icône, installateur, **surcouche Kyber** (`server/kyber-patches/build.sh`), et paquets tiers téléchargés puis vérifiés par empreinte.
- Une requête de signature par version (`signpath/github-action-submit-signing-request`, jeton d'API en secret Actions), avec approbation manuelle sur signpath.io.
- Sur www.kybernow.app :
  - une page « Code signing policy », avec la phrase « Free code signing provided by SignPath.io, certificate by SignPath Foundation » ;
  - les rôles (auteur, relecteur, approbateur) ;
  - une déclaration de confidentialité, qui mentionne la vérification des mises à jour auprès de GitHub toutes les 6 heures.
- Dans chaque fichier signé, `ProductName = KyberNow` et la même version.
- La 2FA sur le compte SignPath.
- Deux points à déclarer, qui peuvent bloquer l'admission :
  - l'installateur ajoute `sudovda.cer` (autosigné, usage « Code Signing ») aux magasins Root et TrustedPublisher ;
  - `WebView2Loader.dll` est un composant Microsoft redistribuable, pas open source.

Intégration, quelle que soit l'option, dans cet ordre :
1. signer `kyber-admin.exe`, `kybernow-tray.exe` et `update.ps1` ;
2. construire `kybernow-update.zip` et `KyberNow-Server-Setup.exe`, puis signer l'installateur ;
3. calculer `SHA256SUMS` et signer le manifeste minisign, en dernier, car la signature Authenticode change les empreintes.

`jsign` (Homebrew) signe depuis le Mac les `.exe`, `.cab` et scripts PowerShell. Avec SignPath, `release.sh` récupérerait les fichiers signés par la CI (`gh run download`) avant l'étape 3.

Pilote d'écran virtuel attesté (pour remplacer SudoVDA autosigné, plus tard) :
- un certificat **EV** ;
- un compte Partner Center « Hardware » (gratuit, l'EV sert à l'inscription) ;
- la soumission d'un `.cab` signé EV, que Microsoft renvoie signé, valable sur Windows 10/11 de bureau.

Azure Artifact Signing n'est pas accepté pour cela. À vérifier avant d'acheter : que Partner Center accepte une EI française.
