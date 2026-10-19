#!/bin/bash
# Lance le client Kyber macOS avec les métriques, puis analyse la session.
#
# Usage : tools/latency/run_client.sh <IP-du-PC> <utilisateur> [options kyclient...]
# Exemple : tools/latency/run_client.sh 192.168.1.100 matthieu --port 30241
#
# Écran virtuel (comme le feront les apps KyberNow) : définir l'adresse de la console.
#   KYBERNOW_CONSOLE=https://192.168.1.100:37462 tools/latency/run_client.sh …
#   KYBERNOW_MODE=2560x1600@120 pour imposer un mode (sinon : écran principal du Mac).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP="$ROOT/third_party/kyber-desktop/Kyber-Client.app/Contents/MacOS/kyclient"
HOST="${1:?IP du PC}"; USER_NAME="${2:?utilisateur}"; shift 2

read -r -s -p "Mot de passe Kyber pour $USER_NAME : " PASS; echo

RUN_DIR="$ROOT/tools/latency/runs/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN_DIR"
echo "$HOST $*" > "$RUN_DIR/params.txt"

HEARTBEAT_PID=""
if [ -n "${KYBERNOW_CONSOLE:-}" ]; then
    MODE="${KYBERNOW_MODE:-$(system_profiler SPDisplaysDataType -json | python3 -c '
import json, re, sys
for gpu in json.load(sys.stdin).get("SPDisplaysDataType", []):
    for d in gpu.get("spdisplays_ndrvs", []):
        if d.get("spdisplays_main") != "spdisplays_yes":
            continue
        w, h = map(int, re.findall(r"\d+", d.get("_spdisplays_pixels", "1920 x 1080"))[:2])
        hz = re.search(r"@ ([\d.]+)Hz", d.get("_spdisplays_resolution", ""))
        hz = round(float(hz.group(1))) if hz else 60
        # Écran HiDPI : macOS annonce plus de pixels que la dalle. On plafonne à la 4K.
        scale = min(1.0, 3840 / w, 2160 / h)
        print(f"{round(w * scale) // 2 * 2}x{round(h * scale) // 2 * 2}@{hz}")
        sys.exit()
print("1920x1080@60")
')}"
    W="${MODE%%x*}"; REST="${MODE#*x}"; H="${REST%%@*}"; HZ="${MODE##*@}"
    echo "Écran virtuel demandé : ${W} × ${H} à ${HZ} Hz"
    auth=(-sk -u "$USER_NAME:$PASS" -H "x-kyber-admin: 1")
    curl "${auth[@]}" -X POST "$KYBERNOW_CONSOLE/api/stream/prepare" -H "content-type: application/json" \
        -d "{\"width\":$W,\"height\":$H,\"refresh\":$HZ}" | tee "$RUN_DIR/prepare.json"; echo
    (while sleep 5; do curl "${auth[@]}" -X POST "$KYBERNOW_CONSOLE/api/stream/heartbeat" > /dev/null || true; done) &
    HEARTBEAT_PID=$!
    disown "$HEARTBEAT_PID"
    trap 'kill $HEARTBEAT_PID 2>/dev/null; curl "${auth[@]}" -X POST "$KYBERNOW_CONSOLE/api/stream/end" > /dev/null || true' EXIT
fi

cd "$RUN_DIR"
echo "Session en cours — fermer la fenêtre du client pour terminer (Ctrl+Alt+F : plein écran)."
"$APP" "$HOST" --tls-tofu --auth-username "$USER_NAME" --auth-password "$PASS" --metrics true --auto-reconnect true "$@" \
    > client.log 2>&1 || echo "Le client s'est terminé avec une erreur (voir $RUN_DIR/client.log)."

if [ -s metrics.json ]; then
    python3 "$ROOT/tools/latency/analyze_metrics.py" metrics.json | tee report.txt
else
    echo "Pas de metrics.json : voir $RUN_DIR/client.log"
fi
