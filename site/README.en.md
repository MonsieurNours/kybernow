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
| `/` | `/en/` | Presentation, the KyberNow family (Station, Admin, Link) and downloads |
| `/guide/` | `/en/guide/` | Step-by-step guide (KyberNow Station, then devices with KyberNow Link: Mac, Windows, Apple TV, iPhone and iPad, Android TV, browser, each behind its tile; VPN, updates, uninstalling) and FAQ |
| `/mentions-legales/` | `/en/legal/` | Publisher, hosting, intellectual property, privacy, credits |

Each page declares its two versions (`<link rel="alternate" hreflang>`). `assets/lang.js`, loaded in `<head>`, sends a visitor whose browser is not in French to the English version (and the other way round), unless they have chosen a language with the selector: that choice is kept in `localStorage` (`kn-lang`). Robots are never redirected. The texts produced by `main.js` (path, simulation, profiles) follow `<html lang>`.

Both languages are written by hand: a text change is carried over to both files.

## Products and logos

KyberNow is the brand; it covers three products, named as in the rest of the repository:

- **KyberNow Station**: the software installed on the Windows computer that streams the image (`KyberNow-Station-Setup.exe`). In the texts, “the Station” after the first mention; “the computer” stays when talking about the machine itself.
- **KyberNow Admin**: the Station's administration in the browser (“Admin”, with no article, after the first mention).
- **KyberNow Link**: the apps that display the image (Mac, Windows, Apple TV, iPhone and iPad, Android TV) and their `/ecran` web version (`KyberNow-Link-Setup.exe` for Windows, `KyberNow-Link-macOS.zip` for the Mac, `KyberNow-Link-Android.apk` for Android TV).

The home page presents them in the “The KyberNow family” section (`#famille`, right after “Why”), each with its signed logo (`assets/img/kybernow-{station,admin,link}-dark.svg`, copies from `brand/v2/logo/`). The download cards and the guide's “Before you start” box reuse the Station and Link logos. A signed logo never goes below 32 px high (below that, the signature is no longer legible); the site only has a dark theme, so only the `-dark` versions are copied.

## Beta

Until KyberNow reaches 1.0, the site says so throughout: a “Beta” badge next to the logo on every page and at the top of the home page (`.beta` class), a “KyberNow is in beta” box in the Download section (`.notice-beta`, `flask` icon), a badge after the version number, and a “Why beta?” FAQ question. At 1.0, remove these elements in both languages.

The KyberNow Link app for Apple TV has no file to download: its card leads to the “On Apple TV” section of the guide (`/en/guide/#apple-tv`), which sums up [apps/tvos/SIDELOAD.en.md](../apps/tvos/SIDELOAD.en.md) (installation from a Mac with Xcode). Likewise for the Link app for iPhone and iPad: card to “On iPhone and iPad” (`/en/guide/#iphone`), which sums up [apps/ios/SIDELOAD.en.md](../apps/ios/SIDELOAD.en.md). To be revisited once the apps are on TestFlight or the App Store. The Android TV app has its APK, but is installed from a computer with `adb`: its card leads to the guide too (`/en/guide/#android-tv`). The guide only presents what has been tried (the NVIDIA Shield TV); Android phones and tablets stay “Planned”.

**Connecting a device** (guide): one tile per system (`.os-tiles`, `tablist` role), its installation below (`.os-panel`). `main.js` opens the one named by the anchor (`#mac`, `#android-tv`…, existing links included), otherwise the visitor's device; arrow keys move between tiles. Without JavaScript, every installation stays one after the other. A new system: a tile and a panel in both languages.

## Publishing

Hosted on GitHub Pages by the `.github/workflows/pages.yml` workflow: each push to `main` that touches `site/` publishes the folder as it is.

Set-up, once only:
1. GitHub › Settings › Pages › Source: “GitHub Actions”.
2. Verify the domain (account Settings › Pages › Add a domain › `kybernow.app`, TXT record at the registrar) before touching the DNS, so that no other account can take it.
3. DNS for `kybernow.app`: four `A` records (185.199.108.153, 185.199.109.153, 185.199.110.153, 185.199.111.153), the `AAAA` records 2606:50c0:8000::153 to 8003::153, and `www` as a `CNAME` to `monsieurnours.github.io`.
4. Settings › Pages › Custom domain: `www.kybernow.app` (the `A` records of `kybernow.app` make the bare domain redirect to `www`), then “Enforce HTTPS” as soon as the certificate is ready (up to 24 h; `.app` requires HTTPS).

## Search and sharing

Goal: a search for remote gaming, remote desktop, self-hosted cloud gaming or an alternative to Sunshine and Moonlight should lead to the site, in search engines as in AI assistants.

