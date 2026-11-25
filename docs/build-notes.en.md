# Build notes

English · [Français](build-notes.md)

## Tool versions

KyberNow's binaries built on Kyber (server overlay, Rust core and libraries of the KyberNow Link apps) are built and checked with these versions. Those of Rust, cargo-c, MinGW, meson, llvm-mingw, wasm-bindgen and binaryen are written in `scripts/kyber-toolchain.sh`, read by the build scripts (server, Mac, Apple TV, Apple apps' core and bridge, Windows, Android, webOS, and `scripts/release.sh`): a different version stops nothing, it is reported at the start ("Attention : rustc 1.90.0, versions vérifiées : 1.89.0"). The Android NDK is checked by `build-native.sh` itself. After a checked update, change both places.

| Tool | Version | For |
|---|---|---|
| Rust stable (`rustc`) | 1.89.0, targets `x86_64-pc-windows-gnu`, `aarch64-apple-ios`, `aarch64-apple-ios-sim` | Windows server, macOS client, client core for iOS |
| Rust nightly | of 2026-10-19 (1.101.0), `rust-src` component | Apple TV client and client core for tvOS (`-Zbuild-std`) |
| cargo-c (`cargo cinstall`) | 0.10.15 | macOS and Apple TV clients, libkynput for iOS |
| mingw-w64 (Homebrew) | 14.0.0, GCC 16.2.0, UCRT | Windows server |
| meson | 1.11.2 | Windows server (dependencies, txproto) |
| llvm-mingw | 20261006 (UCRT, macOS), clang 23.1.3 | Windows ARM64 client, in `third_party/toolchains` |
| wasm-bindgen-cli | 0.2.100 (the one in kyber-web's `Cargo.lock`) | web client engine |
| binaryen (`wasm-opt`) | 133 | web client engine |
| Android NDK | r27c (27.2.12479018), checked with rustc 1.97.0 on Linux (failing that, `build-native.sh` uses the newest installed one, with a warning); Rust targets `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` | client core and JNI bridge for Android (`apps/android/scripts/build-native.sh`) |
| webOS toolchain | webosbrew's (`openlgtv/buildroot-nc4`, gcc 16.2, glibc 2.12, armv7 softfp), downloaded by `apps/webos/scripts/build.sh` into `third_party/webos-toolchain` on Linux x86_64; on an Apple Silicon Mac, `webosbrew/native-toolchain`'s (`webos-d7ed7ee.6`, darwin-arm64, gcc 12.2, glibc 2.12) into `third_party/webos-toolchain-mac` (the release toolchain); Rust target `armv7-unknown-linux-gnueabi`, checked with rustc 1.89.0 (Mac) and 1.97.0 (Linux); webosbrew SDL2 `release-2.30.12-webos.6` with the patches in `apps/webos/sdl-patches`; FFmpeg 7.1.1 reduced to the ac3 and eac3 encoders (libavcodec, libavutil, static) | webOS app (`apps/webos`) |
| Android SDK, Gradle | platform 36, Android Gradle Plugin 8.13.2, Gradle 8.14.3 (wrapper), Kotlin 2.4.21 (`gradle/libs.versions.toml`), JDK 17 or later | Android app (`apps/android`) |
| Linux, Flatpak | Rust 1.89 or later (`rust-version` in `apps/Cargo.toml`) and cargo-c; Flatpak: `org.freedesktop.Platform` 25.08 runtime, `rust-stable` and `llvm20` extensions; FFmpeg 7.1.1 reduced to decoding, or Raspberry Pi's (`jc-kynesim/rpi-ffmpeg`, `dev/7.1.1/rpi_import_1` branch) with `--rpi` | Linux, Steam Deck and Raspberry Pi app (`apps/linux/scripts/build.sh`, see [apps/linux/README.en.md](../apps/linux/README.en.md)) |

Kyber's repositories have no `rust-toolchain.toml`: rustup's default toolchain does the build.

## Build scripts: real failure

`server/kyber-patches/build.sh`, `apps/macos/scripts/build-kyclient.sh`, `apps/tvos/scripts/build-kyber-tvos.sh`, `apps/core/scripts/build-apple.sh`, `apps/apple/scripts/build-apple.sh`, `apps/apple/scripts/build-apple-mobile.sh`, `apps/android/scripts/build-native.sh` and `apps/webos/scripts/build.sh` stop at the first error and then replace nothing (overlay, `Vendor/`, `jniLibs/`, webOS binary):

- **cargo**: its output is filtered (progress lines removed) without the filter deciding the exit code (`set -o pipefail`). Do not put `| grep … || true` on the command itself: a failed build would go unnoticed and the old binary would go into the overlay.
- **Stale binary**: `build.sh` refuses `kyavserver.exe`, `libtxproto-0.dll`, `kycontroller.exe`, `kynput.dll`, `kynputserver.exe` and `kyclient_wasm.wasm` if they are older than its start (it resets the sources every time, so they are always rebuilt). `cargo cinstall` keeps the date of the library already built: the client scripts (and `build.sh` for `kynput.dll`) instead remove the library installed by a previous build before compiling, and the copy fails if this build did not install it again.
- **Patch that does not apply**: the rejected hunk is shown (`patch -r -` for the server, `git apply --check` for the clients).
- **Patch that adds a file**: `build.sh` resets each file of the server patches to the original, one by one; a file missing from the original is deleted instead of restored. No `git checkout -- .`: `kysdk/kyctl` and `kysdk/kymux` also carry the app patches.
- **App and server patches on the same file** (`kynput/Cargo.toml`: 0013 and the webOS patch): reset to the original, such a file would lose the app's part, and the app's script would then find its patch half applied, neither applicable nor removable (“Patch … does not apply”). `build.sh` therefore first removes the app patches that are fully applied (`apps/macos`, `apps/webos`), then applies them again after the server's; those of `apps/macos` are always applied, even if they were not: the server's stylus patch (0027-kynput) relies on the apps' one (0005-kynput, `Pen`), and on origin sources (a new build tree) kynput did not compile. An app patch already half applied stops `build.sh`: reset the repository (`git -C third_party/kyber-desktop/kysdk/kynput checkout -- .`) and run it again; the app scripts apply their patches again.
- **Client-side patches, all reapplied** (`scripts/kyber-client-patches.sh`): the apps' scripts (`build-kyclient.sh`, `build-kyber-tvos.sh`, `apps/core/scripts/build-apple.sh`, `build-apple-mobile.sh`, `build-native.sh`, `apps/windows/scripts/build.sh`, `apps/webos/scripts/build.sh`) first reset the files their patches touch to the original, then apply, as `apply.sh` does for the Station, the Station's patches for those repositories (kycontroller, kywebplayer, kynet, kynput), then their own: the apps are built on the same state as the Station. Before, they applied a patch only if it applied as is or was already applied: after a Station build (server patches on top), “0022 inapplicable” stopped the Mac app's build. The result no longer depends on what another build left behind; a Station build run afterwards reapplies its own. When the same patches are already applied to sources unchanged since (fingerprint in `kyber-desktop/.kybernow-correctifs-apps`), nothing is rewritten: file dates do not move, cargo does not rebuild Kyber.
- **Worktree**: `third_party/` (outside git) can be a link there to the main checkout's. The scripts use its real path (`pwd -P`): meson and cargo record the sources' absolute paths, and another path makes them reconfigure and rebuild everything.
- **App open**: `build-kyclient.sh` and `apps/apple/scripts/build-apple.sh` refuse to run while KyberNow is open (`pgrep -x "KyberNow( Link)?"`); `build-kyclient.sh` checks it at the start and right before copying into `Vendor/` (see "Embedding the libraries in the app without crashing it" below).

## Build trees

Kyber's patches are applied **inside** its sources: the Windows Station (`server/kyber-patches/apply.sh`), each app (`scripts/kyber-client-patches.sh`), the Linux app (its `build.sh` loop) and the Mac Station reset them to the origin then apply their own. As long as they shared `third_party/kyber-desktop/kysdk`, each left the sources in a different state, and the next builds (cargo, meson) started from a state that was not theirs: 0.11.0 had to be relaunched six times (the Mac Station's Kyber built before PyroWave, the server's libavcodec linked by the Windows app, libkynput never rebuilt, the Flatpak copy without `scripts/`, shared libraries rebuilt by other sessions).

`scripts/release.sh` now builds each variant in **its tree** (`scripts/arbres.sh`), under `third_party/arbres/` (ignored by git, seen from the VM as `/media/psf/kyber-apps/third_party/arbres/`):

| Tree | What is built in it | `third_party/` entries of its own |
|---|---|---|
| `station-windows` | Kyber overlay (`server/kyber-patches/build.sh`), web engine, Station installer and package | `kyber-desktop`, `kyber-web`, `kyber-web-kysdk`, `kyber-web-out`, `rootfs-win64`, `rootfs-win64-kynput`, `payload/kyber-overlay`, `pyrowave`, `toolchain-bin` |
| `windows-app` | Windows app x64 and ARM64, installer | `kyber-desktop`, `windows-client-x64`, `windows-client-arm64`, `pyrowave`; `rootfs-win64` and `payload` are `station-windows`'s (the libavcodec of the same release) |
| `android` | APK | `kyber-desktop`, `pyrowave` |
| `webos` | webOS package (webOS patches on top) | `kyber-desktop`, `sdl-webos`, `sdl-webos-dist` |
| `apple` | Mac app, SDKs and Apple TV and iPhone/iPad apps | `kyber-desktop` |
| `station-mac` | Kyber for Mac, app, `.dmg` and package of the Mac Station | `kyber-desktop-macos`; `kyber-web-out` is `station-windows`'s |
| `linux` | Flatpaks, Raspberry Pi `.deb`, KyberNow Link OS (in the VM) | `kyber-desktop`, `linux-aarch64`, `linux-aarch64-rpi`, `pyrowave` |
| `versions` | version alignment and the "KyberNow X.Y.Z" commit | — |

- **A tree** is a detached git worktree at the built commit (`git checkout --detach --force` then `git clean -fd`: ignored files, cargo, Xcode and Gradle caches, stay). Its `third_party/` is its own: the entries the variant writes are **APFS clones** (`cp -c`, ~13 s for `kyber-desktop` and its 107,000 files, taking no space until they change) made once from the shared `third_party/`, without their build folders (`target`, `builddir-*`, PyroWave's `build-*`: they point at the original paths, and ninja would rebuild the original sources) nor their fingerprints (`.kybernow-kynput`, `.kybernow-patches`, `kyber-overlay.stamp`: the tree's first build redoes everything); the paths in `.pc`, `.la` and CMake files are rewritten to the clone (Mac and VM forms). The other entries are **relative links** to the shared `third_party/`, only read (relative: the VM follows them too).
- **Kyber sources reset to the exact origin** at the start of each release (tracked files as in each repository's commit, files added by a known patch removed; nothing else: subprojects extracted by meson stay), **unless** they are still in the state recorded at the end of the previous build with the same patches (`.kybernow-arbre`): then nothing is rewritten, file dates do not move and cargo and meson do not rebuild. The scripts then apply their patches as by hand; `kyber_apply_client_patches` rewrites nothing either when the same patches are already applied to unchanged sources (`kyber-desktop/.kybernow-correctifs-apps`).
- **Caches kept** from one release to the next: cargo (`apps/target`, `kysdk/*/target`), meson (`builddir-win32`, `builddir-macos`), Xcode (`apps/*/build`), Gradle, PyroWave's CMake, flatpak-builder (`~/.cache/kybernow-flatpak-publication` in the VM, seeded from the hand builds' one). A first release rebuilds everything (an hour or more: FFmpeg and the dependencies of the Windows Station and the Mac Station, Android's three targets); the following ones only rebuild what changed.
- **By hand**, `scripts/arbres.sh liste` shows the trees (commit, lock); `scripts/arbres.sh preparer <variant> [REV]` creates or resets a tree, where a build script can be run as in the repository (`cd "$(scripts/arbres.sh chemin apple)" && apps/apple/scripts/build-apple.sh`). Scripts run from the repository or a worktree still patch the shared `third_party/kyber-desktop`, as before, but one build at a time (locks, below); the release no longer depends on it.
- **Space**: ~25 GB apparent per tree, a few real GB at first (APFS clones), then what builds add (`du` counts clones in full; `df` tells the truth). Removing a tree: `git worktree remove --force third_party/arbres/<variant>` (it will be recreated, builds to redo).

## Locks

`scripts/verrou.sh`: a `<path>.verrou` folder created atomically, with the process holding it (pid, start date, machine), the command and the time. Taken by the release (`third_party/arbres/publication` and each tree), by `kyber_apply_client_patches` (the `kyber-desktop` it patches), `server/kyber-patches/build.sh` and `server/macos/scripts/build-kyber.sh` (their sources), and by the scripts that write a `Vendor/` (`build-kyclient.sh`, `apps/apple/scripts/build-apple.sh`, `build-apple-mobile.sh`, `build-kyber-tvos.sh`). A second build on the same sources is refused at once, with the name of the one holding them (`KYBERNOW_VERROU_ATTENTE=<seconds>` to wait). A lock whose process no longer exists (or whose pid was reused: different start date) is taken over silently; a script started by the lock holder (`KYBERNOW_VERROUS` variable) does not take it again. Builds in the VM take no lock on their side: the release, on the Mac, holds the `linux` tree.

## Disk space

The release refuses to start under 40 GB free (note under 80): trees, step caches (`third_party/arbres/.etapes`, the KyberNow Link OS image included), `dist/`. In the VM, `release-image.sh` first removes pi-gen's old base images (`~/kybernow-os/deploy`, 3.6 GB each; only the most recent is used by `build-image.sh --ab-only`) then asks for 20 free; if space still runs out: `sudo rm -rf ~/kybernow-os/pi-gen/work`. The Flatpak source copy (`sources.sh`) no longer takes `apps/linux/dist` (packages and image: several GB that flatpak-builder copied every time).

## Kyber 0.29 macOS client (kyber-desktop) with Xcode 27

**Symptom**: the final link of `kyclient` fails:

```
ld: mis-aligned LINKEDIT string pool, fileOffset=0x006E8D6C in '.../rootfs-arm64-macos/lib/libkyclient.dylib'
```

**Cause**: `cargo-c` builds `libkyclient` with `-C strip=debuginfo`. On macOS, rustc 1.89 does this strip with `rust-objcopy` (LLVM), which leaves the string table aligned on 4 bytes. Xcode 27's linker (ld-27037) requires 8 bytes and refuses the library. Apple's `strip` (`/usr/bin/strip -S`) does not have this problem. `libkynput` is not affected (its alignment happens to fall right).

**Workaround**: turn off Cargo's strip for the whole build:

```bash
export CARGO_PROFILE_RELEASE_STRIP=false
./build-macos.sh -n -p -a arm64 client      # -s pour ne pas recompiler VLC/FFmpeg
```

(`-s` skips rebuilding VLC/FFmpeg.)

Checking a library (must print 0):

```bash
s=$(otool -l rootfs-arm64-macos/lib/libkyclient.0.1.0.dylib | awk '/stroff/{print $2}'); echo $((s % 8))
```

KyberNow's build scripts set `CARGO_PROFILE_RELEASE_STRIP=false`, except `apps/android/scripts/build-native.sh` and `apps/windows/scripts/build.sh` (ELF libraries and PE executables, which the problem does not affect: they remove the debug information themselves, `llvm-strip` and `strip`); the `apps/` workspace has `strip = false` in its release profile anyway.

### libkyclient with the KyberNow patches

`apps/macos/scripts/build-kyclient.sh` applies `apps/macos/kyber-patches/*.patch`, then `server/kyber-patches/*-kyclient-*.patch` (client side of the Station's patches, 0022), to the repository each file names (`kysdk/kyctl`, `kysdk/kynput`, `kysdk/kymux`), rebuilds libkynput and libkyclient (`cargo cinstall`, a few seconds) and replaces them in `apps/macos/Vendor/`. Run it again after each `vendor-kyber.sh`. It first installs them into the rootfs of `third_party/kyber-desktop/rootfs-arm64-macos`, shared by every checkout (a worktree reaches it through its `third_party` symbolic link): run from a worktree, it rewrites the libkynput that the core's tests and every other checkout's `build-apple.sh` use. Since the “several computers” work, the macOS libkynput no longer has an SDL gamepad reader (the core reads gamepads): that is intended once everything is merged, but an older checkout that rebuilds its Apple core against this rootfs loses its gamepads until its own merge (and conversely, a `build-kyclient.sh` from before that work puts kynput's reader back, which the core then leaves alone). `vendor-kyber.sh` also copies Opus's headers (`COpus` module): the sound is decoded by the app.

- **strip**: with the release profile's default `strip`, Xcode 27's linker refuses the library (“mis-aligned LINKEDIT string pool”). The script builds with `CARGO_PROFILE_RELEASE_STRIP=false`.

### Embedding the libraries in the app without crashing it

`apps/macos/scripts/embed-kyber.sh` (Xcode build phase) signs a working copy of `Vendor/Frameworks` (in `DERIVED_FILE_DIR`), re-signing only what has changed. It copies into the app only the files that differ.

- **Why**: rewriting the app's libraries while it runs makes VLC crash at the start of the next session (`config_GetInt` on NULL, in `libsyslog_plugin`). Its modules, loaded on demand from the new files, link a new `libvlccore` file while the old one is already in memory: two VLC cores coexist.
- **After `build-kyclient.sh` or `build-apple.sh`**: the libraries really change. Quit the app before rebuilding, then relaunch it.
- **Outside a `.app` bundle** (test program launched next to `Vendor/Frameworks`): VLC also looks for its modules in the folder where it was built (`rootfs-arm64-macos/lib/vlc/plugins`), with the same crash. In the app, it only takes `Contents/Frameworks/plugins`.

### The Rust core of the Apple apps (apps/apple)

`apps/apple/scripts/build-apple.sh` builds the `kybernow-apple` crate (a hand-written C API over `kybernow-core`, see [coeur-apple.en.md](coeur-apple.en.md)) with `cargo cinstall` for `aarch64-apple-darwin`, places `libkybernow.dylib` in `apps/macos/Vendor/Frameworks` (embedded by `embed-kyber.sh` like the others), the `kybernow.h` header produced by cbindgen and a `CKyberNow` Clang module in `apps/macos/Vendor/KyberNow/include` (`SWIFT_INCLUDE_PATHS` in `project.yml`).

- **`capi`**: cargo-c requires a feature of that name in the crate, even empty (`[features] capi = []`), otherwise “package does not contain this feature: capi”.
- **Constants**: `item_types` in `cbindgen.toml` must name `constants`, or the `KN_*` are missing from the header. They are `#define`s, which Swift imports as `Int32`: compare with `Int32(kind)`, not with the struct's `uint32_t`.
- **Paths**: cargo-c writes an absolute `install_name`; the script brings it back to `@rpath/libkybernow.dylib`, and its dependencies (`libvlc`, `libkynput.0.1`) to `@rpath`.
- **App open**: the script refuses to run while KyberNow is open (see above); `KYBERNOW_IGNORE_RUNNING=1` overrides it when the target `Vendor/` is not the running app's (another worktree).
- **libkyclient**: `libkybernow` embeds the code of the `kyclient` Rust crate. Nothing calls libkyclient's C API; the `CKyber` module still links `libkyclient.dylib` on Mac (embedded, never called), and is used for libkynput's headers (view layout, log).
- **tvOS and iOS** (`apps/apple/scripts/build-apple-mobile.sh tvos|ios`): `cargo cinstall --library-type staticlib` per target (tvOS: Rust nightly and `-Zbuild-std=std,panic_abort`, like `build-kyber-tvos.sh`; iOS: stable), with the target's rootfs for `kynput.pc` (`apps/core/scripts/build-apple.sh` builds it when needed), then `xcodebuild -create-xcframework` into `apps/<platform>/Vendor/KyberNow.xcframework`; the `CKyberNow` module carries no `link` there (static library, linked through the xcframework dependency). On tvOS, libkynput, libopus and VLC's empty functions come from `KyberSDK.xcframework`: the linker takes each symbol from the first library defining it, and `-dead_strip` drops the rest; two static Rust libraries (the core, libkynput) live side by side. For iOS, there is no separate Kyber SDK: libkynput (the target's rootfs), libopus (built with cmake from kymedia's sources, into the rootfs) and VLC's empty functions (generated from the library's `nm -u`, as for the Apple TV) join the core in the same library (`libtool -static`), and `kynput.h` goes into `Vendor/include` (`CKyber` module). The iOS app's strings are extracted with `xcrun xcstringstool sync` from the build's `.stringsdata` (`xcodebuild` does not fill the catalogue by itself), then `apps/macos/scripts/check-strings.py --fix`. About 127 MB per target with debug info.

## Browsers

Kyber's own web client page works in Chrome; in Safari, the screen stays black (WebKit is not supported, as Kyber's docs state). KyberNow's web client (`/link`) also works in Safari and on iPhone and iPad, over WebSocket (their WebTransport does not check a certificate by its fingerprint): see [server/README.en.md](../server/README.en.md).

