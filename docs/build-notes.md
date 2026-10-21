# Notes de build

[English](build-notes.en.md) · Français

## Versions des outils

Les binaires Kyber de KyberNow (surcouche serveur, bibliothèques des apps Mac et Apple TV) sont compilés et vérifiés avec ces versions. Elles sont écrites dans `scripts/kyber-toolchain.sh`, que lisent les trois scripts de compilation : une version différente n'arrête rien, elle est signalée au début (« Attention : rustc 1.90.0, versions vérifiées : 1.89.0 »). Après une mise à jour vérifiée, changer les deux endroits.

| Outil | Version | Pour |
|---|---|---|
| Rust stable (`rustc`) | 1.89.0, cible `x86_64-pc-windows-gnu` | serveur Windows, client macOS |
| Rust nightly | du 2026-10-19 (1.101.0), composant `rust-src` | client Apple TV (`-Zbuild-std`) |
| cargo-c (`cargo cinstall`) | 0.10.15 | clients macOS et Apple TV |
| mingw-w64 (Homebrew) | 14.0.0, GCC 16.2.0, UCRT | serveur Windows |
| meson | 1.11.2 | serveur Windows (dépendances, txproto) |
| llvm-mingw | 20261006 (UCRT, macOS), clang 23.1.3 | client Windows ARM64, dans `third_party/toolchains` |
| wasm-bindgen-cli | 0.2.100 (celle du `Cargo.lock` de kyber-web) | moteur du client web |
| binaryen (`wasm-opt`) | 133 | moteur du client web |

Les dépôts de Kyber n'ont pas de `rust-toolchain.toml` : c'est la chaîne par défaut de rustup qui compile.

## Scripts de compilation : échec réel

`server/kyber-patches/build.sh`, `apps/macos/scripts/build-kyclient.sh` et `apps/tvos/scripts/build-kyber-tvos.sh` s'arrêtent à la première erreur et ne remplacent alors rien (surcouche, `Vendor/`) :

