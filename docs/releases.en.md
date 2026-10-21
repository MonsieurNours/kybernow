# Releases and updates

English · [Français](releases.md)

KyberNow releases are published on GitHub: https://github.com/MonsieurNours/kybernow/releases. The repository is public: updates download without a token, and the code stays available as the AGPL requires (with the patches to Kyber, see [kyber-changes.en.md](kyber-changes.en.md)).

One version number for all of KyberNow: the console, the icon, the installer, the Mac app and the Apple TV app change together.

## What a release contains

| File | What for |
|---|---|
| `KyberNow-Setup.exe` | First installation on a Windows computer |
| `kybernow-update.zip` | Update of a computer that is already installed: Kyber overlay, console, icon, `update.ps1` |
| `update.ps1` | Manual update, without the console |
| `KyberNow-macOS.zip` | The Mac app (first installation: drag it into Applications) |
| `appcast.xml` | Update feed for the Mac app (Sparkle), signed with EdDSA |
| `KyberSDK-tvOS.zip` | Kyber SDK built for Apple TV, downloaded by `apps/tvos/scripts/sideload.sh` to install the app without the App Store ([SIDELOAD.en.md](../apps/tvos/SIDELOAD.en.md)) |
| `kybernow-release.json` | Release manifest: number, channel (`stable` or `beta`), date, minimum installed version for an update from the console, whether a reinstallation is needed, SHA-256 checksum and size of each file above |
| `kybernow-release.json.minisig` | Manifest signature (minisign, ed25519), verified by the console before any installation |
| `SHA256SUMS` | Checksums of all the files |

## Updating

