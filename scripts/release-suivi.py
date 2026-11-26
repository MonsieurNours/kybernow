#!/usr/bin/env python3
"""Suivi d'une publication (scripts/release.sh) : les grandes étapes qui se
cochent au fur et à mesure, et un pourcentage d'avancement.

    scripts/release-suivi.py 0.11.2            redessine toutes les 2 s, jusqu'à la fin
    scripts/release-suivi.py 0.11.2 --une-fois un seul état (essais)

Lit le journal de la publication (~/Library/Logs/kybernow-release-<version>.log,
écrit par release.sh ; ou --journal CHEMIN). Rien n'est lancé ni arrêté :
release.sh reste seul maître de la publication.

Avancement : vérifications, versions et arbres 5 %, constructions 80 % (part
égale par étape, une étape reprise ou refaite compte pleine), fichiers,
vérifications, flux, manifeste et publication 15 %.
"""
import os
import re
import sys
import time
from pathlib import Path

AVANT = ["Vérifications préalables", "Versions", "Arbres de construction"]
CONSTRUCTIONS = [
    "Surcouche Kyber (Windows)", "Station Windows", "App Windows x64+ARM64", "App Android", "App webOS",
    "App Mac", "Notarisation app Mac", "App Apple TV + SDK", "App iPhone/iPad + SDK", "Station Mac",
    "Flatpak aarch64 (VM)", "Flatpak x86_64 (VM)", ".deb Raspberry Pi (VM)", "KyberNow Link OS (VM)",
]
APRES = [
    "Fichiers de la version", "Vérification des fichiers", "Flux de mises à jour de l'app", "Flux cumulatif du site",
    "Empreintes et manifeste signé", "Release GitHub en brouillon", "Tag", "Publication",
]

VERT, ROUGE, JAUNE, GRIS, GRAS, FIN = "\033[32m", "\033[31m", "\033[33m", "\033[90m", "\033[1m", "\033[0m"
LIGNE = re.compile(r"^\[(\d\d:\d\d:\d\d)\]\s+(\S)\s+(.+?)\s{2,}(en cours|refaite|reprise|ÉCHEC|annulée)\s*(.*)$")


def etat(journal):
    """Ce que dit le journal : phases vues, étapes de construction, fin."""
    phases, etapes, debut_etape, fin, erreurs = [], {}, {}, None, []
    try:
        texte = journal.read_text(errors="replace")
    except FileNotFoundError:
        return phases, etapes, debut_etape, fin, erreurs
    for ligne in texte.splitlines():
        if ligne.startswith("==> "):
            phases.append(ligne[4:].strip())
        m = LIGNE.match(ligne)
        if m:
            heure, _, nom, quoi, reste = m.groups()
            etapes[nom.strip()] = (quoi, reste.split(" — ")[0].strip())
            if quoi == "en cours":
                debut_etape[nom.strip()] = heure
        if ligne.startswith("Publiée : "):
            fin = ("ok", ligne[len("Publiée : "):].strip())
        elif "Interrompu." in ligne or ligne.startswith("Échec") or "ÉCHEC" in ligne and not m:
            erreurs.append(ligne.strip())
    if fin is None and any(q == "ÉCHEC" for q, _ in etapes.values()):
        fin = ("echec", next(n for n, (q, _) in etapes.items() if q == "ÉCHEC"))
    return phases, etapes, debut_etape, fin, erreurs


def phase_vue(phases, nom):
    return any(p.startswith(nom) for p in phases)


