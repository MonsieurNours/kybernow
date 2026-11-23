# KyberNow accounts: framing

English · [Français](comptes.md)

A **KyberNow account** (email address, password, two-factor authentication, passkeys) gives access, from any device, to every computer it is entitled to: your own, those opened to you (family, friends), those of your team. The relay becomes a Parsec-style service: it holds the accounts, the rights, the organizations, and **decides on access**; each relay installed elsewhere has its own accounts and stays entirely independent.

This document is the framing, to be approved before the mockup and the code. Status: **decisions of 23/11/2026** (table below), mockup: https://claude.ai/artifact/RSTsS6BYAt9SN9GEMEgiuc; the question still open is at the end.

## Decisions

| Topic | Choice |
|---|---|
| Model | Free for now; paid plans will come (a “plan” field on the account and the organization, nothing else today) |
| Trust | **The relay decides**: it issues signed tokens that the Station verifies; four safeguards on the Station side (below) |
| Account | **Optional**: local network, pairing by code, invitations ([invitations.en.md](invitations.en.md)) and relays without accounts remain possible |
| Fleet | My computers everywhere; family and friends; teams and companies (organizations, roles, groups, log). No enterprise SSO for now |
| Rights on a computer | **Session** (the full desktop), **Lending** (the launcher, [pret.en.md](pret.en.md)), **Sharing** (join its shares without a code), **Administer** |
| Self-hosted relay | Accounts specific to each relay; an app can be connected to several relays |
| Sign-in | Email and password, two-factor authentication (TOTP), passkeys |
| Attaching a Station | From the Station installer, or from Admin (“KyberNow account › Sign in”) |
| Installers | Optional “KyberNow account” step during Station installation (relay pages, “Another relay…”, owner, access turned on); enrollment token to deploy a fleet; sign-in also offered by the Link Windows installer |
| Invitations | Kept, alongside accounts |
| Emails | Configurable sending (SMTP in the relay's configuration); the public relay goes through an EU provider |
| Database | SQLite in the relay's container, backed up |
| What is managed where | **The relay's web console** (everything, fleet included) and **Admin** (attach the Station, switch, accesses seen and blocked); the apps sign in and show the account's computers |
| Legal | Les Lignes Claires, data controller |
| Schedule | **Everything, before the first App Store submission** |
| Administer | Manage the computer's access in the console, restart and update the Station from the app, **including through the relay**; open Admin on the network (below). Not the whole Admin interface through the relay |
| Sharing with an account | A person with the Sharing right **joins without the host's approval**; the host sees them and can remove them |
| Admin on the network | Admin's password stays; **an account with the Administer right also opens Admin** (“Sign in with KyberNow”, on the network only) |
| Person and organization | A computer attached to a person can be **opened to an organization** or one of its groups, like to a person |

## What the user sees

### Creating an account and signing in

- **On the relay's site** (`https://share.kybernow.app/compte`, the console): “Create an account” (email, password), verification email, then an offer to add a passkey and two-factor authentication.
- **In the Link apps**: “Sign in to a KyberNow account” (empty home screen, app settings). The app never sees the password:
  - iPhone, iPad, Mac, Windows, desktop Linux, Android: the relay's sign-in page opens in a system window (`ASWebAuthenticationSession`, Custom Tabs, browser); passkeys and two-factor authentication work there as on the site;
  - Apple TV, Android TV, TVs, living room, Deck: an eight-character code and a QR code are shown; you approve it from your phone (“Connect the living room Apple TV?”);
  - several relays: “Add an account” first asks for the relay (`share.kybernow.app` suggested).
- Creating and deleting the account happen on the site, not in the app (App Store, guideline 5.1.1(v): the app does not create accounts).

### My computers, everywhere

- Once signed in, the app shows **the account's computers** on the home screen, with the same cards as today; a small mark distinguishes a computer that comes from the account (and, with several accounts, which one).
- “Connect” opens the session, on the network or through the relay, without pairing. A computer already paired on this device appears only once.

### Attaching a Station

- Admin › **KyberNow account** › “Sign in”: Admin opens the relay's console in the browser (with an attach code); you sign in, choose the account or the organization, and confirm “Attach Living room PC”.
- The Station then **belongs to that account** (or to that organization); “Accept KyberNow accounts” turns on, and Admin shows the account, the list of accesses, and “Detach”.
- Attaching only happens from Admin: no one can attach someone else's Station.

### In the installers

- **Station** (Windows and Mac installers): a **“KyberNow account”** step, optional (“Later”: attach afterwards in Admin), **during installation** (installation keeps running in the background). It shows the **relay's pages** (sign-up, sign-in, passkey through Windows Hello or Touch ID, two-factor authentication) inside the installer's window.
  - **Relay**: `share.kybernow.app` suggested; “Another relay…” opens an address field (host, port, fingerprint for a self-signed relay), checked before going on. The chosen address becomes the Station's relay (Admin › Away from home).
  - **Owner**: the person; if the account administers organizations, a choice “Me” / “{organization}”.
  - **Attachment** as soon as Admin starts; “Accept KyberNow accounts” and “Access away from home” are turned on, the installer says so, Admin can turn them off.
- **Fleet deployment**: an organization's console gives an **enrollment token** (lifetime, number of Stations); the installer takes it on the command line, silently (`KyberNow-Station-Setup.exe /quiet /enroll=<token> /relay=<address>`), and attaches the Station to the organization without a screen.
- **KyberNow Link for Windows**: its installer also offers, optionally, “Sign in to a KyberNow account” (same relay pages); the app then starts signed in. The other apps sign in from their home screen.

### Adding people

- Console › a computer › **“Add a person”**: their email and their rights (Session, Lending, Sharing, Administer), optionally an expiry date.
- The person receives an email; with an account, the computer appears on their side; without an account, the link offers to create one.
- **Lending**: the allowed apps, the time slots and the Steam account stay set in Admin (“Lending” page); for an account, the “Lending” right replaces the friend account (local username and password), which remains possible without an account.
- **Sharing**: the person joins the computer's shares without typing a code, and without waiting for the host's consent (they are known); the host sees them and can remove them like any guest.

### Fleet (organizations)

- An **organization** (“Les Lignes Claires”) has members, each with a role: **Owner**, **Administrator**, **Member**; **groups** (“Graphic designers”); **computers** attached to the organization rather than to a person.
- Rights are given to a person or to a group. The organization's log says who opened what, when, from where, and every change of rights.
- Two-factor authentication **mandatory** for an organization's Owners and Administrators, and for any account with the Administer right.

## Architecture

### Pieces

| Piece | Where | Role |
|---|---|---|
| Accounts | `server/relay` (new `accounts` module, SQLite) | Accounts, sessions, passkeys, TOTP, organizations, groups, rights, log, emails |
| Console | `server/relay` (pages served by the relay, like the share page) | Create the account, sign in, manage computers, people, organization; approve a device or an attachment |
| Tokens | `server/relay/tunnel` (shared format) | Access token signed by the relay, verified by the Station |
| Station | Admin (`accounts.rs`, `accounts_api.rs`) | Attachment, switch, token verification, list of accesses, blocking |
| Core | `apps/core` (`account.rs`) | Device sign-in (browser or code), the account's computers, token for each call |
| Apps | all | Sign-in screen, home screen with the account's computers, “Accounts” settings |

### The access token

- The relay has a **signing key** (Ed25519), published at `GET /v1/keys`; the Station **pins it when attaching**.
- The signed-in device asks the relay for a token for a computer: `POST /v1/access {station}` with its device session. The relay checks the account, its rights on that computer, the relay-side switch, and returns a token **valid for 5 minutes**: `{v, relay, station, account, device, rights, name, iat, exp, jti}`, signed.
- The device presents it to Admin (`Authorization: KyberNow <jeton>`), on the network or through the relay. Admin checks: signature (pinned key), `station` = its own, `exp`, switch on, account and device not blocked locally. From it, Admin derives the rights.
- **Session**: Admin opens the session as for a paired device, but Kyber is reached with a **Kyber token** (like share guests), not with a Kyber account: no account to create, no Kyber restart, and revocation on the relay applies at the next token (the heartbeat asks for a new token every 4 minutes).
- The relay still does not see the picture or the input: the tunnel stays end to end with Admin pinned.

### The Station's four safeguards

1. **Switch** in Admin, “Accept KyberNow accounts”: when off, Admin refuses any token (paired devices, invitations and the local network work as before).
2. **Local list**: Admin keeps each account and device that came in with a token (name, email, rights, last visit) and can **block** one, without going through the relay; an account blocked here is blocked even if the relay says yes.
3. **Short, signed tokens**: 5 minutes, relay key pinned, `jti` never replayed.
4. **Notice for each new device**: the first time a device opens a computer, the relay sends an email to the owner (or to the organization's Administrators) and Admin adds a line to the feed.

### Routes (first draft)

| Where | Route | What |
|---|---|---|
| Relay | `GET /v1/keys` | Public signing key |
| Relay | `POST /v1/devices/code` → `{code, poll}`; `POST /v1/devices/poll` | Device sign-in by code (TVs) |
| Relay | `GET /compte/connexion?appareil=…` | Device sign-in through the system window (return through the `kybernow://compte` scheme) |
| Relay | `GET /v1/computers` | The account's computers, with the rights and the Station identifier |
| Relay | `POST /v1/access {station}` | Access token (5 min) |
| Relay | `POST /v1/stations/attach` (Station, through its control connection) | Attachment, after approval in the console |
| Admin | `GET/PUT /api/account` (Admin session) | Attached account, switch, detach |
| Admin | `GET /api/account/access`, `POST /api/account/access/{id}/block` | Local list, blocking |
| Admin | all the routes of a paired device | Also accept `Authorization: KyberNow <jeton>` according to the rights |

### Data kept by the relay

| Data | Why | Duration |
|---|---|---|
| Email, Argon2 hash of the password, TOTP secret (encrypted), passkeys (public keys) | Signing in | Lifetime of the account |
| Signed-in devices (name, kind, last visit) | Device sessions, revocation | Lifetime of the device |
| Attached Stations (identifier, name) | The account's computers | Until detached |
| Organizations, members, groups, rights | Fleet | Lifetime of the organization |
| Log (accesses, changes) | Security, fleet | 12 months |
| IP addresses | Security, limits | A few days (logs), as today |

Deleted account: erased within 30 days (backups included). Data export from the console.

### Emails

SMTP set in `relay.toml` (host, port, credentials, sender); without SMTP, a self-hosted relay has neither email verification nor forgotten password (the console says so, the relay's administrator creates the accounts). Messages in French and English: verification, forgotten password, new device, added to a computer or an organization, two-factor authentication changed.

### Relay security

- Passwords: Argon2id; at least 10 characters; list of known passwords refused.
- Console: `__Host-` session cookie, `Secure`, `HttpOnly`, `SameSite=Lax`, CSRF protection (per-form token, `Sec-Fetch-Site` checked); attempt limits per address and per account.
- Signing key: in a separate file, reserved for the relay; rotation possible (two keys published, the Station pins both during the transition).
- A hacked relay can issue tokens for any Station whose switch is on: that is the price of “the relay decides”, bounded by the safeguards above.

## Legal and stores

- **GDPR** (Les Lignes Claires, data controller): record of processing activities, the service's privacy policy (public relay), terms of use, data processing agreement for organizations, breach notification within 72 h, email provider in the EU.
- **App Store**: sign-in in the app, creation and deletion on the site (no creation in the app); no Sign in with Apple (nor Google nor any other third-party sign-in); a **demo account** in the App Review notes, simpler than the invitation.
- **Paid, later**: a subscription sold in the iOS app = Apple in-app purchase (3.1.1); a plan sold directly to organizations = possible outside in-app purchase (3.1.3(c)).

## Batches

1. **Relay, accounts**: SQLite database, creation, verification, sign-in, forgotten password, TOTP, passkeys, console (account), emails, limits.
2. **Attachment and tokens**: signing key, attaching from Admin, access token, verification in Admin, the four safeguards.
3. **Apps**: sign-in through the system window and by code, the account's computers on the home screen, token in every core call, several relays.
4. **People and rights**: adding by email, Session, Lending, Sharing, Administer rights; lending and sharing with an account.
5. **Organizations**: members, roles, groups, the organization's computers, log, mandatory two-factor authentication.
6. **Legal and App Store**: texts, demo account, updated App Review notes.

Each batch has its mockup approved before the code, like invitations.

## Batch 1: accounts on the relay

Mockup: “Sign-up and sign-in” row of https://claude.ai/artifact/RSTsS6BYAt9SN9GEMEgiuc.

### Pages (served by the relay, French and English, like the share page)

| Route | Screen |
|---|---|
| `GET/POST /compte/inscription` | “Create an account”: name, email address, password (at least 10 characters, meter, known passwords refused), terms and privacy; refused when the page comes from an app (`?appareil=`) |
| `GET/POST /compte/verifier` | “Check your email”: link valid 24 h, or six-digit code; “Resend the email”, “Change it” |
| `GET /compte/proteger` | “Protect your account”, after activation: passkey (recommended), two-factor authentication; “Later” |
| `GET/POST /compte/connexion` | “Sign in”: passkey first, then email and password; “Forgot your password?”; “Create an account” except from an app |
| `GET/POST /compte/code` | “Two-factor authentication”: TOTP code, “Don't ask again on this device for 30 days”, “Use a recovery code” |
| `GET/POST /compte/oubli`, `/compte/nouveau-mot-de-passe` | Link valid one hour; always the same answer (“If an account exists at this address, a link has just been sent to it.”); a new password closes the other sessions |
| `GET /compte` | The account (until batch 4's console): email, security (passkeys, two-factor, recovery codes), open sessions, “Sign out”, “Delete the account” |

### Database (SQLite, `accounts.db` in the relay's volume)

`accounts` (id, unique lowercase email, name, Argon2id hash, verified at, created at, deleted at, plan) · `email_tokens` (SHA-256 hash of the token, account, purpose `verify`/`reset`, six-digit code, expiry) · `sessions` (token hash, account, device, created, seen, expiry, 2FA done) · `totp` (account, encrypted secret, enabled at) · `recovery_codes` (account, hash, used at) · `passkeys` (account, credential id, public key, counter, name, created, seen) · `trusted_browsers` (hash, account, expiry) · `audit` (account, kind, address, when). Versioned migrations at startup.

### Settings (`relay.toml`, `[accounts]` section)

`enabled` (off by default on a self-hosted relay), `database`, `secret_key` (file: encrypts TOTP secrets, signs cookies), `public_url`, and `[accounts.smtp]`: `host`, `port`, `starttls`, `user`, `password_file`, `from`. Without SMTP: neither email verification nor password reset; the relay's administrator creates accounts (`kybernow-relay accounts add`).

### Security

`__Host-kn_session` cookie (`Secure`, `HttpOnly`, `SameSite=Lax`), 256-bit token kept as a hash; CSRF token per form; Argon2id; 10 sign-in attempts per address and 5 per account in 15 minutes; emails and tokens never in the logs; same answers whether an account exists or not (sign-in, reset, sign-up with a taken address: an email tells its owner).

### Order

1a. Database, sign-up, verification, sign-in, sign-out, password reset, two-factor and recovery codes, SMTP, limits, account page.
1b. Passkeys (WebAuthn), account deletion, data export.

## Open question

1. **Demo account for Apple**: on the public relay, with the Session right on the demo Station (suggested).
