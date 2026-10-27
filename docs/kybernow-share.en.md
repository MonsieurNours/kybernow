# KyberNow Share: local sharing

English · [Français](kybernow-share.md)

KyberNow Share lets someone else in on your computer: a **spectator**, who sees and hears, or a **player**, whose controllers become Windows' next controller, and who can also get the keyboard and mouse. Each guest gets a picture that suits them, without ever getting in the way of the host's. This first step works **on the same network**; the relay (away from home) comes next ([ROADMAP.en.md](../ROADMAP.en.md)). This document is the shared contract of the Station, the Rust core, every Link app and the web client.

Sharing has no screens of its own: it reuses the home screen, the connection, the session bar and the settings of [link-ui.en.md](link-ui.en.md), which gives the exact texts (“Sharing” section). Every app behaves the same and uses the same words.

## What the user sees

### Sharing

There are two ways to share:

- **From one's session** (the host): “Share” in the session bar. The share shows their main display, and the computer's sound.
- **From the PC itself**, without a session: “Share a screen…” in the menu of the KyberNow Station icon next to the clock, or on Admin's home page. One picks the display to share among the PC's displays; one keeps using the PC as usual, guests see that display and hear the PC's sound. No virtual display is created, the PC's displays do not switch off.

Sharing opens a panel: a six-digit **code** (“123 456”), a **link** and its **QR code** (`kybernow://join?…`, or the web client's address), and the list of guests. The code is valid as long as the share is open; “New code” changes it (guests already there stay). “Stop sharing” makes everyone leave. Closing the host's session also stops the share.

### Guests

- A guest **arrives as a spectator**: they see and hear, and send nothing.
- In the list, the host gives or takes back from each guest **two independent rights**: **“Give controllers”** (their controllers become Windows' next controller, a virtual controller of their own) and **“Give keyboard and mouse”**; “Take back controllers”, “Take back keyboard and mouse” remove them, one without touching the other. “Remove” makes them leave the share; they can come back with the same code, which “New code” prevents (guests already there stay).
- **Four guests at most.** Beyond that, the code is refused with the reason.
- The Station enforces these rights: a spectator has no input on the computer, a guest only what the host gave them (their controllers, the keyboard and mouse, or both; never the clipboard), even if their device tried to send something else.
- Admin shows the share (display, guests, each one's role), can remove a guest and stop the share; its feed tells the story (“Léa is watching your screen”, “Léa is playing with you”).

### Joining

- **“Join with a code”** on the home screen of every Link app (next to “Add a computer”): one types the six digits; the app sends them to the computers it finds on the network, and the one sharing answers. Or one opens the link, or scans the QR code (apps with a camera). In a **browser**, the link opens the web client as a guest, without Admin's password.
- The guest gives their name (suggested: the device's name), remembered for next time.
- A guest session is an ordinary session on screen, with a reduced bar: “Spectator” or “Player”, “Leave”, and the commands their role allows (controller, keyboard and mouse if given). No profile, no computer settings, no virtual display: a guest never changes the computer's displays and never takes over from the host.
- A guest is **not added to the device's list of computers**: nothing is kept after the share, except their name.
- End: “{host} stopped sharing.”, “{host} removed you from the share.” or a failure's reason; never “Take back control”.

### A guest's picture

- **The host always comes first.** Nothing a guest does changes the computer's displays or the host's picture, and a guest's picture is never above the host's (resolution, frame rate, bitrate, 4:4:4): a guest asking for 5K with a host in 4K gets 4K.
- The guest chooses their **“Picture quality”** on the “Join” screen and in their bar: “Auto” (suggested: from the device's screen and what its decoder keeps up with), “Same as host”, “4K · 60 Hz”, “1080p · 60 Hz”, “720p · 30 Hz”. Changing it restarts their picture only.
- A guest who keeps up with the host's picture **shares** it (nothing more for the graphics card). The others get a **separate picture**, encoded for them from the same capture; guests asking for the same quality share theirs. **Two separate pictures at most** besides the host's (one on an Intel card); beyond that, the guest gets the nearest open picture, lighter, and is told so (“The computer can't make another picture: you get {quality}.”).
- Kyber **no longer waits for a guest falling behind**: on the host's picture, they move by themselves to a lighter picture (“Picture lightened to keep up.”); the host sees nothing. On their separate picture, a guest can ask for a keyframe again after an error; on the host's picture, their requests are spaced out.
- The host sees each guest's quality in their panel (“1080p · 60 Hz”), without changing it.
- A device that cannot play the host's format gets a separate picture in a format it plays (otherwise H.264); “Same as host” on a device that cannot play the host's 4:4:4: a separate 4:2:0 picture at the host's resolution, 60 Hz at most. Only a device that plays no possible format says so before joining (“This device cannot play this computer's picture (HEVC).”).
- **Share started from the PC** (no host): three pictures at most for guests (two on an Intel card); “Same as host” is the display as it is.

## Exchange with Admin

All JSON, on Admin's port, certificate pinned by fingerprint as today. The names below are authoritative for the Station, the core and the web client.

**Guest** (no account; rate-limited like pairing: 10 tries per minute and per address, 6-digit codes):

- `POST /api/share/join` — `{"code":"123456","name":"Léa","device":"<device id>","app":"mac|windows|linux|ios|tvos|android|webos|web"}` → `200 {"guest":"<id>","secret":"<guest token>","role":"spectator|player|kbm|player_kbm","host":"<host's name or the computer's name>","computer":"<name>","kyber":{"port":12345,"fingerprint":"sha256:…","token":"<JWT>","expires":120},"display":{"id":2221634456,"width":1920,"height":1080,"refresh":60},"codec":"h264|hevc|av1"|null,"yuv444":false,"beat":5}`. Errors: `404 {"error":"no_share"}`, `409 {"error":"full"}`, `429` (too many tries). `codec` is the current picture's format (absent if the picture has not started yet): the guest checks it can play it before opening the picture.
- `POST /api/share/beat` — header `x-kybernow-guest: <secret>`, `{"guest":"<id>"}`, every `beat` seconds → `{"role":"…","token":"<renewed JWT>","expires":120,"end":null|"stopped"|"removed","image":{"quality":"auto|host|2160p60|1080p60|720p30","width":1920,"height":1080,"fps":60,"codec":"hevc","follows_host":false,"reason":"asked|capped|limit|lag"}}`. Without a sign of life for 20 s, the guest leaves.
- `POST /api/share/quality` — same header, `{"guest":"<id>","quality":"auto|host|2160p60|1080p60|720p30"}` → the `image` above; one change per second and per guest at most, beyond that `429 {"error":"too_many","retry_after":1}` with `Retry-After`; Kyber restarts the guest's picture only. `join` also takes `"quality"` (and, for “Auto”, what the device can show and decode: `"auto":{"width","height","fps","codecs":[…],"yuv444"}`); the host's view gives `guests[].image`. The quality travels in Kyber's token (`kn_quality`); Kyber's plan: patch 0020 ([kyber-changes.en.md](kyber-changes.en.md)).
- `POST /api/share/leave` — same header, `{"guest":"<id>"}`.

**Host** (the device account, like `prepare`; only the device in session):

- `POST /api/share/open` `{}` → `{"code":"123456","link":"kybernow://join?host=…&port=…&fp=…&code=123456","web":"https://<host>:<port>/link/?join=123456","display":{"id":…,"label":"Display 1"}}` (their main display).
- `GET /api/share` → `{"open":true,"code":"…","link":"…","web":"…","display":{…},"guests":[{"id":"…","name":"Léa","app":"ios","role":"spectator","since":<Unix seconds>}]}`; the same in the reply to the host's sign of life (`heartbeat`, field `share`, absent without a share).
- `POST /api/share/code` → a new code (same reply as `open`).
- `POST /api/share/guest/{id}` `{"role":"spectator|player|kbm|player_kbm"}` (`player`: controllers; `kbm`: keyboard and mouse; `player_kbm`: both); `DELETE /api/share/guest/{id}` (remove); `DELETE /api/share` (stop).

**The PC itself** (the clock icon's local routes, the PC user's token): `GET /api/local/displays` → `[{"id":2221634456,"name":"\\\\.\\DISPLAY1","label":"Display 1","width":3840,"height":2160,"primary":true}]`; `POST /api/local/share/open {"display":<id>}` and the same routes as the host under `/api/local/share…`. Admin (its home page) the same with its session.

**`info`**: `"share":{"available":true,"enabled":true}` (`enabled` false if `multi_client` is off); absent for an older Station.

**Link**: `kybernow://join?host=<address>&port=<Admin port>&fp=<Admin fingerprint>&code=<6 digits>`; browser: `https://<address>:<port>/link/?join=<6 digits>`.

**Kyber**: the guest connects with the JWT (token login, like the web client), on the given display, without asking for inputs if they are a spectator. The token carries the role and the display (claims `kn_role`, `kn_display`), which Kyber enforces (patch 0019).

## Station

- **Participants**: Admin moves from a single session to a host and guests (role, name, device, last sign of life). The host stays what the session is today (preparation, virtual display, takeover by another of the host's devices); a guest never goes through `prepare` nor the takeover. The guards (restart, update, end of session, pending accounts) see the guests.
- **API**:
  - host: `POST /api/share/open` (display, from their session), `GET /api/share`, `POST /api/share/code` (new code), `POST /api/share/guest/{id}` (role, keyboard and mouse), `DELETE /api/share/guest/{id}`, `DELETE /api/share`;
  - local PC: the same through the icon's local routes (`/api/local/share…`, the PC user's token), plus the list of physical displays;
  - guest: `POST /api/share/join {code, name}` (public, rate-limited like pairing), then the sign of life `POST /api/share/beat` and `POST /api/share/leave`. The reply gives a short-lived Kyber access token (renewed by the sign of life), Kyber's port and fingerprint, the display to receive, the role, and for the browser a cookie limited to the relay and `/api/share/*`.
  - `info` announces sharing (`share`), so apps know whether the Station supports it.
- **Kyber (patch 0019)**: the role carried by the token (spectator, player, player with keyboard and mouse), applied when inputs open (none for a spectator; controllers only for a player), changed during the share without cutting the picture; the allowed display enforced (the display list filtered, another one refused); a Kyber session Admin can close at once (removing a guest); bitrate and keyframes of the shared picture reserved to the host.
- **Sharing from the PC**: the list of physical displays (name, frame, Kyber id), a “sharing” state that prevents automatic virtual display creation (the “For any connection” mode) and changes of the sound output; the icon next to the clock and Admin's home page start it, show it and stop it.
- Kyber's `multi_client` must be on (the default): if it is off, “Share” says so.
- **Station details** (what it does on top of “Exchange with Admin”, without changing it):
  - “Auto” is settled by Admin from `auto` (`width`, `height`, `fps`; long and short sides compared with the shared display's): the host's picture if the device covers the display and follows its rate, otherwise `2160p60` if it shows at least 3840 × 2160 at 60 Hz, otherwise `1080p60` if it shows at least 1920 × 1080 at 60 Hz, otherwise `720p30`; `join` without `auto` (apps before 0.8.0): the host's picture. The preset goes into `kn_quality`; Kyber always caps it by the host's picture (a 1440p host gives at most 1440p for “4K · 60 Hz”), and a guest that falls behind goes down one notch at a time: host → `2160p60` → `1080p60` → `720p30` (reason `lag`); `auto`'s `codecs` and `yuv444` are accepted, but the client's picture request to Kyber decides the format and 4:4:4. `image` (heartbeat, `quality` reply, `guests[].image`) is always an object: `quality` is the asked quality (`auto` included), the other fields are `null` while Kyber knows nothing (read again every 2 s at most);
  - replies: the host and PC routes all return the same share view, which contains the contract's (`code`, `link`, `web`, `display`) and adds `open`, `origin` (`host` or `local`), `host`, `guests` (with `device` and `last_beat`), `max_guests`, `opened_at`, and in `display` `index`, `width`, `height`, `refresh`; `GET /api/local/displays` adds `index`, `refresh`, `x`, `y`, and only keeps the displays Kyber captures when it answers;
  - host routes: optional `device_id` (body, or query for `GET` and `DELETE`), as in the heartbeat, to tell two devices on the same account apart; refusals `{"error": <text>, "code": …}` with `not_in_session`, `disabled`, `no_share`, `unknown_guest`;
  - guest: refusals beyond the contract's, `400 {"error":"bad_request"}` (invalid code or name), `503 {"error":"unavailable"}` (Kyber without a token key; for `quality`, another preset Kyber did not take: nothing changes, neither the token nor the picture received, the guest may try again); `429` carries `{"error":"too_many","retry_after":<s>}` and `Retry-After`; `beat` and `leave` from an unknown guest (gone, timed out, or more than a minute after leaving): `404 {"error":"unknown_guest"}`;
  - `beat` also gives `display` (`id`, `width`, `height`, `refresh`): a host's share follows its main screen when it prepares again (virtual display recreated under another identifier); the renewed token then carries the new `kn_display`, which Kyber only takes at a new connection (not on renewal);
  - the same device (`device`) joining again gets its place back (same `guest`, same role, new `secret`), without counting as one more guest; its quality of today also goes to Kyber, which would otherwise prefer the one it remembers (10 min) to the token's;
  - a guest's Kyber token: `sub` = `kn-guest-<guest>`, and `kn_rev` (increasing with every role change) besides `kn_role` and `kn_display`;
  - sharing from the PC: refused during a device's session (`code` `session`), while a device is getting ready or another connection took the displays or the sound (`busy`); a session that starts (a device's `prepare`) stops it, even when it crossed the opening (checked again once the share is open, then every second); one share at a time;
  - Kyber (0019): a spectator may open its inputs, Kyber lets nothing through to the computer; “Give controllers” or “Give keyboard and mouse” then applies without restarting the image. A guest that restarts its image costs the shared encoder one keyframe (the one Kyber forces for any arriving client), without changing its configuration. With 0020, its `/force_idr` is allowed on its separate picture, spaced out (one every 2 s) on the host's picture, and its `/set_bitrate` only when it is alone on its separate picture. Each guest has its own input server, hence its own virtual gamepads (eight announced at most are remembered); a guest holds two Kyber sessions at most (a third login: `409`), and six WebSockets at most through Admin's relay;
  - the guards name the guests (“A share is in progress with Léa and Max: restarting will stop it”); with `force`, the share stops first (guests read `stopped`).

## Rust core

Details and exact names: [apps/core/README.en.md](../apps/core/README.en.md#kybernow-share-srcsharers-srcsessionrs-srcinputpipeliners); C bridge: [coeur-apple.en.md](coeur-apple.en.md).

- **The session's role** (`SessionParams::role`): `SessionRole::Host` (as today) or `SessionRole::Guest` (`share_join`'s answer and what the device decodes; `SessionParams::guest`, `SessionParams::from_json` for the bridges). A guest session joins through `share_join` instead of `prepare`, connects to Kyber with the token (renewed by the sign of life), receives the display given by the answer (and reconnects to it when the sign of life gives another one), sends no host `heartbeat` nor `foreground`, no mode, display, profile or bitrate change, no keyframe request, never restarts the Station, refuses a Kyber without the fingerprint Admin gives (never trust on first use), and leaves through `share_leave` (never `end`, no report of restored displays). Its inputs follow its two independent rights (`ShareRole`: `spectator`, `player`, `kbm`, `player_kbm`; `InputRights`: no input chain for a spectator; the controllers, the keyboard and mouse, or both, each given or taken back without touching the other; never the clipboard), changed during the share by the sign of life (`Platform::on_share_role`). Ended by the share (`end: stopped|removed`): `Platform::on_share_end`, with the text from [link-ui.en.md](link-ui.en.md).
- **Host side**: `Console::share_open`, `share_state`, `share_code`, `share_guest`, `share_kick`, `share_close`; the guest list comes with the host's sign of life (`Platform::on_guests`); `Info::share` and `ShareAvailability` for “Share”.
- **Joining**: `share::join` sends the code to the Stations found on the network (like pairing) and returns the first that accepts it; `share::parse_link` reads the `kybernow://join?host&port&fp&code` link (and the web client's).
- **The format**: the guest asks for the host's format if it reads it, otherwise for a format it reads (`share::guest_format`, `share::join_checked`, and the session itself); it only refuses to join when none is possible.
- **A guest's picture**: `ShareQuality` (`auto`, `host`, `1080p60`, `720p30`) in `join` (`JoinRequest::quality`, and `auto` from `share::auto_quality`), `Session::set_share_quality` during the session; the picture received (`ShareImage`) and the notice through `Platform::on_share_image`; keyframes allowed for a guest, bitrate of its own picture only.
- C bridge and JNI: the same functions, for every app (`kn_share_*`, `kn_console_share_*`; `Native.share*`, `Native.consoleShare*`).

## Apps and web client

Every Link app (Mac, Windows, Linux, iPhone, iPad, Apple TV, Android, LG TVs) and the web client: “Join with a code” on the home screen, code entry (on-screen keypad on TVs, as for pairing), the guest session and its reduced bar; “Share” and the host's panel in the session bar. One component per family of apps when possible (KyberKit for the Mac, iPhone, iPad and Apple TV). The texts come from `link-ui.md`.

**Web client** (`server/web/src/link/`): the same session as the host's (same page, `Session.svelte`), with a reduced bar for the guest. It has no Rust core: it does the exchange with Admin above itself (`app: "web"`, `device`: this browser's identifier) and keeps the `kybernow_guest` cookie, which only opens the relay to Kyber and `/api/share/*`; Kyber's engine therefore only loads after `join`, and the picture's format is checked then (otherwise `leave`). The engine opens its inputs from the start and the page filters by role; Kyber refuses what the role does not allow anyway. The host, in the browser: “Share” in the bar, Admin's panel on the host routes, guests read from the heartbeat's answer (`share`).

## Outside this first step

- Joining from outside the network (relay): the next step of KyberNow Share.
- Lending the computer in turns (dedicated Windows account, launcher): later.
- Several displays shared at once: a share shows one display.
