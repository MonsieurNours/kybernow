# Le relais de KyberNow

[English](README.en.md) · Français

Le relais fait passer KyberNow par internet quand votre appareil n'est pas sur le réseau de l'ordinateur : partager son écran avec quelqu'un qui est ailleurs, ou retrouver son ordinateur depuis l'hôtel ou le train. Le contrat (tunnel, routes d'Admin, liens, confiance) est dans [docs/relais.md](../../docs/relais.md).

Le relais public, `share.kybernow.app`, est ouvert à toutes les Stations. Vous pouvez aussi installer le vôtre : une machine Linux avec Docker, un nom de domaine, et un seul port ouvert, **en TCP et en UDP** (le 443 de préférence, voir « Les ports et la box »).

| Fichier | Contenu |
|---|---|
| `src/` | Le serveur relais (`kybernow-relay`) : connexions QUIC et TCP, Stations et clients, codes, pages web (`pages.rs`, `assets.rs`), mandataire du navigateur, métriques |
| `assets/` | Ce que les pages embarquent : feuille de styles (`share.css`, écrite à la main, avec les jetons de la charte `kybernow.css`), polices Archivo et Martian Mono réduites au latin, icônes, logo et favicon de KyberNow Share |
| `tunnel/` | Le crate du tunnel (`kybernow-tunnel`), partagé avec Admin et le cœur des apps |
| `Dockerfile` | L'image : compilée avec Rust 1.89, puis le binaire seul sur distroless, sans root |
| `docker-compose.yml` | Le relais et lego, qui obtient et renouvelle le certificat |
| `lego.sh` | Ce que lance lego : certificat obtenu, puis renouvelé toutes les 12 heures si besoin |
| `.env.example`, `relay.toml.example` | Les réglages à recopier |

## Installer un relais chez soi

### 1. Ce qu'il vous faut

- Une machine Linux toujours allumée, avec Docker et `docker compose`.
- Un nom de domaine dont la zone DNS se règle par une API : Gandi par défaut (le certificat couvre `*.relais.exemple.org`, ce qui demande une preuve par le DNS). Un autre hébergeur DNS pris en charge par lego marche aussi : changez `--dns gandiv5` dans `lego.sh` et la variable du jeton dans `docker-compose.yml`.
- Un port ouvert de l'extérieur vers cette machine, **en TCP et en UDP** : le 443 de préférence, sinon celui de votre choix. L'UDP porte l'image ; sans lui, le relais passe en TCP (bon pour regarder, moins bon pour jouer).

### 2. Le DNS

Deux enregistrements vers votre adresse IPv4 publique :

| Nom | Type | Valeur |
|---|---|---|
| `relais.exemple.org` | A | votre adresse publique |
| `*.relais.exemple.org` | A | la même adresse |

N'ajoutez un AAAA que si la machine est vraiment joignable en IPv6 sur le port public. Le nom générique sert au navigateur : chaque ordinateur a sa propre adresse, `https://<identifiant>.relais.exemple.org` (avec `:<port>` si le port public n'est pas le 443).

### 3. Les ports et la box

Trois ports se suivent, en TCP **et** en UDP :

- **le port d'écoute** du relais sur la machine : `RELAY_LISTEN` dans `.env`, `0.0.0.0:8443` par défaut. L'image tourne sans root : il reste au-dessus de 1024 ;
- **le port public**, celui que les appareils joignent depuis internet : `RELAY_PUBLIC_PORT`, le port d'écoute par défaut. Le relais l'écrit dans ses liens (`/j/<code>` renvoie vers `https://<identifiant>.<nom>[:port]/link/…`) ; au 443, les adresses n'ont pas de port ;
- **la box** fait le lien entre les deux : elle renvoie le port public vers le port d'écoute de la machine.

