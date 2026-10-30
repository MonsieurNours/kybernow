# KyberNow Share: lending the computer

English · [Français](pret.md)

Lending the computer means handing it to a friend while you're not using it. Your friend plays on your PC from their KyberNow Link app or their browser, at your place or from home. They see neither your desktop nor your windows: the Station puts your Windows session in **console mode**, a full-screen launcher that only shows the apps you picked. When the loan ends, everything goes back as it was and the session locks.

This document is the shared contract of the Station (Admin, the icon next to the clock, Kyber), the Rust core, every Link app and the web client. The exact texts are in [link-ui.en.md](link-ui.en.md), section “Lending the computer”. Lending is part of KyberNow Share ([kybernow-share.en.md](kybernow-share.en.md)) and uses the same relay away from home ([relais.en.md](relais.en.md)).

## What the user sees

### What is lent

- **Your own Windows session**, in console mode: no separate Windows account, no second Windows. The session must therefore be **open**: if it is locked, your friend reads “The computer is locked.” and the loan does not start. While lending is on, the Station keeps the session from locking itself and from sleeping.
- **The allowed apps**: Steam (in Big Picture) and the apps you add in Admin, picked from the same catalogue as the app opened at the start of a session (Start menu, suggestions, limited browser).
- **The Steam account, chosen for each loan**: “Their Steam account” (Steam signs out of yours, your friend signs in with theirs; your games through Steam Families if they belong to it) or “Your Steam account” (they play with your games and saves). With “Their Steam account”, the accounts Steam remembers for you are set aside during the loan: your friend cannot pick yours without a password. At the end, Steam goes back to your account.
- **Your files are protected as well as possible, not sealed off**: Explorer, the Windows key, Alt+Tab, Task Manager, command prompts and apps outside the list are blocked, and “Open” and “Save as” windows are closed; but a game or an allowed app can still read your disk. Admin says so: for friends you trust.

### Three ways to lend

1. **A code for one session**: “Lend the computer…” (Admin, the icon next to the clock, the session bar of your Link apps). You pick the duration (“1 hour”, “2 hours”, “4 hours”, “Until I take it back”) and the Steam account; the Station gives a six-digit code, a link and a QR code, as for sharing. The code works once; it is valid for 24 hours if unused, and “Cancel code” erases it. Your friend enters with “Join with a code”, as for a share.
2. **Friend accounts**: in Admin, “Lending” page, you create one account per friend (first name, username, password, suggested Steam account). Admin gives an invitation link and its QR code; your friend opens it, their app adds “{host}'s computer” to its home screen, and they borrow with their username and password (remembered by the app if they want). On your network, “Add a computer” also offers “With a friend account”.
3. **Time slots**: on a friend's account, weekly ranges (“Wednesday, 2 PM to 6 PM”). During their slot, your friend borrows without asking you.

### When your friend arrives

