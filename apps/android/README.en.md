# KyberNow Link for Android and Android TV

English · [Français](README.md)

KyberNow Link on Android: a single app for phone, tablet, TV (Android TV, Google TV) and handheld console, bringing back the computer (KyberNow Station: Kyber 0.29 server + KyberNow Admin) full screen, by touch, with the remote, keyboard and mouse, or a gamepad. Brand v2 ([guidelines](../../brand/v2/README.en.md)); mock-ups: [Claude Design](https://claude.ai/artifact/Ug3n57UPSvsrA3Hd9UMawX) (home, session menu, gamepads and rumble, image settings on the TV; home, adding and session on the phone; home on the tablet).

**Status**: framing, mock-ups, and the Rust core building for Android's three targets (`scripts/build-core.sh`, 22/10/2026). No app code yet. The decision is in [PROJECT.en.md](../../PROJECT.en.md) (“Android and Android TV”).

## Devices

- **Android 11 or later** (API 30): the NVIDIA Shield goes no further than Android 11.
- **TVs**: NVIDIA Shield first, Google TV Streamer, Chromecast with Google TV, Google TV sets. The Chromecast with Google TV and many TVs run 32-bit Android: the app ships as `arm64-v8a` and `armeabi-v7a`.
- **Phones and tablets**, with USB-C external display, Samsung DeX and ChromeOS (resizable window, keyboard and mouse).
- **Handheld consoles** (Retroid, AYN Odin) and **phone gamepads** (Razer Kishi, Backbone): the built-in gamepad is player 1; the interface is driven by focus, as on the TV.
- **Test hardware**: Shield TV 2019 “tube” (no USB port: Bluetooth gamepads only), Xiaomi Redmi A5; DualSense, Xbox, Shield and Switch Pro gamepads. Nothing to try a tablet, a handheld console, DeX or ChromeOS: those parts will be written without being checked.

## What the app will do

What the Apple TV, iPhone and iPad, Mac and Windows apps already do, on the same core:

- **Home**: the chosen computer full frame (its last image, otherwise Admin's preview), its state (local network, VPN, offline), system, graphics card, address and the virtual display waiting for it; “Connect” and the Game, Desktop, Max profiles; a “Ready to play” column on the TV and tablet (gamepads and their motors, sound output and its delay, display mode, last latency); the other computers and those found on the network.
- **Adding**: on the network (`_kybernow._tcp` through NsdManager), a six-digit code accepted in Admin, a camera QR code (`kybernow://` link, also opened from another app), an address with the fingerprint's first four groups. On the TV, in addition: it shows a code, the phone app types it in and sends it its computers, relayed by Admin (in place of iCloud; to be written on the Station side). Passwords in the Android Keystore.
- **Passage** and **session**: the 45° seam and the neon, the “You are entering…” capsule; on the phone, the tab and the bar (latency, profile, trackpad or direct touch, Auto / Desktop / Game mouse, display, sound, Keys, Quit), the row of keys above the keyboard (Esc, Tab, sticky Ctrl, Alt and ⊞, arrows, F1 to F12, Ctrl + Alt + Del); on the TV, the menu through the Back button or L1 + R1 + Start + Select (Resume, Gamepads, Sound, Image, Remote as mouse, Type text, Keys, Disconnect), stream health at the top; states as capsules, never alerts (reconnecting, computer mode changed, fallback, gamepad connected).
- **Gamepads**: four players at most (XInput on the computer), a fifth one waiting; gamepad seen as Xbox 360 or faithful; gamepad as mouse; Guide button (Game, Long press, System); test screen (buttons, sticks, triggers, test rumble).
- **Keyboard and mouse**: raw mouse through Android pointer capture in the Game profile, the computer's pointer drawn by the app in Desktop; text clipboard both ways on phone and tablet.
- **Background**: everything is released on the computer, bitrate capped, image frozen; on return, everything comes back.
- **“Resume on a computer” row** on the Android TV home screen, with each computer's last image; one click opens the session.
- **Settings** per computer and for the app; French and English (system language, or Auto / Français / English).

Out of the first version: on-screen gamepad, foldable layouts.

## Image, refresh rate and game mode

- **Decoding**: kyclient patch 0001 hands video packets to the app; MediaCodec decodes them in low-latency mode (`KEY_LOW_LATENCY`, Android 11) and renders to a `SurfaceView`, which the display receives without composition (like the Apple TV's lone Metal layer). Nothing must stay above the image during the session. Profiles come down to what the device really decodes, checked at launch: the Shield decodes HEVC and H.264, not AV1 nor 4:4:4.
- **Refresh rate**: the app picks the display mode (`preferredDisplayModeId`) and its surface's frame rate (`Surface.setFrameRate`). The Shield outputs HDMI 2.0: 4K at 60 Hz or 1080p at 120 Hz. On phones, 120 Hz must be requested: Android 15 keeps games at 60 Hz unless asked.
- **TV game mode**: `Window.setPreferMinimalPostProcessing(true)` (ALLM over HDMI) during the session, when the display allows it (`Display.isMinimalPostProcessingSupported`).
- **VRR**: Android gives apps no control over HDMI VRR and the Shield has none; the setting is greyed out. On a variable refresh rate display (120 Hz phones and tablets), each frame goes out as soon as it is decoded, the newest replacing the others.
- **HDR**: impossible with Kyber 0.29 (8-bit capture), as in the other apps.

## Sound

Opus decoded by the core's engine (`apps/core/src/audio`), pulled by AAudio in low-latency mode. Auto follows the output: stereo, 5.1 or 7.1 PCM to an HDMI amplifier; the output delay (Bluetooth above all) is measured and flagged beyond 40 ms; sound can stay on the computer.

## Gamepads and rumble

Gamepads arrive through Android events (`KeyEvent`, `MotionEvent`) and go to the core as on the Apple TV (`gamepads_from_app`): kynput does not use SDL here.

The game's rumble already reaches the core (`InputPlatform::rumble`, kynput's network thread); the app plays it on the player's gamepad:

- **Android 12 and later**: `InputDevice.getVibratorManager()` gives each of the gamepad's motors; the big low motor on the left and the small high one on the right, as in XInput.
- **Android 11 (Shield)**: `InputDevice.getVibrator()`, a single vibrator when the gamepad exposes one, at the stronger of the two motors' intensities. Many Bluetooth gamepads do not vibrate there; no home-made USB driver.
- The test screen shows “2 motors”, “1 motor” or “no rumble” for each gamepad, and runs each motor on its own.

## Architecture

- **Core**: `kybernow-core` (session, Admin, profiles, latency, inputs, sound), built for Android with the NDK. Kyber provides for Android in kyclient, kynput and kyvlcplayer (`target_os = "android"`).
- **JNI bridge**: a `cdylib` crate on top of the core, the Kotlin counterpart of `apps/apple`, with libkynput, libopus and VLC's empty functions (referenced by kyvlcplayer, never called: image and sound go to the app) in the same library.
- **Certificates**: on Android, kyclient checks certificates through `rustls-platform-verifier`, to be initialised once from Kotlin with the app's context (`kyclient_init_platform_verifier`); the verifier's Android part is bundled in the APK.
- **App**: Kotlin, Jetpack Compose (Compose for TV for focus), `SurfaceView` and MediaCodec for the image, AAudio for sound.

## Building

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-c --version 0.10.15 --locked
# NDK r27c: Android Studio › SDK Manager › SDK Tools › NDK (Side by side) 27.2.12479018,
# or ANDROID_NDK_HOME pointing to an unpacked NDK.
apps/android/scripts/build-core.sh                 # all three targets
apps/android/scripts/build-core.sh --target aarch64-linux-android --check
```

`scripts/build-core.sh` applies the patches in `apps/macos/kyber-patches`, builds libkynput without SDL into `third_party/kyber-desktop/rootfs-<target>` (`cargo cinstall`), lays down a `libvlc.pc` with no library (vlc-rs, a dependency of kyvlcplayer on Android, only asks pkg-config for it; VLC's functions, never called, will be empty functions in the JNI library, as on iOS), then builds the core for each target, with the NDK's clang for API 30 (C compiler, linker, and `--sysroot` for bindgen). Checked on 22/10/2026 with rustc 1.97.0 and NDK r27c, on Linux: all three targets build (a minute and a half each, libkynput included). Kyber 0.29's sources go in `third_party/kyber-desktop` (see [docs/build-notes.en.md](../../docs/build-notes.en.md)).

## To do

- The JNI bridge, then the Kotlin app.
- On the Station side: relay the phone's computers to the TV (code shown on the TV).
- On the Shield: which gamepads vibrate over Bluetooth under Android 11, 1080p at 120 Hz end to end, the TV's game mode.