**The computer, in one click.**
- The console checks for the latest release every 6 hours, on its **channel**: `stable` (default, published releases) or `beta` (GitHub prereleases too), chosen on the Updates page (`update_channel` setting in `kyber-admin.toml`). The **Updates** page shows it with its notes and an **Install** button; the notification area icon offers “Install version X…”, which opens that page.
- **Nothing installs without a signature.** For a newer release, the console downloads `kybernow-release.json` and `kybernow-release.json.minisig` and verifies the signature with the public key compiled into it (see “Update signing key”). Then: the manifest must announce the same number as the GitHub release (`tag_name`), a channel the computer follows (a `beta` release is refused on `stable`), a `min_installer_version` lower than or equal to the installed version and no `requires_setup` (otherwise “reinstall with KyberNow-Setup.exe”). Any failure (missing manifest, invalid signature or another key's, channel, version) is shown on the page and in `GET /api/update` (`verify_error`), and the **Install** button does not appear; the icon only offers a release whose signature is good.
- The installation downloads and verifies the manifest again, then downloads `kybernow-update.zip` (never more than the manifest's size) and checks its size and SHA-256 checksum against it. The published release number must look like `X.Y.Z`: it is used as a folder name, so any other number is refused. Manifest, signature and package stay in `C:\ProgramData\KyberNow\update\<version>\` (reserved to SYSTEM and Administrators).
- The console then launches **the applicator**, `kyber-admin update apply <package> --version X`, as SYSTEM and detached (it survives the console stopping). It is a copy of the console, `C:\Program Files\KyberNow\.staging\kyber-admin-maj.exe`: it must be able to rename the `admin` folder while it runs. Nothing is run from a folder another account could change, and no script is launched: PowerShell is no longer involved.
- The applicator verifies the signed manifest and the package again (nothing comes from the console that launched it), then extracts the package into `C:\Program Files\KyberNow\.staging\<version>\paquet` (permissions inherited from Program Files: read-only for users). It refuses any path that would leave the folder (`..`, absolute path, drive, NTFS stream) and any link. The package must contain `kyber`, `admin`, `tray` and a `version.txt` equal to the expected version. A failure at this stage touches nothing: the console is still running and shows it.
- It stops the icon, then the server (`kyber` and its processes), then the console (`kyber-admin` and its agents). It prepares each new folder in `.staging\<version>\` (a copy of the current folder, Kyber's settings and certificate included, with the package's files on top), then switches by renaming: `kyber` becomes `kyber.prev` and `.staging\<version>\kyber` becomes `kyber`, and the same for `admin`, `tray` and `webclient` if the package has one. Kyber's logs (`kyber\log`) are moved, not copied. It then restores the services' permissions (`sc sdset`) and the Bonjour firewall rule.
- It restarts the services, then checks the new version's health: within 60 s, the console must answer on its port with `GET /api/session` giving the new version, and the server must be running. Otherwise it stops everything, puts each folder back where it was (`kyber.prev` becomes `kyber` again…), restarts the old version and writes the reason in the log and in `C:\ProgramData\KyberNow\update\resultat.json`. The console reads that file when it starts: the Updates page shows the failure, and `GET /api/update` returns it in `install_error` and `last_failure` (version, reason, rollback, log).
- Whatever happens once the services are stopped (error, a rename that fails because a file is open), the applicator restarts them on its way out: the computer is never left without its server and console.
- When it succeeds, the update writes the new version into `C:\Program Files\KyberNow\kybernow.json` and into “Installed apps” (`DisplayVersion`), and `resultat.json` says `"ok": true`. The previous version stays in the `.prev` folders until the next update (about 300 MB). In `update\`, only the folders of the two latest versions are kept; `.staging\<version>` is deleted.
- The console relaunches the icon in the user's session, with their account (`update\relaunch-tray` file). The page updates itself.
- **Going back to the previous version**: as long as the `.prev` folders exist, the Updates page offers this button (`POST /api/update/rollback`, previous version in `rollback` of `GET /api/update`). The same applicator switches in the other direction (`kyber-admin update rollback`): `kyber.prev` becomes `kyber` again and the removed version goes to `kyber.bad`, and the same for `admin`, `tray` and `webclient`; Kyber's settings changed since the update (`kyber_config.toml`) and its logs are kept. Same health check, with the previous version; if it fails, the version that was running is put back. When it succeeds, the rollback deletes the `.bad` folders: no `.prev` is left, and the removed version is offered for installation again.
- **During a session.** If a device is in session, `POST /api/update/install` (and `POST /api/update/rollback`) answers 409 with “Un appareil est connecté (…) : l'installation coupera sa session” (a device is connected: installing will cut its session); the page asks before going on, and then sends `{"force": true}`.
- Duration: about one minute. A streaming session in progress is cut. Log: `C:\ProgramData\KyberNow\update\<version>\update.log`; the applicator's output goes to `lancement.log`, to read when `update.log` does not exist.
- The console follows the applicator. If it stops while the console is still running (verification or extraction refused), the console says so: Updates page, end of the log, and “Install” becomes possible again.
- Steam's virtual speakers and the Start menu shortcut, set up by the installer, are no longer redone at each update; `update.ps1` (by hand) still does it.
- **Moving to 0.2.6.** A 0.2.5 or older console knows neither the manifest nor the applicator: it updates to 0.2.6 as before, with `SHA256SUMS` and the package's `update.ps1`. Releases therefore keep `SHA256SUMS` and `update.ps1`. From 0.2.6 on, the console only installs a signed release.

**The computer, by hand** (administrator PowerShell):

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

The script downloads `kybernow-update.zip` and `SHA256SUMS`, checks the package's checksum, then performs the same switch with a health check. If it fails, it says so in red (“Mise a jour non installee…”) after restarting everything.

**Manual path: integrity only, until Authenticode signing.** `update.ps1` checks that the package is not damaged (`SHA256SUMS`), not that it comes from KyberNow: `SHA256SUMS` is published in the same place as the package, and verifying the minisign manifest would need a tool that is downloaded too (the release's `kyber-admin.exe`), which goes round in circles. The script itself is not signed. It therefore does not give the console's guarantees: keep it for troubleshooting, on a release whose GitHub page you know. Authenticode signing of the `.exe` files and of `update.ps1` is being considered to close that gap.

To try a package served from the Mac before publishing it: `$env:KYBERNOW_UPDATE_BASE = 'http://<adresse-du-Mac>:8000'`, then the same command, with `python3 -m http.server 8000` running in `server/installer/dist` (`package-update.sh` also writes `SHA256SUMS` there).

**Testing the verification and the applicator without Windows**: `cd server/admin && cargo test`. The tests cover the manifest (reading the published format, version comparison, channel, `min_installer_version`, `requires_setup`), the signature (test key generated in the test: valid signature, manifest changed by one byte, another key, trusted comment that does not match), a manifest actually produced by `scripts/sign-manifest.sh` and the `minisign` binary (`server/admin/src/update/testdata/`, throwaway test key of which only the public key is kept), extraction (`..`, absolute path, drive, links), the switch on a fake installation folder (success, failed health check then rollback, forged package that touches nothing, going back to the previous version and its failure) and the refusal during a session. Windows' services, processes and permissions go through a simulated trait: they are only checked on the PC. To regenerate `testdata/`: `KYBERNOW_MINISIGN_KEY=<temporary folder>/essai.key KYBERNOW_MINISIGN_ITEM=kybernow-minisign-essai scripts/sign-manifest.sh <folder> 0.2.7 beta --min-installer 0.2.6` on a 15-byte `kybernow-update.zip`, then delete the private key and the Keychain item.

**Testing `update.ps1` without Windows**: `pwsh -NoProfile -File server/installer/tests/update-bench.ps1` (`brew install powershell`). The bench simulates the services and the console, and replays a successful update, a silent console (rollback), an incomplete package and a blocked rename.

**The Mac app.**
- Sparkle checks every day, and on request (KyberNow menu › Check for Updates…). The app downloads the archive, verifies its EdDSA signature with the app's public key (`SUPublicEDKey`), replaces itself and relaunches.
- The private key is in this Mac's Keychain (item created by Sparkle's `generate_keys`). Back it up, and never put it in the repository:

```bash
apps/macos/build/SourcePackages/artifacts/sparkle/Sparkle/bin/generate_keys -x ~/sparkle-kybernow.key
```

Without it, no app update could be signed any more. To restore it on another Mac: `generate_keys -f ~/sparkle-kybernow.key`.

## Update signing key (minisign)

Each release publishes a manifest, `kybernow-release.json`, signed with an ed25519 key (minisign):

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

`files` lists those of the published files that are present (`KyberNow-Setup.exe`, `kybernow-update.zip`, `update.ps1`, `KyberNow-macOS.zip`, `appcast.xml`, `KyberSDK-tvOS.zip`). The detached signature, `kybernow-release.json.minisig`, carries the trusted comment `kybernow <version> <channel>`. The public key is in the repository, `server/admin/src/update/kybernow-release.pub` (key ID `D63B24FFFC7E79D1`), and compiled into the console: a SHA-256 checksum alone protects against a damaged download, not against a release forged by whoever controls the GitHub account; the signature does.

The manifest is written and signed with:

```bash
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable            # or beta
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable --min-installer 0.2.6 --requires-setup
```

`--min-installer X.Y.Z`: oldest version from which the console can install this package (0.2.0 by default). `--requires-setup`: this release only installs with `KyberNow-Setup.exe` (new driver, new service…). The script installs `minisign` with Homebrew if it is missing, then checks the signature against the repository's public key: a private key that does not match it is caught before publishing.

**The private key** is in `~/Library/Application Support/KyberNow/minisign.key`, encrypted with a random password kept in the Keychain (item `kybernow-minisign`). The script creates it if it is missing and writes the public key into the repository; if a public key is already in the repository, it refuses to create another one: a new key would make every console already installed refuse all updates, and they would have to be reinstalled with `KyberNow-Setup.exe`.

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

## Publishing a release

1. Add a `## X.Y.Z` section at the top of `CHANGELOG.md`: these are the notes shown by the console and by the app. Add the same section, in English, to `CHANGELOG.en.md`.
2. If the Kyber patches have changed: `server/kyber-patches/build.sh`.
3. Quit the KyberNow built in Release if it is open (another copy does not matter), then run:

```bash
scripts/release.sh X.Y.Z
```

The script aligns the versions, builds the installer, the update package and the app (Release), signs the Sparkle feed and computes `SHA256SUMS`. It commits, tags `vX.Y.Z`, pushes, then creates the GitHub release. The files stay in `dist/vX.Y.Z/`.

Until `release.sh` calls `scripts/sign-manifest.sh` itself, sign the manifest after it and attach it to the release, otherwise the console refuses to install it:

```bash
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable
gh release upload vX.Y.Z -R MonsieurNours/kybernow dist/vX.Y.Z/kybernow-release.json dist/vX.Y.Z/kybernow-release.json.minisig
```

## Signing and notarising the Mac app

The distributed build (Release configuration) is signed “Developer ID Application: Mathieu Requenna (P6DB682S4K)”, with the hardened runtime and a secure timestamp, without the debugging entitlement. The only entitlement requested: the camera (`KyberNow/KyberNow.entitlements`, for the QR code). Development builds (Debug) keep the development certificate, which is faster.

- `scripts/embed-kyber.sh` signs the Kyber and VLC libraries according to the configuration.
- `scripts/sign-release.sh`, after `xcodebuild`, re-signs Sparkle's tools, from the deepest to the framework, then the app. Xcode copies Sparkle after the project's phases, with Sparkle's own signature, which notarisation refuses.
- `scripts/release.sh` sends the app to Apple (`notarytool`, profile `kybernow`), staples the ticket (`stapler`) and checks with Gatekeeper before publishing.

Set-up, once per Mac that publishes:
1. Certificate: Xcode › Settings › Accounts › team P6DB682S4K › Manage Certificates › + › Developer ID Application.
2. App-specific password on https://account.apple.com (Sign-In and Security › App-Specific Passwords), saved for `notarytool` (type it yourself):

```bash
xcrun notarytool store-credentials kybernow --apple-id <votre identifiant Apple> --team-id P6DB682S4K
```

The saved password can only be read while the Mac is unlocked: `release.sh` checks it before building and keeps the display on until the end, so that it does not lock along the way.

The first notarisation for an account can take several hours (Apple's checks); after that, a few minutes. To find a submission: `xcrun notarytool history --keychain-profile kybernow`.
