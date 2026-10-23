#!/bin/sh
# Compilateur et éditeur de liens de la cible webOS (armv7-unknown-linux-gnueabi),
# pour cargo, cc-rs et cmake : le gcc de la chaîne de webosbrew avec sa racine
# système donnée explicitement (celle qu'il connaît d'office ne survit pas au
# déplacement de la chaîne), et libstdc++ en statique (celle
# du téléviseur change selon la version de webOS).
set -eu
: "${WEBOS_TOOLCHAIN:?WEBOS_TOOLCHAIN (posé par scripts/build.sh)}"
SYSROOT="$WEBOS_TOOLCHAIN/arm-webos-linux-gnueabi/sysroot"
n=$#
for arg in "$@"; do
    case "$arg" in
        -lstdc++) set -- "$@" "$SYSROOT/usr/lib/libstdc++.a" ;;
        *) set -- "$@" "$arg" ;;
    esac
done
shift "$n"
exec "$WEBOS_TOOLCHAIN/bin/arm-webos-linux-gnueabi-gcc" --sysroot="$SYSROOT" "$@"
