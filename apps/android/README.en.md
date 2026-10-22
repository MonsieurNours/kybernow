# KyberNow Link for Android and Android TV

English · [Français](README.md)

KyberNow Link on Android: a single app for phone, tablet, TV (Android TV, Google TV) and handheld console, bringing back the computer (KyberNow Station: Kyber 0.29 server + KyberNow Admin) full screen, by touch, with the remote, keyboard and mouse, or a gamepad. Brand v2 ([guidelines](../../brand/v2/README.en.md)); mock-ups: [Claude Design](https://claude.ai/artifact/Ug3n57UPSvsrA3Hd9UMawX) (home, session menu, gamepads and rumble, image settings on the TV; home, adding and session on the phone; home on the tablet).

**Status (22/10/2026)**: the app is written and builds (debug and release APKs, three ABIs); it has not run on a device yet. The decision is in [PROJECT.en.md](../../PROJECT.en.md) (“Android and Android TV”).

## Devices

- **Android 11 or later** (API 30): the NVIDIA Shield goes no further than Android 11.
- **TVs**: NVIDIA Shield first, Google TV Streamer, Chromecast with Google TV, Google TV sets. The Chromecast with Google TV and many TVs run 32-bit Android: the APK carries `arm64-v8a`, `armeabi-v7a` (and `x86_64` for the emulator).
- **Phones and tablets**, with USB-C external display, Samsung DeX and ChromeOS (resizable window, keyboard and mouse).
- **Handheld consoles** (Retroid, AYN Odin) and **phone gamepads** (Razer Kishi, Backbone): the built-in gamepad is a player like any other; the interface is driven by focus, as on the TV.
- **Test hardware**: Shield TV 2019 “tube” (no USB port: Bluetooth gamepads only), Xiaomi Redmi A5; DualSense, Xbox, Shield and Switch Pro gamepads. Nothing to try a tablet, a handheld console, DeX or ChromeOS.

## What the app does

- **Home** (`ui/home`): the chosen computer full frame (its last image, otherwise Admin's preview; very slow zoom, desaturated when offline), its state (local network, VPN, remote, offline; Admin polled every 8 s), system, graphics card and address; “Connect” and the Game, Desktop, Max profiles; a “Ready to play” column on TV and tablet (gamepads and their motors, sound output, display mode and possible refresh rate, decoders, last session); the other computers (long press: Settings, Remove), those found on the network, and adding. Offline with a known MAC address: “Wake the computer” (Wake-on-LAN).
- **Adding** (`ui/add`): on the network (`_kybernow._tcp` through NsdManager, TXT `name`, `fp`, `kyber`), a six-digit code shown to every Station found and accepted in Admin (a Kyber account of the device's own), an address with the fingerprint's first four groups to compare, a camera QR code (CameraX and zxing, no Google services) and a `kybernow://` link opened from another app. Passwords encrypted with an Android Keystore key.
- **Session** (`session/`):
  - The **passage**: home split along the 45° seam, the neon, the halves sliding apart on the first image; “You are entering…” capsule with the current step and Cancel.
  - The **image** full frame, at its aspect ratio (ink bars), on a `SurfaceView` that the decoder renders to directly; the interface and pointer views are removed when there is nothing to show.
  - **TV**: menu through the remote's Back button (configurable) or L1 + R1 + Start + Select: stream health at the top (latency, frames per second, bitrate, mode, losses), Resume, other profiles, Sound, Remote as mouse, Gamepad as mouse, Type text, Keys, Measurements, Disconnect. While the menu is open, gamepads drive the app and the computer releases everything.
  - **Phone and tablet**: tab (live latency) and bar (measurements, profiles, trackpad or direct touch, Auto / Desktop / Game mouse, keyboard, Keys, Quit); a row of keys above the on-screen keyboard (Esc, Tab, sticky Ctrl, Alt and ⊞, arrows, F1 to F12, Ctrl + Alt + Del), text sent as Unicode.
  - **States** as capsules, never alerts: reconnecting (with Quit), image restarted, computer mode changed (“Restore” in the Desktop profile), fallback, virtual display adapted, gamepad connected.
  - **End**: the last image is kept (PixelCopy), the session's latency (median, p95) and duration go to home.
- **Gamepads** (`model/Gamepads.kt`, `session/Inputs.kt`): four players at most in arrival order, a fifth waiting; announced to the computer at every reconnection; buttons, sticks, triggers (analogue or digital) and hat d-pads; a gamepad's Back and Menu go to the game as Select and Start; Guide button (Game, Long press: Start held 600 ms, System); gamepad as mouse (left stick pointer, RT faster, LT finer, right stick wheel, A / X / B clicks, d-pad arrows, Start ⊞, Select Esc, L1 / R1 Alt + Tab). Test screen (`ui/gamepads`): live buttons, sticks, triggers, each gamepad's motors and a test rumble per motor.
- **Keyboard, mouse, touch**: physical keyboard as scancodes (the Linux code from `KeyEvent.getScanCode()` converted by kynput); mouse by position (the computer's pointer drawn by the app) or captured as raw motion when a game wants it (Auto mouse); trackpad (one finger = pointer, tap = click, two fingers = right click and wheel, hold = drag) or direct touch; remote as arrows and Enter, or as a mouse.
- **Clipboard**: text both ways (phone, tablet); the device's text is read only when the computer pastes (Android 12 shows a notice on every read).
- **Background**: everything is released on the computer, bitrate capped, image frozen; with the surface gone, the decoder is rebuilt on a keyframe on return.
- **“Resume on a computer” row** on the Android TV home screen (`tv/TvChannel.kt`): each computer with its last image, lent to the home launcher only; one click opens the session (`kybernow://connect/<id>`). Google TV does not show these rows.
- **Per-computer settings** (`ui/settings`): profile; resolution (match this display, 4K, 1440p, 1080p, 720p), refresh rate, format, bitrate, 4:4:4, match the display to the session, TV game mode, VRR (information); sound; gamepad as seen by the computer, combination, Back, Guide; mouse, touch, clipboard; reliable transport, reconnection, MAC address; account, remove the computer. App settings: language (Auto, Français, English), the device's decoders and display, licences.
- **Languages**: French and English (`res/values-fr`, `res/values`, produced by `scripts/strings.py`); French when Android is in French, English otherwise; per-app choice (also in Android's settings from Android 13).

Out of the first version: on-screen gamepad, foldable layouts, app updates through the signed manifest, sending computers from the phone to the TV (to be written on the Station side).

## Image, refresh rate and game mode

- **Session mode**: “match this display”, at the highest refresh rate it holds; a requested rate the resolution cannot hold lowers the resolution (the Shield, HDMI 2.0: 4K at 60 Hz or 1080p at 120 Hz); brought down to what the decoder accepts (4096 wide at most in H.264). The computer's virtual display is created in this mode.
- **Display**: on the TV, the HDMI output takes the session's mode (`preferredDisplayModeId`, “Match the display” setting); on phones, the same resolution at the session's refresh rate (120 Hz must be requested: Android 15 keeps games at 60 Hz unless asked); the surface announces its frame rate (`Surface.setFrameRate`).
- **TV game mode**: `Window.setPreferMinimalPostProcessing(true)` (ALLM over HDMI) when the display allows it.
- **VRR**: Android gives apps no control over HDMI VRR and the Shield has none. On a variable refresh rate display (120 Hz phones and tablets), each frame goes out as soon as it is decoded, the newest replacing the others.
- **Decoding** (`jni/src/video.rs`): packets from patch 0001 go to MediaCodec (the format's hardware decoder, `low-latency`, real-time priority, the Qualcomm, Exynos and Amlogic vendor keys that turn off frame reordering, retried without them if refused), rendered straight to the surface. The parameter sets are put in front of the keyframe that starts a decoder (no `csd-0`, whatever the format); a loss waits for the next keyframe, requested again by the core; a new surface rebuilds the decoder. Profiles brought down to what the device really decodes (MediaCodecList at launch): the Shield decodes HEVC and H.264, not AV1 nor 4:4:4.
- **HDR**: impossible with Kyber 0.29 (8-bit capture), as in the other apps.

## Sound

Opus decoded by the core's engine (`apps/core/src/audio`), pulled straight by AAudio's real-time callback (`jni/src/audio.rs`), in low-latency mode and exclusive when possible, never through Java. Windows channel order (Android's). Auto follows the output: 5.1 or 7.1 to an HDMI amplifier, stereo elsewhere; an output that refuses multichannel gets stereo mixed down by the engine; an output that changes (headphones, HDMI) is reopened. Sound can stay on the computer.

## Rumble

The game's rumble reaches the core (`InputPlatform::rumble`) and is played on the player's gamepad (`Gamepads.rumble`):

- **Android 12 and later**: `InputDevice.getVibratorManager()`, two motors (big low motor on the left, small high one on the right, as in XInput).
- **Android 11 (Shield)**: `InputDevice.getVibrator()`, a single vibrator at the stronger of the two motors' intensities; many Bluetooth gamepads do not vibrate there.
- Without a duration, 5 s at most (as kynput on Mac); (0, 0) stops. The test screen says “2 motors”, “1 motor” or “no rumble” for each gamepad.

## Architecture

```
apps/android/
├── jni/                 kybernow-android crate → libkybernow.so (JNI bridge, MediaCodec, AAudio)
├── app/                 the app (Kotlin, Jetpack Compose, Compose for TV)
│   └── src/main/java/fr/leslignesclaires/kybernow/
│       ├── core/        Native (JNI functions), SessionListener, ConsoleClient (Admin), DeviceIdentity
│       ├── model/       computers and passwords, discovery, gamepads, decoders, display, sound, Wake-on-LAN
│       ├── ui/          brand theme, icons, home, adding, settings, gamepad test
│       ├── session/     SessionActivity, SessionState, inputs, session interface, pointer
│       └── tv/          Android TV home row
└── scripts/             build-native.sh, strings.py (texts), brand-drawables.py (brand drawings)
```

- **JNI bridge** (`jni/`, the Kotlin counterpart of `apps/apple`): Kotlin object `fr.leslignesclaires.kybernow.core.Native`, core objects as `Long`, Admin replies in JSON (`{"code": 0, "value": …}`), the same session parameters as the Apple apps' `kn_session_start`, plus `combo`. Callbacks to Kotlin (`SessionListener`) go through a global reference; each core thread attaches to the JVM and works inside a local reference frame. The whole hot path (image, sound, input chain) stays in Rust.
- **VLC**: kyvlcplayer references libvlc, never called (image and sound go to the app): `jni/build.rs` compiles an empty function for each `libvlc_*` function of vlc-rs, as on iOS. The script lays down a `libvlc.pc` with no library so vlc-rs finds it.
- **Certificates**: the core always gives the Kyber certificate's fingerprint (the one Admin announces): kyclient checks it itself, without Android's verifier (`rustls-platform-verifier`), which is therefore not initialised.
- **Drawings**: signed logo, mark, adaptive icon and Android TV banner converted from `brand/v2`'s SVGs by `scripts/brand-drawables.py`, untouched. Variable Archivo and Martian Mono fonts from `brand/v2/fonts`.

## Building

```sh
# Once: Rust targets, cargo-c, NDK r27c (Android Studio › SDK Manager › SDK Tools › NDK (Side by side)
# 27.2.12479018, or ANDROID_NDK_HOME), Android SDK (platform 36) and JDK 17 or later.
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-c --version 0.10.15 --locked

# Native library (after every change to apps/core or apps/android/jni):
apps/android/scripts/build-native.sh                     # all three ABIs
apps/android/scripts/build-native.sh --target aarch64-linux-android

# The app:
cd apps/android
./gradlew assembleDebug        # app/build/outputs/apk/debug/app-debug.apk
./gradlew assembleRelease      # signed when KYBERNOW_ANDROID_KEYSTORE (and its passwords) is given
./gradlew lintDebug
./gradlew recordRoborazziDebug  # screenshots of the main screens, no device needed → app/screenshots/
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

`scripts/build-native.sh` applies the patches in `apps/macos/kyber-patches`, builds into `third_party/kyber-desktop/rootfs-<target>` libkynput without SDL (`cargo cinstall`) and libopus 1.6.1 (cmake and the NDK toolchain; sources from the Opus repository's `v1.6.1` tag, cloned into `third_party/opus-1.6.1`), lays down `libvlc.pc`, then builds `kybernow-android` with the NDK's clang for API 30 and copies `libkybernow.so`, without debug information, into `app/src/main/jniLibs/<abi>/` (outside git). Kyber 0.29's sources go in `third_party/kyber-desktop` (see [docs/build-notes.en.md](../../docs/build-notes.en.md)). Versions: Android Gradle Plugin 8.13, Gradle 8.14, Kotlin 2.4, Compose (BOM 2026.06), Compose for TV 1.0; the app's version number is `apps/Cargo.toml`'s.

Screenshots (`app/src/test/.../Screenshots.kt`): Robolectric with native rendering and Roborazzi, with demonstration data and a fake computer image, without the native library; home (`HomeContent`), the gamepad test (`GamepadsContent`) and the session interface (`SessionOverlay`) take their data as parameters for this. Robolectric downloads an Android image on first run (`android-all-instrumented`, 200 MB, Maven Central).

Texts: `scripts/strings.py` (both languages in one file, French typography included) writes `res/values*/strings.xml`. Drawings: `scripts/brand-drawables.py`.

## To do

- A session on a real device (Shield, Redmi A5): image, sound, inputs, rumble, 1080p at 120 Hz, TV game mode.
- On the Shield: which gamepads vibrate over Bluetooth under Android 11.
- On the Station side: relay the phone's computers to the TV (code shown on the TV).
- App updates through the signed manifest; APK attached to published releases; Play Store internal testing.
- External display (image on the connected display, device as the desk), DeX and ChromeOS trials.
