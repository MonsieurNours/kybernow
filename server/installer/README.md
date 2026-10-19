# Installateur KyberNow

`KyberNow-Setup.exe` : un seul fichier qui installe le serveur Kyber 0.29, la console d'administration, le pilote d'écran virtuel SudoVDA et le pilote de manettes ViGEmBus.

## Architecture

```
KyberNow-Setup.exe = [lanceur][contenu tar.zst][pied 32 o « KNPAYLD1 »]
```

- **Lanceur** (`src/bin/setup.rs`) : sans WebView2, droits administrateur. Il dépose l'interface et `WebView2Loader.dll` dans `%TEMP%`, puis lance l'interface. Avec `--uninstall`, il désinstalle (c'est aussi `uninstall.exe` une fois installé).
- **Interface** (`src/bin/ui.rs` + `ui/`) : fenêtre sans bordure (tao + wry/WebView2) qui sert l'app Svelte via le protocole `kn://`. Messages JSON : `window.ipc.postMessage` dans un sens, `window.__knReceive` dans l'autre (`src/events.rs`).
- **Cœur** (`src/`) : `checks` (diagnostic), `ports` (tirage au hasard, plages réservées, TCP + UDP), `install` (tâches), `uninstall`, `payload` (format du contenu).

## Ce que fait l'installation

1. Arrête et retire une version précédente ou Kyber officiel (désinstalleur NSIS lancé avec `_?=` pour l'attendre).
2. Extrait le contenu dans `C:\Program Files\KyberNow\` (`kyber\`, `admin\`, `drivers\`, `licenses\`).
3. Génère le certificat TLS de Kyber (`cert-gen.exe`).
4. `kyber-admin init` : port, encodeur, compte unique (Kyber Basic + console), secret JWT aléatoire à la place de la clé de développement de Kyber.
5. Crée le service `kyber` (chemin entre guillemets, redémarrage automatique en cas d'échec).
6. Pare-feu : ports du stream (TCP/UDP) et de la console (TCP) sur les profils privé et domaine ; réseau « Public » passé en « Privé ».
7. SudoVDA : certificat dans Racine + Éditeurs approuvés, nœud de périphérique et pilote via `nefconc`.
8. ViGEmBus, s'il n'est pas déjà présent (`/quiet /norestart`).
9. Service `kyber-admin` (console), mode d'alimentation Performances élevées, inscription dans « Applications installées », raccourci de la console dans le menu Démarrer.

Journal : `C:\Program Files\KyberNow\install.log` (et `%TEMP%\KyberNow-install.log`).

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
