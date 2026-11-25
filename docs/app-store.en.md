# KyberNow Link on the App Store

English · [Français](app-store.md)

KyberNow Link for iPhone, iPad and Apple TV is heading to the App Store, **with** computer lending and PlayStation consoles (decision of 23/11/2026). This document gives what is done in the repository, the answers to give in App Store Connect, the notes for App Review and what remains. Reference rules: [App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/), version of 16/10/2026.

## Scope

| App | App Store | Why |
|---|---|---|
| iPhone and iPad (`apps/ios`) | Yes | |
| Apple TV (`apps/tvos`) | Yes | |
| Mac (`apps/macos`) | No, stays on Developer ID | The Mac App Store requires the sandbox (2.4.5 i), rejects Sparkle (2.4.5 iv) and the open-at-login item ticked without consent (2.4.5 iii) |

The three apps share one identifier (`fr.leslignesclaires.kybernow`): a single App Store listing, as a **universal purchase** (iPhone, iPad, Apple TV). Free, no in-app purchase.

## What is done in the repository

### Privacy manifests

One `PrivacyInfo.xcprivacy` per app and per KyberKit (the embedded framework, Rust core included), added to the Xcode projects; XcodeGen picks them up on its own (they sit in the source folders). No tracking, no data collected by the developer, and the required-reason APIs:

| API | Reason | Where |
|---|---|---|
| `UserDefaults` | CA92.1: the app's settings | Apps and KyberKit |
| File timestamps | C617.1: files in the app container (computer images) | Apps (`Platform.swift`), KyberKit (Rust core) |
| System boot time | 35F9.1: measuring durations (`mach_absolute_time`, `systemUptime`: frames, gamepads) | Apps and KyberKit |
| Active keyboards | 54BD.1: keyboard layout sent to the computer (`KeymapIOS.swift`) | iOS KyberKit |

