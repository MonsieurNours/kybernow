#!/bin/sh
# Mise à jour du relais (service « updater » du docker-compose.yml) : une fois
# par nuit, à RELAY_UPDATE_HOUR (UTC), reprend la dernière image du relais et
# ne relance le conteneur que si elle a changé. Le certificat (lego) et les
# réglages (.env, config/) ne sont pas touchés.
#
# Rien n'est repris (audit du 27/11, REL-20 et REL-28) :
# - quand l'image est épinglée par son empreinte (RELAY_IMAGE=…@sha256:…) ;
# - quand l'image en place a été construite ici, depuis les sources (aucune
#   empreinte de registre) : la mise à jour ne l'écrase pas ;
# - quand RELAY_COSIGN_KEY nomme une clé publique cosign (chemin dans ce
#   dossier) et que la signature de la nouvelle image ne s'y vérifie pas :
#   l'image précédente reprend son étiquette, le relais reste tel quel.
set -u
hour="$(printf '%02d' "${RELAY_UPDATE_HOUR:-4}")"
image="${RELAY_IMAGE:-ghcr.io/monsieurnours/kybernow-relay:latest}"
cosign_image="${RELAY_COSIGN_IMAGE:-ghcr.io/sigstore/cosign/cosign:v2.4.1}"
if [ "${RELAY_AUTO_UPDATE:-yes}" != "yes" ]; then
  echo "mise à jour automatique coupée (RELAY_AUTO_UPDATE)"
  exec sleep infinity
fi
case "$image" in
  *@sha256:*)
    echo "image épinglée par son empreinte ($image) : pas de mise à jour automatique"
    exec sleep infinity
    ;;
esac

# L'image en place vient-elle d'un registre ? (construite ici : aucune empreinte de registre)
built_here() {
  id="$(docker image inspect --format '{{.Id}}' "$image" 2>/dev/null)" || return 1
  [ -n "$id" ] && [ "$(docker image inspect --format '{{len .RepoDigests}}' "$image" 2>/dev/null)" = "0" ]
}

# Vérifie la signature cosign de l'image tirée, si une clé est donnée.
signed() {
  [ -z "${RELAY_COSIGN_KEY:-}" ] && return 0
  digest="$(docker image inspect --format '{{index .RepoDigests 0}}' "$image" 2>/dev/null)" || return 1
  [ -n "$digest" ] || return 1
  docker run --rm -v "$PWD/${RELAY_COSIGN_KEY}:/cle.pub:ro" "$cosign_image" verify --key /cle.pub "$digest" >/dev/null
}

echo "mise à jour du relais chaque nuit à ${hour} h (UTC)"
last=""
while true; do
  today="$(date -u +%F)"
  if [ "$(date -u +%H)" = "$hour" ] && [ "$today" != "$last" ]; then
    last="$today"
    if built_here; then
      echo "$(date -u '+%F %T') image construite ici ($image) : pas de mise à jour automatique" >&2
    else
      before="$(docker image inspect --format '{{.Id}}' "$image" 2>/dev/null || true)"
      if docker compose pull --quiet relay; then
        if signed; then
          docker compose up -d --no-build relay && echo "$(date -u '+%F %T') relais à jour"
        else
          echo "$(date -u '+%F %T') signature de la nouvelle image refusée : le relais reste tel quel" >&2
          [ -n "$before" ] && docker tag "$before" "$image"
        fi
      else
        echo "$(date -u '+%F %T') image du relais introuvable, nouvel essai demain" >&2
      fi
    fi
  fi
  sleep 600
done
