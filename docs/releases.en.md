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
| `SHA256SUMS` | Checksums of all the files |

## Updating

**The computer, in one click.**
- The console checks for the latest release every 6 hours. The **Updates** page shows it with its notes and an **Install** button; the notification area icon offers “Install version X…”, which opens that page.
- The installation downloads `kybernow-update.zip` and checks its checksum in `SHA256SUMS`. The published release number (`tag_name`) must look like `X.Y.Z`: it is used as a folder name and goes into PowerShell, so any other number is refused. The console then launches PowerShell, detached: it unzips the package and runs its `update.ps1`.
- The script stops the server (`kyber`), then the console (`kyber-admin`), then the icon. It prepares the new folders next to the old ones (`kyber.new`, `admin.new`, `tray.new`: a copy of the current folder, Kyber's settings and certificate included, with the package's files on top), then switches by renaming: `kyber` becomes `kyber.prev` and `kyber.new` becomes `kyber`, and the same for `admin` and `tray`. Kyber's logs (`kyber\log`) are moved, not copied. It then restores the services' permissions, Steam's speakers and the firewall.
- It restarts the services, then checks the new version's health: the console must accept a connection on its port within 60 s, and the server must be running. Otherwise it stops everything, puts the previous folders back (`kyber.prev` becomes `kyber` again…), restarts the old version and writes the reason in the log and in `C:\ProgramData\KyberNow\update\resultat.json`. The console reads that file when it starts: the Updates page shows the failure, and `GET /api/update` returns it in `install_error` and `last_failure` (version, reason, rollback, log).
- Whatever happens once the services are stopped (error, interruption, a rename that fails because a file is open), the script restarts them and relaunches the icon on its way out (`try … finally`): the computer is never left without its server and console.
- When it succeeds, the update records the new version in `C:\Program Files\KyberNow\kybernow.json` and in “Installed apps” (`DisplayVersion`). The previous version stays in the `.prev` folders until the next update (about 300 MB). In `update\`, only the folders of the two latest versions are kept.
- The console relaunches the icon in the user's session, with their account (file `update\relaunch-tray`). The page updates by itself.
- Duration: about one minute. A streaming session in progress is cut. Log: `C:\ProgramData\KyberNow\update\<version>\update.log`.
- The console watches the installation's PowerShell. If it stops while the console is still running (a step failed), the console says so: Updates page, end of the log, and "Install" can be used again. Before 0.2.2, the console stayed on "an installation is already in progress" until it restarted.
- PowerShell runs `update\<version>\lancement.ps1` (written by the console) with a hidden console; its output goes to `lancement.log`, to read when `update.log` does not exist. The 0.2.0 console launched it with no console and no output: it could stop before writing its log (only the zip was left in the folder). In that case, update by hand with `update.ps1`.

**The computer, by hand** (administrator PowerShell):

```powershell
irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
```

The script downloads `kybernow-update.zip` and `SHA256SUMS`, checks the package's checksum like the console does, then performs the same switch with a health check. If it fails, it says so in red (“Mise a jour non installee…”) after restarting everything.

To try a package served from the Mac before publishing it: `$env:KYBERNOW_UPDATE_BASE = 'http://<adresse-du-Mac>:8000'`, then the same command, with `python3 -m http.server 8000` running in `server/installer/dist` (`package-update.sh` also writes `SHA256SUMS` there).

**Testing `update.ps1` without Windows**: `pwsh -NoProfile -File server/installer/tests/update-bench.ps1` (`brew install powershell`). The bench simulates the services and the console, and replays a successful update, a silent console (rollback), an incomplete package and a blocked rename.

**The Mac app.**
- Sparkle checks every day, and on request (KyberNow menu › Check for Updates…). The app downloads the archive, verifies its EdDSA signature with the app's public key (`SUPublicEDKey`), replaces itself and relaunches.
- The private key is in this Mac's Keychain (item created by Sparkle's `generate_keys`). Back it up, and never put it in the repository:

```bash
apps/macos/build/SourcePackages/artifacts/sparkle/Sparkle/bin/generate_keys -x ~/sparkle-kybernow.key
```

Without it, no app update could be signed any more. To restore it on another Mac: `generate_keys -f ~/sparkle-kybernow.key`.

## Publishing a release

1. Add a `## X.Y.Z` section at the top of `CHANGELOG.md`: these are the notes shown by the console and by the app. Add the same section, in English, to `CHANGELOG.en.md`. Commit: the script refuses a modified tree.
2. If the Kyber patches have changed: `server/kyber-patches/build.sh` (it writes `third_party/payload/kyber-overlay.stamp`, the fingerprint of the compiled patches, which the script compares with the repository's).
3. Quit the KyberNow built in Release if it is open (another copy does not matter), then, from `main`:

```bash
scripts/release.sh X.Y.Z --check     # checks only
scripts/release.sh X.Y.Z --dry-run   # full rehearsal, nothing is published
scripts/release.sh X.Y.Z             # release
```

**Order.** The published binaries must be those of the tagged commit (the AGPL requires publishing the exact source of the binaries):
1. preliminary checks, all shown at once; if one fails, nothing is modified;
2. version alignment (console, tray icon, installer, Mac and Apple TV apps, including `Cargo.lock` and the Xcode projects) and commit "KyberNow X.Y.Z"; a refused commit stops everything;
3. build from that commit: installer, update package, Developer ID-signed Mac app, Kyber SDK for the Apple TV and Apple TV app; if the build modifies a tracked file, the script stops (the binaries would no longer match the commit);
4. notarisation of the Mac app, stapling of the ticket, Gatekeeper check;
5. signed Sparkle feed, `SHA256SUMS`, signed manifest `kybernow-release.json` (`scripts/sign-manifest.sh`, minisign);
6. GitHub release created **as a draft** with all the files;
7. tag `vX.Y.Z` on the version commit; 8. push of `main` and the tag, at once (`--atomic`);
9. publication of the draft.

The files stay in `dist/vX.Y.Z/`. The version Windows shows for `KyberNow-Setup.exe`, `kybernow-ui.exe` and `kybernow-tray.exe` (Properties › Details, manifests) follows the one in `Cargo.toml`: their `build.rs` produce the resources from the `resources/*.rc.in` and `*.manifest.in` templates.

**Re-running after a failure**: the same command. The "KyberNow X.Y.Z" commit already at the head is reused (no second commit), an existing draft receives the new files (`--clobber`), the tag is created if missing. Only an already published release is refused.

**Checked before publishing**:

| Check | If it fails |
|---|---|
| Branch `main`, clean tree (`git status --porcelain` empty, ignored files excepted), `origin/main` included | commit, `git switch main`, `git pull` |
| `## X.Y.Z` section in `CHANGELOG.md` and `CHANGELOG.en.md` | add it to both |
| Kyber overlay compiled with the repository's patches: fingerprint of `server/kyber-patches/*.patch` and `build.sh` equal to `kyber-overlay.stamp`; without a fingerprint, overlay newer than all these files | `server/kyber-patches/build.sh` |
| Kyber SDK of the Mac app (`apps/macos/Vendor`) present | `apps/macos/scripts/build-kyclient.sh` |
| No `vX.Y.Z` tag other than on the version commit, no `vX.Y.Z` release already published | `git tag -d vX.Y.Z` if left over from a rehearsal |
| `gh` logged in, `notarytool` profile "kybernow" readable (Mac unlocked) | `gh auth login`, see below |
| `scripts/sign-manifest.sh` present | without a signed manifest, the console refuses the update |
| Tools: xcodegen, xcodebuild, npm, cargo, zstd, MinGW, zip | Homebrew |

**Channels.** A stable release is GitHub's "latest release" (`releases/latest`), the one the console and the Mac app see. `--beta` publishes a *prerelease*, never marked "latest": its manifest has `"channel": "beta"` and its Sparkle feed `<sparkle:channel>beta</sparkle:channel>`. The number stays `X.Y.Z` (the console refuses any other format): a beta takes the next number, and the stable release that follows it a higher one. Current limit of the Mac app: it reads `releases/latest/download/appcast.xml`, which GitHub resolves to the latest **stable** release; to offer it betas, a feed combining both channels (on the website, for example) and `allowedChannels` in the app will be needed.

**Rehearsal (`--dry-run`).** Does everything except the commit, notarisation, release, tag and push, which it prints (`[essai, non lancé] …`). It aligns the versions to build like the release, then puts the version files back as it found them. It does not rebuild the Apple TV SDK (`build-kyber-tvos.sh` applies the patches in `third_party/`, shared with the other checkouts) and reuses `apps/tvos/Vendor`. Failed checks are listed at the beginning and at the end; the rehearsal goes on to show the rest and exits with an error. `--check` stops after the checks and prints the plan.

**Requirements of an update.** `--min-installer A.B.C` (installed KyberNow version below which the package is refused) and `--requires-setup` (the update needs the installer) are passed to `package-update.sh` and to the signed manifest.

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
