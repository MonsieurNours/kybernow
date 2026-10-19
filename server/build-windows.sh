#!/bin/bash
# Compile l'interface Svelte puis kyber-admin.exe pour Windows (depuis macOS ou Linux).
# Prérequis : node, rustup target x86_64-pc-windows-gnu, mingw-w64 (brew install mingw-w64).
# Sortie : server/dist/kyber-admin.exe
set -euo pipefail

SERVER_DIR="$(cd "$(dirname "$0")" && pwd)"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

echo "==> Interface web"
(cd "$SERVER_DIR/web" && npm ci --silent && npm run build --silent)

echo "==> kyber-admin.exe"
cd "$SERVER_DIR/admin"
touch src/assets.rs # force la ré-inclusion de web/dist
cargo build --release --target x86_64-pc-windows-gnu

mkdir -p "$SERVER_DIR/dist"
cp target/x86_64-pc-windows-gnu/release/kyber-admin.exe "$SERVER_DIR/dist/"
ls -lh "$SERVER_DIR/dist/kyber-admin.exe"
