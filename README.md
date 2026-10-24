# KyberNow

[English](README.en.md) · Français

Votre ordinateur Windows sur tous vos appareils : un serveur de streaming clé en main et des apps natives, construits sur [Kyber](https://kyber.tech). Développé par [Les Lignes Claires](https://www.leslignesclaires.fr).

KyberNow réunit trois produits :

- **KyberNow Station** : le serveur, installé sur l'ordinateur Windows qui diffuse son image (installateur `KyberNow-Station-Setup.exe`, services, icône de la zone de notification, mises à jour) ;
- **KyberNow Admin** : l'administration de la Station, dans le navigateur ;
- **KyberNow Link** : les apps qui affichent l'image sur les appareils, le client web servi par Admin, et KyberNow Link OS, l'image système qui fait d'un Raspberry Pi un appareil Link.

| Dossier | Contenu |
|---|---|
| `PROJECT.md` | Cadrage et décisions en vigueur |
| `ROADMAP.md` | Ce qui est prévu, et seulement là |
| `apps/macos/` | KyberNow Link pour Mac (SwiftUI, VideoToolbox, Metal, Core Audio) sur le cœur Rust, voir [son README](apps/macos/README.md) |
| `apps/apple/` | Pont C entre le cœur Rust et les apps Apple (`libkybernow`), voir [docs/coeur-apple.md](docs/coeur-apple.md) |
| `apps/android/` | KyberNow Link pour Android et Android TV (Kotlin, Compose, pont JNI du cœur dans `apps/android/jni`), voir [son README](apps/android/README.md) |
| `apps/webos/` | KyberNow Link pour les téléviseurs LG (webOS) : app native sur le cœur Rust, image par le décodeur du téléviseur (NDL), SDL2 de webOS ; installation en mode développeur, voir [son README](apps/webos/README.md) et [son installation](apps/webos/INSTALLATION.md) |
| `apps/linux/` | KyberNow Link pour Linux, le Steam Deck, SteamOS et le Raspberry Pi (Rust, SDL2, FFmpeg par VA-API ou V4L2 ; Flatpak et .deb), voir [son README](apps/linux/README.md) et [son installation](apps/linux/INSTALLATION.md) ; KyberNow Link OS dans `apps/linux/os`, voir [son README](apps/linux/os/README.md) |
| `apps/tvos/`, `apps/ios/` | KyberNow Link pour Apple TV et pour iPhone/iPad, voir leurs README ([Apple TV](apps/tvos/README.md), [iPhone et iPad](apps/ios/README.md)) |
| `apps/core/`, `apps/windows/` | Cœur client en Rust des apps KyberNow Link (session, dialogue avec Admin, profils, latence, mises à jour, entrées et son partagés, voir [son README](apps/core/README.md)) et KyberNow Link pour Windows (Direct3D 11, FFmpeg, WASAPI), avec son installateur `KyberNow-Link-Setup.exe` (`apps/windows/setup`, x64 et ARM64), voir [son README](apps/windows/README.md) |
| `server/` | KyberNow Station : tout ce qui tourne sur l'ordinateur qui diffuse son image, voir [son README](server/README.md) |
| `server/admin/`, `server/web/` | KyberNow Admin (service Windows, Rust) et son interface (Svelte), qui sert aussi le client web `/ecran` |
| `server/tray/` | Icône de la Station dans la zone de notification |
| `server/installer/` | Installateur `KyberNow-Station-Setup.exe` (Rust + WebView2 + Svelte), voir [son README](server/installer/README.md) |
| `server/kyber-patches/`, `apps/macos/kyber-patches/` | Correctifs de Kyber, décrits dans [docs/kyber-changes.md](docs/kyber-changes.md) |
| `site/` | Site www.kybernow.app (statique, sans npm), voir [son README](site/README.md) |
| `brand/v2/` | Identité visuelle : charte, symbole, logos, jetons, voir [son README](brand/v2/README.md) ; `brand/` garde le logo précédent, inutilisé |
| `docs/` | [Changements de Kyber](docs/kyber-changes.md), [notes de compilation](docs/build-notes.md), [publication des versions](docs/releases.md), [cœur Rust des apps Apple](docs/coeur-apple.md), [langues et règles d'écriture](docs/i18n.md) |
| `docs/archives/` | Relevés datés, qui ne décrivent pas l'état actuel : prototype de référence et ses résultats (19 octobre 2026), audit et mesures demandées (20 octobre 2026), propositions HDR, latence Apple TV et app au démarrage (22 octobre 2026) |
| `tools/latency/` | Mesure de latence |

La documentation existe en français (`*.md`, la source) et en anglais (`*.en.md`, à côté de chaque fichier français) ; chaque évolution met à jour les deux dans le même commit. Elle décrit le projet tel qu'il est ; l'historique est dans `CHANGELOG.md` et dans git, le prévu dans `ROADMAP.md`. Règles d'écriture : [docs/i18n.md](docs/i18n.md#documentation).

KyberNow utilise [Kyber](https://kyber.tech), un moteur de streaming libre (AGPL-3.0) conçu par des développeurs de VLC. KyberNow est un projet communautaire et non commercial, développé par Les Lignes Claires ; il n'est ni édité, ni approuvé par l'équipe de Kyber, et n'y est pas affilié.

Licence : AGPL-3.0-or-later.