- **Platform-agnostic message**: presentation and sharing texts (image, descriptions, structured data, `llms.txt`) say “your computer on all your devices”, never a list of platforms. The actual platforms only appear where you install: the Download section and the guide.
- **Each page's head**: title and description with the searched words, canonical URL, full Open Graph (type, site name, language and alternate language, 1200 × 630 image with alt text) and a `summary_large_image` Twitter card; `apple-touch-icon.png` for the iPhone and iPad home screen. The legal notice stays `noindex`.
- **Share images**: `assets/og/kybernow-fr.jpg` and `kybernow-en.jpg`, laid out in HTML in `scripts/site-og/card.html` (the site's fonts, logo and lit symbol) and rendered by `scripts/site-seo.py images` with headless Chrome. Redo them when the message, the measured latency or the platforms change.
- **Structured data** (JSON-LD): on the home page, the “KyberNow” `SoftwareApplication` (category, systems, free of charge, licence, download, features, `isBasedOn` Kyber) with its parts `#station` (KyberNow Station, Windows, which contains the KyberNow Admin `WebApplication`) and `#link` (KyberNow Link, macOS, Windows, tvOS, iOS, iPadOS, browser), `WebSite` and `Organization` (Les Lignes Claires), written by hand in both languages; on the guides, `FAQPage`, produced by `scripts/site-seo.py faq` from the visible questions. **Run `scripts/site-seo.py faq` again after any change to the FAQ.**
- **Robots**: `robots.txt` opens everything, AI assistants included, and points to `sitemap.xml` (home page and guide, with their language versions; move the `lastmod` date forward when the text changes). `llms.txt` sums KyberNow up for AI assistants (in English, with the measured facts and the comparison): keep it in line with the home page.
- **Texts**: the Compare section is titled “KyberNow, the alternative to Sunshine + Moonlight”; the FAQ answers “can it replace Sunshine and Moonlight”, “is it cloud gaming”, “playing PC games on a Mac or an Apple TV”. `lang.js` never redirects a robot.

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
| `assets/og/` | Share images, French and English (see “Search and sharing”) |
| `apple-touch-icon.png`, `robots.txt`, `sitemap.xml`, `llms.txt` | iPhone and iPad icon, robots, sitemap, summary for AI assistants |
| `assets/img/` | Copies from `brand/v2/`: logo (`kybernow-logo-dark.svg`), signed logos of the three products (`kybernow-{station,admin,link}-dark.svg`), symbols (`kybernow-mark-now-dark.svg` for the iPhone and iPad icon, `kybernow-mark-now-lit-dark.svg` without WebGL and for the share image) |
| `fonts/` | Archivo and Martian Mono (OFL) |
| `vendor/three/` | Three.js r170 (MIT): `three.module.min.js` and the few example modules used (bloom, SVGLoader, RoomEnvironment) |
| `updates/appcast.xml` | Update feed of the Mac app (see "Updating") |

Nothing is loaded from a CDN. The only external request: the GitHub API, to show the latest release and the file sizes (the static text stays if it fails). The size is given the way the system will show it once the file is downloaded: in units of 1,048,576 bytes, truncated, for the `.exe` files (Windows), in millions of bytes for the rest (macOS), to one decimal; the fallback values in `index.html` need redoing whenever a release changes size noticeably.

## Guideline rules applied

- Neon lit only in active moments: one-second ignition on arrival, image stream, path measurement, re-ignition on download. Off in the reading sections.
- On a phone, the 3D symbol moves to the top of the screen, smaller (`MOBILE` in `scene.js`), so it never goes under the text.
- A single main button per view; in the download section, it follows the visitor's system (Mac: the KyberNow Link app, Windows: the KyberNow Station installer, tablet, phone and everything else: Link in the browser, `/ecran`).
- No resolution or refresh rate limit announced: the site talks about “your display's resolution and refresh rate”. The only numbers are those of a measurement, presented as such.
- “Reduce motion”: the symbol stays in place, no flicker or breathing, reveals as a simple fade. “Reduce transparency”: opaque surfaces, no halo.
- Without WebGL: the lit symbol as SVG instead of the scene.

## Updating

- Path figures: `docs/prototype-A-results.md` (`#trajet` section of `index.html`, total in `TOTAL` in `main.js`).
- Download links: `releases/latest/download/<fichier>` on GitHub, nothing to change for each release. A link to a new file name only reaches `main` once the release that contains that file is published (see [docs/releases.en.md](../docs/releases.en.md), “Old installer names”).
- `updates/appcast.xml`: update feed of the Mac app (Sparkle), every release, betas included. Written and committed by `scripts/release.sh` on each release, rebuilt if needed by `scripts/rebuild-appcast.sh`; do not edit it by hand (see [docs/releases.en.md](../docs/releases.en.md), "Mac app update feed"). Its address is compiled into the app: do not move it.
- Three.js: replace the files in `vendor/three/` with those of the same version on unpkg (`three@<version>/build/three.module.min.js`, `examples/jsm/...`).
