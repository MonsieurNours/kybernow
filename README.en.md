# KyberNow

English · [Français](README.md)

Your Windows computer on all your devices: a ready-to-install streaming server and native apps, built on [Kyber](https://kyber.tech). Developed by [Les Lignes Claires](https://www.leslignesclaires.fr).

KyberNow brings together three products:

- **KyberNow Station**: the server, installed on the Windows computer that streams its image (`KyberNow-Station-Setup.exe` installer, services, notification area icon, updates);
- **KyberNow Admin**: the Station's administration, in the browser;
- **KyberNow Link**: the apps that show the image on the devices, the web client served by Admin, and KyberNow Link OS, the system image that turns a Raspberry Pi into a Link device.

| Folder | Contents |
|---|---|
| `PROJECT.md` | Scope and decisions in force (in English: [PROJECT.en.md](PROJECT.en.md)) |
| `ROADMAP.md` | What is planned, and only there (in English: [ROADMAP.en.md](ROADMAP.en.md)) |
| `apps/macos/` | KyberNow Link for Mac (SwiftUI, VideoToolbox, Metal, Core Audio) on the Rust core, see [its README](apps/macos/README.en.md) |
| `apps/apple/` | C bridge between the Rust core and the Apple apps (`libkybernow`), see [docs/coeur-apple.en.md](docs/coeur-apple.en.md) |
| `apps/android/` | KyberNow Link for Android and Android TV (Kotlin, Compose, the core's JNI bridge in `apps/android/jni`), see [its README](apps/android/README.en.md) |
| `apps/webos/` | KyberNow Link for LG TVs (webOS): native app on the Rust core, picture through the TV's decoder (NDL), webOS SDL2; installed in developer mode, see [its README](apps/webos/README.en.md) and [its installation guide](apps/webos/INSTALLATION.en.md) |
| `apps/linux/` | KyberNow Link for Linux, the Steam Deck, SteamOS and the Raspberry Pi (Rust, SDL2, FFmpeg through VA-API or V4L2; Flatpak and .deb), see [its README](apps/linux/README.en.md) and [its installation guide](apps/linux/INSTALLATION.en.md); KyberNow Link OS in `apps/linux/os`, see [its README](apps/linux/os/README.en.md) |
| `apps/tvos/`, `apps/ios/` | KyberNow Link for Apple TV and for iPhone/iPad, see their READMEs ([Apple TV](apps/tvos/README.en.md), [iPhone and iPad](apps/ios/README.en.md)) |
| `apps/core/`, `apps/windows/` | Rust client core of the KyberNow Link apps (session, talking to Admin, profiles, latency, updates, shared inputs and sound, see [its README](apps/core/README.en.md)) and KyberNow Link for Windows (Direct3D 11, FFmpeg, WASAPI), with its installer `KyberNow-Link-Setup.exe` (`apps/windows/setup`, x64 and ARM64), see [its README](apps/windows/README.en.md) |
| `server/` | KyberNow Station: everything that runs on the computer that streams its image, see [its README](server/README.en.md) |
| `server/admin/`, `server/web/` | KyberNow Admin (Windows service, Rust) and its interface (Svelte), which also serves the `/ecran` web client |
| `server/tray/` | The Station's notification area icon |
| `server/installer/` | `KyberNow-Station-Setup.exe` installer (Rust + WebView2 + Svelte), see [its README](server/installer/README.en.md) |
| `server/kyber-patches/`, `apps/macos/kyber-patches/` | Patches to Kyber, described in [docs/kyber-changes.en.md](docs/kyber-changes.en.md) |
| `site/` | www.kybernow.app website (static, no npm), see [its README](site/README.en.md) |
| `brand/v2/` | Visual identity: guidelines, symbol, logos, tokens, see [its README](brand/v2/README.en.md); `brand/` keeps the previous logo, unused |
| `docs/` | [Changes to Kyber](docs/kyber-changes.en.md), [build notes](docs/build-notes.en.md), [publishing releases](docs/releases.en.md), [the Rust core in the Apple apps](docs/coeur-apple.en.md), [languages and writing rules](docs/i18n.en.md) |
| `docs/archives/` | Dated records, which do not describe the current state: reference prototype and its results (19 October 2026), audit and requested measurements (20 October 2026), proposals on HDR, Apple TV latency and app at start (22 October 2026; the audit and proposals are in French only) |
| `tools/latency/` | Latency measurement |

Documentation exists in French (`*.md`, the source) and in English (`*.en.md`, next to each French file); every change updates both in the same commit. It describes the project as it is; history lives in `CHANGELOG.md` and in git, planned work in `ROADMAP.md`. Writing rules: [docs/i18n.en.md](docs/i18n.en.md#documentation).

KyberNow uses [Kyber](https://kyber.tech), a free, open-source streaming engine (AGPL-3.0) built by VLC developers. KyberNow is a community, non-commercial project, developed by Les Lignes Claires; it is neither published nor endorsed by the Kyber team, and is not affiliated with it.

Licence: AGPL-3.0-or-later.
