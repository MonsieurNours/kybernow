#!/bin/bash
# Installateur de l'app Windows : apps/windows/dist/KyberNow-Client-Setup.exe,
# un seul fichier pour x64 et ARM64 (il pose la version du processeur).
#
# Prérequis : les deux dossiers de l'app, construits par
#   apps/windows/scripts/build.sh               (dist/x64)
#   apps/windows/scripts/build.sh --arch arm64  (dist/arm64)
# puis node, zstd, mingw-w64 (x86_64) et rustup (cible x86_64-pc-windows-gnu).
#
# Disposition (apps/windows/setup/src/payload.rs) :
#   [lanceur][x64.tar.zst][arm64.tar.zst][pied de 64 o « KNCLNT01 »]
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
SETUP="$HERE/setup"
TARGET=x86_64-pc-windows-gnu
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

for arch in x64 arm64; do
    [ -f "$HERE/dist/$arch/kybernow.exe" ] || { echo "$HERE/dist/$arch/kybernow.exe absent : lancez d'abord scripts/build.sh --arch $arch" >&2; exit 1; }
done
command -v zstd >/dev/null || { echo "zstd absent : brew install zstd" >&2; exit 1; }

STAGE="$SETUP/target/stage"
rm -rf "$STAGE" && mkdir -p "$STAGE"
FOOTER=()
for arch in x64 arm64; do
    src="$HERE/dist/$arch"
    files=$(find "$src" -type f | wc -l | tr -d ' ')
    bytes=$(find "$src" -type f -exec stat -f %z {} + | awk '{s+=$1} END {print s}')
    COPYFILE_DISABLE=1 tar --no-mac-metadata --format ustar -cf - -C "$src" . | zstd -q -19 -T0 -f -o "$STAGE/$arch.tar.zst"
    len=$(stat -f %z "$STAGE/$arch.tar.zst")
    echo "==> App $arch : $files fichiers, $((bytes / 1048576)) Mo, $((len / 1048576)) Mo compressés"
    FOOTER+=("$len" "$bytes" "$files")
done

echo "==> Interface (Svelte)"
(cd "$SETUP/ui" && npm ci --silent && npm run build --silent)

echo "==> Interface (kybernow-client-setup-ui.exe)"
cd "$SETUP"
touch src/bin/ui.rs
cargo build --release --target $TARGET --features ui --bin kybernow-client-setup-ui

echo "==> Lanceur (KyberNow-Client-Setup.exe)"
WV2_VERSION=$(awk '$0=="name = \"webview2-com-sys\""{getline; gsub(/"/,"",$3); print $3}' Cargo.lock)
LOADER=$(ls -d "$HOME"/.cargo/registry/src/index.crates.io-*/webview2-com-sys-"$WV2_VERSION"/x64/WebView2Loader.dll | head -1)
[ -f "$LOADER" ] || { echo "WebView2Loader.dll (x64, webview2-com-sys $WV2_VERSION) introuvable" >&2; exit 1; }
touch src/bin/setup.rs
KYBERNOW_SETUP_UI_EXE="$SETUP/target/$TARGET/release/kybernow-client-setup-ui.exe" \
KYBERNOW_WEBVIEW2_LOADER="$LOADER" \
    cargo build --release --target $TARGET --bin KyberNow-Client-Setup

OUT="$HERE/dist/KyberNow-Client-Setup.exe"
mkdir -p "$HERE/dist"
cat "$SETUP/target/$TARGET/release/KyberNow-Client-Setup.exe" "$STAGE/x64.tar.zst" "$STAGE/arm64.tar.zst" > "$OUT"
python3 - "$OUT" "${FOOTER[@]}" <<'EOF'
import struct, sys
out, n = sys.argv[1], [int(x) for x in sys.argv[2:]]
with open(out, "ab") as f:
    f.write(struct.pack("<7Q", *n, 0) + b"KNCLNT01")
EOF
shasum -a 256 "$OUT"
ls -lh "$OUT"
