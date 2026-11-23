# The KyberNow relay

English · [Français](README.md)

The relay carries KyberNow over the internet when your device is not on the computer's network: sharing your screen with someone who is elsewhere, or reaching your own computer from a hotel or a train. The contract (tunnel, Admin routes, links, trust) is in [docs/relais.en.md](../../docs/relais.en.md).

The public relay, `share.kybernow.app`, is open to every Station. You can also install your own: a Linux machine with Docker, a domain name, and a single open port, **over TCP and UDP** (443 preferably, see “Ports and the router”).

| File | Contents |
|---|---|
| `src/` | The relay server (`kybernow-relay`): QUIC and TCP connections, Stations and clients, codes, web pages (`pages.rs`, `assets.rs`), accounts (`accounts/`), browser proxy, metrics |
| `assets/` | What the pages embed: stylesheet (`share.css`, written by hand, with the brand tokens `kybernow.css`), Archivo and Martian Mono fonts reduced to Latin, icons, the KyberNow Share logo and favicon |
| `tunnel/` | The tunnel crate (`kybernow-tunnel`), shared with Admin and the apps' core |
| `Dockerfile` | The image: built with Rust 1.89, then the binary alone on distroless, without root |
| `docker-compose.yml` | The relay and lego, which obtains and renews the certificate |
| `lego.sh` | What lego runs: certificate obtained, then renewed every 12 hours when needed |
| `.env.example`, `relay.toml.example` | The settings to copy |

## Installing a relay at home

### 1. What you need

- A Linux machine that stays on, with Docker and `docker compose`.
- A domain name whose DNS zone can be managed through an API: Gandi by default (the certificate covers `*.relay.example.org`, which requires a DNS proof). Another DNS host supported by lego works too: change `--dns gandiv5` in `lego.sh` and the token variable in `docker-compose.yml`.
- One port open from outside to this machine, **over TCP and UDP**: 443 preferably, otherwise one of your choice. UDP carries the picture; without it, the relay falls back to TCP (fine for watching, less so for playing).

### 2. DNS

Two records pointing to your public IPv4 address:

| Name | Type | Value |
|---|---|---|
| `relay.example.org` | A | your public address |
| `*.relay.example.org` | A | the same address |

Only add an AAAA if the machine really is reachable over IPv6 on the public port. The wildcard name is for the browser: each computer gets its own address, `https://<identifier>.relay.example.org` (with `:<port>` if the public port is not 443).

### 3. Ports and the router

Three ports follow one another, over TCP **and** UDP:

- **the listening port** of the relay on the machine: `RELAY_LISTEN` in `.env`, `0.0.0.0:8443` by default. The image runs without root: it stays above 1024;
- **the public port**, the one devices reach from the internet: `RELAY_PUBLIC_PORT`, the listening port by default. The relay writes it in its links (`/j/<code>` sends to `https://<identifier>.<name>[:port]/link/…`); at 443, addresses carry no port;
- **the router** links the two: it forwards the public port to the machine's listening port.

| Router | `.env` | Address given to Admin | Links |
|---|---|---|---|
| 443 outside → 8443 on the machine (the public relay) | `RELAY_PUBLIC_PORT=443` | `relay.example.org` | `https://relay.example.org/j/123456` |
| 9443 outside → 9443 on the machine | `RELAY_LISTEN=0.0.0.0:9443` | `relay.example.org:9443` | `https://relay.example.org:9443/j/123456` |
| 8443 outside → 8443 on the machine | nothing | `relay.example.org:8443` | `https://relay.example.org:8443/j/123456` |

443 is the simplest for those who share: nothing to remember, and networks that only let the web out let it through. A relay address without a port means 443, in Admin as in the apps.

If devices at home cannot reach the relay by its name (the router does not loop back to the local network), have your local DNS (Pi-hole, the router) give the machine's local address for `relay.example.org` and `*.relay.example.org`; they then reach the listening port directly, which must therefore be the same as the public port, or the router must translate it for them too.

### 4. The Gandi token

In your Gandi account, create a personal access token limited to your domain, allowed to manage its technical configuration. Note its expiry date: the certificate stops renewing after it. Write it to a file, nowhere else:

```sh
mkdir -p ~/kybernow-relay/secrets ~/kybernow-relay/certs ~/kybernow-relay/config
cd ~/kybernow-relay
nano secrets/gandi_pat      # the token alone, on one line
chmod 600 secrets/gandi_pat
```

### 5. The files

The relay image is published with every KyberNow version (`ghcr.io/monsieurnours/kybernow-relay`): there is nothing to compile. You only need four files from this folder:

