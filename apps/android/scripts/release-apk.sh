#!/bin/bash
# APK de publication de KyberNow Link pour Android, signé par la clé de
# publication, pour scripts/release.sh (ou à la main) :
#
#   apps/android/scripts/release-apk.sh <fichier.apk>     construit et signe
#   apps/android/scripts/release-apk.sh --check           vérifie clé et outils
#   apps/android/scripts/release-apk.sh --fingerprint     empreinte du certificat
#
# Étapes : bibliothèque native des trois architectures (build-native.sh),
# ./gradlew assembleRelease (R8) signé par la clé, vérification par apksigner
# (signature v2/v3, certificat attendu), copie dans <fichier.apk>.
#
# Clé : ~/Library/Application Support/KyberNow/android-release.jks (alias
# « kybernow »), protégée par un mot de passe aléatoire gardé dans le
# Trousseau (élément « kybernow-android-keystore »). Absente, elle est créée
# ici, sauf si une empreinte de certificat est déjà dans le dépôt
# (release-cert.sha256) : une nouvelle clé empêcherait toutes les apps
# installées de se mettre à jour (Android refuse un APK signé autrement), il
# faudrait les désinstaller. Sauvegarde hors ligne : docs/releases.md.
#
# Essai sans toucher à la vraie clé : KYBERNOW_ANDROID_KEYSTORE=<chemin.jks>
# et KYBERNOW_ANDROID_KEYSTORE_ITEM=<élément du Trousseau>.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
STORE="${KYBERNOW_ANDROID_KEYSTORE:-$HOME/Library/Application Support/KyberNow/android-release.jks}"
ITEM="${KYBERNOW_ANDROID_KEYSTORE_ITEM:-kybernow-android-keystore}"
ALIAS=kybernow
# Empreinte SHA-256 du certificat de publication, commitée : l'APK construit
# doit être signé par lui.
CERT_FILE="$HERE/scripts/release-cert.sha256"

MODE=build OUTPUT=""
case "${1:-}" in
    --check) MODE=check ;;
    --fingerprint) MODE=fingerprint ;;
    ""|-*) echo "usage : release-apk.sh <fichier.apk> | --check | --fingerprint" >&2; exit 1 ;;
    *) OUTPUT="$1" ;;
esac

# JDK : celui de l'environnement, sinon celui d'Android Studio.
if [ -z "${JAVA_HOME:-}" ]; then
    for j in "/Applications/Android Studio.app/Contents/jbr/Contents/Home" "$HOME/Applications/Android Studio.app/Contents/jbr/Contents/Home"; do
        [ -x "$j/bin/java" ] && { export JAVA_HOME="$j"; break; }
    done
fi
KEYTOOL="${JAVA_HOME:+$JAVA_HOME/bin/}keytool"
command -v "$KEYTOOL" > /dev/null || { echo "keytool introuvable : installez Android Studio, ou donnez JAVA_HOME" >&2; exit 1; }

# SDK : variable, local.properties, ou celui d'Android Studio.
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$SDK" ] && [ -f "$HERE/local.properties" ]; then
    SDK="$(sed -n 's/^sdk\.dir=//p' "$HERE/local.properties" | sed 's/\\:/:/g')"
fi
[ -n "$SDK" ] || SDK="$HOME/Library/Android/sdk"
[ -d "$SDK/build-tools" ] || { echo "SDK d'Android introuvable ($SDK) : ANDROID_HOME, ou Android Studio › SDK Manager" >&2; exit 1; }
export ANDROID_HOME="$SDK"
APKSIGNER="$(ls -d "$SDK"/build-tools/*/apksigner 2> /dev/null | sort -V | tail -1)"
[ -x "$APKSIGNER" ] || { echo "apksigner introuvable dans $SDK/build-tools" >&2; exit 1; }

password() { security find-generic-password -s "$ITEM" -w 2> /dev/null; }

fingerprint() {
    # Sortie de keytool en anglais (en français : « SHA 256 »), empreinte en minuscules sans « : ».
    KN_STOREPASS="$1" "$KEYTOOL" -J-Duser.language=en -J-Duser.country=US -list -v -keystore "$STORE" -alias "$ALIAS" -storepass:env KN_STOREPASS 2> /dev/null \
        | sed -nE 's/^[[:space:]]*SHA[- ]?256:[[:space:]]*//p' | tr -d ': ' | tr 'A-F' 'a-f' | head -1
}

