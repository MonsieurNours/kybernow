#!/usr/bin/env python3
"""Les fichiers du dépôt lus par chaque recette de scripts/release.sh sont-ils
dans l'empreinte de son étape ? Lancé par scripts/tests/release-outils.sh.

    entrees-couvertes.py RACINE RELEASE.SH CHEMINS.TSV [EXCEPTIONS]

CHEMINS.TSV : « étape<TAB>chemin » par ligne (release_chemins de
scripts/release-entrees.sh). Pour chaque étape, le corps de construire_<id>
est relu, puis, de proche en proche, les scripts du dépôt qu'il cite (trois
niveaux) : chaque chemin du dépôt cité (apps/…, server/…, scripts/…, brand/…,
LICENSE), s'il est suivi par git, doit être sous l'un des chemins de l'étape.
Les commentaires sont ignorés. EXCEPTIONS : « étape<TAB>chemin<TAB>raison »
(étape « * » : toutes), pour un chemin cité qui n'est pas lu (message, motif
de comparaison) ; il n'est pas relu non plus.

Une ligne « NON  étape : chemin (cité par …) » par manque ; code de sortie :
le nombre de manques.
"""
import os
import re
import subprocess
import sys

CHEMIN = re.compile(r"(?<![\w.$/-])((?:apps|server|scripts|tools|brand)/[A-Za-z0-9_.+/*-]*[A-Za-z0-9_+*]|LICENSE(?:\.md)?)(?![\w.-])")
ROOT_CHEMIN = re.compile(r"\$\{?ROOT\}?\"?/([A-Za-z0-9_.+/*-]*[A-Za-z0-9_+*])")


# L'outillage de la publication elle-même : cité par les recettes pour
# vérifier ou pour piloter, il ne fait pas les binaires ; pas relu.
PILOTES = {"scripts/release.sh", "scripts/release-verifie.py", "scripts/release-entrees.sh"}


def lignes_utiles(texte, py=False):
    for ligne in texte.splitlines():
        s = ligne.strip()
        if s.startswith("#"):
            continue
        # Commentaire en fin de ligne (« # » précédé d'une espace, hors chaîne : approximatif).
        ligne = re.sub(r"\s#\s.*$", "", ligne)
        yield ligne


def chemins_cites(texte):
    vus = set()
    for ligne in lignes_utiles(texte):
        for m in CHEMIN.finditer(ligne):
            vus.add(m.group(1))
        for m in ROOT_CHEMIN.finditer(ligne):
            vus.add(m.group(1))
    out = set()
    for c in vus:
        c = c.rstrip("/")
        # Un motif (*.patch) : son dossier.
        if "*" in c:
            c = c.split("*")[0].rstrip("/")
            c = c if not c.endswith(("-", "_")) else os.path.dirname(c)
        if c:
            out.add(c)
    return out


XCODE_CHEMIN = re.compile(r"(?:path:\s*|\$\(SRCROOT\)/)(\.\./[A-Za-z0-9_.+/-]*[A-Za-z0-9_+])")


def chemins_xcodegen(dossier, texte):
    """Chemins hors du dossier cités par un project.yml de xcodegen (../macos/…)."""
    out = set()
    for ligne in lignes_utiles(texte):
        for m in XCODE_CHEMIN.finditer(ligne):
            out.add(os.path.normpath(os.path.join(dossier, m.group(1))))
    return out


def recette(release, ident):
    """Corps de construire_<id> dans release.sh."""
    lignes = release.splitlines()
    for i, l in enumerate(lignes):
        m = re.match(rf"^construire_{re.escape(ident)}\(\)\s*\{{(.*)$", l)
        if not m:
            continue
        if m.group(1).rstrip().endswith("}"):
            return m.group(1)
        corps = []
        for l2 in lignes[i + 1:]:
            if l2 == "}":
                return "\n".join(corps)
            corps.append(l2)
    return None


def main():
    racine, release_sh, tsv = sys.argv[1:4]
    exceptions = {}
    if len(sys.argv) > 4:
        for l in open(sys.argv[4], encoding="utf-8"):
            c = l.rstrip("\n").split("\t")
            if len(c) >= 2 and not l.startswith("#"):
                exceptions.setdefault(c[0], set()).add(c[1])
    suivis = set(subprocess.run(["git", "-C", racine, "ls-files"], capture_output=True, text=True, check=True).stdout.splitlines())
    dossiers = set()
    for f in suivis:
        d = os.path.dirname(f)
        while d:
            dossiers.add(d)
            d = os.path.dirname(d)

    def suivi(c):
        return c in suivis or c in dossiers

    etapes = {}
    for l in open(tsv, encoding="utf-8"):
        c = l.rstrip("\n").split("\t")
        if len(c) == 2:
            etapes.setdefault(c[0], []).append(c[1].rstrip("/"))
        elif len(c) == 1 and c[0]:
            etapes.setdefault(c[0], [])
    release = open(release_sh, encoding="utf-8").read()
    manques = 0
    for ident, couverts in etapes.items():
        corps = recette(release, ident)
        if corps is None:
            print(f"  NON  {ident} : construire_{ident} introuvable dans {os.path.basename(release_sh)}")
            manques += 1
            continue

        def excepte(c):
            return c in exceptions.get(ident, ()) or c in exceptions.get("*", ())

        def couvert(c):
            return any(c == k or c.startswith(k + "/") for k in couverts)

        a_lire = [(f"construire_{ident}", chemins_cites(corps), 0)]
        lus = set()
        signales = set()
        while a_lire:
            source, cites, niveau = a_lire.pop()
            for c in sorted(cites):
                # Un dossier de premier niveau seul (cd "$ROOT/apps") ne dit pas
                # ce qui est lu : les fichiers lus sont cités à part.
                if not suivi(c) or excepte(c) or ("/" not in c and c not in ("LICENSE", "LICENSE.md")):
                    continue
                if not couvert(c) and c not in signales:
                    signales.add(c)
                    print(f"  NON  {ident} : {c} (cité par {source}) hors de l'empreinte")
                    manques += 1
                # Les scripts cités sont relus à leur tour (fichiers seulement).
                if niveau < 3 and c in suivis and c.endswith((".sh", ".py")) and c not in lus and c not in PILOTES and not c.startswith("scripts/tests/"):
                    lus.add(c)
                    try:
                        a_lire.append((c, chemins_cites(open(os.path.join(racine, c), encoding="utf-8", errors="replace").read()), niveau + 1))
                    except OSError:
                        pass
                # Un projet xcodegen (cd apps/ios && xcodegen) : les sources
                # qu'il prend dans d'autres dossiers (apps/tvos, apps/macos).
                pyml = f"{c}/project.yml"
                if pyml in suivis and pyml not in lus:
                    lus.add(pyml)
                    a_lire.append((pyml, chemins_xcodegen(c, open(os.path.join(racine, pyml), encoding="utf-8").read()), niveau + 1))
    return min(manques, 100)


if __name__ == "__main__":
    sys.exit(main())
