# The KyberNow relay

English · [Français](README.md)

The relay carries KyberNow over the internet when your device is not on the computer's network: sharing your screen with someone who is elsewhere, or reaching your own computer from a hotel or a train. The contract (tunnel, Admin routes, links, trust) is in [docs/relais.en.md](../../docs/relais.en.md).

The public relay, `share.kybernow.app:8443`, is open to every Station. You can also install your own: a Linux machine with Docker, a domain name, and a single open port, **8443 over TCP and UDP**.

| File | Contents |
|---|---|
| `src/` | The relay server (`kybernow-relay`): QUIC and TCP connections, Stations and clients, codes, web page, browser proxy, metrics |
| `tunnel/` | The tunnel crate (`kybernow-tunnel`), shared with Admin and the apps' core |
| `Dockerfile` | The image: built with Rust 1.89, then the binary alone on distroless, without root |
| `docker-compose.yml` | The relay and lego, which obtains and renews the certificate |
| `lego.sh` | What lego runs: certificate obtained, then renewed every 12 hours when needed |
| `.env.example`, `relay.toml.example` | The settings to copy |

## Installing a relay at home

### 1. What you need

- A Linux machine that stays on, with Docker and `docker compose`.
- A domain name whose DNS zone can be managed through an API: Gandi by default (the certificate covers `*.relay.example.org`, which requires a DNS proof). Another DNS host supported by lego works too: change `--dns gandiv5` in `lego.sh` and the token variable in `docker-compose.yml`.
- Port 8443 open from outside to this machine, **over TCP and UDP**. UDP carries the picture; without it, the relay falls back to TCP (fine for watching, less so for playing).

### 2. DNS

Two records pointing to your public IPv4 address:

| Name | Type | Value |
|---|---|---|
| `relay.example.org` | A | your public address |
| `*.relay.example.org` | A | the same address |

Only add an AAAA if the machine really is reachable over IPv6 on port 8443. The wildcard name is for the browser: each computer gets its own address, `https://<identifier>.relay.example.org:8443`.

### 3. The router

Forward port 8443 over TCP **and** UDP to the relay machine. If devices at home cannot reach the relay by its name (the router does not loop back to the local network), have your local DNS (Pi-hole, the router) give the machine's local address for `relay.example.org` and `*.relay.example.org`.

### 4. The Gandi token

In your Gandi account, create a personal access token limited to your domain, allowed to manage its technical configuration. Note its expiry date: the certificate stops renewing after it. Write it to a file, nowhere else:

```sh
mkdir -p ~/kybernow-relay/secrets ~/kybernow-relay/certs ~/kybernow-relay/config
cd ~/kybernow-relay
nano secrets/gandi_pat      # the token alone, on one line
chmod 600 secrets/gandi_pat
```

### 5. The files

Copy this folder's contents into `~/kybernow-relay` (or clone the repository and work in `server/relay`), then:

```sh
cp .env.example .env
nano .env                   # RELAY_DOMAIN, LEGO_EMAIL
```

The relay receives a lot of UDP packets: give it large buffers (once, as root):

```sh
printf 'net.core.rmem_max=8388608\nnet.core.wmem_max=8388608\n' | sudo tee /etc/sysctl.d/90-kybernow-relay.conf
sudo sysctl --system
ss -ltnup | grep 8443       # nothing should already listen on 8443
```

### 6. The certificate, staging first

