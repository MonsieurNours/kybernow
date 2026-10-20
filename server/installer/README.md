# Installateur KyberNow

`KyberNow-Setup.exe` : un seul fichier qui installe le serveur Kyber 0.29, la console d'administration, l'icône de la zone de notification, le pilote d'écran virtuel SudoVDA et le pilote de manettes ViGEmBus.

## Architecture

```
KyberNow-Setup.exe = [lanceur][contenu tar.zst][pied 32 o « KNPAYLD1 »]
```

- **Lanceur** (`src/bin/setup.rs`) : sans WebView2, droits administrateur. Il dépose l'interface et `WebView2Loader.dll` dans `%TEMP%`, puis lance l'interface. Avec `--uninstall`, il désinstalle (c'est aussi `uninstall.exe` une fois installé).
- **Interface** (`src/bin/ui.rs` + `ui/`) : fenêtre sans bordure (tao + wry/WebView2) qui sert l'app Svelte via le protocole `kn://`. Messages JSON : `window.ipc.postMessage` dans un sens, `window.__knReceive` dans l'autre (`src/events.rs`). Identité v2 (symbole « server », Archivo et Martian Mono embarquées), quatre étapes dans la colonne de gauche, dont la couture se remplit au fil du parcours : **Bienvenue** (l'analyse de l'ordinateur tourne déjà à côté, ports modifiables ; rien ne change avant « Installer »), **Votre compte**, **Options** (écran virtuel, manettes, son, démarrage avec l'ordinateur, performances), **Installation** puis « Terminé » (adresse, QR code de la console, `/ecran` dans un navigateur, redémarrage pour les manettes). Version affichée : celle de `Cargo.toml`.
- **Cœur** (`src/`) : `checks` (diagnostic), `ports` (tirage au hasard, plages réservées, TCP + UDP), `install` (tâches), `uninstall`, `payload` (format du contenu).

## Ce que fait l'installation

1. Arrête et retire une version précédente ou Kyber officiel (désinstalleur NSIS lancé avec `_?=` pour l'attendre).
2. Extrait le contenu dans `C:\Program Files\KyberNow\` (`kyber\`, `admin\`, `tray\`, `drivers\`, `licenses\`). Dans `kyber\`, les fichiers de Kyber 0.29 recouverts par la surcouche corrigée (`kyavserver.exe`, txproto et leur runtime, `kycontroller.exe`, voir [docs/kyber-changes.md](../../docs/kyber-changes.md)).
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

`kybernow-tray.exe` tourne dans la session de l'utilisateur, sans droits administrateur. Symbole « server » de l'identité v2 (`resources/kybernow.ico`, régénéré par `resources/make-ico.py` : le symbole seul jusqu'à 32 px, la tuile de l'app au-delà), avec une pastille : verte en marche, Spectre pendant une session, orange pendant un démarrage ou un arrêt ; grisé serveur arrêté.

- **Clic gauche : le panneau** (`src/panel.rs`, `src/panel.html`). Une fenêtre sans bordure (coins arrondis de Windows 11, ombre) posée contre l'icône, du côté de la barre des tâches, qui se ferme dès qu'elle perd le focus ou avec Échap. Vue WebView2 (wry, boucle tao), données dans `%LOCALAPPDATA%\KyberNow\tray-webview`. En haut, l'état : en marche (nom de l'ordinateur, dernière session), en direct (appareil, depuis quand, latence), démarrage, arrêté. Puis : « Mettre fin à la session » (`POST /api/local/end`), « Ouvrir la console », « Installer KyberNow x.y » si une version est publiée, « Redémarrer le serveur » (`kyber` et `kyber-admin`), « Arrêter / Démarrer le serveur » (service `kyber`, la console reste en marche), « Démarrer avec l'ordinateur », « Réinitialiser le mot de passe de la console… », et « Fermer l'icône », qui propose d'arrêter aussi le serveur.
- **Clic droit : le menu de Windows**, avec les mêmes actions. Sans WebView2, le menu s'ouvre aussi au clic gauche. `WebView2Loader.dll` est installé à côté de l'icône (`tray\`), par l'installateur et par la mise à jour.
- **Réinitialiser le mot de passe de la console.** L'icône se relance en administrateur (`--reset-console-password`, invite UAC) : elle arrête la console, lance `kyber-admin.exe reset-password`, la redémarre. La console écrit alors un code d'initialisation (`C:\ProgramData\KyberNow\setup-code.txt`), que le panneau affiche et copie, et la console s'ouvre sur la première visite. Les appareils et leurs comptes ne changent pas.

« Ouvrir la console » : `https://localhost:<port>`, port lu dans `C:\ProgramData\KyberNow\kyber-admin.toml`, sinon `kybernow.json`. Une seconde ouverture (menu Démarrer) ouvre la console.

- **Droits des services.** L'installateur applique aux deux services le SDDL `SERVICE_SDDL` (`src/install.rs`) : celui de Windows par défaut, plus `RP` (démarrer), `WP` (arrêter) et `DT` (suspendre) pour `IU` (utilisateurs interactifs). La modification de la configuration (`DC`) reste réservée aux administrateurs : donner `DC` permettrait de changer l'exécutable d'un service qui tourne en SYSTEM.
- **Démarrer avec l'ordinateur.** La case suit le type de démarrage du service `kyber`. La basculer change celui des deux services (automatique ou manuel) et la valeur Run de l'utilisateur. Changer le type de démarrage demande l'élévation : l'icône se relance alors en administrateur (`--start-with-computer on|off`), avec une invite UAC ponctuelle.
- **Session en cours.** La console répond à `GET /api/local/state` et `POST /api/local/end` sans authentification, mais seulement depuis l'ordinateur lui-même (`127.0.0.1`) : état du service `kyber`, session d'un appareil (écran ou son virtuels ouverts par l'app), identifiant Kyber, appareil connecté et sa latence, dernière session, nom et adresse de l'ordinateur.
- **Désinstallation.** Le processus est arrêté et la valeur Run retirée (pour l'utilisateur qui désinstalle).

Compiler seule : `cd server/tray && cargo build --release --target x86_64-pc-windows-gnu` (avec l'éditeur de liens de `build.sh`).

## Construire (depuis le Mac)

`build.sh` produit `dist/KyberNow-Setup.exe` ; `package-update.sh` produit ensuite le paquet de mise à jour d'un ordinateur déjà installé (`dist/kybernow-update.zip`, avec `update.ps1`). Publication : [docs/releases.md](../../docs/releases.md).

```bash
server/installer/build.sh      # → server/installer/dist/KyberNow-Setup.exe
```

Prérequis : `brew install mingw-w64 zstd`, `rustup target add x86_64-pc-windows-gnu`, node, et les contenus extraits dans `third_party/payload/` (voir l'en-tête du script).

## Développer l'interface

```bash
npm --prefix server/installer/ui run dev     # navigateur : backend simulé (ui/src/lib/backend.ts)
```

Hors Windows, `kybernow-ui` fonctionne aussi avec un déroulé d'installation simulé.