- **Computer free** (no device session, no share, nobody at the PC for 5 minutes): the loan starts.
- **Computer in use**:
  - code, or friend account outside a slot: you get “Léa wants to borrow the computer” (the icon's notification, Admin, your session's bar) with “Accept” and “Decline”; your friend waits (“Waiting for {host}…”). Accepting closes your session (or puts the PC in console mode if you were at it);
  - **during their slot, your friend goes first**: you are warned (“Léa's time slot: the computer is lent to them in 2 min.”), then your session closes.
- A code created from your session counts as consent: your session closes when your friend arrives (the panel says so).
- One loan at a time; during a loan, no share and no other device's session.

### During the loan

- **Your friend** has an ordinary session on their screen (a virtual display sized to their device, Game profile by default), with the keyboard, the mouse and their controllers, no clipboard. Their bar says “Lent by {host}”, the time left (“1 h 20 left”, or “Until {host} takes it back”), and “Give back the computer”. Closing the window or losing the network does not give the computer back: they can come back for 15 minutes, after which the loan stops.
- **The launcher** (on their screen): KyberNow Share logo, one tile per allowed app, the time left, “Give back the computer”. With a controller, the keyboard or the mouse. A closed app brings the launcher back.
- **Your PC's screen** is covered: KyberNow Share logo, “Computer lent to Léa”, the end time when there is one, and “To take back the computer, press Enter.”. The keyboard and mouse plugged into the PC do nothing else; Enter asks “Take back the computer? Léa has 2 minutes to save.” (Enter: take back, Esc: cancel).

### Taking back

- **“Take back the computer”**: Admin, the icon next to the clock, your Link apps (this computer's home says “Lent to Léa”), the PC's screen (Enter). Your friend has **2 minutes**: “{host} is taking back the computer in 2 min: save your game.” Meanwhile, “Take back now” cuts without waiting.
- **Planned end** (code duration, end of the slot): the same 2-minute notice, “The loan ends in 2 min: save your game.”.
- **End**: the loan's apps close, Steam goes back to your account, your windows come back, the virtual display goes away, and the Windows session **locks**. Your friend reads “{host} took back the computer.”, “The loan is over.” or “Your time slot is over.”.

## Exchange with Admin

All JSON, on Admin's port, certificate pinned as for sharing. The names below are authoritative.

**The friend** (no paired device; rate-limited as for sharing: 10 tries per minute per address, and accounts blocked for 10 minutes after 10 refused passwords):

- `POST /api/lend/status` — `{"login":"lea","password":"…"}` → `{"state":"free|busy|lent|locked|off","host":"Matthieu","computer":"PC-SALON","slot":{"now":false,"next":{"start":<Unix s>,"end":<Unix s>}}|null}`: the state of the “{host}'s computer” card. `401 {"error":"bad_credentials"}`.
- `POST /api/lend/borrow` — `{"login","password"}` or `{"code":"123456"}`, plus `{"name":"Léa","device":"<id>","app":"mac|windows|linux|ios|tvos|android|webos|web","screen":{"width":2560,"height":1440,"refresh":120},"codecs":["hevc","h264"]}` →
  - `200 {"lend":"<id>","secret":"<token>","host":"Matthieu","computer":"PC-SALON","kyber":{"port":…,"fingerprint":"sha256:…","token":"<JWT>","expires":120},"display":{"id":…,"width":2560,"height":1440,"refresh":120},"until":<Unix s>|null,"steam":"friend|host","beat":5}` once console mode is ready (30 s at most);
  - `202 {"request":"<id>","secret":"<token>"}`: the host must agree, the app calls `POST /api/lend/wait` (same header as `beat`, 30 s long poll, repeated) → the `200` above, `403 {"error":"refused"}`, or `410 {"error":"expired"}` (2 min without an answer);
  - refusals: `401 bad_credentials`, `404 no_code`, `409 lent` (a loan in progress), `423 locked` (Windows session locked), `503 off` (lending off, or no open Windows session), `429 too_many` (`retry_after`).
- `POST /api/lend/beat` — header `x-kybernow-lend: <secret>`, `{"lend":"<id>"}`, every `beat` seconds → `{"token":"<JWT>","expires":120,"until":…|null,"notice":null|{"reason":"taken_back|time_up|slot_end|slot_start","at":<Unix s>},"end":null|"taken_back|time_up|slot_end|left|stopped"}`. Without a heartbeat for 20 s, their picture stops; the loan goes on for 15 minutes, and the same device (or the same account, or the same code) comes back to it through `borrow`.
- `POST /api/lend/leave` — same header: “Give back the computer”.

**The host** (paired device, as for `share/open`; the icon through its local routes `/api/local/lend…`; Admin with its session) — every route returns the **loan view**:

- `GET /api/lend` → `{"enabled":true,"state":"free|preparing|lent|ending","borrower":{"name":"Léa","kind":"code|friend|slot","app":"ios","via":"lan|relay","since":…,"until":…|null,"steam":"friend|host","connected":true}|null,"notice":null|{"reason":…,"at":…},"requests":[{"id":"…","name":"Léa","kind":"code|friend","app":"…","via":"…","since":…}],"code":{"code":"123456","link":"kybernow://join?…","web":"https://…/link/?join=123456","duration":7200|null,"steam":"friend|host","expires":…,"relay":null|{"state":"connecting|joined|unreachable"}}|null}`; like the share view since 0.8.1, the code carries `"relay":null|{"state":"connecting|joined|unreachable"}` (`null` with no relay set): `link` and `web` switch to the relay link as soon as the code is held there (`joined`), and Admin, the icon and the apps wait for that link as for sharing (“Getting the away-from-home link ready…”, copy and QR code held 10 s at most, then the network link); the same in the reply to the host's heartbeat (field `lend`, absent when there is nothing to say).
- `POST /api/lend/code` `{"duration":3600|7200|14400|null,"steam":"friend|host"}`; `DELETE /api/lend/code`.
- `POST /api/lend/requests/{id}` `{"accept":true,"steam":"friend|host"}`.
- `POST /api/lend/take-back` `{"now":false}` (2-minute notice) or `{"now":true}`.

**Settings** (Admin only): `GET`/`PUT /api/lend/settings` `{"enabled":false,"apps":[<Launch>…],"steam":true}`; `GET /api/lend/friends`, `POST /api/lend/friends` `{"name","login","password","steam":"friend|host","slots":[{"days":[3],"start":"14:00","end":"18:00"}],"enabled":true}`, `PUT`/`DELETE /api/lend/friends/{id}`; `GET /api/lend/friends/{id}/invite` → `{"link":"kybernow://borrow?host=…&port=…&fp=…&login=lea[&relay=…&sid=…]&name=Matthieu","web":"https://…/link/?borrow=lea&name=Matthieu"}`. `name`: the host's first name (URL-encoded), for “{host} is lending you their computer.” before the account is checked; absent if the Station does not know it. Passwords are stored hashed (Argon2) and never returned.

**`info`**: `"lend":{"available":true,"enabled":false}`; absent for an older Station.

**Codes**: the same six-digit space as sharing (Admin never hands out a code already taken). `POST /api/share/join` with a lending code answers `404 {"error":"no_share","lend":true}`: the app then calls `borrow`. Away from home, the relay also holds the lending code, and lets `/api/lend/status`, `borrow`, `wait`, `beat` and `leave` through (not the host's routes, except for a paired device with “Away-from-home access”).

**Kyber**: the friend connects with the JWT, role `player_kbm` (keyboard, mouse, controllers, never the clipboard) and `kn_display` = their virtual display (patch 0019, unchanged). `sub` = `kn-lend-<lend>`.

## Station

- **Console mode** (agent `kybernow-tray.exe --console`, the icon's executable, started by Admin in the user's session, with their account; launcher and screen cover as `wry` windows; see “Station details”):
  1. virtual display sized to the friend's device (as for a device session), as the primary display; the PC's displays stay on and are covered (KyberNow Share logo, dark page);
  2. the user's windows minimised (restored in the same state at the end), taskbar hidden, cursor kept on the virtual display (`ClipCursor`, reapplied);
  3. low-level keyboard and mouse hooks: **physical** input (without `LLKHF_INJECTED`) is swallowed, except Enter and Esc for the cover; input injected by Kyber goes through, except Win, Alt+Tab, Alt+Esc, Ctrl+Esc, Ctrl+Shift+Esc;
  4. watching the session's processes (process creation, plus a pass every 500 ms): allowed are the launcher, the allowed apps and their child processes, Steam and its games (`steamapps`), and Windows programs outside a refused list (a new Explorer, Task Manager, `cmd`, PowerShell, Terminal, Registry Editor, `mmc`, Control Panel, Settings); any other new process is closed. The user's processes opened before the loan are left alone;
  5. “Open” and “Save as” windows of the allowed apps closed;
  6. Steam: “Their Steam account” → Steam closed (`steam -shutdown`), `config/loginusers.vdf` and auto-login (`AutoLoginUser`) set aside, Steam reopened in Big Picture at the sign-in screen; at the end, Steam closed, what the friend left erased, the user's files put back, Steam reopened if it was running.
- **End**: allowed apps closed (`WM_CLOSE`, then killed after 10 s), Steam restored, windows restored, taskbar, virtual display removed, cursor freed, hooks removed, `LockWorkStation`.
- **Locked session**: the loan does not start (`423 locked`). Lending on: `SetThreadExecutionState` prevents sleep and the display turning off, which would lock the session (to check on the computer with “Require sign-in”).
- **Computer in use**: a device session, an open share, or physical input in the last 5 minutes (`GetLastInputInfo`, not counting Kyber's injected input).
- **Time slots** in the PC's local time; 2-minute notice before the end; a slot that starts while the computer is in use does nothing until the friend arrives.
- **Guards**: restart, update, end of session and pending accounts see the loan (“The computer is lent to Léa: restarting will stop the loan.”).
- **Admin's feed**: “Computer lent to Léa”, “Léa gave back the computer”, “You took back the computer”.
- **Admin, “Lending” page**: the “Lend the computer” switch (off at first) and the warning “Your friend plays in your Windows session…”; “Allowed apps” (Steam, then “Add an app” from the catalogue); “Friend accounts” (first name, username, password, suggested Steam account, time slots, invitation link and QR code); the current loan and the one-session code.
- **Icon next to the clock**: “Lend the computer…” (code panel), “Take back the computer” during a loan, the requests' notification (“Accept”, “Decline”).

### Station details

What the Station does on top of the above, or differently. None of it changes what the apps and the core follow; the extra fields can be ignored.

- **Console agent**: the icon's executable, `kybernow-tray.exe --console`, started by Admin in the user's session with their account (even if the icon is closed), instead of `display-agent console`. It carries the launcher, the cover, the hooks, the windows, the taskbar, the cursor, Steam and the “Open” windows. **Process watching** (point 4) is done by Admin itself (SYSTEM), which can close an elevated Task Manager or command prompt; so is the final lock (`display-agent lock`). The agent talks to Admin through local routes (loopback and local token, like the icon): `GET /api/local/lend/console` (phase, friend, host, computer, end, notice, Steam, virtual display, allowed apps, language), `POST /api/local/lend/console/ready` (Admin waits for this signal, 30 s at most, before answering `200` to the friend), `…/beat` every 2 s (with no news for 10 s during a loan, Admin restarts the agent once, then stops the loan), `…/launch` `{"id"}` (Admin opens the allowed app), `…/give-back`, `…/take-back` `{"now"}` (Enter on the cover, then Enter during the notice), `…/ended` (restoration done: Admin closes what is left, removes the virtual display and locks), `GET /api/local/lend/icons/{file}`. With no answer from Admin for 60 s, the agent restores everything on its own and locks the session.
- **Displays**: for a loan, the virtual display is added to the PC's displays (extended, to their right, primary display) instead of replacing them as for a device session; Admin does not re-isolate displays during a loan.
- **System keys**: Kyber (patch 0021) refuses Ctrl+Alt+Del and Win+L to the friend as to every share guest; kynput itself turns them into a secure attention sequence and `LockWorkStation`, out of reach of the agent's hooks.
- **Loan view**: in addition, `computer` (the computer's name), `host` (the host's first name: the full name of the session's Windows account, otherwise its account name) and `available: true`; in `code`, `web_local` (the network link, even when `web` is the relay's) and `from_session` (code created from the current session: it counts as approval). `/api/status` (Admin) and `/api/local/state` (icon) carry it too, in `lend`.
- **Host routes**: `GET /api/lend`, `POST`/`DELETE /api/lend/code`, `POST /api/lend/requests/{id}` and `POST /api/lend/take-back` for a paired device (Basic account, like `info`) or Admin's session; `/api/local/lend`, `/api/local/lend/code`, `/api/local/lend/requests/{id}` and `/api/local/lend/take-back` for the icon. Refusals as for sharing: `{"error":"<translated text>","code":"disabled|lent|no_lend|unknown_request|bad_request|forbidden"}`. Settings and friend accounts: Admin's session only, never through the relay; `GET /api/lend/settings` adds `steam_found`; username of 3 to 32 characters `a-z0-9._-`, unique; password of at least 6 characters; time slot: `end` after `start`, never across midnight; `days` from 1 (Monday) to 7 (Sunday).
- **Friend**: `wait` with no answer from the host after 30 s returns `202 {"pending":true}` (call again); an unknown request or loan, `404 {"error":"unknown"}`. Through the relay, the `200` reply of `borrow` and `wait` carries `"relay": true` (picture over WebSocket, as for a guest). For the browser, that reply sets the `kybernow_lend` cookie (`__Host-kybernow_lend` through the relay), like `kybernow_guest` for a guest: Admin's proxy to Kyber accepts it while the loan lives; `leave` clears it.
- **Relay**: the relay agent stays joined while there is a lending code, a loan, or lending on with a friend account; the friend's routes answer `503 off` when lending is off.
- **Sleep**: `SetThreadExecutionState` is held while lending is on, not only during a loan.
- **Steam** (differs from point 6): the user's files (`config/loginusers.vdf`, and `%LOCALAPPDATA%\Steam\local.vdf` where Steam keeps its sign-in token) are **moved** (`*.kybernow`), not copied: the friend does not see the saved accounts; Steam opens with `steam.exe -gamepadui` (Big Picture, straight on the sign-in screen) instead of `steam://open/bigpicture`; stopped with `-shutdown`, 20 s, then killed. At the end, the user's files go back over the friend's (a file that did not exist before is deleted), and Steam is reopened with `-silent` if it was running. “Your Steam account”: `steam://close/bigpicture` at the end, or `-shutdown` if Steam was not running before. A failed switch leaves the loan without `ready`: it stops, and the friend never lands on the host's account.
- **Timings**: switching Steam takes up to 25 s; Admin waits for `ready` 35 s at most, without the preparation going past 40 s (the core waits 45 s for `borrow`); it waits for `ended` 45 s at most (windows 10 s, Steam 25 s).
- **Crash**: the agent's state is written before any change (`%LOCALAPPDATA%\KyberNow\lend\state.json`); a restarted agent resumes the same loan or repairs the old one; the icon repairs at start if Admin no longer knows a loan.
- **End of the loan, Admin side**: what the loan opened and is still running after the agent's restoration is closed, except Windows, KyberNow, Kyber and Steam (its games, `steamapps`, are closed).
- **Known limits**: the host's physical gamepads stay active (XInput cannot be filtered from the session), touchscreens too; the agent is not elevated: it neither minimises nor closes the windows of an elevated app (Admin's watch does close a refused elevated process). A friend who closes their page during preparation leaves the loan alive for 15 minutes (the possible return), unless the host takes it back.

## Rust core

- **Borrowing**: `lend::status`, `lend::borrow` (code or account), `lend::wait`, and a borrowing session `SessionRole::Borrower`: it starts from the `borrow` reply (no `prepare`: the virtual display is already there), connects to Kyber with the token renewed by `beat`, has the keyboard, the mouse and the controllers (`InputRights` of the `player_kbm` role, no clipboard), sends no host heartbeat, no `foreground`, no Station display or profile change, never restarts the Station; reconnects on its own during the 15 minutes; leaves through `lend::leave`. Notice and end: `Platform::on_lend_notice`, `Platform::on_lend_end`, with the link-ui texts.
- **Join with a code**: `share::join` moves on to `lend::borrow` when the Station answers `"lend":true`.
- **Borrowed computers**: `lend::parse_invite` reads `kybernow://borrow?…`; the app keeps the computer apart from its paired computers (username, password in its keychain).
- **Host**: `Console::lend_state`, `lend_code`, `lend_cancel_code`, `lend_answer`, `lend_take_back`; the view also comes with the host's heartbeat (`Platform::on_lend`); `Info::lend`.
- C bridge and JNI: the same functions (`kn_lend_*`, `kn_console_lend_*`; `Native.lend*`).

## Apps and web client

Every Link app and the web client:

- **Friend**: “Join with a code” also takes lending codes; the “{host}'s computer” card on the home screen (added by the invitation link, or “Add a computer” › “With a friend account”), its state and “Borrow”; waiting for approval; the borrowing session and its bar; the notice and the end.
- **Host**: in the session bar, “Lend the computer…” (duration, Steam account, code, link, QR code) and the requests; on this computer's home, “Lent to Léa” and “Take back the computer”.
- TVs and Apple TV: entering the code and the username with the on-screen keypad, as for pairing.
- **Web client**: the friend through `/link/?join=<code>` or `/link/?borrow=<username>`; the host in Admin (“Lending” page).

## Beyond this first step

- Waking a sleeping computer for a friend away from your home.
- Several friends at once, or lending while you play (virtual machine).
- A separate, sealed Windows account: set aside (too heavy), console mode is the chosen path.
