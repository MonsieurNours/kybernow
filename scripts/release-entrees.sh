# shellcheck shell=bash
# Entrées de chaque étape de scripts/release.sh, en un seul endroit : ce qui
# décide si une étape est refaite ou reprise de son cache. Lu (source) par
# scripts/release.sh et par scripts/tests/release-outils.sh, qui vérifie que
# chaque fichier du dépôt lu par une recette (construire_<id> et les scripts
# qu'elle lance) est bien dans les chemins de son étape.
#
#   release_chemins <id>   chemins du dépôt (dossiers ou fichiers), un par ligne
#   release_kyber <id>     sources de Kyber lues : « arbre:dossier de third_party »
#   release_tp <id>        dépendances lues dans le third_party/ partagé
#   release_empreintes <id> étapes dont l'empreinte entre dans celle-ci
#
# RELEASE_REV : le commit construit (défaut HEAD), pour la liste des dossiers
# de server/.

# Les dossiers de server/ sauf ceux qu'on nomme.
release_server_sauf() {
    git ls-tree --name-only "${RELEASE_REV:-HEAD}" server/ | grep -vxF -f <(printf 'server/%s\n' "$@")
}

# Le cœur et ce que toutes les apps lisent avec lui.
RELEASE_COEUR=(apps/core apps/Cargo.toml apps/Cargo.lock apps/macos/kyber-patches server/kyber-patches server/relay
    server/admin/src/update/kybernow-release.pub scripts/kyber-client-patches.sh scripts/verrou.sh scripts/kyber-toolchain.sh brand)
# Les apps Linux (Flatpak, .deb du Pi) : leurs licences sont copiées dans les paquets.
RELEASE_LINUX=(apps/linux apps/webos/kyber-patches LICENSE LICENSE.md)
# Le décodeur et l'encodeur PyroWave (port Metal) : commit et sources vendues.
RELEASE_PYROWAVE=(apps/macos/PyroWave)

release_chemins() {
    case "$1" in
        surcouche) printf '%s\n' server/kyber-patches server/encoder-check server/camera server/micro \
            apps/macos/kyber-patches apps/webos/kyber-patches scripts/kyber-toolchain.sh scripts/verrou.sh scripts/arbres.sh \
            scripts/release-verifie.py "${RELEASE_PYROWAVE[@]}" ;;
        station) release_server_sauf macos linux; printf '%s\n' scripts/kyber-toolchain.sh scripts/verrou.sh brand LICENSE LICENSE.md ;;
        windows) printf '%s\n' "${RELEASE_COEUR[@]}" apps/windows "${RELEASE_PYROWAVE[@]}" scripts/release-verifie.py ;;
        android) printf '%s\n' "${RELEASE_COEUR[@]}" apps/android ;;
        webos) printf '%s\n' "${RELEASE_COEUR[@]}" apps/webos ;;
        app_mac) printf '%s\n' "${RELEASE_COEUR[@]}" apps/macos apps/apple ;;
        notarisation) ;;
        # L'app Apple TV compile des sources de l'app Mac, et l'app iPhone/iPad
        # celles de l'Apple TV (apps/ios/project.yml : KyberKitTV).
        tvos) printf '%s\n' "${RELEASE_COEUR[@]}" apps/macos apps/apple apps/tvos ;;
        ios) printf '%s\n' "${RELEASE_COEUR[@]}" apps/macos apps/apple apps/tvos apps/ios ;;
        station_mac) release_server_sauf linux; printf '%s\n' apps/macos/kyber-patches apps/webos/kyber-patches "${RELEASE_PYROWAVE[@]}" \
            scripts/kyber-toolchain.sh scripts/verrou.sh brand LICENSE LICENSE.md ;;
        flatpak_aarch64 | flatpak_x86_64 | deb_pi) printf '%s\n' "${RELEASE_COEUR[@]}" "${RELEASE_LINUX[@]}" ;;
        link_os) printf '%s\n' apps/linux/os apps/linux/scripts/vm.sh ;;
        # build-kyber.sh pose aussi les correctifs des apps (stylet 0005, dont
        # dépend 0027) ; package.sh copie LICENSE ; la VM est jointe par vm.sh.
        station_linux) release_server_sauf macos; printf '%s\n' apps/macos/kyber-patches apps/webos/kyber-patches \
            apps/linux/scripts/vm.sh scripts/kyber-toolchain.sh brand LICENSE LICENSE.md ;;
        *) echo "étape inconnue : $1" >&2; return 1 ;;
    esac
}

