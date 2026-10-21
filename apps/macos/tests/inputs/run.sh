#!/bin/bash
# Test hors ligne du module d'entrées partagé (voir main.swift) : Keymap,
# PadCombo, file de secours de PacketQueue, filtrage des axes.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
MAC="$HERE/../.."
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
KIT="$MAC/KyberKit"
swiftc -O -o "$WORK/inputs" "$HERE/main.swift" \
    "$KIT/Keymap.swift" "$KIT/PadCombo.swift" "$KIT/PacketQueue.swift" "$KIT/InputPipeline.swift" \
    "$KIT/GamepadGate.swift" "$KIT/PointerLayer.swift" "$KIT/ClipboardBridge.swift" "$KIT/PasteboardMac.swift" "$KIT/KyberLog.swift" \
    -I "$MAC/Vendor/include" -L "$MAC/Vendor/Frameworks" -lkynput.0.1 -lkyclient.0.1 \
    -Xlinker -rpath -Xlinker "$MAC/Vendor/Frameworks" 2>&1 | grep -E "error" || true
"$WORK/inputs"
