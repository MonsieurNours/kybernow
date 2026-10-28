#!/bin/sh
# Mise à jour du relais (service « updater » du docker-compose.yml) : une fois
# par nuit, à RELAY_UPDATE_HOUR (UTC), reprend la dernière image du relais et
# ne relance le conteneur que si elle a changé. Le certificat (lego) et les
# réglages (.env, config/) ne sont pas touchés.
set -u
hour="$(printf '%02d' "${RELAY_UPDATE_HOUR:-4}")"
if [ "${RELAY_AUTO_UPDATE:-yes}" != "yes" ]; then
  echo "mise à jour automatique coupée (RELAY_AUTO_UPDATE)"
  exec sleep infinity
fi
echo "mise à jour du relais chaque nuit à ${hour} h (UTC)"
last=""
while true; do
  today="$(date -u +%F)"
  if [ "$(date -u +%H)" = "$hour" ] && [ "$today" != "$last" ]; then
    last="$today"
    if docker compose pull --quiet relay; then
      docker compose up -d --no-build relay && echo "$(date -u '+%F %T') relais à jour"
    else
      echo "$(date -u '+%F %T') image du relais introuvable, nouvel essai demain" >&2
    fi
  fi
  sleep 600
done
