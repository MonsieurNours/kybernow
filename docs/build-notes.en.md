# Build notes

English · [Français](build-notes.md)

## Tool versions

KyberNow's Kyber binaries (server overlay, Mac and Apple TV app libraries) are built and checked with these versions. They are written in `scripts/kyber-toolchain.sh`, read by the three build scripts: a different version stops nothing, it is reported at the start ("Attention : rustc 1.90.0, versions vérifiées : 1.89.0"). After a checked update, change both places.

| Tool | Version | For |
|---|---|---|
| Rust stable (`rustc`) | 1.89.0, target `x86_64-pc-windows-gnu` | Windows server, macOS client |
| Rust nightly | of 2026-10-19 (1.101.0), `rust-src` component | Apple TV client (`-Zbuild-std`) |
| cargo-c (`cargo cinstall`) | 0.10.15 | macOS and Apple TV clients |
| mingw-w64 (Homebrew) | 14.0.0, GCC 16.2.0, UCRT | Windows server |
| meson | 1.11.2 | Windows server (dependencies, txproto) |
| llvm-mingw | 20261006 (UCRT, macOS), clang 23.1.3 | Windows ARM64 client, in `third_party/toolchains` |
| wasm-bindgen-cli | 0.2.100 (the one in kyber-web's `Cargo.lock`) | web client engine |
| binaryen (`wasm-opt`) | 133 | web client engine |

Kyber's repositories have no `rust-toolchain.toml`: rustup's default toolchain does the build.

## Build scripts: real failure

`server/kyber-patches/build.sh`, `apps/macos/scripts/build-kyclient.sh` and `apps/tvos/scripts/build-kyber-tvos.sh` stop at the first error and then replace nothing (overlay, `Vendor/`):

- **cargo**: its output is filtered (progress lines removed) without the filter deciding the exit code (`set -o pipefail`). Before, `| grep … || true` hid a failed build and the old binary went into the overlay.
- **Stale binary**: `build.sh` refuses `kyavserver.exe`, `libtxproto-0.dll`, `kycontroller.exe`, `kynput.dll`, `kynputserver.exe` and `kyclient_wasm.wasm` if they are older than its start (it resets the sources every time, so they are always rebuilt). `cargo cinstall` keeps the date of the library already built: the client scripts (and `build.sh` for `kynput.dll`) instead remove the library installed by a previous build before compiling, and the copy fails if this build did not install it again.
- **Patch that does not apply**: the rejected hunk is shown (`patch -r -` for the server, `git apply --check` for the clients).
- **Patch that adds a file**: `build.sh` resets each file of the server patches to the original, one by one; a file missing from the original is deleted instead of restored. No `git checkout -- .`: `kysdk/kyctl` and `kysdk/kymux` also carry the app patches.
- **Worktree**: `third_party/` (outside git) can be a link there to the main checkout's. The scripts use its real path (`pwd -P`): meson and cargo record the sources' absolute paths, and another path makes them reconfigure and rebuild everything.
- **App open**: `build-kyclient.sh` refuses to run while KyberNow is open (`pgrep -x KyberNow`), at the start and right before copying into `Vendor/` (see "Embedding the libraries in the app without crashing it" below).

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

To report to Kyber, and to keep in mind for the KyberKit build (macOS/tvOS xcframework).

### libkyclient with the KyberNow patches

`apps/macos/scripts/build-kyclient.sh` applies `apps/macos/kyber-patches/*.patch` to the `kysdk/kyctl` repository, rebuilds libkyclient (`cargo cinstall`, a few seconds) and replaces it in `apps/macos/Vendor/`. Run it again after each `vendor-kyber.sh`. `vendor-kyber.sh` also copies Opus's headers (`COpus` module): the sound is decoded by the app.

- **strip**: with the release profile's default `strip`, Xcode 27's linker refuses the library (“mis-aligned LINKEDIT string pool”). The script builds with `CARGO_PROFILE_RELEASE_STRIP=false`.

### Embedding the libraries in the app without crashing it

`apps/macos/scripts/embed-kyber.sh` (Xcode build phase) signs a working copy of `Vendor/Frameworks` (in `DERIVED_FILE_DIR`), re-signing only what has changed. It copies into the app only the files that differ.

- **Why**: rewriting the app's libraries while it runs makes VLC crash at the start of the next session (`config_GetInt` on NULL, in `libsyslog_plugin`). Its modules, loaded on demand from the new files, link a new `libvlccore` file while the old one is already in memory: two VLC cores coexist.
- **After `build-kyclient.sh`**: the libraries really change. Quit the app before rebuilding, then relaunch it.
- **Outside a `.app` bundle** (test program launched next to `Vendor/Frameworks`): VLC also looks for its modules in the folder where it was built (`rootfs-arm64-macos/lib/vlc/plugins`), with the same crash. In the app, it only takes `Contents/Frameworks/plugins`.

## Browsers

The Kyber web client works in Chrome. In Safari, the screen stays black (WebKit is not supported, as Kyber's docs state).

## Kyber 0.29 server for Windows (kyavserver), from macOS

`server/kyber-patches/build.sh` runs everything in sequence (versions: see above). Pitfalls met with Homebrew's MinGW toolchain:

- **luac**: txproto compiles its Lua scripts during the build and wanted to run `luac.exe`. You need a native `luac` 5.4 in the PATH (`brew install lua@5.4`); the bytecode is the same (64-bit, little-endian).
- **Jinja**: the libplacebo archive does not include its submodules. Copy `jinja2` and `markupsafe` (pip wheels) into `subprojects/libplacebo-*/3rdparty/{jinja,markupsafe}/src`.
- **bison**: macOS's (2.3) is too old for VLC; use `/opt/homebrew/opt/bison/bin`.
- **pkg-config**: meson expects `x86_64-w64-mingw32-pkg-config`; a simple wrapper to `pkg-config` is enough (the rootfs is set by the cross file).
- **bindgen**: `BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu --sysroot=<mingw> -I<mingw>/include"`, otherwise `errno.h` cannot be found.
- **Runtime**: Homebrew's MinGW produces UCRT binaries, official Kyber uses msvcrt. So we replace the whole kyavserver chain (see the overlay), never a single DLL. Safe exception: `kynput.dll`, see below.

## Kyber 0.29 input server for Windows (kynput), from macOS

`server/kyber-patches/build.sh` also rebuilds `kynput.dll` (`cargo cinstall`, `sdl2` feature as in the official release, installed into `third_party/rootfs-win64-kynput`) and `kynputserver.exe` (`win32-hide-console`), for the `*-kynput-*` patches.

- **SDL2**: `kynput.dll` links against Kyber 0.29's `SDL2.dll` (`third_party/payload/kyber-0.29.0`), which is not rebuilt. `sdl2-sys` looks for it through pkg-config: the script writes a minimal `sdl2.pc` (`third_party/toolchain-bin/pkgconfig-sdl2`) pointing to that folder, and MinGW's linker links the DLL directly, without an import library.
- **kynputserver's build.rs**: it tested `cfg!(target_os = "macos")`, that is the machine doing the build; from a Mac, it assembled the macOS code with MinGW and failed. Fixed by 0012 (tests `CARGO_CFG_TARGET_OS`).
- **Runtime**: like the rest of the overlay, `kynput.dll` and `kynputserver.exe` are UCRT, next to the official `kyclient.dll` and `kyclient.exe` (msvcrt) which load the same `kynput.dll`. No risk here: kynput's C API always frees what it allocates itself (`kynput_*_unref`, `kynput_free_string`). It exports the official release's functions, plus the clipboard ones if client patch 0002 is applied in `third_party` at build time.

## Windows client (apps/windows), from macOS

`apps/windows/scripts/build.sh` builds the client core (`apps/core`) and the Windows app with the server's toolchain (`x86_64-pc-windows-gnu`, Homebrew MinGW), relying on what `server/kyber-patches/build.sh` produced: static FFmpeg and libopus (`third_party/rootfs-win64`), `kynput.dll` (`third_party/rootfs-win64-kynput`).

- **Kyber's crates**: the app depends directly on the Rust crate `kyclient` (not its C API). Its dependencies name each other by their crates.io version: `apps/Cargo.toml` points them to `third_party/kyber-desktop/kysdk` (`[patch.crates-io]`, like `kysdk/.cargo/config.toml`), and `apps/Cargo.lock` starts from `kysdk/kyctl`'s to keep the same versions.
- **VLC**: `kyvlcplayer` always links `libvlc` on Windows (`vlc-rs`, through pkg-config), even when image and sound go to the app. The script makes a `libvlc.a` of empty functions (one per export of `libvlc.dll.a`) and a `libvlc.pc` pointing to it, in `third_party/windows-client-x64`. FFmpeg's static libraries are linked from a separate folder (`third_party/windows-client-x64/native`): the rootfs one also holds `libvlc.dll.a`, which the linker preferred to the empty functions (the executable then depended on `libvlc.dll`). The script refuses an executable that still depends on it.
- **libavcodec**: its Opus decoder calls libswresample, which must be linked too.
- **FFmpeg bindings**: generated by bindgen at build time (`apps/windows/build.rs`) from the rootfs headers, with the same `BINDGEN_EXTRA_CLANG_ARGS` as kynput. `AVD3D11VADeviceContext` is copied by hand (pointers) so that bindgen does not read Direct3D's COM headers.
- **Size**: the executable carries the encoders of the server's libavcodec (x264, VPL); the `dist/` copy has no debug information (36 MB).
- **ARM64** (`--arch arm64`): Rust target `aarch64-pc-windows-gnullvm` (`rustup target add`) and the [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) toolchain (UCRT build for macOS, unpacked into `third_party/toolchains`): Homebrew's MinGW does not target ARM64. `apps/windows/scripts/build-arm64-deps.sh` builds into `third_party/rootfs-winarm64` what the server does not provide for ARM64: libopus (`--disable-rtcd`: CPU detection does not exist for Windows ARM, NEON is always there), an FFmpeg reduced to decoding (H.264, HEVC, AV1, D3D11VA acceleration, without x264 or VPL: 16 MB instead of 170), SDL2 2.32.8 (the archive downloaded and verified by kynput's script) and `kynput.dll` (`cargo cinstall --dlltool llvm-dlltool`). The executable needs llvm-mingw's `libunwind.dll`, copied into `dist/arm64`.
- **set -e**: in `make && make install`, a failing `make` does not stop the script (only the last command of an `&&` list counts); libopus had failed silently. Both are on two lines, and the script checks the libraries produced.
- **Tauri app**: Tauri **2.11.6** and tauri-build 2.6.3, built for `windows-gnu` and `windows-gnullvm` from the Mac (like the server installer's `tao` and `wry`). Tauri 2.12 and its crates need Rust 1.90 while KyberNow is verified with 1.89: the `apps/` workspace uses resolver 3, which picks versions compatible with `rust-version`. The interface (`ui/`, Svelte and Vite, same versions as the console) is built by `build.sh` before cargo and embedded in the executable; `WebView2Loader.dll` (the one from `webview2-com-sys`, x64 or arm64) is copied next to it. The icon (`resources/kybernow.ico`, “now” symbol) comes from `resources/make-ico.py`.
- **Installer** (Homebrew NSIS 3.13): `makensis` rejects a UTF-8 script (“Bad text encoding”) when the terminal's locale is not UTF-8; `build-installer.sh` runs it with `LC_ALL=en_US.UTF-8`, and the script is saved with a byte order mark.
- **Core tests on the Mac**: `cargo test -p kybernow-core` with `PKG_CONFIG_PATH` pointing to `third_party/kyber-desktop/rootfs-arm64-macos/lib/pkgconfig` (libvlc and kynput of Kyber's macOS client).

## Web client engine (kyclient_wasm), from macOS

The engine the web client loads comes from Kyber's `kyber/apps/kyber-web` repository (`kyclient-wasm` crate, the JS API), not from kyber-desktop. `server/kyber-patches/build.sh` rebuilds it with patches 0013 to 0015 and lays `kyclient_wasm.js` and `kyclient_wasm_bg.wasm` into the overlay's `webclient/`.

- **Sources**: `git clone --depth 1 --branch 0.29.0 https://gitlab.com/kyber/apps/kyber-web.git third_party/kyber-web`. Its `kysdk` submodule is the same commit as kyber-desktop 0.29's (`ebca32a`): it is not cloned, the script replaces it with a link.
- **Tools**: `rustup target add wasm32-unknown-unknown`, `cargo install wasm-bindgen-cli --version 0.2.100 --locked` (the version must be the one of the `wasm-bindgen` crate in kyber-web's `Cargo.lock`, otherwise wasm-bindgen refuses the module), `brew install binaryen` (`wasm-opt`). The rest of kyber-web's `build-wasm.sh` (pnpm, Kyber's interface) is not needed: the interface does not change.
- **Original kyctl**: `third_party`'s `kysdk/kyctl` also carries the apps' patches, which change kyclient's API (the `packet_sink` field of `AudioSessionConfig` breaks kyber-web's build). So the script composes `third_party/kyber-web-kysdk/`: `kyctl` extracted from git (`git archive HEAD`) plus the `*-kywebplayer-*` patches, and links to `third_party`'s `kymedia`, `kymux`, `kynput` and `kyutil`, server patches applied. `third_party/kyber-web/kysdk` points to that folder.
- **Steps**: `cargo build --target wasm32-unknown-unknown -p kyclient-wasm --release` (with `RUSTFLAGS=--cfg=web_sys_unstable_apis`), `wasm-bindgen --target web --no-typescript`, then `wasm-opt -Os -g`, as Kyber's script does. The result keeps exactly the JS API of Kyber 0.29's engine (same classes, functions and methods): the `/ecran` page has nothing to change.
