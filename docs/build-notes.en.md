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

Kyber's repositories have no `rust-toolchain.toml`: rustup's default toolchain does the build.

## Build scripts: real failure

`server/kyber-patches/build.sh`, `apps/macos/scripts/build-kyclient.sh` and `apps/tvos/scripts/build-kyber-tvos.sh` stop at the first error and then replace nothing (overlay, `Vendor/`):

- **cargo**: its output is filtered (progress lines removed) without the filter deciding the exit code (`set -o pipefail`). Before, `| grep … || true` hid a failed build and the old binary went into the overlay.
- **Stale binary**: `build.sh` refuses `kyavserver.exe`, `libtxproto-0.dll` and `kycontroller.exe` if they are older than its start (it resets the sources every time, so they are always rebuilt). `cargo cinstall` keeps the date of the library already built: the client scripts instead remove the library installed by a previous build before compiling, and the copy fails if this build did not install it again.
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
- **Runtime**: Homebrew's MinGW produces UCRT binaries, official Kyber uses msvcrt. So we replace the whole kyavserver chain (see the overlay), never a single DLL.
