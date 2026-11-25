#!/bin/bash
# Kit de mesure de PyroWave pour un Raspberry Pi 5 (docs/pyrowave-client-pi.md),
# fait dans la VM « KyberNow Linux » (aarch64) : le vérificateur hors ligne
# (check.c et src/native/pyrowave.c, comme run.sh) compilé contre le préfixe du
# Pi, et des flux encodés sur le Mac (samples-mac.sh DOSSIER 1920x1080 60). Sur
# le Pi, rien à compiler : la bibliothèque PyroWave est celle du paquet
# (/usr/lib/libpyrowave-shared.so.1), le pilote celui du système.
#
#   apps/linux/tests/pyrowave/pi-kit.sh FLUX SORTIE [--444]
#
# FLUX : le dossier de samples-mac.sh ; SORTIE : le dossier à copier sur le Pi
# (`scp -r SORTIE kybernow@<pi>:`), puis, sur le Pi : `SORTIE/mesure.sh`.
# --444 : le flux 4:4:4 aussi (sa source pèse 370 Mo en 1080p).
# PREFIX : le préfixe du Pi (third_party/linux-aarch64-rpi par défaut).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$HERE/../.."
ROOT="$APP/../.."
PREFIX="${PREFIX:-$ROOT/third_party/linux-$(uname -m)-rpi}"
FLUX="$(cd "${1:?usage : pi-kit.sh FLUX SORTIE [--444]}" && pwd)"
OUT="${2:?usage : pi-kit.sh FLUX SORTIE [--444]}"
WITH444=0
[ "${3:-}" = --444 ] && WITH444=1
[ "$(uname -m)" = aarch64 ] || { echo "à faire sur un Linux aarch64 (la VM)" >&2; exit 1; }
mkdir -p "$OUT"
# Le préfixe du Pi a le FFmpeg de Raspberry Pi (libudev) ; KN_PYROWAVE_DIR vide :
# la bibliothèque du paquet (/usr/lib), ou KYBERNOW_PYROWAVE_LIB.
cc -O2 -o "$OUT/check" "$HERE/check.c" "$APP/src/native/pyrowave.c" "$APP/src/native/decoder.c" -I"$PREFIX/include" \
    "$PREFIX/lib/libavcodec.a" "$PREFIX/lib/libavutil.a" -lva -lva-drm -ldrm -ludev -ldl -lm -lpthread -lgcc
n=0
for f in "$FLUX"/*-420.pyrowave $([ "$WITH444" = 1 ] && echo "$FLUX"/*-444.pyrowave); do
    [ -f "$f" ] || continue
    name="$(basename "$f" .pyrowave)"
    cp "$f" "$FLUX/$name.yuv" "$OUT/"
    n=$((n + 1))
done
[ "$n" -gt 0 ] || { echo "aucun flux dans $FLUX (samples-mac.sh)" >&2; exit 1; }
cat > "$OUT/mesure.sh" <<'SH'
#!/bin/bash
# Sur le Raspberry Pi : la carte et le pilote, puis chaque flux (sonde, PSNR,
# décodage relecture comprise cadencé à 60 i/s et à la chaîne). Le chemin par
# fragments est celui que l'app prend sur v3dv ; KYBERNOW_PYROWAVE_CPU=1 n'est
# pas mis (le Pi a une vraie carte graphique).
cd "$(dirname "$0")"
command -v vulkaninfo >/dev/null && vulkaninfo --summary 2>/dev/null | grep -E "deviceName|driverName|driverInfo|apiVersion" | head -4
fail=0
for p in *.pyrowave; do
    echo "== ${p%.pyrowave}"
    ./check "$p" "${p%.pyrowave}.yuv" || fail=1
done
exit $fail
SH
chmod +x "$OUT/mesure.sh"
du -sh "$OUT"
