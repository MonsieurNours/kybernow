#!/bin/bash
# Règles de session sans interface (iPhone, iPad, Apple TV), sans simulateur :
# veille retenue, perte du premier plan, menu pendant la reconnexion.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$HERE/../.."
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
swiftc -o "$WORK/session-rules" "$HERE/main.swift" "$APP/KyberNowIOS/SessionRules.swift"
"$WORK/session-rules"
