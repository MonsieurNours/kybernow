# Serveur KyberNow (Windows)

Tout ce qui tourne sur l'ordinateur partagé : le serveur Kyber 0.29 (corrigé, voir [docs/kyber-changes.md](../docs/kyber-changes.md)), la console d'administration, l'icône de la zone de notification et l'installateur qui pose le tout.

| Dossier | Contenu |
|---|---|
| `admin/` | Console `kyber-admin.exe` : service Windows (Rust, axum), HTTPS |
| `web/` | Interface de la console (Svelte), embarquée dans le binaire |
| `tray/` | Icône de la zone de notification `kybernow-tray.exe` |
| `installer/` | `KyberNow-Setup.exe`, voir [installer/README.md](installer/README.md) |
| `kyber-patches/` | Correctifs de Kyber côté serveur et leur script de compilation |

## La console (`kyber-admin`)

Installée par `KyberNow-Setup.exe`, en service Windows, sur un port tiré au hasard à l'installation (affiché à la fin, et dans l'icône de la zone de notification). Le compte est celui choisi pendant l'installation.

| Page | Rôle |
|---|---|
| Tableau de bord | État du serveur et de la session, GPU NVIDIA (NVML), démarrer, arrêter, redémarrer |
| Réglages, Avancé | `kyber_config.toml` (commentaires préservés, sauvegarde à chaque écriture), éditeur brut |
| Utilisateurs | Comptes Kyber (Basic) |
| Écran virtuel | SudoVDA : déclenchement, test, état ; disposition enregistrée par ensemble d'écrans |
| Son | Sortie audio virtuelle (haut-parleurs de Steam), voies, qualité Opus |
| Appareils | QR code et lien `kybernow://` pour ajouter l'ordinateur dans l'app, empreinte du certificat |
| Mises à jour | Version installée, dernière version publiée sur GitHub et ses notes, installation en un clic |
| Journaux | Kyber (`%ProgramData%\Kyber\logs`) et KyberNow |

**Pour les apps.** La console s'annonce en Bonjour (`_kybernow._tcp`, TXT `v`, `name`, `fp`), d'où la règle de pare-feu UDP 5353. Elle sert aussi l'API de session :
- `GET /api/stream/info` : nom, GPU, AV1, écran virtuel, haut-parleurs ;
- `POST /api/stream/prepare` : écran virtuel à la taille du client, son au bon nombre de voies ;
- `POST /api/stream/heartbeat` : signal de vie (sans lui, l'écran virtuel disparaît) ;
- `GET /api/stream/foreground` : app au premier plan sur l'ordinateur, en plein écran ou non, jeu ou non (mode Auto de la souris des apps). La console l'obtient d'un agent qui tourne dans la session Windows le temps d'une session (`display-agent foreground`, état écrit toutes les 400 ms). L'agent s'arrête seul sans signal de vie pendant 20 s ;
- `POST /api/stream/end` : écrans et sortie audio d'origine rendus, avec un compte rendu ;
- `GET /api/pairing` : lien d'appairage.

**Mises à jour** (compte de la console) : `GET /api/update` (version installée, dernière version publiée, notes), `POST /api/update/check`, `POST /api/update/install`. `GET /api/local/state` (réservé à l'ordinateur lui-même, pour l'icône) donne aussi `update_available` et `version`.

L'authentification est en Basic, avec le compte Kyber. Les apps épinglent le certificat de la console par son empreinte.

Les changements d'écran, de son et de pointeur sont faits dans la session de l'utilisateur par `kyber-admin.exe display-agent …`, lancé par le service.

| Emplacement | Contenu |
|---|---|
| `C:\ProgramData\KyberNow\kyber-admin.toml` | réglages : `listen`, dossier de Kyber, empreinte du mot de passe, écran virtuel, son |
| `C:\ProgramData\KyberNow\tls\` | certificat de la console |
| `C:\ProgramData\KyberNow\backups\` | versions précédentes de `kyber_config.toml` |
| `C:\ProgramData\KyberNow\display-session.json`, `audio-session.json`, `pointer-session.json` | ce qu'il faut restaurer en fin de session, même après un plantage |
| `C:\ProgramData\KyberNow\update\<version>\` | paquet téléchargé, contenu, journal `update.log` |
| `C:\ProgramData\KyberNow\foreground\` | veille du plein écran pendant une session (`run` : signal de vie, `state.json` : dernier état) |
| `C:\ProgramData\KyberNow\kyber-admin.log` | journal du service |

Commandes : `kyber-admin.exe service install|uninstall`, `kyber-admin.exe reset-password` (puis redémarrer le service), `kyber-admin.exe init …` (utilisée par l'installateur), `kyber-admin.exe` seul pour lancer en console (diagnostic).

## Développer sur le Mac

```bash
cd server/web && npm run build                       # ou npm run dev (proxy /api vers la console)
cd ../admin && cargo run -- --http --listen 127.0.0.1:9443 \
    --data-dir dev-data --kyber-dir dev-data/kyber    # copier un kyber_config.toml dans dev-data/kyber
```

Hors Windows, le service Kyber, l'écran virtuel, le son et le GPU sont simulés ou absents.

## Compiler pour Windows (depuis le Mac)

```bash
server/build-windows.sh            # console seule → server/dist/kyber-admin.exe
server/kyber-patches/build.sh      # Kyber corrigé → third_party/payload/kyber-overlay/
server/installer/build.sh          # tout → server/installer/dist/KyberNow-Setup.exe
```

## Mettre à jour un ordinateur déjà installé

En un clic : page **Mises à jour** de la console (ou « Installer la version X… » dans le menu de l'icône). La console télécharge la dernière version publiée sur GitHub, vérifie son empreinte et lance `update.ps1`. Détails, mise à jour à la main et publication d'une version : [docs/releases.md](../docs/releases.md).

`server/installer/update.ps1` remplace la console, l'icône et la surcouche Kyber sans réinstaller ni changer les ports. Il garde les fichiers Kyber remplacés dans `kyber\avant-maj`, installe les haut-parleurs de Steam s'ils manquent et ajoute la règle de pare-feu Bonjour. `server/installer/package-update.sh` fabrique son paquet (`dist/kybernow-update.zip`).
