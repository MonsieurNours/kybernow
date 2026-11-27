# Releases and updates

English · [Français](releases.md)

KyberNow releases are published on GitHub: https://github.com/MonsieurNours/kybernow/releases. The repository is public: updates download without a token, and the code stays available as the AGPL requires (with the patches to Kyber, see [kyber-changes.en.md](kyber-changes.en.md)).

One version number for all of KyberNow: KyberNow Station (installer, icon, services), KyberNow Admin and every KyberNow Link app change together. The `apps/` workspace (`apps/Cargo.toml`) gives the number of the core and of the Windows, Android, webOS and Linux apps; the `project.yml` files, that of the Mac, Apple TV and iPhone/iPad apps. The Apple TV and iPhone/iPad apps are not published as files: `release.sh` publishes their SDK, which their sideload scripts download.

## What a release contains

| File | What for |
|---|---|
| `KyberNow-Station-Setup.exe` | KyberNow Station: first installation on the Windows computer that will stream its image (full installation, or reinstallation when an update requires it) |
| `kybernow-update.zip` | Update of a Station that is already installed: Kyber overlay, Admin (with the VB-CABLE pack in `admin/vbcable`, [micro.en.md](micro.en.md)), icon, `update.ps1` |
| `KyberNow-Station-Mac.dmg` | Station installer for Mac: "Install KyberNow Station" and "Uninstall KyberNow Station", Developer ID signed and notarized (`server/macos/scripts/build-station-app.sh`) |
| `KyberNow-Station-Mac-update.zip` | Update of a Mac Station that is already installed: the whole "KyberNow Station" app, notarized |
| `KyberNow-Station-Linux-x86_64.deb`, `-arm64.deb`, `-x86_64.rpm`, `-arm64.rpm` | KyberNow Station for Linux (docs/station-linux.en.md): first install (`sudo apt install ./…deb`, `sudo dnf install ./…rpm`) and update from Admin, with the package for its architecture and format. Published in every release (`KYBERNOW_STATION_LINUX=0` to leave them out) |
| `KyberNow-Station-Linux-x86_64.tar.xz`, `-aarch64.tar.xz` | KyberNow Station for immutable systems (Bazzite, Fedora Atomic, SteamOS), installed in the user's folder, and its update from Admin (`kybernow-station-home`). Published with the packages |
| `update.ps1` | Manual update, without Admin |
| `KyberNow-Link-macOS.zip` | The Mac app, “KyberNow Link.app” (first installation: drag it into Applications); the Sparkle feed's archive |
| `appcast.xml` | Update feed for the Mac app (Sparkle), signed with EdDSA |
| `KyberNow-Link-Setup.exe` | KyberNow Link for Windows, x64 and ARM64 in a single file: per-user installer (first installation and app updates, see [apps/windows/README.en.md](../apps/windows/README.en.md)) |
| `KyberNow-Link-Android.apk` | KyberNow Link for Android and Android TV (arm64-v8a, armeabi-v7a, x86_64), signed with the release key: first installation and app updates (see [apps/android/README.en.md](../apps/android/README.en.md)) |
| `KyberNow-Link-webOS.ipk` | KyberNow Link for LG TVs (webOS 5 and later, armv7), package installed in developer mode (webOS Dev Manager or `ares-install`, see [apps/webos/INSTALLATION.en.md](../apps/webos/INSTALLATION.en.md)): first installation and updates |
| `KyberNow-Link-Linux-x86_64.flatpak`, `KyberNow-Link-Linux-aarch64.flatpak` | KyberNow Link for Linux, the Steam Deck and SteamOS (org.freedesktop.Platform 25.08 runtime, pulled from Flathub), one Flatpak bundle per architecture installed with `flatpak install --user` (see [apps/linux/INSTALLATION.en.md](../apps/linux/INSTALLATION.en.md)): first installation and updates |
| `KyberNow-Link-RaspberryPi-arm64.deb` | KyberNow Link for the Raspberry Pi on Raspberry Pi OS (Debian 13, arm64; Pi 3, 4 and 5, the chip's decoders through V4L2), Debian package installed with `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`, or through the KyberNow apt repository (`apps/linux/scripts/apt-repo.sh`), see [apps/linux/INSTALLATION.en.md](../apps/linux/INSTALLATION.en.md): first installation and updates |
| `KyberNow-Link-OS.img.xz` | KyberNow Link OS: the image to flash on a Raspberry Pi (Pi 3, 4 and 5, arm64; Raspberry Pi OS Lite + Link alone, A/B layout), with Raspberry Pi Imager (file, or the repository `https://www.kybernow.app/updates/rpi-imager.json`), see [apps/linux/INSTALLATION.en.md](../apps/linux/INSTALLATION.en.md): first installation; A/B updates, the OS downloads them itself from each release (see [apps/linux/os/README.en.md](../apps/linux/os/README.en.md)) |
| `KyberSDK-tvOS.zip` | Kyber SDK and KyberNow core (`KyberNow.xcframework`) built for Apple TV, downloaded by `apps/tvos/scripts/sideload.sh` to install the app without the App Store ([SIDELOAD.en.md](../apps/tvos/SIDELOAD.en.md)) |
| `KyberSDK-iOS.zip` | KyberNow core, libkynput, libopus and their headers built for iPhone and iPad (`apps/ios/Vendor`, attached to releases 0.4.0 and later), downloaded by `apps/ios/scripts/sideload.sh` to install the app without TestFlight or the App Store ([SIDELOAD.en.md](../apps/ios/SIDELOAD.en.md)) |
| `KyberNow-Server-Setup.exe`, `KyberNow-Setup.exe`, `KyberNow-Client-Setup.exe`, `KyberNow-macOS.zip` | Old names of the installers and of the Mac archive, identical copies: see “Old installer names” below |
| `kybernow-release.json` | Release manifest: number, channel (`stable` or `beta`), date, minimum installed version for an update from Admin, whether a reinstallation is needed, SHA-256 checksum and size of each file above (old names included) |
| `kybernow-release.json.minisig` | Manifest signature (minisign, ed25519), verified by Admin before any installation |
| `SHA256SUMS` | Checksums of all the files (old names included) |

### Old installer names

The installers are called `KyberNow-Station-Setup.exe` (the Station) and `KyberNow-Link-Setup.exe` (the Windows app), the Mac app's archive `KyberNow-Link-macOS.zip`. Their old names are also published, **identical** (copies made by `release.sh`, same checksum):

| Old name | Copy of | Why it stays |
|---|---|---|
| `KyberNow-Server-Setup.exe` | `KyberNow-Station-Setup.exe` | Admin 0.3.1 and earlier points to this name (“reinstall with KyberNow-Server-Setup.exe”); existing links to `releases/latest/download/` (website, guide, messages) |
| `KyberNow-Setup.exe` | `KyberNow-Station-Setup.exe` | The installer's name before 0.3.0: existing links |
| `KyberNow-Client-Setup.exe` | `KyberNow-Link-Setup.exe` | The Windows app 0.3.1 and earlier looks for it in the signed manifest and in the release to update itself; existing links |
| `KyberNow-macOS.zip` | `KyberNow-Link-macOS.zip` | The Mac archive's name up to 0.4.2: existing links (the Sparkle feed points to each release's own archive) |

Admin and the Windows app look for the new name first, then for the old one if it is missing (manifest of a release older than 0.4.0). The signed manifest and `SHA256SUMS` list both names, with the same checksum.

**Why they stay.** Without them, a Windows app left on 0.3.1 or earlier would no longer find its update, and Admin 0.3.1 or earlier would point to a missing file: it would have to be reinstalled by hand with the new file. The copies are listed in `LEGACY_STATION`, `LEGACY_LINK` and `LEGACY_MAC` in `release.sh` and in `FILES` in `scripts/sign-manifest.sh`; removing them from these lists removes them from the following releases.

**Pitfall.** The website goes live on every push to `main` that touches `site/`: a `releases/latest/download/` link to a new file name leads nowhere until the release that contains that file is published. For the renamed files (installers, Mac archive), `site/assets/main.js` (`LEGACY` table) falls back to the old name while the latest release only publishes that one; any other new file name waits for the release.

## Updating

