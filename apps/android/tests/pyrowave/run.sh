#!/bin/bash
# Essai hors ligne du décodage PyroWave d'Android (docs/pyrowave-client-android.md),
# sur l'appareil branché (adb), sans ordinateur distant :
#   1. la sonde du lancement (kn_pw_probe) ;
#   2. une mire (ffmpeg testsrc2, 120 images, BT.709 plage réduite) encodée SUR
#      L'APPAREIL par l'encodeur de l'amont (API C de PyroWave), une image = ses
#      paquets bout à bout comme un paquet média de la Station ;
#   3. décodée et présentée par le vrai chemin de l'app (device partagé,
#      décodeur, swapchain sur une surface AImageReader), cadencée à 60 i/s :
#      PSNR de la première image en RGB contre la source, image tronquée sautée,
#      temps moyen ; puis la même chose étiquetée HDR10 (plans R16, PQ → SDR).
#
#   apps/android/tests/pyrowave/run.sh                 1920x1080, 4:2:0 et 4:4:4
#   apps/android/tests/pyrowave/run.sh 1280x720        une autre définition
#   ANDROID_SERIAL=… apps/android/tests/pyrowave/run.sh   un appareil parmi plusieurs
#
# Prérequis : apps/android/scripts/build-native.sh (construit aussi l'outil
# kn_pyrowave_test, dans target/pyrowave-android/<abi>/), ffmpeg sur le Mac.
# Débit : 1,2 bit par pixel à 60 i/s (≈ 150 Mbit/s en 1080p).
# Si l'encodeur de l'amont refuse le GPU de l'appareil (il demande plus que le
# décodeur), l'essai le dit et s'arrête après la sonde.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
APPS="$(cd "$HERE/../../.." && pwd)"
ADB="${ADB:-$(command -v adb || echo "$HOME/Library/Android/sdk/platform-tools/adb")}"
size="${1:-1920x1080}"
w=${size%x*}; h=${size#*x}
mbps=$(( w * h * 60 * 12 / 10 / 1000000 ))

abi=$("$ADB" shell getprop ro.product.cpu.abi | tr -d '\r')
tool="${CARGO_TARGET_DIR:-$APPS/target}/pyrowave-android/$abi/kn_pyrowave_test"
[ -x "$tool" ] || { echo "Outil absent pour $abi : $tool (apps/android/scripts/build-native.sh)" >&2; exit 1; }
echo "Appareil : $("$ADB" shell getprop ro.product.model | tr -d '\r') ($abi, Android $("$ADB" shell getprop ro.build.version.release | tr -d '\r'))"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"; "$ADB" shell rm -rf /data/local/tmp/kn_pw >/dev/null 2>&1 || true' EXIT
D=/data/local/tmp/kn_pw
"$ADB" shell mkdir -p $D
"$ADB" push -q "$tool" $D/kn_pyrowave_test >/dev/null
"$ADB" shell $D/kn_pyrowave_test probe || exit 0

status=0
for c in 420 444; do
    ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=${size}:rate=60" -frames:v 120 \
        -vf "scale=out_color_matrix=bt709:out_range=tv" -pix_fmt "yuv${c}p" -f rawvideo "$WORK/src.yuv"
    head -c $(( w * h * (c == 420 ? 3 : 6) / 2 )) "$WORK/src.yuv" > "$WORK/first.yuv"
    "$ADB" push -q "$WORK/src.yuv" $D/src.yuv >/dev/null
    "$ADB" push -q "$WORK/first.yuv" $D/first.yuv >/dev/null
    if ! "$ADB" shell $D/kn_pyrowave_test encode $D/src.yuv $D/test.pwv "$w" "$h" "$c" "$mbps"; then
        echo "Encodeur de l'amont indisponible sur cet appareil : essai du décodage impossible ici." >&2
        exit 0
    fi
    "$ADB" shell $D/kn_pyrowave_test decode $D/test.pwv $D/first.yuv || status=1
    "$ADB" shell $D/kn_pyrowave_test decode $D/test.pwv $D/first.yuv hdr || status=1
done
exit $status
