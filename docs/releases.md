# Versions et mises à jour

Les versions de KyberNow sont publiées sur GitHub : https://github.com/MonsieurNours/kybernow/releases. Le dépôt est public : les mises à jour se téléchargent sans jeton, et le code reste disponible comme l'exige l'AGPL (avec les correctifs de Kyber, voir [kyber-changes.md](kyber-changes.md)).

Un seul numéro de version pour tout KyberNow : la console, l'icône, l'installateur et l'app Mac changent ensemble.

## Ce que contient une version

| Fichier | Pour quoi |
|---|---|
| `KyberNow-Setup.exe` | Première installation sur un ordinateur Windows |
| `kybernow-update.zip` | Mise à jour d'un ordinateur déjà installé : surcouche Kyber, console, icône, `update.ps1` |
| `update.ps1` | Mise à jour à la main, sans la console |
| `KyberNow-macOS.zip` | L'app Mac (première installation : la glisser dans Applications) |
| `appcast.xml` | Flux des mises à jour de l'app Mac (Sparkle), signé EdDSA |
| `SHA256SUMS` | Empreintes de tous les fichiers |

## Mettre à jour

**L'ordinateur, en un clic.**
- La console vérifie la dernière version toutes les 6 heures. La page **Mises à jour** l'affiche avec ses notes et un bouton **Installer** ; l'icône de la zone de notification propose « Installer la version X… », qui ouvre cette page.
- L'installation télécharge `kybernow-update.zip` et vérifie son empreinte dans `SHA256SUMS`. La console lance ensuite PowerShell, détaché : il décompresse le paquet et exécute son `update.ps1`.
- Le script arrête l'icône et les services, remplace les fichiers (ceux de Kyber remplacés sont gardés dans `kyber\avant-maj`), puis remet les droits des services, les haut-parleurs de Steam et le pare-feu.
- Il redémarre ensuite les services. La nouvelle console relance l'icône dans la session de l'utilisateur, avec son compte (fichier `update\relaunch-tray`). La page se met à jour seule.
- Durée : environ une minute. Une session de streaming en cours est coupée. Journal : `C:\ProgramData\KyberNow\update\<version>\update.log`.

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

1. Ajouter une section `## X.Y.Z` en haut de `CHANGELOG.md` : ce sont les notes affichées par la console et par l'app.
2. Si les correctifs de Kyber ont changé : `server/kyber-patches/build.sh`.
3. Quitter KyberNow sur le Mac, puis lancer :

```bash
scripts/release.sh X.Y.Z
```

Le script aligne les versions, construit l'installateur, le paquet de mise à jour et l'app (Release), signe le flux Sparkle et calcule `SHA256SUMS`. Il commite, étiquette `vX.Y.Z`, pousse, puis crée la version GitHub. Les fichiers restent dans `dist/vX.Y.Z/`.

## Reste à faire : signature Developer ID

L'app est signée avec le certificat de développement des Lignes Claires. Elle s'ouvre et se met à jour sur les Mac de l'équipe, mais macOS la refusera sur un autre Mac téléchargé depuis GitHub. Pour la diffuser :

1. Créer le certificat : Xcode › Réglages › Comptes › (l'équipe P6DB682S4K) › Gérer les certificats › + › Developer ID Application.
2. Créer un mot de passe pour app sur https://account.apple.com (Connexion et sécurité › Mots de passe pour app), puis l'enregistrer pour `notarytool` (à saisir vous-même) :

```bash
xcrun notarytool store-credentials kybernow --apple-id <votre identifiant Apple> --team-id P6DB682S4K
```

3. Ensuite, dans le projet : runtime renforcé avec l'autorisation caméra (QR code), signature Developer ID en Release, et notarisation dans `scripts/release.sh` (`notarytool submit --keychain-profile kybernow --wait` puis `stapler staple`).