## Kyber 0.29 server for Windows (kyavserver), from macOS

`server/kyber-patches/build.sh` runs everything in sequence (versions: see above). Pitfalls met with Homebrew's MinGW toolchain:

- **luac**: txproto compiles its Lua scripts during the build and wants to run `luac.exe`. You need a native `luac` 5.4 in the PATH (`brew install lua@5.4`); the bytecode is the same (64-bit, little-endian).
- **Jinja**: the libplacebo archive does not include its submodules. Copy `jinja2` and `markupsafe` (pip wheels) into `subprojects/libplacebo-*/3rdparty/{jinja,markupsafe}/src`.
- **PyroWave** (patch 0025, [pyrowave.en.md](pyrowave.en.md)): `server/kyber-patches/pyrowave.sh` clones PyroWave at the pinned commit into `third_party/pyrowave` (with the partial Granite of its `checkout_granite.sh`), builds it with cmake and MinGW (`libpyrowave-shared-1.dll`, `pyrowave-device-validation.exe`) and installs it into `third_party/rootfs-win64` with `pyrowave-shared.pc`, before FFmpeg, which links to it. meson extracts FFmpeg only once: when `ffmpeg.wrap` or `packagefiles/ffmpeg` change (0025 adds a patch there), `build.sh` deletes `subprojects/FFmpeg-n8.1` and its build folder so that it is extracted and configured again (`.kybernow-wrap` marker). In a fresh rootfs, FFmpeg's first configuration can fail (x264 and the others not installed yet): run it again. Requirement: cmake (Homebrew).
- **bison**: macOS's (2.3) is too old for VLC; use `/opt/homebrew/opt/bison/bin`.
- **pkg-config**: meson expects `x86_64-w64-mingw32-pkg-config`; a simple wrapper to `pkg-config` is enough (the rootfs is set by the cross file).
- **bindgen**: `BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu --sysroot=<mingw> -I<mingw>/include"`, otherwise `errno.h` cannot be found.
- **Runtime**: Homebrew's MinGW produces UCRT binaries, official Kyber uses msvcrt. So we replace the whole kyavserver chain (see the overlay), never a single DLL. Safe exception: `kynput.dll`, see below.

