# KyberNow

Serveur de streaming Windows clé en main et clients macOS / tvOS, construits sur [Kyber](https://kyber.tech). Développé par [Les Lignes Claires](https://www.leslignesclaires.fr).

| Dossier | Contenu |
|---|---|
| `PROJECT.md` | Cadrage et décisions |
| `apps/macos/` | App macOS (SwiftUI, VideoToolbox, Metal, Core Audio), voir [son README](apps/macos/README.md) |
| `server/` | Tout ce qui tourne sur l'ordinateur partagé, voir [son README](server/README.md) |
| `server/admin/`, `server/web/` | Console d'administration (service Windows, Rust) et son interface (Svelte) |
| `server/tray/` | Icône de la zone de notification |
| `server/installer/` | Installateur `KyberNow-Setup.exe` (Rust + WebView2 + Svelte), voir [son README](server/installer/README.md) |
| `server/kyber-patches/`, `apps/macos/kyber-patches/` | Correctifs de Kyber, décrits dans [docs/kyber-changes.md](docs/kyber-changes.md) |
| `brand/` | Logo et déclinaisons |
| `docs/` | Changements de Kyber, notes de compilation, prototype de référence |
| `tools/latency/` | Mesure de latence |

Licence : AGPL-3.0-or-later.
