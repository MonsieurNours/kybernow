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
#         duplication DXGI (seulement pour un écran que le repère de session
#         d'Admin nomme, jamais en (0, 0) ; agent attendu 1,5 s, dernière
#         image reprise dès l'ouverture et renvoyée chaque seconde, repli sur
#         la duplication sans image en 2,5 s) ; pointeur lu par
#         GetCursorInfo ; méthode, première image et latence au journal
#   0019  KyberNow Share (kycontroller et serveur d'entrées, deux fichiers) :
#         rôle d'un invité porté par son jeton (kn_role, kn_display, kn_rev)
#         et imposé (entrées selon le rôle, changées sans couper l'image,
#         jamais le presse-papiers ; seulement son écran ; ni débit ni image
#         clé) ; routes /kybernow (sessions, role, close) pour KyberNow Admin
#   0020  image à part d'un invité (KyberNow Share) : kyavservice et txproto
#         (0020-kyavservice-*, 0020-txproto-*) : scale et max_fps dans
#         KymuxStartVideo ; image réduite sur la carte (scale_d3d11 pour NVENC
#         et AMF, scale_qsv, scale logiciel pour x264), toujours 4:2:0 ;
#         cadence plafonnée dans la capture (duplication et WGC, dernière image
#         retenue jusqu'à son heure) ; plancher de débit à 1 Mbit/s pour une
#         image d'invité ; libavconv : réduction seulement, max_fps refusé.
#         kycontroller (0020-kycontroller-*) : seule une session d'hôte crée
#         ou redéfinit l'image de l'hôte ; image à part par qualité d'invité
#         (kn_quality, choose(), limites par encodeur) ; sorties d'invité de
#         kydup qui ne retiennent jamais la source, déplaçables ; image clé et
#         débit par image ; descente d'un cran en retard ; /kybernow/quality.
#         Moteur web, image clé au lieu de couper (0020-kyctlweb-*, kyclient et
#         kywebplayer de la copie du moteur web seulement) : le décodeur qui
#         attend une image clé (après une erreur, ou une image clé perdue) la
#         demande (POST /force_idr, une par seconde au plus) ; la session ne
#         s'arrête (decode_error) que sans image clé 3 s après une erreur
#   0021  touches système (serveur d'entrées et kycontroller, deux fichiers) :
#         droit system_keys (Ctrl+Alt+Del, Win+L) réservé à l'hôte ; refusé à
#         tout invité du partage et à l'ami à qui l'ordinateur est prêté
#   0022  HDR (cinq fichiers, docs/hdr.md). Serveur : txproto et libavgrab capturent
#         en 16 bits flottants (DuplicateOutput1, et l'anneau WGC d'Admin en
#         R16G16B16A16) et convertissent sur la carte (shaders HLSL compilés
#         par d3dcompiler_47.dll, communs aux deux moteurs ; texture capturée
#         lue sans copie, HDR10 en une passe par bloc 2 × 2 et table PQ, puis
#         une passe légère pour la luma) : HDR10 en P010
#         (PQ, BT.2020, plage limitée) ou SDR (courbe douce, BGRA) ; HEVC
#         Main 10 et AV1 10 bits sur NVENC, AMF et QSV, métadonnées HDR10
#         d'après IDXGIOutput6::GetDesc1 (insérées après l'encodage pour
#         NVENC, SDK 12.1) ; kyavservice : hdr et hdr_capture dans
#         KymuxStartVideo, HDR refusé avec H.264, x264 ou 4:4:4 (SDR) ;
#         kycontroller : hdr de chaque écran lu dans la requête de démarrage,
#         image à part en SDR pour un invité sans HDR face à un hôte en HDR
#         0022-kyclient-hdr : côté client (kyclient, son API
#         C, kyclient-rs) et champ hdr de la requête de démarrage
#         (kyctl-common, lu par kycontroller : posé ici avant
#         0022-kycontroller-hdr) ; les apps le posent aussi, après leurs
#         propres correctifs (apps/macos/scripts/build-kyclient.sh…)
#   0023  latence (txproto, et libavgrab/libavconv) : priorité GPU au
#         démarrage de la capture (classe HIGH du processus, réglable par 0024 ;
#         SetGPUThreadPriority(7) ; mode d'ordonnancement lu pour le journal) ;
#         chronométrage par étape : la capture joint à chaque image ses heures
#         (affichage par Windows, prise, soumission à la carte) et le temps de
#         la carte pour la copie ou la conversion (requêtes d'horodatage D3D11
#         lues sans attendre) ; l'encodeur écrit toutes les 5 s « Timing in
#         ms … » (moyennes et 95e centile), en SDR comme en HDR
#   0024  priorité GPU réglable (kyavservice, txproto, libavgrab) :
#         [kyavserver] gpu_priority = "normal" (rien de changé, comme
#         Kyber), "high" (par défaut, la classe de 0023) ou "realtime"
#         (REALTIME, HIGH si Windows la refuse) ; autre valeur : high et un
#         avertissement ; option de capture des deux moteurs ; la ligne
#         « GPU priority … » du journal dit le réglage puis la classe obtenue
#   0025  PyroWave (docs/pyrowave.md), format d'image de plus à côté de
#         H.264, HEVC et AV1 (cinq fichiers). 0025-kymedia-ffmpeg-pyrowave :
#         encodeur FFmpeg libpyrowave (AV_CODEC_ID_PYROWAVE) qui prend les
#         images D3D11 de la capture sans copie par le processeur (copie sur
#         la carte dans une texture partagée avec Vulkan, même carte par son
#         LUID, fence D3D11 partagée, pyrowave_encoder_encode_gpu_scaled :
#         BGRA en SDR, P010 en HDR10, image réduite d'un invité) ;
#         libpyrowave-shared-1.dll compilée par pyrowave.sh (commit épinglé).
#         0025-kyavservice : VideoCodec::PyroWave, encodeur choisi quel que
#         soit [kyavserver] encoder, dans les deux moteurs, identifiant kymux
#         0x70777631 (« pwv1 ») ; 0025-txproto : le même dans packet_sink.c ;
#         [kyavserver] pyrowave (vrai par défaut). 0025-kyclient : variante
#         PyroWave côté client (kyctl-common, kyclient et son API C,
#         kyclient-rs) et fallback_codec de la requête de démarrage ;
#         0025-kycontroller : PyroWave seulement si la Station le propose,
#         sinon le format de repli du client. Moteur web
#         (docs/pyrowave-client-web.md) : 0025-kyctlweb (copie du moteur web
#         seulement) : PyroWave et fallback_codec dans kyctl-common et
#         kyclient, et kywebplayer passe les paquets « pwv1 » au décodeur
#         WebGPU de la page (globalThis.__kybernowPyroWave) au lieu de
#         WebCodecs ; 0025-kyberweb : codec « pyrowave » et fallback_codec
#         dans la configuration JS de l'image
#   0026  débit changé en direct (docs/debit-automatique.md) : correctif
#         FFmpeg 0023-avcodec-debit-en-direct ajouté au wrap (comme 0025) :
#         NVENC sans remise à zéro ni image clé, mémoire VBV qui suit le
#         débit ; VideoToolbox qui applique un débit changé en cours de session
#   0060  reprise sans image clé (docs/reprise-apres-perte.md, quatre
#         fichiers) : correctif FFmpeg 0060-avcodec-reprise-sans-image-cle
#         posé après 0017 dans le wrap (NVENC : invalidation et références
#         longues ; VideoToolbox : références longues ; options ref_*) ;
#         txproto : commande recover, numéro de chaque image HEVC/H.264 et
#         borne de l'image qui répare dans un SEI (kn_recovery.h) ;
#         kyavservice : VideoRecover, [kyavserver] loss_repair ;
#         kycontroller : POST /recover
#
# Prérequis (Homebrew) : mingw-w64 meson ninja nasm cmake bison lua@5.4
# binaryen, wasm-bindgen-cli (cargo install), rustup target
# wasm32-unknown-unknown, le client web de Kyber 0.29 dans third_party/kyber-web,
# Kyber 0.29 extrait dans third_party/payload/kyber-0.29.0 (sa SDL2.dll),
# rustup target x86_64-pc-windows-gnu, et les sources Kyber dans
# third_party/kyber-desktop (avec Jinja dans
# kysdk/kymedia/subprojects/libplacebo-*/3rdparty, voir docs/build-notes.md).
# Versions vérifiées : scripts/kyber-toolchain.sh (une autre est signalée).
# PyroWave : cmake et git (sources téléchargées par pyrowave.sh).
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
# Une seule construction à la fois sur ces sources de Kyber (scripts/verrou.sh).
. "$ROOT/scripts/verrou.sh"
kyber_verrou_sortie "$THIRD_PARTY/kyber-desktop" "server/kyber-patches/build.sh" || exit 1