## Kyber 0.29 input server for Windows (kynput), from macOS

`server/kyber-patches/build.sh` also rebuilds `kynput.dll` (`cargo cinstall`, `sdl2` feature as in the official release, installed into `third_party/rootfs-win64-kynput`) and `kynputserver.exe` (`win32-hide-console`), for the `*-kynput-*` patches.

- **SDL2**: `kynput.dll` links against Kyber 0.29's `SDL2.dll` (`third_party/payload/kyber-0.29.0`), which is not rebuilt. `sdl2-sys` looks for it through pkg-config: the script writes a minimal `sdl2.pc` (`third_party/toolchain-bin/pkgconfig-sdl2`) pointing to that folder, and MinGW's linker links the DLL directly, without an import library.
- **kynputserver's build.rs**: Kyber's tests `cfg!(target_os = "macos")`, that is the machine doing the build; from a Mac, it assembles the macOS code with MinGW and fails. Patch 0012 tests `CARGO_CFG_TARGET_OS`.
- **Runtime**: like the rest of the overlay, `kynput.dll` and `kynputserver.exe` are UCRT, next to the official `kyclient.dll` and `kyclient.exe` (msvcrt) which load the same `kynput.dll`. No risk here: kynput's C API always frees what it allocates itself (`kynput_*_unref`, `kynput_free_string`). It exports the official release's functions, plus the clipboard ones if client patch 0002 is applied in `third_party` at build time.

