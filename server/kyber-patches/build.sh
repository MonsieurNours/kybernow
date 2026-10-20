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
#   0004  son basse latence : paquets Opus de 10 ms, Opus « lowdelay »
#   0005  image clé à la demande : POST /force_idr sur kycontroller (le client
#         la demande quand il a perdu une référence), une par session toutes
#         les 150 ms au plus
#   0006  pointeur : ses déplacements aussi (position seule, 60 par seconde au plus)
#   0007  envoi de l'image (kynet) : tailles des files de datagrammes QUIC et
#         des mémoires UDP en options (quinn jetait les plus anciens morceaux
#         d'une grosse image)
#   0008  moteur libavconv (kyavservice) : écrans relus sans carte imposée et
#         identifiants stables comme txproto, déplacements et délai de masquage
#         du pointeur, son 5.1/7.1 (ou stéréo : audio_layout) en paquets Opus de
#         10 ms « lowdelay »
#   0009  moteur libavconv (libavgrab) : son horodaté au fil des échantillons,
#         silences à zéro, nombre de voies des sorties audio
#   0010  politique d'envoi (kycontroller) : [kycontroller] datagram_send_queue
#         = "deep" (8 Mo, par défaut) ou "short" (3 Mo) ; file de réception et
#         mémoires UDP à 8 Mo
#
# Prérequis (Homebrew) : mingw-w64 meson ninja nasm cmake bison lua@5.4,
# rustup target x86_64-pc-windows-gnu, et les sources Kyber dans
# third_party/kyber-desktop (avec Jinja dans
# kysdk/kymedia/subprojects/libplacebo-*/3rdparty, voir docs/build-notes.md).
# Versions vérifiées : scripts/kyber-toolchain.sh (une autre est signalée).
#
# Sortie : third_party/payload/kyber-overlay/. Le script s'arrête à la première
# erreur (correctif inapplicable, compilation ratée) sans toucher à la surcouche
# précédente, et n'y copie que des binaires produits par cette exécution.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# Chemin réel de third_party/ (un lien dans un worktree) : meson et cargo
# gardent ainsi leurs dossiers de build, liés au chemin absolu des sources.
THIRD_PARTY="$(cd "$ROOT/third_party" 2>/dev/null && pwd -P)" || { echo "third_party/ introuvable (docs/build-notes.md)" >&2; exit 1; }
. "$ROOT/scripts/kyber-toolchain.sh"

# Repère du début : un binaire plus ancien n'a pas été produit par ce build.
STARTED=$(mktemp -t kyber-build)
trap 'rm -f "$STARTED"' EXIT

KYMEDIA="$THIRD_PARTY/kyber-desktop/kysdk/kymedia"
KYCTL="$THIRD_PARTY/kyber-desktop/kysdk/kyctl"
KYMUX="$THIRD_PARTY/kyber-desktop/kysdk/kymux"
ROOTFS="$THIRD_PARTY/rootfs-win64"
OVERLAY="$THIRD_PARTY/payload/kyber-overlay"
MINGW=/opt/homebrew/opt/mingw-w64/toolchain-x86_64/x86_64-w64-mingw32

# pkg-config pour la cible : ne regarde que le rootfs Windows.
TOOLS="$THIRD_PARTY/toolchain-bin"
mkdir -p "$TOOLS"
printf '#!/bin/sh\nexec pkg-config "$@"\n' > "$TOOLS/x86_64-w64-mingw32-pkg-config"
chmod +x "$TOOLS/x86_64-w64-mingw32-pkg-config"

export PATH="/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/lua@5.4/bin:$TOOLS:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export MINGW_PREFIX="$MINGW"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
export CARGO_PROFILE_RELEASE_STRIP=false
export BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu --sysroot=$MINGW -I$MINGW/include"

kyber_toolchain_check rustc mingw meson

# Sortie de cargo sans les lignes de progression ; avec pipefail, le code de
# retour reste celui de cargo.
cargo_quiet() {
    "$@" 2>&1 | { grep -Ev '^ +(Compiling|Checking|Fresh|Downloaded|Downloading|Locking|Adding|Updating) ' || true; }
}