Read Let's Encrypt's terms (https://letsencrypt.org/repository/), set `LEGO_ACCEPT_TOS=yes` in `.env`, then:

```sh
docker compose up -d --build
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

On each Station: Admin › Link devices › “Away from home” › “Relay” = `relay.example.org:8443`. The share panel then says “On the same network or elsewhere…”, and the links carry your relay's address. A code typed in an app goes through the public relay and the relays of computers the app already knows; for a guest who does not know your relay, give the link or the QR code.

Without a real certificate (self-signed relay), the apps work if the address carries the certificate's fingerprint (`relay.example.org:8443#sha256:ab:cd:…`); the browser, though, needs a real certificate.

### Updating

```sh
git pull                    # or copy the new files
docker compose up -d --build
```

The image is built with the machine's network (`network: host` in `docker-compose.yml`): behind a local DNS (Pi-hole, unbound), Docker's build network does not resolve names.

## Settings

Environment variables (`docker-compose.yml`) take precedence over `config/relay.toml` (template: `relay.toml.example`).

| Variable | Default | Role |
|---|---|---|
| `KN_RELAY_DOMAIN` | `share.kybernow.app` | The relay's name; Stations are served under `<id>.<name>` |
| `KN_RELAY_LISTEN` | `0.0.0.0:8443` | Listening address, TCP and UDP |
| `KN_RELAY_PUBLIC_PORT` | the listening port | Port written in links, if the router publishes another one |
| `KN_RELAY_CERT`, `KN_RELAY_KEY` | `/certs/certificates/<name>.crt` and `.key` | lego's certificate |
| `KN_RELAY_BANLIST` | none | Ban list, reloaded live |
| `KN_RELAY_METRICS` | `127.0.0.1:9464` | Metrics and `/healthz` (`off` to turn off; the image's health check needs it) |
| `KN_RELAY_EGRESS_ALERT_MBPS` | `0` | Warning in the log above this bitrate |
| `KN_RELAY_GLOBAL_CAP_MBPS` | `0` | Global cap on relayed packets (`0`: none) |
| `KN_RELAY_CONFIG` | `/config/relay.toml` | The settings file |

The limits (`[limits]` in `relay.toml`) follow [docs/relais.en.md](../../docs/relais.en.md#relay-limits-and-protections): code lookups (10 per minute and 60 per hour per address, 30 per minute per /24 or /48, 600 per minute overall), QUIC connections (16 per address, 60 handshakes per minute, 4096 overall, address validation above 100 handshakes per second), TCP (32 per address, TLS and headers within 10 s, 16 KiB of headers), 4 Stations per address, 8 clients per Station, 4 UDP flows per client, 4 codes per Station. No bitrate cap by default.

## Operations

- **Logs**: `docker compose logs relay`, rotated by Docker (20 MB × 5). They keep Stations' IP addresses and connection times, never a share code nor a page path. `RUST_LOG=debug` says more.
- **Metrics** (Prometheus, `http://127.0.0.1:9464/metrics`): `relay_stations_connected{transport}`, `relay_clients_connected{kind}`, `relay_sessions_active`, `relay_bytes_total{leg,dir}`, `relay_datagrams_total{result}`, `relay_code_lookups_total{result}`, `relay_codes_active`, `relay_handshakes_total{alpn,result}`, `relay_rtt_seconds{peer}`, `relay_limit_hits_total{limit}`, `relay_tls_cert_expiry_seconds`, `relay_egress_mbps`. An alert on `relay_tls_cert_expiry_seconds < 1209600` (14 days) warns of an expired Gandi token.
- **Banning**: one address or network per line in `config/banlist.txt` (`203.0.113.0/24`, `#` for a comment); taken into account within 30 seconds.
- **Health**: `docker compose ps` shows the health check's state (`kybernow-relay --healthcheck`).
- **Pages**: `https://<name>:8443/` (code entry), `/j/<code>`, `/privacy`, `/v1/health`.

## Development

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-features --all-targets
```

The tests run on the loopback, without network: a test authority, a relay, a fake Station (the tunnel agent, a fake Admin, TCP and UDP echoes) and a fake app (the tunnel and its local listening points). They cover QUIC, the TCP fallback (forced, and when UDP does not get through), a Station over TCP with an app over QUIC, Admin's refusals, the Station absent then connected, the Station and the relay restarting, codes and waiting with a fake Admin, the relay's page, the browser proxy (headers, cookies, WebSocket) and the limits.

The `tunnel/` crate is taken by path: `features = ["station"]` for Admin, the default `client` feature for the apps' core, `server` for the relay. It builds for iOS and Android (rustls with ring, no OpenSSL).
