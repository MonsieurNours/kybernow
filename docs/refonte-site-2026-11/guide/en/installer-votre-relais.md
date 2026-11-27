---
slug: installer-votre-relais
slug_en: install-your-relay
titre: Install your own relay
title: Install your own relay · KyberNow
description: Run shares, access away from home and KyberNow accounts through your own relay, on a Linux machine with Docker and your domain name.
rubrique: Hors de chez vous
rubrique_en: Away from home
univers: pro
duree: 45 min
connexes: [hors-de-chez-vous, familles-et-equipes, compte-kybernow]
---

KyberNow's public relay works for everyone, with nothing to install. Your own relay sends shares, access away from home and accounts through a machine you run yourself, for a team, a club or on principle.

This article is for someone comfortable with a Linux server. The relay is a ready-made Docker image, published with every KyberNow version: there is nothing to compile.

## What you need

- A Linux machine that is always on, with Docker and `docker compose`.
- A domain name, for example relay.example.org, whose DNS zone can be managed through an API. Gandi is the default; another DNS host supported by lego, the tool that obtains the certificate, works too, by changing `--dns gandiv5` in `lego.sh` and the token variable in `docker-compose.yml`.
- A port open from outside to the machine, over TCP and UDP, 443 preferably. UDP carries the picture; without it, the relay falls back to TCP, fine for watching and less good for playing.
- For KyberNow accounts, an outgoing mail server (SMTP), if you want self-service sign-up.

## 1. DNS

Create two records pointing to your public IPv4 address:

| Name | Type | Value |
|---|---|---|
| `relay.example.org` | A | your public address |
| `*.relay.example.org` | A | the same address |

The wildcard name is for the browser: each computer gets its own address under your domain. On port 443, the browser also goes end to end through `<identifier>.station.relay.example.org`: the relay forwards the connection as it is, and each computer obtains its own certificate from Let's Encrypt. The wildcard name already answers for it; a `*.station.relay.example.org` record pointing to the same address is clearer. If the zone has CAA records, they must allow `letsencrypt.org`. Only add an AAAA record if the machine is genuinely reachable over IPv6 on the public port.

## 2. Ports and the router

Three ports follow one another, over TCP and UDP:

- the relay's **listening port** on the machine, `RELAY_LISTEN` in `.env`, 8443 by default;
- the **public port**, the one devices reach from the internet, `RELAY_PUBLIC_PORT`;
- the **router**, which forwards the public port to the machine's listening port.

| On the router | In `.env` | Address to give Admin |
|---|---|---|
| 443 outside to 8443 on the machine | `RELAY_PUBLIC_PORT=443` | `relay.example.org` |
| 9443 outside to 9443 on the machine | `RELAY_LISTEN=0.0.0.0:9443` | `relay.example.org:9443` |
| 8443 outside to 8443 on the machine | nothing | `relay.example.org:8443` |

Port 443 is the simplest: nothing to remember for the people you share with, and networks that only let web traffic out let it through. It is also the only port that lets the browser go end to end.

## 3. The DNS host's token

In your Gandi account, create a personal access token limited to your domain, with permission to manage its technical configuration. Let's Encrypt uses it to check that the domain is yours. Note its expiry date: the certificate stops renewing after it.

```sh
mkdir -p ~/kybernow-relay/secrets ~/kybernow-relay/certs ~/kybernow-relay/config
cd ~/kybernow-relay
nano secrets/gandi_pat      # the token alone, on one line
chmod 600 secrets/gandi_pat
touch secrets/smtp_password # the SMTP password for accounts, empty without SMTP
chmod 644 secrets/smtp_password
```

## 4. The files

Download the relay's four files, then prepare `.env`:

```sh
cd ~/kybernow-relay
for f in docker-compose.yml lego.sh update.sh .env.example; do curl -fsSLO https://raw.githubusercontent.com/MonsieurNours/kybernow/main/server/relay/$f; done
cp .env.example .env
nano .env                   # RELAY_DOMAIN, LEGO_EMAIL, and the ports
```

