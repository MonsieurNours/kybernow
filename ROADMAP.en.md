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
- **KyberNow Share and relay**: sharing away from home and reaching your computer through `share.kybernow.app` from each family of apps (an iPhone's cellular radio included); a guest's own pictures on AMD (AMF) and Intel (QSV) cards; the rest of the checks of patches 0019 and 0020 is in [docs/kyber-changes.en.md](docs/kyber-changes.en.md). Two sessions at once on Windows and Linux are on the “Several computers at once” line.
- **Audit measurements** described in [docs/archives/mesures.en.md](docs/archives/mesures.en.md): Mac app display latency, Windows permissions, saturated Wi-Fi, SudoVDA virtual display modes.

## KyberNow Station and Admin

- **H.264 silent on an Intel card**: PC-ATELIER (Intel UHD) does send HEVC but no picture in H.264 (“its encoder does not respond”), already in 0.5; to understand on the Station side (Kyber's encoder, QSV).
- **Virtual display without workarounds**: a driver that truly handles several displays (SudoVDA fixed and rebuilt, or Kidd), to remove the pointer enlargement and the Windows Graphics Capture of secondary displays (see `server/README.en.md`).
- **HDR**: FP16 capture and HDR10 conversion in Kyber (costed study, in French: [docs/archives/propositions-2026-10-22.md](docs/archives/propositions-2026-10-22.md)).
- **Kidd virtual display** once Kyber ships it, behind the interface already in place.
- **KyberNow-signed audio driver** (EV certificate + Partner Center), should Steam's speakers no longer be enough.
- **Attested virtual display driver** (EV + Partner Center), so SudoVDA's certificate no longer has to be added to the root authorities.
- **Authenticode signing** of the installer, the Windows app and the manual update package (SignPath targeted; prerequisite: Windows binaries built in CI).
- **Wake on LAN on iPhone, iPad and Apple TV**: ask Apple for the multicast entitlement (`com.apple.developer.networking.multicast`), so the packet goes out by broadcast and not only directly.
- **Controller API**: list what `kycontroller` exposes over HTTP (session state, metrics).

## KyberNow Link

- **Android**: foldable layouts; external display as a desk (image on the attached display, device as the desk); Play Store internal testing.
- **Android TV + Station**: send the phone's computers to the TV (code shown on the TV, relayed by Admin).
- **iPhone and iPad**: Apple Pencil; TestFlight for internal testers (App Store profile, Release archive, `xcodebuild -exportArchive`), then the App Store.
- **Apple TV**: TestFlight, then the App Store.
- **LG TVs**: a Homebrew Channel repository, so updates install from the TV; DualSense rumble and adaptive triggers through webOS's Bluetooth service.
- **Linux**: zero-copy DMA-BUF import of VA-API frames (4K at 120 Hz); NVDEC decoding for NVIDIA cards; a Flatpak repository or Flathub, so updates install by themselves.
- **Raspberry Pi**: publish the apt repository (GPG key, hosting, called from `release.sh`); zero-copy path (SAND frames on a DRM plane).
- **KyberNow Link OS**: install the update during standby; a composed soundtrack for the ignition, instead of `chime.rs`'s synthesis.

## KyberNow Share

Everything that lets someone else onto your computer. Name chosen on 26/10/2026; logo signed SHARE, like STATION, ADMIN and LINK.

- **Browser picture over WebTransport through the relay** (except Safari): the relay would read the name in the first QUIC packet and pass it on without decrypting; today, through the relay, the browser gets its picture over WebSocket.
- **“Auto” that follows the window**: a guest's “Auto” quality is measured when they arrive (the device's display, the decoder); redo it when their window moves to another display.
- **Share started from the PC**: the first guest sets the format and bitrate of the shared picture.
- **Removed guest**: they can come back with the same code until the host asks for a “New code”.
- **Public Suffix List**: add `share.kybernow.app` to the list, so browsers treat each Station's address (`<id>.share.kybernow.app`) as a separate site (cookies, storage).
- **Fair use of the home's upload bandwidth**: split it fairly between several guests.
- **Lending the computer** (in progress, 0.9.0; contract: [docs/pret.en.md](docs/pret.en.md)): in turns, in **console mode** in your own Windows session (full-screen launcher in KyberNow Share colours, chosen apps, PC's screen covered, take-back with a 2-minute notice, session locked at the end); one-session code, friend accounts, time slots; Steam account chosen for each loan.
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
- **Brand**: confirm the KyberNow name (and Station / Admin / Link / Share) with the Kyber team; the `fr.leslignesclaires.kybernow` identifier does not change.
