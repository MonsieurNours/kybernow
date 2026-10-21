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

1. Add a `## X.Y.Z` section at the top of `CHANGELOG.md`: these are the notes shown by the console and by the app. Add the same section, in English, to `CHANGELOG.en.md`.
2. If the Kyber patches have changed: `server/kyber-patches/build.sh`.
3. Quit the KyberNow built in Release if it is open (another copy does not matter), then run:

```bash
scripts/release.sh X.Y.Z
```

The script aligns the versions, builds the installer, the update package and the app (Release), signs the Sparkle feed and computes `SHA256SUMS`. It commits, tags `vX.Y.Z`, pushes, then creates the GitHub release. The files stay in `dist/vX.Y.Z/`.

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

## Keys, GitHub account and Windows signing

### Backing up the signing keys

Two private keys must never be lost: a new key would make everything already installed refuse all updates, and it would have to be reinstalled by hand.

| Key | Where it lives | Expected public key |
|---|---|---|
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
- **`gh` on the publishing Mac**: `gh`'s OAuth token (broad rights, development Mac). `release.sh` uses it for releases; `git push` goes over SSH. A fine-grained token limited to the repository would need at least *Contents: Read and write*: GitHub has no narrower permission for releases, so such a token could also push. Remove the unneeded `admin:public_key` scope, which can add an SSH key to the account: `gh auth refresh -h github.com --remove-scopes admin:public_key`.
- **CI**: workflows use the `GITHUB_TOKEN` provided for each run, reduced to `contents: read`. The repository default is read-only as well. No secret to create.

Read the rules back:

```bash
gh api repos/MonsieurNours/kybernow/rulesets --jq '.[].name'
gh api repos/MonsieurNours/kybernow/immutable-releases
```

### Windows signing (Authenticode): on hold

Today `KyberNow-Setup.exe`, `kyber-admin.exe`, `kybernow-tray.exe` and `update.ps1` are not signed: Windows shows “Unknown publisher” (SmartScreen, UAC). This signature would not replace minisign. Authenticode says who signed; minisign proves it is KyberNow's key, built into the console, which also protects against a compromised GitHub account.

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
2. build `kybernow-update.zip` and `KyberNow-Setup.exe`, then sign the installer;
3. compute `SHA256SUMS` and sign the minisign manifest, last, because the Authenticode signature changes the hashes.

`jsign` (Homebrew) signs `.exe`, `.cab` and PowerShell scripts from the Mac. With SignPath, `release.sh` would fetch the files signed by the CI (`gh run download`) before step 3.

Attested virtual display driver (to replace the self-signed SudoVDA, later):
- an **EV** certificate;
- a Partner Center “Hardware” account (free, the EV is used to register);
- submitting a `.cab` signed with the EV, which Microsoft returns signed, valid on desktop Windows 10/11.

Azure Artifact Signing is not accepted for this. To check before buying: that Partner Center accepts a French sole proprietorship.
