#!/bin/bash
# Test hors ligne du presse-papiers partagé (voir main.swift).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
MAC="$HERE/../.."
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
swiftc -O -o "$WORK/clipboard" "$HERE/main.swift" "$MAC/KyberKit/ClipboardBridge.swift" \
    -I "$MAC/Vendor/include" -L "$MAC/Vendor/Frameworks" -lkynput.0.1 -lkyclient.0.1 \
    -Xlinker -rpath -Xlinker "$MAC/Vendor/Frameworks" 2>&1 | grep -E "error" || true
"$WORK/clipboard"