| Box | `.env` | Adresse donnée à Admin | Liens |
|---|---|---|---|
| 443 dehors → 8443 sur la machine (le relais public) | `RELAY_PUBLIC_PORT=443` | `relais.exemple.org` | `https://relais.exemple.org/j/123456` |
| 9443 dehors → 9443 sur la machine | `RELAY_LISTEN=0.0.0.0:9443` | `relais.exemple.org:9443` | `https://relais.exemple.org:9443/j/123456` |
| 8443 dehors → 8443 sur la machine | rien | `relais.exemple.org:8443` | `https://relais.exemple.org:8443/j/123456` |

Le 443 est le plus simple pour ceux qui partagent : rien à retenir, et les réseaux qui ne laissent sortir que le web le laissent passer. Une adresse de relais sans port vaut 443, dans Admin comme dans les apps.

Si les appareils de la maison n'arrivent pas au relais par son nom (la box ne fait pas le « retour » vers le réseau local), faites donner l'adresse locale de la machine pour `relais.exemple.org` et `*.relais.exemple.org` par votre DNS local (Pi-hole, la box) ; ils joignent alors le port d'écoute directement, qui doit donc être le même que le port public, ou la box doit faire elle-même la traduction pour eux.

### 4. Le jeton Gandi

Dans votre compte Gandi, créez un jeton d'accès personnel limité à votre domaine, avec le droit de gérer sa configuration technique. Notez sa date d'expiration : le certificat ne se renouvelle plus après. Écrivez-le dans un fichier, jamais ailleurs :

```sh
mkdir -p ~/kybernow-relay/secrets ~/kybernow-relay/certs ~/kybernow-relay/config
cd ~/kybernow-relay
nano secrets/gandi_pat      # le jeton seul, sur une ligne
chmod 600 secrets/gandi_pat
```

### 5. Les fichiers

L'image du relais est publiée à chaque version de KyberNow (`ghcr.io/monsieurnours/kybernow-relay`) : il n'y a rien à compiler. Il suffit de quatre fichiers de ce dossier :

```sh
mkdir -p ~/kybernow-relay && cd ~/kybernow-relay
for f in docker-compose.yml lego.sh update.sh .env.example; do curl -fsSLO https://raw.githubusercontent.com/MonsieurNours/kybernow/main/server/relay/$f; done
cp .env.example .env
nano .env                   # RELAY_DOMAIN, LEGO_EMAIL, et les ports (« Les ports et la box »)
```

Le relais reçoit beaucoup de paquets UDP : donnez-lui de grands tampons (une fois, en root) :

```sh
printf 'net.core.rmem_max=8388608\nnet.core.wmem_max=8388608\n' | sudo tee /etc/sysctl.d/90-kybernow-relay.conf
sudo sysctl --system
ss -ltnup | grep 8443       # rien ne doit déjà écouter sur le port d'écoute (8443 par défaut)
```

### 6. Le certificat, d'abord en test

Lisez les conditions de Let's Encrypt (https://letsencrypt.org/repository/), mettez `LEGO_ACCEPT_TOS=yes` dans `.env`, puis :

```sh
docker compose up -d
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

Sur chaque Station : Admin › Appareils Link › « Hors de chez vous » › « Relais » = l'adresse du relais avec son port public : `relais.exemple.org` pour le 443, `relais.exemple.org:9443` pour un autre. Le panneau du partage dit alors « Sur le même réseau ou ailleurs… », et les liens portent l'adresse de votre relais. Un code tapé dans une app passe par le relais public et par les relais des ordinateurs que l'app connaît déjà ; pour un invité qui ne connaît pas votre relais, donnez le lien ou le QR code.

Sans vrai certificat (relais signé par lui-même), les apps marchent si l'adresse porte l'empreinte du certificat (`relais.exemple.org:9443#sha256:ab:cd:…`) ; le navigateur, lui, demande un vrai certificat.

### Mettre à jour

**Tout seul** : le service `updater` du compose reprend chaque nuit la dernière image du relais (à 4 h UTC, `RELAY_UPDATE_HOUR` dans `.env`), et ne relance le relais que si elle a changé. `RELAY_AUTO_UPDATE=no` le coupe. Le certificat et les réglages ne sont pas touchés. Lancez `docker compose` depuis le dossier du relais : le service le voit au même chemin que la machine.

