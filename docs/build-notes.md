# Notes de build

[English](build-notes.en.md) · Français

## Versions des outils

Les binaires Kyber de KyberNow (surcouche serveur, bibliothèques des apps Mac, Apple TV, iPhone et iPad) sont compilés et vérifiés avec ces versions. Elles sont écrites dans `scripts/kyber-toolchain.sh`, que lisent les scripts de compilation : une version différente n'arrête rien, elle est signalée au début (« Attention : rustc 1.90.0, versions vérifiées : 1.89.0 »). Après une mise à jour vérifiée, changer les deux endroits.

| Outil | Version | Pour |
|---|---|---|
| Rust stable (`rustc`) | 1.89.0, cibles `x86_64-pc-windows-gnu`, `aarch64-apple-ios`, `aarch64-apple-ios-sim` | serveur Windows, client macOS, cœur client pour iOS |
| Rust nightly | du 2026-10-19 (1.101.0), composant `rust-src` | client Apple TV et cœur client pour tvOS (`-Zbuild-std`) |
| cargo-c (`cargo cinstall`) | 0.10.15 | clients macOS et Apple TV, libkynput pour iOS |
| mingw-w64 (Homebrew) | 14.0.0, GCC 16.2.0, UCRT | serveur Windows |
| meson | 1.11.2 | serveur Windows (dépendances, txproto) |
| llvm-mingw | 20261006 (UCRT, macOS), clang 23.1.3 | client Windows ARM64, dans `third_party/toolchains` |
| wasm-bindgen-cli | 0.2.100 (celle du `Cargo.lock` de kyber-web) | moteur du client web |
| binaryen (`wasm-opt`) | 133 | moteur du client web |

Les dépôts de Kyber n'ont pas de `rust-toolchain.toml` : c'est la chaîne par défaut de rustup qui compile.

## Scripts de compilation : échec réel

`server/kyber-patches/build.sh`, `apps/macos/scripts/build-kyclient.sh`, `apps/tvos/scripts/build-kyber-tvos.sh`, `apps/core/scripts/build-apple.sh`, `apps/apple/scripts/build-apple.sh` et `apps/apple/scripts/build-apple-mobile.sh` s'arrêtent à la première erreur et ne remplacent alors rien (surcouche, `Vendor/`) :

