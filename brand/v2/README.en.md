# KyberNow identity v2

English · [Français](README.md)

Adopted on 20 October 2026, Spectre signal. Replaces the ribbon K in `brand/`; the apps move to it as they evolve. Full guidelines: [`charte/index.html`](charte/index.html) (in French), to open in a browser (online version: https://claude.ai/artifact/R3mNbayaFan3XPCtbmzCFt). Animated mock-ups of the apps (home, transition and session on the Mac, installer, console): https://claude.ai/artifact/YGA7unwqztvuA26CMtdbv8.

## In short

- **“Relay” symbol**: a square cut in two by a 45° seam. Top left the computer, bottom right you, between the two the link. No letter: it survives a name change.
- **Three products, three states**: **KyberNow Link** (the Mac, Apple TV, iPhone, iPad and Windows apps, and the ecosystem: state `now`, the lower half in signal colour), **KyberNow Station** (the server on the Windows computer: state `server`, the upper half), **KyberNow Admin** (the Station's administration in the browser: state `console`, the seam alone). The state names stay those of the files and the code.
- **Signature**: in logos, the product signs under “KyberNow” in Martian Mono capitals, on a tag with its corner cut at 45° like the seam, right-aligned on the foot of the “w”. Below 32 px high, the symbol and the name only.
- **Name**: Archivo width 125, “Kyber” at weight 420, “Now” at 720, outlined.
- **Colours**: ink `#05070A`, frost `#E3EDF0`, Spectre signal `#9A8CFF` (flat `#6D5BFF`, on a light background `#5340E6`). The Frost and Tally directions were set aside; `generate.mjs` can still produce them.
- **Fonts**: Archivo (titles at width 125, text at width 100) and Martian Mono (data). OFL licences in `fonts/`.
- **Light**: the signal is also a neon tube (core `#F4F1FF`, sheath `#9A8CFF`, halo `#6D5BFF`). Off at rest; standby on hover, flash on click, lit while a connection, a stream or a live measurement is in progress, a one-second ignition when a screen appears. Never in front of the image during a session.
- **Icons**: Phosphor (MIT). Regular at 20 px, Bold at 16 px and below, Fill for the active state.

## Files

| Folder | Contents |
|---|---|
| `mark/` | Symbol, three states, dark and light background; `*-lit-dark.svg`: lit (neon) |
| `logo/` | Horizontal, vertical, name only; signed logos `kybernow-{station,admin,link}-{dark,light}.svg` |
| `app-icon/` | 1024 icons (macOS template): Station, Admin, Link, neon seam |
| `favicon/` | Favicon that follows the browser's theme |
| `tokens/` | `kybernow.css` and `Theme+Brand.swift` (colours, shapes, motion, neon: `.neon(_:)`, `.neonIgnition(trigger:)`, `.knPrimary`) |
| `fonts/` | Archivo and Martian Mono, variable |
| `charte/` | The guidelines (standalone page) |
| `tools/` | Generation scripts |

## Regenerating

Do not edit the SVGs by hand.

```sh
# SVG (Spectre par défaut)
node brand/v2/tools/generate.mjs

# Charte : il faut les icônes Phosphor (npm i @phosphor-icons/core@2.1.1 dans un dossier de travail)
node brand/v2/tools/build-charte.mjs <…>/node_modules/@phosphor-icons/core/assets

# Nom vectorisé, si la police ou les réglages changent (fontTools requis)
python3 brand/v2/tools/outline.py '{"kyber":{"text":"Kyber","wght":420,"wdth":125,"tracking":-1.2},"now":{"text":"Now","wght":720,"wdth":125,"tracking":-1.2}}' > brand/v2/tools/wordmark.json
# Signatures des produits (Martian Mono)
python3 brand/v2/tools/outline.py '{"station":{"text":"STATION","wght":540,"wdth":100,"tracking":8,"src":"brand/v2/fonts/MartianMono-wdth-wght.ttf"},"admin":{"text":"ADMIN","wght":540,"wdth":100,"tracking":8,"src":"brand/v2/fonts/MartianMono-wdth-wght.ttf"},"link":{"text":"LINK","wght":540,"wdth":100,"tracking":8,"src":"brand/v2/fonts/MartianMono-wdth-wght.ttf"}}' > brand/v2/tools/signature.json
```

(The first command generates the SVGs, Spectre by default. The guidelines need the Phosphor icons: `npm i @phosphor-icons/core@2.1.1` in a working folder. The outlined name and the product signatures only need regenerating if the font or the settings change; fontTools required.)

The symbol's geometry is in `tools/generate.mjs` (grid of 48, seam of 4, radii of 7 and 1.2); `tools/geometry.json` is its output (with the layout of the signed logos), used by the guidelines and the 3D scene.

## To decide before release

The name: “KyberNow” contains a third party's trademark. See section 14 of the guidelines (“Rapport à Kyber”, the relationship with Kyber).