```sh
mkdir -p ~/kybernow-relay && cd ~/kybernow-relay
for f in docker-compose.yml lego.sh update.sh .env.example; do curl -fsSLO https://raw.githubusercontent.com/MonsieurNours/kybernow/main/server/relay/$f; done
cp .env.example .env
nano .env                   # RELAY_DOMAIN, LEGO_EMAIL, and the ports (“Ports and the router”)
```

The relay receives a lot of UDP packets: give it large buffers (once, as root):

```sh
printf 'net.core.rmem_max=8388608\nnet.core.wmem_max=8388608\n' | sudo tee /etc/sysctl.d/90-kybernow-relay.conf
sudo sysctl --system
ss -ltnup | grep 8443       # nothing should already listen on the listening port (8443 by default)
```

### 6. The certificate, staging first

Read Let's Encrypt's terms (https://letsencrypt.org/repository/), set `LEGO_ACCEPT_TOS=yes` in `.env`, then:

```sh
docker compose up -d
docker compose logs -f lego
```

`LEGO_SERVER` first points to Let's Encrypt's staging server: its certificate is trusted by no browser, but it proves that DNS and the token work, without any risk of being rate-limited. Once `certs/certificates/<domain>.crt` is there, switch to production:

```sh
nano .env                   # LEGO_SERVER=https://acme-v02.api.letsencrypt.org/directory
rm -rf certs/certificates
docker compose up -d lego
```

The relay rereads the certificate on its own (every 30 seconds, if it changed), without restarting. lego renews it 30 days before it expires.

### 7. In Admin

On each Station: Admin › Link devices › “Away from home” › “Relay” = the relay's address with its public port: `relay.example.org` for 443, `relay.example.org:9443` for another one. The share panel then says “On the same network or elsewhere…”, and the links carry your relay's address. A code typed in an app goes through the public relay and the relays of computers the app already knows; for a guest who does not know your relay, give the link or the QR code.

Without a real certificate (self-signed relay), the apps work if the address carries the certificate's fingerprint (`relay.example.org:9443#sha256:ab:cd:…`); the browser, though, needs a real certificate.

### Updating

**By itself**: the compose's `updater` service pulls the latest relay image every night (at 4 am UTC, `RELAY_UPDATE_HOUR` in `.env`) and only restarts the relay if it changed. `RELAY_AUTO_UPDATE=no` turns it off. The certificate and settings are not touched. Run `docker compose` from the relay's folder: the service sees it at the same path as the machine.

**By hand**, whenever you like:

```sh
docker compose pull
docker compose up -d
```

When a version changes the compose file itself (rare, the changelog says so), fetch the files again (`for f in docker-compose.yml lego.sh update.sh; do curl -fsSLO https://raw.githubusercontent.com/MonsieurNours/kybernow/main/server/relay/$f; done`) before these two commands.

**From source**, without the published image: clone the repository and, in `server/relay`, `docker compose build relay && docker compose up -d`. The image is built with the machine's network (`network: host`): behind a local DNS (Pi-hole, unbound), Docker's build network does not resolve names. `RELAY_IMAGE` in `.env` picks another image (a specific version, `…:0.8.0`).

## Settings

Environment variables (`docker-compose.yml`) take precedence over `config/relay.toml` (template: `relay.toml.example`).

| Variable | Default | Role |
|---|---|---|
| `KN_RELAY_DOMAIN` | `share.kybernow.app` | The relay's name; Stations are served under `<id>.<name>` |
| `KN_RELAY_LISTEN` (`RELAY_LISTEN` in `.env`) | `0.0.0.0:8443` | Listening address, TCP and UDP |
| `KN_RELAY_PUBLIC_PORT` (`RELAY_PUBLIC_PORT`) | the listening port | Announced public port, when the router translates it: the one in `/j/<code>` links and `https://<id>.<name>` addresses, without a port when it is 443 |
| `KN_RELAY_CERT`, `KN_RELAY_KEY` | `/certs/certificates/<name>.crt` and `.key` | lego's certificate |
| `KN_RELAY_BANLIST` | none | Ban list, reloaded live |
| `KN_RELAY_METRICS` | `127.0.0.1:9464` | Metrics and `/healthz` (`off` to turn off; the image's health check needs it) |
| `KN_RELAY_EGRESS_ALERT_MBPS` | `0` | Warning in the log above this bitrate |
| `KN_RELAY_GLOBAL_CAP_MBPS` | `0` | Global cap on relayed packets (`0`: none) |
| `KN_RELAY_CONFIG` | `/config/relay.toml` | The settings file |

The limits (`[limits]` in `relay.toml`) follow [docs/relais.en.md](../../docs/relais.en.md#relay-limits-and-protections): code lookups (10 per minute and 60 per hour per address, 30 per minute per /24 or /48, 600 per minute overall), QUIC connections (16 per address, 60 handshakes per minute, 4096 overall, address validation above 100 handshakes per second), TCP (32 per address, TLS and headers within 10 s, 16 KiB of headers), 4 Stations per address, 20 Station connections per minute per address, 8 clients per Station with 2 per address (`clients_per_station_per_ip`), a client sent away after 30 s with no flow or connection (`client_idle_secs`) and, at a full Station, the oldest idle client giving its seat, 4 UDP flows per client, 4 codes per Station. No bitrate cap by default.

## Accounts

KyberNow accounts ([docs/comptes.en.md](../../docs/comptes.en.md), batch 1): sign-up (with the “terms and privacy” box), address verification (link valid for 24 h or six-digit code), sign-in, two-factor authentication (authenticator app, recovery codes, “Don't ask again on this device for 30 days”), passkeys, forgotten password, and the account page (`/compte`: address, security, passkeys, open sessions, data export, account deletion). **Off by default**: without them, `/compte` is a not-found page.

The pages are the relay's (`https://<name>/compte/connexion`, `/compte/inscription`, `/compte/verifier`, `/compte/proteger`, `/compte/code`, `/compte/oubli`, `/compte/nouveau-mot-de-passe`, `/compte`, `/compte/supprimer`), in French and English; they are the only ones taking POST (forms of 16 KiB at most). They have a single script, `/assets/passkey.js`, served by the relay and allowed on these pages only (`script-src 'self'`): it calls WebAuthn and sends the answer through the page's form. Without it, the passkey buttons don't show and everything else works.

With accounts, the relay also serves `/conditions` and `/confidentialite`: the service's terms of use and privacy policy ([docs/juridique](../../docs/juridique/README.en.md)), **drafts** marked as such, copied into `assets/legal/` (a test checks the copies follow `docs/juridique`). These are the public relay's texts: a relay run by someone else must replace them with its own. Without accounts, `/confidentialite` stays the `/privacy` page.

```sh
mkdir -p data && sudo chown 65532:65532 data      # the database and key, owned by the image's user
nano config/relay.toml                             # [accounts] enabled = true (template: relay.toml.example)
docker compose up -d relay
```

- **Database**: SQLite, `/data/accounts.db` (`database`), migrations at start-up. Back up the `data/` folder along with the key.
- **Key**: `/data/secret.key` (`secret_key`), created at first start (64 hex digits, readable by the relay only). It encrypts the two-factor secrets (AES-256-GCM) and signs the cookies: losing it turns two-factor authentication off for every account.
- **Email**: `[accounts.smtp]` (`host`, `port`, `starttls`, `user`, `password_file`, `from`), over TLS (rustls). The password goes in a file (`config/smtp_password`, `chown 65532` then `chmod 600`). Without SMTP, no sign-up, no verification and no forgotten password: the administrator creates the accounts, already verified:

  ```sh
  docker compose exec relay kybernow-relay accounts add ana@example.org --name Ana
  # then type the password (visible); or --password-file /config/password
  ```

- **Passkeys** (WebAuthn): verified by the relay on `ring` (ES256, Ed25519, RS256; no attestation requested; user verification required, so a passkey counts as two-factor authentication). The relay's WebAuthn identifier is the host name of `public_url` and the expected origin is `public_url`: a passkey created on `share.kybernow.app` only works there. Challenges signed with the relay's key, bound to the browser, valid for 10 minutes and single-use; at most 20 passkeys per account; an email on every addition.
- **Account data**: “Export my data” gives a JSON file (`kybernow-compte.json`: account, accepted terms, security without secrets, sessions, attached computers, connected devices, log). “Delete the account” asks again for the password or a passkey, then erases the account and everything that belongs to it (its computers are detached, its devices disconnected); the backups of the `data/` folder must rotate within 30 days. An account never verified is erased seven days after sign-up. The version and date of the terms and privacy policy accepted are kept on each account.
- **Security**: `__Host-kn_session` cookie (`Secure`, `HttpOnly`, `SameSite=Lax`), 256-bit token stored as a hash; signed CSRF token in every form, forms from another site (a Station's Admin included) refused; Argon2id passwords (at least 10 characters, known passwords refused); 10 sign-in attempts per IP address and 5 per account in 15 minutes (same for codes; only failures count); identical answers whether an account exists or not. The logs hold neither email addresses nor tokens; the accounts' audit log (in the database) erases IP addresses after 7 days.
- **Settings from the environment**: `KN_RELAY_ACCOUNTS` (`on` or `off`, `RELAY_ACCOUNTS` in `.env`), `KN_RELAY_ACCOUNTS_DATABASE`, `KN_RELAY_ACCOUNTS_SECRET_KEY`, `KN_RELAY_ACCOUNTS_PUBLIC_URL`, `KN_RELAY_SMTP_HOST`, `KN_RELAY_SMTP_PORT`, `KN_RELAY_SMTP_STARTTLS`, `KN_RELAY_SMTP_USER`, `KN_RELAY_SMTP_PASSWORD_FILE`, `KN_RELAY_SMTP_FROM`.

## Operations

- **Logs**: `docker compose logs relay`, rotated by Docker (20 MB × 5). They keep Stations' IP addresses and connection times, never a share code nor a page path. `RUST_LOG=debug` says more.
- **Metrics** (Prometheus, `http://127.0.0.1:9464/metrics`): `relay_stations_connected{transport}`, `relay_clients_connected{kind}`, `relay_sessions_active`, `relay_bytes_total{leg,dir}`, `relay_datagrams_total{result}`, `relay_code_lookups_total{result}`, `relay_codes_active`, `relay_handshakes_total{alpn,result}`, `relay_rtt_seconds{peer}`, `relay_limit_hits_total{limit}`, `relay_tls_cert_expiry_seconds`, `relay_egress_mbps`. An alert on `relay_tls_cert_expiry_seconds < 1209600` (14 days) warns of an expired Gandi token.
- **Banning**: one address or network per line in `config/banlist.txt` (`203.0.113.0/24`, `#` for a comment); taken into account within 30 seconds.
- **Health**: `docker compose ps` shows the health check's state (`kybernow-relay --healthcheck`).
- **Pages**: `https://<name>/` (code entry; `https://<name>:<port>/` other than 443), `/j/<code>`, `/privacy`, `/assets/…`, `/v1/health`; with accounts, `/compte/…`, `/conditions` and `/confidentialite`.

## The home page

`https://<name>/` keeps only the essentials: the KyberNow Share logo, the “Join with a code” field (it sends `/j?code=…`, which redirects to the computer's page or says why not), a “Discover KyberNow” link to the site and the privacy page. The site presents KyberNow; the links lead to `https://www.kybernow.app/`, in French or English depending on the page. The texts follow [docs/ton-du-site.en.md](../../docs/ton-du-site.en.md), with no list of platforms.

- **Language**: French if the browser is set to French, English otherwise (`Accept-Language`); `?lang=en` or `?lang=fr` forces the language (the “EN” or “FR” link of the other pages), with no cookie, and the form keeps it through to `/j`.
- **Files**: everything comes from the relay, nothing from a third party. `/assets/share.css` (tokens and layout, a single stylesheet), `/assets/archivo.woff2`, `/assets/martian-mono.woff2`, `/assets/kybernow-share-dark.svg` and `/assets/kybernow-favicon.svg` are embedded in the binary (`include_bytes!`, about 95 KB more); the icons are written inline in the page. Every address carries the files' version (`?v=`, eight digits of a SHA-256 of their content): the response is kept for a year (`immutable`), and a changed file changes address. An unknown address gives the “not found” page.
- **Content policy**: `default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'`. No script, no cookie, no audience measurement, no request to another site. The pages stay `noindex`.
- **Ports**: the page never writes the relay's address (relative paths); it is the same on the listening port and on the public port (`https://<name>` without a port at 443).
- **Rebuilding the files**: `share.css` is written by hand. The others come from the brand files: the logo and favicon by `node brand/v2/tools/generate.mjs`, and the rest (`kybernow.css`, fonts reduced to Latin, icons from `site/assets/icons.svg`, licences) by `scripts/relay-assets.py`, which needs `fonttools` and `brotli` (`python3 -m venv /tmp/fonts && /tmp/fonts/bin/pip install fonttools brotli`). The Docker image only sees this folder: the copies are in the repository.

## Development

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-features --all-targets
```

The tests run on the loopback, without network: a test authority, a relay, a fake Station (the tunnel agent, a fake Admin, TCP and UDP echoes) and a fake app (the tunnel and its local listening points). They cover QUIC, the TCP fallback (forced, and when UDP does not get through), a Station over TCP with an app over QUIC, Admin's refusals, the Station absent then connected, the Station and the relay restarting, codes and waiting with a fake Admin, the relay's home page and its files (languages, content policy, caching), the browser proxy (headers, cookies, WebSocket), the limits and the accounts (sign-up, verification, sign-in, two-factor authentication, recovery codes, forgotten password, attempt limits, CSRF, without SMTP, passkeys with a software authenticator, export, deletion, accepted terms, erasure of never-verified accounts).

The `tunnel/` crate is taken by path: `features = ["station"]` for Admin, the default `client` feature for the apps' core, `server` for the relay. It builds for iOS and Android (rustls with ring, no OpenSSL).
