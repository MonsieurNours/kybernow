# KyberNow

[English](README.en.md) · Français

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
| `site/` | Site kybernow.app (statique, sans npm), voir [son README](site/README.md) |
| `brand/` | Logo et déclinaisons ; identité v2 dans `brand/v2/` (charte, symbole, jetons) |
| `docs/` | Changements de Kyber, notes de compilation, prototype de référence |
| `tools/latency/` | Mesure de latence |

La documentation existe en français (`*.md`, la source) et en anglais (`*.en.md`, à côté de chaque fichier français). Chaque évolution met à jour les deux versions dans le même commit.

KyberNow utilise [Kyber](https://kyber.tech), un moteur de streaming libre (AGPL-3.0) conçu par des développeurs de VLC. KyberNow est un projet communautaire et non commercial, développé par Les Lignes Claires ; il n'est ni édité, ni approuvé par l'équipe de Kyber, et n'y est pas affilié.

Licence : AGPL-3.0-or-later.
