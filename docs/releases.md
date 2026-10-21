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

## Publier une version

1. Ajouter une section `## X.Y.Z` en haut de `CHANGELOG.md` : ce sont les notes affichées par la console et par l'app. Ajouter la même section, en anglais, à `CHANGELOG.en.md`. Commiter : le script refuse un arbre modifié.
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
5. flux Sparkle signé, `SHA256SUMS`, manifeste signé `kybernow-release.json` (`scripts/sign-manifest.sh`, minisign) ;
6. release GitHub créée **en brouillon** avec tous les fichiers ;
7. tag `vX.Y.Z` sur le commit de version ; 8. push de `main` et du tag, en une fois (`--atomic`) ;
9. publication du brouillon.

Les fichiers restent dans `dist/vX.Y.Z/`. La version que Windows affiche pour `KyberNow-Setup.exe`, `kybernow-ui.exe` et `kybernow-tray.exe` (Propriétés › Détails, manifestes) suit celle de `Cargo.toml` : leurs `build.rs` produisent les ressources depuis les gabarits `resources/*.rc.in` et `*.manifest.in`.

**Relancer après un échec** : la même commande. Le commit « KyberNow X.Y.Z » déjà en tête est réutilisé (pas de second commit), un brouillon existant reçoit les nouveaux fichiers (`--clobber`), le tag est posé s'il manque. Seule une version déjà publiée est refusée.

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
| Outils : xcodegen, xcodebuild, npm, cargo, zstd, MinGW, zip | Homebrew |

**Canaux.** Une version stable est la « dernière version » de GitHub (`releases/latest`), celle que voient la console et l'app Mac. `--beta` publie une *prerelease*, jamais marquée « dernière » : son manifeste porte `"channel": "beta"` et son flux Sparkle `<sparkle:channel>beta</sparkle:channel>`. Le numéro reste `X.Y.Z` (la console refuse tout autre format) : une bêta prend le numéro suivant, et la stable qui la suit un numéro plus haut. Limite actuelle de l'app Mac : elle lit `releases/latest/download/appcast.xml`, que GitHub résout vers la dernière **stable** ; pour lui proposer les bêtas, il faudra un flux qui réunit les deux canaux (par exemple sur le site) et `allowedChannels` dans l'app.

**Essai (`--dry-run`).** Fait tout sauf le commit, la notarisation, la release, le tag et le push, qu'il affiche (`[essai, non lancé] …`). Il aligne les versions pour construire comme la publication, puis remet les fichiers de version tels qu'il les a trouvés. Il ne recompile pas le SDK de l'Apple TV (`build-kyber-tvos.sh` applique les correctifs dans `third_party/`, partagé avec les autres copies) et reprend `apps/tvos/Vendor`. Les vérifications en échec sont listées au début et à la fin, l'essai continue pour montrer la suite et sort en erreur. `--check` s'arrête après les vérifications et affiche le plan.

**Exigences d'une mise à jour.** `--min-installer A.B.C` (version de KyberNow déjà installée en dessous de laquelle le paquet est refusé) et `--requires-setup` (la mise à jour exige l'installateur) sont passés à `package-update.sh` et au manifeste signé.

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

**Mise à jour qui exige l'installateur.** Certaines versions ne peuvent pas s'installer par le paquet de mise à jour : nouveau pilote (écran virtuel, manettes), service à recréer, droits à reposer. Elles se publient avec `scripts/release.sh X.Y.Z --requires-setup` : le manifeste signé porte `"requires_setup": true` et la console refuse alors « Installer » en renvoyant vers `KyberNow-Setup.exe`. De même, `--min-installer A.B.C` refuse le paquet sur un ordinateur installé avant la version A.B.C (`min_installer_version`). Le paquet en garde une copie (`requirements.json`), le manifeste signé fait foi.
