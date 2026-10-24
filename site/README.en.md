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
| `/` | `/en/` | Home, product side: presentation video, the three apps with their screens, Link by use and the Raspberry Pi, headline features, proof it is a real product, three steps, downloads |
| `/technologie/` | `/en/technology/` | The path of an image (9.4 ms, pinned section), QUIC (simulation), Kyber, the comparison with Sunshine + Moonlight, features and profiles |
| `/guide/` | `/en/guide/` | Step-by-step guide (KyberNow Station, then devices with KyberNow Link: Mac, Windows, Linux, Apple TV, iPhone and iPad, Android, LG TV, Steam Deck and SteamOS, Raspberry Pi, browser, each behind its tile; VPN, updates, uninstalling) and FAQ |
| `/mentions-legales/` | `/en/legal/` | Publisher, hosting, intellectual property, privacy, credits |

Each page declares its two versions (`<link rel="alternate" hreflang>`). `assets/lang.js`, loaded in `<head>`, sends a visitor whose browser is not in French to the English version (and the other way round), unless they have chosen a language with the selector: that choice is kept in `localStorage` (`kn-lang`). Robots are never redirected. The texts produced by `main.js` (path, simulation, profiles) follow `<html lang>`.

Both languages are written by hand: a text change is carried over to both files.

## Products and logos

KyberNow is the brand; it covers three products, named as in the rest of the repository:

- **KyberNow Station**: the software installed on the Windows computer that streams the image (`KyberNow-Station-Setup.exe`). In the texts, “the Station” after the first mention; “the computer” stays when talking about the machine itself.
- **KyberNow Admin**: the Station's administration in the browser (“Admin”, with no article, after the first mention).
- **KyberNow Link**: the apps that display the image (Mac, Windows, Linux and Steam Deck, Apple TV, iPhone and iPad, Android, LG TVs) and their `/ecran` web version (`KyberNow-Link-Setup.exe` for Windows, `KyberNow-Link-macOS.zip` for the Mac, `KyberNow-Link-Android.apk` for Android, `KyberNow-Link-webOS.ipk` for LG TVs, `KyberNow-Link-Linux-x86_64.flatpak` and `KyberNow-Link-Linux-aarch64.flatpak` for Linux and the Steam Deck).

The home page presents them in the “The KyberNow family” section (`#apps`, right after the video), each with its signed logo and its screen recreated in HTML (see “App screens”) (`assets/img/kybernow-{station,admin,link}-dark.svg`, copies from `brand/v2/logo/`). The download cards and the guide's “Before you start” box reuse the Station and Link logos. A signed logo never goes below 32 px high (below that, the signature is no longer legible); the site only has a dark theme, so only the `-dark` versions are copied.

## Home page: video, app screens, proof

