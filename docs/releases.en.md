# Releases and updates

English · [Français](releases.md)

KyberNow releases are published on GitHub: https://github.com/MonsieurNours/kybernow/releases. The repository is public: updates download without a token, and the code stays available as the AGPL requires (with the patches to Kyber, see [kyber-changes.en.md](kyber-changes.en.md)).

One version number for all of KyberNow: the console, the icon, the installer, the Mac app and the Apple TV app change together.

## What a release contains

| File | What for |
|---|---|
| `KyberNow-Server-Setup.exe` | KyberNow Server: first installation on the Windows computer to share (the name tells it apart from the app installer, `KyberNow-Client-Setup.exe`). A copy under the old name, `KyberNow-Setup.exe`, is still published for a few releases for existing links; the site should only go live with the new links once the release is published |
| `kybernow-update.zip` | Update of a computer that is already installed: Kyber overlay, console, icon, `update.ps1` |
| `update.ps1` | Manual update, without the console |
| `KyberNow-macOS.zip` | The Mac app (first installation: drag it into Applications) |
| `appcast.xml` | Update feed for the Mac app (Sparkle), signed with EdDSA |
| `KyberNow-Client-Setup.exe` | The Windows app, x64 and ARM64 in a single file: per-user installer (first installation and app updates, see [apps/windows/README.en.md](../apps/windows/README.en.md)) |
| `KyberSDK-tvOS.zip` | Kyber SDK built for Apple TV, downloaded by `apps/tvos/scripts/sideload.sh` to install the app without the App Store ([SIDELOAD.en.md](../apps/tvos/SIDELOAD.en.md)) |
| `kybernow-release.json` | Release manifest: number, channel (`stable` or `beta`), date, minimum installed version for an update from the console, whether a reinstallation is needed, SHA-256 checksum and size of each file above |
| `kybernow-release.json.minisig` | Manifest signature (minisign, ed25519), verified by the console before any installation |
| `SHA256SUMS` | Checksums of all the files |

## Updating

