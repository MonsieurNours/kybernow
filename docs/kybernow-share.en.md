# KyberNow Share: local sharing

English · [Français](kybernow-share.md)

KyberNow Share lets someone else in on your computer: a **spectator**, who sees and hears, or a **player**, whose controllers become Windows' next controller. This first step works **on the same network**; the relay (away from home) and a separate encode for a remote guest come next ([ROADMAP.en.md](../ROADMAP.en.md)). This document is the shared contract of the Station, the Rust core, every Link app and the web client.

Sharing has no screens of its own: it reuses the home screen, the connection, the session bar and the settings of [link-ui.en.md](link-ui.en.md), which gives the exact texts (“Sharing” section). Every app behaves the same and uses the same words.

## What the user sees

### Sharing

There are two ways to share:

- **From one's session** (the host): “Share” in the session bar. The share shows their main display, and the computer's sound.
- **From the PC itself**, without a session: “Share a screen…” in the menu of the KyberNow Station icon next to the clock, or on Admin's home page. One picks the display to share among the PC's displays; one keeps using the PC as usual, guests see that display and hear the PC's sound. No virtual display is created, the PC's displays do not switch off.

Sharing opens a panel: a six-digit **code** (“123 456”), a **link** and its **QR code** (`kybernow://join?…`, or the web client's address), and the list of guests. The code is valid as long as the share is open; “New code” changes it (guests already there stay). “Stop sharing” makes everyone leave. Closing the host's session also stops the share.

### Guests

- A guest **arrives as a spectator**: they see and hear, and send nothing.
- In the list, the host can **“Let play”** a guest: their controllers become Windows' next controller (a virtual controller of their own). Then **“Give keyboard and mouse”**, if they want; “Take back keyboard and mouse” removes them. “Remove” makes them leave the share.
- **Four guests at most.** Beyond that, the code is refused with the reason.
- The Station enforces these rights: a spectator has no input on the computer, a player only their controllers (and the keyboard and mouse if the host gave them), even if their device tried to send something else.
- Admin shows the share (display, guests, each one's role), can remove a guest and stop the share; its feed tells the story (“Léa is watching your screen”, “Léa is playing with you”).

### Joining

- **“Join with a code”** on the home screen of every Link app (next to “Add a computer”): one types the six digits; the app sends them to the computers it finds on the network, and the one sharing answers. Or one opens the link, or scans the QR code (apps with a camera). In a **browser**, the link opens the web client as a guest, without Admin's password.
- The guest gives their name (suggested: the device's name), remembered for next time.
- A guest session is an ordinary session on screen, with a reduced bar: “Spectator” or “Player”, “Leave”, and the commands their role allows (controller, keyboard and mouse if given). No profile, no computer settings, no virtual display: a guest never changes the computer's displays and never takes over from the host.
- A guest is **not added to the device's list of computers**: nothing is kept after the share, except their name.
- End: “{host} stopped sharing.”, “{host} removed you from the share.” or a failure's reason; never “Take back control”.

### What stays as before

- The picture is **shared**: the guest gets the format the host chose (H.264, HEVC, AV1) and its resolution. A device that cannot play that format says so before joining (“This device cannot play this computer's picture (HEVC).”). The separate encode comes with remote access.
- A slow guest can slow down everyone's picture (Kyber waits for the slowest): rare on the same network; the separate encode will fix it.

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

## Rust core

- **The session's role**: host (as today) or guest (spectator, player). A guest session joins through `share_join` instead of `prepare`, sends no `foreground` nor mode, display or bitrate change, never restarts the Station, refuses Kyber's fingerprint without Admin's, and leaves through `share_leave`. Its inputs follow its rights (no input chain for a spectator; controllers only for a player, keyboard and mouse when given), changed during the share by the sign of life.
- **Host side**: `share_open`, `share_code`, `share_guest`, `share_kick`, `share_close`; the guest list comes with the host's sign of life (`Platform::on_guests`).
- **Joining**: send the code to the Stations found on the network (like pairing), read the `kybernow://join?host&port&fp&code` link.
- **The format**: before joining, the guest checks it can play the host's picture format.
- C bridge and JNI: the same functions, for every app.

## Apps and web client

Every Link app (Mac, Windows, Linux, iPhone, iPad, Apple TV, Android, LG TVs) and the web client: “Join with a code” on the home screen, code entry (on-screen keypad on TVs, as for pairing), the guest session and its reduced bar; “Share” and the host's panel in the session bar. One component per family of apps when possible (KyberKit for the Mac, iPhone, iPad and Apple TV). The texts come from `link-ui.md`.

## Outside this first step

- Joining from outside the network (relay), a separate encode for a remote guest: the next steps of KyberNow Share.
- Lending the computer in turns (dedicated Windows account, launcher): later.
- Several displays shared at once: a share shows one display.
