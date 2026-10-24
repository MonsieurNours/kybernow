# Logo KyberNow

[English](README.en.md) · Français

> Logo précédent, gardé comme archive : ni les apps, ni Admin, ni les installateurs, ni le site ne s'en servent. L'identité en vigueur est [`v2/`](v2/README.md) (symbole « Relais », signal Spectre).

Le K en ruban plié, traversé par trois lignes de signal qui pulsent dans son ouverture : l'image qui part de l'ordinateur et arrive sur l'écran.

## Fichiers

| Fichier | Usage |
|---|---|
| `kybernow-logo.svg` | Logo vertical (symbole + nom), fond sombre |
| `kybernow-logo-light.svg` | Logo vertical, fond clair |
| `kybernow-logo-horizontal.svg` | Logo horizontal, fond sombre |
| `kybernow-logo-horizontal-light.svg` | Logo horizontal, fond clair |
| `kybernow-mark.svg` / `kybernow-mark-light.svg` | Symbole seul |
| `kybernow-app-icon.svg` | Icône d'application (fond nuit arrondi) |
| `kybernow-app-icon-1024.png` | La même, en PNG 1024 × 1024 |
| `kybernow-mark-animated.svg` | Démonstration d'animation |
| `preview.html` | Planche de toutes les déclinaisons |

Le nom est vectorisé (Bricolage Grotesque, graisse 700) : aucun fichier ne dépend d'une police installée.

## Couleurs

| Rôle | Fond sombre | Fond clair |
|---|---|---|
| K, « Kyber » | `#F4F7FA` | `#151B23` |
| Signal, « Now » | `#6FE3C1` (menthe) | `#12A37E` |
| Ombre du bras | `#9AA6B4` → `#DCE3EA` | `#5B6878` → `#2A3441` |
| Plis du ruban | `#B6C1CD` | `#46525F` |
| Fond d'icône | `#0D1117` | — |

## Structure pour l'animation

Chaque pièce est un élément séparé, nommé, sans transformation intermédiaire :

```
#kn-mark
├── #kn-signal              lignes de signal (dessinées derrière le K)
│   ├── #kn-wave-1  .kn-wave   pathLength="1000"
│   ├── #kn-wave-2  .kn-wave   pathLength="1000"  (ligne principale)
│   └── #kn-wave-3  .kn-wave   pathLength="1000"
└── #kn-k
    ├── #kn-arm-bottom  .kn-piece
    ├── #kn-arm-top     .kn-piece
    ├── #kn-stem        .kn-piece
    ├── #kn-fold-top    .kn-piece
    └── #kn-fold-bottom .kn-piece
#kn-wordmark
├── #kn-word-kyber
└── #kn-word-now
```

- Les lignes ont `pathLength="1000"` : on les dessine ou on y fait circuler une impulsion avec `stroke-dasharray` et `stroke-dashoffset`, quelle que soit leur longueur réelle.
- Les pièces du K s'animent individuellement avec `transform-box: fill-box; transform-origin: center`.
- Toujours prévoir `@media (prefers-reduced-motion: reduce)`.

Exemple (repris de `kybernow-mark-animated.svg`) :

```css
.kn-wave { stroke-dasharray: 1000; stroke-dashoffset: 1000; animation: kn-draw 1.6s forwards; }
@keyframes kn-draw { to { stroke-dashoffset: 0 } }

/* impulsion qui parcourt la ligne principale */
.kn-pulse { stroke-dasharray: 60 940; animation: kn-travel 2.4s linear infinite; }
@keyframes kn-travel { from { stroke-dashoffset: 1000 } to { stroke-dashoffset: 0 } }
```

Pour l'insérer dans une page et l'animer, inclure le SVG en ligne (pas en `<img>`), afin que le CSS de la page atteigne ses éléments.

## Régénérer

Le script qui produit ces fichiers à partir de la géométrie du symbole et de la police n'est pas dans le dépôt : une retouche à la main se reporte dans toutes les déclinaisons.
