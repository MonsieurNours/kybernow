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
- **Icon next to the clock**: on the Mac, system notifications (and their permission request), registering to open at login and a real launch at login, closing the window in full screen during a session; on a real Windows PC, the top of the menu during a session and the lit icon, 100% and 200% scaling, notification banners, “still not responding” after 2 min of waking.
- **Multiple displays**: on a Windows PC and a Linux machine with several displays (validated on Mac on 26/10/2026); a session on several displays right after the Station is installed or restarted.
- **Several computers at once**: two sessions running at the same time on Windows and on Linux (checked on Mac on 27/10/2026); on Windows, a session end that once cut another session in the VM (every end path is now logged).
- **KyberNow Share and relay**: a guest's own pictures on AMD (AMF) and Intel (QSV) cards; the rest of the checks of patches 0019 and 0020 is in [docs/kyber-changes.en.md](docs/kyber-changes.en.md). Two sessions at once on Windows and Linux are on the “Several computers at once” line.
- **HDR**: the Windows, Android and Linux apps on a real HDR screen (written, never tried on an HDR screen).
- **PlayStation consoles**: a PS4; a game away from home through the real relay and a real Station (only tried on the Mac's local loop, towards the PS5 Pro); the apps other than the Mac's.
- **KyberNow Station for Linux** (beta): on a real x86_64 machine, with an NVIDIA (NVENC) or Intel/AMD (VA-API) card: an end-to-end session from an app (picture, sound, latency), updating from Admin (apt and dnf, rollback), the tray icon, lending, controllers, the pen, the clipboard under Wayland; Bazzite, Fedora Atomic and SteamOS.
- **Pen**: a real graphics tablet (Mac, Windows, Linux), an Apple Pencil and an Android stylus, against a Station.
- **Measurements from the 20/10/2026 audit** (procedure: [docs/archives/mesures.en.md](docs/archives/mesures.en.md)): Mac app display latency (the “one frame in flight” guard, full screen presented directly or composited); Windows permissions (`ProgramData` permissions, the tray icon's integrity level); saturated Wi-Fi (deep or short send queue by default); SudoVDA virtual display modes (the “follow / restore” policy).

## KyberNow Station and Admin

- **Mac Station, requests to Apple** ([docs/station-macos.en.md](docs/station-macos.en.md)): the virtual HID entitlement (`com.apple.developer.hid.virtual.device`), which will bring controllers (CoreHID, ready); `persistent-content-capture`, so the monthly screen recording reminder goes away.
- **Linux Station, HDR**: once GNOME or KDE can create a BT.2100 virtual display ([docs/station-linux.en.md](docs/station-linux.en.md), “HDR”).
- **H.264 silent on an Intel card**: PC-ATELIER (Intel UHD) does send HEVC but no picture in H.264 (“its encoder does not respond”), already in 0.5; to understand on the Station side (Kyber's encoder, QSV).
- **Virtual display without workarounds**: a driver that truly handles several displays (SudoVDA fixed and rebuilt, or Kidd), to remove the pointer enlargement and the Windows Graphics Capture of secondary displays (see `server/README.en.md`).
- **Kidd virtual display** once Kyber ships it, behind the interface already in place.
- **KyberNow-signed audio driver** (EV certificate + Partner Center), should Steam's speakers no longer be enough.
- **Attested virtual display driver** (EV + Partner Center), so SudoVDA's certificate no longer has to be added to the root authorities.
- **Authenticode signing** of the installer, the Windows app and the manual update package (SignPath targeted; prerequisite: Windows binaries built in CI).
- **Wake on LAN on iPhone, iPad and Apple TV**: ask Apple for the multicast entitlement (`com.apple.developer.networking.multicast`), so the packet goes out by broadcast and not only directly.
- **Controller API**: list what `kycontroller` exposes over HTTP (session state, metrics).

## KyberNow Link

- **Android**: foldable layouts; external display as a desk (image on the attached display, device as the desk); Play Store internal testing.
- **Android TV + Station**: send the phone's computers to the TV (code shown on the TV, relayed by Admin).
- **iPhone and iPad**: TestFlight for internal testers (App Store profile, Release archive, `xcodebuild -exportArchive`), then the App Store.
- **Apple TV**: TestFlight, then the App Store.
- **App Store (iPhone, iPad, Apple TV)**: what remains before submission, see [docs/app-store.en.md](docs/app-store.en.md): demo Station reachable through the relay for App Review, video, listing and screenshots, privacy labels, encryption questionnaire (ANSSI declaration), Kyber licence to confirm.
- **LG TVs**: a Homebrew Channel repository, so updates install from the TV; DualSense rumble and adaptive triggers through webOS's Bluetooth service.
- **Linux**: zero-copy DMA-BUF import of VA-API frames (4K at 120 Hz); NVDEC decoding for NVIDIA cards; a Flatpak repository or Flathub, so updates install by themselves.
- **Raspberry Pi**: publish the apt repository (GPG key, hosting, called from `release.sh`); zero-copy path (SAND frames on a DRM plane); HDR (a 10-bit plane in KMS/DRM, which SDL2 doesn't give).
- **Web client**: the pen (Pointer Events to kynput) and the microphone (upstream in the wasm engine; the page is ready).
- **KyberNow Link OS**: install the update during standby; a composed soundtrack for the ignition, instead of `chime.rs`'s synthesis.

## KyberNow Share

Everything that lets someone else onto your computer. Name chosen on 26/10/2026; logo signed SHARE, like STATION, ADMIN and LINK.

- **Browser picture over WebTransport through the relay** (except Safari): the relay would read the name in the first QUIC packet and pass it on without decrypting; today, through the relay, the browser gets its picture over WebSocket.
- **“Auto” that follows the window**: a guest's “Auto” quality is measured when they arrive (the device's display, the decoder); redo it when their window moves to another display.
- **Share started from the PC**: the first guest sets the format and bitrate of the shared picture.
- **Removed guest**: they can come back with the same code until the host asks for a “New code”.
- **Public Suffix List**: add `share.kybernow.app` to the list, so browsers treat each Station's address (`<id>.share.kybernow.app`) as a separate site (cookies, storage).
- **Fair use of the home's upload bandwidth**: split it fairly between several guests.
- **Lending the computer** (released in 0.9.0, contract: [docs/pret.en.md](docs/pret.en.md)), next: waking a sleeping computer for a friend away from your home; controllers and touchscreen plugged into the PC blocked during a loan; windows of apps run as administrator.
- **Lead**: lending the computer while you use it, through a virtual machine that shares the graphics card (Hyper-V); needs Windows Pro and a second licence, runs into anti-cheat, encoder to be checked.

## Sony controllers in “Faithful” mode

The computer sees a DualShock 4 (ViGEmBus driver): buttons, sticks, triggers and rumble go through; the touchpad is only a button.

- **Light bar**: send back to the app the colour the game gives the virtual DualShock 4 (the driver already receives it), to light the real controller.
- **Gyroscope and touchpad**: extend Kyber's controller protocol (patches on the apps' side and the Station's side, to offer to Kyber) and the virtual DualShock 4's extended report, then each app.
- No adaptive triggers: Windows does not see a DualSense, and no free driver creates a virtual one.

## Publishing, CI, website, brand

- **Old-name copies** (`KyberNow-Server-Setup.exe`, `KyberNow-Setup.exe`, `KyberNow-Client-Setup.exe`, `KyberNow-macOS.zip`): drop them once installations on 0.3.1 and earlier are gone.
- **CI**: build the Mac app (with a `KyberSDK-macOS.zip`) and the Windows binaries.
- **Kyber**: send the `strip` fix upstream (cargo-c, Xcode 27's linker).
- **Website**: remove the “beta” markers at 1.0; revisit the Apple TV, iPhone and iPad cards once those apps go through TestFlight or the App Store.
