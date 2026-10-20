# Notes de build

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

`server/kyber-patches/build.sh` enchaîne tout. Pièges rencontrés avec la chaîne MinGW de Homebrew :

- **luac** : txproto compile ses scripts Lua pendant le build et voulait lancer `luac.exe`. Il faut un `luac` 5.4 natif dans le PATH (`brew install lua@5.4`) ; le bytecode est le même (64 bits, petit-boutiste).
- **Jinja** : l'archive de libplacebo n'inclut pas ses sous-modules. Copier `jinja2` et `markupsafe` (roues pip) dans `subprojects/libplacebo-*/3rdparty/{jinja,markupsafe}/src`.
- **bison** : celui de macOS (2.3) est trop ancien pour VLC ; utiliser `/opt/homebrew/opt/bison/bin`.
- **pkg-config** : meson attend `x86_64-w64-mingw32-pkg-config` ; un simple relais vers `pkg-config` suffit (le rootfs est imposé par le fichier croisé).
- **bindgen** : `BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu --sysroot=<mingw> -I<mingw>/include"`, sinon `errno.h` est introuvable.
- **Runtime** : MinGW Homebrew produit des binaires UCRT, Kyber officiel est en msvcrt. On remplace donc toute la chaîne de kyavserver (voir la surcouche), jamais une DLL isolée.