KYMEDIA="$THIRD_PARTY/kyber-desktop/kysdk/kymedia"
KYCTL="$THIRD_PARTY/kyber-desktop/kysdk/kyctl"
# shellcheck disable=SC2034  # lu par apply.sh
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

# shellcheck source=apply.sh
. "$HERE/apply.sh"
kyber_apply_server_patches
cd "$KYMEDIA"

# PyroWave (0025) : la bibliothèque, dans le rootfs avant FFmpeg qui s'y lie.
# shellcheck source=pyrowave.sh
. "$HERE/pyrowave.sh"
pyrowave_fetch
pyrowave_build
# FFmpeg est extrait une fois par meson, avec les correctifs du wrap de ce
# moment-là : quand le wrap ou ses fichiers changent (0025 y ajoute
# l'encodeur PyroWave, 0026 le débit en direct), il faut l'extraire et le
# configurer à nouveau.
FFMPEG_STAMP="$KYMEDIA/subprojects/FFmpeg-n8.1/.kybernow-wrap"
FFMPEG_WRAP=$(cat "$KYMEDIA/subprojects/ffmpeg.wrap" $(find "$KYMEDIA/subprojects/packagefiles/ffmpeg" -type f | sort) | shasum -a 256 | cut -d' ' -f1)
if [ -d "$KYMEDIA/subprojects/FFmpeg-n8.1" ] && [ "$(cat "$FFMPEG_STAMP" 2>/dev/null)" != "$FFMPEG_WRAP" ]; then
    echo "==> FFmpeg : wrap changé, nouvelle extraction"
    rm -rf "$KYMEDIA/subprojects/FFmpeg-n8.1" "$KYMEDIA/builddir-win32/subprojects/FFmpeg-n8.1"
