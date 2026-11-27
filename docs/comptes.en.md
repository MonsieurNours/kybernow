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
| Emails | Configurable sending (SMTP in the relay's configuration); the public relay goes through SendGrid (Twilio), sender `hello@kybernow.app` |
| Database | SQLite in the relay's container, backed up |
| What is managed where | **The relay's web console** (everything, fleet included) and **Admin** (attach the Station, switch, accesses seen and blocked); the apps sign in and show the account's computers |
| Legal | Les Lignes Claires, data controller |
| Schedule | **Everything, before the first App Store submission** |
| Administer | Manage the computer's access in the console, restart and update the Station from the app, **including through the relay**; open Admin on the network (below); **all of Admin from anywhere** with “Remote Admin”, off at the start (“Admin from KyberNow Share”, decided on 11/25/2026) |
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

### Watch, access links and computer groups

Decided on 25/11/2026.

**Watch** (right `view`, “Watch” / « Regarder »): see a computer's screen without taking it over: no keyboard, mouse, controller or clipboard.
- The right's sentence: “See the computer's screen, with no keyboard, mouse or controller.”; example: “Your son who wants to show you his game”.
- It is the **real screen**, as it is: no virtual display, nothing changes for the person using the computer. Watching goes through sharing (KyberNow Share): the person joins the computer's open share as a **spectator** guest; if there is none, the Station opens one for them, of the PC's main screen, or of the session in progress if someone is connected. That share has no code to show; it closes when the last spectator leaves.
- **Several screens**: the bar offers “Screen” and the list of the PC's screens; changing it restarts the picture on the other screen.
- **The host knows**: Admin's feed says “{name} is watching your screen”, the share panel (Admin, tray icon, session bar) shows them like any guest, with “Remove”; the host can also give them the controllers or the keyboard, like any guest.
- **Refusals**: computer lent (“{name} is lent right now.”), sharing turned off in Admin, accounts turned off, Station too old (“This Station can't be watched yet: update it.”).
- With the “Open the desktop” right, the card keeps “Connect”; “Watch” is then in the computer's menu.

**Access links** (“Access links”, a computer's page, for whoever manages its access): open the computer to someone whose address you don't have, through a **single-use** link.
- Create: the rights (the same tiles), “The link works for” 1, 7 (suggested) or 30 days, and “Access until (optional)”. The link (`https://{relay}/compte/lien/kla_…`) is **shown only once**, with “Copy link” and “Send it to one person: it works once and is only shown here.”; the relay keeps only its hash. Ten pending links at most per computer (“Revoke a link before creating another one.”).
- Opening the link: signed out, the page offers to sign in or create an account, then comes back to the link; signed in: “{name} is opening {computer} to you”, the rights in words, the end date if any, “Accept”. Accepting gives these rights to the account (added to the ones it already has on this computer, each with its own end date), and the link is **used**: it no longer works for anyone. The owner opening their own link: “This is your computer: this link is for someone else.”, the link stays pending.
- List: “Pending · until {date}”, “Used by {name} on {date}”, “Expired”, “Revoked”; “Revoke” (“Revoke this link? It will no longer work.”). Revoking a used link doesn't remove the access: remove it in “Who has access”.
- Refusals on opening: “This link has already been used.”, “This link has expired.”, “This link has been revoked.”, “This link can't be read.”.
- Log: creation, use (who) and revocation, in the family's or team's log for one of its computers, and in the account's log.

**Computer groups** (family or team, “Computers” tab): a **list of computers** of the family or team, chosen by hand (“Studio workstations”).
- Create (“Name”), add or remove a computer, delete the group (“Delete the group “{name}”? The rights given on this group go with it.”). A computer can be in several groups; a computer leaving the family or team leaves its groups.
- **Open a group**: to the whole family or team, to one of its groups of people, or to one of its members, with rights and an optional end date. These rights apply to every computer in the group, **present and future**, and add to the ones given computer by computer.
- On a computer's page, “Who has access” also shows the rights coming from a group: “Through the group {group}”, with no “Remove” (they are changed on the group).
- **Deploy**: an enrollment token can put the enrolled computers straight into a computer group (“Put in the group”).
- Managing computer groups: Owners and Administrators, with two-factor authentication.

### Invitations, adding members and sessions

Decided on 25/11/2026.

**Invitations** (a person added to a computer by their address, a member invited to a family or team):
- A pending invitation **expires after 30 days**; the rights it carried no longer pass to the account. It stays in the list, “Invitation expired”, until it is resent or removed.
- Under the address: “Invitation sent on {date} · expires on {date}”; expired: “Invitation expired on {date}”.
- **“Resend the invitation”**: sends the e-mail again and restarts the 30 days (also for an expired invitation); at most once an hour per invitation (“Invitation already resent less than an hour ago.”); notice “Invitation resent to {address}.”; a line in the log.
- French: « Invitation envoyée le {date} · expire le {date} », « Invitation expirée le {date} », « Renvoyer l'invitation », « Invitation déjà renvoyée il y a moins d'une heure. », « Invitation renvoyée à {adresse}. ».

**Adding someone** (“Add a person”, “Invite a member”, “Open a group” to a member):
- The address field **suggests people you already know** as you type: the members of your families and teams, and the people you've already opened a computer to, “First Last — address”. No script: a browser list (`datalist`).
- **No duplicates**: addresses are compared ignoring case and spaces; adding someone who already has access to this computer changes their rights instead of adding a line (“{name} already had access: their rights are updated.”); inviting someone already in the family or team is refused (“{name} is already a member.”); a pending invitation for the same address is replaced, not doubled. The “People” page shows each person once, even if added by address on one computer and by account on another.
- French: « {nom} avait déjà accès : ses droits sont mis à jour. », « {nom} en fait déjà partie. ».

**Sessions**:
- The access token a device shows a Station lasts **2 minutes** (instead of 5); the apps renew it on their own. A Station still accepts up to 5 minutes (older relay).
- A **device session** (an app signed in to the account) **expires after 90 days unused**: the app says “This account was signed out of this device: sign in again.”, as after signing out from the account's page.
- “Account and security”, list of devices: “Used {duration} ago”; help “A device unused for 90 days is signed out.” (« Utilisé il y a {durée} » ; « Un appareil qui ne sert pas pendant 90 jours est déconnecté. »).

### Synced settings and moving a computer

Decided on 25/11/2026.

**Synced settings**: an account computer's settings follow the person from one device to another.
- What follows: each computer's **profile** (Auto, Game, Desktop…), **video format**, **bitrate**, **sharp text (4:4:4)**, **HDR** and **immersive mode**. What stays on the device: its screen and resolution, its sound, its controllers, its shortcuts, its app preferences.
- Kept by the relay, per account and per computer (a small object, 4 KB at most); the last change wins. A device reads them when it starts, at each account refresh and before a session; it sends them when one of these settings changes. A value a device can't apply (a format it can't decode, HDR without an HDR screen, immersive mode on a TV) is kept as is, but the device applies what it can, as today.
- For a computer both paired and on an account (same Station), the account's settings apply too.
- App settings, “Accounts” section: “Sync computer settings” (on by default), help “Each computer's profile, format, bitrate, HDR and immersive mode follow you on your devices.” (« Synchroniser les réglages des ordinateurs » ; « Le profil, le format, le débit, le HDR et le mode immersif de chaque ordinateur vous suivent sur vos appareils. »).
- Deleted with the account; a computer detached or no longer open to the account loses its settings.

**Moving a computer** (a computer's page, “Move…” / « Déplacer… »): to you, or to a family or team you manage, from you or from a family or team you manage (Owner or Administrator, two-factor authentication, on both sides).
- Choose the destination, then a **preview** “What changes” / « Ce qui change »: “Lose access:” the previous owner's families, teams and groups and the access coming from a computer group; “Leaves the computer groups: {groups}”; “Pending links cancelled: {n}”. A box “Keep the access given to people” / « Garder les accès donnés à des personnes » (checked) keeps the rights given one by one to people; unchecked, they go too.
- Button “Move {computer}” / « Déplacer {ordinateur} », then the notice “{computer} now belongs to {destination}.” / « {ordinateur} est maintenant à {destination}. ». Both sides' logs record it; the Station learns it like an attachment (Admin shows the new owner).
- Refusals: “You don't manage {destination}.”, “Turn on two-factor authentication first.” (« Vous ne gérez pas {destination}. », « Activez d'abord la double authentification. »).

## The account space

Decided on 11/25/2026 with the mockup (https://claude.ai/artifact/KVvUYFTNCdRFRmYTSTtXev). The relay's console (`/compte/…`) becomes an app: a **fixed side menu** and a **content column of constant width** (920 px), whatever the page; on a phone (under 880 px), the menu becomes a **tab bar at the bottom**.

- **Menu**: “Computers” (with their count), “People”, “Families and teams” (then each one, with its initial in its color), “Account and security”; at the bottom, “Getting started” (steps done out of 4) and the signed-in person, with “Sign out”. The logo is KyberNow Share's.
- **Family or team**: an organization now has a **kind**, chosen when it's created (`kind`: `family` or `team`, migration 6; earlier organizations are teams). Both have the same rights, roles and groups; the words and examples change (“The Martins”, “The studio”), a family is amber, a team sky blue, and only a team has the “Deploy” tab (enrollment tokens). “Families and teams” explains the principle with a diagram: people, gathered in a group, open computers.
- **Rights, in everyday words**: Session becomes “Open the desktop”, Lending “Borrow to play”, Sharing “Join your shares”, Administer stays. Each right has a sentence and an example (“Your nephew playing on Wednesday afternoons”); they are ticked as tiles. The technical names (`session`, `lend`, `share`, `admin`) don't change.
- **Pages**: “Computers” (cards, “Attach a computer”, “Opened for you”, a section per family or team); a computer's page (“Who has access”, “What each right allows”, “Settings”); “People” (`/compte/personnes`: everyone you opened your computers to); “Families and teams” (`/compte/familles`); a family's or team's page (Members, Groups, Computers, Log, Deploy tabs); “Account and security” (`/compte`).
- **Panels**: “Add a person”, “Open to a family or team”, “Invite a member”, “Create a group”, “New token” and “Attach a computer” open in a panel on the right, through the address anchor (`#ajouter`), without script; a refused form reopens its panel.
- **Welcome tour** (`/compte/bienvenue?etape=0…3`): full screen, one idea per screen with its illustration: welcome, protect your account (passkey, or two-factor authentication), attach your computer (the page reloads every 5 s while none is attached), open it to a family or team. It opens after the address is verified, and the first time “Computers” opens; “Skip” or the end mark it seen (`welcomed_at`, migration 6). A passkey added from the tour brings you back to the next step (`suite=bienvenue`).
- **“Getting started”**: attach a computer, protect your account, open a computer to someone, create your family or team; the card disappears once all four are done.
- **Transitions**: pages go from one to the next with the browser's transitions (`@view-transition`): the menu stays, the content slides; the tour's steps slide from right to left. `prefers-reduced-motion` turns them off. Still no script, except `passkey.js`.
- **After signing in**, the space opens on “Computers” (`/compte/ordinateurs`), no longer on the account page.
- **Seeing the pages without a relay**: `KN_PAGES_DIR=<folder> cargo test ecrire_les_pages -- --ignored` (in `server/relay`) writes every page, in French and English, with the relay's files; `python3 -m http.server` serves them.

## Admin from KyberNow Share

Decided with the author on 11/25/2026; **reverses a scoping decision** (“Not the whole Admin interface through the relay”, decisions table): with a new switch, off at the start, **all of Admin** opens from anywhere, through the relay, for an account with the Administer right. Approved and built on 11/25/2026 (tunnel, relay, Admin, Windows and Mac icon); not yet tried on a real Station.

### What the user sees

- **The “Open Admin” link**, for whoever has the Administer right (owner, or owner and administrator of the family or team, with two-factor authentication): in the header of a computer's page (button), on its card in “Computers” (small “Admin” shortcut), and on each row of a family's or team's “Computers” tab.
- **One click, already signed in**: the link leads to “Open {computer}'s Admin?”, then “Confirm with a passkey”; Admin opens, signed in, without its password.
- **Where it works**:
  - “Remote Admin” on at the Station: everywhere, through `https://<id>.station.share.kybernow.app`, encrypted end to end to the Station (0.11.8, relais.en.md, “Browser encrypted end to end”); through `https://<id>.share.kybernow.app`, decrypted by the relay, with an older Station or relay;
  - off: only on the computer's network, through its local address; the page says so (“Remote Admin is off on {computer}: this link only works at home, on the same network. Turn it on in Admin › Away from home.”) and still offers “Open on the network”.
  - Station offline: the link is greyed out, “Offline”.
- **Switch in Admin**, Away from home: “Remote Admin” (“This computer's administrators open Admin from KyberNow Share, from anywhere.”), **off at the start**; it needs “Access away from home” on and the Station attached to an account. It can only be turned on **on site** (Admin session on the network); it can be turned off from anywhere.
- **Warnings at each remote opening**:
  - the computer says so: on Mac, a notification “Admin is open remotely by {name}”; on Windows, the icon's panel opens with the same line (the icon has no notifications); and Admin shows a banner “Admin opened remotely, through KyberNow Share.” to whoever opened it that way;
  - a line in Admin's feed and, for a family's or team's computer, in its log;
  - an email to the owners (and to a family's or team's administrators), when the relay sends emails: “{name} opened {computer}'s Admin on {date}, through KyberNow Share.” (no place: the relay doesn't locate addresses; the email's reason says “owner” or “you manage the family or team”);
  - the Admin session opened through the relay **closes after 30 minutes of inactivity** (and 8 hours at most).
- **Passkey required** remotely: each opening must be confirmed with a passkey, even when already signed in. An account without a passkey sees “Add a passkey to your account first: it protects opening Admin remotely.” On the network, the rule stays today's (two-factor authentication done).
- **All of Admin** remotely: the same pages and settings as on the network, including Admin's password, the ports, the certificate, the attached account and uninstalling. One common-sense exception: “Remote Admin” can't be turned on remotely.

### How

- **Local address**: the Station gives the relay, in `hello` (and `admin_status` when they change), its Admin addresses on the network (`admin_lan`: `["https://192.168.1.20:8443", "https://pc-salon.local:8443"]`) and `remote_admin` (the switch). The relay keeps them **in memory only**, for the connected Station, and shows them only to an account that manages it (`/v1/computers` carries them for those accounts; the console too). No database, no log.
- **Start**: the console leads to `GET /compte/admin?id=<station>` (relay): it checks the Administer right and two-factor authentication, then sends to Admin's start:
  - remotely: `https://<id>.station.share.kybernow.app/api/login/kybernow/start` when the Station holds its end-to-end certificate, otherwise `https://<id>.share.kybernow.app/api/login/kybernow/start`;
  - on the network: `<admin_lan>/api/login/kybernow/start`.
- **Admin**: `GET /api/login/kybernow/start` (new) draws the `nonce` (cookie `kyber_admin_kn`, or `__Host-kyber_admin_kn` through the relay) and sends to `https://<relay>/compte/ouvrir-admin?station=…&nonce=…&retour=<its origin>`, like today's “Sign in with KyberNow” button.
- **Relay, `/compte/ouvrir-admin`**: `retour` is either a local network address (today's rule), or **exactly** `https://<id>.<relay domain>[:public port]` or `https://<id>.<tls_zone>` (end to end, 0.11.8) of this Station. For the latter: Station connected with `remote_admin` true, and a **passkey assertion** on the confirmation page (challenge bound to the browser, as for “Delete the account”). The token issued also carries `via: "relay"` (2 minutes since migration 8, single use, `jti` = `nonce`). To the end-to-end origin, it goes in the fragment (`/api/login/kybernow#token=…`), never sent to the server; to the old one, as `?token=` (Admin 0.11.7).
- **Admin, through the relay** (`Via::Relay { kind: browser_tls }`, or `admin_http` as long as the Station has never been end to end with this relay): the guard opens **all** of the Admin session's routes to a session opened by `/api/login/kybernow` **through the relay** with a `via: "relay"` token, `remote_admin` on, Administer right. Otherwise nothing changes (“Admin's guard by path”, relais.md). Session cookie through the relay: `__Host-kyber_admin_session` (`Secure`, `HttpOnly`, `SameSite=Strict`, no `Domain`). Inactivity: 30 minutes; 8 hours at most. Turning “Remote Admin” off closes these sessions at once; detaching the Station too. A session is only valid on the path it was opened on (end to end, or decrypted); a Station that became end to end closes those opened in the clear, and then refuses any “Remote Admin” in the clear (`403 remote_admin_e2e`); suspended (relay no longer passing TLS through, unknown certificate in the public logs): `403 remote_admin_suspended`, until the host turns it on again on site.
- **Exception**: `PUT /api/relay` that would turn `remote_admin` on is refused through the relay (`403 network_only`).
- **Warnings**: Admin records `admin_distance_ouvert {account, name}` in its feed, shows the notification through the icon (`/api/local/state` carries `remote_admin_sessions`), and tells the relay `admin_opened {account}`; the relay writes to the family's or team's log and sends the email (`Message::AdminOpened`), at most one per account per hour.
- **Trust**: since 0.11.8, the browser is encrypted end to end to the Station (relais.en.md, “Browser encrypted end to end”): the relay no longer sees Admin's pages. It still signs the `via: "relay"` token: a compromised relay could still open a session itself (relais.en.md, “Later”). With an older relay or Station, the relay decrypts, and Admin says so under the switch.

### Texts

See the French table above (both languages).

### Tests to plan

Relay: another Station's `retour` refused; `remote_admin` false, no passkey or another account's assertion refused; local addresses shown only to managers. Admin: without a `via: "relay"` token, nothing more through the relay than today; remote session closed at 30 minutes, when the switch is turned off, on detaching; `remote_admin` can't be turned on through the relay. On the real Windows and Mac Station: notification, email, log.

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
- The signed-in device asks the relay for a token for a computer: `POST /v1/access {station}` with its device session. The relay checks the account, its rights on that computer, the relay-side switch, and returns a token **valid for 2 minutes** (5 until 25/11/2026, see “Sessions”): `{v, relay, station, account, device, rights, name, iat, exp, jti}`, signed.
- The device presents it to Admin (`Authorization: KyberNow <jeton>`), on the network or through the relay. Admin checks: signature (pinned key), `station` = its own, `exp`, switch on, account and device not blocked locally. From it, Admin derives the rights.
- **Session**: Admin opens the session as for a paired device, but Kyber is reached with a **Kyber token** (like share guests), not with a Kyber account: no account to create, no Kyber restart, and revocation on the relay applies at the next token (the heartbeat, every 5 seconds, returns a fresh one; the access token is asked again every 90 seconds).
- The relay still does not see the picture or the input: the tunnel stays end to end with Admin pinned.

### The Station's four safeguards

1. **Switch** in Admin, “Accept KyberNow accounts”: when off, Admin refuses any token (paired devices, invitations and the local network work as before).
2. **Local list**: Admin keeps each account and device that came in with a token (name, email, rights, last visit) and can **block** one, without going through the relay; an account blocked here is blocked even if the relay says yes.
3. **Short, signed tokens**: 2 minutes (a Station accepts up to 5, older relay), relay key pinned, `jti` never replayed.
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
| Relay | `POST /v1/access {station}` | Access token (2 min) |
| Relay | `attach_request` → `attach_code`, then `attached`; `attach_status`; `detach` / `detached` (Station, through its control connection) | Attachment, after approval in the console (`/compte/rattacher?code=…`); detaching on both sides |
| Admin | `GET/PUT /api/account` (Admin session) | Attached account, switch, detach |
| Admin | `POST /api/account/attach` (Admin session) | “Sign in”: attach code asked of the relay, console page to open |
| Admin | `GET /api/local/account`, `PUT /api/local/account/relay`, `POST`/`DELETE /api/local/account/attach`, `POST /api/local/account/enroll` (local token; to set the relay, attach and enroll, also the installer token, `x-kyber-installer`, read from `installer-token` in the data folder, which only administrators can read) | The installer: chosen relay, attaching during installation, enrollment of a silent installation (“Installers”) |
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
- **Verification by the Station**, in this order: pinned key (one of the keys from the attachment), version, `relay` = the attachment's relay, `station` = its own, `exp - iat` of 5 minutes at most (`MAX_LIFETIME_SECS`, independent of the lifetime of the tokens issued, `LIFETIME_SECS` = 2 minutes: an up-to-date Station still accepts an older relay's tokens), `iat` not in the future and `exp` not past, within **60 s** (clocks), then the `jti`.
- **`jti` never replayed**: the same token serves all of its device's calls during its 2 minutes (session, heartbeat), but a `jti` is never accepted for different content; to restart or update the Station, the token is **single use** (once consumed, it is good for nothing else). `jti`s are forgotten when they expire.
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
  - **Session**: the `/api/stream/*` routes and the host's (share, lending) like a paired device; `prepare` also returns `kyber_token` `{token, user, expires}`, the Kyber token (2 min, the access token's lifetime) under the `kn-account-…` account, never written to Kyber's configuration; the heartbeat returns a fresh one (batch 3), which Kyber reads again if it reconnects;
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
- **Device sessions**: separate from browser sessions; 256-bit bearer token (`Authorization: Bearer …`), kept hashed; **90 days unused** (`devices.seen_at`, noted at most every 5 minutes; `DEVICE_IDLE_SECS`) and it is refused like a signed-out session (`401 bad_session`, the apps say “This account was signed out of this device: sign in again.”), erased, `appareil_expire` in the account's log; the relay's sweep also erases those no longer shown; “Connected devices” on the account page (“Used {duration} ago”, the 90-day help, “Disconnect”); a new password closes them all, and so does deleting the account (its Stations are then detached and receive `detached`). System window: `retour` is `kybernow://compte` (nothing else), `etat` is given back as is; after “Connect”, a page sends back to `kybernow://compte?code=…&etat=…` (single-use code, 5 minutes, exchanged at `POST /v1/devices/token`). Code: eight characters `XXXX-XXXX` (no vowel, no 0, 1), 10 minutes, polled every 5 seconds (`202` pending, `200` the session, `403 denied`, `410 expired`); 20 codes asked and 10 wrong codes typed per address in 15 minutes.
- **Access token**: `GET /v1/computers` returns `{computers:[{station, name, rights, owner, mine, online, attached_at}]}`, and for a connected Station its Admin fingerprint signed by it (`station_key`, `admin_fp`, `sig`, like `GET /v1/codes/{code}`, batch 3); `POST /v1/access {station}` returns `{token, station, rights, expires, lifetime}`, `404 unknown_station` for a Station not in the account, `401 bad_session` without a device session. In batch 2, an account's computers are the ones it owns, with all rights; batches 4 and 5 add those opened to it and those of its organizations (“People, rights and organizations”).
- **Notice for each new device**: a device's first token for a Station (`device_stations` table) sends an email to the owner (“New device on {computer}”) and adds a `nouvel_appareil` line to the log.
- **Database**: migration 3 (`stations`, `devices`, `device_codes`, `device_stations`); migration 2 is batch 1b's.
- **Code**: `server/relay/src/accounts/stations.rs` (key, attaching), `devices.rs` (devices, API, tokens), `web/attach_pages.rs` and `web/device_pages.rs` (pages); the hub (`hub.rs`) forwards the messages.

### People, rights and organizations (batches 4 and 5)

Done on the relay side (console, API, database) and on the Admin side; the apps are described in “Apps (batch 3)”, the installers' account step in “Installers”.

- **An account's rights on a computer**: all of them for its owner (a person), and for the Owners and Administrators of the owning organization; otherwise the union of the **given rights** not expired, to them, to an organization they are a member of, or to one of its groups. **Administer only counts with two-factor authentication** (active TOTP or a passkey, which is worth it), owner included: without it, the token and `/v1/computers` don't have `admin`, and the “People” page says so. Likewise, an organization Owner or Administrator without two-factor doesn't manage the organization and only has the given rights, like a member.
- **Managing a computer's access** (“People”): its owner, the Owners and Administrators of the owning organization, and anyone with the (effective) Administer right.
- **Adding a person**: their address, their rights, an optional end date (the end of the chosen day in the browser's time zone, through `assets/tz.js`; UTC without it: REL-29). With a verified account, an invitation they accept (or decline) in their space, “Invitations” page (27/11 audit, REL-11); someone who already has access sees their rights change; without an account, it waits for the address and moves to the account **when it is verified** (sign-up, forgotten-password link, or account created by the relay's administrator). Adding the same address again changes their rights. An email says so (“{name} opened {computer} for you”), with the console or the sign-up page.
- **Opening to an organization**: a computer (a person's or an organization's) opens to a whole organization one is a member of, or to one of its groups, with rights; its log records it.
- **Organization**: creating one requires two-factor authentication (the creator is its Owner). Inviting a member by address (verified account: an invitation they accept or decline in their space, “Invitations” page, email; no account: pending invitation, taken when the address is verified; the answer is the same in both cases, “Invitation sent”). Only an Owner names or removes an Owner; an organization always keeps one (“Leave” and changing the last one's role are refused). A deleted account: its organization gets another Owner (the oldest Administrator, otherwise the oldest member), or goes away with its computers if nobody is left.
- **Organization log**: every change (members, roles, invitations, groups, rights given and removed, computers attached, detached, enrolled, tokens) and every opening of one of its computers (who, which device, from where; an opening: a first token after an hour without), and every new device. IP addresses forgotten after 7 days, lines after 12 months; seen by Owners and Administrators only.
- **New-device notice**: for an added person's device, the email goes to the owner and names the person (“with Léa's KyberNow account”); for an organization's computer, to the Owners and Administrators, with a log line.
- **Attaching**: the “Attach {name}” page offers “Me” and the organizations the account manages (Owner or Administrator, two-factor); `attached` then carries `owner {kind: "organization", id: "org_…", name}`. Attaching to another owner erases the given rights and first visits.
- **Enrollment tokens** (“Deploy Stations”): a name, a duration (1 to 365 days), a number of Stations (1 to 10,000), and rights given to a group on each enrolled Station; `kne_` and 32 characters (160 bits), **shown only once**, kept hashed. The Station presents it through its control connection (`enroll`); the relay attaches it to the organization (same answer if it already is, without counting one more Station), gives the rights to the group, writes the log. Revoking prevents new enrollments; enrolled Stations stay.
- **Admin on the network** (“Sign in with KyberNow”): Admin's sign-in page offers the button when the Station is attached and accepts accounts, **never through the relay**. Admin draws a `nonce` (16 bytes, 10 minutes, `kyber_admin_kn` cookie, `SameSite=Lax`) and opens `https://<relay>/compte/ouvrir-admin?station=…&nonce=…&retour=<Admin's address>`. Signed in, with the Administer right (two-factor), the person confirms “Open Admin of {computer}?”; the relay issues an access token (2 min, device `web`) whose **`jti` is the `nonce`**, and sends back to `<retour>/api/login/kybernow?token=…` (published relays) or, better, `<retour>/api/login/kybernow#token=…`: since 0.11.8, Admin then serves a small page that reads the fragment (never sent to the server nor kept in history or logs), erases it and posts it as a form to `POST /api/login/kybernow/return` (`token=…`, without Admin's header: a navigation); a relay may also post that form itself. `retour` is only a local network address (private, link-local or loopback IP, dotless name, `.local`, `.lan`, `.home.arpa`), without a path; when the Station gave its Admin addresses, an IP address is only accepted among them (or loopback), and a name only with their scheme. Admin checks the token (Administer right, **single use**) and that its `jti` is this browser's `nonce`, then opens its session; a line in the feed.
- **Lending with an account**: `POST /api/lend/status` and `/api/lend/borrow` accept `Authorization: KyberNow <token>` with the Lending right instead of a friend account's login and password; borrowing happens under the account's name (no time slots: they stay those of friend accounts; Steam: “Their Steam account”). The allowed apps stay those of the “Lending” page.
- **Enrollment on the Admin side**: two ways in, a single `enroll` message to the relay. The silent installer hands the token over through the local route `POST /api/local/account/enroll` (answered at once, for its exit code: “Installers”); a deployment tool can also write `[account] enroll = "kne_…"` in `kyber-admin.toml` (with the relay in `[relay]`): Admin presents it as soon as the relay is reached, takes `attached` like an asked attachment (accounts accepted, access away from home on), then erases it; refused for good (`enroll_refused`): a line in the feed and the token is erased; relay unreachable: new try, further and further apart (10 min at most).
- **Access links** (relay): `kla_` and 32 characters (160 bits), kept as a hash with its start (`kla_7qm2x…`) for the list; shown only once, on the page that follows its creation. At most ten pending per computer. Accepting is one transaction: the link is marked used only if it is still pending, then its rights are added to the account's rights on the computer (union; each right keeps its end date, the furthest for a right already there, “no end” wins; a right that already ended is replaced); two people accepting at the same time: only one gets it, the other reads “This link was already used.”. Administer, given by a link, needs two-factor authentication like any given right (the link's page says so before “Accept”). Implementation details: “It's your computer: this link is for someone else.” also applies to the Owners and Administrators of the owning family or team and to whoever created the link; a link whose author no longer manages the computer's access stops working (“This link was revoked.”); “Revoke” is offered for a pending link or one already used (with no effect on the access); links go away when the computer is detached or attached to another owner, and a closed link (used, revoked, expired) is forgotten after 30 days. Opened while signed out, the link is remembered for an hour (signed cookie `__Host-kn_suite`): after signing in, or after verifying the address of a brand new account, you come back to it (the remembered page comes before the welcome tour). Unknown links are limited like codes (5 per IP address in 15 minutes); the link is never written to the logs. Log: `lien_cree`, `lien_utilise` (who), `lien_revoque` in the owning family's or team's log; in the account's log, `lien_cree`, `lien_revoque`, `lien_utilise` (for the author) and `lien_accepte` (for whoever accepts it).
- **Computer groups** (relay): the rights given on a group (to the whole family or team, to one of its groups of people, to one of its members) count in the only two rights queries (`granted_rights` and `visible_stations`, `people.rs`), for each computer in the group at that moment, on top of the rights given computer by computer; the end date is respected. A right given to a member only counts while they are a member. Only the family's or team's computers go into its groups; a computer that is detached, or attached to another owner, leaves them. An enrollment token's “File in the group” files each enrolled Station in that group (`enroll`). Log: `groupe_ordinateurs_cree`, `_supprime`, `_ajoute`, `_retire` (a computer), `_ouvert`, `_ferme` (a given right), and `station_enrolee` names the group.

#### Console pages

| Route | Screen |
|---|---|
| `GET /compte/ordinateurs` | “Computers”: “Yours”, “Opened for you”, one section per organization; “Create an organization” |
| `GET /compte/ordinateur?id=…` | A computer's “People”: rights per person (Session, Lending, Sharing, Administer), “Add a person”, “Open to an organization”, “Remove”, “Detach this computer”; only one's own rights for whoever doesn't manage |
| `POST /compte/ordinateur/ajouter` | `id`, `email`, `session`/`lend`/`share`/`admin` (`1`), `fin` (`yyyy-mm-dd` or `dd/mm/yyyy`, optional) |
| `POST /compte/ordinateur/ouvrir` | `id`, `a` (`org_…`, or `org_…:<group>`), rights |
| `POST /compte/ordinateur/retirer` | `id`, `acces` |
| `POST /compte/ordinateur/renvoyer` | “Resend the invitation” of an address: `id`, `acces`; once an hour; if the address has a verified account by now, the computer goes to it |
| `GET/POST /compte/organisations/creer` | `nom`; two-factor required |
| `GET /compte/organisation?id=org_…&vue=membres\|groupes\|ordinateurs\|journal` | “Organization”: members and roles, pending invitations, groups, computers and their access, log |
| `POST /compte/organisation/inviter`, `/role`, `/retirer`, `/quitter`, `/renommer`, `/renvoyer` | `email` and `role`; `compte` and `role`; `compte` or `invitation`; nothing; `nom`; `invitation` (“Resend the invitation”, once an hour) |
| `POST /compte/organisation/groupes/creer`, `/supprimer`, `/membre` | `nom`; `groupe`; `groupe`, `compte`, `action` (`ajouter`, `retirer`) |
| `GET /compte/organisation/deployer?id=…`; `POST /compte/organisation/jetons/creer`, `/revoquer` | “Deploy Stations”; `nom`, `jours`, `nombre`, `groupe`, rights, `groupe_ordinateurs` (“File in the group”, optional); `jeton` |
| `POST /compte/ordinateur/liens/creer`, `/revoquer` | “Access links” on a computer's page (whoever manages its access; `#lien` and `#lien-<n>` panels to confirm): `id`, rights, `jours` (`1`, `7`, `30`), `fin` (optional); `id`, `lien` |
| `GET/POST /compte/lien/kla_…` | “{name} opens {computer} to you”: signed out, “Sign in” and “Create an account” (then back here); signed in, the rights, the end date and “Accept” (the POST); the refusals |
| `POST /compte/organisation/groupes-ordinateurs/creer`, `/supprimer`, `/ordinateur`, `/ouvrir`, `/retirer` | “Computers” tab (Owners and Administrators; `#groupe-ordinateurs`, `#ordinateurs-<n>`, `#supprimer-ordinateurs-<n>` panels): `nom`; `groupe`; `groupe`, `ordinateur`, `action` (`ajouter`, `retirer`); `groupe`, `a` (`org`, `grp:<n>`, `compte:<n>`), rights, `fin`; `groupe`, `acces` |
| `GET/POST /compte/ouvrir-admin` | “Open Admin of {computer}?” (`station`, `nonce`, `retour`) |
| `POST /compte/ordinateur/deplacer` | “Move…”: `id`, `vers` (`moi`, `org_…`) → “What changes”; `action=deplacer`, `garder` (`1`) → the move (see “Synced settings and moving a computer (migration 9)”) |

The console bar (“Computers”, each organization, “Account”) is also at the top of `/compte`. Detaching from “People” goes back to “Computers”.

**Invitations, adding members** (see “Invitations, adding members and sessions” above): an invitation (`grants` with `email`, `org_invites`) expires `INVITE_SECS` (30 days) after `sent_at`; `claim_invitations(account, now)` only takes the ones still valid and leaves the others; adding (or replacing) an invitation sets `sent_at` to now; so does “Resend the invitation”, which also notes `resent_at` (`RESEND_GAP_SECS`, one hour, between two resends). Giving a right to an account (`grant_person`, `set_member`) removes an invitation of its address at the same place. Log: `invitation_renvoyee` in the account's log, and in the family's or team's (the computer's owner, or the invitation's). The suggestions (`known_people`, 200 at most): the members of the account's families and teams (and their pending invitations), the people and addresses added to its computers or by it; never anyone else. “Open a group” to a member is chosen from a list (`compte:<n>`), with no address to type: no suggestions there. “People” (`/compte/personnes`) groups by account (an invitation whose address has a verified account goes with it), by address otherwise, by family, team or group; each line names its computers and the union of its rights. Suggestions read “First Last · address” (the pages' middle dot: they don't use the em dash).

#### API and messages

- `GET /v1/computers`: one's own, those opened to them (to them, their organizations, their groups), and all those of the organizations they manage; `owner` is `{kind: "person", id: "acc_…", name}` or `{kind: "organization", id: "org_…", name}`, `mine` says whether they own it, `rights` their rights at this moment. `POST /v1/access`: the same rights, `404 unknown_station` without any right.
- Control connection: `enroll {req, token, name}` (Station → relay); answer `attached {req, …}`, or `error {req, code: "enroll_refused", message}` with `unknown`, `expired`, `revoked` or `used_up`; relay without accounts: `no_accounts`. The `tunnel` crate carries it (`Agent::enroll`).
- Admin: `POST /api/login/kybernow {origin, lang}` → `{url, expires}` (no session, `403 network_only` through the relay, `409 unavailable` without attachment or with the switch off); `GET /api/login/kybernow?token=…` → Admin's session (a page that goes back home) or the refusal; `GET /api/session` says `kybernow_login`.

#### Database

- Migration 4: `grants` (Station, `account` or an invitation's `email`, `rights` as words `session,share`, `expires_at`, `invited_by`, `created_at`); `device_stations.seen_at` (last visit, for the log).
- Migration 5: `organizations` (public identifier `org_…`, name, plan), `org_members` (role `owner`, `admin`, `member`), `org_invites`, `org_groups`, `group_members`, `org_log` (who, what as JSON, address, when), `enroll_tokens` (hash, start of the token, number, used, group and rights, expiry, revocation); `stations` rebuilt (`owner_kind` `person` or `organization`, `account` or `org`, `enrolled_by`); `grants.org` and `grants.grp`.
- Migration 8: `grants.sent_at`, `grants.resent_at`, `org_invites.sent_at`, `org_invites.resent_at` (sending invitations). Invitations already pending get the migration's time: a full 30 days from the relay's update, rather than a sudden expiry of those sent more than a month ago.
- Migration 7: `access_links` (Station, hash, start of the link, rights, `link_expires_at`, `access_expires_at`, `created_by`, `created_at`, `used_by`, `used_at`, `revoked_at`); `station_groups` (organization, name unique within the organization), `station_group_members` (group, Station); `group_grants` (computer group, and `account`, `org` or `grp`, rights, end date, `invited_by`): a separate table, `grants` stays “the rights given on a Station”; `enroll_tokens.sgroup` (“File in the group”). No foreign key to `stations` (like `grants`): detaching or attaching to another owner erases the Station's rows.
- Code: `server/relay/src/accounts/people.rs` (given rights, an account's rights, invitations and their sending, migration 8, suggestions), `orgs.rs` (organizations, log, tokens, `enroll`), `links.rs` (access links, migration 7), `station_groups.rs` (computer groups), `console_pages.rs` (pages), `web/console.rs`, `web/org_pages.rs` (and computer groups), `web/link_pages.rs` (access links), `web/admin_login.rs`; tests: `web/tests/lot6.rs`, `web/tests/invitations.rs`; Admin: `accounts.rs` (sign in with KyberNow, enrollment), `lend_api.rs` (lending with an account); interface: `server/web/src/pages/Login.svelte`.

### Synced settings and moving a computer (migration 9)

Done on the relay side on 11/25/2026 (the core and the apps read and write the settings on their side).

- **Database**: migration 9, `computer_settings` (`account` → `accounts` with `ON DELETE CASCADE`, `station`, `"values"`: the JSON as sent, `updated_at`; key `(account, station)`, index on `station`). No foreign key to `stations`, like `grants`.
- **Device API** (device session, `Authorization: Bearer …`, like `/v1/computers`):
  - `GET /v1/settings` → `200 {"settings":[{"station":"<id>","updated_at":<unix>,"values":{…}}]}`, only the computers the account has rights on at that moment (those of `/v1/computers`); the other rows are deleted on that read.
  - `PUT /v1/settings/<station>` `{"values":{…}}` → `200 {"updated_at":<unix>}`; the last one wins; values are kept as given (an unknown key stays, for newer apps). `404 {"error":"unknown_station"}` without a right (or an unreadable id), `400 {"error":"bad_values"}` if `values` isn't an object, `413 {"error":"too_large"}` above 4096 bytes once serialized, `401 bad_session` without a session, `405` for another method.
  - Known keys: `profile` (text, or `null` for Auto), `codec` (text), `yuv444` (boolean), `mbps` (number), `hdr` (boolean), `immersive` (`"fullscreen"`, `"always"`, `"never"`).
- **Cleanup**: with the account (foreign key; and other accounts' rows on its computers, or on those of a family or team that goes with it); all rows of a computer detached (console or Admin) or attached to another owner; after a move, those of the accounts that no longer have rights on it; on read (`GET`) and at the relay's periodic cleanup (every 30 s), those of a computer the account no longer sees. The data export carries them (`synced_settings`).
- **Moving**: `POST /compte/ordinateur/deplacer` (`id`, `vers` = `moi` or `org_…`) renders “What changes”; with `action=deplacer` (and `garder=1`, the box) it moves. The `#deplacer` panel of a computer's page picks the destination: “Me” (unless it's already yours) and the families and teams one is Owner or Administrator of, except the current owner. “Move…” is offered to its owner and to the Owners and Administrators of its family or team (not to someone who only has the Administer right); without two-factor authentication, the refusal says so. On Admin's side (`accounts::on_attached`), an unrequested `attached` is taken when the relay is the same and its keys are the ones already pinned: only the displayed owner changes.
- **What goes** (one transaction, `moves.rs`): rights given to families, teams or groups, **except those given to the destination or one of its groups** (they still make sense there); its computer groups (and the access that came from them); pending links (revoked, they stay in the list); rights given to people and invitations if the box is unchecked; the right given to the new owner (a person), who now has everything; the devices' first visits (the new owner is told about each device, as after an attachment). `enrolled_by` is cleared. The preview is computed from the same data.
- **Logs**: `ordinateur_deplace {station, from, to}` in the log of the old and the new family or team; `ordinateur_deplace` in the account log of whoever moves (the person on one side is them).
- **The Station**: the relay sends it `attached` (without `req`) with the new `owner`, as after “Attach”. **To do in Admin**: today `on_attached` ignores an unrequested `attached` whose owner changes (only the same owner's keys and names follow); for Admin to show the new owner, it must accept it when the relay is the same and the keys are those already pinned. The access token doesn't depend on the owner: access works without this change.
- **Code**: `server/relay/src/accounts/settings.rs` (migration 9, API), `moves.rs` (preview, move), `web/move_pages.rs`, `console_pages.rs` (`move_preview`, `#deplacer` panel); tests: `web/tests/deplacer.rs`.

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
  - the **whole account**, as the app keeps it itself (`apps/windows/src/store.rs`, `save_account`): generic credential in the Windows Credential Manager, target `KyberNowCompte:<relay>|<account id>` (`<relay>` in its normal form: `host`, or `host:port` other than 443, `#sha256:…` for a self-signed relay; the core's `Account::key`), user the account's email, secret the JSON `{"relay","token","account":{"id","name","email"},"device":{"id","name"}}` in UTF-8 (`kybernow_core::account::Account`), `LOCAL_MACHINE` persistence; an account already there for the same relay and account is replaced. Nothing in a file (`%APPDATA%\KyberNow\accounts.json` is the app's own list of the accounts' computers, with nothing secret).

  The app reads its accounts at start-up and starts signed in. “Create an account” opens the site's sign-up in the window, then “Account created: sign in”. “Later”: the app signs in from its home screen. Uninstalling while erasing data also forgets the accounts (`--forget-all` removes the `KyberNow:*` passwords and, apart, the `KyberNowCompte:*` accounts).

Gaps of this step:

- the “Me” / “{organization}” choice and the computer's name are not installer screens: it's the relay's attach page, shown in its window;
- a self-signed relay's page opens in the browser, not in the window;
- the silent installation takes the default options (no `/user=` or `/password=`, which would show in the process list);
- on the Mac, the step must be finished (attached or “Later”) before permissions: their end restarts Admin, which would forget an attachment in progress.

### Apps (batch 3)

- **Core** (`apps/core/src/account.rs`): everything that talks to the relay for the apps, testable without Kyber.
  - **System window**: `sign_in(relay, device name)` returns `https://<relay>/compte/connexion?appareil=…&retour=kybernow://compte&etat=…` (`etat`: 16 random bytes); `finish_sign_in` reads the return (`etat` checked, `erreur=annule`: cancelled) and exchanges the code (`POST /v1/devices/token`).
  - **Code**: `tv_code` (`POST /v1/devices/code`, with `url_label` to write and `issued_at` for “The code changes in {n} min.”), `tv_poll` (`202` pending, `200` the account, `403`, `410`), `tv_wait` (one request every 5 s at least, silent relay tolerated, cancellable).
  - **Account** (`Account {relay, token, account, device}`): kept **whole** by the app in its system's vault (Keychain without iCloud sync, Android Keystore, Windows Credential Manager, private file or Secret Service on Linux); `Debug` hides the token, `summary()` leaves it out; key `relay|account` (several accounts, several relays). `sign_out`: `POST /v1/devices/revoke` (`401`: already done).
  - **Computers**: `computers` (`GET /v1/computers`); the Admin fingerprint is kept only if the Station's signature holds (key ↔ identifier); `remember_fingerprints` keeps the ones already seen (the relay only gives them for a connected Station); `merge`: one computer once (same Admin fingerprint or same Station at the relay; the paired one stays, with the account's badge); `origin`: the badge (“Account”, “Account · {relay}”, the organization, “Opened by {name}”, “Paired”). `AccountComputer::route`: the route through the relay, Admin pinned.
  - **Access token**: `StationAccess` asks `POST /v1/access` and keeps the token until 30 seconds before it expires (`EXPIRY_MARGIN`; 90 seconds for a 2-minute token, 4 minutes at most with an older relay; `expires` counts too, as the device may have slept); `single_use` to restart or update the Station (Admin consumes it); `invalidate` after a refusal.
  - **Admin**: `Console::account(host, port, access, fingerprint)` carries `Authorization: KyberNow <token>` on each call, on the network as through the relay (`with_relay`); `401 token_expired` or `bad_token`: fresh token and a second try (info, prepare, heartbeat, HDR, consoles); without a token, `ConsoleError::Account`.
  - **Session**: `SessionParams::account` (`{"account": …, "station": …}` in the bridges' JSON); the access token is asked before probing the network; Kyber is reached with the Kyber token from `prepare` (`kyber_token`), never a Basic account; the heartbeat returns a fresh one (Admin), which Kyber reads again when it reconnects.
  - **Errors** (`AccountError`, bridge codes 70 to 80): device session closed (the app forgets the account), computer removed from the account, code declined or expired, return refused, cancelled, too many attempts, relay without accounts, unreadable address, silent relay, the rest.
- **Bridges**: `apps/apple/src/account.rs` (`kn_account_*`, `kn_console_new_account`, `KN_ACCOUNT_*`) and `apps/android/jni/src/account.rs` (`Native.account*`, `consoleNewAccount`); the Windows, Linux and webOS apps call the core directly.
- **Texts**: [link-ui.en.md](link-ui.en.md), “KyberNow accounts”.

### Data kept by the relay

| Data | Why | Duration |
|---|---|---|
| Email, Argon2 hash of the password, TOTP secret (encrypted), passkeys (public keys) | Signing in | Lifetime of the account |
| Signed-in devices (name, kind, last visit) | Device sessions, revocation | Until signed out, or 90 days unused |
| Attached Stations (identifier, name) | The account's computers | Until detached |
| Given rights (person or invited address, rights, end date) | Added people | Until removed or detached; an expired invitation goes after 7 days |
| Organizations, members, invitations, groups, rights | Fleet | Lifetime of the organization |
| Enrollment tokens (hash, start, number) | Deploying a fleet | Lifetime of the organization |
| Synced settings (per account and per computer, 4 KB at most) | Settings that follow the person | As long as the account has rights on the computer |
| Log (accesses, changes; organization log) | Security, fleet | 12 months (IP addresses: 7 days) |
| IP addresses | Security, limits | A few days (logs), as today |

Deleted account: erased within 30 days (backups included). Data export from the console.

### Emails

SMTP set in `relay.toml` (host, port, credentials, sender); without SMTP, a self-hosted relay has neither email verification nor forgotten password (the console says so, the relay's administrator creates the accounts). Messages in French and English: verification, forgotten password, new device, added to a computer or an organization, two-factor authentication changed, passkey added, account deleted, Admin opened remotely.

**Layout** (`server/relay/src/accounts/mail.rs`): each email goes out as HTML with its plain-text version, both rendered from the same content (a heading, paragraphs, a box for the detail or the code, a button, a "Wasn't you?" note and a footer saying why the email arrives). Light brand look: `#F2F5F6` background, white card, `#5340E6` signal button, system fonts. Tables and inline styles for Outlook and webmail; PNG logo (`/assets/kybernow-share-mail.png`, unversioned so that emails already sent keep it), since Gmail and Outlook don't show SVG. With SendGrid, the `X-SMTPAPI` header turns off click and open tracking: links stay the relay's own. To look at them: `KN_PAGES_DIR=… cargo test ecrire_les_e_mails -- --ignored`.

### Relay security

- Passwords: Argon2id; at least 10 characters; list of known passwords refused.
- Console: `__Host-` session cookie, `Secure`, `HttpOnly`, `SameSite=Lax`, CSRF protection (per-form token, `Sec-Fetch-Site` checked); attempt limits per address and per account.
- Signing key: in a separate file, reserved for the relay; rotation possible (two keys published, the Station pins both during the transition).
- A hacked relay can issue tokens for any Station whose switch is on: that is the price of “the relay decides”, bounded by the safeguards above.

## Legal and stores

- **GDPR** (Les Lignes Claires, data controller): record of processing activities, the service's privacy policy (public relay), terms of use, data processing agreement for organizations, breach notification within 72 h, email provider: SendGrid (Twilio, United States, EU-U.S. Data Privacy Framework and standard contractual clauses).
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

`__Host-kn_session` cookie (`Secure`, `HttpOnly`, `SameSite=Lax`), 256-bit token kept as a hash; CSRF token per form; Argon2id; 10 sign-in attempts per address, 5 per account and network and 30 per account in 15 minutes; emails and tokens never in the logs; same answers whether an account exists or not (sign-in, reset, sign-up with a taken address: an email tells its owner).

**Hardening from the 27/11 audit** (`docs/audit-2026-11-27/relais.md`, fixes in `docs/audit-2026-11-27/corrections/relais.md`):

- **Per-right end dates** (migration 10, `grants.right_ends`): accepting a link or claiming an invitation adds rights, each with its own end date; an endless “Watch” link no longer makes a limited Administer permanent.
- **Bounded delegation**: whoever received Administer (neither owner nor Owner or Administrator of the owning team) only gives their own rights, until they end (adding, links, and when a link is accepted); they only open the computer to a team or group if their rights have no end date. When their right is removed (or loses Administer), what they gave on the computer goes with it, step by step, and their pending links are revoked. Removing someone from a team also removes their individual rights on the team's computers.
- **Adding a passkey or turning on two-factor**: no password in the 15 minutes after signing in, otherwise the password is asked again. A new password removes passkeys added in the last 7 days. An unreadable TOTP secret (accounts key changed) refuses the sign-in instead of letting it through, and the relay refuses to start without its key when accounts have two-factor.
- **Sign-up**: the pending cookie names the account this browser created; “change address” and the session opened after verification only apply to it (signing up with a taken address no longer touches someone else's pending account).
- **Sign-in attempts**: 10 per address, 5 per account and network (/24, /48) and 30 per account in 15 minutes; a trusted browser of the account is never blocked by other people's attempts.
- **Invitations**: 20 per hour and 100 per day per account (teams and computers, resends included); at most 20 families or teams per owner.
- **Open Admin**: the token only goes to an Admin address the Station gave (or loopback, or a network name with the same scheme); a network IP address Admin did not give is refused.
- **Return to the app**: optional PKCE S256 (`code_challenge`, `code_challenge_method=S256` on `/compte/connexion?appareil=…`, `code_verifier` on `POST /v1/devices/token`); a code requested with a challenge is only exchanged with its verifier.
- **Teams**: only an Owner (or the owner) moves or detaches a team computer; no enrollment tokens for a family.
- **Short secrets**: recovery codes and email codes kept as an HMAC with the relay's key (older recovery codes still work). One-year HSTS on the relay's pages.

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
- an existing account added to a computer or an organization gets an **invitation** they accept or decline in their space (“Invitations”, and a banner on “Computers”); before 27/11 it was added right away (REL-11); an address without an account waits for the address to be verified;
- no end date for rights given to an organization or a group (only to people), and no role per group; lending's time slots and Steam account stay those of friend accounts;
- the organization log keeps the openings of its own computers; those of a person's computer opened to the organization stay in the owner's account log;
- “Copy” for the enrollment token isn't a button (the pages have no script): the token is selected in one click;
- “Add a person” is a form on the page, not a window on top as in the mockup.

## Open question

1. **Demo account for Apple**: on the public relay, with the Session right on the demo Station (suggested).