**The computer, in one click.**
- The console checks for the latest release every 6 hours, on its **channel**: `stable` (default, published releases) or `beta` (GitHub prereleases too), chosen on the Updates page (`update_channel` setting in `kyber-admin.toml`). The **Updates** page shows it with its notes and an **Install** button; the notification area icon offers “Install version X…”, which opens that page.
- **Nothing installs without a signature.** For a newer release, the console downloads `kybernow-release.json` and `kybernow-release.json.minisig` and verifies the signature with the public key compiled into it (see “Update signing key”). Then: the manifest must announce the same number as the GitHub release (`tag_name`), a channel the computer follows (a `beta` release is refused on `stable`), a `min_installer_version` lower than or equal to the installed version and no `requires_setup` (otherwise “reinstall with KyberNow-Server-Setup.exe”). Any failure (missing manifest, invalid signature or another key's, channel, version) is shown on the page and in `GET /api/update` (`verify_error`), and the **Install** button does not appear; the icon only offers a release whose signature is good.
- The installation downloads and verifies the manifest again, then downloads `kybernow-update.zip` (never more than the manifest's size) and checks its size and SHA-256 checksum against it. The published release number must look like `X.Y.Z`: it is used as a folder name, so any other number is refused. Manifest, signature and package stay in `C:\ProgramData\KyberNow\update\<version>\` (reserved to SYSTEM and Administrators).
- The console then launches **the applicator**, `kyber-admin update apply <package> --version X`, as SYSTEM and detached (it survives the console stopping). It is a copy of the console, `C:\Program Files\KyberNow\.staging\kyber-admin-maj.exe`: it must be able to rename the `admin` folder while it runs. Nothing is run from a folder another account could change, and no script is launched: PowerShell is no longer involved.
- The applicator verifies the signed manifest and the package again (nothing comes from the console that launched it), then extracts the package into `C:\Program Files\KyberNow\.staging\<version>\paquet` (permissions inherited from Program Files: read-only for users). It refuses any path that would leave the folder (`..`, absolute path, drive, NTFS stream), any name that would not be an ordinary file on Windows (`CON`, `NUL`, `COM1`…, a name ending with a dot or a space, `<>"|?*`) and any link. It also refuses a release that is not newer than the installed one (going back has its own path). The package must contain `kyber`, `admin`, `tray` and a `version.txt` equal to the expected version. A failure at this stage touches nothing: the console is still running and shows it.
- It stops the icon, then the server (`kyber` and its processes), then the console (`kyber-admin` and its agents), then the server again if the console had restarted it in the meantime. The automatic restart of `kyber` after a crash (set by the installer) is suspended until the restart, so that Windows does not restart it in the middle of the renames. It prepares each new folder in `.staging\<version>\` (a copy of the current folder, Kyber's settings and certificate included, with the package's files on top), then switches by renaming: `kyber` becomes `kyber.prev` and `.staging\<version>\kyber` becomes `kyber`, and the same for `admin`, `tray` and `webclient` (if it is in the package or already installed: every folder present thus gets its `.prev`). Kyber's logs (`kyber\log`) are moved, not copied. It then restores the services' permissions (`sc sdset`) and the Bonjour firewall rule.
- It restarts the services, then checks the new version's health: within 60 s, the console must answer at the address of its `listen` setting (127.0.0.1 if it listens everywhere, no proxy) with `GET /api/session` giving the new version, and the server must be running. Otherwise it stops everything (the icon included, which the new console may have relaunched), puts each folder back where it was (`kyber.prev` becomes `kyber` again…), restarts the old version and writes the reason in the log and in `C:\ProgramData\KyberNow\update\resultat.json`. The console reads that file when it starts: the Updates page shows the failure, and `GET /api/update` returns it in `install_error` and `last_failure` (version, reason, rollback, log). If a folder resists the rollback, the applicator puts the others back, keeps `.staging\<version>`, writes it down (`undo_failed`) and the page says so, even when the failed version is the one running.
- Whatever happens once the services are stopped (error, a rename that fails because a file is open), the applicator restarts them on its way out: the computer is never left without its server and console.
- When it succeeds, the update writes the new version into `C:\Program Files\KyberNow\kybernow.json` and into “Installed apps” (`DisplayVersion`), and `resultat.json` says `"ok": true`. The previous version stays in the `.prev` folders until the next update (about 300 MB). In `update\`, only the folders of the two latest versions are kept; `.staging\<version>` is deleted.
- The console relaunches the icon in the user's session, with their account (`update\relaunch-tray` file). The page updates itself.
- **Going back to the previous version**: as long as the `.prev` folders exist, the Updates page offers this button (`POST /api/update/rollback`, previous version in `rollback` of `GET /api/update`). The same applicator switches in the other direction (`kyber-admin update rollback`): `kyber.prev` becomes `kyber` again and the removed version goes to `kyber.bad`, and the same for `admin`, `tray` and `webclient`; a folder brought by the removed version, with no `.prev` (`webclient`), goes too; Kyber's settings changed since the update (`kyber_config.toml`) and its logs are kept. Same health check, with the previous version; if it fails, the version that was running is put back. When it succeeds, the rollback deletes the `.bad` folders: no `.prev` is left, and the removed version is offered for installation again.
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

**The Windows app.** It checks at launch then every 6 hours for the latest version of its channel (app settings: stable, or betas too), verifies the signed manifest like the console and its architecture's installer against it (size, SHA-256), then runs it silently (`/S /UPDATE`): it waits for the app to close, replaces its files and relaunches it. A version without a Windows installer is not offered.

**The Mac app.**
- Sparkle checks every day, and on request (KyberNow menu › Check for Updates…). The app downloads the archive, verifies its EdDSA signature with the app's public key (`SUPublicEDKey`), replaces itself and relaunches.
- The app reads the website's cumulative feed, `https://www.kybernow.app/updates/appcast.xml`, which lists every release. It only offers stable releases, unless "Receive beta versions" is ticked in KyberNow › Settings…: see "Mac app update feed".
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

`files` lists those of the published files that are present (`KyberNow-Server-Setup.exe`, `kybernow-update.zip`, `update.ps1`, `KyberNow-macOS.zip`, `appcast.xml`, `KyberSDK-tvOS.zip`). The detached signature, `kybernow-release.json.minisig`, carries the trusted comment `kybernow <version> <channel>`. The public key is in the repository, `server/admin/src/update/kybernow-release.pub` (key ID `D63B24FFFC7E79D1`), and compiled into the console: a SHA-256 checksum alone protects against a damaged download, not against a release forged by whoever controls the GitHub account; the signature does.

The manifest is written and signed with:

```bash
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable            # or beta
scripts/sign-manifest.sh dist/vX.Y.Z X.Y.Z stable --min-installer 0.2.6 --requires-setup
```

`--min-installer X.Y.Z`: oldest version from which the console can install this package (0.2.0 by default). `--requires-setup`: this release only installs with `KyberNow-Server-Setup.exe` (new driver, new service…). The script installs `minisign` with Homebrew if it is missing, then checks the signature against the repository's public key: a private key that does not match it is caught before publishing.

**The private key** is in `~/Library/Application Support/KyberNow/minisign.key`, encrypted with a random password kept in the Keychain (item `kybernow-minisign`). The script creates it if it is missing and writes the public key into the repository; if a public key is already in the repository, it refuses to create another one: a new key would make every console already installed refuse all updates, and they would have to be reinstalled with `KyberNow-Server-Setup.exe`.

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

0. Check that the CI is green on `main` (the repository's Actions tab, see "Continuous integration"): it does not cover the Mac app, which `--dry-run` builds.
1. Add a `## X.Y.Z` section at the top of `CHANGELOG.md`: these are the notes shown by the console and by the app. Add the same section, in English, to `CHANGELOG.en.md`. Commit: the script refuses a modified tree. Both languages ship with the release ([docs/i18n.en.md](i18n.en.md)): the GitHub release carries the French section, a `<!-- en -->` line, then the English one (the console and the Windows app take the part in their language); in the Sparkle feed, English is the default description and French a per-language link (`xml:lang="fr"`) to `site/updates/notes/X.Y.Z/KyberNow-macOS.fr.html` (`scripts/notes-html.py`), committed with the site feed.
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
5. signed Sparkle feed, `SHA256SUMS`, signed manifest `kybernow-release.json` (`scripts/sign-manifest.sh`, minisign); the website's cumulative feed (`site/updates/appcast.xml`) gets this release and is committed on top of the version commit ("Flux des mises à jour : KyberNow X.Y.Z");
6. GitHub release created **as a draft** with all the files;
7. tag `vX.Y.Z` on the version commit; 8. push of `main` (version commit and feed commit) and the tag, at once (`--atomic`): the Pages workflow publishes the feed;
9. publication of the draft.

The files stay in `dist/vX.Y.Z/`. The version Windows shows for `KyberNow-Server-Setup.exe`, `kybernow-ui.exe` and `kybernow-tray.exe` (Properties › Details, manifests) follows the one in `Cargo.toml`: their `build.rs` produce the resources from the `resources/*.rc.in` and `*.manifest.in` templates.

**Re-running after a failure**: the same command. The "KyberNow X.Y.Z" commit already at the head is reused (no second commit), an existing draft receives the new files (`--clobber`), the tag is created if missing. Only an already published release is refused. If the failure came after step 5, the feed commit is at the head with the version commit right below it: it is recognised, the app is rebuilt and the feed redone (the archive changed, and so did its signature); its commit is replaced if it has not been pushed yet, followed by a new one otherwise.

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
| Cumulative feed `site/updates/appcast.xml` present | `scripts/rebuild-appcast.sh`, once, then commit |
| Tools: xcodegen, xcodebuild, npm, cargo, zstd, MinGW, zip, python3, xmllint | Homebrew (python3 and xmllint: Xcode tools) |

**Channels.** A stable release is GitHub's "latest release" (`releases/latest`), the one the console and the Mac app see. `--beta` publishes a *prerelease*, never marked "latest": its manifest has `"channel": "beta"` and its Sparkle feed `<sparkle:channel>beta</sparkle:channel>`. The number stays `X.Y.Z` (the console refuses any other format): a beta takes the next number, and the stable release that follows it a higher one. The Mac app reads the website's cumulative feed, which combines both channels: it only offers a beta if the user asked for them ("Mac app update feed", below).

**Rehearsal (`--dry-run`).** Does everything except the commit, notarisation, release, tag and push, which it prints (`[essai, non lancé] …`). It aligns the versions to build like the release, then puts the version files back as it found them. It writes the cumulative feed to `dist/vX.Y.Z/appcast-site.xml` and shows what would change in `site/updates/appcast.xml`, without touching the latter. It does not rebuild the Apple TV SDK (`build-kyber-tvos.sh` applies the patches in `third_party/`, shared with the other checkouts) and reuses `apps/tvos/Vendor`. Failed checks are listed at the beginning and at the end; the rehearsal goes on to show the rest and exits with an error. `--check` stops after the checks and prints the plan.

**Requirements of an update.** `--min-installer A.B.C` (installed KyberNow version below which the package is refused, 0.2.0 by default) and `--requires-setup` (the update needs the installer) are passed to `package-update.sh`, which keeps a copy in the package (`requirements.json`), and to `scripts/sign-manifest.sh`, whose signed manifest is authoritative. When to use them: "Settings migrations", below.

**What the author sets up once**: `gh auth login` with a token limited to the `MonsieurNours/kybernow` repository (*fine-grained* token, Contents permission read and write: releases, tags and push of `main`; plus Workflows if `main` pushes a change to `.github/workflows/`), the notarisation profile and the Developer ID certificate ("Signing and notarising the Mac app"), the Sparkle key (above) and the minisign key (header of `scripts/sign-manifest.sh`).

## Mac app update feed

The Mac app (from 0.2.7 on) reads `https://www.kybernow.app/updates/appcast.xml` (`SUFeedURL` in `apps/macos/project.yml`): the repository's `site/updates/appcast.xml` file, published by the Pages workflow on every push to `main` that touches `site/`.

- **Every published release**, stable and beta, newest first. Each entry is the one from the `appcast.xml` attached to its release, copied as is by `scripts/merge-appcast.py`: download link to `releases/download/vX.Y.Z/KyberNow-macOS.zip`, length, EdDSA signature, notes. Nothing is re-signed: the app still verifies the archive with `SUPublicEDKey`.
- **Channels.** A beta's entries carry `<sparkle:channel>beta</sparkle:channel>`, stable ones no channel. The app (Sparkle delegate in `KyberNowApp.swift`) asks for the `beta` channel when "Receive beta versions" is ticked in KyberNow › Settings… (`UserDefaults`, key `receiveBetaUpdates`), for none otherwise. Sparkle offers the newest allowed entry: a stable release newer than an installed beta is offered normally, even with the box ticked; unticking the box does not go back to the previous stable release, the app waits for the next one.
- **Older apps keep working.** Versions up to 0.2.6 read `releases/latest/download/appcast.xml`, which GitHub resolves to the latest stable release: every release keeps its `appcast.xml`, so they receive 0.2.7 that way, then read the website's feed.
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
| Console, tray icon, installer | macOS | `cargo test` of the console and the installer; `cargo check --target x86_64-pc-windows-gnu` of the console, the installer (`--features ui`) and the tray icon, Windows resources included (MinGW from Homebrew) |
| Svelte interfaces | Linux | `npm ci`, `npm run build`, `npm run check` in `server/web` and `server/installer/ui` |
| Scripts and update.ps1 | Linux | `shellcheck` of all the scripts, `update.ps1` in ASCII, `update.ps1` test bench (`server/installer/tests/update-bench.ps1`, simulated services) |
| Apple TV app | macOS | `xcodegen`, then `xcodebuild` for the simulator, unsigned, with the latest release's `KyberSDK-tvOS.zip` |

What it does not cover:
- **The Mac app.** It links Kyber's client SDK and VLC (`apps/macos/Vendor`, 88 MB, built from Kyber's sources by `build-kyclient.sh`), published nowhere. To build it in CI, each release would need a `KyberSDK-macOS.zip` (the contents of `apps/macos/Vendor`), like `KyberSDK-tvOS.zip` for the Apple TV.
- **The Apple TV app when the client patches have changed** since the latest release (`apps/macos/kyber-patches`, `build-kyber-tvos.sh`): the published SDK no longer matches the code, so the build is skipped with a warning until the next release. This is the case until 0.2.6 (`KYCLIENT_MEDIA_PACKET_KIND_CLOSED`).
- **Linked Windows binaries and Kyber**: the CI checks the code; it produces neither `KyberNow-Server-Setup.exe` nor the overlay (Kyber patches, `third_party/` absent), and runs nothing on Windows.
- **Nothing is signed or published**: releasing stays `scripts/release.sh`, on the author's Mac (notarisation, Sparkle and minisign keys). A CI that builds the Windows binaries would be the next step, to have them Authenticode-signed by SignPath (free programme for open-source projects, which requires a CI build).

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

## Settings migrations

The console's settings (`C:\ProgramData\KyberNow\kyber-admin.toml`) carry the version of their format: `schema = 1` since 0.2.6 (missing = 0). At each start, and before `init` or `reset-password`, the console brings the file to the format it knows (`server/admin/src/migrate.rs`):

- one migration per version (from N to N+1), in order, each idempotent: running it again changes nothing more;
- before writing, the original is copied to `backups\kyber-admin.toml.schema-<N>-<timestamp>`; each change is written to the log (`kyber-admin.log`); the file's comments are kept;
- a file from a **newer** console (after an update rollback) is left as it is, and the settings this console does not know are kept when it rewrites the file;
- a migration that cannot be done (unreadable file) does not prevent the console from starting: the file is then set aside as before (`kyber-admin.toml.invalide-…`).

By hand: `kyber-admin.exe migrate --dry-run` shows what would change, `kyber-admin.exe migrate` does it.

| Schema | Since | Change |
|---|---|---|
| 1 | 0.2.6 | `listen` becomes a valid `IP:port` address: port 9443 added to a bare address (`"0.0.0.0"`, which prevented the console from starting), `"0.0.0.0:<port>"` for a bare port (`listen = 9443`, which made the whole file unreadable: it was set aside, password included), spaces removed; an unusable value is removed (default address). |

Adding a migration: a `to_N` function in `migrate.rs`, added to `MIGRATIONS`, `CURRENT` raised to N, a test on a file of version N-1, and a row in this table.

**Update that needs the installer.** Some releases cannot be installed by the update package: new driver (virtual display, gamepads), service to recreate, permissions to set again. They are published with `scripts/release.sh X.Y.Z --requires-setup`: the signed manifest carries `"requires_setup": true` and the console then refuses "Install", pointing to `KyberNow-Server-Setup.exe`. Likewise, `--min-installer A.B.C` refuses the package on a computer installed before version A.B.C (`min_installer_version`). The package keeps a copy (`requirements.json`, for a future check by `update.ps1`, which has no manifest); the signed manifest is authoritative.
## Keys, GitHub account and Windows signing

### Backing up the signing keys

Two private keys must never be lost: a new key would make everything already installed refuse all updates, and it would have to be reinstalled by hand.

| Key | Where it lives | Expected public key |
| Sparkle (Mac app, EdDSA) | Login Keychain, item created by `generate_keys` | `SUPublicEDKey` in `apps/macos/project.yml` |
| minisign (Windows releases) | `~/Library/Application Support/KyberNow/minisign.key`, encrypted with the password of the `kybernow-minisign` Keychain item | `server/admin/src/update/kybernow-release.pub` |

The backup (made on 2026-10-21) is an AES-256 encrypted disk image, `kybernow-cles.dmg`, volume `KyberNow-Cles`. It contains:
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

{ printf '\x30\x2e\x02\x01\x00\x30\x05\x06\x03\x2b\x65\x70\x04\x22\x04\x20'; base64 -d -i /Volumes/KyberNow-Cles/sparkle-kybernow.key; } \
  | openssl pkey -inform DER -pubout -outform DER | tail -c 32 | base64
echo test > /tmp/essai.txt
tr -d '\n' < /Volumes/KyberNow-Cles/minisign-motdepasse.txt | { read -r PW; printf '%s\n' "$PW"; } \
  | minisign -S -s /Volumes/KyberNow-Cles/kybernow-minisign.key -m /tmp/essai.txt -x /tmp/essai.txt.minisig >/dev/null 2>&1 \
  && minisign -V -q -p server/admin/src/update/kybernow-release.pub -m /tmp/essai.txt -x /tmp/essai.txt.minisig && echo minisign OK
rm -f /tmp/essai.txt /tmp/essai.txt.minisig

Restore the keys on another Mac (image open):

$GK -f /Volumes/KyberNow-Cles/sparkle-kybernow.key && $GK -p      # must print SUPublicEDKey
mkdir -p ~/Library/Application\ Support/KyberNow
cp /Volumes/KyberNow-Cles/kybernow-minisign.key ~/Library/Application\ Support/KyberNow/minisign.key
cp /Volumes/KyberNow-Cles/kybernow-minisign.pub ~/Library/Application\ Support/KyberNow/minisign.pub
chmod 600 ~/Library/Application\ Support/KyberNow/minisign.key
printf 'add-generic-password -a kybernow -s kybernow-minisign -l kybernow-minisign -w %s\n' \
  "$(cat /Volumes/KyberNow-Cles/minisign-motdepasse.txt)" | security -i >/dev/null

`printf` is a shell builtin: the password does not show up in the process list.

### GitHub settings that protect releases

State on 2026-10-21, checked with `gh api`:
- **2FA** enabled on the `MonsieurNours` account, the repository's only collaborator.
- **Immutable releases** (Settings › General › Releases). A published release can no longer change its files or its tag. `release.sh` creates a draft, uploads everything, then publishes, which stays compatible. A mistake in a published release is fixed with a new release: the number of a deleted release cannot be reused.
- **Rules (Settings › Rules)**:
  - `tags-v-creation`: only the admin role can create a `v*` tag. The Actions `GITHUB_TOKEN` cannot.
  - `tags-v-figes`: no `v*` tag can be moved or deleted, with no exception. If a release fails after the tag was pushed, publish the next version, or disable the rule while cleaning up.
  - `main-legere`: `main` cannot be deleted or rewritten (force push). Direct pushes are still allowed.
- **`gh` on the publishing Mac**: `gh`'s OAuth token (broad rights, development Mac). `release.sh` uses it for releases; `git push` goes over SSH. A fine-grained token limited to the repository would need at least *Contents: Read and write*: GitHub has no narrower permission for releases, so such a token could also push. Remove the unneeded `admin:public_key` scope, which can add an SSH key to the account: `gh auth refresh -h github.com --remove-scopes admin:public_key`.
- **CI**: workflows use the `GITHUB_TOKEN` provided for each run, reduced to `contents: read`. The repository default is read-only as well. No secret to create.

Read the rules back:

gh api repos/MonsieurNours/kybernow/rulesets --jq '.[].name'
gh api repos/MonsieurNours/kybernow/immutable-releases

### Windows signing (Authenticode): on hold

Today `KyberNow-Server-Setup.exe`, `kyber-admin.exe`, `kybernow-tray.exe` and `update.ps1` are not signed: Windows shows “Unknown publisher” (SmartScreen, UAC). This signature would not replace minisign. Authenticode says who signed; minisign proves it is KyberNow's key, built into the console, which also protects against a compromised GitHub account.

Decision of 2026-10-21: **postponed**, to be picked up when the Windows build is more mature. The intended option is SignPath Foundation (free).

Worth knowing before coming back to it (checked on 2026-10-21):
- **Displayed name.** SIREN 510 178 577 is listed in SIRENE under “MATTHIEU REQUENNA”, with no trade name. No certificate can carry “Les Lignes Claires” until that name is declared to the register (INPI one-stop shop, to be checked).
- **Maximum lifetime** of a certificate: 460 days since 2026-03-01.
- **SmartScreen.** No certificate, EV included, gives immediate reputation any more. Reputation builds with installs and is kept as long as the signing identity does not change.

| Option | Price (2026-10) | Displayed publisher | Signing from the Mac | Notes |
|---|---|---|---|---|
| SignPath Foundation | free | “SignPath Foundation” | no | Windows build required on GitHub runners, manual approval of each release |
| Certum Open Source | €69 (card + reader) or €49 (SimplySign cloud) per year | “Open Source Developer, Matthieu Requenna” | yes, `jsign` (card) | individuals only |
| Azure Artifact Signing | $9.99/month | legal name | yes, `jsign` | individuals: US and Canada only; French sole proprietorship undocumented |
| IV / OV (SSL.com, Sectigo, Certum) | IV $129/yr + token or cloud; OV ≈ $200–400/yr | “Matthieu Requenna” | yes, `jsign` | key on a token or in the cloud, mandatory |
| EV | ≈ €330–380/yr | “Matthieu Requenna” | yes, `jsign` | required for an attested driver |

What SignPath will require:
- **Building the whole Windows side in GitHub Actions**: console, tray icon, installer, **Kyber overlay** (`server/kyber-patches/build.sh`), and third-party packages downloaded then checked by hash.
- One signing request per release (`signpath/github-action-submit-signing-request`, API token as an Actions secret), with manual approval on signpath.io.
- On www.kybernow.app:
  - a “Code signing policy” page, with the sentence “Free code signing provided by SignPath.io, certificate by SignPath Foundation”;
  - the roles (author, reviewer, approver);
  - a privacy statement, which mentions the update check against GitHub every 6 hours.
- In every signed file, `ProductName = KyberNow` and the same version.
- 2FA on the SignPath account.
- Two points to declare, which may block admission:
  - the installer adds `sudovda.cer` (self-signed, “Code Signing” usage) to the Root and TrustedPublisher stores;
  - `WebView2Loader.dll` is a redistributable Microsoft component, not open source.

Integration, whatever the option, in this order:
1. sign `kyber-admin.exe`, `kybernow-tray.exe` and `update.ps1`;
2. build `kybernow-update.zip` and `KyberNow-Server-Setup.exe`, then sign the installer;
3. compute `SHA256SUMS` and sign the minisign manifest, last, because the Authenticode signature changes the hashes.

`jsign` (Homebrew) signs `.exe`, `.cab` and PowerShell scripts from the Mac. With SignPath, `release.sh` would fetch the files signed by the CI (`gh run download`) before step 3.

Attested virtual display driver (to replace the self-signed SudoVDA, later):
- an **EV** certificate;
- a Partner Center “Hardware” account (free, the EV is used to register);
- submitting a `.cab` signed with the EV, which Microsoft returns signed, valid on desktop Windows 10/11.

Azure Artifact Signing is not accepted for this. To check before buying: that Partner Center accepts a French sole proprietorship.
