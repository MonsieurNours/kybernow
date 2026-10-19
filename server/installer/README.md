# Installateur KyberNow

`KyberNow-Setup.exe` : un seul fichier qui installe le serveur Kyber 0.29, la console d'administration, l'icône de la zone de notification, le pilote d'écran virtuel SudoVDA et le pilote de manettes ViGEmBus.

## Architecture

```
KyberNow-Setup.exe = [lanceur][contenu tar.zst][pied 32 o « KNPAYLD1 »]
```

- **Lanceur** (`src/bin/setup.rs`) : sans WebView2, droits administrateur. Il dépose l'interface et `WebView2Loader.dll` dans `%TEMP%`, puis lance l'interface. Avec `--uninstall`, il désinstalle (c'est aussi `uninstall.exe` une fois installé).
- **Interface** (`src/bin/ui.rs` + `ui/`) : fenêtre sans bordure (tao + wry/WebView2) qui sert l'app Svelte via le protocole `kn://`. Messages JSON : `window.ipc.postMessage` dans un sens, `window.__knReceive` dans l'autre (`src/events.rs`).
- **Cœur** (`src/`) : `checks` (diagnostic), `ports` (tirage au hasard, plages réservées, TCP + UDP), `install` (tâches), `uninstall`, `payload` (format du contenu).

## Ce que fait l'installation

1. Arrête et retire une version précédente ou Kyber officiel (désinstalleur NSIS lancé avec `_?=` pour l'attendre).
2. Extrait le contenu dans `C:\Program Files\KyberNow\` (`kyber\`, `admin\`, `tray\`, `drivers\`, `licenses\`). Dans `kyber\`, les fichiers de Kyber 0.29 recouverts par la surcouche corrigée (`kyavserver.exe`, txproto et leur runtime, voir [docs/kyber-changes.md](../../docs/kyber-changes.md)).
3. Génère le certificat TLS de Kyber (`cert-gen.exe`).
4. `kyber-admin init` : port, encodeur, compte unique (Kyber Basic + console), secret JWT aléatoire à la place de la clé de développement de Kyber.
5. Crée le service `kyber` (chemin entre guillemets, redémarrage automatique en cas d'échec) et ouvre son démarrage et son arrêt aux utilisateurs interactifs (`sc sdset`, voir plus bas).
6. Pare-feu : ports du stream (TCP/UDP) et de la console (TCP), annonce Bonjour de la console (UDP 5353, pour que les apps trouvent l'ordinateur), sur les profils privé et domaine ; réseau « Public » passé en « Privé ».
7. SudoVDA : certificat dans Racine + Éditeurs approuvés, nœud de périphérique et pilote via `nefconc`.
8. ViGEmBus, s'il n'est pas déjà présent (`/quiet /norestart`).
8. bis. Son à distance (option) : haut-parleurs virtuels de Steam (`SteamStreamingSpeakers.inf` via `pnputil /add-driver /install`) si Steam est installé. La console y bascule la sortie de Windows pendant les sessions : le son quitte les haut-parleurs du PC et part au bon nombre de voies.
9. Service `kyber-admin` (console, mêmes droits que `kyber`), mode d'alimentation Performances élevées, inscription dans « Applications installées », raccourcis de la console et de l'icône dans le menu Démarrer.
10. Icône de la zone de notification : lancée à la fin de l'installation, et à chaque ouverture de session si « Démarrer avec Windows » est coché (valeur `KyberNow` de `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). Sans cette option, les deux services passent en démarrage manuel.

Journal : `C:\Program Files\KyberNow\install.log` (et `%TEMP%\KyberNow-install.log`).

## Icône de la zone de notification (`server/tray`)

`kybernow-tray.exe` tourne dans la session de l'utilisateur, sans droits administrateur. Menu : état du serveur et de la session (l'icône prend une pastille verte pendant une session, orange pendant un démarrage ou un arrêt, et se grise serveur arrêté), « Ouvrir la console » (`https://localhost:<port>`, port lu dans `C:\ProgramData\KyberNow\kyber-admin.toml`, sinon `kybernow.json`), « Arrêter / Démarrer le serveur » (service `kyber` ; la console reste en marche), « Redémarrer le serveur » (`kyber` et `kyber-admin`), « Démarrer avec l'ordinateur » et « Quitter », qui propose d'arrêter aussi le serveur. Une seconde ouverture (menu Démarrer) ouvre la console.

- **Droits des services.** L'installateur applique aux deux services le SDDL `SERVICE_SDDL` (`src/install.rs`) : celui de Windows par défaut, plus `RP` (démarrer), `WP` (arrêter) et `DT` (suspendre) pour `IU` (utilisateurs interactifs). La modification de la configuration (`DC`) reste réservée aux administrateurs : donner `DC` permettrait de changer l'exécutable d'un service qui tourne en SYSTEM.
- **Démarrer avec l'ordinateur.** La case suit le type de démarrage du service `kyber`. La basculer change celui des deux services (automatique ou manuel) et la valeur Run de l'utilisateur. Changer le type de démarrage demande l'élévation : l'icône se relance alors en administrateur (`--start-with-computer on|off`), avec une invite UAC ponctuelle.
- **Session en cours.** La console répond à `GET /api/local/state` sans authentification, mais seulement depuis l'ordinateur lui-même (`127.0.0.1`) : état du service `kyber`, session d'un appareil (écran ou son virtuels ouverts par l'app), identifiant Kyber.
- **Désinstallation.** Le processus est arrêté et la valeur Run retirée (pour l'utilisateur qui désinstalle).

Compiler seule : `cd server/tray && cargo build --release --target x86_64-pc-windows-gnu` (avec l'éditeur de liens de `build.sh`).

## Construire (depuis le Mac)

```bash
server/installer/build.sh      # → server/installer/dist/KyberNow-Setup.exe
```

Prérequis : `brew install mingw-w64 zstd`, `rustup target add x86_64-pc-windows-gnu`, node, et les contenus extraits dans `third_party/payload/` (voir l'en-tête du script).

## Développer l'interface

```bash
npm --prefix server/installer/ui run dev     # navigateur : backend simulé (ui/src/lib/backend.ts)
```

Hors Windows, `kybernow-ui` fonctionne aussi avec un déroulé d'installation simulé.