fi

echo "==> Compilation (dépendances, txproto, kyavserver)"
./build-win32.sh -w -o "$ROOTFS"
echo "$FFMPEG_WRAP" > "$FFMPEG_STAMP"

echo "==> Essai PyroWave (pyrowave-capture-test.exe, pas livré)"
# Capture DXGI → images D3D11 → encodeur libpyrowave → décodage de contrôle :
# docs/pyrowave.md, « Essai sur le PC ». À côté de la DLL, hors surcouche.
PYROWAVE_TEST="$THIRD_PARTY/pyrowave/test-win64"
mkdir -p "$PYROWAVE_TEST"
# shellcheck disable=SC2046  # options de pkg-config, mot à mot
x86_64-w64-mingw32-gcc -O2 -o "$PYROWAVE_TEST/pyrowave-capture-test.exe" "$HERE/tools/pyrowave-capture-test.c" \
    -I"$ROOTFS/include" -I"$ROOTFS/include/pyrowave" \
    $(PKG_CONFIG_LIBDIR="$ROOTFS/lib/pkgconfig" pkg-config --static --libs libavcodec libavutil) \
    -L"$ROOTFS/lib" -lpyrowave-shared -ld3d11 -ldxgi -static-libgcc
x86_64-w64-mingw32-strip --strip-debug "$PYROWAVE_TEST/pyrowave-capture-test.exe"
cp "$ROOTFS/bin/libpyrowave-shared-1.dll" "$ROOTFS/bin/pyrowave-device-validation.exe" \
    "$MINGW/bin/libwinpthread-1.dll" "$PYROWAVE_TEST/"

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
# plus les correctifs *-kywebplayer-* et *-kyctlweb-* (ceux-ci posés ici
# seulement), dans l'ordre de leurs numéros. Les autres dépôts sont ceux de
# third_party, correctifs serveur appliqués (kynput, kymux…).
WEB_KYSDK="$THIRD_PARTY/kyber-web-kysdk"
rm -rf "$WEB_KYSDK/kyctl" && mkdir -p "$WEB_KYSDK/kyctl"
git -C "$KYCTL" archive HEAD | tar -x -C "$WEB_KYSDK/kyctl"
for p in "$HERE"/*.patch; do
    case "$(basename "$p")" in *-kywebplayer-* | *-kyctlweb-*) ;; *) continue ;; esac
    echo "    $(basename "$p") (moteur web)"
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
# PyroWave (0025) : la bibliothèque (runtime C++ en statique) que charge
# libtxproto-0.dll, et l'outil par lequel Admin vérifie le Vulkan de la carte.
for f in libpyrowave-shared-1.dll pyrowave-device-validation.exe; do
    [ -f "$ROOTFS/bin/$f" ] || { echo "$ROOTFS/bin/$f introuvable (pyrowave.sh)" >&2; exit 1; }
    cp "$ROOTFS/bin/$f" "$OVERLAY/"
done
cp "$KYCONTROLLER" "$KYNPUT_ROOTFS/bin/kynput.dll" "$KYNPUTSERVER" "$OVERLAY/"
# L'outil par lequel Admin essaie l'encodeur (docs/profil-auto.md), contre le
# FFmpeg de kyavserver ; lancé depuis le dossier de Kyber.
"$ROOT/server/encoder-check/build.sh" windows "$ROOTFS" "$OVERLAY/kybernow-encoder-check.exe"
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
# Avec les correctifs des apps qu'apply.sh repose après ceux du serveur
# (0027 s'appuie sur 0005) : scripts/release.sh calcule la même empreinte.
cat "$HERE"/*.patch "$HERE/build.sh" "$HERE/pyrowave.sh" "$ROOT"/apps/macos/kyber-patches/*.patch "$ROOT"/apps/webos/kyber-patches/*.patch | shasum -a 256 | cut -d' ' -f1 > "$OVERLAY.stamp"
