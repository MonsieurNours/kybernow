# The KyberNow relay: sharing and connecting away from home

English · [Français](relais.md)

The relay carries KyberNow over the internet when the device is not on the computer's network. It does two things:

- **Sharing away from home** (KyberNow Share, [kybernow-share.en.md](kybernow-share.en.md)): the guest types the same six-digit code, opens the link or scans the QR code, wherever they are. The host **accepts each guest** who comes from the internet. On the same network nothing changes: no approval needed.
- **Connecting to your own computer away from home**: your paired devices open the computer through the relay when they cannot find it on the network. Admin setting “Access away from home”, **off at first**.

This document is the common contract of the relay, Admin, the Rust core, the Link apps and the web client: the names it gives are final. The visible texts are in [link-ui.en.md](link-ui.en.md) (“Sharing” section, “Away from home”).

## In short

- The public relay is `share.kybernow.app:8443`, at the author's home. **A single port, 8443, over TCP and UDP.** It is open to every Station, with no bitrate cap. Anyone can install their own (Docker image, [server/relay/README.en.md](../server/relay/README.en.md)); the Station picks its relay in Admin.
- The relay does not see what the apps exchange with the Station: they talk to Admin over TLS **end to end** (Admin's certificate pinned), and to Kyber with its fingerprint. It sees addresses, bitrates and share codes. The **browser** is the exception: the relay presents a real certificate (`*.share.kybernow.app`) and decrypts that traffic to forward it.
- Kyber is not modified. The apps do not have to change kyclient: the core opens a tunnel and gives kyclient an address on the device's loopback (`127.0.0.1:L`, over TCP and UDP on the same number).
- Cost: one more round trip through the relay (close to nothing when the relay sits in the same house as the computer, 5 to 20 ms within France otherwise), under half a millisecond of processing, about thirty bytes per packet.

## The pieces

| Piece | Where | Role |
|---|---|---|
| Relay server | `server/relay` (Linux, Docker) | Receives Stations and devices, forwards the flows, reserves codes, serves the web page and the browser |
| Tunnel | `server/relay/tunnel` (crate `kybernow-tunnel`) | Messages, frames, Station identifier, QUIC and TLS transports, UDP flows; shared by the relay, Admin and the core |
| Station agent | Admin (`relay.rs`) | Holds the connection to the relay, serves in its own process what arrives through the relay, reaches Kyber through the local network address |
| Apps' tunnel | core (`apps/core/src/relay.rs`) | Opens the tunnel to a Station, exposes Admin and Kyber on the loopback |

## The tunnel

### Connections

Each end (the Station, an app) opens to the relay, on port 8443:

| Connection | Transport | ALPN | Content |
|---|---|---|---|
| Control | QUIC (UDP) | `kn-ctl/1` | The messages (first bidirectional stream), then one bidirectional QUIC stream per relayed TCP connection |
| Picture | QUIC (UDP) | `kn-media/1` | The relayed UDP packets, as QUIC datagrams |
| Fallback | TLS 1.3 over TCP | `kn-tcp/1` | Everything at once, multiplexed with yamux, when UDP does not get through |

- Name presented (SNI): the relay's name (`share.kybernow.app`). The relay presents its certificate; the apps and the Station check it against the web's usual authorities (`webpki-roots`), or by its fingerprint when the relay's address carries one (a relay installed at home, self-signed certificate): `relay.example.org:8443#sha256:ab:cd:…` (SHA-256 of the certificate, same form as Admin's fingerprint; the name is then not checked).
- QUIC: initial MTU 1280 bytes, discovery up to 1472. Keep-alive every 10 s for the Station, 5 s for an app; cut after 30 s without news.
- `kn-ctl/1` keeps the usual congestion control (Cubic). `kn-media/1` has a **permissive** controller: a very large window, no pacing, losses ignored; the datagram send queue is bounded (the oldest are dropped). Kyber already runs its own rate control inside.
- Order: `kn-ctl/1` first; `kn-media/1` after the `welcome` answer. If QUIC does not answer within 3 s, the end switches to the `kn-tcp/1` fallback, and retries QUIC every 5 minutes. The fallback shows (“Via the relay (TCP)”): fine for watching, less so for playing.

