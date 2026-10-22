# KyberNow

[English](README.en.md) · Français

Serveur de streaming Windows clé en main et clients macOS, tvOS, iOS/iPadOS et Windows, construits sur [Kyber](https://kyber.tech). Développé par [Les Lignes Claires](https://www.leslignesclaires.fr).

KyberNow réunit trois produits :

- **KyberNow Station** : le serveur, installé sur l'ordinateur Windows qui diffuse son image (installateur `KyberNow-Station-Setup.exe`, services, icône de la zone de notification, mises à jour) ;
- **KyberNow Admin** : l'administration de la Station, dans le navigateur ;
- **KyberNow Link** : les apps qui affichent l'image (Mac, Apple TV, iPhone et iPad, Windows avec `KyberNow-Link-Setup.exe`) et le client web servi par Admin.

| Dossier | Contenu |
|---|---|
| `PROJECT.md` | Cadrage et décisions |
| `apps/macos/` | KyberNow Link pour Mac (SwiftUI, VideoToolbox, Metal, Core Audio) sur le cœur Rust, voir [son README](apps/macos/README.md) |
| `apps/apple/` | Pont C entre le cœur Rust et les apps Apple (`libkybernow`), voir [docs/coeur-apple.md](docs/coeur-apple.md) |
| `apps/tvos/`, `apps/ios/` | KyberNow Link pour Apple TV et pour iPhone/iPad, voir leurs README ([Apple TV](apps/tvos/README.md), [iPhone et iPad](apps/ios/README.md)) |
| `apps/core/`, `apps/windows/` | Cœur client en Rust des apps KyberNow Link (session, dialogue avec Admin, profils, latence, mises à jour, entrées et son partagés, voir [son README](apps/core/README.md)) et KyberNow Link pour Windows (Direct3D 11, FFmpeg, WASAPI), avec son installateur `KyberNow-Link-Setup.exe` (`apps/windows/setup`, x64 et ARM64), voir [son README](apps/windows/README.md) |
| `server/` | KyberNow Station : tout ce qui tourne sur l'ordinateur qui diffuse son image, voir [son README](server/README.md) |
| `server/admin/`, `server/web/` | KyberNow Admin (service Windows, Rust) et son interface (Svelte), qui sert aussi le client web `/ecran` |
| `server/tray/` | Icône de la Station dans la zone de notification |
| `server/installer/` | Installateur `KyberNow-Station-Setup.exe` (Rust + WebView2 + Svelte), voir [son README](server/installer/README.md) |
| `server/kyber-patches/`, `apps/macos/kyber-patches/` | Correctifs de Kyber, décrits dans [docs/kyber-changes.md](docs/kyber-changes.md) |
| `site/` | Site www.kybernow.app (statique, sans npm), voir [son README](site/README.md) |
| `brand/v2/` | Identité visuelle : charte, symbole, logos, jetons, voir [son README](brand/v2/README.md) ; `brand/` garde le logo précédent, inutilisé |
| `docs/` | Changements de Kyber, notes de compilation, prototype de référence, audit du 20 octobre 2026 et [mesures à faire sur le matériel](docs/mesures.md) |
| `tools/latency/` | Mesure de latence |

La documentation existe en français (`*.md`, la source) et en anglais (`*.en.md`, à côté de chaque fichier français) ; chaque évolution met à jour les deux dans le même commit. Elle décrit le projet tel qu'il est ; l'historique est dans `CHANGELOG.md` et dans git. Règles d'écriture : [docs/i18n.md](docs/i18n.md#documentation).

KyberNow utilise [Kyber](https://kyber.tech), un moteur de streaming libre (AGPL-3.0) conçu par des développeurs de VLC. KyberNow est un projet communautaire et non commercial, développé par Les Lignes Claires ; il n'est ni édité, ni approuvé par l'équipe de Kyber, et n'y est pas affilié.

Licence : AGPL-3.0-or-later.