- **cargo** : sa sortie est filtrée (lignes de progression retirées) sans que le filtre décide du code de retour (`set -o pipefail`). Ne pas écrire `| grep … || true` sur la commande elle-même : une compilation ratée passerait inaperçue et l'ancien binaire partirait dans la surcouche.
- **Binaire périmé** : `build.sh` refuse `kyavserver.exe`, `libtxproto-0.dll`, `kycontroller.exe`, `kynput.dll`, `kynputserver.exe` et `kyclient_wasm.wasm` s'ils sont plus anciens que son lancement (il remet les sources à l'origine à chaque fois, donc ils sont toujours recompilés). `cargo cinstall` garde la date de la bibliothèque déjà compilée : les scripts client (et `build.sh` pour `kynput.dll`) retirent plutôt la bibliothèque installée par un build précédent avant de compiler, et la copie échoue si ce build ne l'a pas réinstallée.
- **Correctif inapplicable** : le hunk refusé est affiché (`patch -r -` côté serveur, `git apply --check` côté client).
- **Correctif qui ajoute un fichier** : `build.sh` remet à l'origine chaque fichier des correctifs serveur, un par un ; un fichier absent de l'origine est supprimé au lieu d'être restauré. Pas de `git checkout -- .` : `kysdk/kyctl` et `kysdk/kymux` portent aussi les correctifs des apps.
- **Worktree** : `third_party/` (hors git) peut y être un lien vers celui du dépôt principal. Les scripts en prennent le chemin réel (`pwd -P`) : meson et cargo enregistrent les chemins absolus des sources, et un autre chemin les fait tout reconfigurer et tout recompiler.
- **App ouverte** : `build-kyclient.sh` et `apps/apple/scripts/build-apple.sh` refusent de tourner si KyberNow est ouvert (`pgrep -x KyberNow`) ; `build-kyclient.sh` le vérifie au début et juste avant de copier dans `Vendor/` (voir « Intégrer les bibliothèques dans l'app sans la faire planter » plus bas).

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

À remonter à Kyber. Tous les scripts de compilation Rust de KyberNow posent `CARGO_PROFILE_RELEASE_STRIP=false`.

### libkyclient avec les correctifs KyberNow

`apps/macos/scripts/build-kyclient.sh` applique `apps/macos/kyber-patches/*.patch` au dépôt que nomme chaque fichier (`kysdk/kyctl`, `kysdk/kynput`, `kysdk/kymux`), recompile libkynput et libkyclient (`cargo cinstall`, quelques secondes) et les remplace dans `apps/macos/Vendor/`. À relancer après chaque `vendor-kyber.sh`. `vendor-kyber.sh` copie aussi les en-têtes d'Opus (module `COpus`) : le son est décodé par l'app.

- **strip** : avec le `strip` par défaut du profil release, l'éditeur de liens d'Xcode 27 refuse la bibliothèque (« mis-aligned LINKEDIT string pool »). Le script compile avec `CARGO_PROFILE_RELEASE_STRIP=false`.

### Intégrer les bibliothèques dans l'app sans la faire planter

`apps/macos/scripts/embed-kyber.sh` (phase de build Xcode) signe une copie de travail de `Vendor/Frameworks` (dans `DERIVED_FILE_DIR`), en ne re-signant que ce qui a changé. Il ne recopie dans l'app que les fichiers qui diffèrent.

- **Pourquoi** : réécrire les bibliothèques de l'app pendant qu'elle tourne fait planter VLC au démarrage de la session suivante (`config_GetInt` sur NULL, dans `libsyslog_plugin`). Ses modules, chargés à la demande depuis les nouveaux fichiers, lient un nouveau fichier `libvlccore` alors que l'ancien est déjà en mémoire : deux cœurs de VLC coexistent.
- **Après `build-kyclient.sh` ou `build-apple.sh`** : les bibliothèques changent réellement. Il faut quitter l'app avant de recompiler, puis la relancer.
- **Hors d'un paquet `.app`** (programme de test lancé à côté de `Vendor/Frameworks`) : VLC cherche aussi ses modules dans le dossier où il a été compilé (`rootfs-arm64-macos/lib/vlc/plugins`), avec le même plantage. Dans l'app, il ne prend que `Contents/Frameworks/plugins`.

### Le cœur Rust des apps Apple (apps/apple)

`apps/apple/scripts/build-apple.sh` compile la crate `kybernow-apple` (API C écrite à la main sur `kybernow-core`, voir [coeur-apple.md](coeur-apple.md)) avec `cargo cinstall` pour `aarch64-apple-darwin`, pose `libkybernow.dylib` dans `apps/macos/Vendor/Frameworks` (embarquée par `embed-kyber.sh` comme les autres), l'en-tête `kybernow.h` produit par cbindgen et un module Clang `CKyberNow` dans `apps/macos/Vendor/KyberNow/include` (`SWIFT_INCLUDE_PATHS` du `project.yml`).

- **`capi`** : cargo-c exige une feature de ce nom dans la crate, même vide (`[features] capi = []`), sinon « package does not contain this feature: capi ».
- **Constantes** : `item_types` de `cbindgen.toml` doit nommer `constants`, sans quoi les `KN_*` manquent à l'en-tête. Ce sont des `#define`, que Swift importe en `Int32` : comparer avec `Int32(kind)`, pas avec le `uint32_t` de la structure.
- **Chemins** : cargo-c inscrit un `install_name` absolu ; le script le ramène à `@rpath/libkybernow.dylib`, et ses dépendances (`libvlc`, `libkynput.0.1`) à `@rpath`.
- **App ouverte** : le script refuse de tourner si KyberNow est lancée (voir plus haut) ; `KYBERNOW_IGNORE_RUNNING=1` passe outre quand le `Vendor/` visé n'est pas celui de l'app qui tourne (un autre worktree).
- **libkyclient** : `libkybernow` embarque le code de la crate Rust `kyclient`. Rien n'appelle l'API C de libkyclient ; le module `CKyber` lie quand même `libkyclient.dylib` sur Mac (embarquée, jamais appelée), et sert pour les en-têtes de libkynput (mise en page de la vue, journal).
- **tvOS et iOS** (`apps/apple/scripts/build-apple-mobile.sh tvos|ios`) : `cargo cinstall --library-type staticlib` par cible (tvOS : Rust nightly et `-Zbuild-std=std,panic_abort`, comme `build-kyber-tvos.sh` ; iOS : stable), avec le rootfs de la cible pour `kynput.pc` (`apps/core/scripts/build-apple.sh` le construit au besoin), puis `xcodebuild -create-xcframework` vers `apps/<plateforme>/Vendor/KyberNow.xcframework` ; le module `CKyberNow` n'y porte pas de `link` (bibliothèque statique, liée par la dépendance au xcframework). Sur tvOS, libkynput, libopus et les fonctions vides de VLC viennent de `KyberSDK.xcframework` : l'éditeur de liens prend chaque symbole dans la première bibliothèque qui le définit, et `-dead_strip` écarte le reste ; deux bibliothèques Rust statiques (le cœur, libkynput) cohabitent. Pour iOS, il n'y a pas de SDK Kyber à part : libkynput (rootfs de la cible), libopus (compilée par cmake depuis les sources de kymedia, dans le rootfs) et les fonctions vides de VLC (générées d'après `nm -u` de la bibliothèque, comme pour l'Apple TV) rejoignent le cœur dans la même bibliothèque (`libtool -static`), et `kynput.h` va dans `Vendor/include` (module `CKyber`). Les chaînes des apps iOS s'extraient par `xcrun xcstringstool sync` depuis les `.stringsdata` du build (`xcodebuild` ne remplit pas le catalogue lui-même), puis `apps/macos/scripts/check-strings.py --fix`. Environ 127 Mo par cible avec les informations de débogage.

## Navigateurs

Le client web Kyber fonctionne dans Chrome. Dans Safari, l'écran reste noir (WebKit n'est pas pris en charge, comme l'indique la doc Kyber).

## Serveur Kyber 0.29 pour Windows (kyavserver), depuis macOS

`server/kyber-patches/build.sh` enchaîne tout (versions : voir plus haut). Pièges rencontrés avec la chaîne MinGW de Homebrew :

- **luac** : txproto compile ses scripts Lua pendant le build et veut lancer `luac.exe`. Il faut un `luac` 5.4 natif dans le PATH (`brew install lua@5.4`) ; le bytecode est le même (64 bits, petit-boutiste).
- **Jinja** : l'archive de libplacebo n'inclut pas ses sous-modules. Copier `jinja2` et `markupsafe` (roues pip) dans `subprojects/libplacebo-*/3rdparty/{jinja,markupsafe}/src`.
- **bison** : celui de macOS (2.3) est trop ancien pour VLC ; utiliser `/opt/homebrew/opt/bison/bin`.
- **pkg-config** : meson attend `x86_64-w64-mingw32-pkg-config` ; un simple relais vers `pkg-config` suffit (le rootfs est imposé par le fichier croisé).
- **bindgen** : `BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu --sysroot=<mingw> -I<mingw>/include"`, sinon `errno.h` est introuvable.
- **Runtime** : MinGW Homebrew produit des binaires UCRT, Kyber officiel est en msvcrt. On remplace donc toute la chaîne de kyavserver (voir la surcouche), jamais une DLL isolée. Exception sûre : `kynput.dll`, voir plus bas.

## Serveur d'entrées de Kyber 0.29 pour Windows (kynput), depuis macOS

`server/kyber-patches/build.sh` recompile aussi `kynput.dll` (`cargo cinstall`, fonctionnalité `sdl2` comme la version officielle, installée dans `third_party/rootfs-win64-kynput`) et `kynputserver.exe` (`win32-hide-console`), pour les correctifs `*-kynput-*`.

- **SDL2** : `kynput.dll` se lie à la `SDL2.dll` de Kyber 0.29 (`third_party/payload/kyber-0.29.0`), qu'on ne recompile pas. `sdl2-sys` la cherche par pkg-config : le script écrit un `sdl2.pc` minimal (`third_party/toolchain-bin/pkgconfig-sdl2`) qui pointe ce dossier, et l'éditeur de liens de MinGW lie la DLL directement, sans bibliothèque d'import.
- **build.rs de kynputserver** : celui de Kyber teste `cfg!(target_os = "macos")`, c'est-à-dire la machine qui compile ; depuis un Mac, il assemble le code macOS avec MinGW et échoue. Le correctif 0012 teste `CARGO_CFG_TARGET_OS`.
- **Runtime** : comme le reste de la surcouche, `kynput.dll` et `kynputserver.exe` sont en UCRT, à côté de `kyclient.dll` et `kyclient.exe` officiels (msvcrt) qui chargent la même `kynput.dll`. Sans risque ici : l'API C de kynput libère toujours elle-même ce qu'elle alloue (`kynput_*_unref`, `kynput_free_string`). Elle exporte les fonctions de la version officielle, plus celles du presse-papiers si le correctif client 0002 est appliqué dans `third_party` au moment du build.

## Client Windows (apps/windows), depuis macOS

`apps/windows/scripts/build.sh` compile le cœur client (`apps/core`) et l'app Windows avec la chaîne du serveur (`x86_64-pc-windows-gnu`, MinGW de Homebrew), en s'appuyant sur ce que `server/kyber-patches/build.sh` a produit : FFmpeg et libopus statiques (`third_party/rootfs-win64`), `kynput.dll` (`third_party/rootfs-win64-kynput`).

- **Crates de Kyber** : l'app dépend directement de la crate Rust `kyclient` (pas de son API C). Ses dépendances se nomment par leur version crates.io : `apps/Cargo.toml` les renvoie vers `third_party/kyber-desktop/kysdk` (`[patch.crates-io]`, comme `kysdk/.cargo/config.toml`), et `apps/Cargo.lock` part de celui de `kysdk/kyctl` pour garder les mêmes versions.
- **VLC** : `kyvlcplayer` lie toujours `libvlc` sous Windows (`vlc-rs`, par pkg-config), même quand l'image et le son vont à l'app. Le script fabrique une `libvlc.a` de fonctions vides (une par export de `libvlc.dll.a`) et un `libvlc.pc` qui la désigne, dans `third_party/windows-client-x64`. Les bibliothèques statiques de FFmpeg sont liées depuis un dossier à part (`third_party/windows-client-x64/native`) : celui du rootfs contient aussi `libvlc.dll.a`, que l'éditeur de liens préférerait aux fonctions vides (l'exécutable dépendrait alors de `libvlc.dll`). Le script refuse un exécutable qui en dépend encore.
- **libavcodec** : son décodeur Opus appelle libswresample, à lier aussi.
- **Liaisons de FFmpeg** : générées par bindgen au build (`apps/windows/build.rs`) depuis les en-têtes du rootfs, avec les mêmes `BINDGEN_EXTRA_CLANG_ARGS` que kynput. `AVD3D11VADeviceContext` est recopiée à la main (des pointeurs) pour ne pas faire lire les en-têtes COM de Direct3D à bindgen.
- **Taille** : l'exécutable embarque les encodeurs de la libavcodec du serveur (x264, VPL) ; la copie de `dist/` est sans informations de débogage (36 Mo).
- **ARM64** (`--arch arm64`) : cible Rust `aarch64-pc-windows-gnullvm` (`rustup target add`) et chaîne [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) (version UCRT pour macOS, décompressée dans `third_party/toolchains`) : MinGW de Homebrew ne vise pas ARM64. `apps/windows/scripts/build-arm64-deps.sh` compile dans `third_party/rootfs-winarm64` ce que le serveur ne fournit pas pour ARM64 : libopus (`--disable-rtcd` : la détection du processeur n'existe pas pour Windows ARM, NEON est toujours là), un FFmpeg réduit au décodage (H.264, HEVC, AV1, accélération D3D11VA, sans x264 ni VPL : 16 Mo au lieu de 170), SDL2 2.32.8 (l'archive téléchargée et vérifiée par le script de kynput) et `kynput.dll` (`cargo cinstall --dlltool llvm-dlltool`). L'exécutable demande `libunwind.dll` de llvm-mingw, copiée dans `dist/arm64`.
- **set -e** : dans `make && make install`, un échec de `make` n'arrête pas le script (seule la dernière commande d'une liste `&&` compte). Les deux sont sur deux lignes, et le script vérifie les bibliothèques produites.
- **App Tauri** : Tauri **2.11.6** et tauri-build 2.6.3, compilés pour `windows-gnu` et `windows-gnullvm` depuis le Mac (comme `tao` et `wry` de l'installateur de la Station). Tauri 2.12 et ses briques demandent Rust 1.90 alors que KyberNow est vérifié en 1.89 : l'espace de travail `apps/` utilise le résolveur 3, qui choisit les versions compatibles avec `rust-version`. L'interface (`ui/`, Svelte et Vite, mêmes versions qu'Admin) est construite par `build.sh` avant cargo et embarquée dans l'exécutable ; `WebView2Loader.dll` (celle de `webview2-com-sys`, x64 ou arm64) est copiée à côté. L'icône (`resources/kybernow.ico`, symbole « now ») vient de `resources/make-ico.py`.
- **Installateur** (`apps/windows/setup`) : projet à part, avec son `Cargo.lock` copié de celui de l'installateur de la Station (`wry` 0.57, `tao` 0.37, `zstd`), construit pour x64 seulement (il tourne émulé sur ARM). L'interface est embarquée sans bibliothèque : `build.rs` écrit la liste des fichiers de `ui/dist` en `include_bytes!`. Le lanceur et le désinstalleur sont le même exécutable (sans son contenu pour `uninstall.exe`). `cmd /C` avec un chemin entre guillemets : la ligne doit passer par `raw_arg`, les arguments de Rust échappant les guillemets en `\"`, que cmd ne comprend pas (un désinstalleur ne pourrait alors pas effacer son dossier).
- **Tests du cœur sur le Mac** : `cargo test -p kybernow-core` avec `PKG_CONFIG_PATH` vers `third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig` (libvlc et kynput du client macOS de Kyber).