### Who is who

- **Station**: an Ed25519 key created by Admin on first use (`<data folder>/relay/station.key`, a folder protected like the rest of the data), and a self-signed certificate, presented as a **client certificate** on all three connections. **Station identifier**: the first 16 bytes of the SHA-256 of its public key (SPKI, DER), in lowercase base32 without padding (RFC 4648 alphabet `a-z2-7`), i.e. 26 characters, usable as a DNS label (`<id>.share.kybernow.app`). A reinstall that keeps the data keeps the identifier; a clean reinstall gives a new one.
- **App**: no certificate. It draws a **token** (32 random bytes, hexadecimal) for each tunnel and keeps it across reconnections. **Client identifier**: the first 10 bytes of the SHA-256 of `kn-client/1` followed by the token, in lowercase base32 (16 characters). The relay computes it and gives it to the Station: the same client keeps the same identifier after a reconnection, and nobody can take someone else's without their token. A new connection with the same identifier replaces the old one.
- **Browser**: the relay gives a random identifier to each TCP connection (16 characters).

### Frames and messages

A frame: a 4-byte length (big-endian, 65,536 at most), then a UTF-8 JSON object whose `t` field gives the type. An unknown type is ignored, so is an unknown field: we add without breaking. `hello.v` gives the protocol version (1); the relay answers with its own in `welcome.v`, or `error {code: "version"}` and closes.

The connecting end opens the first bidirectional stream of `kn-ctl/1` (or the first yamux stream): that is the message stream. `req` is a number chosen by the requester and returned in the answer.