if [ ! -f "$STORE" ]; then
    if [ "$MODE" = check ]; then echo "clé de publication Android absente : $STORE (la lancer une fois sans --check la crée)" >&2; exit 1; fi
    if [ -z "${KYBERNOW_ANDROID_KEYSTORE:-}" ] && [ -s "$CERT_FILE" ]; then
        echo "Clé de publication absente ($STORE), mais son certificat est dans le dépôt ($CERT_FILE)." >&2
        echo "Remettez la sauvegarde (docs/releases.md, « Clé de signature de l'app Android ») ;" >&2
        echo "une nouvelle clé empêcherait les apps installées de se mettre à jour." >&2
        exit 1
    fi
    echo "==> Clé de publication Android absente : création ($STORE)"
    mkdir -p "$(dirname "$STORE")"
    if ! password > /dev/null; then
        # Ajouté par « security -i » : le mot de passe ne passe pas dans la liste des processus.
        printf 'add-generic-password -a kybernow -s %s -l %s -w %s\n' "$ITEM" "$ITEM" "$(openssl rand -hex 24)" | security -i > /dev/null
    fi
    PW="$(password)" || { echo "mot de passe introuvable dans le Trousseau ($ITEM)" >&2; exit 1; }
    KN_STOREPASS="$PW" "$KEYTOOL" -genkeypair -keystore "$STORE" -storetype PKCS12 -alias "$ALIAS" -keyalg RSA -keysize 4096 -validity 36500 \
        -dname "CN=KyberNow Link, O=Les lignes claires, C=FR" -storepass:env KN_STOREPASS -keypass:env KN_STOREPASS > /dev/null
    chmod 600 "$STORE"
    if [ -z "${KYBERNOW_ANDROID_KEYSTORE:-}" ]; then
        NEW="$(fingerprint "$PW")"
        [ -n "$NEW" ] || { echo "clé créée mais empreinte illisible : $STORE" >&2; exit 1; }
        printf '%s\n' "$NEW" > "$CERT_FILE"
        echo "    certificat : $(cat "$CERT_FILE") (écrit dans ${CERT_FILE#"$ROOT"/}, à commiter)"
        echo "    à sauvegarder hors ligne avec son mot de passe (docs/releases.md)"
    fi
fi
PW="$(password)" || { echo "mot de passe de la clé Android introuvable dans le Trousseau (élément $ITEM) : Mac verrouillé, ou voir docs/releases.md" >&2; exit 1; }
CERT="$(fingerprint "$PW")"
[ -n "$CERT" ] || { echo "clé illisible avec le mot de passe du Trousseau : $STORE" >&2; exit 1; }
if [ -z "${KYBERNOW_ANDROID_KEYSTORE:-}" ]; then
    [ -s "$CERT_FILE" ] || { echo "$CERT_FILE absent : la clé existe mais son certificat n'est pas dans le dépôt ($CERT)" >&2; exit 1; }
    [ "$CERT" = "$(cat "$CERT_FILE")" ] || { echo "la clé $STORE n'est pas celle du dépôt (${CERT_FILE#"$ROOT"/})" >&2; exit 1; }
fi

case "$MODE" in
    fingerprint) echo "$CERT"; exit 0 ;;
    check) echo "clé de publication Android : $CERT"; exit 0 ;;
esac

echo "==> Bibliothèque native (arm64-v8a, armeabi-v7a, x86_64)"
"$HERE/scripts/build-native.sh"

echo "==> APK de publication (R8, signé)"
APK="$HERE/app/build/outputs/apk/release/app-release.apk"
rm -f "$APK"
(cd "$HERE" && KYBERNOW_ANDROID_KEYSTORE="$STORE" KYBERNOW_ANDROID_KEYSTORE_PASSWORD="$PW" KYBERNOW_ANDROID_KEY_ALIAS="$ALIAS" \
    ./gradlew --quiet assembleRelease)
[ -f "$APK" ] || { echo "Gradle n'a pas produit $APK (APK non signé ?)" >&2; exit 1; }

echo "==> Vérification de la signature"
SIGNED="$("$APKSIGNER" verify --print-certs "$APK" | sed -n 's/^Signer #1 certificate SHA-256 digest: //p' | head -1)"
[ "$SIGNED" = "$CERT" ] || { echo "APK signé par $SIGNED au lieu de $CERT" >&2; exit 1; }
for abi in arm64-v8a armeabi-v7a x86_64; do
    unzip -l "$APK" "lib/$abi/libkybernow.so" > /dev/null || { echo "libkybernow.so absente pour $abi" >&2; exit 1; }
done
mkdir -p "$(dirname "$OUTPUT")"
cp "$APK" "$OUTPUT"
echo "    $(basename "$OUTPUT") : $(du -h "$OUTPUT" | cut -f1), certificat $CERT"