- **cargo** : sa sortie est filtrée (lignes de progression retirées) sans que le filtre décide du code de retour (`set -o pipefail`). Avant, `| grep … || true` masquait une compilation ratée et l'ancien binaire partait dans la surcouche.
- **Binaire périmé** : `build.sh` refuse `kyavserver.exe`, `libtxproto-0.dll`, `kycontroller.exe`, `kynput.dll`, `kynputserver.exe` et `kyclient_wasm.wasm` s'ils sont plus anciens que son lancement (il remet les sources à l'origine à chaque fois, donc ils sont toujours recompilés). `cargo cinstall` garde la date de la bibliothèque déjà compilée : les scripts client (et `build.sh` pour `kynput.dll`) retirent plutôt la bibliothèque installée par un build précédent avant de compiler, et la copie échoue si ce build ne l'a pas réinstallée.
- **Correctif inapplicable** : le hunk refusé est affiché (`patch -r -` côté serveur, `git apply --check` côté client).
- **Correctif qui ajoute un fichier** : `build.sh` remet à l'origine chaque fichier des correctifs serveur, un par un ; un fichier absent de l'origine est supprimé au lieu d'être restauré. Pas de `git checkout -- .` : `kysdk/kyctl` et `kysdk/kymux` portent aussi les correctifs des apps.
- **Worktree** : `third_party/` (hors git) peut y être un lien vers celui du dépôt principal. Les scripts en prennent le chemin réel (`pwd -P`) : meson et cargo enregistrent les chemins absolus des sources, et un autre chemin les fait tout reconfigurer et tout recompiler.
- **App ouverte** : `build-kyclient.sh` refuse de tourner si KyberNow est ouvert (`pgrep -x KyberNow`), au début et juste avant de copier dans `Vendor/` (voir « Intégrer les bibliothèques dans l'app sans la faire planter » plus bas).

## Client macOS Kyber 0.29 (kyber-desktop) avec Xcode 27

**Symptôme** : l'édition de liens finale de `kyclient` échoue :

```
ld: mis-aligned LINKEDIT string pool, fileOffset=0x006E8D6C in '.../rootfs-arm64-macos/lib/libkyclient.dylib'
```

**Cause** : `cargo-c` compile `libkyclient` avec `-C strip=debuginfo`. Sur macOS, rustc 1.89 fait ce strip avec `rust-objcopy` (LLVM), qui laisse la table des chaînes alignée sur 4 octets. Le linker d'Xcode 27 (ld-27037) exige 8 octets et refuse la bibliothèque. Le `strip` d'Apple (`/usr/bin/strip -S`) ne pose pas ce problème. `libkynput` n'est pas touchée (son alignement tombe juste par hasard).

**Contournement** : désactiver le strip de Cargo pour tout le build :

```bash
export CARGO_PROFILE_RELEASE_STRIP=false
./build-macos.sh -n -p -a arm64 client      # -s pour ne pas recompiler VLC/FFmpeg
```

Vérification d'une bibliothèque (doit afficher 0) :

```bash
s=$(otool -l rootfs-arm64-macos/lib/libkyclient.0.1.0.dylib | awk '/stroff/{print $2}'); echo $((s % 8))
```

À remonter à Kyber, et à garder en tête pour le build de KyberKit (xcframework macOS/tvOS).

### libkyclient avec les correctifs KyberNow

`apps/macos/scripts/build-kyclient.sh` applique `apps/macos/kyber-patches/*.patch` au dépôt `kysdk/kyctl`, recompile libkyclient (`cargo cinstall`, quelques secondes) et la remplace dans `apps/macos/Vendor/`. À relancer après chaque `vendor-kyber.sh`. `vendor-kyber.sh` copie aussi les en-têtes d'Opus (module `COpus`) : le son est décodé par l'app.

- **strip** : avec le `strip` par défaut du profil release, l'éditeur de liens d'Xcode 27 refuse la bibliothèque (« mis-aligned LINKEDIT string pool »). Le script compile avec `CARGO_PROFILE_RELEASE_STRIP=false`.

### Intégrer les bibliothèques dans l'app sans la faire planter

`apps/macos/scripts/embed-kyber.sh` (phase de build Xcode) signe une copie de travail de `Vendor/Frameworks` (dans `DERIVED_FILE_DIR`), en ne re-signant que ce qui a changé. Il ne recopie dans l'app que les fichiers qui diffèrent.

- **Pourquoi** : réécrire les bibliothèques de l'app pendant qu'elle tourne fait planter VLC au démarrage de la session suivante (`config_GetInt` sur NULL, dans `libsyslog_plugin`). Ses modules, chargés à la demande depuis les nouveaux fichiers, lient un nouveau fichier `libvlccore` alors que l'ancien est déjà en mémoire : deux cœurs de VLC coexistent.
- **Après `build-kyclient.sh`** : les bibliothèques changent réellement. Il faut quitter l'app avant de recompiler, puis la relancer.
- **Hors d'un paquet `.app`** (programme de test lancé à côté de `Vendor/Frameworks`) : VLC cherche aussi ses modules dans le dossier où il a été compilé (`rootfs-arm64-macos/lib/vlc/plugins`), avec le même plantage. Dans l'app, il ne prend que `Contents/Frameworks/plugins`.

## Navigateurs

Le client web Kyber fonctionne dans Chrome. Dans Safari, l'écran reste noir (WebKit n'est pas pris en charge, comme l'indique la doc Kyber).

## Serveur Kyber 0.29 pour Windows (kyavserver), depuis macOS

`server/kyber-patches/build.sh` enchaîne tout (versions : voir plus haut). Pièges rencontrés avec la chaîne MinGW de Homebrew :

- **luac** : txproto compile ses scripts Lua pendant le build et voulait lancer `luac.exe`. Il faut un `luac` 5.4 natif dans le PATH (`brew install lua@5.4`) ; le bytecode est le même (64 bits, petit-boutiste).
- **Jinja** : l'archive de libplacebo n'inclut pas ses sous-modules. Copier `jinja2` et `markupsafe` (roues pip) dans `subprojects/libplacebo-*/3rdparty/{jinja,markupsafe}/src`.
- **bison** : celui de macOS (2.3) est trop ancien pour VLC ; utiliser `/opt/homebrew/opt/bison/bin`.
- **pkg-config** : meson attend `x86_64-w64-mingw32-pkg-config` ; un simple relais vers `pkg-config` suffit (le rootfs est imposé par le fichier croisé).
- **bindgen** : `BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu --sysroot=<mingw> -I<mingw>/include"`, sinon `errno.h` est introuvable.
- **Runtime** : MinGW Homebrew produit des binaires UCRT, Kyber officiel est en msvcrt. On remplace donc toute la chaîne de kyavserver (voir la surcouche), jamais une DLL isolée. Exception sûre : `kynput.dll`, voir plus bas.

## Serveur d'entrées de Kyber 0.29 pour Windows (kynput), depuis macOS

`server/kyber-patches/build.sh` recompile aussi `kynput.dll` (`cargo cinstall`, fonctionnalité `sdl2` comme la version officielle, installée dans `third_party/rootfs-win64-kynput`) et `kynputserver.exe` (`win32-hide-console`), pour les correctifs `*-kynput-*`.

- **SDL2** : `kynput.dll` se lie à la `SDL2.dll` de Kyber 0.29 (`third_party/payload/kyber-0.29.0`), qu'on ne recompile pas. `sdl2-sys` la cherche par pkg-config : le script écrit un `sdl2.pc` minimal (`third_party/toolchain-bin/pkgconfig-sdl2`) qui pointe ce dossier, et l'éditeur de liens de MinGW lie la DLL directement, sans bibliothèque d'import.
- **build.rs de kynputserver** : il testait `cfg!(target_os = "macos")`, c'est-à-dire la machine qui compile ; depuis un Mac, il assemblait le code macOS avec MinGW et échouait. Corrigé par 0012 (test de `CARGO_CFG_TARGET_OS`).
- **Runtime** : comme le reste de la surcouche, `kynput.dll` et `kynputserver.exe` sont en UCRT, à côté de `kyclient.dll` et `kyclient.exe` officiels (msvcrt) qui chargent la même `kynput.dll`. Sans risque ici : l'API C de kynput libère toujours elle-même ce qu'elle alloue (`kynput_*_unref`, `kynput_free_string`). Elle exporte les fonctions de la version officielle, plus celles du presse-papiers si le correctif client 0002 est appliqué dans `third_party` au moment du build.

## Client Windows (apps/windows), depuis macOS

`apps/windows/scripts/build.sh` compile le cœur client (`apps/core`) et l'app Windows avec la chaîne du serveur (`x86_64-pc-windows-gnu`, MinGW de Homebrew), en s'appuyant sur ce que `server/kyber-patches/build.sh` a produit : FFmpeg et libopus statiques (`third_party/rootfs-win64`), `kynput.dll` (`third_party/rootfs-win64-kynput`).

- **Crates de Kyber** : l'app dépend directement de la crate Rust `kyclient` (pas de son API C). Ses dépendances se nomment par leur version crates.io : `apps/Cargo.toml` les renvoie vers `third_party/kyber-desktop/kysdk` (`[patch.crates-io]`, comme `kysdk/.cargo/config.toml`), et `apps/Cargo.lock` part de celui de `kysdk/kyctl` pour garder les mêmes versions.
- **VLC** : `kyvlcplayer` lie toujours `libvlc` sous Windows (`vlc-rs`, par pkg-config), même quand l'image et le son vont à l'app. Le script fabrique une `libvlc.a` de fonctions vides (une par export de `libvlc.dll.a`) et un `libvlc.pc` qui la désigne, dans `third_party/windows-client-x64`. Les bibliothèques statiques de FFmpeg sont liées depuis un dossier à part (`third_party/windows-client-x64/native`) : celui du rootfs contient aussi `libvlc.dll.a`, que l'éditeur de liens préférait aux fonctions vides (l'exécutable dépendait alors de `libvlc.dll`). Le script refuse un exécutable qui en dépend encore.
- **libavcodec** : son décodeur Opus appelle libswresample, à lier aussi.
- **Liaisons de FFmpeg** : générées par bindgen au build (`apps/windows/build.rs`) depuis les en-têtes du rootfs, avec les mêmes `BINDGEN_EXTRA_CLANG_ARGS` que kynput. `AVD3D11VADeviceContext` est recopiée à la main (des pointeurs) pour ne pas faire lire les en-têtes COM de Direct3D à bindgen.
- **Taille** : l'exécutable embarque les encodeurs de la libavcodec du serveur (x264, VPL) ; la copie de `dist/` est sans informations de débogage (36 Mo).
- **ARM64** (`--arch arm64`) : cible Rust `aarch64-pc-windows-gnullvm` (`rustup target add`) et chaîne [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) (version UCRT pour macOS, décompressée dans `third_party/toolchains`) : MinGW de Homebrew ne vise pas ARM64. `apps/windows/scripts/build-arm64-deps.sh` compile dans `third_party/rootfs-winarm64` ce que le serveur ne fournit pas pour ARM64 : libopus (`--disable-rtcd` : la détection du processeur n'existe pas pour Windows ARM, NEON est toujours là), un FFmpeg réduit au décodage (H.264, HEVC, AV1, accélération D3D11VA, sans x264 ni VPL : 16 Mo au lieu de 170), SDL2 2.32.8 (l'archive téléchargée et vérifiée par le script de kynput) et `kynput.dll` (`cargo cinstall --dlltool llvm-dlltool`). L'exécutable demande `libunwind.dll` de llvm-mingw, copiée dans `dist/arm64`.
- **set -e** : dans `make && make install`, un échec de `make` n'arrête pas le script (seule la dernière commande d'une liste `&&` compte) ; libopus avait échoué sans bruit. Les deux sont sur deux lignes, et le script vérifie les bibliothèques produites.
- **App Tauri** : Tauri **2.11.6** et tauri-build 2.6.3, compilés pour `windows-gnu` et `windows-gnullvm` depuis le Mac (comme `tao` et `wry` de l'installateur du serveur). Tauri 2.12 et ses briques demandent Rust 1.90 alors que KyberNow est vérifié en 1.89 : l'espace de travail `apps/` utilise le résolveur 3, qui choisit les versions compatibles avec `rust-version`. L'interface (`ui/`, Svelte et Vite, mêmes versions que la console) est construite par `build.sh` avant cargo et embarquée dans l'exécutable ; `WebView2Loader.dll` (celle de `webview2-com-sys`, x64 ou arm64) est copiée à côté. L'icône (`resources/kybernow.ico`, symbole « now ») vient de `resources/make-ico.py`.
- **Installateur** (NSIS 3.13 de Homebrew) : `makensis` refuse un script en UTF-8 (« Bad text encoding ») si la langue du terminal n'est pas en UTF-8 ; `build-installer.sh` le lance avec `LC_ALL=en_US.UTF-8`, et le script est enregistré avec une marque d'ordre des octets.
- **Tests du cœur sur le Mac** : `cargo test -p kybernow-core` avec `PKG_CONFIG_PATH` vers `third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig` (libvlc et kynput du client macOS de Kyber).

## Moteur du client web (kyclient_wasm), depuis macOS

Le moteur que charge le client web vient du dépôt `kyber/apps/kyber-web` de Kyber (crate `kyclient-wasm`, l'API JS), pas de kyber-desktop. `server/kyber-patches/build.sh` le recompile avec les correctifs 0013 à 0015 et pose `kyclient_wasm.js` et `kyclient_wasm_bg.wasm` dans `webclient/` de la surcouche.

- **Sources** : `git clone --depth 1 --branch 0.29.0 https://gitlab.com/kyber/apps/kyber-web.git third_party/kyber-web`. Son sous-module `kysdk` est le même commit que celui de kyber-desktop 0.29 (`ebca32a`) : il n'est pas cloné, le script le remplace par un lien.
- **Outils** : `rustup target add wasm32-unknown-unknown`, `cargo install wasm-bindgen-cli --version 0.2.100 --locked` (la version doit être celle de la crate `wasm-bindgen` du `Cargo.lock` de kyber-web, sinon wasm-bindgen refuse le module), `brew install binaryen` (`wasm-opt`). Le reste du `build-wasm.sh` de kyber-web (pnpm, interface de Kyber) n'est pas nécessaire : l'interface ne change pas.
- **kyctl d'origine** : le `kysdk/kyctl` de `third_party` porte aussi les correctifs des apps, qui changent l'API de kyclient (le champ `packet_sink` de `AudioSessionConfig` casse la compilation de kyber-web). Le script compose donc `third_party/kyber-web-kysdk/` : `kyctl` extrait de git (`git archive HEAD`) plus les correctifs `*-kywebplayer-*`, et des liens vers `kymedia`, `kymux`, `kynput` et `kyutil` de `third_party`, correctifs serveur appliqués. `third_party/kyber-web/kysdk` pointe vers ce dossier.
- **Étapes** : `cargo build --target wasm32-unknown-unknown -p kyclient-wasm --release` (avec `RUSTFLAGS=--cfg=web_sys_unstable_apis`), `wasm-bindgen --target web --no-typescript`, puis `wasm-opt -Os -g`, comme le script de Kyber. Le résultat garde exactement l'API JS du moteur de Kyber 0.29 (mêmes classes, fonctions et méthodes) : la page `/ecran` n'a rien à changer.
