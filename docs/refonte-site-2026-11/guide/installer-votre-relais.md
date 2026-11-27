---
slug: installer-votre-relais
titre: Installer votre propre relais
title: Installer votre propre relais · KyberNow
description: Faites passer partages, accès hors de chez vous et comptes KyberNow par votre relais, sur une machine Linux avec Docker et votre nom de domaine.
rubrique: Hors de chez vous
univers: pro
duree: 45 min
connexes: [hors-de-chez-vous, familles-et-equipes, compte-kybernow]
---

Le relais public de KyberNow marche pour tout le monde, sans rien installer. Le vôtre fait passer partages, accès hors de chez vous et comptes par une machine que vous tenez, pour une équipe, une association ou par principe.

Cet article s'adresse à quelqu'un d'à l'aise avec un serveur Linux. Le relais est une image Docker toute faite, publiée à chaque version de KyberNow : il n'y a rien à compiler.

## Ce qu'il vous faut

- Une machine Linux toujours allumée, avec Docker et `docker compose`.
- Un nom de domaine, par exemple relais.exemple.org, dont la zone DNS se règle par une API. Gandi est prévu par défaut ; un autre hébergeur DNS pris en charge par lego, l'outil qui obtient le certificat, marche aussi, en changeant `--dns gandiv5` dans `lego.sh` et la variable du jeton dans `docker-compose.yml`.
- Un port ouvert de l'extérieur vers la machine, en TCP et en UDP, le 443 de préférence. L'UDP porte l'image ; sans lui, le relais passe en TCP, bon pour regarder et moins bon pour jouer.
- Pour les comptes KyberNow, un serveur d'envoi d'e-mails (SMTP), si vous voulez l'inscription en libre-service.

## 1. Le DNS

Créez deux enregistrements vers votre adresse IPv4 publique :

| Nom | Type | Valeur |
|---|---|---|
| `relais.exemple.org` | A | votre adresse publique |
| `*.relais.exemple.org` | A | la même adresse |

Le nom générique sert au navigateur : chaque ordinateur a sa propre adresse sous votre domaine. Au port 443, le navigateur passe aussi de bout en bout par `<identifiant>.station.relais.exemple.org` : le relais fait suivre la connexion telle quelle, et chaque ordinateur obtient lui-même son certificat auprès de Let's Encrypt. Le nom générique y répond déjà ; un enregistrement `*.station.relais.exemple.org` vers la même adresse est plus clair. Si la zone a des enregistrements CAA, ils doivent permettre `letsencrypt.org`. N'ajoutez un AAAA que si la machine est vraiment joignable en IPv6 sur le port public.

## 2. Les ports et la box

Trois ports se suivent, en TCP et en UDP :

- le **port d'écoute** du relais sur la machine, `RELAY_LISTEN` dans `.env`, 8443 par défaut ;
- le **port public**, celui que les appareils joignent depuis internet, `RELAY_PUBLIC_PORT` ;
- la **box**, qui renvoie le port public vers le port d'écoute de la machine.

| Sur la box | Dans `.env` | Adresse à donner à Admin |
|---|---|---|
| 443 dehors vers 8443 sur la machine | `RELAY_PUBLIC_PORT=443` | `relais.exemple.org` |
| 9443 dehors vers 9443 sur la machine | `RELAY_LISTEN=0.0.0.0:9443` | `relais.exemple.org:9443` |
| 8443 dehors vers 8443 sur la machine | rien | `relais.exemple.org:8443` |

Le 443 est le plus simple : rien à retenir pour ceux qui partagent, et les réseaux qui ne laissent sortir que le web le laissent passer. C'est aussi le seul port qui permet au navigateur de passer de bout en bout.

## 3. Le jeton de l'hébergeur DNS

Dans votre compte Gandi, créez un jeton d'accès personnel limité à votre domaine, avec le droit de gérer sa configuration technique. Let's Encrypt s'en sert pour vérifier que le domaine est à vous. Notez sa date d'expiration : le certificat ne se renouvelle plus après.

```sh
mkdir -p ~/kybernow-relay/secrets ~/kybernow-relay/certs ~/kybernow-relay/config
cd ~/kybernow-relay
nano secrets/gandi_pat      # le jeton seul, sur une ligne
chmod 600 secrets/gandi_pat
touch secrets/smtp_password # le mot de passe SMTP des comptes, vide sans SMTP
chmod 644 secrets/smtp_password
```

## 4. Les fichiers

Téléchargez les quatre fichiers du relais, puis préparez `.env` :

```sh
cd ~/kybernow-relay
for f in docker-compose.yml lego.sh update.sh .env.example; do curl -fsSLO https://raw.githubusercontent.com/MonsieurNours/kybernow/main/server/relay/$f; done
cp .env.example .env
nano .env                   # RELAY_DOMAIN, LEGO_EMAIL, et les ports
```