The home page talks product, the technology lives on `/technologie/`. In order: hero (the Mac app and the iPhone behind the text, 3D symbol set back), **the presentation** (`#presentation`), **the three apps** (`#apps`), **Link, by use** (`#appareils`, then the Raspberry Pi), **headline features** (`#fonctions`: the screen's resolution and refresh rate, 7.1 sound, variable refresh rate, clipboard and 4:4:4, every gamepad, encryption; then “the computer becomes a console”, the app launched at the start of the session depending on the device, and built-in gamepad navigation), **already in your hands** (`#reel`: latest version, number of versions, measured latency, licence, signatures, updates, community), three steps and technology in short (`#comment`), downloads (`#telecharger`, compact three-column grid, main button according to the device).

- **Presentation video** (motion design with music): `<video muted playsinline loop preload="none">` in `.video`. **Muted until clicked**: the video starts muted (the only way browsers allow autoplay); a click on the video, the sound button (`data-video="sound"`, speaker crossed out or not) or the big play button turns the sound on; after that, a click on the video pauses or resumes. The state shows at the bottom left (“Muted · click to hear it”, “Sound on”, “Paused”). **Quality follows bandwidth**: HLS at 720p, 1080p and 4K, 60 fps (`assets/video/hls/presentation.m3u8` and one folder per quality, 4 s fMP4 segments), played by hls.js (`vendor/hls/hls.light.min.js` 1.5.20, loaded as the video approaches, capped to the player size: 1080p for an 860 px player on a Retina display, 4K in full screen), or by Safari's native HLS when MSE is missing; with neither, `assets/video/presentation-720p.mp4`. All of it (≈ 170 MB) is rebuilt with `scripts/site-video.sh MASTER.mp4` from the 4K 60 fps master (AAC 160 kbit/s stereo sound in each quality). Poster `assets/video/presentation-poster.jpg`, rendered by `scripts/site-seo.py images` from `scripts/site-og/video-poster.html`. `main.js` plays it when it comes on screen (45% visible) and pauses it when it leaves; pause, replay and full-screen buttons; with “Reduce motion”, nothing starts on its own. The hero's “Watch the presentation” button scrolls to it.
- **App screens**: recreated in HTML and CSS (`assets/screens.css`, `.sc-` prefix) from the apps' code (Admin and installer strings, the Mac home screen, the LG TV and Android screenshots), never screenshots: sharp at any size, nothing to redo with each version. Each screen is drawn at its real size (`--w`, `--h` on `.shot`) and `main.js` scales it down to its card's width (`--s`, `transform: scale`). Decorative: `aria-hidden`, described by the `.shot` element's `aria-label`. Screens: Link for Mac (home), Link on iPhone, Link in the living room (TV), Link on the Steam Deck (Gaming Mode), Link in a session (measurement bar), KyberNow Admin (overview), the KyberNow Station installer (Welcome). The data is that of an example computer, “Atelier” (RTX 3080 Ti, the one used for the measurement). Written by hand in both languages, like the rest.
- **Proof**: the figures (latest version, date, number of versions) are written in the page and updated by `main.js` from the GitHub API (`releases?per_page=100`); redo them by hand now and then so the fallback stays right.
- **3D scene**: kept, discreet. States `hero-soft`, `edge-l`, `edge-r` (symbol at the edge of the screen, small, unlit) and `assemble-soft` (download) in `scene.js`; the rich states (`flow` for the path, `side`…) stay on `/technologie/`.
- **Link, by use** (`#appareils`): three cards, **in the living room** (TV: Apple TV, Android TV, LG, Raspberry Pi, SteamOS), **in your hands** (iPhone and iPad, Android, Steam Deck) and **at the desk** (Mac, Windows, Linux, browser), each with its screen, what you do there and its devices (`.uses`, `.use`). The gamepad is common to every app: it has its card in the headline features, not under one platform.
- **Raspberry Pi and KyberNow Link OS**: below the three cards, a block (`.pi-feature`) with two real captures of the OS, `assets/img/pi/`: start-up, captured under QEMU emulation (`apps/linux/os/emulate.sh`, `qmp.py shot`), and the living-room home, rendered by the app itself (`kybernow --preview`, `os-10b-accueil-a-jour.png`, sample computers; the emulator reaches no PC), in French (`link-os-accueil.webp`) and in English (`link-os-home.webp`); the message: a box for the TV that boots into Link and updates itself. Download card (`data-os="pi"`, direct links to `KyberNow-Link-OS.img.xz` and the `.deb`) and guide tile (`/en/guide/#raspberry-pi`). `site/updates/rpi-imager.json` is written by `scripts/release.sh`.

## Beta

KyberNow is at version 0.x, and the site says so throughout: a “Beta” badge next to the logo on every page and at the top of the home page (`.beta` class), a “KyberNow is in beta” box in the Download section (`.notice-beta`, `flask` icon), a badge after the version number, and a “Why beta?” FAQ question, in both languages.

The KyberNow Link app for Apple TV has no file to download: its card leads to the “On Apple TV” section of the guide (`/en/guide/#apple-tv`), which sums up [apps/tvos/SIDELOAD.en.md](../apps/tvos/SIDELOAD.en.md) (installation from a Mac with Xcode). Likewise for the Link app for iPhone and iPad: card to “On iPhone and iPad” (`/en/guide/#iphone`), which sums up [apps/ios/SIDELOAD.en.md](../apps/ios/SIDELOAD.en.md). The Android app (Android 11 or later: phone, tablet, TV) downloads as an APK; its card also leads to the guide (`/en/guide/#android`) for installing on a TV, with `adb` from a computer. `#android-tv`, the anchor of the first links, opens the same section. The LG TV app (webOS 5 or later) installs in developer mode from a computer: its card leads to “On an LG TV” (`/en/guide/#lg`), which sums up [apps/webos/INSTALLATION.en.md](../apps/webos/INSTALLATION.en.md); a visitor whose browser is an LG TV's (`Web0S` in the user agent) sees that card highlighted. The Linux app downloads as a Flatpak (`x86_64`, and `aarch64` as a second link); its card also leads to “On the Steam Deck and SteamOS” (`/en/guide/#steam-deck`). The guide has two tiles, “Linux” (`#linux`, the desktop) and “Steam Deck” (`#steam-deck`: Desktop Mode, adding to Steam, Gaming Mode, living room), which sum up [apps/linux/INSTALLATION.en.md](../apps/linux/INSTALLATION.en.md). A browser on Linux highlights the card; in the guide, Steam's (`Valve Steam` in the user agent) opens the Steam Deck tile, Raspberry Pi OS's Chromium (`Raspberry Pi` or `aarch64` in the user agent) the Raspberry Pi one (`#raspberry-pi`: KyberNow Link OS, then the app as a `.deb`), others the Linux one.

**Connecting a device** (guide): one tile per system (`.os-tiles`, `tablist` role), its installation below (`.os-panel`). `main.js` opens the one named by the anchor (`#mac`, `#android`…, existing links included), otherwise the visitor's device; arrow keys move between tiles. Without JavaScript, every installation stays one after the other. A new system: a tile and a panel in both languages. A last tile alone on its row takes the full width (`doc.css`).

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
- **Share images**: `assets/og/kybernow-fr.jpg` and `kybernow-en.jpg`, laid out in HTML in `scripts/site-og/card.html` (the site's fonts, logo and lit symbol) and rendered by `scripts/site-seo.py images` with headless Chrome (`CHROME=<path>` for another Chromium); the same script renders the video poster and the iPhone and iPad icon (`scripts/site-og/touch-icon.html` → `apple-touch-icon.png`). Redo them when the message, the measured latency or the platforms change.
- **Structured data** (JSON-LD): on the home page, the “KyberNow” `SoftwareApplication` (category, systems, free of charge, licence, download, features, `subjectOf` the Technology page, `isBasedOn` Kyber) with its parts `#station` (KyberNow Station, Windows, which contains the KyberNow Admin `WebApplication`) and `#link` (KyberNow Link: macOS, Windows, tvOS, iOS, iPadOS, Android, webOS, Linux, SteamOS, Raspberry Pi OS, KyberNow Link OS, browser), `WebSite` and `Organization` (Les Lignes Claires), written by hand in both languages; on the guides, `FAQPage`, produced by `scripts/site-seo.py faq` from the visible questions. **Run `scripts/site-seo.py faq` again after any change to the FAQ.**
- **Robots**: `robots.txt` opens everything, AI assistants included, and points to `sitemap.xml` (home page, technology and guide, with their language versions; move the `lastmod` date forward when the text changes). `llms.txt` sums KyberNow up for AI assistants (in English, with the measured facts and the comparison): keep it in line with the home page.
- **Texts**: the Compare section (on `/technologie/`, “Compare” entry in the navigation) is titled “KyberNow, the alternative to Sunshine + Moonlight”; the FAQ answers “can it replace Sunshine and Moonlight”, “is it cloud gaming”, “playing PC games on a Mac or an Apple TV”. `lang.js` never redirects a robot.

## Community

KyberNow Discord server, permanent invitation: https://discord.gg/sVy2mu5WdT (opens #bienvenue). Links in the navigation, the mobile menu, the download section, the guide and the footer.

## Contents

| File | Role |
|---|---|
| `index.html`, `technologie/`, `guide/`, `mentions-legales/`, `en/` | The pages (see above) |
| `assets/lang.js` | Language choice (detection, remembered preference) |
| `assets/doc.css` | Layout of the reading pages (contents, steps, FAQ, legal notice) |
| `assets/kybernow.css` | v2 guideline tokens (copy of `brand/v2/tokens/kybernow.css`) |
| `assets/site.css` | Layout |
| `assets/main.js` | Reveals on scroll, mobile menu, presentation video, scaling of the app screens, pinned section for the path (counter, steps), QUIC / single-stream simulation, profiles, main button according to the visitor's system, version and number of versions read from GitHub; texts in French or English according to the page |
| `assets/screens.css` | The app screens recreated in HTML (see “Home page”) |
| `assets/video/` | Presentation video: `hls/` (720p, 1080p, 4K), `presentation-720p.mp4` fallback, poster; rebuilt by `scripts/site-video.sh` |
| `assets/scene.js` | 3D scene (Three.js): “Relay” symbol extruded from the geometry in `brand/v2/tools/geometry.json`, neon seam (bloom), particle flow; one state per section (`data-scene`) |
| `assets/icons.svg` | Phosphor 2.1.1 icons (MIT, `assets/PHOSPHOR-LICENSE`) as a sprite, plus a few simple paths in the same style (play, pause, full screen, gear, plus…) for the home page |
| `assets/og/` | Share images, French and English (see “Search and sharing”) |
| `apple-touch-icon.png`, `robots.txt`, `sitemap.xml`, `llms.txt` | iPhone and iPad icon, robots, sitemap, summary for AI assistants |
| `assets/img/` | Copies from `brand/v2/`: logo (`kybernow-logo-dark.svg`), signed logos of the three products (`kybernow-{station,admin,link}-dark.svg`), symbols (`kybernow-mark-now-dark.svg` for the iPhone and iPad icon, `kybernow-mark-now-lit-dark.svg` without WebGL and for the share image); `pi/`, the KyberNow Link OS captures (see “Home page”) |
| `fonts/` | Archivo and Martian Mono (OFL) |
| `vendor/three/` | Three.js r170 (MIT): `three.module.min.js` and the few example modules used (bloom, SVGLoader, RoomEnvironment) |
| `vendor/hls/` | hls.js 1.5.20 (Apache-2.0), light build: HLS playback of the presentation video outside Safari |
| `updates/appcast.xml` | Update feed of the Mac app (see "Updating") |
| `updates/notes/X.Y.Z/` | French notes of each release, the per-language link of the Mac app's feed (`KyberNow-Link-macOS.fr.html`, `KyberNow-macOS.fr.html` up to 0.4.2), written by `scripts/release.sh` |
| `updates/rpi-imager.json` | Raspberry Pi Imager repository for KyberNow Link OS (name, image of the latest release, sizes and checksums, Pi 3, 4 and 5 boards in 64-bit), written by `scripts/release.sh` (`apps/linux/os/imager-json.sh`) |

Nothing is loaded from a CDN. The only external request: the GitHub API, to show the latest release and the file sizes (the static text stays if it fails). The size is given the way the system will show it once the file is downloaded: in units of 1,048,576 bytes, truncated, for the `.exe` files (Windows), in millions of bytes for the rest (macOS), to one decimal; the fallback values in `index.html` need redoing whenever a release changes size noticeably.

## Guideline rules applied

- Neon lit only in active moments: one-second ignition on arrival, image stream, path measurement, re-ignition on download. Off in the reading sections.
- On a phone, the 3D symbol moves to the top of the screen, smaller (`MOBILE` in `scene.js`), so it never goes under the text.
- The app screens and the video sit in front of the symbol, never the other way round.
- A single main button per view; in the download section, it follows the visitor's system (Mac: the KyberNow Link app, Windows: the KyberNow Station installer, tablet, phone and everything else: Link in the browser, `/ecran`).
- Resolution and refresh rate: the site talks about “your display's resolution and refresh rate” and gives examples (4K at 120 Hz, 5K, 6K, 240 Hz), stating that the only limit is the graphics card's encoder. The latency figures are those of a measurement, presented as such.
- “Reduce motion”: the symbol stays in place, no flicker or breathing, reveals as a simple fade. “Reduce transparency”: opaque surfaces, no halo.
- Without WebGL: the lit symbol as SVG instead of the scene.

## Updating

- Path figures (9.4 ms median latency, prototype A measurement; dated record in `docs/archives/prototype-A-results.md`): `#trajet` section of `technologie/index.html`, total in `TOTAL` in `main.js`, and the home page hero.
- Presentation video: `scripts/site-video.sh MASTER.mp4` for each new master (16:9, 4K at 60 fps); the “ninety seconds” duration is written in the section title (the poster carries none).
- Download links: `releases/latest/download/<file>` on GitHub, nothing to change for each release. A link to a new file name only reaches `main` once the release that contains that file is published (see [docs/releases.en.md](../docs/releases.en.md), “Old installer names”).
- `updates/appcast.xml`: update feed of the Mac app (Sparkle), every release, betas included. Written and committed by `scripts/release.sh` on each release, rebuilt if needed by `scripts/rebuild-appcast.sh`; do not edit it by hand (see [docs/releases.en.md](../docs/releases.en.md), "Mac app update feed"). Its address is compiled into the app: do not move it.
- Three.js: replace the files in `vendor/three/` with those of the same version on unpkg (`three@<version>/build/three.module.min.js`, `examples/jsm/...`).
