#!/bin/bash
# Construit KyberNow-Station-Setup.exe depuis macOS (stat -f et
# tar --no-mac-metadata : pas depuis Linux).
#
# Prérequis : node, zstd, mingw-w64, rustup target x86_64-pc-windows-gnu,
# et les contenus extraits dans third_party/payload/ :
#   kyber-0.29.0/   (installeur officiel Kyber 0.29.0, extrait avec 7-Zip)
#   apollo-0.4.6/   (Apollo 0.4.6, pour le pilote SudoVDA et ViGEmBus)
#   kyber-overlay/  (facultatif : binaires corrigés et moteur web, voir server/kyber-patches)
#   vbcable/        (facultatif : VBCABLE_Driver_Pack de vb-cable.com extrait, micro virtuel)
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

# WebView2Loader.dll (x64) de la version de webview2-com-sys du Cargo.lock
# donné : jamais par tri alphabétique (0.9 passerait avant 0.39).
webview2_loader() {
    local version loader
    version=$(awk '$0=="name = \"webview2-com-sys\""{getline; gsub(/"/,"",$3); print $3}' "$1")
    loader=$(ls -d "$HOME"/.cargo/registry/src/index.crates.io-*/webview2-com-sys-"$version"/x64/WebView2Loader.dll 2>/dev/null | head -1)
    [ -n "$version" ] && [ -f "$loader" ] || { echo "WebView2Loader.dll (x64, webview2-com-sys ${version:-?} de $1) introuvable : cargo fetch" >&2; exit 1; }
    echo "$loader"
}

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
cp "$(webview2_loader "$ROOT/server/tray/Cargo.lock")" "$STAGE/tray/"
for f in SudoVDA.dll SudoVDA.inf sudovda.cat sudovda.cer nefconc.exe; do
    cp "$PAYLOAD_SRC/apollo-0.4.6/drivers/sudovda/$f" "$STAGE/drivers/sudovda/"
done
cp "$PAYLOAD_SRC/apollo-0.4.6/scripts/vigembus_installer.exe" "$STAGE/drivers/vigembus/ViGEmBus_1.21.442_x64_x86_arm64.exe"
# VB-CABLE (micro de l'appareil, docs/micro.md) : le pack de VB-Audio
# (VBCABLE_Driver_Pack*.zip, vb-cable.com) extrait dans third_party/payload/vbcable/,
# livré tel quel (donationware ; ne jamais renommer ni retoucher ses fichiers).
if [ -f "$PAYLOAD_SRC/vbcable/VBCABLE_Setup_x64.exe" ]; then
    mkdir -p "$STAGE/drivers/vbcable"
    cp -R "$PAYLOAD_SRC/vbcable/." "$STAGE/drivers/vbcable/"
else
    echo "    (pas de VB-CABLE dans $PAYLOAD_SRC/vbcable : la Station n'aura pas de micro virtuel)"
fi
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
LOADER=$(webview2_loader "$HERE/Cargo.lock")
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
