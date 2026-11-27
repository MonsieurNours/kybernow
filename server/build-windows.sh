#!/bin/bash
# Compile l'interface Svelte puis kyber-admin.exe (KyberNow Admin) pour Windows (depuis macOS ou Linux,
# shasum de Perl requis).
# Prérequis : node, rustup target x86_64-pc-windows-gnu, mingw-w64 (brew install mingw-w64).
# Sortie : server/dist/kyber-admin.exe
set -euo pipefail

SERVER_DIR="$(cd "$(dirname "$0")" && pwd)"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

echo "==> Interface web"
# npm ci seulement si package-lock.json a changé depuis la dernière installation.
if [ ! -f "$SERVER_DIR/web/node_modules/.package-lock.json" ] || [ "$SERVER_DIR/web/package-lock.json" -nt "$SERVER_DIR/web/node_modules/.package-lock.json" ]; then
    (cd "$SERVER_DIR/web" && npm ci --silent)
fi
(cd "$SERVER_DIR/web" && npm run build --silent)

echo "==> kyber-admin.exe"
cd "$SERVER_DIR/admin"
# web/dist est inclus à la compilation (rust-embed) : Admin n'est recompilé
# pour lui que si son contenu a changé.
STAMP="target/web-dist.stamp"
DIST_SUM=$(cd "$SERVER_DIR/web/dist" && find . -type f -print0 | sort -z | xargs -0 shasum -a 256 | shasum -a 256 | cut -d' ' -f1)
if [ "$(cat "$STAMP" 2>/dev/null)" != "$DIST_SUM" ]; then
    touch src/assets.rs
fi
cargo build --release --target x86_64-pc-windows-gnu
mkdir -p target && echo "$DIST_SUM" > "$STAMP"

mkdir -p "$SERVER_DIR/dist"
cp target/x86_64-pc-windows-gnu/release/kyber-admin.exe "$SERVER_DIR/dist/"
ls -lh "$SERVER_DIR/dist/kyber-admin.exe"
