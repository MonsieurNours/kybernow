#!/bin/bash
# Recompile pour Windows, depuis macOS et avec les correctifs KyberNow,
# kyavserver.exe (et txproto), kycontroller.exe, le serveur d'entrées
# (kynput.dll, kynputserver.exe) et le moteur du client web (kyclient_wasm),
# puis assemble la surcouche posée par l'installateur par-dessus les fichiers
# de Kyber 0.29.
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
#   0006  pointeur : ses déplacements aussi (position seule), au plus
#         [kyavserver] cursor_move_rate par seconde (60 par défaut, 120 au plus)
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
#   0011  pointeur (serveur d'entrées kynput) : les déplacements partent sans
#         la forme (CursorData::None), la forme n'est remise qu'au besoin
#   0012  kynputserver : compilable depuis macOS (build.rs testait la machine
#         qui compile au lieu de la cible)
#   0013  moteur web (kynput wasm) : texte des champs data-kynput-text en
#         paquets Unicode, tout relâcher (blur, onglet masqué, capture
#         perdue), manettes « standard » seulement, boucle rAF arrêtée
#   0014  moteur web (kywebplayer) : flush() avant configure(), images
#         intermédiaires écartées jusqu'à l'image clé, décodeur recréé après
#         une erreur
#   0015  moteur web (kyber-web) : position absolue tant que le pointeur n'est
#         pas capturé
#   0016  débit par écran (kycontroller) : le débit d'une session à plusieurs
#         écrans se répartit selon les pixels par seconde de chaque écran
#         (parts égales si un mode est inconnu), au démarrage et à /set_bitrate
#   (0017, diagnostic de la capture des écrans secondaires : retiré, son
#         rôle est fini)
#   0018  capture des écrans secondaires (txproto) : pour un écran dont
#         KyberNow Admin a lancé la capture Windows Graphics Capture, les
#         images viennent de ses textures D3D11 partagées au lieu de la
#         duplication DXGI (agent attendu 1,5 s, dernière image reprise dès
#         l'ouverture et renvoyée chaque seconde, repli sur la duplication
#         sans image en 2,5 s) ; pointeur lu par GetCursorInfo ; méthode,
#         première image et latence de capture au journal
#
# Prérequis (Homebrew) : mingw-w64 meson ninja nasm cmake bison lua@5.4
# binaryen, wasm-bindgen-cli (cargo install), rustup target
# wasm32-unknown-unknown, le client web de Kyber 0.29 dans third_party/kyber-web,
# Kyber 0.29 extrait dans third_party/payload/kyber-0.29.0 (sa SDL2.dll),
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
KYNPUT="$THIRD_PARTY/kyber-desktop/kysdk/kynput"
# Client web de Kyber 0.29 (crate kyclient-wasm, l'API JS du moteur), même
# kysdk que kyber-desktop.
KYWEB="$THIRD_PARTY/kyber-web"
# kynput.dll et son en-tête, installés à part du rootfs de kyavserver.
KYNPUT_ROOTFS="$THIRD_PARTY/rootfs-win64-kynput"
# Kyber 0.29 d'origine : kynput.dll se lie à sa SDL2.dll (manettes du client
# Kyber installé sur l'ordinateur), qu'on ne recompile pas.
KYBER_PAYLOAD="$THIRD_PARTY/payload/kyber-0.29.0"
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

kyber_toolchain_check rustc mingw meson wasm-bindgen binaryen
for tool in wasm-bindgen wasm-opt; do
    command -v $tool >/dev/null || { echo "$tool introuvable (docs/build-notes.md, moteur web)" >&2; exit 1; }
