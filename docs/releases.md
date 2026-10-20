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
- L'installation télécharge `kybernow-update.zip` et vérifie son empreinte dans `SHA256SUMS`. La console lance ensuite PowerShell, détaché : il décompresse le paquet et exécute son `update.ps1`.
- Le script arrête l'icône et les services, remplace les fichiers (ceux de Kyber remplacés sont gardés dans `kyber\avant-maj`), puis remet les droits des services, les haut-parleurs de Steam et le pare-feu.
- Il redémarre ensuite les services. La nouvelle console relance l'icône dans la session de l'utilisateur, avec son compte (fichier `update\relaunch-tray`). La page se met à jour seule.
- Durée : environ une minute. Une session de streaming en cours est coupée. Journal : `C:\ProgramData\KyberNow\update\<version>\update.log`.
- La console suit le PowerShell de l'installation. S'il s'arrête alors que la console tourne encore (une étape a échoué), elle l'indique : page Mises à jour, fin du journal, et « Installer » redevient possible. Avant la 0.2.2, la console restait sur « une installation est déjà en cours » jusqu'à son redémarrage.
- PowerShell exécute `update\<version>\lancement.ps1` (écrit par la console) avec une console cachée ; sa sortie va dans `lancement.log`, à lire si `update.log` n'existe pas. La console 0.2.0 le lançait sans console ni sortie : il pouvait s'arrêter avant d'écrire son journal (seul le zip restait dans le dossier). Dans ce cas, mettre à jour à la main avec `update.ps1`.

**L'ordinateur, à la main** (PowerShell administrateur) :

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

Pour essayer un paquet servi depuis le Mac avant de le publier : `$env:KYBERNOW_UPDATE_BASE = 'http://<adresse-du-Mac>:8000'`, puis la même commande, avec `python3 -m http.server 8000` lancé dans `server/installer/dist`.

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
3. Quitter KyberNow sur le Mac, puis lancer :

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

La première notarisation d'un compte peut prendre plusieurs heures (vérifications d'Apple) ; ensuite, quelques minutes. Pour retrouver un envoi : `xcrun notarytool history --keychain-profile kybernow`.