# Refuse un binaire plus ancien que le début du script : il viendrait d'un
# build précédent, sans les correctifs de celui-ci.
check_fresh() {
    local f
    for f in "$@"; do
        [ -e "$f" ] || { echo "$f introuvable : surcouche non mise à jour" >&2; exit 1; }
        [ "$f" -nt "$STARTED" ] && continue
        echo "$f n'a pas été produit par ce build (daté du $(date -r "$f" '+%d/%m %H:%M')) : surcouche non mise à jour" >&2
        exit 1
    done
}

echo "==> Correctifs"
# Les correctifs se suivent et touchent parfois les mêmes fichiers : on part
# toujours des sources d'origine, puis on les applique tous, dans l'ordre.
# Ceux nommés *-kycontroller-* visent kysdk/kyctl, *-kynet-* kysdk/kymux,
# les autres kysdk/kymedia. Seuls les fichiers des correctifs sont remis à
# l'origine : kyctl et kymux portent aussi les correctifs des apps.
repo_of() { case "$(basename "$1")" in *-kycontroller-*) echo "$KYCTL" ;; *-kynet-*) echo "$KYMUX" ;; *) echo "$KYMEDIA" ;; esac; }
for p in "$HERE"/*.patch; do
    repo=$(repo_of "$p")
    for f in $(grep -h '^+++ b/' "$p" | sed 's|^+++ b/||; s|[[:space:]].*||'); do
        case "$f" in
            subprojects/txproto/*) dir="$repo/subprojects/txproto" f="${f#subprojects/txproto/}" ;;
            subprojects/libavconv/*) dir="$repo/subprojects/libavconv" f="${f#subprojects/libavconv/}" ;;
            *) dir="$repo" ;;
        esac
        if git -C "$dir" cat-file -e "HEAD:$f" 2>/dev/null; then
            git -C "$dir" checkout -q HEAD -- "$f"
        else
            rm -f "$dir/$f"     # ajouté par le correctif : absent de l'origine
        fi
    done
done
for p in "$HERE"/*.patch; do
    echo "    $(basename "$p")"
    (cd "$(repo_of "$p")" && patch -p1 --forward --no-backup-if-mismatch -r - < "$p" | sed 's/^/        /') || {
        echo "Correctif $(basename "$p") inapplicable (hunk en échec ci-dessus)" >&2
        exit 1
    }
done
cd "$KYMEDIA"

echo "==> Compilation (dépendances, txproto, kyavserver)"
./build-win32.sh -w -o "$ROOTFS"

echo "==> Contrôleur (kycontroller.exe, mêmes options que la version officielle)"
(cd "$KYCTL" && cargo_quiet cargo build -p kycontroller --target x86_64-pc-windows-gnu --release \
    --features=auth-jwt,auth-oidc,cli,kyhub,win32-hide-console,win32-tray)

echo "==> Surcouche"
KYCONTROLLER="$KYCTL/target/x86_64-pc-windows-gnu/release/kycontroller.exe"
check_fresh "$ROOTFS/bin/kyavserver.exe" "$ROOTFS/bin/libtxproto-0.dll" "$KYCONTROLLER"
rm -rf "$OVERLAY" && mkdir -p "$OVERLAY"
for f in kyavserver.exe libtxproto-0.dll liblua.dll libspirv-cross-c-shared.dll; do
    cp "$ROOTFS/bin/$f" "$OVERLAY/"
done
cp "$KYCONTROLLER" "$OVERLAY/"
# Runtime MinGW (UCRT) de la chaîne kyavserver : aucun autre binaire de Kyber
# n'utilise ces DLL, on peut remplacer celles de la version officielle.
cp "$MINGW/lib/libstdc++-6.dll" "$MINGW/lib/libgcc_s_seh-1.dll" "$MINGW/bin/libwinpthread-1.dll" "$OVERLAY/"
x86_64-w64-mingw32-strip --strip-debug "$OVERLAY"/*.exe "$OVERLAY"/*.dll
ls -lh "$OVERLAY"
