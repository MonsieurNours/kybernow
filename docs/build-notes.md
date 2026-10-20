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

Les dépôts de Kyber n'ont pas de `rust-toolchain.toml` : c'est la chaîne par défaut de rustup qui compile.

## Scripts de compilation : échec réel

`server/kyber-patches/build.sh`, `apps/macos/scripts/build-kyclient.sh` et `apps/tvos/scripts/build-kyber-tvos.sh` s'arrêtent à la première erreur et ne remplacent alors rien (surcouche, `Vendor/`) :

- **cargo** : sa sortie est filtrée (lignes de progression retirées) sans que le filtre décide du code de retour (`set -o pipefail`). Avant, `| grep … || true` masquait une compilation ratée et l'ancien binaire partait dans la surcouche.
- **Binaire périmé** : `build.sh` refuse `kyavserver.exe`, `libtxproto-0.dll` et `kycontroller.exe` s'ils sont plus anciens que son lancement (il remet les sources à l'origine à chaque fois, donc ils sont toujours recompilés). `cargo cinstall` garde la date de la bibliothèque déjà compilée : les scripts client retirent plutôt la bibliothèque installée par un build précédent avant de compiler, et la copie échoue si ce build ne l'a pas réinstallée.
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
- **Runtime** : MinGW Homebrew produit des binaires UCRT, Kyber officiel est en msvcrt. On remplace donc toute la chaîne de kyavserver (voir la surcouche), jamais une DLL isolée.
