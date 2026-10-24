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
- **Worktree**: `third_party/` (outside git) can be a link there to the main checkout's. The scripts use its real path (`pwd -P`): meson and cargo record the sources' absolute paths, and another path makes them reconfigure and rebuild everything.
- **App open**: `build-kyclient.sh` and `apps/apple/scripts/build-apple.sh` refuse to run while KyberNow is open (`pgrep -x "KyberNow( Link)?"`); `build-kyclient.sh` checks it at the start and right before copying into `Vendor/` (see "Embedding the libraries in the app without crashing it" below).

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

`apps/macos/scripts/build-kyclient.sh` applies `apps/macos/kyber-patches/*.patch` to the repository each file names (`kysdk/kyctl`, `kysdk/kynput`, `kysdk/kymux`), rebuilds libkynput and libkyclient (`cargo cinstall`, a few seconds) and replaces them in `apps/macos/Vendor/`. Run it again after each `vendor-kyber.sh`. `vendor-kyber.sh` also copies Opus's headers (`COpus` module): the sound is decoded by the app.

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

Kyber's own web client page works in Chrome; in Safari, the screen stays black (WebKit is not supported, as Kyber's docs state). KyberNow's web client (`/ecran`) also works in Safari and on iPhone and iPad, over WebSocket (their WebTransport does not check a certificate by its fingerprint): see [server/README.en.md](../server/README.en.md).

## Kyber 0.29 server for Windows (kyavserver), from macOS

`server/kyber-patches/build.sh` runs everything in sequence (versions: see above). Pitfalls met with Homebrew's MinGW toolchain:

- **luac**: txproto compiles its Lua scripts during the build and wants to run `luac.exe`. You need a native `luac` 5.4 in the PATH (`brew install lua@5.4`); the bytecode is the same (64-bit, little-endian).
- **Jinja**: the libplacebo archive does not include its submodules. Copy `jinja2` and `markupsafe` (pip wheels) into `subprojects/libplacebo-*/3rdparty/{jinja,markupsafe}/src`.
- **bison**: macOS's (2.3) is too old for VLC; use `/opt/homebrew/opt/bison/bin`.
- **pkg-config**: meson expects `x86_64-w64-mingw32-pkg-config`; a simple wrapper to `pkg-config` is enough (the rootfs is set by the cross file).
- **bindgen**: `BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu --sysroot=<mingw> -I<mingw>/include"`, otherwise `errno.h` cannot be found.
- **Runtime**: Homebrew's MinGW produces UCRT binaries, official Kyber uses msvcrt. So we replace the whole kyavserver chain (see the overlay), never a single DLL. Safe exception: `kynput.dll`, see below.

## Kyber 0.29 input server for Windows (kynput), from macOS

`server/kyber-patches/build.sh` also rebuilds `kynput.dll` (`cargo cinstall`, `sdl2` feature as in the official release, installed into `third_party/rootfs-win64-kynput`) and `kynputserver.exe` (`win32-hide-console`), for the `*-kynput-*` patches.

- **SDL2**: `kynput.dll` links against Kyber 0.29's `SDL2.dll` (`third_party/payload/kyber-0.29.0`), which is not rebuilt. `sdl2-sys` looks for it through pkg-config: the script writes a minimal `sdl2.pc` (`third_party/toolchain-bin/pkgconfig-sdl2`) pointing to that folder, and MinGW's linker links the DLL directly, without an import library.
- **kynputserver's build.rs**: Kyber's tests `cfg!(target_os = "macos")`, that is the machine doing the build; from a Mac, it assembles the macOS code with MinGW and fails. Patch 0012 tests `CARGO_CFG_TARGET_OS`.
- **Runtime**: like the rest of the overlay, `kynput.dll` and `kynputserver.exe` are UCRT, next to the official `kyclient.dll` and `kyclient.exe` (msvcrt) which load the same `kynput.dll`. No risk here: kynput's C API always frees what it allocates itself (`kynput_*_unref`, `kynput_free_string`). It exports the official release's functions, plus the clipboard ones if client patch 0002 is applied in `third_party` at build time.

## Windows client (apps/windows), from macOS

`apps/windows/scripts/build.sh` builds the client core (`apps/core`) and the Windows app with the server's toolchain (`x86_64-pc-windows-gnu`, Homebrew MinGW), relying on what `server/kyber-patches/build.sh` produced: static FFmpeg and libopus (`third_party/rootfs-win64`), `kynput.dll` (`third_party/rootfs-win64-kynput`).

- **Kyber's crates**: the app depends directly on the Rust crate `kyclient` (not its C API). Its dependencies name each other by their crates.io version: `apps/Cargo.toml` points them to `third_party/kyber-desktop/kysdk` (`[patch.crates-io]`, like `kysdk/.cargo/config.toml`), and `apps/Cargo.lock` starts from `kysdk/kyctl`'s to keep the same versions.
- **VLC**: `kyvlcplayer` always links `libvlc` on Windows (`vlc-rs`, through pkg-config), even when image and sound go to the app. The script makes a `libvlc.a` of empty functions (one per export of `libvlc.dll.a`) and a `libvlc.pc` pointing to it, in `third_party/windows-client-x64`. FFmpeg's static libraries are linked from a separate folder (`third_party/windows-client-x64/native`): the rootfs one also holds `libvlc.dll.a`, which the linker would prefer to the empty functions (the executable would then depend on `libvlc.dll`). The script refuses an executable that still depends on it.
- **libavcodec**: its Opus decoder calls libswresample, which must be linked too.
- **FFmpeg bindings**: generated by bindgen at build time (`apps/windows/build.rs`) from the rootfs headers, with the same `BINDGEN_EXTRA_CLANG_ARGS` as kynput. `AVD3D11VADeviceContext` is copied by hand (pointers) so that bindgen does not read Direct3D's COM headers.
- **Size**: the executable carries the encoders of the server's libavcodec (x264, VPL); the `dist/` copy has no debug information (36 MB).
- **ARM64** (`--arch arm64`): Rust target `aarch64-pc-windows-gnullvm` (`rustup target add`) and the [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) toolchain (UCRT build for macOS, unpacked into `third_party/toolchains`): Homebrew's MinGW does not target ARM64. `apps/windows/scripts/build-arm64-deps.sh` builds into `third_party/rootfs-winarm64` what the server does not provide for ARM64: libopus (`--disable-rtcd`: CPU detection does not exist for Windows ARM, NEON is always there), an FFmpeg reduced to decoding (H.264, HEVC, AV1, D3D11VA acceleration, without x264 or VPL: 16 MB instead of 170), SDL2 2.32.8 (the archive downloaded and verified by kynput's script) and `kynput.dll` (`cargo cinstall --dlltool llvm-dlltool`). The executable needs llvm-mingw's `libunwind.dll`, copied into `dist/arm64`.
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
- **Steps**: `cargo build --target wasm32-unknown-unknown -p kyclient-wasm --release` (with `RUSTFLAGS=--cfg=web_sys_unstable_apis`), `wasm-bindgen --target web --no-typescript`, then `wasm-opt -Os -g`, as Kyber's script does. The result keeps exactly the JS API of Kyber 0.29's engine (same classes, functions and methods): the `/ecran` page has nothing to change.
