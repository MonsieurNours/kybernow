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

1. Ajouter une section `## X.Y.Z` en haut de `CHANGELOG.md` : ce sont les notes affichées par la console et par l'app. Ajouter la même section, en anglais, à `CHANGELOG.en.md`.
2. Si les correctifs de Kyber ont changé : `server/kyber-patches/build.sh`.
3. Quitter la KyberNow compilée en Release si elle est ouverte (une autre copie ne gêne pas), puis lancer :

```bash
scripts/release.sh X.Y.Z
```

Le script aligne les versions, construit l'installateur, le paquet de mise à jour et l'app (Release), signe le flux Sparkle et calcule `SHA256SUMS`. Il commite, étiquette `vX.Y.Z`, pousse, puis crée la version GitHub. Les fichiers restent dans `dist/vX.Y.Z/`.

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

## Clés, compte GitHub et signature Windows

### Sauvegarde des clés de signature

Deux clés privées ne doivent jamais se perdre : une nouvelle clé ferait refuser toutes les mises à jour par ce qui est déjà installé, qu'il faudrait réinstaller à la main.

| Clé | Où elle vit | Clé publique attendue |
|---|---|---|
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
- **`gh` sur le Mac de publication** : jeton OAuth de `gh` (droits larges, Mac de développement). `release.sh` s'en sert pour les releases ; les `git push` passent par SSH. Un jeton à granularité fine limité au dépôt aurait besoin au minimum de *Contents: Read and write* : GitHub n'a pas de permission plus étroite pour les releases, et ce jeton permettrait donc aussi de pousser. Retirer le droit inutile `admin:public_key`, qui permet d'ajouter une clé SSH au compte : `gh auth refresh -h github.com --remove-scopes admin:public_key`.
- **CI** : les workflows utilisent le `GITHUB_TOKEN` fourni à chaque exécution, réduit à `contents: read`. Le réglage par défaut du dépôt est aussi la lecture seule. Aucun secret à créer.

Relire les règles :

```bash
gh api repos/MonsieurNours/kybernow/rulesets --jq '.[].name'
gh api repos/MonsieurNours/kybernow/immutable-releases
```

### Signature Windows (Authenticode) : en attente

Aujourd'hui, `KyberNow-Setup.exe`, `kyber-admin.exe`, `kybernow-tray.exe` et `update.ps1` ne sont pas signés : Windows affiche « Éditeur inconnu » (SmartScreen, UAC). Cette signature ne remplacerait pas minisign. Authenticode dit qui a signé ; minisign prouve que c'est la clé de KyberNow, gravée dans la console, ce qui protège aussi contre un compte GitHub compromis.

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
2. construire `kybernow-update.zip` et `KyberNow-Setup.exe`, puis signer l'installateur ;
3. calculer `SHA256SUMS` et signer le manifeste minisign, en dernier, car la signature Authenticode change les empreintes.

`jsign` (Homebrew) signe depuis le Mac les `.exe`, `.cab` et scripts PowerShell. Avec SignPath, `release.sh` récupérerait les fichiers signés par la CI (`gh run download`) avant l'étape 3.

Pilote d'écran virtuel attesté (pour remplacer SudoVDA autosigné, plus tard) :
- un certificat **EV** ;
- un compte Partner Center « Hardware » (gratuit, l'EV sert à l'inscription) ;
- la soumission d'un `.cab` signé EV, que Microsoft renvoie signé, valable sur Windows 10/11 de bureau.

Azure Artifact Signing n'est pas accepté pour cela. À vérifier avant d'acheter : que Partner Center accepte une EI française.
