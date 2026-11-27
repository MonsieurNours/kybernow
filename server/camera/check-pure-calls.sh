#!/bin/bash
# Refuse un objet qui appelle directement `__cxa_pure_virtual` : signe qu'un
# appel par une interface implémentée hors du programme (Windows, COM) a été
# « dévirtualisé » vers la méthode pure, et plantera à l'exécution.
# Usage : check-pure-calls.sh <objet.o>...
set -euo pipefail
status=0
for obj in "$@"; do
    # Sortie lue en entier (grep -q fermerait le tube : objdump mourrait de
    # SIGPIPE et, avec pipefail, l'erreur passerait inaperçue).
    listing=$(x86_64-w64-mingw32-objdump -dr "$obj")
    if [[ "$listing" == *__cxa_pure_virtual* ]]; then
        echo "ERREUR : $obj appelle directement __cxa_pure_virtual (interface dévirtualisée : déclarez-la hors de l'espace de noms anonyme)" >&2
        status=1
    fi
done
exit $status