Top Shelf calls none of these APIs: no manifest. Any new API from [Apple's list](https://developer.apple.com/documentation/bundleresources/describing-use-of-required-reason-api) must go into the manifest of the same bundle.

### Encryption

`ITSAppUsesNonExemptEncryption` is `true` on iOS and tvOS: the apps encrypt with standard algorithms that are not Apple's system ones (rustls TLS, quinn QUIC, ring; AES in the PlayStation console transport). In App Store Connect:

1. "Does your app use encryption?": **Yes**.
2. Type: **standard encryption algorithms instead of, or in addition to, Apple's operating system encryption** (no proprietary algorithm).
3. United States: mass-market item (5D992), no annual report since the BIS rule of March 2021.
4. France: App Store Connect asks for the **declaration of supply of a cryptology means** filed with ANSSI when the app is distributed there. To be filed before the first submission; Apple then gives a compliance code, to put in `ITSEncryptionExportComplianceCode` so the question is no longer asked on every version.

## App Store Connect

### Privacy labels

- **Tracking**: no.
- **Data collected by the developer**: the relay (`share.kybernow.app`) keeps IP addresses in its logs for a few days, for operation and security, not linked to an identity. Apple has no "IP address" category: nothing to declare as long as it is not used to locate the user. If the relay kept more (accounts, usage statistics), revisit the labels.
- Everything else stays on the device or in the user's iCloud, out of the developer's reach (computers, passwords in the Keychain, PSN account ID and online name): nothing to declare.
- **Privacy policy URL**: the Privacy section of the legal page (`https://www.kybernow.app/en/legal/`, `/mentions-legales/`). Add the PlayStation accounts there (ID and online name read once from Sony, kept on the device and in iCloud) before submitting.

### Age rating

The app shows the computer's screen unfiltered (browser, games, any content): answer "yes" to unrestricted web access, which gives the highest bracket.

### Listing

- Name Apple's platforms and Windows; do not mention Android, Google or KyberNow's other platforms (2.3.10).
- PlayStation: text only, as compatibility ("play your PS5 and PS4"), with "PlayStation, PS5 and PS4 are trademarks of Sony Interactive Entertainment Inc."; no Sony logo or image, neither in the icon nor in the screenshots.
- Screenshots on real computers, without protected game content in the spotlight.
- Category: Utilities, secondary category: Productivity (the app is not a game). No Game Mode on iPhone and iPad (`GCSupportsGameMode`, `LSSupportsGameMode`) while the app is listed this way: it would turn on at launch, and an app that is not a game and turns it on risks rejection. The Mac app, outside the App Store, has it (see [apps/macos/README.en.md](../apps/macos/README.en.md), “macOS Game Mode”).

## App Review

The reviewer has no Station: with no way to connect, the submission is rejected (2.1). It takes:

- **A demo Station** switched on during review, with "Access away from home" on, reachable through the public relay;
- **An invitation** ([invitations.en.md](invitations.en.md)) as the main path: Admin › Link devices › “Invite a device…”, name “App Review”, 30 days. The reviewer types the code in “Join with a code”, then the password: the computer is added to their home screen through the relay, with nobody to accept it, and “Connect” opens the whole desktop, the generic case of 4.2.7. Five devices at most (iPhone, iPad, Apple TV); after a rejection or a new submission: “New password”, or a new invitation. No friend account in the notes: lending is described and shown in the video, not offered for testing;
- **A video** on a real iPhone and a real Apple TV: adding, session, sharing, lending, PlayStation console (the console cannot be lent to the reviewer: the video shows it).

### Notes for App Review

> KyberNow Link is a remote desktop client for the user's own Windows PC, running our free host software KyberNow Station (https://www.kybernow.app). It is a generic mirror of the host's screen (guideline 4.2.7): every app runs on the user's PC, nothing is executed or sold in the client. No account is created in the app; there is no in-app purchase.
>
> To test: tap "Join with a code", enter **{code}**, then the password **{password}**. This adds our demo PC to the app (no approval is needed on our side; the access is valid until {date}). Then tap "Connect": you get the PC's full desktop, with touch, keyboard, mouse and game controllers. The demo PC is reachable over the internet through our relay and stays on during review. The same code and password work on iPhone, iPad and Apple TV (up to five devices).
>
> Lending (KyberNow Share): the owner of a PC can lend it to a trusted friend, from the host (KyberNow Admin). During a loan the friend sees the owner's own Windows session, restricted by the owner to the apps they chose; the owner creates and manages the friend's access on the host, and can end it at any time. It is shown in the attached video.
>
> PlayStation consoles: the app can also display the user's own PS5 or PS4 on their home network, like Sony's Remote Play. Signing in with Sony happens once, on Sony's own page, only to read the user's PSN account ID, which the console requires for pairing; it is stored on the device. A console cannot be provided for review: see the attached video, {link}.
>
> Video of the full flow on iPhone and Apple TV: {link}.

## Accepted risks

- **Lending (4.2.7)**: during a loan the friend sees a launcher limited to the apps the host chose; a reviewer may see it as a "mirror of specific software" under 4.2.7 a, which requires a computer owned by the user, on their local network. Our answer: it is the host's own Windows session, restricted by them; access is created and managed on the host (4.2.7 c); nothing can be bought or browsed like a store (4.2.7 d).
- **PlayStation (5.2.1, 5.2.2)**: Sony's protocol reimplemented, Sony sign-in in a web view, trademark names. Other PlayStation remote-play apps are on the App Store, but Apple may ask for Sony's authorisation, and a complaint from Sony would get the app pulled.
- **Licences**: rights holders may publish the app under the AGPL. The console code is written after chiaki-ng (AGPL-3.0): if it takes its expression and not just the protocol, its authors are rights holders too, and the GPL is deemed incompatible with the App Store terms (VLC pulled in 2011). Same question for any outside contribution.
- **Reviewer path**: the reviewer comes in through an invitation and sees the whole desktop; lending is only described and shown. It must not be hidden for all that (2.3.1, hidden features): the notes and the video present it. A reviewer, or a later re-review, may still try it.
- **Fallback plan**: if Apple rejects one of these features, hide it in App Store builds with a build flag (`APPSTORE`), without touching the versions installed from a Mac.

## Left before submission

- [ ] App Store listing (universal purchase iPhone, iPad, Apple TV), FR and EN text, screenshots.
- [ ] Demo Station (on, no sleep, “Access away from home” on), 30-day invitation, video.
- [ ] Legal page: add the PlayStation accounts to the Privacy section.
- [ ] ANSSI declaration, then `ITSEncryptionExportComplianceCode`.
- [ ] Kyber licence (`third_party/kyber-desktop`): confirm it allows App Store distribution.
- [ ] Console code: check what comes from chiaki-ng, or get its authors' consent.
- [ ] Add a `LICENSE` file at the repository root (only `apps/Cargo.toml` states `AGPL-3.0-or-later`), possibly with an additional permission for app stores (AGPL, section 7).
- [ ] App Store distribution profiles (iOS, tvOS, Top Shelf), Release archive, internal TestFlight first.
- [ ] Multicast entitlement (`com.apple.developer.networking.multicast`), optional: wake by broadcast.
