# Install KyberNow Link on your iPhone or iPad (without the App Store)

English · [Français](SIDELOAD.md)

The iPhone and iPad app is neither on the App Store nor on TestFlight: it is installed from a Mac, signed with **your** Apple account. Allow ten minutes the first time, one minute afterwards.

## What you need

- An **iPhone** or **iPad** on iOS 18 or iPadOS 18 or later, and a cable to connect it to the Mac (Wi-Fi is enough afterwards).
- A **Mac** with **Xcode** (free, on the Mac App Store; open it once so it finishes installing).
- An **Apple account**:
  - **free** (your usual Apple ID): it works, but the app stops opening after **7 days**; just run the script again to renew it. No iCloud and no shared Keychain (the "From your Mac" adding stays empty); at most three apps installed this way;
  - **paid** (Apple Developer Program, 99/year): the app stays installed for a year, with iCloud and the shared Keychain. “From your Mac” adding only sees the computers published by a Mac app signed with the same team and the same app identifier: not those of the Mac app published on the website, whose iCloud storage belongs to the KyberNow team.
- A computer with **KyberNow Station** installed (version 0.3.1 or later).

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

## 2. Prepare the iPhone or iPad

1. Connect it to the Mac, unlock it and accept **Trust This Computer**.
2. Turn on **Developer Mode**: **Settings › Privacy & Security › Developer Mode** (the device restarts). Without it, iOS refuses any app that does not come from the App Store.
3. In Xcode, **Window › Devices and Simulators**: the device shows up on the left. Wait for Xcode to finish preparing it (a few minutes the first time).

## 3. Install the app

From the `kybernow` folder:

```sh
# Free account
apps/ios/scripts/sideload.sh --team ABCDE12345

# Paid account: every feature (iCloud, shared Keychain)
apps/ios/scripts/sideload.sh --team ABCDE12345 --full
```

The script:
1. downloads the KyberNow core and its libraries already built for iOS (`KyberSDK-iOS.zip`, attached to releases from 0.4.0 on: the latest one, or `--version X.Y.Z`), into `apps/ios/Vendor`; when they are already there (built locally, or downloaded on a previous run), it reuses them without downloading anything;
2. prepares the Xcode project in your name (app identifier `kybernow.<your team>`, change it with `--bundle` if needed);
3. builds the app (Release configuration, automatic signing); Xcode creates the signing profile itself;
4. installs it on the connected device (`--device "iPad name"` if there are several).

## 4. First launch

- With a free account, iOS first refuses to open the app: **Settings › General › VPN & Device Management**, tap your account under "Developer App", then **Trust**.
- Open **KyberNow**, allow **local network** access: without it, the app cannot find your computers.
- Add your computer: **On the network** (its account), **On-screen code** (accept it in KyberNow Admin, Link devices), **Pairing QR code** (shown by KyberNow Admin, with the camera) or **Address**. "From your Mac" (iCloud) needs an installation with `--full` and a Mac app signed with the same team (see "What you need").

## Renewing or updating

Run the same command again after a `git pull`. With a free account, do it before the 7 days are up; your computers and settings stay on the device.

The SDK is only downloaded when `apps/ios/Vendor` does not contain `KyberNow.xcframework`: to move to a new release, delete that folder before running the script again (`rm -rf apps/ios/Vendor`). It comes from a published release, while `git pull` gives the most recent code: if the core changed since that release, the build fails. Then take the code of the SDK's release (`git checkout vX.Y.Z`), or build the SDK yourself (below).

## Troubleshooting

- **"iPhone ou iPad introuvable"** (not found): it is not connected, not unlocked, or Xcode does not see it yet (Window › Devices and Simulators). Developer Mode must be on.
- **The build fails after an update**: the SDK in `apps/ios/Vendor` no longer matches the code, see “Renewing or updating”.
- **Signing error**: open `apps/ios/KyberNowIOSSideload.xcodeproj` (made by the script) in Xcode, *Signing & Capabilities* tab of the KyberNowIOS target: Xcode explains what is missing there. With a free account, do not use `--full`, and keep at most three apps installed this way.
- **"Untrusted Developer"** when opening: see "First launch".
- **The app does not find the computer**: the computer must be on, on the same network, with KyberNow Station installed; otherwise use the address or the QR code (the code only goes to the computers found on the network). Over VPN, discovery does not get through: use the address.
- **Building the SDK yourself** (developers): `apps/core/scripts/build-apple.sh --target aarch64-apple-ios` (and `aarch64-apple-ios-sim`), then `apps/apple/scripts/build-apple-mobile.sh ios`, see [README.en.md](README.en.md).

KyberNow is based on [Kyber](https://kyber.tech) (AGPLv3), with no affiliation with the Kyber team.
