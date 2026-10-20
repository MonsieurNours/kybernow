#!/bin/bash
# Recompile kyavserver.exe (et txproto) et kycontroller.exe pour Windows avec les correctifs
# KyberNow, depuis macOS, puis assemble la surcouche posée par l'installateur
# par-dessus les fichiers de Kyber 0.29.
#
# Correctifs :
#   0001  son HD : débit Opus selon le nombre de canaux, réglages
#         audio_quality (standard, hd, studio) et audio_bitrate
#   0002  curseur stable : masquage transmis après 250 ms seulement
#   0003  son continu : horodatage audio au fil des échantillons, silences à zéro
#   0004  son basse latence : capture et paquets Opus de 10 ms, Opus « lowdelay »
#   0005  image clé à la demande : POST /force_idr sur kycontroller (le client
#         la demande quand il a perdu une référence)
#
# Prérequis (Homebrew) : mingw-w64 meson ninja nasm cmake bison lua@5.4,
# rustup target x86_64-pc-windows-gnu, et les sources Kyber dans
# third_party/kyber-desktop (avec Jinja dans
# kysdk/kymedia/subprojects/libplacebo-*/3rdparty, voir docs/build-notes.md).
#
# Sortie : third_party/payload/kyber-overlay/
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
KYMEDIA="$ROOT/third_party/kyber-desktop/kysdk/kymedia"
KYCTL="$ROOT/third_party/kyber-desktop/kysdk/kyctl"
ROOTFS="$ROOT/third_party/rootfs-win64"
OVERLAY="$ROOT/third_party/payload/kyber-overlay"
MINGW=/opt/homebrew/opt/mingw-w64/toolchain-x86_64/x86_64-w64-mingw32

# pkg-config pour la cible : ne regarde que le rootfs Windows.
TOOLS="$ROOT/third_party/toolchain-bin"
mkdir -p "$TOOLS"
printf '#!/bin/sh\nexec pkg-config "$@"\n' > "$TOOLS/x86_64-w64-mingw32-pkg-config"
chmod +x "$TOOLS/x86_64-w64-mingw32-pkg-config"

export PATH="/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/lua@5.4/bin:$TOOLS:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export MINGW_PREFIX="$MINGW"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
export CARGO_PROFILE_RELEASE_STRIP=false
export BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu --sysroot=$MINGW -I$MINGW/include"

echo "==> Correctifs"
# Les correctifs se suivent et touchent parfois les mêmes fichiers : on part
# toujours des sources d'origine, puis on les applique tous, dans l'ordre.
# Ceux nommés *-kycontroller-* visent kysdk/kyctl, les autres kysdk/kymedia.
repo_of() { case "$(basename "$1")" in *-kycontroller-*) echo "$KYCTL" ;; *) echo "$KYMEDIA" ;; esac; }
for p in "$HERE"/*.patch; do
    repo=$(repo_of "$p")
    for f in $(grep -h '^+++ b/' "$p" | sed 's|^+++ b/||; s|[[:space:]].*||'); do
        case "$f" in
            subprojects/txproto/*) git -C "$repo/subprojects/txproto" checkout -q -- "${f#subprojects/txproto/}" ;;
            *) git -C "$repo" checkout -q -- "$f" ;;
        esac
    done
done
for p in "$HERE"/*.patch; do
    (cd "$(repo_of "$p")" && patch -p1 -s --forward --no-backup-if-mismatch < "$p")
    echo "    $(basename "$p")"
done
cd "$KYMEDIA"

echo "==> Compilation (dépendances, txproto, kyavserver)"
./build-win32.sh -w -o "$ROOTFS"

echo "==> Contrôleur (kycontroller.exe, mêmes options que la version officielle)"
(cd "$KYCTL" && cargo build -p kycontroller --target x86_64-pc-windows-gnu --release \
    --features=auth-jwt,auth-oidc,cli,kyhub,win32-hide-console,win32-tray 2>&1 | grep -E "^error|Finished" || true)

echo "==> Surcouche"
rm -rf "$OVERLAY" && mkdir -p "$OVERLAY"
for f in kyavserver.exe libtxproto-0.dll liblua.dll libspirv-cross-c-shared.dll; do
    cp "$ROOTFS/bin/$f" "$OVERLAY/"
done
cp "$KYCTL/target/x86_64-pc-windows-gnu/release/kycontroller.exe" "$OVERLAY/"
# Runtime MinGW (UCRT) de la chaîne kyavserver : aucun autre binaire de Kyber
# n'utilise ces DLL, on peut remplacer celles de la version officielle.
cp "$MINGW/lib/libstdc++-6.dll" "$MINGW/lib/libgcc_s_seh-1.dll" "$MINGW/bin/libwinpthread-1.dll" "$OVERLAY/"
x86_64-w64-mingw32-strip --strip-debug "$OVERLAY"/*.exe "$OVERLAY"/*.dll
ls -lh "$OVERLAY"
