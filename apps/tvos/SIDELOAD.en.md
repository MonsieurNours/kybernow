# Install KyberNow Link on your Apple TV (without the App Store)

English · [Français](SIDELOAD.md)

The Apple TV app is not on the App Store: it is installed from a Mac, signed with **your** Apple account. Allow ten minutes the first time, one minute afterwards.

## What you need

- An **Apple TV 4K** (2nd generation, 2021, or later), up to date (tvOS 18 or later).
- A **Mac** on the same network, with **Xcode** (free, on the Mac App Store; open it once so it finishes installing).
- An **Apple account**:
  - **free** (your usual Apple ID): it works, but the app stops opening after **7 days**; just run the script again to renew it. No iCloud ("From your Mac" adding), no Top Shelf and no per-user profiles on the Apple TV;
  - **paid** (Apple Developer Program, 99/year): the app stays installed for a year, with iCloud, the Top Shelf and per-user profiles. “From your Mac” adding only sees the computers published by a Mac app signed with the same team and the same app identifier: not those of the Mac app published on the website, whose iCloud storage belongs to the KyberNow team.
- A computer with **KyberNow Station** installed (version 0.2.2 or later for code pairing).

## 1. Prepare the Mac

1. In Terminal, install XcodeGen (with [Homebrew](https://brew.sh)):

   ```sh
   brew install xcodegen
   ```

2. Get the KyberNow code:

   ```sh
   git clone https://github.com/MonsieurNours/kybernow.git
   cd kybernow
   ```

3. In **Xcode › Settings › Accounts**, add your Apple account (**+** button). Note the **team ID**: 10 characters, for example `ABCDE12345`. Xcode shows it when you pick the team to sign an app; for a paid account it is also on [developer.apple.com/account](https://developer.apple.com/account) (Membership). For a free account, it is the "(Personal Team)" team.

## 2. Pair the Apple TV with Xcode

1. On the Apple TV: **Settings › Remotes and Devices › Remote App and Devices**. Leave this screen open.
2. On the Mac, in Xcode: **Window › Devices and Simulators**. The Apple TV shows up on the left; click **Pair** and type the code shown on the TV.
3. Wait for Xcode to finish preparing the Apple TV (a few minutes the first time).

## 3. Install the app

From the `kybernow` folder:

```sh
# Free account
apps/tvos/scripts/sideload.sh --team ABCDE12345

# Paid account: every feature (iCloud, Top Shelf, tvOS profiles)
apps/tvos/scripts/sideload.sh --team ABCDE12345 --full
```

The script:
1. downloads the Kyber SDK and the KyberNow core already built for Apple TV (`KyberSDK-tvOS.zip`, attached to every published release: the latest one, or `--version X.Y.Z`, 0.3.1 or later, older archives lack the core), into `apps/tvos/Vendor`; when they are already there (built locally, or downloaded on a previous run), it reuses them without downloading anything;
2. prepares the Xcode project in your name (app identifier `kybernow.<your team>`, change it with `--bundle` if needed);
3. builds the app (Release configuration); Xcode creates the signing profile itself;
4. installs it on the paired Apple TV (`--device "Apple TV name"` if there are several).

**Wake the Apple TV before running the script**: asleep, it accepts the installation but refuses to open the app ("System is asleep").

## 4. First launch

- Open **KyberNow** from the Apple TV home screen.
- Allow **local network** access: without it, the app cannot find your computers.
- Add your computer: **On-screen code** (accept it in KyberNow Admin, Link devices), **On the network** (its account, typed on the iPhone keyboard), or **Address**.

## Renewing or updating

Run the same command again after a `git pull`. With a free account, do it before the 7 days are up; your computers and settings stay on the Apple TV.

The SDK is only downloaded when `apps/tvos/Vendor` is missing: to move to a new release, delete that folder before running the script again (`rm -rf apps/tvos/Vendor`). It comes from a published release, while `git pull` gives the most recent code: if the core changed since that release, the build fails. Then take the code of the SDK's release (`git checkout vX.Y.Z`), or build the SDK yourself (below).

## Troubleshooting

- **"Apple TV introuvable"** (not found): it is not paired, or Xcode cannot see it (same network, Apple TV awake). Check in Window › Devices and Simulators.
- **The build fails after an update**: the SDK in `apps/tvos/Vendor` no longer matches the code, see “Renewing or updating”.
- **Signing error**: open `apps/tvos/KyberNowTVSideload.xcodeproj` (made by the script) in Xcode, *Signing & Capabilities* tab of the KyberNowTV target: Xcode explains what is missing there. With a free account, do not use `--full`.
- **The app does not find the computer**: the computer must be on, on the same network, with KyberNow Station installed; otherwise use the address (the code only goes to the computers found on the network). Over VPN, discovery does not get through: use the address.
- **Building the Kyber SDK yourself** (developers): `apps/tvos/scripts/build-kyber-tvos.sh`, then the core: `apps/apple/scripts/build-apple-mobile.sh tvos`, see [README.en.md](README.en.md).

KyberNow is based on [Kyber](https://kyber.tech) (AGPLv3), with no affiliation with the Kyber team.
