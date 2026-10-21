# KyberNow

English · [Français](README.md)

Ready-to-install Windows streaming server and macOS, Windows and tvOS clients, built on [Kyber](https://kyber.tech). Developed by [Les Lignes Claires](https://www.leslignesclaires.fr).

| Folder | Contents |
|---|---|
| `PROJECT.md` | Scope and decisions (in English: [PROJECT.en.md](PROJECT.en.md)) |
| `apps/macos/` | macOS app (SwiftUI, VideoToolbox, Metal, Core Audio), see [its README](apps/macos/README.en.md) |
| `apps/core/`, `apps/windows/` | Rust client core (session, console, profiles, latency, updates, shared inputs and sound, see [its README](apps/core/README.en.md)) and Windows app (Direct3D 11, FFmpeg, WASAPI), with its installer `KyberNow-Client-Setup.exe` (`apps/windows/setup`, x64 and ARM64), see [its README](apps/windows/README.en.md) |
| `server/` | Everything that runs on the shared computer, see [its README](server/README.en.md) |
| `server/admin/`, `server/web/` | Administration console (Windows service, Rust) and its interface (Svelte) |
| `server/tray/` | Notification area icon |
| `server/installer/` | `KyberNow-Server-Setup.exe` installer (Rust + WebView2 + Svelte), see [its README](server/installer/README.en.md) |
| `server/kyber-patches/`, `apps/macos/kyber-patches/` | Patches to Kyber, described in [docs/kyber-changes.en.md](docs/kyber-changes.en.md) |
| `site/` | www.kybernow.app website (static, no npm), see [its README](site/README.en.md) |
| `brand/` | Logo and variants; v2 identity in `brand/v2/` (guidelines, symbol, tokens) |
| `docs/` | Changes to Kyber, build notes, reference prototype, the 20 October 2026 audit and [measurements to run on the hardware](docs/mesures.en.md) |
| `tools/latency/` | Latency measurement |

Documentation exists in French (`*.md`, the source) and in English (`*.en.md`, next to each French file). Every change updates both versions in the same commit.

KyberNow uses [Kyber](https://kyber.tech), a free, open-source streaming engine (AGPL-3.0) built by VLC developers. KyberNow is a community, non-commercial project, developed by Les Lignes Claires; it is neither published nor endorsed by the Kyber team, and is not affiliated with it.

Licence: AGPL-3.0-or-later.
