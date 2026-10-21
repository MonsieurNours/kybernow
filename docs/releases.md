# Versions et mises à jour

[English](releases.en.md) · Français

Les versions de KyberNow sont publiées sur GitHub : https://github.com/MonsieurNours/kybernow/releases. Le dépôt est public : les mises à jour se téléchargent sans jeton, et le code reste disponible comme l'exige l'AGPL (avec les correctifs de Kyber, voir [kyber-changes.md](kyber-changes.md)).

Un seul numéro de version pour tout KyberNow : la console, l'icône, l'installateur, l'app Mac et l'app Apple TV changent ensemble.

## Ce que contient une version

| Fichier | Pour quoi |
|---|---|
| `KyberNow-Setup.exe` | Première installation sur un ordinateur Windows |
| `kybernow-update.zip` | Mise à jour d'un ordinateur déjà installé : surcouche Kyber, console, icône, `update.ps1` |
| `update.ps1` | Mise à jour à la main, sans la console |
| `KyberNow-macOS.zip` | L'app Mac (première installation : la glisser dans Applications) |
| `appcast.xml` | Flux des mises à jour de l'app Mac (Sparkle), signé EdDSA |
| `KyberSDK-tvOS.zip` | SDK de Kyber compilé pour l'Apple TV, que télécharge `apps/tvos/scripts/sideload.sh` pour installer l'app sans l'App Store ([SIDELOAD.md](../apps/tvos/SIDELOAD.md)) |
| `kybernow-release.json` | Manifeste de la version : numéro, canal (`stable` ou `beta`), date, version minimale déjà installée pour une mise à jour depuis la console, réinstallation nécessaire ou non, empreinte SHA-256 et taille de chaque fichier ci-dessus |
| `kybernow-release.json.minisig` | Signature du manifeste (minisign, ed25519), vérifiée par la console avant toute installation |
| `SHA256SUMS` | Empreintes de tous les fichiers |

## Mettre à jour