release_kyber() {
    case "$1" in
        surcouche) echo "station-windows:kyber-desktop station-windows:kyber-web" ;;
        windows) echo "windows-app:kyber-desktop" ;;
        android) echo "android:kyber-desktop" ;;
        webos) echo "webos:kyber-desktop" ;;
        app_mac | tvos | ios) echo "apple:kyber-desktop" ;;
        station_mac) echo "station-mac:kyber-desktop-macos" ;;
        flatpak_aarch64 | flatpak_x86_64 | deb_pi) echo "linux:kyber-desktop" ;;
        # La Station Linux se compile sur les sources de Kyber de la VM et du
        # chroot x86_64 : release.sh lit leurs commits par ssh (VM_KYBER_STATION).
        *) echo "" ;;
    esac
}

release_tp() {
    case "$1" in
        surcouche) echo "payload/kyber-0.29.0" ;;
        station) echo "payload/kyber-0.29.0 payload/vbcable kyber-installer" ;;
        windows) echo "rootfs-winarm64 toolchains/llvm-mingw-$KYBER_LLVM_MINGW-ucrt-macos-universal" ;;
        android) echo "opus-1.6.1" ;;
        webos) echo "webos-toolchain-mac ffmpeg-7.1.1 opus-1.6.1" ;;
        station_mac) echo "payload/kyber-0.29.0" ;;
        *) echo "" ;;
    esac
}

release_empreintes() {
    case "$1" in
        station | windows | station_mac) echo surcouche ;;
        notarisation) echo app_mac ;;
        link_os) echo deb_pi ;;
        *) echo "" ;;
    esac
}

# Les étapes et leur libellé (scripts/release-etapes.txt, lu aussi par
# scripts/release-suivi.py).
RELEASE_ETAPES_TXT="$(dirname "${BASH_SOURCE[0]}")/release-etapes.txt"
release_etapes() { awk -F'\t' '!/^#/ && NF >= 2 {print $1}' "$RELEASE_ETAPES_TXT"; }
release_libelle() { awk -F'\t' -v id="$1" '!/^#/ && $1 == id {print $2}' "$RELEASE_ETAPES_TXT"; }

# Les fichiers publiés (scripts/release-fichiers.txt, lu aussi par
# scripts/sign-manifest.sh et scripts/release-verifie.py), par groupe :
# construit, copie (ancien nom : copie d'un autre), flux, station-linux, signature.
RELEASE_FICHIERS_TXT="$(dirname "${BASH_SOURCE[0]}")/release-fichiers.txt"
release_fichiers() {
    local g
    for g in "$@"; do awk -v g="$g" '!/^#/ && NF >= 2 && $2 == g {print $1}' "$RELEASE_FICHIERS_TXT"; done
}
# release_copie_de <nom> : le fichier dont <nom> est la copie.
release_copie_de() { awk -v n="$1" '!/^#/ && $1 == n && $2 == "copie" {print $3}' "$RELEASE_FICHIERS_TXT"; }

# release_chemins_couverts <id> : ses chemins et ceux des étapes dont
# l'empreinte entre dans la sienne (ce que l'essai des entrées accepte).
release_chemins_couverts() {
    local d
    release_chemins "$1"
    for d in $(release_empreintes "$1"); do release_chemins_couverts "$d"; done
}
