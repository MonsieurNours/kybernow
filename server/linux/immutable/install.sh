#!/bin/bash
# Installe KyberNow Station dans votre dossier, sans toucher au système :
# pour Bazzite, les Fedora Atomic (Silverblue, Kinoite…) et le mode bureau de
# SteamOS, et toute autre distribution sur laquelle vous préférez ne pas
# installer de paquet (docs/station-linux.md, « Systèmes immuables »).
#
#   ./install.sh              installe (ou remplace) et démarre la Station
#   ./install.sh --no-sudo    sans jamais demander le mot de passe (la
#                             commande de la règle udev est seulement affichée)
#
# Ensuite : ~/.local/lib/kybernow-station/current/bin/kybernow-station-home
# status | uninstall.
HERE="$(cd "$(dirname "$0")" && pwd)"
exec "$HERE/bin/kybernow-station-home" install "$HERE" "$@"