**The Station, in one click.**
- Admin checks for the latest release every 6 hours, on its **channel**: `stable` (default, published releases) or `beta` (GitHub prereleases too), chosen on the Updates page (`update_channel` setting in `kyber-admin.toml`). The **Updates** page shows it with its notes and an **Install** button; the notification area icon offers “Install version X…”, which opens that page.
- **From the KyberNow Link apps** (0.11.3): when a Station has a release, the app's home screen offers it in a banner (“Update”, “Later” for 7 days), and the first launch after an app update gathers the Stations to update in a notice. Right after its own update, or facing an older Station, the app asks Admin to check right away (`POST /api/stream/station/check`, at most once a day per app and once every 10 minutes on Admin's side): without that, the Station could miss the release for up to 6 hours. Rules and texts: docs/link-ui.en.md, "Station updates".
- **Nothing installs without a signature.** For a newer release, Admin downloads `kybernow-release.json` and `kybernow-release.json.minisig` and verifies the signature with the public key compiled into its program (see “Update signing key”). Then: the manifest must announce the same number as the GitHub release (`tag_name`), a channel the Station follows (a `beta` release is refused on `stable`), a `min_installer_version` lower than or equal to the installed version and no `requires_setup` (otherwise “reinstall with KyberNow-Station-Setup.exe”). Any failure (missing manifest, invalid signature or another key's, channel, version) is shown on the page and in `GET /api/update` (`verify_error`), and the **Install** button does not appear; the icon only offers a release whose signature is good.
- The installation downloads and verifies the manifest again, then downloads `kybernow-update.zip` (never more than the manifest's size) and checks its size and SHA-256 checksum against it. The published release number must look like `X.Y.Z`: it is used as a folder name, so any other number is refused. Manifest, signature and package stay in `C:\ProgramData\KyberNow\update\<version>\` (reserved to SYSTEM and Administrators).
- Admin then launches **the applicator**, `kyber-admin update apply <package> --version X`, as SYSTEM and detached (it survives Admin stopping). It is a copy of Admin, `C:\Program Files\KyberNow\.staging\kyber-admin-maj.exe`: it must be able to rename the `admin` folder while it runs. Nothing is run from a folder another account could change, and no script is launched: PowerShell is not involved.
- The applicator verifies the signed manifest and the package again (nothing comes from the Admin process that launched it), then extracts the package into `C:\Program Files\KyberNow\.staging\<version>\paquet` (permissions inherited from Program Files: read-only for users). It refuses any path that would leave the folder (`..`, absolute path, drive, NTFS stream), any name that would not be an ordinary file on Windows (`CON`, `NUL`, `COM1`…, a name ending with a dot or a space, `<>"|?*`) and any link. It also refuses a release that is not newer than the installed one (going back has its own path). The package must contain `kyber`, `admin`, `tray` and a `version.txt` equal to the expected version. A failure at this stage touches nothing: Admin is still running and shows it.
- It stops the icon, then the Kyber server (`kyber` and its processes), then Admin (`kyber-admin` and its agents), then the Kyber server again if Admin had restarted it in the meantime. It also stops the microphone and camera helpers (`kybernow-micro.exe`, `kybernow-camera.exe`, the same list as the installer and `update.ps1`: `server/admin/src/update/windows_common.rs`) and Frame Server (the Windows 11 camera); a camera library still loaded by an app that films is set aside in `.staging\verrouilles\` (deleted at the next restart). The automatic restart of `kyber` and `kyber-admin` after a crash is suspended until the restart, so that Windows does not restart them in the middle of the renames; Admin sets it again at every start anyway. One installation at a time: the applicator, `update.ps1` and the installer take the `Global\KyberNowUpdate` lock. It prepares each new folder in `.staging\<version>\` (a copy of the current folder, Kyber's settings and certificate included, with the package's files on top), then switches by renaming: `kyber` becomes `kyber.prev` and `.staging\<version>\kyber` becomes `kyber`, and the same for `admin` and `tray`. Kyber's logs (`kyber\log`) are moved, not copied. It then restores the services' permissions (`sc sdset`) and the Bonjour firewall rule.
- **PC freezing during or just after the update.** On 03/11, just after 0.9.3, a freeze left `kyber_config.toml` and `kybertest_key.pem` filled with zeros (files written that Windows had not yet put on disk): Kyber no longer started. Since 0.9.4: before each update and each rollback, the applicator backs up Kyber's settings, certificate and key into `C:\ProgramData\KyberNow\backups\` if they are readable (`kyber_config-<date>.toml`, `kybertest_cert-<date>.pem`, `kybertest_key-<date>.pem`); every extracted or copied file is forced to disk (`FlushFileBuffers`) before the switch, renames are written at once (`MOVEFILE_WRITE_THROUGH`), and the whole volume is forced to disk before and after the switch and after a rollback. If a file is damaged anyway, Admin repairs it when it starts (see server/README, “Damaged Kyber settings”).
- It restarts the services, then checks the new version's health: within 60 s, Admin must answer at the address of its `listen` setting (127.0.0.1 if it listens everywhere, no proxy) with `GET /api/session` giving the new version, and the Kyber server must be running. Otherwise it stops everything (the icon included, which the new version of Admin may have relaunched), puts each folder back where it was (`kyber.prev` becomes `kyber` again…), restarts the old version and writes the reason in the log and in `C:\ProgramData\KyberNow\update\resultat.json`. Admin reads that file when it starts: the Updates page shows the failure, and `GET /api/update` returns it in `install_error` and `last_failure` (version, reason, rollback, log). If a folder resists the rollback, the applicator puts the others back, keeps `.staging\<version>`, writes it down (`undo_failed`) and the page says so, even when the failed version is the one running.
- **Power cut during the switch.** Before the first rename, the applicator writes `update\bascule.json` (forced to disk); it erases it once every folder is in place, and writes it again during a rollback. If the power goes out between two renames, Admin finds it when it starts: the running console (the old one, or the new one if its folder already switched) finishes the job its way, any missing part taken back from `.prev` or `.staging`, and the Updates page says so. If `admin\` itself is missing, the installer takes care of it before looking for the settings to keep.
- Whatever happens once the services are stopped (error, a rename that fails because a file is open), the applicator restarts them on its way out: the Station is never left without its Kyber server and Admin.
- When it succeeds, the update writes the new version into `C:\Program Files\KyberNow\kybernow.json` and into “Installed apps” (`DisplayVersion`), and `resultat.json` says `"ok": true`. The previous version stays in the `.prev` folders until the next update (about 300 MB). In `update\`, only the folders of the two latest versions are kept; `.staging\<version>` is deleted.
- Admin relaunches the icon in the user's session, with their account (`update\relaunch-tray` file). The page updates itself.
- **Going back to the previous version**: as long as the `.prev` folders exist, the Updates page offers this button (`POST /api/update/rollback`, previous version in `rollback` of `GET /api/update`). The same applicator switches in the other direction (`kyber-admin update rollback`): `kyber.prev` becomes `kyber` again and the removed version goes to `kyber.bad`, and the same for `admin` and `tray`; Kyber's settings changed since the update (`kyber_config.toml`) and its logs are kept, the settings only if they are readable (valid TOML, no null bytes, a `[kycontroller]` table): otherwise those of `kyber.prev` stay, and the log says so. On 03/11, copying the damaged file had overwritten the good one. The automatic return after a failure copies nothing: it puts the old folders back as they were. Same health check, with the previous version; if it fails, the version that was running is put back. When it succeeds, the rollback deletes the `.bad` folders: no `.prev` is left, and the removed version is offered for installation again.
- **During a session.** If a device is in session, `POST /api/update/install` (and `POST /api/update/rollback`) answers 409 with “Un appareil est connecté (…) : l'installation coupera sa session” (a device is connected: installing will cut its session); the page asks before going on, and then sends `{"force": true}`.
- Duration: about one minute. A streaming session in progress is cut. Log: `C:\ProgramData\KyberNow\update\<version>\update.log`; the applicator's output goes to `lancement.log`, to read when `update.log` does not exist.
- Admin follows the applicator. If the applicator stops while Admin is still running (verification or extraction refused), Admin says so: Updates page, end of the log, and “Install” becomes possible again.
- Steam's virtual speakers, set up by the installer, are not redone by the applicator; `update.ps1` (by hand) redoes them. After a successful switch, the applicator sets the installer's display names (services, “Installed apps”, Start menu shortcuts): a Station installed with 0.3.1 or earlier thus gets the KyberNow Station and KyberNow Admin names. A failure at this step is only written to the log.
- **Stations on 0.2.5 and earlier.** Their console (Admin's name in those versions) knows neither the manifest nor the applicator: it installs the package with `SHA256SUMS` and the `update.ps1` it contains. One more reason for every release to publish `SHA256SUMS` and `update.ps1`, which the manual path uses too.

**The Station, by hand** (administrator PowerShell):

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

The script downloads `kybernow-update.zip` and `SHA256SUMS`, checks the package's checksum and its requirements (`requirements.json`: a release that needs the installer, or a Station installed before `min_installer_version`, is refused with a message pointing to `KyberNow-Station-Setup.exe`, before anything is stopped), refuses a release that is not newer than the installed one (unless `-Force`: `& ([scriptblock]::Create((irm …/update.ps1))) -Force`), then performs the same switch with a health check. If it fails, it says so in red (“Mise a jour non installee…”) after restarting everything.

**Manual path: integrity only.** `update.ps1` checks that the package is not damaged (`SHA256SUMS`), not that it comes from KyberNow: `SHA256SUMS` is published in the same place as the package, and verifying the minisign manifest would need a tool that is downloaded too (the release's `kyber-admin.exe`), which goes round in circles. The script itself is not signed. It therefore does not give Admin's guarantees: keep it for troubleshooting, on a release whose GitHub page you know. The `.exe` files and `update.ps1` are not Authenticode-signed (see “Windows signing (Authenticode)”).

To try a package served from the Mac before publishing it: `$env:KYBERNOW_UPDATE_BASE = 'http://<adresse-du-Mac>:8000'`, then the same command, with `python3 -m http.server 8000` running in `server/installer/dist` (`package-update.sh` also writes `SHA256SUMS` there).

**Testing the verification and the applicator without Windows**: `cd server/admin && cargo test`. The tests cover the manifest (reading the published format, version comparison, channel, `min_installer_version`, `requires_setup`), the signature (test key generated in the test: valid signature, manifest changed by one byte, another key, trusted comment that does not match), a manifest actually produced by `scripts/sign-manifest.sh` and the `minisign` binary (`server/admin/src/update/testdata/`, throwaway test key of which only the public key is kept), extraction (`..`, absolute path, drive, links), the switch on a fake installation folder (success, failed health check then rollback, forged package that touches nothing, going back to the previous version and its failure) and the refusal during a session. Windows' services, processes and permissions go through a simulated trait: they are only checked on the PC. To regenerate `testdata/`: `KYBERNOW_MINISIGN_KEY=<temporary folder>/essai.key KYBERNOW_MINISIGN_ITEM=kybernow-minisign-essai scripts/sign-manifest.sh <folder> 0.2.7 beta --min-installer 0.2.6` on a 15-byte `kybernow-update.zip`, then delete the private key and the Keychain item.

**Testing `update.ps1` without Windows**: `pwsh -NoProfile -File server/installer/tests/update-bench.ps1` (`brew install powershell`). The bench simulates the services and Admin, and replays a successful update, a silent Admin (rollback), an incomplete package, a blocked rename, and a package that needs the installer or a more recent Station.

**The Windows app (KyberNow Link).** It checks at launch then every 6 hours for the latest version of its channel (app settings: stable, or betas too), verifies the signed manifest like Admin and the installer (`KyberNow-Link-Setup.exe`, or `KyberNow-Client-Setup.exe` in the manifest of a release older than 0.4.0) against it (size, SHA-256), then runs it silently (`/S /UPDATE`): it waits for the app to close, replaces its files and relaunches it. A version without a Windows installer is not offered.

**The Android app.** At launch, when you come back to the app, then every 6 hours, it looks for the latest release on its channel that publishes `KyberNow-Link-Android.apk` (app settings › Updates: stable, or betas too). The Rust core checks the signed manifest as for the Windows app; the downloaded APK must match its size and SHA-256. A button downloads it, then the app installs it through PackageInstaller: Android asks once to allow the app to install apps (on the Shield: Settings › Apps › Security & restrictions › Unknown sources), then confirms each installation, except from Android 12 on, where the app updates itself without asking. Android refuses an APK signed with a different key than the installed app: an app built by Android Studio (debug key) must be uninstalled once before installing the published APK.

**The LG TV app.** At launch then every 6 hours it looks for the latest release on its channel that publishes `KyberNow-Link-webOS.ipk` (Settings › This TV › Updates: stable, or betas too), checked by the Rust core as for Android (signed manifest, package checksum and size). An app installed in developer mode cannot install itself: it shows "Version X available" on the home screen and in its settings, and the package is installed from the computer (webOS Dev Manager or `ares-install`). `release.sh` builds it with `apps/webos/scripts/build.sh` (webOS toolchain downloaded on first run, Rust target `armv7-unknown-linux-gnueabi`, Kyber's sources).

**The Linux app.** At launch then every 6 hours it looks for the latest release on its channel that publishes the Flatpak for its architecture (`KyberNow-Link-Linux-x86_64.flatpak` or `-aarch64`), checked by the Rust core (signed manifest, checksum and size). A Flatpak bundle installed by hand does not update itself: the app shows "Version X available" and the new bundle is installed over it (`flatpak install --user`). `release.sh` builds both bundles with `apps/linux/scripts/release-flatpaks.sh`, in the Parallels VM "KyberNow Linux" (Ubuntu ARM64, Rosetta for x86_64, repository as a shared folder), started when needed; the same script then builds the Raspberry Pi .deb there (`apps/linux/scripts/deb.sh`, Raspberry Pi's FFmpeg). On the Pi, a package installed from the file updates like the Flatpak ("Version X available", then `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`). `apps/linux/scripts/apt-repo.sh` assembles an apt repository from the .deb packages (signed if a GPG key is given), to be served by a static site; `release.sh` does not call it and no repository is published. **KyberNow Link OS**: `release.sh` then builds the image in the same VM (`apps/linux/os/release-image.sh`, pi-gen, long) and writes the Raspberry Pi Imager repository JSON (`apps/linux/os/imager-json.sh` → `site/updates/rpi-imager.json`, committed with the site feed); on the Pi, the OS looks for `KyberNow-Link-OS.img.xz` in each release (signed manifest), downloads it and writes it into the other A/B slot (Device settings › Update).

**The Mac app.**
- Sparkle checks every day, and on request (KyberNow menu › Check for Updates…). The app downloads the archive, verifies its EdDSA signature with the app's public key (`SUPublicEDKey`), replaces itself and relaunches.
- The app reads the website's cumulative feed, `https://www.kybernow.app/updates/appcast.xml`, which lists every release. It only offers stable releases, unless "Receive beta versions" is ticked in KyberNow Link › Settings…: see "Mac app update feed".
- The private key is in this Mac's Keychain (item created by Sparkle's `generate_keys`). Back it up, and never put it in the repository:

```bash
apps/macos/build/SourcePackages/artifacts/sparkle/Sparkle/bin/generate_keys -x ~/sparkle-kybernow.key
```

Without it, no app update could be signed any more. To restore it on another Mac: `generate_keys -f ~/sparkle-kybernow.key`.

**The Mac Station** (docs/station-macos.en.md, milestone 6) follows the same path up to the verified package, with `KyberNow-Station-Mac-update.zip` instead of `kybernow-update.zip`; a release whose manifest doesn't list that package is refused on Mac ("reinstall with `KyberNow-Station-Mac.dmg`" if it requires the installer). The applier (`server/admin/src/update/mac.rs`) is a copy of Admin started by launchd as a separate job (`fr.leslignesclaires.kybernow.station.update`): launchd would stop with Admin anything Admin starts. It extracts the app (`ditto`), checks its signature again (`codesign --verify --deep --strict`, identifier `fr.leslignesclaires.kybernow.station`, KyberNow's team built into Admin, `P6DB682S4K`, checked through a code requirement, expected version), stops the icon, Admin and Kyber, puts the app in place by renaming (the old one into `update/previous`, for "Go back to the previous version"), restarts everything and waits for Admin to answer in the new version within 60 s, otherwise puts the old one back. Same signature: macOS keeps the permissions, and nothing asks for the Mac's password; a newer sound output in the app is only noted in the log (installing it would need that password). Log in `~/Library/Application Support/KyberNow/Station/admin/update/<version>/update.log`, result in `update/resultat.json` as on Windows. The icon's "Update" window goes through Admin's local routes (`GET /api/local/update`, `POST /api/local/update/check`, `POST /api/local/update/install`, local token and pinned certificate) and follows the Station until it comes back.