## Kyber 0.29 server for Mac (KyberNow Station for Mac)

`server/macos/scripts/build-kyber.sh` builds kycontroller, kyavserver (txproto: ScreenCaptureKit capture, VideoToolbox encoding), kynputserver and their libraries for `aarch64-apple-darwin`, with **the same patches as the Windows Station**: applying them is shared, in `server/kyber-patches/apply.sh` (sourced by `build.sh` and by the Mac script), then those in `server/macos/kyber-patches` if any. Sources: **`third_party/kyber-desktop-macos`**, a separate copy of kyber-desktop (2026-11-03, when the Mac Station was merged): its Mac patches and its `rootfs-arm64-macos` don't mix with those of the Windows Station and of the Mac app's libraries (`build-kyclient.sh`, `vendor-kyber.sh`), which share `third_party/kyber-desktop`. `KYBER_DIR` names another one. Output: `third_party/kyber-desktop-macos/rootfs-arm64-macos`, which `run-kyber.sh package` and `build-station-app.sh` package; `release.sh` checks it is there. After a new Mac patch, run `build-kyber.sh` again: `run-kyber.sh package` doesn't rebuild libtxproto. `server/macos/scripts/run-kyber.sh start|stop|status` turns it into a signed bundle and starts it for tests ([docs/station-macos.en.md](station-macos.en.md)).

