#!/bin/bash
# Construit KyberNow-Station-Setup.exe depuis macOS ou Linux.
#
# Prérequis : node, zstd, mingw-w64, rustup target x86_64-pc-windows-gnu,
# et les contenus extraits dans third_party/payload/ :
#   kyber-0.29.0/   (installeur officiel Kyber 0.29.0, extrait avec 7-Zip)
#   apollo-0.4.6/   (Apollo 0.4.6, pour le pilote SudoVDA et ViGEmBus)
#   kyber-overlay/  (facultatif : binaires corrigés et moteur web, voir server/kyber-patches)
#
# Sortie : server/installer/dist/KyberNow-Station-Setup.exe
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
PAYLOAD_SRC="$ROOT/third_party/payload"
STAGE="$HERE/target/stage"
DIST="$HERE/dist"
TARGET=x86_64-pc-windows-gnu
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

echo "==> KyberNow Admin (kyber-admin.exe)"
"$ROOT/server/build-windows.sh" > /dev/null

echo "==> Icône de la zone de notification (kybernow-tray.exe)"
(cd "$ROOT/server/tray" && cargo build --release --target $TARGET)

echo "==> Contenu à installer"
rm -rf "$STAGE" && mkdir -p "$STAGE"/{kyber,admin,tray,drivers/sudovda,drivers/vigembus,licenses}
rsync -a --exclude '$PLUGINSDIR' --exclude '*.bat' --exclude 'INSTALL.md' \
    "$PAYLOAD_SRC/kyber-0.29.0/" "$STAGE/kyber/"
# Surcouche corrigée de Kyber (server/kyber-patches/build.sh, docs/kyber-changes.md)
if [ -d "$PAYLOAD_SRC/kyber-overlay" ]; then
    # -R : la surcouche a des sous-dossiers (webclient/, moteur du client web).
    cp -R "$PAYLOAD_SRC/kyber-overlay/." "$STAGE/kyber/"
else
    echo "    (pas de surcouche : Kyber 0.29 d'origine, lancez server/kyber-patches/build.sh)"
fi
cp "$ROOT/server/dist/kyber-admin.exe" "$STAGE/admin/"
cp "$ROOT/server/tray/target/$TARGET/release/kybernow-tray.exe" "$STAGE/tray/"
# Le panneau de l'icône est une vue WebView2 : son chargeur l'accompagne.
cp "$(ls -d "$HOME"/.cargo/registry/src/index.crates.io-*/webview2-com-sys-*/x64/WebView2Loader.dll | tail -1)" "$STAGE/tray/"
for f in SudoVDA.dll SudoVDA.inf sudovda.cat sudovda.cer nefconc.exe; do
    cp "$PAYLOAD_SRC/apollo-0.4.6/drivers/sudovda/$f" "$STAGE/drivers/sudovda/"
done
cp "$PAYLOAD_SRC/apollo-0.4.6/scripts/vigembus_installer.exe" "$STAGE/drivers/vigembus/ViGEmBus_1.21.442_x64_x86_arm64.exe"
cp "$ROOT/third_party/kyber-installer/COPYING.AGPLv3" "$STAGE/licenses/AGPL-3.0.txt"
cp "$PAYLOAD_SRC/kyber-0.29.0/license.txt" "$STAGE/licenses/Kyber.txt"
cp "$HERE/resources/THIRD-PARTY.txt" "$STAGE/licenses/"

FILES=$(find "$STAGE" -type f | wc -l | tr -d ' ')
BYTES=$(find "$STAGE" -type f -exec stat -f %z {} + | awk '{s+=$1} END {print s}')
echo "    $FILES fichiers, $((BYTES / 1048576)) Mo"

echo "==> Compression (zstd)"
ARCHIVE="$HERE/target/payload.tar.zst"
COPYFILE_DISABLE=1 tar --no-mac-metadata --format ustar -cf - -C "$STAGE" . | zstd -q -19 -T0 -f -o "$ARCHIVE"
ARCHIVE_LEN=$(stat -f %z "$ARCHIVE")
echo "    $((ARCHIVE_LEN / 1048576)) Mo compressés"

echo "==> Interface (Svelte)"
(cd "$HERE/ui" && npm ci --silent && npm run build --silent)

echo "==> Interface (kybernow-ui.exe)"
cd "$HERE"
touch src/bin/ui.rs
cargo build --release --target $TARGET --features ui --bin kybernow-ui

echo "==> Lanceur (KyberNow-Station-Setup.exe)"
LOADER=$(ls -d "$HOME"/.cargo/registry/src/index.crates.io-*/webview2-com-sys-*/x64/WebView2Loader.dll | tail -1)
export KYBERNOW_UI_EXE="$HERE/target/$TARGET/release/kybernow-ui.exe"
export KYBERNOW_WEBVIEW2_LOADER="$LOADER"
touch src/bin/setup.rs
cargo build --release --target $TARGET --bin KyberNow-Station-Setup

echo "==> Assemblage"
mkdir -p "$DIST"
OUT="$DIST/KyberNow-Station-Setup.exe"
cat "$HERE/target/$TARGET/release/KyberNow-Station-Setup.exe" "$ARCHIVE" > "$OUT"
python3 - "$OUT" "$ARCHIVE_LEN" "$BYTES" "$FILES" <<'EOF'
import struct, sys
out, archive_len, unpacked, files = sys.argv[1], *map(int, sys.argv[2:])
with open(out, "ab") as f:
    f.write(struct.pack("<QQQ", archive_len, unpacked, files) + b"KNPAYLD1")
EOF
shasum -a 256 "$OUT" | tee "$OUT.sha256"
ls -lh "$OUT"