Le relais reçoit beaucoup de paquets UDP : donnez-lui de grands tampons, une fois, en root.

```sh
printf 'net.core.rmem_max=8388608\nnet.core.wmem_max=8388608\n' | sudo tee /etc/sysctl.d/90-kybernow-relay.conf
sudo sysctl --system
```

## 5. Le certificat, d'abord en essai

Lisez les conditions de Let's Encrypt, mettez `LEGO_ACCEPT_TOS=yes` dans `.env`, puis lancez le relais :

```sh
docker compose up -d
docker compose logs -f lego
```

Le relais commence par un certificat d'essai, reconnu par aucun navigateur, qui prouve que le DNS et le jeton marchent. Quand `certs/certificates/<domaine>.crt` est là, passez en production :

```sh
nano .env                   # LEGO_SERVER=https://acme-v02.api.letsencrypt.org/directory
rm -rf certs/certificates
docker compose up -d lego
```

Le relais relit son certificat de lui-même, sans redémarrer, et lego le renouvelle 30 jours avant son expiration.

## 6. Dans Admin, sur chaque Station

1. Ouvrez Admin, espace Appareils Link, section « Hors de chez vous ».
2. Dans « Relais », tapez l'adresse de votre relais, avec son port si ce n'est pas le 443 : `relais.exemple.org`, ou `relais.exemple.org:9443`.
3. « Enregistrer ». « Relais joint » confirme que la Station l'a trouvé.

Les liens et les QR codes des partages portent alors l'adresse de votre relais. Un code tapé dans une app passe par le relais public et par les relais des ordinateurs que l'app connaît déjà : pour un invité qui ne connaît pas votre relais, donnez le lien ou le QR code.

> Bon à savoir : sans vrai certificat, avec un relais qui signe lui-même le sien, les apps marchent si l'adresse porte l'empreinte du certificat (`relais.exemple.org:9443#sha256:ab:cd:…`). Le navigateur, lui, demande un vrai certificat.

## Les comptes KyberNow sur votre relais

Les comptes sont coupés au départ. Pour les allumer :

```sh
mkdir -p data && sudo chown 65532:65532 data      # la base et la clé, à l'utilisateur de l'image
nano config/relay.toml                             # [accounts] enabled = true (modèle : relay.toml.example)
docker compose up -d relay
```

- **Les e-mails** : `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER` et `SMTP_FROM` dans `.env`, le mot de passe dans `secrets/smtp_password`. Sans SMTP, il n'y a ni inscription, ni vérification d'adresse, ni mot de passe oublié : vous créez les comptes vous-même, déjà vérifiés.

  ```sh
  docker compose exec relay kybernow-relay accounts add ana@exemple.org --name Ana
  ```

- **La sauvegarde** : gardez le dossier `data/`, avec la base et la clé du relais. Perdre la clé coupe la double authentification de tous les comptes.
- **Dans les apps**, « Connecter un compte » puis « Un autre relais… » ; dans les installateurs de la Station, « Autre relais… ». Les comptes de votre relais sont indépendants de ceux de share.kybernow.app, et une passkey ne vaut que sur le relais où elle a été créée.

## Mettre à jour le relais

Le relais se met à jour seul chaque nuit, à 4 h UTC (`RELAY_UPDATE_HOUR`), et ne redémarre que si l'image a changé. `RELAY_AUTO_UPDATE=no` coupe cette mise à jour. Pour n'accepter qu'une image signée, donnez une clé publique cosign dans `RELAY_COSIGN_KEY` ; pour figer une version, épinglez l'image par son empreinte dans `RELAY_IMAGE`. À la main :

```sh
docker compose pull
docker compose up -d
```

## Si ça ne marche pas

- **Pas de certificat.** Lisez `docker compose logs -f lego` : un enregistrement DNS manque, ou le jeton n'a pas les bons droits, ou il a expiré.
- **Admin affiche « Relais injoignable ».** Vérifiez la redirection de la box, en TCP et en UDP, et le port dans l'adresse.
- **Les sessions passent en « Par le relais (TCP) ».** L'UDP n'arrive pas jusqu'à la machine : vérifiez la box et le pare-feu.
- **Les appareils de la maison n'arrivent pas au relais par son nom.** La box ne fait pas le retour vers le réseau local. Faites donner l'adresse locale de la machine pour `relais.exemple.org` et `*.relais.exemple.org` par votre DNS local (Pi-hole, la box).
- **Un certificat qui va expirer.** Une alerte sur la mesure `relay_tls_cert_expiry_seconds`, à 14 jours, prévient d'un jeton Gandi échu.

Le pas à pas complet, avec tous les réglages, l'exploitation et les journaux, est dans le [README du relais sur GitHub](https://github.com/MonsieurNours/kybernow/blob/main/server/relay/README.md).
