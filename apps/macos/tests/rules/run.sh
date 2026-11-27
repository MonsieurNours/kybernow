#!/bin/bash
# Essai des règles sans état de l'app Mac (KyberNow/SessionRules.swift) :
# garde de fin de session, modificateurs du mode immersif.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
swiftc -O -o "$WORK/rules" "$HERE/main.swift" "$HERE/../../KyberNow/SessionRules.swift"
"$WORK/rules"