def dessiner(version, journal, en_cours):
    phases, etapes, debut_etape, fin, _ = etat(journal)
    lignes = []
    # État de chaque phase hors constructions : faite si une phase suivante a commencé.
    ordre = AVANT + ["Constructions"] + APRES
    vues = [phase_vue(phases, p) for p in ordre]
    derniere = max((i for i, v in enumerate(vues) if v), default=-1)

    def statut_phase(i):
        if fin and fin[0] == "ok":
            return "fait"
        if i < derniere:
            return "fait"
        if i == derniere:
            return "echec" if fin and fin[0] == "echec" and ordre[i] != "Constructions" and not etapes else "cours"
        return "attente"

    # Pourcentage.
    pct = 0.0
    for i, p in enumerate(AVANT):
        pct += 5 / len(AVANT) * (1 if statut_phase(i) == "fait" else 0)
    faites = sum(1 for n in CONSTRUCTIONS if etapes.get(n, ("",))[0] in ("refaite", "reprise"))
    pct += 80 * faites / len(CONSTRUCTIONS)
    base = len(AVANT) + 1
    for j, p in enumerate(APRES):
        pct += 15 / len(APRES) * (1 if statut_phase(base + j) == "fait" else 0)
    if fin and fin[0] == "ok":
        pct = 100.0

    largeur = 40
    plein = int(round(largeur * pct / 100))
    couleur = ROUGE if fin and fin[0] == "echec" else VERT
    lignes.append(f"{GRAS}KyberNow {version} — publication{FIN}")
    lignes.append(f"{couleur}{'█' * plein}{GRIS}{'░' * (largeur - plein)}{FIN}  {GRAS}{pct:3.0f} %{FIN}")
    lignes.append("")

    def puce(s):
        return {"fait": f"{VERT}✓{FIN}", "cours": f"{JAUNE}▶{FIN}", "echec": f"{ROUGE}✗{FIN}", "attente": f"{GRIS}·{FIN}"}[s]

    for i, p in enumerate(AVANT):
        lignes.append(f" {puce(statut_phase(i))} {p}")
    lignes.append(f" {puce(statut_phase(len(AVANT)))} Constructions ({faites}/{len(CONSTRUCTIONS)})")
    maintenant = time.strftime("%H:%M:%S")
    for n in CONSTRUCTIONS:
        quoi, duree = etapes.get(n, ("", ""))
        if quoi in ("refaite", "reprise"):
            detail = f"{GRIS}{'reprise' if quoi == 'reprise' else duree}{FIN}"
            lignes.append(f"     {VERT}✓{FIN} {n:<28} {detail}")
        elif quoi == "en cours":
            h = debut_etape.get(n)
            ecoule = ""
            if h:
                s = (_sec(maintenant) - _sec(h)) % 86400
                ecoule = f"{s // 60} min {s % 60:02d} s" if s >= 60 else f"{s} s"
            lignes.append(f"     {JAUNE}▶{FIN} {n:<28} {JAUNE}{ecoule}{FIN}")
        elif quoi == "ÉCHEC":
            lignes.append(f"     {ROUGE}✗ {n:<28} échec{FIN}")
        elif quoi == "annulée":
            lignes.append(f"     {GRIS}– {n:<28} annulée{FIN}")
        else:
            lignes.append(f"     {GRIS}· {n}{FIN}")
    for j, p in enumerate(APRES):
        lignes.append(f" {puce(statut_phase(base + j))} {p}")
    lignes.append("")
    if fin and fin[0] == "ok":
        lignes.append(f"{VERT}{GRAS}Publiée{FIN} : {fin[1]}")
    elif fin and fin[0] == "echec":
        lignes.append(f"{ROUGE}{GRAS}Échec{FIN} : {fin[1]} (détails dans {journal})")
    elif not en_cours:
        lignes.append(f"{GRIS}La publication ne tourne plus.{FIN}")
    else:
        lignes.append(f"{GRIS}Journal complet : {journal}{FIN}")
    return "\n".join(lignes), fin


def _sec(h):
    a, b, c = (int(x) for x in h.split(":"))
    return a * 3600 + b * 60 + c


def publication_en_cours(version):
    return os.system(f"pgrep -f 'scripts/release.sh {re.escape(version)}' >/dev/null 2>&1") == 0


def main():
    args = sys.argv[1:]
    if not args or args[0].startswith("-"):
        print(__doc__.strip())
        return 2
    version = args[0]
    journal = Path.home() / "Library/Logs" / f"kybernow-release-{version}.log"
    if "--journal" in args:
        journal = Path(args[args.index("--journal") + 1])
    une_fois = "--une-fois" in args
    while True:
        en_cours = publication_en_cours(version)
        ecran, fin = dessiner(version, journal, en_cours)
        sys.stdout.write("\033[H\033[2J" if not une_fois else "")
        sys.stdout.write(ecran + "\n")
        sys.stdout.flush()
        # Après un échec, les autres voies finissent leur étape : on suit
        # jusqu'à l'arrêt de release.sh.
        if une_fois or not en_cours or (fin and fin[0] == "ok"):
            return 0 if not fin or fin[0] == "ok" else 1
        time.sleep(2)


if __name__ == "__main__":
    sys.exit(main())