- **Separate sources**: the script resets the patched files to their origin, like `build.sh`. Do not run it on the `third_party` of a release in progress: in a worktree, copy `third_party/kyber-desktop` without the `target`, `builddir-*` and `rootfs-*` folders (2 GB), then `git reset --hard` in each submodule of the copy.
- **cargo**: rustup's comes from Homebrew (`/opt/homebrew/opt/rustup/bin`), not `~/.cargo/bin`. Without it, Kyber's `build-macos.sh` stops at "cargo: command not found" **and returns 0**: the script therefore checks that kycontroller, kyavserver and kynputserver come from this build.
- **Screen Recording**: started from a terminal (or by Claude), kyavserver is attributed to the app that opened the terminal; without its permission, ScreenCaptureKit returns no display ("SCK getShareableContent error", `Got initial displays: []`) or the session stalls after the encoder is created. `run-kyber.sh` therefore starts a bundle (`fr.leslignesclaires.kybernow.station.kyber`, "KyberNow Station (Kyber)") as a LaunchAgent: permission is asked for it, and the Developer ID signature keeps it across builds. After granting it, restart (`launchctl kickstart -k`).
- **Bundle signature**: Kyber's `package_server` leaves the certificates and `kyber_config.toml` in `Contents/MacOS`, which `codesign` refuses ("code object is not signed at all"); `run-kyber.sh` moves them to `Resources`.
- **Patched crates** (`server/macos/crates/<crate>-<version>-*.patch`): the script copies from cargo's registry the version locked by `kysdk/kyctl/Cargo.lock` (it stops if that version changed), applies the patch in `target/kybernow-crates/<crate>` and adds it to `kysdk/.cargo/config.toml` (reset to the original at each build). `cargo fetch` runs from `kysdk/kyctl`: cargo reads its config from the current folder, not from `--manifest-path`. `kysdk/kyctl/Cargo.lock` is reset to the original first: a previous build's points at the patched copy, and cargo, without it, resolved it again to a newer version (quinn-udp 0.5.16). A file a Mac patch creates (`0003`: `host_cursor.rs`) is removed before being applied again. The Mac patches' files are reset to the original only once, before the first: two patches may touch the same file (`0003` and `0004`: `mod.rs`, `pipeline_config.rs`), the second applies on top of the first.
- **Patches**: the 30 server patches apply and build as they are (the DXGI, WGC and WASAPI ones only touch Windows code); `*-kyctlweb-*` and `*-kyberweb-*` stay with the web engine.