## Cœur client (apps/core) pour les cibles Apple, depuis macOS

`apps/core/scripts/build-apple.sh` compile la crate `kybernow-core` pour `aarch64-apple-darwin` (référence : le Mac qui compile), `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-apple-tvos` et `aarch64-apple-tvos-sim`, déploiement iOS 18 et tvOS 18 (`--target <cible>` pour une seule, `--check` pour `cargo check`). La crate est une rlib : le script vérifie que toute la chaîne de dépendances compile pour chaque cible et laisse `apps/target/<cible>/release/libkybernow_core.rlib` ; la bibliothèque statique et le xcframework viennent de la crate du pont C (`apps/apple/scripts/build-apple-mobile.sh`, voir plus haut) ; ce script-ci sert à vérifier la chaîne pour chaque cible et à construire libkynput pour iOS.

- **Cibles** : iOS est de rang 2 (`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`, Rust stable 1.89) ; tvOS de rang 3 (Rust nightly du dépôt et `-Zbuild-std=std,panic_abort`, exactement comme `build-kyber-tvos.sh` compile libkyclient et libkynput).
- **Mêmes sources pour toutes les cibles** : rien de propre à une cible dans les sources du cœur, `Cargo.toml` ni `Cargo.lock`. `kyvlcplayer` ne lie VLC que sur Android, Linux, macOS et Windows : sur iOS, tvOS et visionOS, `vlc-rs` est sans pkg-config, et les symboles `libvlc_*` que kyclient appelle sont fournis à l'édition de liens de l'app par des fonctions vides, comme sous Windows. `kyclient` avec `kymux`, `rustls` et `ring`, `ureq`, `getrandom` 0.2 et `minisign-verify` compilent tels quels pour tvOS et pour iOS.
- **kynput-sys** : `kynput-rs` lit l'API C de libkynput (bindgen sur `kynput.h`, trouvé par pkg-config), donc chaque cible a besoin d'un rootfs `third_party/kyber-desktop/rootfs-<cible>` avec `kynput.pc`, et de `PKG_CONFIG_ALLOW_CROSS=1`. Celui du Mac (`rootfs-arm64-macos`) vient de `vendor-kyber.sh` (il fournit aussi `libvlc.pc`, que `kyvlcplayer` lie sur macOS) ; ceux de l'Apple TV, de `build-kyber-tvos.sh` ; pour iOS, le script compile lui-même libkynput (`cargo cinstall -p kynput --library-type=staticlib`, sans la fonctionnalité `sdl2` : comme sur l'Apple TV, les manettes passent par GameController), et ferait de même pour une cible tvOS pas encore construite. Pour une rlib, cargo ne lie rien : seuls `kynput.pc` et l'en-tête servent ici ; `libkynput.a` sert au pont C pour iOS.
- **Cible de déploiement** : une seule variable à la fois (`IPHONEOS_DEPLOYMENT_TARGET` ou `TVOS_DEPLOYMENT_TARGET`, posée par cible). Le clang de bindgen refuse d'en voir deux dans l'environnement (« conflicting deployment targets »), même pour la cible macOS.
- **strip** : `CARGO_PROFILE_RELEASE_STRIP=false`, comme les autres scripts (voir « Client macOS Kyber 0.29 » plus haut).
- **Durées** (`apps/target` vide, registre cargo déjà rempli) : macOS 19 s, iOS 17 s, simulateur iOS 17 s, tvOS 27 s, simulateur tvOS 26 s, soit 1 min 46 s pour les cinq ; libkynput pour iOS, 5 s.
- **Vérifié avec** : `cargo test -p kybernow-core` sur le Mac (tests au vert) et `cargo check` de `apps/windows` pour `x86_64-pc-windows-gnu` (interface `ui/dist` construite d'abord, sinon `tauri::generate_context!` échoue).

## Moteur du client web (kyclient_wasm), depuis macOS

Le moteur que charge le client web vient du dépôt `kyber/apps/kyber-web` de Kyber (crate `kyclient-wasm`, l'API JS), pas de kyber-desktop. `server/kyber-patches/build.sh` le recompile avec les correctifs 0013 à 0015 et pose `kyclient_wasm.js` et `kyclient_wasm_bg.wasm` dans `webclient/` de la surcouche.

- **Sources** : `git clone --depth 1 --branch 0.29.0 https://gitlab.com/kyber/apps/kyber-web.git third_party/kyber-web`. Son sous-module `kysdk` est le même commit que celui de kyber-desktop 0.29 (`ebca32a`) : il n'est pas cloné, le script le remplace par un lien.
- **Outils** : `rustup target add wasm32-unknown-unknown`, `cargo install wasm-bindgen-cli --version 0.2.100 --locked` (la version doit être celle de la crate `wasm-bindgen` du `Cargo.lock` de kyber-web, sinon wasm-bindgen refuse le module), `brew install binaryen` (`wasm-opt`). Le reste du `build-wasm.sh` de kyber-web (pnpm, interface de Kyber) n'est pas nécessaire : l'interface ne change pas.
- **kyctl d'origine** : le `kysdk/kyctl` de `third_party` porte aussi les correctifs des apps, qui changent l'API de kyclient (le champ `packet_sink` de `AudioSessionConfig` casse la compilation de kyber-web). Le script compose donc `third_party/kyber-web-kysdk/` : `kyctl` extrait de git (`git archive HEAD`) plus les correctifs `*-kywebplayer-*`, et des liens vers `kymedia`, `kymux`, `kynput` et `kyutil` de `third_party`, correctifs serveur appliqués. `third_party/kyber-web/kysdk` pointe vers ce dossier.
- **Étapes** : `cargo build --target wasm32-unknown-unknown -p kyclient-wasm --release` (avec `RUSTFLAGS=--cfg=web_sys_unstable_apis`), `wasm-bindgen --target web --no-typescript`, puis `wasm-opt -Os -g`, comme le script de Kyber. Le résultat garde exactement l'API JS du moteur de Kyber 0.29 (mêmes classes, fonctions et méthodes) : la page `/ecran` n'a rien à changer.
