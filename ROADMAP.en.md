# KyberNow — planned

English · [Français](ROADMAP.md)

What is planned or still to try, and only here: the other documents describe what exists ([docs/i18n.en.md](docs/i18n.en.md#documentation)). A line leaves this file once it is done; what it brought then goes into the docs of the app concerned and into `CHANGELOG.md`.

## Trials to run

- **Steam Deck**: a session on the Deck (the Linux app has only been tried in a session in the VM).
- **Raspberry Pi**: the app (.deb) and KyberNow Link OS on a real board: firmware (tryboot), Plymouth, V4L2 decoder, HDMI output (including 1080p at 120 Hz on a Pi 5), sound, Bluetooth, CEC.
- **iPad**: external display and Stage Manager.
- **Android**: 1080p at 120 Hz on the Shield, end-to-end update from a published release, a tablet, a handheld console, DeX and ChromeOS; DualSense and Shield controller rumble on the Shield; sound losses on the Shield at 4K 60 (100 Mbit/s Ethernet) to check again over gigabit or at a lower bitrate.
- **LG TVs**: latency on the G2 at 4K 120 Hz, 1440p and 1080p, at 30, 60 and 120 fps; audio-video offset and added latency of 5.1 Dolby; Magic Remote as mouse: motion sent while the TV's pointer is hidden.
- **Station**: the “For any connection” mode (virtual display created during the stream); on a real Windows PC, restarting and updating the Station from an app, the connection last resort, the network cards listed for waking, and a real wake from sleep and after a full shutdown.
- **Audit measurements** described in [docs/archives/mesures.en.md](docs/archives/mesures.en.md): Mac app display latency, Windows permissions, saturated Wi-Fi, SudoVDA virtual display modes.

## KyberNow Station and Admin

- **HDR**: FP16 capture and HDR10 conversion in Kyber (costed study, in French: [docs/archives/propositions-2026-10-22.md](docs/archives/propositions-2026-10-22.md)).
- **Kidd virtual display** once Kyber ships it, behind the interface already in place.
- **KyberNow-signed audio driver** (EV certificate + Partner Center), should Steam's speakers no longer be enough.
- **Attested virtual display driver** (EV + Partner Center), so SudoVDA's certificate no longer has to be added to the root authorities.
- **Authenticode signing** of the installer, the Windows app and the manual update package (SignPath targeted; prerequisite: Windows binaries built in CI).
- **Wake on LAN on iPhone, iPad and Apple TV**: ask Apple for the multicast entitlement (`com.apple.developer.networking.multicast`), so the packet goes out by broadcast and not only directly.
- **Controller API**: list what `kycontroller` exposes over HTTP (session state, metrics).
- **Open question**: several simultaneous users on the same computer?

## KyberNow Link

- **Windows**: a session across several of the PC's displays.
- **Android**: on-screen gamepad; foldable layouts; external display as a desk (image on the attached display, device as the desk); Play Store internal testing.
- **Android TV + Station**: send the phone's computers to the TV (code shown on the TV, relayed by Admin).
- **iPhone and iPad**: on-screen virtual gamepad, Apple Pencil; TestFlight for internal testers (App Store profile, Release archive, `xcodebuild -exportArchive`), then the App Store.
- **Apple TV**: TestFlight, then the App Store.
- **LG TVs**: a Homebrew Channel repository, so updates install from the TV; DualSense rumble and adaptive triggers through webOS's Bluetooth service.
- **Linux**: zero-copy DMA-BUF import of VA-API frames (4K at 120 Hz); NVDEC decoding for NVIDIA cards; a Flatpak repository or Flathub, so updates install by themselves.
- **Raspberry Pi**: publish the apt repository (GPG key, hosting, called from `release.sh`); zero-copy path (SAND frames on a DRM plane).
- **KyberNow Link OS**: install the update during standby; a composed soundtrack for the ignition, instead of `chime.rs`'s synthesis.

## Publishing, CI, website, brand

- **Old-name copies** (`KyberNow-Server-Setup.exe`, `KyberNow-Setup.exe`, `KyberNow-Client-Setup.exe`, `KyberNow-macOS.zip`): drop them once installations on 0.3.1 and earlier are gone.
- **CI**: build the Mac app (with a `KyberSDK-macOS.zip`) and the Windows binaries.
- **Kyber**: send the `strip` fix upstream (cargo-c, Xcode 27's linker).
- **Website**: remove the “beta” markers at 1.0; revisit the Apple TV, iPhone and iPad cards once those apps go through TestFlight or the App Store.
- **Brand**: confirm the KyberNow name (and Station / Admin / Link) with the Kyber team; the `fr.leslignesclaires.kybernow` identifier does not change.