## Windows client (apps/windows), from macOS

`apps/windows/scripts/build.sh` builds the client core (`apps/core`) and the Windows app with the server's toolchain (`x86_64-pc-windows-gnu`, Homebrew MinGW), relying on what `server/kyber-patches/build.sh` produced: static FFmpeg and libopus (`third_party/rootfs-win64`).

- **The client's kynput.dll**: built by the script itself, for x64 as for ARM64 (`cargo cinstall --library-type cdylib`, into `third_party/windows-client-<arch>/kynput`, with its own build folder: `kysdk/kynput`'s serves the server, with other options), without the `sdl2` feature (the core reads gamepads once for every session) and with `--cfg kybernow_app_clipboard` (client patch 0004: no kynput clipboard, the core holds its own). The server's (`third_party/rootfs-win64-kynput`) does not change: it serves the Station and the official Kyber client installed on the computer. Harmless cargo-c warning: `--dlltool` is now built in.
- **SDL2 for the core**: `kybernow-core`'s `sdl` feature (gamepad reader) links SDL2 through pkg-config; the script writes a minimal `sdl2.pc` into `third_party/windows-client-<arch>/pkgconfig`, pointing to a folder that only holds it (`…/sdl2`: Kyber 0.29's `SDL2.dll` on x64, which MinGW's linker links directly; the import library `libSDL2.dll.a` on ARM64, since lld does not link a DLL). Kyber's folder also holds `libvlc.dll`, which would take the place of VLC's empty functions.

