# www.kybernow.app

English · [Français](README.md)

KyberNow's presentation and download website. Static, with no npm and no build step: the folder is served as it is.

```sh
python3 -m http.server 5200 --directory site
```

(or the `site` configuration in `.claude/launch.json`, which turns off the cache). Paths are absolute (`/assets/…`): the site must be served at the root of a domain, and `file://` does not work.

## Pages and languages

| French | English | Contents |
|---|---|---|
| `/` | `/en/` | Presentation and downloads |
| `/guide/` | `/en/guide/` | Step-by-step guide (server, devices, VPN, updates, uninstalling) and FAQ |
| `/mentions-legales/` | `/en/legal/` | Publisher, hosting, intellectual property, privacy, credits |

Each page declares its two versions (`<link rel="alternate" hreflang>`). `assets/lang.js`, loaded in `<head>`, sends a visitor whose browser is not in French to the English version (and the other way round), unless they have chosen a language with the selector: that choice is kept in `localStorage` (`kn-lang`). Robots are never redirected. The texts produced by `main.js` (path, simulation, profiles) follow `<html lang>`.

Both languages are written by hand: a text change is carried over to both files.

## Publishing

Hosted on GitHub Pages by the `.github/workflows/pages.yml` workflow: each push to `main` that touches `site/` publishes the folder as it is.

Set-up, once only:
1. GitHub › Settings › Pages › Source: “GitHub Actions”.
2. Verify the domain (account Settings › Pages › Add a domain › `kybernow.app`, TXT record at the registrar) before touching the DNS, so that no other account can take it.
3. DNS for `kybernow.app`: four `A` records (185.199.108.153, 185.199.109.153, 185.199.110.153, 185.199.111.153), the `AAAA` records 2606:50c0:8000::153 to 8003::153, and `www` as a `CNAME` to `monsieurnours.github.io`.
4. Settings › Pages › Custom domain: `www.kybernow.app` (the `A` records of `kybernow.app` make the bare domain redirect to `www`), then “Enforce HTTPS” as soon as the certificate is ready (up to 24 h; `.app` requires HTTPS).

## Community

KyberNow Discord server, permanent invitation: https://discord.gg/sVy2mu5WdT (opens #bienvenue). Links in the navigation, the mobile menu, the download section, the guide and the footer.

## Contents

| File | Role |
|---|---|
| `index.html`, `guide/`, `mentions-legales/`, `en/` | The pages (see above) |
| `assets/lang.js` | Language choice (detection, remembered preference) |
| `assets/doc.css` | Layout of the reading pages (contents, steps, FAQ, legal notice) |
| `assets/kybernow.css` | v2 guideline tokens (copy of `brand/v2/tokens/kybernow.css`) |
| `assets/site.css` | Layout |
| `assets/main.js` | Reveals on scroll, mobile menu, pinned section for the path (counter, steps), QUIC / single-stream simulation, profiles, main button according to the visitor's system, version read from GitHub; texts in French or English according to the page |
| `assets/scene.js` | 3D scene (Three.js): “Relay” symbol extruded from the geometry in `brand/v2/tools/geometry.json`, neon seam (bloom), particle flow; one state per section (`data-scene`) |
| `assets/icons.svg` | Phosphor 2.1.1 icons (MIT, `assets/PHOSPHOR-LICENSE`) as a sprite |
| `assets/img/` | Logo and symbols (copies from `brand/v2/`) |
| `fonts/` | Archivo and Martian Mono (OFL) |
| `vendor/three/` | Three.js r170 (MIT): `three.module.min.js` and the few example modules used (bloom, SVGLoader, RoomEnvironment) |
| `updates/appcast.xml` | Update feed of the Mac app (see "Updating") |

Nothing is loaded from a CDN. The only external request: the GitHub API, to show the latest release and the file sizes (the static text stays if it fails).

## Guideline rules applied

- Neon lit only in active moments: one-second ignition on arrival, image stream, path measurement, re-ignition on download. Off in the reading sections.
- On a phone, the 3D symbol moves to the top of the screen, smaller (`MOBILE` in `scene.js`), so it never goes under the text.
- A single main button per view; in the download section, it follows the visitor's system (Mac: the app, Windows: the installer, tablet, phone and everything else: the `/ecran` web client).
- No resolution or refresh rate limit announced: the site talks about “your display's resolution and refresh rate”. The only numbers are those of a measurement, presented as such.
- “Reduce motion”: the symbol no longer travels, no flicker or breathing, reveals as a simple fade. “Reduce transparency”: opaque surfaces, no halo.
- Without WebGL: the lit symbol as SVG instead of the scene.

## Updating

- Path figures: `docs/prototype-A-results.md` (`#trajet` section of `index.html`, total in `TOTAL` in `main.js`).
- Download links: `releases/latest/download/<fichier>` on GitHub, nothing to change for each release.
- `updates/appcast.xml`: update feed of the Mac app (Sparkle), every release, betas included. Written and committed by `scripts/release.sh` on each release, rebuilt if needed by `scripts/rebuild-appcast.sh`; do not edit it by hand (see [docs/releases.en.md](../docs/releases.en.md), "Mac app update feed"). Its address is compiled into the app: do not move it.
- Three.js: replace the files in `vendor/three/` with those of the same version on unpkg (`three@<version>/build/three.module.min.js`, `examples/jsm/...`).