| Message | Direction | Fields |
|---|---|---|
| `hello` | Station → relay | `v`, `role: "station"`, `version` (Admin), `admin_fp` (Admin's fingerprint), `sig` (the Station's signature over `admin_fp`, below), `codes` (codes to keep, after a reconnection) |
| `hello` | app → relay | `v`, `role: "client"`, `station` (identifier, or absent for a connection used only for `status`), `token` |
| `welcome` | relay → end | `v`, `id` (the Station's identifier, or the client's), `relay` (relay version); for an app: `online` (is the Station there); for the Station: `codes` (those from `hello` the relay kept; the others must be reserved again) |
| `error` | relay → end | `req` (when it is an answer), `code`: `version`, `bad_request`, `unknown_station`, `too_many`, `banned`, `replaced`, `no_code`, `not_bound`; optional `message` (logs, never shown as is) |
| `reserve` | Station → relay | `req`, `preferred` (wanted code, optional) |
| `reserved` | relay → Station | `req`, `code` (the wanted code if free, otherwise another) |
| `release` | Station → relay | `code` |
| `open_flow` | app → relay | `req`, `key` (the app's flow number, stable while its listening point lives) |
| `flow_open` | relay → Station | `flow`, `client`, `key`, `ip` (the client's public address) |
| `flow_accept` / `flow_refuse` | Station → relay | `flow`; `reason` for a refusal |
| `flow_ready` / `flow_refused` | relay → app | `req`, `flow`; `reason` for a refusal |
| `flow_close` | both ways | `flow` (the receiver's flow number) |
| `client_gone` | relay → Station | `client` (no connection left from that client) |
| `station` | relay → app | `online` (the Station leaves or comes back) |
| `stats` | relay → app | `rtt_station_ms` (relay ↔ Station round trip), every 5 s |
| `status` | app → relay | `req`, `stations` (identifiers, 16 at most) |
| `presence` | relay → app | `req`, `online` (one boolean per identifier) |
| `ping` / `pong` | both ways | `n`; keep-alive and measurement over the TCP fallback |

**Signature**: `sig` is the Ed25519 signature, by the Station's key, of the text `kybernow-relay/1 admin_fp ` followed by `admin_fp`, in base64. The public key is the client certificate's: the relay hands it back (`station_key`, SPKI DER, base64) with the code, and any device that knows the Station's identifier checks that the key matches it and that the signature holds.

### Relayed TCP connections

- The app opens a bidirectional stream on `kn-ctl/1` (after `welcome`), writes a frame `{"t":"open","service":"admin_tls"|"kyber_tcp"}`, then its bytes.
- The relay opens a stream on the Station's connection and first writes the **header**, a frame only the relay writes (an app cannot forge it): `{"t":"stream","v":1,"kind":…,"client":"<id>","ip":"<public address>","host":null|"<requested name>"}`. Then it copies both ways; closing one side closes the other (half-close), a reset resets both.
- `kind`: `admin_tls` (an app to Admin, TLS end to end), `admin_http` (a browser, decrypted by the relay, plain HTTP/1.1 inside the tunnel, `host` = the requested name), `kyber_tcp` (kyclient to Kyber: HTTPS control, WebSocket).
- Station absent: the relay resets the app's stream.

### Relayed UDP packets

- Opening: the app sends `open_flow {req, key}`; the relay passes `flow_open` to the Station; Admin accepts (authorised client, see “Admin's guard”) or refuses; the app receives `flow_ready {req, flow}` or `flow_refused`. Each end has its own flow numbers; the relay translates them.
- Datagram on `kn-media/1`: `[flow number, QUIC variable-length integer][UDP packet]`.
- **Size rule**: a packet that does not fit in a datagram (`max_datagram_size`) goes, if it is 1200 bytes or less, into a backup unidirectional stream of `kn-media/1` (one packet per stream, same form); larger, it is dropped (Kyber's MTU discovery comes back down on its own).
- The `kn-media/1` connection starts with a unidirectional stream opened by the connecting end, a frame `{"t":"media"}` (Station, recognised by its certificate) or `{"t":"media","token":"…"}` (app).
- On the Station side, the agent holds for each flow a UDP socket to Kyber, kept for 25 s after a cut: a client that comes back with the same identifier and the same `key` gets the same socket, and Kyber sees no new connection.

### TCP fallback (`kn-tcp/1`)

TLS 1.3 over TCP 8443, ALPN `kn-tcp/1`, client certificate for the Station; yamux on top (the connecting end is the yamux client). First stream: the messages (as on `kn-ctl/1`). Second stream, opened by the connecting end after `welcome`: the datagrams, a `{"t":"media"…}` frame as above then records `[length of the rest, 2 bytes big-endian][flow number][UDP packet]`. Other streams: the relayed TCP connections, as over QUIC. A Station on the fallback and an app on QUIC (or the reverse) talk normally.

### Reconnection

- The Station is connected to the relay only when a share is open or “Access away from home” is on. Attempts after 1, 2, 4, 8, 16 then 30 s (±20%). At each reconnection: `hello` with the current codes; the relay keeps a departed Station's codes for 120 s.
- QUIC follows address changes (Wi-Fi to 4G) without reconnecting.
- One connection per Station: the last one wins, the old one receives `error {code: "replaced"}`.

## The relay on the web

On TCP 8443, the relay looks at the start of each TLS connection:

- ALPN `kn-tcp/1`: the tunnel's fallback;
- name `share.kybernow.app`: its pages;
- name `<id>.share.kybernow.app`: the browser to Station `<id>`;
- any other name: connection closed.

| Address | Answer |
|---|---|
| `GET /` | Home page and code entry (French or English depending on the browser) |
| `GET /j/{code}` | Redirects (302) to `https://<id>.share.kybernow.app:8443/link/?join=<code>`; unknown code, or Station absent: a page that says so |
| `GET /v1/codes/{code}` | `200 {"station":"<id>","station_key":"<SPKI base64>","admin_fp":"sha256:…","sig":"<base64>","online":true}`; `404 {"error":"unknown_code"}`; `400 {"error":"bad_request"}`; `429 {"error":"too_many","retry_after":<s>}` with `Retry-After` |
| `GET /v1/health` | `200 {"ok":true,"version":"…"}`; the iPhone and iPad apps call it before opening the tunnel (wakes the cellular radio) |
| `https://<id>.share…:8443/*` | HTTP/1.1 proxy to the Station's Admin (WebSocket included); Station absent: “computer unreachable” page (503) |

Proxy: `Host` passed as is, `X-Forwarded-For` and `Forwarded` removed (the browser's address travels in the stream header), `Domain=` removed from every `Set-Cookie`. One origin per Station: one Station's code never reads another's cookies.

Codes: the relay **reserves codes**, unique across all the Stations connected to it. Four codes at most per Station. The relay's logs never contain a code.

## Admin

### Settings

| Route | Content |
|---|---|
| `GET /api/relay` | `{"server":"share.kybernow.app:8443","remote_access":false,"id":"<identifier>","state":"off"|"connecting"|"joined"|"unreachable","reason":null|"<text>","transport":null|"quic"|"tcp","locked_until":null|<Unix seconds>,"sessions":[{"device":"<name>","since":<Unix seconds>}]}` |
| `PUT /api/relay` | `{"server":"…","remote_access":true}` (each field optional) → same answer |

Admin session only, never through the relay. Empty `server`: no relay (neither sharing away from home nor access). `state`: `off` (nothing needs the relay: no open share and access off), `connecting`, `joined`, `unreachable` with `reason` (in Admin's language). Default: `share.kybernow.app:8443`, access off. The installer keeps the data's `relay\` folder (same identifier after an update or a reinstall that keeps the data).

### `info`

`GET /api/stream/info` adds `"relay":{"server":"share.kybernow.app:8443","id":"<identifier>","remote_access":true}` when a relay is set (absent otherwise, and for an earlier Station). The app keeps it for each computer.

### Sharing

- **Share view** (`open`, `GET /api/share`, `code`, the host's heartbeat, local routes):
  - `relay`: `null` (no relay) or `{"state":"joined"|"unreachable"}`;
  - relay reached: `link` also carries `&relay=<relay>&sid=<identifier>`, and `web` becomes `https://share.kybernow.app:8443/j/<code>`; `web_local` always keeps the address on the network (`https://<address>:<port>/link/?join=<code>`);
  - `guests[]` gains `pending` (true for a request not yet accepted) and `via` (`"lan"` or `"relay"`). A request is a spectator, `since` is the time of the request; it takes no place until accepted.
- **Codes**: with the relay reached, Admin no longer draws the code itself: it asks `reserve` (“New code”: `reserve` then `release` of the old one). Relay unreachable: code drawn by Admin, offered to the relay on connection (replaced only if taken). The code always works on the network, without approval.
- **Joining through the relay**: `POST /api/share/join` (same body) → `202 {"pending":true,"guest":"<id>","secret":"<secret>","expires":120}`. No host name yet: it is given only after approval. Refusals on top of the share's: `409 {"error":"pending_full"}` (three pending requests at most), `403 {"error":"refused"}` (device refused less than 10 minutes ago).
- **Waiting**: `POST /api/share/wait`, header `x-kybernow-guest: <secret>`, `{"guest":"<id>"}`, answer held for 25 s at most: `202 {"pending":true}` (still waiting, call again), `200` (the answer of `join` on the network, with `"relay":true`), `403 {"error":"refused"}`, `410 {"error":"expired"}` (no answer within 120 s), `404 {"error":"no_share"}` (share stopped). `POST /api/share/leave` cancels a request.
- **Accept, decline**: `POST /api/share/guest/{id}/accept`, `POST /api/share/guest/{id}/refuse` (host), and `/api/local/share/guest/{id}/accept|refuse` (the PC) → the share view. Places are counted on acceptance: `409 {"error":"full"}` if all four are taken.
- The answer of `join` and `wait` carries `"relay":true` through the relay, and so does `GET /api/session`: the web client then takes the picture over WebSocket (reason `relay`), never WebTransport.
- Browser cookie through the relay: `__Host-kybernow_guest` (`Secure`, `Path=/`, `SameSite=Strict`, `HttpOnly`, no `Domain`); Admin reads both names.

### Activity feed

“{device} is in a session away from home” when a session through the relay starts; “Access away from home blocked for 10 min after too many attempts.” when blocking ([link-ui.en.md](link-ui.en.md)).

## Admin's guard by route

Admin considers “local” what comes from the loopback (`/api/local/*`, `local_or_admin`). The relay must never pass for local:

- The agent **serves itself** what arrives through the relay, inside Admin's process (`admin_tls`: TLS on Admin's certificate, then HTTP; `admin_http`: HTTP), never by reconnecting over TCP to `127.0.0.1`. The peer address is the one given by the stream header, **forced outside the loopback and private addresses**, and the request carries the route (`Via::Relay {kind, client}`).
- A separate `Router`, `relay_guard`, only opens:
  - to a **guest**, when a share is open: `/api/share/join|wait|beat|quality|leave`, `/link/*`, `/assets/*`, a reduced `GET /api/session`, Admin's proxy to Kyber (`kyber_proxy`) with an accepted guest's cookie (WebSocket only, never `/kymux/start_mux`);
  - to a **paired device** (Basic account), only when “Access away from home” is on: `/api/stream/*` and the host routes `/api/share*`; access off: `403 {"error":…,"code":"remote_access_off"}`;
  - **never** through the relay: `/api/login`, `/api/setup`, `/api/password`, `/api/web/ticket`, `/api/pair*`, `/api/pairing`, `/api/local/*`, configuration, accounts, logs, service, display, sound, Admin's update, devices, apps, `/api/events`, Admin's interface. Pairing happens on the network only.
- **Blocking**: 20 Basic account failures through the relay within 10 minutes block access through the relay for 10 minutes (`429 {"code":"locked"}`), with a line in the feed.
- **Kyber**: the agent reaches Kyber only for an authorised client (an accepted guest, while it keeps sending signs of life; a paired device after `prepare`, for 30 s renewed by the heartbeat), and always **from and to the computer's local network address**, never through the loopback (patch 0019 keeps `/kybernow/*` for the loopback). Kyber listens on all addresses.
- Admin authorises a **relay client** (its identifier): the one that made the successful `wait`, `beat` or `prepare`/`heartbeat`. A client that reconnects keeps its identifier.

## Links

- Apps: `kybernow://join?host=<address>&port=<Admin port>&fp=<Admin fingerprint>&code=<6 digits>&relay=<relay>&sid=<identifier>` (with `relay` and `sid` when the relay is reached; the relay keeps its possible `#sha256:…`, encoded). The app tries the network first (`host`, `port`), then the relay to `sid`, Admin pinned on `fp`.
- Browser: `https://share.kybernow.app:8443/j/<code>`, which redirects to `https://<id>.share.kybernow.app:8443/link/?join=<code>`. No universal link on iPhone (it would need port 443): the page offers “Open in the app”.
- QR code: the apps' link.

## Joining with a code, away from home

1. The app sends the code to the Stations on the network (as today) **and at the same time** asks `GET /v1/codes/{code}` from the default relay and from the relays of the computers it knows (`info.relay.server`).
2. A Station on the network answers within 1.5 s: the guest joins as today. Otherwise, if the relay knows the code: tunnel to `station`, `POST /api/share/join` end to end, Admin pinned on `admin_fp` (and `sig` checked when the app knows `sid`).
3. “Waiting for the host to accept…”: `POST /api/share/wait` until `200`, `403` or `410`. “Cancel”: `leave`.
4. An ordinary guest session, through the relay: Admin through the tunnel, kyclient on `127.0.0.1:L` (the answer's `kyber.port` replaced by L).

## Connecting to your computer, away from home

- `Route::Auto`: the app asks `info` directly (1.5 s); on a network failure, it goes through the relay (`info.relay` kept). **Fingerprint changed: never a switch** to the relay (the fingerprint error stays).
- Host session through the relay: `prepare`, `heartbeat`, `end`, `foreground` with Admin through the tunnel; kyclient on `127.0.0.1:L` for `prepare`'s `kyber_port`. Through the relay, Admin's and Kyber's fingerprints are mandatory: never trust on first use.
- Presence on the home screen: `status` to the relay; a computer reachable only through the relay is “Away from home”.
- During a session, it stays on the relay until the end.

## Core and apps

- `apps/core/src/relay.rs` (its own tokio runtime, two threads): `relay::Tunnel::open(&RelayRoute {server, station, admin_fp})`, `admin_addr()`, `kyber_addr(port)`, `stats()`; one tunnel per (relay, Station), closed after 30 s unused; `lookup(&relay, code)`, `presence`. Sharing: `share::join_anywhere`, `share::join_link`, `share::wait`, `share::cancel` ([apps/core/README.en.md](../apps/core/README.en.md), “Relay” section).
- `SessionParams::route`: `Route::Direct`, `Route::Relay(RelayRoute)`, `Route::Auto(Option<RelayRoute>)`; `Console::with_relay(Option<RelayRoute>)`.
- Indicator: `Platform::on_route(&RouteInfo {transport: direct|relay|relay_tcp, relay, rtt_client, rtt_station})`; “Via the relay” chip, “Path” line in the statistics, `route` in “Last session”.
- `KYBERNOW_RELAY=force` (always through the relay, to try it at home) or `off`.
- Listening points: always on `127.0.0.1`, never `0.0.0.0`; TCP bound on port 0, then UDP on the same number (try again if taken). iPhone, iPad, Apple TV: no “Local Network” permission for the loopback; the sockets are recreated on coming back to the foreground (port changed: the session restarts).
- Bridges: `kn_relay_*`, `kn_share_wait`; JNI `Native.relay*`, `Native.shareWait`.

## Trust

- **The relay** is checked by its certificate (web authorities, or pinned fingerprint).
- **Typed code**: the relay gives Admin's fingerprint. A lying relay could get in the middle; that is the price of a six-digit code. The host sees and accepts each guest.
- **Link and QR code**: `fp` and `sid` come from the Station; the exchange is end to end, the relay cannot get in the middle.
- **Paired devices**: fingerprints pinned at pairing (on the network). Through the relay, never trust on first use.
- **Browser**: the relay decrypts (real `*.share.kybernow.app` certificate). The apps stay encrypted end to end.
- The public relay is in France; it keeps IP addresses in its logs (7 days), without codes. Privacy page on the relay.

## Relay limits and protections

- Code lookup: 10 per minute and 60 per hour per address, 30 per minute per /24 (IPv4) or /48 (IPv6), 600 per minute overall. Admin keeps its 10 `join` attempts per minute per real address.
- QUIC: 16 connections per address, 60 handshakes per minute per address, 4096 overall; beyond a threshold, the relay asks for address validation (Retry).
- TCP: 32 connections per address; TLS and HTTP headers within 10 s, 16 KiB of headers at most.
- 4 Stations per address, one connection per Station (the last wins), 8 clients per Station, 4 UDP flows per client.
- No bitrate cap. The relay only forwards to connected Stations, and Admin only lets authorised clients through. Ban list reloaded live; optional global cap, off by default; Prometheus metrics and a bitrate alert.

## Installing a relay at home

A Linux machine with Docker, a domain name whose DNS zone accepts an API token (Gandi by default, for the wildcard certificate), port 8443 open over TCP **and** UDP. The step by step is in [server/relay/README.en.md](../server/relay/README.en.md). In each Station's Admin: “Away from home › Relay” = `relay.example.org:8443`.

## Later

- The browser's picture over WebTransport through the relay (except Safari): the relay would read the name in the first QUIC packet and forward without decrypting.
- Fair sharing of the home's uplink between several guests.
