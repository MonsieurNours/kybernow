# KyberNow logo

English · [Français](README.md)

> Previous logo, kept as an archive: neither the apps, nor Admin, nor the installers, nor the website use it. The identity in force is [`v2/`](v2/README.en.md) (“Relay” symbol, Spectre signal).

The K as a folded ribbon, crossed by three signal lines that pulse in its opening: the image leaving the computer and reaching the screen.

## Files

| File | Use |
|---|---|
| `kybernow-logo.svg` | Vertical logo (symbol + name), dark background |
| `kybernow-logo-light.svg` | Vertical logo, light background |
| `kybernow-logo-horizontal.svg` | Horizontal logo, dark background |
| `kybernow-logo-horizontal-light.svg` | Horizontal logo, light background |
| `kybernow-mark.svg` / `kybernow-mark-light.svg` | Symbol alone |
| `kybernow-app-icon.svg` | App icon (rounded night background) |
| `kybernow-app-icon-1024.png` | The same, as a 1024 × 1024 PNG |
| `kybernow-mark-animated.svg` | Animation demo |
| `preview.html` | Sheet of every variant |

The name is outlined (Bricolage Grotesque, weight 700): no file depends on an installed font.

## Colours

| Role | Dark background | Light background |
|---|---|---|
| K, “Kyber” | `#F4F7FA` | `#151B23` |
| Signal, “Now” | `#6FE3C1` (mint) | `#12A37E` |
| Arm shadow | `#9AA6B4` → `#DCE3EA` | `#5B6878` → `#2A3441` |
| Ribbon folds | `#B6C1CD` | `#46525F` |
| Icon background | `#0D1117` | — |

## Structure for animation

Each piece is a separate, named element, with no intermediate transform:

```
#kn-mark
├── #kn-signal              signal lines (drawn behind the K)
│   ├── #kn-wave-1  .kn-wave   pathLength="1000"
│   ├── #kn-wave-2  .kn-wave   pathLength="1000"  (main line)
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

- The lines have `pathLength="1000"`: they can be drawn, or a pulse sent along them, with `stroke-dasharray` and `stroke-dashoffset`, whatever their real length.
- The pieces of the K animate individually with `transform-box: fill-box; transform-origin: center`.
- Always provide `@media (prefers-reduced-motion: reduce)`.

Example (taken from `kybernow-mark-animated.svg`):

```css
.kn-wave { stroke-dasharray: 1000; stroke-dashoffset: 1000; animation: kn-draw 1.6s forwards; }
@keyframes kn-draw { to { stroke-dashoffset: 0 } }

/* impulsion qui parcourt la ligne principale */
.kn-pulse { stroke-dasharray: 60 940; animation: kn-travel 2.4s linear infinite; }
@keyframes kn-travel { from { stroke-dashoffset: 1000 } to { stroke-dashoffset: 0 } }
```

(The comment, in French, reads “pulse travelling along the main line”.)

To embed it in a page and animate it, include the SVG inline (not as an `<img>`), so that the page's CSS reaches its elements.

## Regenerating

The script that produces these files from the symbol's geometry and the font is not in the repository: a hand edit has to be carried over to every variant.