**The Linux Station** (docs/station-linux.en.md, "Updates and tray icon") follows the same path up to the verified package, with the package for its architecture and format (`KyberNow-Station-Linux-{x86_64,arm64}.{deb,rpm}`); a release that doesn't publish it is refused on Linux. Admin, which is never root, drops its request on the `/run/kybernow-station/update.sock` socket; the root service `kybernow-station-update@.service` (`kyber-admin update serve`, `server/admin/src/update/linux.rs`) only accepts the Station's account (the first one that asked for an update, noted in `proprietaire`), one request at a time (`flock`) and 10 s to read it; it copies the files of the requesting account, opened with that account's rights and without following links, into `/var/lib/kybernow-station/update/<version>/` (0700, files 0600, erased when the check fails), checks everything again there (signature, version newer than the installed package, channel, hash), answers Admin, then stops that account's tray icon, Kyber and Admin, installs the package (apt or dpkg, dnf or rpm), restarts the services and waits for Admin to answer in the new version within 60 s, otherwise reinstalls the version that was running (its published package, downloaded and checked by Admin before installing, kept by the root service). "Go back to the previous version" reinstalls the kept version the same way. Log in `/var/lib/kybernow-station/update/<version>/update.log` (and `journalctl -u 'kybernow-station-update@*'`), result in `resultat.json` next to it, same format. Publishing: `release.sh` builds the four packages with `server/linux/scripts/release-station.sh` (arm64 in the "KyberNow Linux" VM, x86_64 on the `KYBERNOW_STATION_X86_SSH` machine when set, otherwise in the same VM's x86_64 chroot through Rosetta, `server/linux/scripts/x86-chroot.sh`; repository at the same commit) unless `KYBERNOW_STATION_LINUX=0`, and `sign-manifest.sh` lists them in the manifest. A release published with `KYBERNOW_STATION_LINUX=0` doesn't have them, and a Linux Station doesn't update to it.

## Update signing key (minisign)

Each release publishes a manifest, `kybernow-release.json`, signed with an ed25519 key (minisign), checked by Admin and by the Rust core of the apps that update themselves or announce their releases (Windows, Android, LG TVs, Linux, KyberNow Link OS):

```json
{
  "version": "0.2.6",
  "channel": "stable",
  "published": "2026-10-21T08:00:00Z",
  "min_installer_version": "0.2.0",
  "requires_setup": false,
  "files": {
    "kybernow-update.zip": { "sha256": "…", "size": 123456789 },
    "…": {}
  }
}
```

`files` lists those of the published files (`scripts/release-fichiers.txt`, the list `release.sh` also reads) that are present (`KyberNow-Station-Setup.exe`, `kybernow-update.zip`, `update.ps1`, `KyberNow-Link-macOS.zip`, `KyberNow-macOS.zip`, `appcast.xml`, `KyberSDK-tvOS.zip`, `KyberSDK-iOS.zip`, `KyberNow-Link-Setup.exe`, `KyberNow-Link-Android.apk`, `KyberNow-Link-webOS.ipk`, `KyberNow-Link-Linux-x86_64.flatpak`, `KyberNow-Link-Linux-aarch64.flatpak`, `KyberNow-Link-RaspberryPi-arm64.deb`, `KyberNow-Link-OS.img.xz`, and the old installer names `KyberNow-Server-Setup.exe`, `KyberNow-Setup.exe`, `KyberNow-Client-Setup.exe`, see “Old installer names”). The detached signature, `kybernow-release.json.minisig`, carries the trusted comment `kybernow <version> <channel>`. The public key is in the repository, `server/admin/src/update/kybernow-release.pub` (key ID `D63B24FFFC7E79D1`), and compiled into Admin and into the apps' core (`apps/core/src/update.rs`): a SHA-256 checksum alone protects against a damaged download, not against a release forged by whoever controls the GitHub account; the signature does.

The manifest is written and signed with:

```bash
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable            # or beta
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable --min-installer 0.2.6 --requires-setup
```

`--min-installer X.Y.Z`: oldest version from which Admin can install this package (0.2.0 by default). `--requires-setup`: this release only installs with `KyberNow-Station-Setup.exe` (new driver, new service…). The script installs `minisign` with Homebrew if it is missing, then checks the signature against the repository's public key: a private key that does not match it is caught before publishing.

**The private key** is in `~/Library/Application Support/KyberNow/minisign.key`, encrypted with a random password kept in the Keychain (item `kybernow-minisign`). The script creates it if it is missing and writes the public key into the repository; if a public key is already in the repository, it refuses to create another one: a new key would make every Station already installed refuse all updates, and they would have to be reinstalled with `KyberNow-Station-Setup.exe`.

**Offline backup** (USB stick, password vault), like the Sparkle key, never in the repository: the private key and its password, one without the other is useless.

```bash
cp ~/Library/Application\ Support/KyberNow/minisign.key /Volumes/<backup>/kybernow-minisign.key
security find-generic-password -s kybernow-minisign -w     # password, to copy into the vault
```

**Restoring on another Mac**:

```bash
mkdir -p ~/Library/Application\ Support/KyberNow
cp /Volumes/<backup>/kybernow-minisign.key ~/Library/Application\ Support/KyberNow/minisign.key
chmod 600 ~/Library/Application\ Support/KyberNow/minisign.key
security add-generic-password -a kybernow -s kybernow-minisign -w   # paste the password at the prompt
```

Then a dry run on a folder holding any `update.ps1` (`scripts/sign-manifest.sh <folder> 0.0.0 stable`, followed by `minisign -V -p server/admin/src/update/kybernow-release.pub -m <folder>/kybernow-release.json`) confirms that the key matches the repository.

The password can only be read while the Mac is unlocked, like the notarisation profile.

## Android app signing key

Android only installs an update signed with the same key as the installed app: this key must never change or be lost. `apps/android/scripts/release-apk.sh` (run by `release.sh`) creates it the first time:

- `~/Library/Application Support/KyberNow/android-release.jks` (PKCS12, alias `kybernow`, RSA 4096, 100 years), protected by a random password kept in the Keychain (item `kybernow-android-keystore`);
- the SHA-256 fingerprint of its certificate in the repository, `apps/android/scripts/release-cert.sha256`, to commit: the script then refuses any other key, and checks with `apksigner` that every published APK is signed by this certificate.

If the fingerprint is in the repository and the key is missing, the script refuses to create another one: restore the backup.

**Offline backup**, alongside the minisign key, never in the repository:

```bash
cp ~/Library/Application\ Support/KyberNow/android-release.jks /Volumes/<backup>/
security find-generic-password -s kybernow-android-keystore -w     # password, copy it into your vault
```

**Restoring on another Mac**: copy the file back to the same place (`chmod 600`), then `security add-generic-password -a kybernow -s kybernow-android-keystore -w` (paste the password), and check with `apps/android/scripts/release-apk.sh --check`.

## Publishing a release

0. Check that the CI is green on `main` (the repository's Actions tab, see "Continuous integration"): it does not cover the Mac app, which the rehearsal builds.
1. Add a `## X.Y.Z` section at the top of `CHANGELOG.md`: these are the notes shown by Admin and by the apps. Add the same section, in English, to `CHANGELOG.en.md`. Commit: the script refuses a modified tree. Both languages ship with the release ([docs/i18n.en.md](i18n.en.md)): the GitHub release carries the French section, a `<!-- en -->` line, then the English one (Admin and the apps that update through the Rust core take the part in their language); in the Sparkle feed, English is the default description and French a per-language link (`xml:lang="fr"`) to `site/updates/notes/X.Y.Z/KyberNow-Link-macOS.fr.html` (`scripts/notes-html.py`), committed with the site feed.
2. Nothing to rebuild by hand: the Windows Station's Kyber overlay, the Mac Station's Kyber, the apps' client libraries and libkynput are rebuilt by the release, in its build trees, when their patches changed. KyberNow Link may stay open (the trees have their own `Vendor/`). Then, from `main`:

```bash
scripts/release.sh X.Y.Z --check        # checks only (about ten seconds)
scripts/release.sh X.Y.Z --repetition   # full rehearsal, nothing committed or published
scripts/release.sh X.Y.Z                # release
scripts/release-suivi.py X.Y.Z          # in another terminal: major steps ticked off and a percentage
```

**Progress view.** `scripts/release-suivi.py X.Y.Z` redraws every two seconds the major steps (checks, versions, trees, the fifteen builds (fourteen without the Linux Station), files, feeds, manifest, publication), ticked off as they finish, with a bar and a percentage (builds 80 %, before 5 %, after 15 %); it reads the log `~/Library/Logs/kybernow-release-X.Y.Z.log` (run the release with `2>&1 | tee` to that file) and drives nothing.

**Order.** The published binaries must be those of the tagged commit (the AGPL requires publishing the exact code of the binaries):
1. pre-checks, fast and all shown at once (table below); a single failure and nothing is modified;
2. version alignment (Admin, tray icon, Station installer, the `apps/` workspace — core and the Windows, Android, webOS and Linux apps —, the Windows app's installer and `tauri.conf.json`, the Xcode projects of the Mac, Apple TV and iPhone/iPad apps, the Flatpak's AppStream file, `Cargo.lock` included) in the `third_party/arbres/versions` tree, and the "KyberNow X.Y.Z" commit; `main` fast-forwards to it; an alignment touching any other file stops everything;
3. builds from that commit, each in its **build tree** ([build-notes.en.md](build-notes.en.md), "Build trees"), in five parallel lanes:
   - Windows, Android, webOS: the Windows Station's Kyber overlay (if its patches changed), the Station installer and update package, the Windows app x64 and ARM64 and its `KyberNow-Link-Setup.exe` installer, the signed Android APK (`apps/android/scripts/release-apk.sh`), the webOS package;
   - Apple: the Mac app signed with Developer ID (Kyber client libraries and Rust core included), the Kyber SDK and core for the Apple TV (`KyberSDK-tvOS.zip`) and the Apple TV app, the core and libraries of the iPhone/iPad app (`KyberSDK-iOS.zip`; both apps built unsigned to check them), the Mac Station (its Kyber if its patches changed, app, `.dmg` and update package, notarised);
   - notarisation of the Mac app, started as soon as the app is signed, stapling, Gatekeeper;
   - Flatpak aarch64, then x86_64 once the KyberNow Link OS image is done ("KyberNow Linux" VM);
   - the Raspberry Pi `.deb` then the KyberNow Link OS image (same VM);

   a tree modified by its build (tracked file) stops everything: the binaries would no longer match the commit;
4. checks of the files ("Checked before publishing", second table);
5. signed Sparkle feed, the website's cumulative feed (`site/updates/appcast.xml`) completed with this release, French notes (`site/updates/notes/X.Y.Z/`) and the Raspberry Pi Imager repository (`site/updates/rpi-imager.json`, `apps/linux/os/imager-json.sh`), committed on top of the version commit ("Flux des mises à jour : KyberNow X.Y.Z"); `SHA256SUMS`, signed manifest `kybernow-release.json` (`scripts/sign-manifest.sh`, minisign), checked again (checksums, sizes, version, channel, signature with Admin's public key);
6. GitHub release created **as a draft** with all the files;
7. `vX.Y.Z` tag on the version commit; 8. push of `main` (version commit and feed commit) and of the tag, at once (`--atomic`): the Pages workflow publishes the feed;
9. publication of the draft;
10. summary: duration of each step, rebuilt or reused, total (also in `dist/.journaux-vX.Y.Z/resume.txt`).

The files are gathered in `dist/vX.Y.Z/` (`dist/vX.Y.Z-repetition/` for a rehearsal). The version Windows shows for `KyberNow-Station-Setup.exe`, `kybernow-ui.exe` and `kybernow-tray.exe` (Properties › Details, manifests) follows `Cargo.toml`: their `build.rs` generate the resources from the `resources/*.rc.in` and `*.manifest.in` templates.

**During the builds**, the terminal shows the state of each step (waiting, running with its duration, rebuilt, reused, failed), updated in place; outside a terminal (log, background task), one line per change and a progress line every five minutes. **Logs**: each step writes all its output to `dist/.journaux-vX.Y.Z/<step>.log` (builds in the VM are brought back there, and kept in the VM's `~/flatpak-<arch>.log`, `~/deb.log`, `~/os-image.log`). **A failure** shows the end of its log at once; the other lanes finish their current step then stop ("annulée"), nothing is published, and the end of each failed step's log is shown again at the end with the summary.

**Resuming: per-step fingerprints.** Each step has a fingerprint of **its own inputs**: the git objects, in the built commit, of the folders it reads (for the Windows app, for instance: `apps/windows`, `apps/core`, `apps/Cargo.toml` and `Cargo.lock`, the client and Station patches, `server/relay`, `brand`, `scripts/kyber-*.sh`), the tool versions (stable and nightly Rust, cargo-c, Xcode, xcodegen, MinGW, Node, meson, CMake, Java, NDK, macOS; the VM's Flatpak and Rust for Linux), the origin commit of its tree's Kyber sources, and the `third_party/` dependencies it reads (names, sizes and dates of their files: ARM64 rootfs and llvm-mingw for Windows, the webOS toolchain…). After a success, its files and fingerprint are kept in `third_party/arbres/.etapes/<step>/`. On a re-run, a step whose fingerprint did not change and whose files are intact is **reused** without compiling anything; a fix only rebuilds the steps whose inputs it touches, and the published files all come from the tagged commit (the inputs are files of that commit). When in doubt, an input is counted: a change in `apps/macos` rebuilds the Apple TV and iPhone/iPad apps, which compile some of its sources; a Kyber patch rebuilds everything that applies it. Version numbers are inputs: a new version rebuilds everything; a rehearsal followed by the release of the same version reuses everything except the Apple apps (their build number is the date) and what depends on today's date (AppStream file). **KyberNow Link OS** depends on its files (`apps/linux/os`) and on the Raspberry Pi `.deb` it contains (fingerprint of the `.deb` step): it is rebuilt for each new version (the `.deb` carries the version), reused otherwise (re-run, rehearsal then release). Notarisation, feeds, checksums and the release are redone every time.

**Inputs in one place.** What goes into each step's fingerprint lives in `scripts/release-entrees.sh` (repository paths, Kyber sources, `third_party/` dependencies), plus the **recipe** itself (`declare -f construire_<id>`: changing how a step is built rebuilds it) and its list of files. The nightly counted is the one the scripts actually run (`cargo +nightly`), no longer `nightly-<date>`, which was often absent. Kyber for Mac is rebuilt when `apps/macos/PyroWave` changes (it is in its fingerprint, `kyber_station_hash` in `server/kyber-patches/apply.sh`, which `build-kyber.sh` writes and `release.sh` compares): the Mac Station's encoder and the apps' decoder come from the same commit. The Linux Station counts the app patches it applies (pen), the licences, and the commits of the Kyber sources in the VM and its x86_64 chroot, read over ssh during the checks. `scripts/tests/release-outils.sh` reads each recipe and, step by step, the scripts it runs and the xcodegen `project.yml` files: a repository file they cite that is not in the step's fingerprint fails the test (justified exceptions in `scripts/tests/entrees-exceptions.tsv`). The steps and their labels are in `scripts/release-etapes.txt` (also read by `release-suivi.py`), the published files in `scripts/release-fichiers.txt` (read by `release.sh`, `sign-manifest.sh` and `release-verifie.py`).

**A lane that dies** (disk full, `set -e`) before finishing its steps: they are marked failed and the other lanes stop, instead of waiting forever for a step that will never come. **Ctrl-C** also stops, in the VM, the commands started from the `linux` tree before releasing the locks. In the VM, the x86_64 Flatpak also waits for the Linux Station: both go through Rosetta. An error inside a `release.sh` function is reported ("Échec pendant…", `set -E`), and temporary folders are removed on exit.

**Durations** (M4 Max Mac, rehearsals of 0.11.1 on 25/11/2026, trees already built once): pre-checks 7 to 8 s; rebuilding everything with warm caches, about 10 min (the x86_64 Flatpak under Rosetta and KyberNow Link OS, one after the other in the VM, make the longest path: ~11 min); a fix to a Linux script rebuilds the Apple and Linux apps and reuses the rest: 12 min; nothing changed: 30 s (everything reused, then checks, feeds and manifest). For comparison, the 0.11.0 release, serial: ~11 min without Linux, ~18 more for Linux, the image and notarisation. In the VM, the x86_64 Flatpak waits for pi-gen to finish: started together, `rustc --version` under Rosetta returned an empty output and the build failed.

**Re-running after a failure**: fix, commit on top, run the same command again. A "KyberNow X.Y.Z" commit at the head (or right below the feed commit) is reused; if `HEAD` already has its versions aligned on X.Y.Z (a fix committed after the version commit), `HEAD` is built and tagged, without a new version commit. An existing draft receives the new files (`--clobber`), the tag is created if missing. Only an already published release is refused. If the failure came after step 5, the feed commit is at the head with the version commit right below it: it is recognised and the feed redone; its commit is replaced if it has not been pushed yet, followed by a new one otherwise.

**One release at a time.** The release (or rehearsal) takes a lock on `third_party/arbres/publication` and on each of its trees ([build-notes.en.md](build-notes.en.md), "Locks"); another one started at the same time, or a hand build in one of these trees, is refused with the name of the one holding it.

**Checked before building** (about ten seconds; in a rehearsal, what would only block a release is a "note"):

| Check | If it fails |
|---|---|
| Branch `main`, clean tree (`git status --porcelain` empty, ignored files excepted, as well as the Xcode projects' shared schemes, which an open Xcode rewrites in its own format), `origin/main` included | commit, `git switch main`, `git pull` |
| `## X.Y.Z` section in `CHANGELOG.md` and `CHANGELOG.en.md` | add it to both |
| No `vX.Y.Z` tag other than on the version commit, no `vX.Y.Z` release already published; `gh` logged in | `git tag -d vX.Y.Z` if left over from a rehearsal; `gh auth login` |
| No other release running, no tree held by another build | wait, or see who holds it (message) |
| Mac disk: at least 40 GB free (note under 80) | [build-notes.en.md](build-notes.en.md), "Disk space" |
| Notarisation: `xcrun notarytool history` answers ("kybernow" profile readable, Mac unlocked); an expired Apple agreement answers 403 and is named as such | accept the agreement on developer.apple.com (Account › Agreements) and App Store Connect; or `xcrun notarytool store-credentials` (below) |
| `scripts/sign-manifest.sh`, cumulative feed `site/updates/appcast.xml` | without a signed manifest, Admin refuses the update; `scripts/rebuild-appcast.sh` once |
| Android app: release key and tools (`release-apk.sh --check`); webOS app: Rust target `armv7-unknown-linux-gnueabi` | "Android app signing key"; `rustup target add …` |
| Shared sources and dependencies: `kyber-desktop`, `kyber-desktop-macos`, `kyber-web`, Kyber 0.29's SDL2, ARM64 rootfs, llvm-mingw | [build-notes.en.md](build-notes.en.md) |
| Windows app: **linking** of a small program against the same libraries as `kybernow.exe` (`apps/windows/scripts/link-check.sh`, x64 against the libavcodec of the tree's Station, ARM64 against its own): a missing symbol is named within a second | see the symbol: in 0.11.0, the PyroWave encoder of the server's libavcodec without its import library |
| Kyber overlay and Mac Station Kyber of their trees: current patches (fingerprints, both from `kyber_station_hash` in `server/kyber-patches/apply.sh`, the list shared by the three Stations: Kyber for Mac's also covers `apps/macos/PyroWave`, its PyroWave encoder's source; the overlay's, computed by `server/kyber-patches/stamp.sh` for `build.sh` and for the release alike, also covers the sources of the micro, camera and encoder-check helpers: in 0.11.7, the fixed helpers shipped in their old version; when it changes, `build.sh` starts again from the tree's original sources, `arbre_kyber_origine`; it is computed in the tree, at the version commit, which changes the helpers' `Cargo.toml`) | nothing: reported ("note"), the release rebuilds them |
| "KyberNow Linux" VM: startup view "headless" (set if needed, never a window that would grab the keyboard), reachable (started if needed), tools, Rosetta in binfmt, 20 GB free for pi-gen | [apps/linux/README.en.md](../apps/linux/README.en.md); `sudo rm -rf ~/kybernow-os/pi-gen/work` in the VM if the disk is full |
| Flatpak copy: `apps/linux/scripts/sources.sh` (origin Kyber sources) contains what `build.sh` reads, the client and webOS patches apply to it, the reduced workspace resolves (`cargo metadata`) | the named file or patch |
| Cargo dependencies downloaded: `cargo fetch --locked` in each workspace (after a merge, a new `Cargo.lock`) | network, or `Cargo.lock` to update |
| Tools: xcodegen, xcodebuild, npm, cargo, zstd, MinGW, zip, ditto, python3, xmllint, rsync, minisign, meson, cmake, llvm-mingw's llvm-objdump | Homebrew |
| Rust nightly (`rustup run nightly`) present; a date other than the one in `scripts/kyber-toolchain.sh` is a "note" | `rustup toolchain install nightly-<date>` |
| Release tooling tests (`scripts/tests/release-outils.sh`: locks, trees, patches, step inputs, published files), a few seconds | the test named; a missing input goes into `scripts/release-entrees.sh` |
| Each tree's Kyber sources at the same commit as the main repository's `third_party/` (copied once: Kyber updated there never reached them) | "note": `scripts/arbres.sh recopier <variant>` |

**Checked before publishing** (`scripts/release-verifie.py`, then Apple's and Android's tools):

| Check | On what |
|---|---|
| Present and of a plausible size (at least 1 MB, 200 MB for the image; a few KiB for `update.ps1`, feed and manifest) | each published file, old names included |
| Version X.Y.Z readable inside | Mac and Mac Station apps (`Info.plist` in the archives), `kybernow-update.zip` (`version.txt`), Windows installers (version resource), `.deb` and `.ipk` (`Version` field), APK (`versionName`, `aapt2`), `appcast.xml`; fields of `KyberNow-Link-OS.img.xz.info` |
| Developer ID signature, stapled notarisation ticket, Gatekeeper | Mac app, Mac Station app (update package), Mac Station `.dmg` |
| APK signed by the release key (`apksigner`, certificate of `apps/android/scripts/release-cert.sha256`; that file missing or unreadable fails the check, it no longer accepts any key) | `KyberNow-Link-Android.apk` |
| DLL dependencies (`llvm-objdump -p`): Windows DLLs or DLLs shipped alongside only | the tree's `apps/windows/dist/x64` and `arm64`; Kyber overlay (with Kyber 0.29) |
| Windows subsystem (no terminal window) of the helpers started in the user's session: `kybernow-micro.exe`, `kybernow-camera.exe` | Kyber overlay |
| Correct `SHA256SUMS`, consistent manifest (each file: checksum, size; version, channel; every published file present, from `scripts/release-fichiers.txt`, in both), valid minisign signature with Admin's public key | `dist/vX.Y.Z/` |

**Channels.** A stable release is GitHub's "latest release" (`releases/latest`), the one Admin and the Mac app see. `--beta` publishes a *prerelease*, never marked "latest": its manifest has `"channel": "beta"` and its Sparkle feed `<sparkle:channel>beta</sparkle:channel>`. The number stays `X.Y.Z` (Admin refuses any other format): a beta takes the next number, and the stable release that follows it a higher one. The Mac app reads the website's cumulative feed, which combines both channels: it only offers a beta if the user asked for them ("Mac app update feed", below).

**Rehearsal (`--repetition`, or `--dry-run`).** Everything a release does — same checks, same parallel builds in the same trees, reuse of unchanged steps, notarisation (except with `--sans-notarisation`), file checks, feeds, signed manifest —, without commit, release, tag or push, which it prints (`[répétition, non lancé] …`). The version commit is made in the `versions` tree, on no branch: the working tree is not touched. It builds `HEAD` (uncommitted changes are not included). What would only block a release (branch, modified tree, release notes, tag) is reported as a "note". The files go to `dist/vX.Y.Z-repetition/`, with the cumulative feed in `appcast-site.xml` and what would change in `site/updates/appcast.xml`. Run it after any big merge: what would break a release breaks here, harmlessly, and the release that follows reuses its steps. `--check` stops after the checks and prints the plan.

**Requirements of an update.** `--min-installer A.B.C` (installed KyberNow version below which the package is refused, 0.2.0 by default) and `--requires-setup` (the update needs the installer) are passed to `package-update.sh`, which keeps a copy in the package (`requirements.json`, checked by `update.ps1`), and to `scripts/sign-manifest.sh`, whose signed manifest is authoritative. When to use them: "Settings migrations", below.

**What the author sets up once**: `gh auth login` (token and rights: “GitHub settings that protect releases”; a *fine-grained* token limited to the repository needs Contents read and write, plus Workflows if `main` pushes a change to `.github/workflows/`), the notarisation profile and the Developer ID certificate ("Signing and notarising the Mac app"), the Sparkle key (above) and the minisign key (header of `scripts/sign-manifest.sh`).

## Mac app update feed

The Mac app reads `https://www.kybernow.app/updates/appcast.xml` (`SUFeedURL` in `apps/macos/project.yml`): the repository's `site/updates/appcast.xml` file, published by the Pages workflow on every push to `main` that touches `site/`.

- **Every published release**, stable and beta, newest first. Each entry is the one from the `appcast.xml` attached to its release, copied as is by `scripts/merge-appcast.py`: download link to `releases/download/vX.Y.Z/KyberNow-Link-macOS.zip` (`KyberNow-macOS.zip` up to 0.4.2), length, EdDSA signature, notes. Nothing is re-signed: the app still verifies the archive with `SUPublicEDKey`.
- **Channels.** A beta's entries carry `<sparkle:channel>beta</sparkle:channel>`, stable ones no channel. The app (Sparkle delegate in `KyberNowApp.swift`) asks for the `beta` channel when "Receive beta versions" is ticked in KyberNow Link › Settings… (`UserDefaults`, key `receiveBetaUpdates`), for none otherwise. Sparkle offers the newest allowed entry: a stable release newer than an installed beta is offered normally, even with the box ticked; unticking the box does not go back to the previous stable release, the app waits for the next one.
- **Mac apps on 0.2.6 and earlier.** They read `releases/latest/download/appcast.xml`, which GitHub resolves to the latest stable release. Every release therefore keeps its `appcast.xml`: these apps find the latest stable release there, which then reads the website's feed.
- **Delay.** GitHub Pages caches for about 10 minutes, and the feed goes online a minute or two after the push, whereas the release is published a few seconds after it: in practice the app never sees an entry whose archive is still a draft (unless something fails between the push and the publication: re-running the same command publishes the draft).
- **Rebuilding the feed** from the existing releases (bootstrap, or damaged feed): `scripts/rebuild-appcast.sh` lists the published releases (`gh release list`, drafts excluded), takes each one's `appcast.xml`, marks prereleases as `beta`, and runs `generate_appcast` (Sparkle key from the Keychain) for a release that would have the Mac app without a feed. It writes `site/updates/appcast.xml` (or `-o file`) and checks it with `xmllint`; it commits nothing. Committing the file to `main` publishes it.

```bash
scripts/rebuild-appcast.sh
xmllint --noout site/updates/appcast.xml
git add site/updates/appcast.xml && git commit -m "Flux des mises à jour : reconstruit"
```

## Continuous integration

`.github/workflows/ci.yml` checks each push and each pull request to `main` (the repository's Actions tab):

| Job | Machine | What it checks |
|---|---|---|
| Admin, tray icon, installer | macOS | `cargo test` of Admin and the Station installer; `cargo check --target x86_64-pc-windows-gnu` of Admin, the installer (`--features ui`) and the tray icon, Windows resources included (MinGW from Homebrew) |
| Svelte interfaces | Linux | `npm ci`, `npm run build`, `npm run check` in `server/web` and `server/installer/ui` |
| Scripts and update.ps1 | Linux | `shellcheck` of all the scripts, `update.ps1` in ASCII, `update.ps1` test bench (`server/installer/tests/update-bench.ps1`, simulated services) |
| iPhone/iPad app (rules) | macOS | each `apps/ios/tests/*/run.sh` (on-screen gamepad and touch, session rules; a test not merged yet is skipped), no simulator |
| Apple TV app | macOS | `xcodegen`, then `xcodebuild` for the simulator, unsigned, with the latest release's `KyberSDK-tvOS.zip` |

`.github/workflows/outillage.yml`, at the same moments:

| Job | Machine | What it checks |
|---|---|---|
| Release tooling | macOS | `scripts/tests/release-outils.sh` (also run by the `release.sh` pre-checks) |
| Relay | Linux | `cargo test --locked --workspace` of `server/relay` and its tunnel |
| Measurement bench | Linux | `cargo build --locked` of `tools/banc`, which recompiles `apps/core/src/adaptive.rs` |

`.github/workflows/relay-image.yml` runs the relay tests again before building the Docker image: a regression does not reach the relays, which update themselves every night. Its version (manual run) goes through the script's environment, never into its text.

What it does not cover:
- **The Mac app.** It links Kyber's client SDK and VLC (`apps/macos/Vendor`, 88 MB, built from Kyber's sources by `build-kyclient.sh`), published nowhere.
- **The Apple TV app when the client patches or the Rust core have changed** since the latest release (`apps/macos/kyber-patches`, `apps/webos/kyber-patches`, the Station patches the apps also apply, `*-kyclient-*`, `*-kynput-*`, `*-kynet-*`, `*-kycontroller-*`, `*-kywebplayer-*`, `scripts/kyber-client-patches.sh`, `kyber-toolchain.sh`, `build-kyber-tvos.sh`, `apps/core`, `apps/apple`, `apps/Cargo.*`): the published SDK no longer matches the code, so the build is skipped with a warning until the next release.
- **The KyberNow Link apps other than Apple TV** (Windows, Android, webOS, Linux, iPhone and iPad): neither built nor tested in CI, they need Kyber's sources (`third_party/`); only the iPhone/iPad app's UI-free rules are (`apps/ios/tests`). The Rust core has its own workflow (`coeur.yml`).
- **Linked Windows binaries and Kyber**: the CI checks the code; it produces neither `KyberNow-Station-Setup.exe` nor the overlay (Kyber patches, `third_party/` absent), and runs nothing on Windows.
- **Nothing is signed or published**: releasing stays `scripts/release.sh`, on the author's Mac (notarisation, Sparkle and minisign keys).

## Signing and notarising the Mac app

The distributed build (Release configuration) is signed “Developer ID Application: Mathieu Requenna (P6DB682S4K)”, with the hardened runtime and a secure timestamp, without the debugging entitlement. Entitlements (`apps/macos/KyberNow/KyberNow.entitlements`): the camera (pairing QR code), and the iCloud key-value store and Keychain group shared with the Apple TV app (computers published for the Apple TV), which require a provisioning profile. Development builds (Debug) keep the development certificate, which is faster.

- `apps/macos/scripts/embed-kyber.sh` signs the Kyber and VLC libraries according to the configuration.
- The team and the certificate are written once, in `scripts/identite.sh` (`KYBER_SIGN_IDENTITY` for another certificate), read by the signing scripts of the app and the Mac Station.
- `apps/macos/scripts/sign-release.sh`, after `xcodebuild`, re-signs Sparkle's tools, from the deepest to the framework, then the app. Xcode copies Sparkle after the project's phases, with Sparkle's own signature, which notarisation refuses.
- `scripts/release.sh` sends the app to Apple (`notarytool`, profile `kybernow`), staples the ticket (`stapler`) and checks with Gatekeeper before publishing.

Set-up, once per Mac that publishes:
1. Certificate: Xcode › Settings › Accounts › team P6DB682S4K › Manage Certificates › + › Developer ID Application.
2. App-specific password on https://account.apple.com (Sign-In and Security › App-Specific Passwords), saved for `notarytool` (type it yourself):

```bash
xcrun notarytool store-credentials kybernow --apple-id <votre identifiant Apple> --team-id P6DB682S4K
```

The saved password can only be read while the Mac is unlocked: `release.sh` checks it before building and keeps the display on until the end, so that it does not lock along the way.

The first notarisation for an account can take several hours (Apple's checks); after that, a few minutes. To find a submission: `xcrun notarytool history --keychain-profile kybernow`.

## Settings migrations

Admin's settings (`C:\ProgramData\KyberNow\kyber-admin.toml`) carry the version of their format, `schema` (missing = 0: file written by Admin 0.2.5 or earlier). At each start, and before `init` or `reset-password`, Admin brings the file to the format it knows (`server/admin/src/migrate.rs`):

- one migration per version (from N to N+1), in order, each idempotent: running it again changes nothing more;
- before writing, the original is copied to `backups\kyber-admin.toml.schema-<N>-<timestamp>`; each change is written to the log (`kyber-admin.log`); the file's comments are kept;
- a file from a **newer** version of Admin (after an update rollback) is left as it is, and the settings this version does not know are kept when it rewrites the file;
- a migration that cannot be done (unreadable file) does not prevent Admin from starting: the file is then set aside (`kyber-admin.toml.invalide-…`).

By hand: `kyber-admin.exe migrate --dry-run` shows what would change, `kyber-admin.exe migrate` does it.

| Schema | Since | Change |
|---|---|---|
| 1 | 0.2.6 | `listen` becomes a valid `IP:port` address: port 9443 added to a bare address (`"0.0.0.0"`, which prevented Admin from starting), `"0.0.0.0:<port>"` for a bare port (`listen = 9443`, which made the whole file unreadable: it was set aside, password included), spaces removed; an unusable value is removed (default address). |

Adding a migration: a `to_N` function in `migrate.rs`, added to `MIGRATIONS`, `CURRENT` raised to N, a test on a file of version N-1, and a row in this table.

**Update that needs the installer.** Some releases cannot be installed by the update package: new driver (virtual display, gamepads), service to recreate, permissions to set again. They are published with `scripts/release.sh X.Y.Z --requires-setup`: the signed manifest carries `"requires_setup": true` and Admin then refuses "Install", pointing to `KyberNow-Station-Setup.exe` (`KyberNow-Server-Setup.exe` for Admin 0.3.1 and earlier, hence its copy). Likewise, `--min-installer A.B.C` refuses the package on a Station installed before version A.B.C (`min_installer_version`). The package keeps a copy (`requirements.json`), which `update.ps1`, having no manifest, checks in the same way; for Admin, the signed manifest is authoritative.

## Relay image signature

The relay's Docker image (`ghcr.io/monsieurnours/kybernow-relay`) is built and pushed by the `relay-image.yml` workflow, on GitHub, for each published release. It signs it with [cosign](https://docs.sigstore.dev/cosign/) by its digest (all its tags with it) when the repository secrets `COSIGN_PRIVATE_KEY` and `COSIGN_PASSWORD` are set; without them, the image ships unsigned and the workflow notes it, as do the `release.sh` pre-checks (a "note", never a failure). A relay whose `.env` names a public key (`RELAY_COSIGN_KEY`) only takes a new image if its signature verifies against it (`server/relay/update.sh`); otherwise it keeps the previous one.

**Creating the key, once, on the Mac** (offline; never in the repository):

```bash
mkdir -p ~/Library/Application\ Support/KyberNow && cd ~/Library/Application\ Support/KyberNow
cosign generate-key-pair          # asks for a password; writes cosign.key and cosign.pub
```

Then, on the repository, the two secrets the workflow reads (`gh` reads them from standard input, nothing goes through arguments):

```bash
gh secret set COSIGN_PRIVATE_KEY -R MonsieurNours/kybernow < ~/Library/Application\ Support/KyberNow/cosign.key
gh secret set COSIGN_PASSWORD -R MonsieurNours/kybernow    # type the key's password
```

Back up `cosign.key` and its password with the other keys ("Backing up the signing keys", below): without them, a new pair is needed, and the new public key has to be put on every relay.

**On the relay machine** (for the author to do): copy `cosign.pub` into the relay's folder (the one with `docker-compose.yml`), then add `RELAY_COSIGN_KEY=cosign.pub` to its `.env` and run `docker compose up -d` again. Verification goes through cosign's official image (`RELAY_COSIGN_IMAGE` to change it). Images published before the first signature are unsigned: set the key only after a signed release.

## Keys, GitHub account and Windows signing

### Backing up the signing keys

Three private keys must never be lost: a new key would make everything already installed refuse all updates, and it would have to be reinstalled by hand.

| Key | Where it lives | Expected public key |
|---|---|---|
| Sparkle (Mac app, EdDSA) | Login Keychain, item created by `generate_keys` | `SUPublicEDKey` in `apps/macos/project.yml` |
| minisign (each release's manifest, checked by Admin and by the apps' Rust core) | `~/Library/Application Support/KyberNow/minisign.key`, encrypted with the password of the `kybernow-minisign` Keychain item | `server/admin/src/update/kybernow-release.pub` (compiled into Admin and into `apps/core/src/update.rs`) |
| Android (published APK) | `~/Library/Application Support/KyberNow/android-release.jks`, password in the `kybernow-android-keystore` Keychain item | fingerprint in `apps/android/scripts/release-cert.sha256` (see “Android app signing key”) |

The backup of the Sparkle and minisign keys (made on 2026-10-21) is an AES-256 encrypted disk image, `kybernow-cles.dmg`, volume `KyberNow-Cles`. It contains:
- `sparkle-kybernow.key`, the Sparkle key exported by `generate_keys -x`, in plain text inside the image;
- `kybernow-minisign.key` and its password `minisign-motdepasse.txt`, taken from the Keychain without going through the screen;
- `kybernow-minisign.pub`;
- a `LISEZMOI.txt` with the restore commands.

The image password is written on paper, kept separately. The GitHub 2FA recovery codes can go into the image too. The image must live **off the Mac that publishes** (external drive, NAS, ideally two places). Open it once a year to check that it still reads.

Create or reopen the image and write to it, without displaying anything:

```bash
hdiutil create -size 20m -fs APFS -encryption AES-256 -volname KyberNow-Cles ~/kybernow-cles.dmg
hdiutil attach ~/kybernow-cles.dmg
GK=apps/macos/build/SourcePackages/artifacts/sparkle/Sparkle/bin/generate_keys
$GK -x /Volumes/KyberNow-Cles/sparkle-kybernow.key
cp ~/Library/Application\ Support/KyberNow/minisign.key /Volumes/KyberNow-Cles/kybernow-minisign.key
cp ~/Library/Application\ Support/KyberNow/minisign.pub /Volumes/KyberNow-Cles/kybernow-minisign.pub
security find-generic-password -s kybernow-minisign -w > /Volumes/KyberNow-Cles/minisign-motdepasse.txt
hdiutil detach /Volumes/KyberNow-Cles
```

Check a backup without revealing the key (image open). The first command recomputes the Sparkle public key, which must equal `SUPublicEDKey`. The second signs a test file with the backed-up key and password, then verifies the signature with the repository's public key:

```bash
{ printf '\x30\x2e\x02\x01\x00\x30\x05\x06\x03\x2b\x65\x70\x04\x22\x04\x20'; base64 -d -i /Volumes/KyberNow-Cles/sparkle-kybernow.key; } \
  | openssl pkey -inform DER -pubout -outform DER | tail -c 32 | base64
echo test > /tmp/essai.txt
tr -d '\n' < /Volumes/KyberNow-Cles/minisign-motdepasse.txt | { read -r PW; printf '%s\n' "$PW"; } \
  | minisign -S -s /Volumes/KyberNow-Cles/kybernow-minisign.key -m /tmp/essai.txt -x /tmp/essai.txt.minisig >/dev/null 2>&1 \
  && minisign -V -q -p server/admin/src/update/kybernow-release.pub -m /tmp/essai.txt -x /tmp/essai.txt.minisig && echo minisign OK
rm -f /tmp/essai.txt /tmp/essai.txt.minisig
```

Restore the keys on another Mac (image open):

```bash
$GK -f /Volumes/KyberNow-Cles/sparkle-kybernow.key && $GK -p      # must print SUPublicEDKey
mkdir -p ~/Library/Application\ Support/KyberNow
cp /Volumes/KyberNow-Cles/kybernow-minisign.key ~/Library/Application\ Support/KyberNow/minisign.key
cp /Volumes/KyberNow-Cles/kybernow-minisign.pub ~/Library/Application\ Support/KyberNow/minisign.pub
chmod 600 ~/Library/Application\ Support/KyberNow/minisign.key
printf 'add-generic-password -a kybernow -s kybernow-minisign -l kybernow-minisign -w %s\n' \
  "$(cat /Volumes/KyberNow-Cles/minisign-motdepasse.txt)" | security -i >/dev/null
```

`printf` is a shell builtin: the password does not show up in the process list.

### GitHub settings that protect releases

State on 2026-10-21, checked with `gh api`:
- **2FA** enabled on the `MonsieurNours` account, the repository's only collaborator.
- **Immutable releases** (Settings › General › Releases). A published release can no longer change its files or its tag. `release.sh` creates a draft, uploads everything, then publishes, which stays compatible. A mistake in a published release is fixed with a new release: the number of a deleted release cannot be reused.
- **Rules (Settings › Rules)**:
  - `tags-v-creation`: only the admin role can create a `v*` tag. The Actions `GITHUB_TOKEN` cannot.
  - `tags-v-figes`: no `v*` tag can be moved or deleted, with no exception. If a release fails after the tag was pushed, publish the next version, or disable the rule while cleaning up.
  - `main-legere`: `main` cannot be deleted or rewritten (force push). Direct pushes are still allowed.
- **`gh` on the publishing Mac**: `gh`'s OAuth token (broad rights, development Mac). `release.sh` uses it for releases; `git push` goes over SSH. A fine-grained token limited to the repository would need at least *Contents: Read and write*: GitHub has no narrower permission for releases, so such a token could also push. The `admin:public_key` scope (adding an SSH key to the account) is not needed for releasing: `gh auth refresh -h github.com --remove-scopes admin:public_key` removes it.
- **CI**: workflows use the `GITHUB_TOKEN` provided for each run, reduced to `contents: read` (plus `pages: write` and `id-token: write` for the website workflow, which deploying to Pages requires). The repository default is read-only as well. No secret to create.

Read the rules back:

```bash
gh api repos/MonsieurNours/kybernow/rulesets --jq '.[].name'
gh api repos/MonsieurNours/kybernow/immutable-releases
```

### Windows signing (Authenticode)

`KyberNow-Station-Setup.exe`, `KyberNow-Link-Setup.exe`, `kyber-admin.exe`, `kybernow-tray.exe` and `update.ps1` are not Authenticode-signed: Windows shows “Unknown publisher” (SmartScreen, UAC). Authenticode would not replace minisign: it says who signed; minisign proves it is KyberNow's key, built into Admin, which also protects against a compromised GitHub account.

The virtual display driver (SudoVDA) is self-signed: the Station installer adds its certificate, `sudovda.cer` (“Code Signing” usage), to the Root and TrustedPublisher stores (`server/installer/src/install.rs`), and the uninstaller removes it.

**Pitfall, for any Authenticode signing**: it changes the checksums of the signed files. `SHA256SUMS` and the minisign manifest are computed after it, and the copies under the old names are made from the signed files.
