# Identité KyberNow v2

[English](README.en.md) · Français

Adoptée le 20 octobre 2026, signal Spectre. Remplace le K en ruban de `brand/` ; les apps y passent au fil des évolutions. Charte complète : [`charte/index.html`](charte/index.html), à ouvrir dans un navigateur (version en ligne : https://claude.ai/artifact/R3mNbayaFan3XPCtbmzCFt). Maquettes animées des apps (accueil, passage et session sur Mac, installateur, console) : https://claude.ai/artifact/YGA7unwqztvuA26CMtdbv8.

## En bref

- **Symbole « Relais »** : un carré coupé en deux par une couture à 45°. En haut à gauche l'ordinateur, en bas à droite vous, entre les deux le lien. Aucune lettre : il survit à un changement de nom.
- **Trois états** : `now` (app Mac/Apple TV et écosystème, la moitié basse en signal), `server` (la moitié haute), `console` (la couture seule).
- **Nom** : Archivo largeur 125, « Kyber » en graisse 420, « Now » en 720, vectorisé.
- **Couleurs** : encre `#05070A`, givre `#E3EDF0`, signal Spectre `#9A8CFF` (aplat `#6D5BFF`, sur fond clair `#5340E6`). Les pistes Givre et Tally ont été écartées ; `generate.mjs` sait encore les produire.
- **Polices** : Archivo (titres en largeur 125, texte en largeur 100) et Martian Mono (données). Licences OFL dans `fonts/`.
- **Lumière** : le signal est aussi un tube néon (cœur `#F4F1FF`, gaine `#9A8CFF`, halo `#6D5BFF`). Éteint au repos ; veille au survol, éclair au clic, allumé tant qu'une connexion, un flux ou une mesure en direct est en cours, allumage d'une seconde à l'apparition d'un écran. Jamais devant l'image en session.
- **Icônes** : Phosphor (MIT). Regular à 20 px, Bold à 16 px et moins, Fill pour l'état actif.

## Fichiers

| Dossier | Contenu |
|---|---|
| `mark/` | Symbole, trois états, fond sombre et clair ; `*-lit-dark.svg` : allumé (néon) |
| `logo/` | Horizontal, vertical, nom seul |
| `app-icon/` | Icônes 1024 (gabarit macOS) : client, server, console, couture en néon |
| `favicon/` | Favicon qui suit le thème du navigateur |
| `tokens/` | `kybernow.css` et `Theme+Brand.swift` (couleurs, formes, mouvement, néon : `.neon(_:)`, `.neonIgnition(trigger:)`, `.knPrimary`) |
| `fonts/` | Archivo et Martian Mono variables |
| `charte/` | La charte (page autonome) |
| `tools/` | Scripts de génération |

## Régénérer

Ne pas retoucher les SVG à la main.

```sh
# SVG (Spectre par défaut)
node brand/v2/tools/generate.mjs

# Charte : il faut les icônes Phosphor (npm i @phosphor-icons/core@2.1.1 dans un dossier de travail)
node brand/v2/tools/build-charte.mjs <…>/node_modules/@phosphor-icons/core/assets

# Nom vectorisé, si la police ou les réglages changent (fontTools requis)
python3 brand/v2/tools/outline.py '{"kyber":{"text":"Kyber","wght":420,"wdth":125,"tracking":-1.2},"now":{"text":"Now","wght":720,"wdth":125,"tracking":-1.2}}' > brand/v2/tools/wordmark.json
```

La géométrie du symbole est dans `tools/generate.mjs` (grille de 48, couture de 4, rayons de 7 et 1,2) ; `tools/geometry.json` en est la sortie, utilisée par la charte et la 3D.

## À trancher avant diffusion

Le nom : « KyberNow » contient la marque d'un tiers. Voir la section 14 de la charte (Rapport à Kyber).