**À la main**, quand vous voulez :

```sh
docker compose pull
docker compose up -d
```

Quand une version change le compose lui-même (rare, le journal des versions le dit), reprenez les fichiers (`for f in docker-compose.yml lego.sh update.sh; do curl -fsSLO https://raw.githubusercontent.com/MonsieurNours/kybernow/main/server/relay/$f; done`) avant ces deux commandes.

**Depuis les sources**, sans l'image publiée : clonez le dépôt et, dans `server/relay`, `docker compose build relay && docker compose up -d`. L'image se construit avec le réseau de la machine (`network: host`) : derrière un DNS local (Pi-hole, unbound), le réseau de construction de Docker ne résout pas les noms. `RELAY_IMAGE` dans `.env` choisit une autre image (une version précise, `…:0.8.0`).

## Réglages

Les variables d'environnement (`docker-compose.yml`) passent avant `config/relay.toml` (modèle : `relay.toml.example`).

| Variable | Défaut | Rôle |
|---|---|---|
| `KN_RELAY_DOMAIN` | `share.kybernow.app` | Le nom du relais ; les Stations sont servies sous `<id>.<nom>` |
| `KN_RELAY_LISTEN` (`RELAY_LISTEN` dans `.env`) | `0.0.0.0:8443` | Adresse d'écoute, TCP et UDP |
| `KN_RELAY_PUBLIC_PORT` (`RELAY_PUBLIC_PORT`) | le port d'écoute | Port public annoncé, quand la box le traduit : celui des liens `/j/<code>` et des adresses `https://<id>.<nom>`, sans port quand c'est 443 |
| `KN_RELAY_CERT`, `KN_RELAY_KEY` | `/certs/certificates/<nom>.crt` et `.key` | Le certificat de lego |
| `KN_RELAY_BANLIST` | aucune | Liste de bannissement, relue à chaud |
| `KN_RELAY_METRICS` | `127.0.0.1:9464` | Métriques et `/healthz` (`off` pour couper ; le contrôle de santé de l'image en a besoin) |
| `KN_RELAY_EGRESS_ALERT_MBPS` | `0` | Avertissement dans le journal au-delà de ce débit |
| `KN_RELAY_GLOBAL_CAP_MBPS` | `0` | Plafond global des paquets relayés (`0` : aucun) |
| `KN_RELAY_CONFIG` | `/config/relay.toml` | Le fichier de réglages |

Les limites (`[limits]` de `relay.toml`) suivent [docs/relais.md](../../docs/relais.md#limites-et-protections-du-relais) : recherches de code (10 par minute et 60 par heure par adresse, 30 par minute par /24 ou /48, 600 par minute en tout), connexions QUIC (16 par adresse, 60 négociations par minute, 4096 en tout, preuve d'adresse au-delà de 100 négociations par seconde), TCP (32 par adresse, TLS et en-têtes en 10 s, 16 Kio d'en-têtes), 4 Stations par adresse, 20 connexions de Station par minute et par adresse, 8 clients par Station dont 2 par adresse (`clients_per_station_per_ip`), un client renvoyé après 30 s sans flux ni connexion (`client_idle_secs`) et, Station pleine, le plus ancien client inactif qui cède sa place, 4 flux UDP par client, 4 codes par Station. Pas de plafond de débit par défaut.

## Exploitation

- **Journaux** : `docker compose logs relay`, tournés par Docker (20 Mo × 5). Ils gardent les adresses IP des Stations et l'heure des connexions, jamais un code de partage ni un chemin de page. `RUST_LOG=debug` en dit plus.
- **Métriques** (Prometheus, `http://127.0.0.1:9464/metrics`) : `relay_stations_connected{transport}`, `relay_clients_connected{kind}`, `relay_sessions_active`, `relay_bytes_total{leg,dir}`, `relay_datagrams_total{result}`, `relay_code_lookups_total{result}`, `relay_codes_active`, `relay_handshakes_total{alpn,result}`, `relay_rtt_seconds{peer}`, `relay_limit_hits_total{limit}`, `relay_tls_cert_expiry_seconds`, `relay_egress_mbps`. Une alerte sur `relay_tls_cert_expiry_seconds < 1209600` (14 jours) prévient d'un jeton Gandi expiré.
- **Bannir** : une adresse ou un réseau par ligne dans `config/banlist.txt` (`203.0.113.0/24`, `#` pour un commentaire) ; pris dans les 30 secondes.
- **Santé** : `docker compose ps` montre l'état du contrôle de santé (`kybernow-relay --healthcheck`).
- **Pages** : `https://<nom>/` (saisie du code ; `https://<nom>:<port>/` hors du 443), `/j/<code>`, `/privacy`, `/assets/…`, `/v1/health`.

## La page d'accueil

`https://<nom>/` ne garde que l'essentiel : le logo de KyberNow Share, le champ « Rejoindre avec un code » (il envoie `/j?code=…`, qui renvoie vers la page de l'ordinateur ou dit pourquoi pas), un lien « Découvrir KyberNow » vers le site et la confidentialité. Le site présente KyberNow ; les liens mènent à `https://www.kybernow.app/`, en français ou en anglais selon la page. Les textes suivent [docs/ton-du-site.md](../../docs/ton-du-site.md), sans liste de plateformes.

- **Langue** : français si le navigateur est en français, anglais sinon (`Accept-Language`) ; `?lang=en` ou `?lang=fr` force la langue (le lien « EN » ou « FR » des autres pages), sans cookie, et le formulaire le garde jusqu'à `/j`.
- **Fichiers** : tout vient du relais, rien d'un tiers. `/assets/share.css` (jetons et mise en page, une seule feuille), `/assets/archivo.woff2`, `/assets/martian-mono.woff2`, `/assets/kybernow-share-dark.svg` et `/assets/kybernow-favicon.svg` sont embarqués dans le binaire (`include_bytes!`, environ 95 Ko de plus) ; les icônes sont écrites en ligne dans la page. Chaque adresse porte la version des fichiers (`?v=`, huit chiffres d'un SHA-256 de leur contenu) : la réponse se garde un an (`immutable`), et un fichier changé change d'adresse. Une adresse inconnue donne la page « introuvable ».
- **Politique de contenu** : `default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'`. Pas de script, pas de cookie, pas de mesure d'audience, aucune requête vers un autre site. Les pages restent en `noindex`.
- **Ports** : la page ne cite jamais l'adresse du relais (chemins relatifs) ; elle est la même sur le port d'écoute et sur le port public (`https://<nom>` sans port au 443).
- **Refaire les fichiers** : `share.css` s'écrit à la main. Les autres viennent de la charte : le logo et le favicon par `node brand/v2/tools/generate.mjs`, et le reste (`kybernow.css`, polices réduites au latin, icônes de `site/assets/icons.svg`, licences) par `scripts/relay-assets.py`, qui demande `fonttools` et `brotli` (`python3 -m venv /tmp/fonts && /tmp/fonts/bin/pip install fonttools brotli`). L'image Docker ne voit que ce dossier : les copies sont dans le dépôt.

## Développement

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-features --all-targets
```

Les essais tournent sur la boucle locale, sans réseau : une autorité d'essai, un relais, une fausse Station (l'agent du tunnel, un faux Admin, des échos TCP et UDP) et une fausse app (le tunnel et ses points d'écoute locaux). Ils couvrent QUIC, le repli TCP (forcé, et quand l'UDP ne passe pas), une Station en TCP avec une app en QUIC, les refus d'Admin, la Station absente puis connectée, la Station et le relais qui redémarrent, les codes et l'attente avec un faux Admin, la page d'accueil du relais et ses fichiers (langues, politique de contenu, cache), le mandataire du navigateur (en-têtes, cookies, WebSocket) et les limites.

Le crate `tunnel/` se prend par chemin : `features = ["station"]` pour Admin, la fonction par défaut `client` pour le cœur des apps, `server` pour le relais. Il compile pour iOS et Android (rustls avec ring, pas d'OpenSSL).
