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
| Relay | `POST /v1/devices/code {name}` → `{code, poll, expires, interval, url, url_complete}`; `POST /v1/devices/poll {poll}` | Device sign-in by code (TVs) |
| Relay | `GET /tv`; `GET/POST /compte/appareil?code=…` | Entering a TV's code; “Connect {device}?”, “It wasn't me” |
| Relay | `GET /compte/connexion?appareil=…&retour=kybernow://compte[&etat=…]`; `GET/POST /compte/connecter-appareil`; `POST /v1/devices/token {code}` | Device sign-in through the system window (return through the `kybernow://compte?code=…` scheme, code exchanged for the device session) |
| Relay | `POST /v1/devices/revoke` | The app signs out (its device session) |
| Relay | `GET /v1/computers` | The account's computers, with the rights and the Station identifier |
| Relay | `POST /v1/access {station}` | Access token (5 min) |
| Relay | `attach_request` → `attach_code`, then `attached`; `attach_status`; `detach` / `detached` (Station, through its control connection) | Attachment, after approval in the console (`/compte/rattacher?code=…`); detaching on both sides |
| Admin | `GET/PUT /api/account` (Admin session) | Attached account, switch, detach |
| Admin | `POST /api/account/attach` (Admin session) | “Sign in”: attach code asked of the relay, console page to open |
| Admin | `GET /api/local/account`, `PUT /api/local/account/relay`, `POST`/`DELETE /api/local/account/attach`, `POST /api/local/account/enroll` (local token) | The installer: chosen relay, attaching during installation, enrollment of a silent installation (“Installers”) |
| Relay | `enroll` → `attached` (Station, through its control connection) | Enrollment with an organization token (batch 5) |
| Admin | `GET /api/account/access`, `POST /api/account/access/{id}/block` | Local list, blocking |
| Admin | all the routes of a paired device | Also accept `Authorization: KyberNow <jeton>` according to the rights |

### Token format (batch 2)

Defined in the shared crate `server/relay/tunnel` (`access` module): the relay signs, the Station verifies, the apps' core carries it without reading it.

- **Compact form**: `<payload>.<signature>`, each in base64url without padding; 4 KiB at most. The payload is the JSON below; the Ed25519 signature covers `kybernow-access/1 ` followed by the encoded payload (no other signature by the relay key passes for a token).
- **Payload**:

  ```json
  {"v":1,"relay":"share.kybernow.app","station":"<Station identifier>",
   "account":"<account>","device":"<device>","rights":["session","share"],
   "name":{"account":"Léa Martin","email":"lea@example.org","device":"Léa's iPad"},
   "iat":1795000000,"exp":1795000300,"jti":"<16 bytes in base32>"}
  ```

  `rights`: `session`, `lend`, `share`, `admin` (an unknown word from a later version is ignored). `name` carries what Admin shows in its list (the email is optional).
- **Published key** (`GET /v1/keys`): `{"keys":[{"kid":"<8 bytes of the key's SHA-256, base32>","alg":"Ed25519","key":"<32 bytes, base64url>"}]}`; two keys during a rotation. Admin shows the `sha256:…` fingerprint of the raw key.
- **Verification by the Station**, in this order: pinned key (one of the keys from the attachment), version, `relay` = the attachment's relay, `station` = its own, `exp - iat` of 5 minutes at most, `iat` not in the future and `exp` not past, within **60 s** (clocks), then the `jti`.
- **`jti` never replayed**: the same token serves all of its device's calls during its 5 minutes (session, heartbeat), but a `jti` is never accepted for different content; to restart or update the Station, the token is **single use** (once consumed, it is good for nothing else). `jti`s are forgotten when they expire.
- Header: `Authorization: KyberNow <token>`.

### Admin (batch 2)

