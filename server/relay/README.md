# Le relais de KyberNow

[English](README.en.md) · Français

Le relais fait passer KyberNow par internet quand votre appareil n'est pas sur le réseau de l'ordinateur : partager son écran avec quelqu'un qui est ailleurs, ou retrouver son ordinateur depuis l'hôtel ou le train. Le contrat (tunnel, routes d'Admin, liens, confiance) est dans [docs/relais.md](../../docs/relais.md).

Le relais public, `share.kybernow.app:8443`, est ouvert à toutes les Stations. Vous pouvez aussi installer le vôtre : une machine Linux avec Docker, un nom de domaine, et un seul port ouvert, **8443 en TCP et en UDP**.

| Fichier | Contenu |
|---|---|
| `src/` | Le serveur relais (`kybernow-relay`) : connexions QUIC et TCP, Stations et clients, codes, page web, mandataire du navigateur, métriques |
| `tunnel/` | Le crate du tunnel (`kybernow-tunnel`), partagé avec Admin et le cœur des apps |
| `Dockerfile` | L'image : compilée avec Rust 1.89, puis le binaire seul sur distroless, sans root |
| `docker-compose.yml` | Le relais et lego, qui obtient et renouvelle le certificat |
| `lego.sh` | Ce que lance lego : certificat obtenu, puis renouvelé toutes les 12 heures si besoin |
| `.env.example`, `relay.toml.example` | Les réglages à recopier |

## Installer un relais chez soi

### 1. Ce qu'il vous faut

- Une machine Linux toujours allumée, avec Docker et `docker compose`.
- Un nom de domaine dont la zone DNS se règle par une API : Gandi par défaut (le certificat couvre `*.relais.exemple.org`, ce qui demande une preuve par le DNS). Un autre hébergeur DNS pris en charge par lego marche aussi : changez `--dns gandiv5` dans `lego.sh` et la variable du jeton dans `docker-compose.yml`.
- Le port 8443 ouvert de l'extérieur vers cette machine, **en TCP et en UDP**. L'UDP porte l'image ; sans lui, le relais passe en TCP (bon pour regarder, moins bon pour jouer).

### 2. Le DNS

Deux enregistrements vers votre adresse IPv4 publique :

| Nom | Type | Valeur |
|---|---|---|
| `relais.exemple.org` | A | votre adresse publique |
| `*.relais.exemple.org` | A | la même adresse |

N'ajoutez un AAAA que si la machine est vraiment joignable en IPv6 sur le port 8443. Le nom générique sert au navigateur : chaque ordinateur a sa propre adresse, `https://<identifiant>.relais.exemple.org:8443`.

### 3. La box

Redirigez le port 8443 en TCP **et** en UDP vers la machine du relais. Si les appareils de la maison n'arrivent pas au relais par son nom (la box ne fait pas le « retour » vers le réseau local), faites donner l'adresse locale de la machine pour `relais.exemple.org` et `*.relais.exemple.org` par votre DNS local (Pi-hole, la box).

### 4. Le jeton Gandi

Dans votre compte Gandi, créez un jeton d'accès personnel limité à votre domaine, avec le droit de gérer sa configuration technique. Notez sa date d'expiration : le certificat ne se renouvelle plus après. Écrivez-le dans un fichier, jamais ailleurs :

```sh
mkdir -p ~/kybernow-relay/secrets ~/kybernow-relay/certs ~/kybernow-relay/config
cd ~/kybernow-relay
nano secrets/gandi_pat      # le jeton seul, sur une ligne
chmod 600 secrets/gandi_pat
```

### 5. Les fichiers

Copiez dans `~/kybernow-relay` le contenu de ce dossier (ou clonez le dépôt et travaillez dans `server/relay`), puis :

```sh
cp .env.example .env
nano .env                   # RELAY_DOMAIN, LEGO_EMAIL
```

Le relais reçoit beaucoup de paquets UDP : donnez-lui de grands tampons (une fois, en root) :

```sh
printf 'net.core.rmem_max=8388608\nnet.core.wmem_max=8388608\n' | sudo tee /etc/sysctl.d/90-kybernow-relay.conf
sudo sysctl --system
ss -ltnup | grep 8443       # rien ne doit déjà écouter sur 8443
```

### 6. Le certificat, d'abord en test

Lisez les conditions de Let's Encrypt (https://letsencrypt.org/repository/), mettez `LEGO_ACCEPT_TOS=yes` dans `.env`, puis :

```sh
docker compose up -d --build
docker compose logs -f lego
```

`LEGO_SERVER` vise d'abord le serveur de test de Let's Encrypt : son certificat n'est reconnu par aucun navigateur, mais il prouve que le DNS et le jeton marchent, sans risque d'être bloqué. Quand `certs/certificates/<domaine>.crt` est là, passez en production :

```sh
nano .env                   # LEGO_SERVER=https://acme-v02.api.letsencrypt.org/directory
rm -rf certs/certificates
docker compose up -d lego
```

Le relais relit le certificat de lui-même (toutes les 30 secondes, s'il a changé), sans redémarrer. lego le renouvelle 30 jours avant son expiration.

### 7. Dans Admin

Sur chaque Station : Admin › Appareils Link › « Hors de chez vous » › « Relais » = `relais.exemple.org:8443`. Le panneau du partage dit alors « Sur le même réseau ou ailleurs… », et les liens portent l'adresse de votre relais. Un code tapé dans une app passe par le relais public et par les relais des ordinateurs que l'app connaît déjà ; pour un invité qui ne connaît pas votre relais, donnez le lien ou le QR code.

Sans vrai certificat (relais signé par lui-même), les apps marchent si l'adresse porte l'empreinte du certificat (`relais.exemple.org:8443#sha256:ab:cd:…`) ; le navigateur, lui, demande un vrai certificat.

### Mettre à jour

```sh
git pull                    # ou copiez les nouveaux fichiers
docker compose up -d --build
```

## Réglages

Les variables d'environnement (`docker-compose.yml`) passent avant `config/relay.toml` (modèle : `relay.toml.example`).

| Variable | Défaut | Rôle |
|---|---|---|
| `KN_RELAY_DOMAIN` | `share.kybernow.app` | Le nom du relais ; les Stations sont servies sous `<id>.<nom>` |
| `KN_RELAY_LISTEN` | `0.0.0.0:8443` | Adresse d'écoute, TCP et UDP |
| `KN_RELAY_PUBLIC_PORT` | le port d'écoute | Port écrit dans les liens, si la box en publie un autre |
| `KN_RELAY_CERT`, `KN_RELAY_KEY` | `/certs/certificates/<nom>.crt` et `.key` | Le certificat de lego |
| `KN_RELAY_BANLIST` | aucune | Liste de bannissement, relue à chaud |
| `KN_RELAY_METRICS` | `127.0.0.1:9464` | Métriques et `/healthz` (`off` pour couper ; le contrôle de santé de l'image en a besoin) |
| `KN_RELAY_EGRESS_ALERT_MBPS` | `0` | Avertissement dans le journal au-delà de ce débit |
| `KN_RELAY_GLOBAL_CAP_MBPS` | `0` | Plafond global des paquets relayés (`0` : aucun) |
| `KN_RELAY_CONFIG` | `/config/relay.toml` | Le fichier de réglages |

Les limites (`[limits]` de `relay.toml`) suivent [docs/relais.md](../../docs/relais.md#limites-et-protections-du-relais) : recherches de code (10 par minute et 60 par heure par adresse, 30 par minute par /24 ou /48, 600 par minute en tout), connexions QUIC (16 par adresse, 60 négociations par minute, 4096 en tout, preuve d'adresse au-delà de 100 négociations par seconde), TCP (32 par adresse, TLS et en-têtes en 10 s, 16 Kio d'en-têtes), 4 Stations par adresse, 8 clients par Station, 4 flux UDP par client, 4 codes par Station. Pas de plafond de débit par défaut.

## Exploitation

- **Journaux** : `docker compose logs relay`, tournés par Docker (20 Mo × 5). Ils gardent les adresses IP des Stations et l'heure des connexions, jamais un code de partage ni un chemin de page. `RUST_LOG=debug` en dit plus.
- **Métriques** (Prometheus, `http://127.0.0.1:9464/metrics`) : `relay_stations_connected{transport}`, `relay_clients_connected{kind}`, `relay_sessions_active`, `relay_bytes_total{leg,dir}`, `relay_datagrams_total{result}`, `relay_code_lookups_total{result}`, `relay_codes_active`, `relay_handshakes_total{alpn,result}`, `relay_rtt_seconds{peer}`, `relay_limit_hits_total{limit}`, `relay_tls_cert_expiry_seconds`, `relay_egress_mbps`. Une alerte sur `relay_tls_cert_expiry_seconds < 1209600` (14 jours) prévient d'un jeton Gandi expiré.
- **Bannir** : une adresse ou un réseau par ligne dans `config/banlist.txt` (`203.0.113.0/24`, `#` pour un commentaire) ; pris dans les 30 secondes.
- **Santé** : `docker compose ps` montre l'état du contrôle de santé (`kybernow-relay --healthcheck`).
- **Pages** : `https://<nom>:8443/` (saisie du code), `/j/<code>`, `/privacy`, `/v1/health`.

## Développement

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-features --all-targets
```

Les essais tournent sur la boucle locale, sans réseau : une autorité d'essai, un relais, une fausse Station (l'agent du tunnel, un faux Admin, des échos TCP et UDP) et une fausse app (le tunnel et ses points d'écoute locaux). Ils couvrent QUIC, le repli TCP (forcé, et quand l'UDP ne passe pas), une Station en TCP avec une app en QUIC, les refus d'Admin, la Station absente puis connectée, la Station et le relais qui redémarrent, les codes et l'attente avec un faux Admin, la page du relais, le mandataire du navigateur (en-têtes, cookies, WebSocket) et les limites.

Le crate `tunnel/` se prend par chemin : `features = ["station"]` pour Admin, la fonction par défaut `client` pour le cœur des apps, `server` pour le relais. Il compile pour iOS et Android (rustls avec ring, pas d'OpenSSL).