The relay receives a lot of UDP packets: give it large buffers, once, as root.

```sh
printf 'net.core.rmem_max=8388608\nnet.core.wmem_max=8388608\n' | sudo tee /etc/sysctl.d/90-kybernow-relay.conf
sudo sysctl --system
```

## 5. The certificate, as a test first

Read Let's Encrypt's terms, set `LEGO_ACCEPT_TOS=yes` in `.env`, then start the relay:

```sh
docker compose up -d
docker compose logs -f lego
```

The relay starts with a test certificate, which no browser recognises, to prove that the DNS and the token work. Once `certs/certificates/<domain>.crt` is there, switch to production:

```sh
nano .env                   # LEGO_SERVER=https://acme-v02.api.letsencrypt.org/directory
rm -rf certs/certificates
docker compose up -d lego
```

The relay reloads its certificate by itself, without restarting, and lego renews it 30 days before it expires.

## 6. In Admin, on each Station

1. Open Admin, under Link devices, in the “Away from home” section.
2. In “Relay”, type your relay's address, with its port if it isn't 443: `relay.example.org`, or `relay.example.org:9443`.
3. “Save”. “Relay reached” confirms that the Station has found it.

Share links and QR codes then carry your relay's address. A code typed into an app goes through the public relay and through the relays of the computers the app already knows: for a guest who doesn't know your relay, give them the link or the QR code.

> Good to know: without a real certificate, with a relay that signs its own, the apps work if the address carries the certificate's fingerprint (`relay.example.org:9443#sha256:ab:cd:…`). The browser, on the other hand, needs a real certificate.

## KyberNow accounts on your relay

Accounts are off to begin with. To turn them on:

```sh
mkdir -p data && sudo chown 65532:65532 data      # the database and key, owned by the image's user
nano config/relay.toml                             # [accounts] enabled = true (template: relay.toml.example)
docker compose up -d relay
```

- **Email**: `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER` and `SMTP_FROM` in `.env`, the password in `secrets/smtp_password`. Without SMTP, there is no sign-up, no address verification and no forgotten-password reset: you create the accounts yourself, already verified.

  ```sh
  docker compose exec relay kybernow-relay accounts add ana@example.org --name Ana
  ```

- **Backups**: keep the `data/` folder, with the relay's database and key. Losing the key turns off two-factor authentication for every account.
- **In the apps**, “Connect an account” then “Another relay…”; in the Station installers, “Other relay…”. Your relay's accounts are separate from those on share.kybernow.app, and a passkey only works on the relay where it was created.

## Update the relay

The relay updates itself every night at 4 am UTC (`RELAY_UPDATE_HOUR`), and only restarts if the image has changed. `RELAY_AUTO_UPDATE=no` turns this update off. To accept only a signed image, give a cosign public key in `RELAY_COSIGN_KEY`; to freeze a version, pin the image by its digest in `RELAY_IMAGE`. By hand:

```sh
docker compose pull
docker compose up -d
```

## If something goes wrong

- **No certificate.** Read `docker compose logs -f lego`: a DNS record is missing, or the token doesn't have the right permissions, or it has expired.
- **Admin shows “Relay unreachable”.** Check the router's forwarding, over TCP and UDP, and the port in the address.
- **Sessions switch to “Via the relay (TCP)”.** UDP isn't reaching the machine: check the router and the firewall.
- **Devices at home can't reach the relay by its name.** The router doesn't loop back to the local network (no NAT loopback). Have your local DNS (Pi-hole, the router) give the machine's local address for `relay.example.org` and `*.relay.example.org`.
- **A certificate about to expire.** An alert on the `relay_tls_cert_expiry_seconds` metric, at 14 days, warns you about an expired Gandi token.

The full step by step, with every setting, operations and logs, is in the [relay's README on GitHub](https://github.com/MonsieurNours/kybernow/blob/main/server/relay/README.en.md).