done
[ -f "$KYWEB/Cargo.toml" ] || { echo "$KYWEB introuvable : client web de Kyber 0.29 (docs/build-notes.md, moteur web)" >&2; exit 1; }

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
# Ceux nommés *-kycontroller-* et *-kywebplayer-* visent kysdk/kyctl,
# *-kynet-* kysdk/kymux, *-kynput-* kysdk/kynput, *-kyberweb-* kyber-web, les
# autres kysdk/kymedia. Seuls les fichiers des correctifs sont remis à
# l'origine : kyctl et kymux portent aussi les correctifs des apps.
repo_of() {
    case "$(basename "$1")" in
        *-kycontroller-* | *-kywebplayer-*) echo "$KYCTL" ;;
        *-kyberweb-*) echo "$KYWEB" ;;
        *-kynet-*) echo "$KYMUX" ;;
        *-kynput-*) echo "$KYNPUT" ;;
        *) echo "$KYMEDIA" ;;
    esac
}
# Piège : les correctifs des apps (apps/macos, apps/webos) posés par leurs
# scripts dans kyctl, kymux et kynput touchent parfois les mêmes fichiers
# (kynput/Cargo.toml : 0013 et le correctif webOS). Remis à l'origine, ces
# fichiers perdraient leur part, et le script de l'app trouverait ensuite son
# correctif à moitié posé, ni applicable ni retirable. Ceux qui sont posés en
# entier sont donc retirés d'abord (du dernier au premier), puis reposés après
# ceux du serveur.
client_repo_of() {
    case "$(basename "$1")" in
        *-kyclient-*) echo "$KYCTL" ;;
        *-kynput-*) echo "$KYNPUT" ;;
        *-kynet-*) echo "$KYMUX" ;;
    esac
}
CLIENT_APPLIED=()
for p in "$ROOT"/apps/macos/kyber-patches/*.patch "$ROOT"/apps/webos/kyber-patches/*.patch; do
    repo=$(client_repo_of "$p")
    [ -n "$repo" ] && git -C "$repo" apply --reverse --check "$p" 2>/dev/null && CLIENT_APPLIED=("$p" ${CLIENT_APPLIED[@]+"${CLIENT_APPLIED[@]}"})
done
for p in ${CLIENT_APPLIED[@]+"${CLIENT_APPLIED[@]}"}; do
    git -C "$(client_repo_of "$p")" apply --reverse "$p"
done
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
REAPPLY=()
for p in ${CLIENT_APPLIED[@]+"${CLIENT_APPLIED[@]}"}; do REAPPLY=("$p" ${REAPPLY[@]+"${REAPPLY[@]}"}); done
for p in ${REAPPLY[@]+"${REAPPLY[@]}"}; do
    repo=$(client_repo_of "$p")
    if git -C "$repo" apply --check "$p" 2>/dev/null; then
        git -C "$repo" apply "$p" && echo "    $(basename "$p") (app) reposé"
    elif ! git -C "$repo" apply --reverse --check "$p" 2>/dev/null; then
        echo "Correctif d'app $(basename "$p") à moitié posé : remettre $repo à l'origine (git -C $repo checkout -- .), puis relancer" >&2
        exit 1
    fi
done
cd "$KYMEDIA"

echo "==> Compilation (dépendances, txproto, kyavserver)"
./build-win32.sh -w -o "$ROOTFS"

echo "==> Contrôleur (kycontroller.exe, mêmes options que la version officielle)"
(cd "$KYCTL" && cargo_quiet cargo build -p kycontroller --target x86_64-pc-windows-gnu --release \
    --features=auth-jwt,auth-oidc,cli,kyhub,win32-hide-console,win32-tray)

echo "==> Serveur d'entrées (kynput.dll, kynputserver.exe)"
[ -f "$KYBER_PAYLOAD/SDL2.dll" ] || { echo "$KYBER_PAYLOAD/SDL2.dll introuvable (Kyber 0.29 extrait, voir server/installer/build.sh)" >&2; exit 1; }
# sdl2-sys trouve la SDL par pkg-config : un .pc qui pointe la DLL de Kyber
# (l'éditeur de liens de MinGW lie une DLL sans bibliothèque d'import).
SDL2_PC="$TOOLS/pkgconfig-sdl2"
mkdir -p "$SDL2_PC"
printf 'Name: sdl2\nDescription: SDL2.dll de Kyber 0.29\nVersion: 2.0.0\nLibs: -L%s -lSDL2\nCflags:\n' "$KYBER_PAYLOAD" > "$SDL2_PC/sdl2.pc"
rm -f "$KYNPUT_ROOTFS/bin/kynput.dll"
(cd "$KYNPUT" && PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_LIBDIR="$SDL2_PC" cargo_quiet cargo cinstall -p kynput \
    --target x86_64-pc-windows-gnu --dlltool x86_64-w64-mingw32-dlltool --release \
    --prefix="$KYNPUT_ROOTFS" --features sdl2)
(cd "$KYNPUT" && PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_LIBDIR="$KYNPUT_ROOTFS/lib/pkgconfig" cargo_quiet cargo build \
    -p kynputserver --target x86_64-pc-windows-gnu --release --features win32-hide-console)

echo "==> Moteur web (kyclient_wasm)"
# kyber-web compile kyclient et kywebplayer depuis son dossier kysdk. Le
# kysdk/kyctl de third_party porte aussi les correctifs des apps, qui changent
# l'API de kyclient : le moteur web part d'un kyctl d'origine (git archive),
# plus les correctifs *-kywebplayer-*. Les autres dépôts sont ceux de
# third_party, correctifs serveur appliqués (kynput, kymux…).
WEB_KYSDK="$THIRD_PARTY/kyber-web-kysdk"
rm -rf "$WEB_KYSDK/kyctl" && mkdir -p "$WEB_KYSDK/kyctl"
git -C "$KYCTL" archive HEAD | tar -x -C "$WEB_KYSDK/kyctl"
for p in "$HERE"/*-kywebplayer-*.patch; do
    [ -e "$p" ] || continue
    (cd "$WEB_KYSDK/kyctl" && patch -p1 --forward --no-backup-if-mismatch -r - < "$p" | sed 's/^/        /') || {
        echo "Correctif $(basename "$p") inapplicable au kyctl d'origine" >&2
        exit 1
    }
done
for r in kymedia kymux kynput kyutil; do
    ln -sfn "$THIRD_PARTY/kyber-desktop/kysdk/$r" "$WEB_KYSDK/$r"
done
# kysdk : sous-module vide dans un clone de kyber-web, remplacé par ce lien.
[ -L "$KYWEB/kysdk" ] || rmdir "$KYWEB/kysdk"
ln -sfn "$WEB_KYSDK" "$KYWEB/kysdk"
(cd "$KYWEB" && RUSTFLAGS=--cfg=web_sys_unstable_apis cargo_quiet cargo build \
    --target wasm32-unknown-unknown -p kyclient-wasm --release)
WEB_OUT="$THIRD_PARTY/kyber-web-out"
rm -rf "$WEB_OUT" && mkdir -p "$WEB_OUT"
check_fresh "$KYWEB/target/wasm32-unknown-unknown/release/kyclient_wasm.wasm"
# Mêmes étapes que le build-wasm.sh de kyber-web (sans l'interface de Kyber,
# inchangée) : liaison JS, puis optimisation en gardant les noms.
wasm-bindgen --target web --no-typescript --out-dir "$WEB_OUT" \
    "$KYWEB/target/wasm32-unknown-unknown/release/kyclient_wasm.wasm"
wasm-opt "$WEB_OUT/kyclient_wasm_bg.wasm" -o "$WEB_OUT/kyclient_wasm_bg.wasm" -Os -g

echo "==> Surcouche"
KYCONTROLLER="$KYCTL/target/x86_64-pc-windows-gnu/release/kycontroller.exe"
KYNPUTSERVER="$KYNPUT/target/x86_64-pc-windows-gnu/release/kynputserver.exe"
check_fresh "$ROOTFS/bin/kyavserver.exe" "$ROOTFS/bin/libtxproto-0.dll" "$KYCONTROLLER" \
    "$KYNPUT_ROOTFS/bin/kynput.dll" "$KYNPUTSERVER"
rm -rf "$OVERLAY" "$OVERLAY.stamp" && mkdir -p "$OVERLAY"
for f in kyavserver.exe libtxproto-0.dll liblua.dll libspirv-cross-c-shared.dll; do
    cp "$ROOTFS/bin/$f" "$OVERLAY/"
done
cp "$KYCONTROLLER" "$KYNPUT_ROOTFS/bin/kynput.dll" "$KYNPUTSERVER" "$OVERLAY/"
# Runtime MinGW (UCRT) de la chaîne kyavserver : aucun autre binaire de Kyber
# n'utilise ces DLL, on peut remplacer celles de la version officielle.
cp "$MINGW/lib/libstdc++-6.dll" "$MINGW/lib/libgcc_s_seh-1.dll" "$MINGW/bin/libwinpthread-1.dll" "$OVERLAY/"
x86_64-w64-mingw32-strip --strip-debug "$OVERLAY"/*.exe "$OVERLAY"/*.dll
# Moteur du client web (servi par kycontroller depuis kyber\webclient, relayé
# par Admin sur /link).
mkdir -p "$OVERLAY/webclient"
cp "$WEB_OUT/kyclient_wasm.js" "$WEB_OUT/kyclient_wasm_bg.wasm" "$OVERLAY/webclient/"
ls -lhR "$OVERLAY"
# Empreinte des correctifs compilés, vérifiée par scripts/release.sh (à côté de la surcouche : pas livrée).
cat "$HERE"/*.patch "$HERE/build.sh" | shasum -a 256 | cut -d' ' -f1 > "$OVERLAY.stamp"
