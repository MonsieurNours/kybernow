#!/bin/bash
# Essai hors ligne du décodage des extraits du profil Auto (docs/profil-auto.md),
# sans ordinateur distant : les six extraits du cœur (apps/core/samples) décodés
# par AutoTestBench.run, comme à l'étape « Décodage de cet appareil » du test,
# chacun 1 s au plus. Échoue si un extrait que la puce sait décoder ne l'est
# pas ; un plantage (0.11.2 : sémaphore rendu en cours d'usage) arrête tout.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="$HERE/../../KyberKit"
PW="$HERE/../../PyroWave"
SAMPLES="$HERE/../../../core/samples"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# Une compilation ratée arrête l'essai (MAC-39 : `| grep error || true`
# laissait lancer un binaire absent ou d'avant).
compile() {
    local log
    if ! log="$(swiftc "$@" 2>&1)"; then
        echo "$log" | grep -E "error" >&2 || echo "$log" >&2
        echo "Compilation de l'essai ratée." >&2
        exit 1
    fi
}
cd "$WORK"

for f in pyrowave_common.mm pyrowave_decoder.mm pyrowave_bitstream.cpp; do
    clang++ -std=c++17 -O2 -fobjc-arc -c "$PW/upstream/$f" -o "$f.o"
done
# Le paquet de configuration de PyroWave est lu par le cœur (libkybernow de Vendor/).
VENDOR="$HERE/../../Vendor"
compile -O -I "$PW/module" -I "$VENDOR/KyberNow/include" -L "$VENDOR/Frameworks" -lkybernow -Xlinker -rpath -Xlinker "$VENDOR/Frameworks" \
    -framework Metal -framework IOSurface -lc++ ./*.o -o autotest \
    "$HERE/main.swift" "$KIT/AutoTestBench.swift" "$KIT/VideoDecoder.swift" "$KIT/PyroWaveDecoder.swift" \
    "$KIT/PyroWaveConfig.swift" "$KIT/AV1.swift"
./autotest "$SAMPLES"/hevc-420.kns "$SAMPLES"/hevc-444.kns "$SAMPLES"/av1-420.kns \
    "$SAMPLES"/h264-420.kns "$SAMPLES"/pyrowave-420.kns "$SAMPLES"/pyrowave-444.kns