- **Kyber's crates**: the app depends directly on the Rust crate `kyclient` (not its C API). Its dependencies name each other by their crates.io version: `apps/Cargo.toml` points them to `third_party/kyber-desktop/kysdk` (`[patch.crates-io]`, like `kysdk/.cargo/config.toml`), and `apps/Cargo.lock` starts from `kysdk/kyctl`'s to keep the same versions.
- **VLC**: `kyvlcplayer` always links `libvlc` on Windows (`vlc-rs`, through pkg-config), even when image and sound go to the app. The script makes a `libvlc.a` of empty functions (one per export of `libvlc.dll.a`) and a `libvlc.pc` pointing to it, in `third_party/windows-client-x64`. FFmpeg's static libraries are linked from a separate folder (`third_party/windows-client-x64/native`): the rootfs one also holds `libvlc.dll.a`, which the linker would prefer to the empty functions (the executable would then depend on `libvlc.dll`). The script refuses an executable that still depends on it.
- **libavcodec**: its Opus decoder calls libswresample, which must be linked too.
- **FFmpeg bindings**: generated by bindgen at build time (`apps/windows/build.rs`) from the rootfs headers, with the same `BINDGEN_EXTRA_CLANG_ARGS` as kynput. `AVD3D11VADeviceContext` is copied by hand (pointers) so that bindgen does not read Direct3D's COM headers.
- **Size**: the executable carries the encoders of the server's libavcodec (x264, VPL); the `dist/` copy has no debug information (36 MB).
- **ARM64** (`--arch arm64`): Rust target `aarch64-pc-windows-gnullvm` (`rustup target add`) and the [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) toolchain (UCRT build for macOS, unpacked into `third_party/toolchains`): Homebrew's MinGW does not target ARM64. `apps/windows/scripts/build-arm64-deps.sh` builds into `third_party/rootfs-winarm64` what the server does not provide for ARM64: libopus (`--disable-rtcd`: CPU detection does not exist for Windows ARM, NEON is always there), an FFmpeg reduced to decoding (H.264, HEVC, AV1, D3D11VA acceleration, without x264 or VPL: 16 MB instead of 170), and SDL2 2.32.8 (the archive downloaded and verified by kynput's script); `kynput.dll` is built by `build.sh`, as on x64 (`--dlltool llvm-dlltool`). The executable needs llvm-mingw's `libunwind.dll`, copied into `dist/arm64`.
- **set -e**: in `make && make install`, a failing `make` does not stop the script (only the last command of an `&&` list counts). Both are on two lines, and the script checks the libraries produced.
- **Tauri app**: Tauri **2.11.6** and tauri-build 2.6.3, built for `windows-gnu` and `windows-gnullvm` from the Mac (like the Station installer's `tao` and `wry`). Tauri 2.12 and its crates need Rust 1.90 while KyberNow is verified with 1.89: the `apps/` workspace uses resolver 3, which picks versions compatible with `rust-version`. The interface (`ui/`, Svelte and Vite, same versions as Admin) is built by `build.sh` before cargo and embedded in the executable; `WebView2Loader.dll` (the one from `webview2-com-sys`, x64 or arm64) is copied next to it. The icon (`resources/kybernow.ico`, “now” symbol) comes from `resources/make-ico.py`.
- **Installer** (`apps/windows/setup`): a separate project, with its `Cargo.lock` copied from the Station installer's (`wry` 0.57, `tao` 0.37, `zstd`), built for x64 only (it runs emulated on ARM). The interface is embedded without a library: `build.rs` writes the list of `ui/dist` files as `include_bytes!`. The launcher and the uninstaller are the same executable (without its contents for `uninstall.exe`). `cmd /C` with a quoted path: the line must go through `raw_arg`, since Rust's arguments escape quotes as `\"`, which cmd does not understand (an uninstaller could then not delete its folder).
- **Core tests on the Mac**: `cargo test -p kybernow-core` with `PKG_CONFIG_PATH` pointing to `third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig` (libvlc and kynput of Kyber's macOS client).

## Client core (apps/core) for the Apple targets, from macOS

`apps/core/scripts/build-apple.sh` builds the `kybernow-core` crate for `aarch64-apple-darwin` (reference: the Mac doing the build), `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-apple-tvos` and `aarch64-apple-tvos-sim`, deployment iOS 18 and tvOS 18 (`--target <target>` for a single one, `--check` for `cargo check`). The crate is an rlib: the script checks that the whole dependency chain builds for each target and leaves `apps/target/<target>/release/libkybernow_core.rlib`; the static library and the xcframework come from the C bridge crate (`apps/apple/scripts/build-apple-mobile.sh`, see above); this script checks the chain for each target and builds libkynput for iOS.

- **Targets**: iOS is tier 2 (`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`, Rust stable 1.89); tvOS is tier 3 (the repository's Rust nightly and `-Zbuild-std=std,panic_abort`, exactly as `build-kyber-tvos.sh` builds libkyclient and libkynput).
- **Same sources for every target**: nothing target-specific in the core's sources, `Cargo.toml` or `Cargo.lock`. `kyvlcplayer` links VLC only on Android, Linux, macOS and Windows: on iOS, tvOS and visionOS, `vlc-rs` is without pkg-config, and the `libvlc_*` symbols kyclient calls are provided when linking the app, by empty functions, as on Windows. `kyclient` with `kymux`, `rustls` and `ring`, `ureq`, `getrandom` 0.2 and `minisign-verify` build as they are for tvOS and for iOS.
- **kynput-sys**: `kynput-rs` reads libkynput's C API (bindgen on `kynput.h`, found through pkg-config), so each target needs a rootfs `third_party/kyber-desktop/rootfs-<target>` with `kynput.pc`, and `PKG_CONFIG_ALLOW_CROSS=1`. The Mac's (`rootfs-arm64-macos`) comes from `vendor-kyber.sh` (it also provides `libvlc.pc`, which `kyvlcplayer` links on macOS); the Apple TV's from `build-kyber-tvos.sh`; for iOS, the script builds libkynput itself (`cargo cinstall -p kynput --library-type=staticlib`, without the `sdl2` feature: as on the Apple TV, gamepads go through GameController), and would do the same for a tvOS target not built yet. For an rlib, cargo links nothing: only `kynput.pc` and the header matter here; `libkynput.a` serves the C bridge for iOS.
- **Deployment target**: one variable at a time (`IPHONEOS_DEPLOYMENT_TARGET` or `TVOS_DEPLOYMENT_TARGET`, set per target). bindgen's clang refuses to see two of them in the environment ("conflicting deployment targets"), even for the macOS target.
- **strip**: `CARGO_PROFILE_RELEASE_STRIP=false`, like the other scripts (see "Kyber 0.29 macOS client" above).
- **Durations** (empty `apps/target`, cargo registry already filled): macOS 19 s, iOS 17 s, iOS simulator 17 s, tvOS 27 s, tvOS simulator 26 s, 1 min 46 s for all five; libkynput for iOS, 5 s.
- **Checked with**: `cargo test -p kybernow-core` on the Mac (tests green) and `cargo check` of `apps/windows` for `x86_64-pc-windows-gnu` (the `ui/dist` interface built first, otherwise `tauri::generate_context!` fails).

## Web client engine (kyclient_wasm), from macOS

The engine the web client loads comes from Kyber's `kyber/apps/kyber-web` repository (`kyclient-wasm` crate, the JS API), not from kyber-desktop. `server/kyber-patches/build.sh` rebuilds it with patches 0013 to 0015 and lays `kyclient_wasm.js` and `kyclient_wasm_bg.wasm` into the overlay's `webclient/`.

- **Sources**: `git clone --depth 1 --branch 0.29.0 https://gitlab.com/kyber/apps/kyber-web.git third_party/kyber-web`. Its `kysdk` submodule is the same commit as kyber-desktop 0.29's (`ebca32a`): it is not cloned, the script replaces it with a link.
- **Tools**: `rustup target add wasm32-unknown-unknown`, `cargo install wasm-bindgen-cli --version 0.2.100 --locked` (the version must be the one of the `wasm-bindgen` crate in kyber-web's `Cargo.lock`, otherwise wasm-bindgen refuses the module), `brew install binaryen` (`wasm-opt`). The rest of kyber-web's `build-wasm.sh` (pnpm, Kyber's interface) is not needed: the interface does not change.
- **Original kyctl**: `third_party`'s `kysdk/kyctl` also carries the apps' patches, which change kyclient's API (the `packet_sink` field of `AudioSessionConfig` breaks kyber-web's build). So the script composes `third_party/kyber-web-kysdk/`: `kyctl` extracted from git (`git archive HEAD`) plus the `*-kywebplayer-*` patches, and links to `third_party`'s `kymedia`, `kymux`, `kynput` and `kyutil`, server patches applied. `third_party/kyber-web/kysdk` points to that folder.
- **Steps**: `cargo build --target wasm32-unknown-unknown -p kyclient-wasm --release` (with `RUSTFLAGS=--cfg=web_sys_unstable_apis`), `wasm-bindgen --target web --no-typescript`, then `wasm-opt -Os -g`, as Kyber's script does. The result keeps exactly the JS API of Kyber 0.29's engine (same classes, functions and methods): the `/link` page has nothing to change.
