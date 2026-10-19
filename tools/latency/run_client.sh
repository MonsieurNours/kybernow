#!/bin/bash
# Lance le client Kyber macOS avec les métriques, puis analyse la session.
# Usage : tools/latency/run_client.sh <IP-du-PC> <utilisateur> [options kyclient...]
# Exemple : tools/latency/run_client.sh 192.168.1.100 matthieu --video-codec hevc
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP="$ROOT/third_party/kyber-desktop/Kyber-Client.app/Contents/MacOS/kyclient"
HOST="${1:?IP du PC}"; USER_NAME="${2:?utilisateur}"; shift 2

read -r -s -p "Mot de passe Kyber pour $USER_NAME : " PASS; echo

RUN_DIR="$ROOT/tools/latency/runs/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN_DIR"
echo "$HOST $*" > "$RUN_DIR/params.txt"

cd "$RUN_DIR"
echo "Session en cours — fermer la fenêtre du client pour terminer (Ctrl+Alt+F : plein écran)."
"$APP" "$HOST" --tls-tofu --auth-username "$USER_NAME" --auth-password "$PASS" --metrics true "$@" \
    > client.log 2>&1 || echo "Le client s'est terminé avec une erreur (voir $RUN_DIR/client.log)."

if [ -s metrics.json ]; then
    python3 "$ROOT/tools/latency/analyze_metrics.py" metrics.json | tee report.txt
else
    echo "Pas de metrics.json : voir $RUN_DIR/client.log"
fi