**L'ordinateur, en un clic.**
- La console vérifie la dernière version toutes les 6 heures. La page **Mises à jour** l'affiche avec ses notes et un bouton **Installer** ; l'icône de la zone de notification propose « Installer la version X… », qui ouvre cette page.
- L'installation télécharge `kybernow-update.zip` et vérifie son empreinte dans `SHA256SUMS`. Le numéro de la version publiée (`tag_name`) doit être de la forme `X.Y.Z` : il sert de nom de dossier et passe dans PowerShell, tout autre numéro est refusé. La console lance ensuite PowerShell, détaché : il décompresse le paquet et exécute son `update.ps1`.
- Le script arrête le serveur (`kyber`), puis la console (`kyber-admin`), puis l'icône. Il prépare les nouveaux dossiers à côté des anciens (`kyber.new`, `admin.new`, `tray.new` : copie du dossier actuel, réglages et certificat de Kyber compris, puis les fichiers du paquet par-dessus), puis bascule par renommage : `kyber` devient `kyber.prev` et `kyber.new` devient `kyber`, de même pour `admin` et `tray`. Les journaux de Kyber (`kyber\log`) sont déplacés, pas copiés. Il remet ensuite les droits des services, les haut-parleurs de Steam et le pare-feu.
- Il redémarre les services, puis contrôle la santé de la nouvelle version : la console doit accepter une connexion sur son port en 60 s, et le serveur tourner. Sinon, il arrête tout, remet les dossiers précédents (`kyber.prev` redevient `kyber`…), redémarre l'ancienne version et écrit la raison dans le journal et dans `C:\ProgramData\KyberNow\update\resultat.json`. La console relit ce fichier à son démarrage : la page Mises à jour affiche l'échec, et `GET /api/update` le donne dans `install_error` et `last_failure` (version, raison, retour arrière, journal).
- Quoi qu'il arrive après l'arrêt des services (erreur, coupure, renommage impossible parce qu'un fichier est ouvert), le script les redémarre et relance l'icône en sortant (`try … finally`) : l'ordinateur n'est jamais laissé sans serveur ni console.
- Réussie, la mise à jour inscrit la nouvelle version dans `C:\Program Files\KyberNow\kybernow.json` et dans « Applications installées » (`DisplayVersion`). La version précédente reste dans les dossiers `.prev` jusqu'à la mise à jour suivante (environ 300 Mo). Dans `update\`, seuls les dossiers des deux dernières versions sont gardés.
- La console relance l'icône dans la session de l'utilisateur, avec son compte (fichier `update\relaunch-tray`). La page se met à jour seule.
- Durée : environ une minute. Une session de streaming en cours est coupée. Journal : `C:\ProgramData\KyberNow\update\<version>\update.log`.
- La console suit le PowerShell de l'installation. S'il s'arrête alors que la console tourne encore (une étape a échoué), elle l'indique : page Mises à jour, fin du journal, et « Installer » redevient possible. Avant la 0.2.2, la console restait sur « une installation est déjà en cours » jusqu'à son redémarrage.
- PowerShell exécute `update\<version>\lancement.ps1` (écrit par la console) avec une console cachée ; sa sortie va dans `lancement.log`, à lire si `update.log` n'existe pas. La console 0.2.0 le lançait sans console ni sortie : il pouvait s'arrêter avant d'écrire son journal (seul le zip restait dans le dossier). Dans ce cas, mettre à jour à la main avec `update.ps1`.

**L'ordinateur, à la main** (PowerShell administrateur) :

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

Le script télécharge `kybernow-update.zip` et `SHA256SUMS`, vérifie l'empreinte du paquet comme la console, puis fait la même bascule avec contrôle de santé. En cas d'échec, il le dit en rouge (« Mise a jour non installee… ») après avoir tout redémarré.

Pour essayer un paquet servi depuis le Mac avant de le publier : `$env:KYBERNOW_UPDATE_BASE = 'http://<adresse-du-Mac>:8000'`, puis la même commande, avec `python3 -m http.server 8000` lancé dans `server/installer/dist` (`package-update.sh` y écrit aussi `SHA256SUMS`).

**Tester `update.ps1` sans Windows** : `pwsh -NoProfile -File server/installer/tests/update-bench.ps1` (`brew install powershell`). Le banc simule les services et la console, et rejoue une mise à jour réussie, une console muette (retour arrière), un paquet incomplet et un renommage bloqué.

**L'app Mac.**
- Sparkle vérifie chaque jour, et à la demande (menu KyberNow › Rechercher des mises à jour…). L'app télécharge l'archive, vérifie sa signature EdDSA avec la clé publique de l'app (`SUPublicEDKey`), la remplace et se relance.
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

`files` liste ceux des fichiers publiés qui sont présents (`KyberNow-Setup.exe`, `kybernow-update.zip`, `update.ps1`, `KyberNow-macOS.zip`, `appcast.xml`, `KyberSDK-tvOS.zip`). La signature détachée, `kybernow-release.json.minisig`, porte le commentaire de confiance `kybernow <version> <canal>`. La clé publique est dans le dépôt, `server/admin/src/update/kybernow-release.pub` (identifiant `D63B24FFFC7E79D1`), et compilée dans la console : une empreinte SHA-256 seule protège d'un téléchargement abîmé, pas d'une version falsifiée par qui aurait la main sur le compte GitHub ; la signature, si.

Le manifeste s'écrit et se signe avec :

```bash
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable            # ou beta
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable --min-installer 0.2.6 --requires-setup
```

`--min-installer X.Y.Z` : version la plus ancienne depuis laquelle la console peut installer ce paquet (0.2.0 par défaut). `--requires-setup` : cette version ne s'installe qu'avec `KyberNow-Setup.exe` (nouveau pilote, nouveau service…). Le script installe `minisign` avec Homebrew s'il manque, puis vérifie la signature contre la clé publique du dépôt : une clé privée qui ne lui correspond pas est vue avant la publication.

**La clé privée** est dans `~/Library/Application Support/KyberNow/minisign.key`, chiffrée par un mot de passe aléatoire gardé dans le Trousseau (élément `kybernow-minisign`). Le script la crée si elle manque et écrit la clé publique dans le dépôt ; si une clé publique est déjà dans le dépôt, il refuse d'en créer une autre : une nouvelle clé ferait refuser toutes les mises à jour par les consoles déjà installées, qu'il faudrait réinstaller avec `KyberNow-Setup.exe`.

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

1. Ajouter une section `## X.Y.Z` en haut de `CHANGELOG.md` : ce sont les notes affichées par la console et par l'app. Ajouter la même section, en anglais, à `CHANGELOG.en.md`.
2. Si les correctifs de Kyber ont changé : `server/kyber-patches/build.sh`.
3. Quitter la KyberNow compilée en Release si elle est ouverte (une autre copie ne gêne pas), puis lancer :

```bash
scripts/release.sh X.Y.Z
```

Le script aligne les versions, construit l'installateur, le paquet de mise à jour et l'app (Release), signe le flux Sparkle et calcule `SHA256SUMS`. Il commite, étiquette `vX.Y.Z`, pousse, puis crée la version GitHub. Les fichiers restent dans `dist/vX.Y.Z/`.

Tant que `release.sh` n'appelle pas `scripts/sign-manifest.sh` lui-même, signez le manifeste après lui et joignez-le à la version, sans quoi la console refuse de l'installer :

```bash
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable
gh release upload vX.Y.Z -R MonsieurNours/kybernow dist/vX.Y.Z/kybernow-release.json dist/vX.Y.Z/kybernow-release.json.minisig
```

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
