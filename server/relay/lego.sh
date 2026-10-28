#!/bin/sh
# Certificat du relais de KyberNow : <domaine> et *.<domaine>, par DNS-01
# chez Gandi (jeton lu dans GANDIV5_PERSONAL_ACCESS_TOKEN_FILE). Obtenu au
# premier passage, renouvelé 30 jours avant l'expiration ; un passage toutes
# les 12 heures. Le relais relit le certificat de lui-même.
set -eu
: "${RELAY_DOMAIN:?RELAY_DOMAIN manque}"
: "${LEGO_EMAIL:?LEGO_EMAIL manque}"

if [ "${LEGO_ACCEPT_TOS:-no}" != "yes" ]; then
  echo "lego : lisez les conditions de Let's Encrypt (https://letsencrypt.org/repository/)," >&2
  echo "       puis mettez LEGO_ACCEPT_TOS=yes dans .env." >&2
  sleep 3600
  exit 1
fi

server="${LEGO_SERVER:-https://acme-staging-v02.api.letsencrypt.org/directory}"
set -- --accept-tos --email "$LEGO_EMAIL" --server "$server" --dns gandiv5 \
  --domains "$RELAY_DOMAIN" --domains "*.$RELAY_DOMAIN" --path /certs

# Le serveur qui a donné le certificat en place : passer du serveur de test à
# celui de production refait un certificat (un renouvellement garderait celui de
# test, encore valable).
marker=/certs/.lego-server
while true; do
  if [ -f "/certs/certificates/$RELAY_DOMAIN.crt" ] && [ "$(cat "$marker" 2>/dev/null)" = "$server" ]; then
    /lego "$@" renew --days 30 || echo "lego : renouvellement manqué, nouvel essai dans 12 heures" >&2
  elif /lego "$@" run; then
    echo "$server" > "$marker"
  else
    echo "lego : certificat pas obtenu, nouvel essai dans 12 heures" >&2
  fi
  # Le relais tourne sans root (utilisateur 65532 de l'image distroless) :
  # il doit pouvoir lire la clé, personne d'autre.
  if [ -d /certs/certificates ]; then
    chown -R 65532:65532 /certs/certificates
    chmod 0750 /certs/certificates
    chmod 0640 /certs/certificates/* 2>/dev/null || true
  fi
  sleep 43200
done
