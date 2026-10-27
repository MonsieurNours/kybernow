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

- **Shared session** (in progress: local sharing, contract in [docs/kybernow-share.en.md](docs/kybernow-share.en.md), including sharing a display from the PC itself): a spectator (image and sound) or a second player (their controllers become Windows' next controller; keyboard and mouse only if the host hands them over), invited with a code, in the Link app or the browser. Kyber already accepts several devices on the same image (`multi_client`); Admin moves from a single device in session to participants who each have a role.
- **Rights enforced by the Station**: a Kyber patch to refuse a spectator's inputs and keep only a second player's controllers (today only the device decides to send nothing).
- **Separate encode for a remote guest**: the host keeps their quality, the guest gets an image that fits their connection (Kyber patch: a stream of its own beside the shared one, so the slowest device no longer slows the others down).
- **Relay**: a layer-4 relay (TCP and UDP) that does not see the image (end-to-end encryption, Kyber's certificate pinned by its fingerprint); the Station opens the connection, nothing to open on the router. A public relay run by Les Lignes Claires by default, replaceable by your own (Docker image). Next, access from outside the home without a VPN through the same relay, with a capped bitrate on the public relay. Web client: WebTransport through the relay (not in Safari).
- **Lending the computer**, in turns (Windows has only one session open on screen): a dedicated Windows account with no rights and no access to your files; the desktop replaced by a launcher that restarts the program when it closes, no File Explorer; a list of apps chosen by the host, the Station's service closes the rest. The friend plays with their own Steam account (your games: Steam Families). Three ways in: a friend access you switch on (taken back with notice), a code for one sitting, time slots. Tricky part: opening the dedicated account's session without typing its password.
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
- **Brand**: confirm the KyberNow name (and Station / Admin / Link / Share) with the Kyber team; draw the logo signed SHARE in `brand/v2/`; the `fr.leslignesclaires.kybernow` identifier does not change.