- **Settings**: `[account]` section of `kyber-admin.toml`: `relay`, `owner` (`kind` person or organization, `id`, `name`, `email`), `keys` (pinned keys), `pinned_at`, `accept` (the switch). Attaching (the relay's `attached`, see below) calls `accounts::pin`; it turns the switch and “Access away from home” on, which Admin lets you turn off.
- **Local list**: `account-access.json` in the data folder, one entry per account and device (names, rights of the last token, first and last visit, `lan` or `relay`); blocking a device or the whole account. A first visit adds a line to the feed. Detaching clears the list.
- **Routes** (Admin session, never through the relay):

  | Route | Body | Response |
  |---|---|---|
  | `GET /api/account` | | `{attached, relay, owner, keys:[{kid, fingerprint}], pinned_at, accept, test, station, console, can_attach, attaching}` |
  | `PUT /api/account` | `{accept?: bool, detach?: true, cancel_attach?: true}` | The same view; turning on without an attachment: `409 not_attached`; detaching tells the relay |
  | `POST /api/account/attach` | | `{url, expires}`: the console page to open; `409 no_relay` or `no_accounts`, `503 relay_unreachable` |
  | `GET /api/account/access` | | `{entries:[{id, account, device, name, email, device_name, rights, first_seen, last_seen, via, blocked, account_blocked, owner}]}` |
  | `POST /api/account/access/{id}/block` | `{blocked: bool, account?: bool}` | The list; `account`: the whole account |

- **According to the rights**, on the network as through the relay (the guard's `Device` class, “Access away from home” on):
  - **Session**: the `/api/stream/*` routes and the host's (share, lending) like a paired device; `prepare` also returns `kyber_token` `{token, user, expires}`, the Kyber token (5 min) under the `kn-account-…` account, never written to Kyber's configuration;
  - **Share**: `POST /api/share/join` without `code` gets in right away (no code, no host approval), under the person's name; the host sees them and can remove them;
  - **Administer**: `POST /api/stream/station/restart` and `/update`, also **through the relay** (closed through the relay to a Kyber account), with a single-use token;
  - **Lend**: carried by the token and shown in the list; the launcher with an account comes in batch 4.
- **Refusals**: `{error, code}`, `code` among `accounts_off`, `not_attached`, `bad_token`, `token_expired` (401: the app asks for a new token), `blocked`, `no_right` (403).
- **Test mode**: `KYBERNOW_ACCOUNT_TEST=1` at startup pins a local key (`account-test.key`, relay `essai.kybernow.invalid`) when the Station is attached to no relay; `kyber-admin account-token --rights session,share` writes a token signed by it for this Station.
- **Code**: `server/admin/src/accounts.rs` (settings, verification, list, routes), wired into `api.rs` (`stream_user`, restart and update), `relay.rs` (guard) and `share_api.rs` (join).
- **Interface**: Admin's “KyberNow account” space (`#/compte`): attached, the account, the relay, the pinned key, “Manage in the console”, “Detach”, the switch and the list (“Block”, “Block the account”, “Unblock”); not attached, “Sign in” (active when a relay is set, `can_attach`) opens the relay's console in the browser, then the page waits for the confirmation (`attaching`, “Reopen the page”, “Cancel”).

### Attaching and devices on the relay (batch 2, relay half of batch 3)

- **Signing key**: `signing.key` (Ed25519 in PKCS#8), next to `secret_key`, created at first start, reserved to the relay (0600). Rotation: the relay's administrator drops `signing.next.key`; it is published along with the current key (`GET /v1/keys`) and given to the Stations that attach or reconnect (`attached`); once they all have it, it replaces `signing.key`. The relay's name in the tokens (`relay`) is the host of `public_url`, with its port when not 443.
- **Control connection messages** (docs/relais.en.md, “Frames and messages”; a relay or Station from before accounts ignores them):

  | Message | Direction | Fields |
  |---|---|---|
  | `attach_request` | Station → relay | `req`, `name` (the computer's name) |
  | `attach_code` | relay → Station | `req`, `code`, `url` (`https://<relay>/compte/rattacher?code=…`), `expires` (900 s); relay without accounts: `error {req, code: "no_accounts"}` |
  | `attached` | relay → Station | `req` (in answer to `attach_status`), `relay`, `account`, `owner {kind, id, name, email}`, `keys` (like `GET /v1/keys`) |
  | `attach_status` | Station → relay | `req`; at each connection of an attached Station: answer `attached` (up-to-date keys) or `detached` |
  | `detach` | Station → relay | Detached in Admin |
  | `detached` | relay → Station | `req` (in answer to `attach_status`); detached in the console |
  | `enroll` | Station → relay | `req`, `token` (the enrollment token), `name`; answer `attached` with this `req`, or `error {req, code}`: `enroll_refused` (`message`: `unknown`, `expired`, `revoked`, `used_up`), `no_accounts` (see “Installers” and “People, rights and organizations”) |

- **Attaching**: Admin › “Sign in” asks for a code (kept 15 minutes in memory by the relay, one per Station) and opens the page in the browser; you sign in (the page is remembered, `__Host-kn_suite` cookie), “Attach {name}”, owner “Me” (organizations come in batch 5), “Attach”; the relay records the link (`stations` table) and sends `attached`. Station offline: it learns at its next connection (`attach_status`). Admin only takes an `attached` if “Sign in” was asked within 15 minutes, except to update the keys of the same attachment.
- **Detaching**: from the account page (“Computers”, “Detach”: `detached` to the Station) or in Admin (`detach` to the relay, as soon as it is reached).
- **Device sessions**: separate from browser sessions; 256-bit bearer token (`Authorization: Bearer …`), kept hashed, without expiry; “Connected devices” on the account page (“Disconnect”); a new password closes them all, and so does deleting the account (its Stations are then detached and receive `detached`). System window: `retour` is `kybernow://compte` (nothing else), `etat` is given back as is; after “Connect”, a page sends back to `kybernow://compte?code=…&etat=…` (single-use code, 5 minutes, exchanged at `POST /v1/devices/token`). Code: eight characters `XXXX-XXXX` (no vowel, no 0, 1), 10 minutes, polled every 5 seconds (`202` pending, `200` the session, `403 denied`, `410 expired`); 20 codes asked and 10 wrong codes typed per address in 15 minutes.
- **Access token**: `GET /v1/computers` returns `{computers:[{station, name, rights, owner, mine, online, attached_at}]}`; `POST /v1/access {station}` returns `{token, station, rights, expires, lifetime}`, `404 unknown_station` for a Station not in the account, `401 bad_session` without a device session. In batch 2, an account's computers are the ones it owns, with all rights.
- **Notice for each new device**: a device's first token for a Station (`device_stations` table) sends an email to the owner (“New device on {computer}”) and adds a `nouvel_appareil` line to the log.
- **Database**: migration 3 (`stations`, `devices`, `device_codes`, `device_stations`); migration 2 is batch 1b's.
- **Code**: `server/relay/src/accounts/stations.rs` (key, attaching), `devices.rs` (devices, API, tokens), `web/attach_pages.rs` and `web/device_pages.rs` (pages); the hub (`hub.rs`) forwards the messages.

### People, rights and organizations (batches 4 and 5)

Done on the relay side (console, API, database) and on the Admin side; the apps (batch 3) are separate work, the installers' account step is described in “Installers”.

- **An account's rights on a computer**: all of them for its owner (a person), and for the Owners and Administrators of the owning organization; otherwise the union of the **given rights** not expired, to them, to an organization they are a member of, or to one of its groups. **Administer only counts with two-factor authentication** (active TOTP or a passkey, which is worth it), owner included: without it, the token and `/v1/computers` don't have `admin`, and the “People” page says so. Likewise, an organization Owner or Administrator without two-factor doesn't manage the organization and only has the given rights, like a member.
- **Managing a computer's access** (“People”): its owner, the Owners and Administrators of the owning organization, and anyone with the (effective) Administer right.
- **Adding a person**: their address, their rights, an optional end date (the end of the chosen day, UTC). With a verified account, the right is theirs right away; without an account, it waits for the address and moves to the account **when it is verified** (sign-up, forgotten-password link, or account created by the relay's administrator). Adding the same address again changes their rights. An email says so (“{name} opened {computer} for you”), with the console or the sign-up page.
- **Opening to an organization**: a computer (a person's or an organization's) opens to a whole organization one is a member of, or to one of its groups, with rights; its log records it.
- **Organization**: creating one requires two-factor authentication (the creator is its Owner). Inviting a member by address (verified account: member right away, email; no account: pending invitation, taken when the address is verified). Only an Owner names or removes an Owner; an organization always keeps one (“Leave” and changing the last one's role are refused). A deleted account: its organization gets another Owner (the oldest Administrator, otherwise the oldest member), or goes away with its computers if nobody is left.
- **Organization log**: every change (members, roles, invitations, groups, rights given and removed, computers attached, detached, enrolled, tokens) and every opening of one of its computers (who, which device, from where; an opening: a first token after an hour without), and every new device. IP addresses forgotten after 7 days, lines after 12 months; seen by Owners and Administrators only.
- **New-device notice**: for an added person's device, the email goes to the owner and names the person (“with Léa's KyberNow account”); for an organization's computer, to the Owners and Administrators, with a log line.
- **Attaching**: the “Attach {name}” page offers “Me” and the organizations the account manages (Owner or Administrator, two-factor); `attached` then carries `owner {kind: "organization", id: "org_…", name}`. Attaching to another owner erases the given rights and first visits.
- **Enrollment tokens** (“Deploy Stations”): a name, a duration (1 to 365 days), a number of Stations (1 to 10,000), and rights given to a group on each enrolled Station; `kne_` and 32 characters (160 bits), **shown only once**, kept hashed. The Station presents it through its control connection (`enroll`); the relay attaches it to the organization (same answer if it already is, without counting one more Station), gives the rights to the group, writes the log. Revoking prevents new enrollments; enrolled Stations stay.
- **Admin on the network** (“Sign in with KyberNow”): Admin's sign-in page offers the button when the Station is attached and accepts accounts, **never through the relay**. Admin draws a `nonce` (16 bytes, 10 minutes, `kyber_admin_kn` cookie, `SameSite=Lax`) and opens `https://<relay>/compte/ouvrir-admin?station=…&nonce=…&retour=<Admin's address>`. Signed in, with the Administer right (two-factor), the person confirms “Open Admin of {computer}?”; the relay issues an access token (5 min, device `web`) whose **`jti` is the `nonce`**, and sends back to `<retour>/api/login/kybernow?token=…`. `retour` is only a local network address (private, link-local or loopback IP, dotless name, `.local`, `.lan`, `.home.arpa`), without a path. Admin checks the token (Administer right, **single use**) and that its `jti` is this browser's `nonce`, then opens its session; a line in the feed.
- **Lending with an account**: `POST /api/lend/status` and `/api/lend/borrow` accept `Authorization: KyberNow <token>` with the Lending right instead of a friend account's login and password; borrowing happens under the account's name (no time slots: they stay those of friend accounts; Steam: “Their Steam account”). The allowed apps stay those of the “Lending” page.
- **Enrollment on the Admin side**: two ways in, a single `enroll` message to the relay. The silent installer hands the token over through the local route `POST /api/local/account/enroll` (answered at once, for its exit code: “Installers”); a deployment tool can also write `[account] enroll = "kne_…"` in `kyber-admin.toml` (with the relay in `[relay]`): Admin presents it as soon as the relay is reached, takes `attached` like an asked attachment (accounts accepted, access away from home on), then erases it; refused for good (`enroll_refused`): a line in the feed and the token is erased; relay unreachable: new try, further and further apart (10 min at most).

#### Console pages

| Route | Screen |
|---|---|
| `GET /compte/ordinateurs` | “Computers”: “Yours”, “Opened for you”, one section per organization; “Create an organization” |
| `GET /compte/ordinateur?id=…` | A computer's “People”: rights per person (Session, Lending, Sharing, Administer), “Add a person”, “Open to an organization”, “Remove”, “Detach this computer”; only one's own rights for whoever doesn't manage |
| `POST /compte/ordinateur/ajouter` | `id`, `email`, `session`/`lend`/`share`/`admin` (`1`), `fin` (`yyyy-mm-dd` or `dd/mm/yyyy`, optional) |
| `POST /compte/ordinateur/ouvrir` | `id`, `a` (`org_…`, or `org_…:<group>`), rights |
| `POST /compte/ordinateur/retirer` | `id`, `acces` |
| `GET/POST /compte/organisations/creer` | `nom`; two-factor required |
| `GET /compte/organisation?id=org_…&vue=membres\|groupes\|ordinateurs\|journal` | “Organization”: members and roles, pending invitations, groups, computers and their access, log |
| `POST /compte/organisation/inviter`, `/role`, `/retirer`, `/quitter`, `/renommer` | `email` and `role`; `compte` and `role`; `compte` or `invitation`; nothing; `nom` |
| `POST /compte/organisation/groupes/creer`, `/supprimer`, `/membre` | `nom`; `groupe`; `groupe`, `compte`, `action` (`ajouter`, `retirer`) |
| `GET /compte/organisation/deployer?id=…`; `POST /compte/organisation/jetons/creer`, `/revoquer` | “Deploy Stations”; `nom`, `jours`, `nombre`, `groupe`, rights; `jeton` |
| `GET/POST /compte/ouvrir-admin` | “Open Admin of {computer}?” (`station`, `nonce`, `retour`) |

The console bar (“Computers”, each organization, “Account”) is also at the top of `/compte`. Detaching from “People” goes back to “Computers”.

#### API and messages

- `GET /v1/computers`: one's own, those opened to them (to them, their organizations, their groups), and all those of the organizations they manage; `owner` is `{kind: "person", id: "acc_…", name}` or `{kind: "organization", id: "org_…", name}`, `mine` says whether they own it, `rights` their rights at this moment. `POST /v1/access`: the same rights, `404 unknown_station` without any right.
- Control connection: `enroll {req, token, name}` (Station → relay); answer `attached {req, …}`, or `error {req, code: "enroll_refused", message}` with `unknown`, `expired`, `revoked` or `used_up`; relay without accounts: `no_accounts`. The `tunnel` crate carries it (`Agent::enroll`).
- Admin: `POST /api/login/kybernow {origin, lang}` → `{url, expires}` (no session, `403 network_only` through the relay, `409 unavailable` without attachment or with the switch off); `GET /api/login/kybernow?token=…` → Admin's session (a page that goes back home) or the refusal; `GET /api/session` says `kybernow_login`.

#### Database

- Migration 4: `grants` (Station, `account` or an invitation's `email`, `rights` as words `session,share`, `expires_at`, `invited_by`, `created_at`); `device_stations.seen_at` (last visit, for the log).
- Migration 5: `organizations` (public identifier `org_…`, name, plan), `org_members` (role `owner`, `admin`, `member`), `org_invites`, `org_groups`, `group_members`, `org_log` (who, what as JSON, address, when), `enroll_tokens` (hash, start of the token, number, used, group and rights, expiry, revocation); `stations` rebuilt (`owner_kind` `person` or `organization`, `account` or `org`, `enrolled_by`); `grants.org` and `grants.grp`.
- Code: `server/relay/src/accounts/people.rs` (given rights, an account's rights), `orgs.rs` (organizations, log, tokens, `enroll`), `console_pages.rs` (pages), `web/console.rs`, `web/org_pages.rs`, `web/admin_login.rs`; Admin: `accounts.rs` (sign in with KyberNow, enrollment), `lend_api.rs` (lending with an account); interface: `server/web/src/pages/Login.svelte`.

### Installers

Mock-up: “Installateurs” row. Code: `server/installer/src/account.rs` (flow, relay check, Admin routes), `quiet.rs` (silent installation), `ui/src/lib/account.svelte.ts` and `AccountStep.svelte` (interface shared by Windows and the Mac); `apps/windows/setup/src/account.rs` and `ui/src/screens/Account.svelte` (Link).

- **Station, during installation** (Windows and Mac): the Installation screen becomes “Meanwhile: your KyberNow account” (“Sign in”, “Create an account”, “Relay: share.kybernow.app · Other relay…”, “Later”); installation goes on in the background, its progress stays at the top. The step is skipped for an already attached Station reinstalled keeping its settings (`[account] relay` in `kyber-admin.toml`).
- **“Other relay…”**: address `host[:port][#sha256:…]`, checked live by the installer (`GET /v1/health`: reached, version; `GET /v1/keys`: accounts on or not; web authorities, or the fingerprint of a self-signed relay). A relay without accounts is kept for sharing and away-from-home access, without attaching. The chosen relay becomes the Station's (Admin › Away from home), even with “Later”.
- **Attaching**: as soon as the “admin” task is done (Admin started), the installer calls Admin through its local routes (below); Admin asks the relay for the code (`attach_request`) and returns the page `/compte/rattacher?code=…`, which the installer shows **in its window** (wry child view: WebView2, WKWebView), without leaving the relay (any other link goes to the browser; “Create an account” replaces the first sign-in page with sign-up, and the relay's follow-up cookie brings back to attaching). “Whose computer is this?” is the relay's page (“Me” or an organization the account manages, batch 5). The installer follows Admin (`GET /api/local/account` every 2 s, 15 minutes at most) until `attached`; the final screen shows the three accesses: attached (account or organization, relay), “KyberNow accounts accepted”, “Away-from-home access on”. A self-signed relay: the page opens in the browser (the window's view would refuse its certificate).
- **Admin's local routes** (loopback and local token, like `/api/local/*`; never through the relay):

  | Route | Body | Answer |
  |---|---|---|
  | `GET /api/local/account` | | The view of `GET /api/account`, plus `server` (the set relay) and `remote_access` |
  | `PUT /api/local/account/relay` | `{relay}` | Sets the relay (“Later”); the same view |
  | `POST /api/local/account/attach` | `{relay?}` | Sets the relay, then like `POST /api/account/attach`: `{url, expires}` |
  | `DELETE /api/local/account/attach` | | Abandons the attaching in progress (“Back”, “Later”) |
  | `POST /api/local/account/enroll` | `{token, relay?}` | Enrollment: the view, attached; `400 bad_token` (shape), `403 bad_token` (refused by the relay: `enroll_refused`), `409 no_relay` / `no_accounts`, `503 relay_unreachable` |

  All refuse an already attached Station (`409 already_attached`): attaching elsewhere goes through “Detach” in Admin. The local token is the tray icon's: the person at the computer can already accept a pairing; they can also attach a Station that isn't attached.
- **Enrollment** (deploying a fleet): `KyberNow-Station-Setup.exe /quiet /enroll=<token> /relay=<address>` installs without any screen or WebView2 (default options, settings kept from an existing installation, installation account `kybernow` with a random password: you take over through the tray icon, “Reset Admin password…”), then hands the token to Admin (`POST /api/local/account/enroll`, 3 minutes of patience while Admin starts), which sends it to the relay through its control connection: `{"t":"enroll","req":3,"token":"kne_…","name":"GRAPH-01"}`. The relay answers as for an attachment confirmed in the console, `attached` carrying the same `req` (owner: the token's organization), or `error {req, code: "enroll_refused" | "no_accounts"}` (`enroll_refused` carries the reason: `unknown`, `expired`, `revoked`, `used_up`); a relay from before enrollment doesn't answer (10 s: `no_accounts`). Admin pins the answer like a requested `attached` (switch and away-from-home access on). Exit codes: `0`, `3010` (installed, reboot needed), `1` (installation failed), `2` (installed, not enrolled); everything is written to `install.log`. On the Mac: `kybernow-ui --quiet --enroll=<token> --relay=<address>` (the executable of “Install KyberNow Station” in the .dmg), in the user's session (not as root); macOS asks once for the administrator password, permissions are granted afterwards (tray icon, or a PPPC profile). Admin only checks the token's shape (8 to 256 characters `A-Z a-z 0-9 _ - . ~`); the relay decides (batch 5: duration, number of Stations, revocation). Tests: `server/relay/tunnel/tests/enrolement.rs` and Admin's `accounts::tests::enrolement_avec_un_relais_simule`, with the tunnel's simulated relay (`test-relay` feature).
- **KyberNow Link for Windows**: on first installation, optional step “Your KyberNow account” between Welcome and Installation (not for an update). “Sign in” opens in the window `https://<relay>/compte/connexion?appareil=<PC name>&retour=kybernow://compte&etat=<16 random bytes>`; the return `kybernow://compte?code=…&etat=…` is intercepted (`etat` must match), the code exchanged (`POST /v1/devices/token`), and the device session **handed to the app** (batch 3):
  - the **token** in the Windows Credential Manager, generic credential, target `KyberNow:account:<relay>` (`<relay>`: `host`, or `host:port` other than 443), user the account's email, secret the token in UTF-16LE (like the computers' passwords, `KyberNow:<id>`), `LOCAL_MACHINE` persistence;
  - the **rest** in `%APPDATA%\KyberNow\accounts.json`: `{"accounts":[{"relay":"share.kybernow.app","account":{"id","name","email"},"device":{"id","name"},"added":<Unix seconds>}]}` (one entry per relay, replaced when signing in again; the token is never in it; `relay` carries `#sha256:…` for a self-signed relay).

  The app reads both places at start-up and starts signed in; if it stores its accounts differently, it takes them from there then deletes the file. “Create an account” opens the site's sign-up in the window, then “Account created: sign in”. “Later”: the app signs in from its home screen. Uninstalling while erasing data also erases both places (`--forget-all` removes the `KyberNow:*` targets).

Gaps of this step:

- the “Me” / “{organization}” choice and the computer's name are not installer screens: it's the relay's attach page, shown in its window;
- a self-signed relay's page opens in the browser, not in the window;
- the silent installation takes the default options (no `/user=` or `/password=`, which would show in the process list);
- on the Mac, the step must be finished (attached or “Later”) before permissions: their end restarts Admin, which would forget an attachment in progress.

### Data kept by the relay

| Data | Why | Duration |
|---|---|---|
| Email, Argon2 hash of the password, TOTP secret (encrypted), passkeys (public keys) | Signing in | Lifetime of the account |
| Signed-in devices (name, kind, last visit) | Device sessions, revocation | Lifetime of the device |
| Attached Stations (identifier, name) | The account's computers | Until detached |
| Given rights (person or invited address, rights, end date) | Added people | Until removed or detached; an expired invitation goes after 7 days |
| Organizations, members, invitations, groups, rights | Fleet | Lifetime of the organization |
| Enrollment tokens (hash, start, number) | Deploying a fleet | Lifetime of the organization |
| Log (accesses, changes; organization log) | Security, fleet | 12 months (IP addresses: 7 days) |
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
4. **People and rights**: adding by email, Session, Lending, Sharing, Administer rights; lending and sharing with an account. **Done on the relay and Admin sides** (“People, rights and organizations”).
5. **Organizations**: members, roles, groups, the organization's computers, log, mandatory two-factor authentication. **Done on the relay and Admin sides**; the installer takes the enrollment token (“Installers”).
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
| `POST /compte/connexion/passkey` | “Continue with a passkey” (batch 1b): the session opens in full, with no two-factor code |
| `POST /compte/passkeys/ajouter`, `/renommer`, `/retirer` | Add a passkey (from `/compte` or `/compte/proteger`), rename it, remove it |
| `GET/POST /compte/supprimer` | “Delete the account”: password or passkey asked again, then erasure (attached computers detached, connected devices closed) and a confirmation email |
| `POST /compte/export` | “Export my data”: `kybernow-compte.json` (account, accepted terms, security without secrets, sessions, attached computers, connected devices, log) |
| `GET /conditions`, `/confidentialite` | Terms of use and privacy policy ([legal](juridique/README.en.md)), drafts marked as such, served with accounts |

### Database (SQLite, `accounts.db` in the relay's volume)

`accounts` (id, unique lowercase email, name, Argon2id hash, verified at, created at, deleted at, plan) · `email_tokens` (SHA-256 hash of the token, account, purpose `verify`/`reset`, six-digit code, expiry) · `sessions` (token hash, account, device, created, seen, expiry, 2FA done) · `totp` (account, encrypted secret, enabled at) · `recovery_codes` (account, hash, used at) · `passkeys` (account, credential id, public key, counter, name, created, seen) · `trusted_browsers` (hash, account, expiry) · `audit` (account, kind, address, when). Versioned migrations at startup.

Migration 2 (batch 1b): on `accounts`, `webauthn_user` (WebAuthn identifier, 32 random bytes), `terms_version`, `terms_accepted_at`, `privacy_version`, `privacy_accepted_at`; on `passkeys`, `alg` (COSE) and `transports`; `webauthn_used` (hash of a challenge already used, expiry). Migration 3 is batch 2's (see “Attaching and devices on the relay”).

### Settings (`relay.toml`, `[accounts]` section)

`enabled` (off by default on a self-hosted relay), `database`, `secret_key` (file: encrypts TOTP secrets, signs cookies), `public_url`, and `[accounts.smtp]`: `host`, `port`, `starttls`, `user`, `password_file`, `from`. Without SMTP: neither email verification nor password reset; the relay's administrator creates accounts (`kybernow-relay accounts add`).

### Security

`__Host-kn_session` cookie (`Secure`, `HttpOnly`, `SameSite=Lax`), 256-bit token kept as a hash; CSRF token per form; Argon2id; 10 sign-in attempts per address and 5 per account in 15 minutes; emails and tokens never in the logs; same answers whether an account exists or not (sign-in, reset, sign-up with a taken address: an email tells its owner).

Passkeys (batch 1b): verification written on `ring`, with no WebAuthn library (the Rust ones pull in OpenSSL, which the image doesn't have): CBOR and COSE keys read by the relay, ES256, Ed25519 and RS256, no attestation requested, user verification required (a passkey counts as two-factor authentication), a counter going backwards refused. Challenges aren't stored: signed with the relay's key, bound to the browser (CSRF cookie) and to their purpose, valid for 10 minutes, recorded once used (single-use). The pages keep a single script, `/assets/passkey.js`, served by the relay (`script-src 'self'` on `/compte/…` only); without it, the passkey buttons don't show.

### Order

1a. Database, sign-up, verification, sign-in, sign-out, password reset, two-factor and recovery codes, SMTP, limits, account page. **Done.**
1b. Passkeys (WebAuthn), account deletion, data export; version and date of the accepted terms, `/conditions` and `/confidentialite` pages; erasure of never-verified accounts after 7 days. **Done.**

Batch 1b departures:

- deletion erases the account **right away** (no 30-day delay in the database); the 30 days are those of the backup rotation;
- the legal texts served are the public relay's, as drafts; a self-hosted relay has no setting yet to serve its own, and previous versions can't be browsed yet;
- passkey sign-in doesn't offer autofill yet (`mediation: "conditional"`): a button is enough for now.

Batches 4 and 5 departures:

- the two-factor authentication required for Administer applies **to the computer's owner too** (in batch 2, they had all rights unconditionally); a passkey is worth it, as at sign-in;
- an existing account added to a computer or an organization is added **right away**, without accepting (the email tells them); an address without an account waits for the address to be verified;
- no end date for rights given to an organization or a group (only to people), and no role per group; lending's time slots and Steam account stay those of friend accounts;
- the organization log keeps the openings of its own computers; those of a person's computer opened to the organization stay in the owner's account log;
- “Copy” for the enrollment token isn't a button (the pages have no script): the token is selected in one click;
- “Add a person” is a form on the page, not a window on top as in the mockup.

## Open question

1. **Demo account for Apple**: on the public relay, with the Session right on the demo Station (suggested).
