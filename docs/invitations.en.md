# Invitations: adding a computer from afar

English · [Français](invitations.md)

An invitation adds a device to the computer **without anyone having to accept it** on site: a six-digit code and a password, valid for a few days. The device can be anywhere: it goes through the relay ([relais.en.md](relais.en.md)). It is the way to give trial access (Apple's App Review, [app-store.en.md](app-store.en.md)) or to add your own phone when you are away from home.

Once added, the device is a paired device like any other: it has its own account, opens the computer's full session (the desktop, not the lending launcher), on the network or away from home. It loses its access when the invitation expires or is deleted.

This document is the shared contract of Admin, the Rust core, every Link app and the web client. The exact texts are in [link-ui.en.md](link-ui.en.md), section “Invitations”.

## What the user sees

### In Admin (Link devices)

- **“Invitations”** section: “Invite a device…”. You give a **name** (“App Review”) and a **duration** (1, 7 or 30 days; 7 suggested).
- Admin then shows the **code** (“123 456”), the **password** (`k7mq-p2xa-9fz3`, twelve unambiguous characters, shown **once only**), the expiry and the QR code of the apps' link.
- With a relay set, creating an invitation **turns on “Access away from home”** (the device added from afar must be able to connect afterwards); Admin says so. Without a relay, the invitation only works on the network.
- The list: name, code, expiry, devices added (“2 / 5”), with their names; “New password” (the old one stops working, devices already added keep their access); “Delete” (the devices added by the invitation lose their access).
- **Two invitations at most** at a time, **five devices** at most per invitation.
- In the devices list, a device added by an invitation shows “Invitation “{name}””. Removing it frees its slot.

### In the Link apps

1. **“Join with a code”**, as for a share: type the six digits (or open the link, scan the QR code).
2. The Station answers that it is an invitation: the app shows **“Add {computer}”**, with the **“Password”** field.
3. Right password: the computer appears on the home screen, like a paired computer; the app selects it and says “{computer} has been added.”. It then opens with “Connect”, on the network or through the relay.
4. The same computer already on the home screen (same Admin fingerprint) is not added twice: it takes the new access.

The web client is served by the computer itself: an invitation code there only says “This code adds the computer in the KyberNow Link app.”

## Security

- **The code alone gives nothing**: it finds the Station (on the network, or at the relay); only the password adds the device.
- **Password**: twelve characters from `abcdefghjkmnpqrstuvwxyz23456789` (59 bits), drawn by Admin; kept as an Argon2 hash (`invites.json`, data folder restricted to SYSTEM and Administrators). Case, spaces and dashes do not matter: typed with a remote, it works the same.
- **Attempts**: five wrong passwords from one address (a /64 in IPv6) within 10 minutes block it for 10 minutes; fifty in total block every invitation for 10 minutes (a line in the feed). 0.8 s wait after each wrong password.
- **Trust**: with a typed code, the relay gives Admin's fingerprint, as for a share ([relais.en.md](relais.en.md), “Trust”); a lying relay could step in and read the password. The link and QR code carry the fingerprint from the Station: prefer them when you can pass them on.
- **Expiry**: at the end, the invitation is deleted along with the devices it added (Kyber accounts deleted, ongoing session ended).

## Admin

### Routes

| Route | Who | What |
|---|---|---|
| `GET /api/invites` | Admin session | `{"invites":[…],"max":2,"days":[1,7,30],"relay":{"set":bool,"remote_access":bool}}`; an invitation: `id`, `name`, `code`, `created`, `expires`, `devices` (`[{id,name,kind,last_seen}]`), `max_devices`, `last_used`, `link` (apps' link, `kybernow://join?…&code=…`, with `relay` and `sid` when the relay holds the code), `relay` (`null` or `{"state":"connecting"|"joined"|"unreachable"}`) |
| `POST /api/invites` | Admin session | `{"name","days"}` → the view, plus `invite`, `password` (in clear, only here) and `remote_access_turned_on`; `400` refusals: empty name, unknown duration, two invitations already |
| `DELETE /api/invites/{id}` | Admin session | Deletes the invitation and its devices; returns the view |
| `POST /api/invites/{id}/password` | Admin session | New password: the view, plus `invite` and `password` |
| `POST /api/share/join` | Everyone | With an invitation code (no share under that code): `404 {"error":"no_share","invite":true,"computer":"<name>"}` |
| `POST /api/pair/invite` | Everyone (on the network, and through the relay) | `{"code","password","device":{id,name,kind,os,app}}` → `200 {"username","password","hostname","address","port","fingerprint","relay":{"server","id","remote_access"},"invite":{"name","expires"}}`; refusals `{"error":<code>,"message":<text>}`: `404 no_invite`, `401 bad_password`, `409 full`, `429 too_many` (`retry_after`, `Retry-After` header), `400 bad_request`, `503 unavailable`; through the relay, `403 remote_access_off` (access away from home off) |

### Relay

- Guard ([relais.en.md](relais.en.md)): `POST /api/pair/invite` goes through the relay when an invitation is open and access away from home is on; `POST /api/share/join` also passes when an invitation is open. The `/api/invites*` routes never do.
- Codes: the Station holds at the relay the code of every open invitation, with the share's and the loan's (four codes at most per Station: hence two invitations); a code taken elsewhere is replaced by the one the relay gives. An open invitation keeps the agent connected to the relay.

### Account

The device gets its own Kyber account, as after an accepted pairing (`Pairing`: the device's name and four characters, 128-bit secret); Kyber restarts as soon as it serves nobody to pick it up. A device coming back through the same invitation replaces its old account. The invitation keeps the list of its devices (`devices`).

### Feed

“Invitation “{name}” created”; “{device} added by the invitation “{name}””; “Invitation “{name}” deleted, with its devices”; “Invitation “{name}” expired: its devices are removed”; “Invitations blocked for 10 min for {address} after too many wrong passwords.”; “Invitations blocked for 10 min after too many wrong passwords.”

## Core and apps

- `share::ShareError::Invite(Box<invite::InviteCode>)` (`kind` `invite`): “Join” landed on an invitation code; `InviteCode { code, target, route, computer }` says which Station, by which path, and the computer's name. Like `ShareError::Lend`, it comes from `join_anywhere`, `join_link` and `join`.
- `invite::redeem(&InviteCode, password, &DeviceInfo) -> Result<Invited, InviteError>` (blocking, 15 s at most): `Invited { hostname, address, port, fingerprint, username, password, relay, invite_name, expires, via_relay }`, to keep like a paired computer (Admin's address and port, pinned fingerprint, account, `info.relay`).
- `InviteError`: `BadPassword`, `NoInvite`, `Full`, `TooManyTries { seconds }`, `AccessOff`, `Other(ConsoleError)`; `text(lang, computer)`, `kind()` (`bad_password`, `no_invite`, `full`, `too_many`, `access_off`, `other`), `code()` for the bridges (60 to 65).
- Bridges: C (`apps/apple`, `kn_invite_redeem`), JNI (`apps/android/jni`), and the Rust apps (Windows, Linux, webOS) call the core directly.
- An older Station does not know invitations: it answers “No share with this code”, nothing more to do.
