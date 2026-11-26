#!/usr/bin/env python3
"""Banc de mesure de KyberNow (docs/banc-de-mesure.md) : l'écran à l'écran
et le rapport qui compare deux versions.

  banc.py ecran [--duree 20] [--etiquette main] [--plancher] [--port 8765]
      Sert la sonde (sonde.html) : l'ouvrir en plein écran sur la Station,
      puis la session KyberNow Link au premier plan sur ce Mac. Le lecteur
      capture l'écran de ce Mac et lit le compteur de la sonde ; le banc en
      tire la latence de l'écran de la Station à celui du Mac.
      --plancher : la sonde s'ouvre sur ce Mac même, sans KyberNow entre les
      deux : le retard propre de l'instrument.

  banc.py analyse <dossier de mesure>
      Recalcule une mesure à partir de ses données brutes.

  banc.py debit [VERSION…] [--besoin 60] [--rtt 10] [-o rapport.html]
      Rejoue le débit automatique (apps/core/src/adaptive.rs) de chaque
      version git (« . » : l'arbre de travail) sur les profils de réseau, sans
      réseau, puis écrit le rapport qui les compare. Par défaut : main et « . ».

  banc.py rapport A.json B.json [C.json…] [-o rapport.html]
      Compare des mesures (écran à écran, rejeu du débit automatique) dans
      une page HTML autonome ; la première est la référence.

Les mesures vont dans tools/banc/mesures/<date>-<étiquette>/ (hors du dépôt,
sauf les références copiées à la main dans tools/banc/references/).
"""

import argparse
import html
import json
import math
import os
import platform
import socket
import statistics
import subprocess
import sys
import threading
import time
from datetime import datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ICI = Path(__file__).resolve().parent
PERIODE = 1 << 11
# Une valeur lue est gardée si la suivante ou la précédente la suit de près
# (0 à 16 images) : une lecture fausse ne tombe presque jamais si près.
PROCHE = 16


# --- Construction du lecteur -------------------------------------------------

def lecteur():
    """Le lecteur compilé (target/lecteur), reconstruit si la source a changé."""
    src = ICI / "lecteur.swift"
    exe = ICI / "target" / "lecteur"
    if not exe.exists() or exe.stat().st_mtime < src.stat().st_mtime:
        exe.parent.mkdir(exist_ok=True)
        print("Construction du lecteur (swiftc)…", file=sys.stderr)
        subprocess.run(["swiftc", "-O", str(src), "-o", str(exe)], check=True)
    return exe


# --- Serveur de la sonde -----------------------------------------------------

class Sonde:
    def __init__(self):
        self.images = []          # (n, t sur l'horloge de la page)
        self.horloges = []        # {rtt, decalage, a}
        self.ecran = None
        self.verrou = threading.Lock()
        self.premiere = threading.Event()


def serveur(sonde, port):
    page = (ICI / "sonde.html").read_bytes()

    class Gestion(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def repondre(self, corps, type_="application/json"):
            self.send_response(200)
            self.send_header("Content-Type", type_)
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(corps)))
            self.end_headers()
            self.wfile.write(corps)

        def do_GET(self):
            if self.path.startswith("/horloge"):
                self.repondre(json.dumps({"t": time.time() * 1000}).encode())
            elif self.path.startswith("/sonde") or self.path == "/":
                self.repondre(page, "text/html; charset=utf-8")
            else:
                self.send_error(404)

        def do_POST(self):
            corps = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
            with sonde.verrou:
                if self.path == "/sonde/images":
                    sonde.images.extend(corps["images"])
                    sonde.ecran = corps.get("ecran")
                    sonde.premiere.set()
                elif self.path == "/sonde/horloge":
                    sonde.horloges.append(corps)
            self.repondre(b"{}")

    s = ThreadingHTTPServer(("0.0.0.0", port), Gestion)
    threading.Thread(target=s.serve_forever, daemon=True).start()
    return s


def adresse_locale():
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.connect(("192.0.2.1", 9))
            return s.getsockname()[0]
    except OSError:
        return "127.0.0.1"


# --- Analyse -------------------------------------------------------------------

def centile(v, p):
    if not v:
        return None
    v = sorted(v)
    k = (len(v) - 1) * p / 100
    a, b = int(k), min(int(k) + 1, len(v) - 1)
    return v[a] + (v[b] - v[a]) * (k - a)


def analyser(brut):
    """brut : {images, horloges, lectures} → résultats."""
    horloges = [h for h in brut["horloges"] if h.get("rtt") is not None]
    if not horloges:
        raise SystemExit("La sonde n'a pas donné son écart d'horloge.")
    # Écart d'horloge de la page : celui de l'aller-retour le plus court
    # parmi les plus proches dans le temps (les horloges dérivent peu).
    horloges.sort(key=lambda h: h["a"])

    def decalage(t):
        proches = sorted(horloges, key=lambda h: abs(h["a"] - t))[:3]
        return min(proches, key=lambda h: h["rtt"])["decalage"]

    sonde = {}
    for n, t in brut["images"]:
        sonde.setdefault(n % PERIODE, []).append(t + decalage(t))
    for v in sonde.values():
        v.sort()

    lues = [l for l in brut["lectures"] if l.get("n") is not None]
    lues.sort(key=lambda l: l["t"])
    gardees, rejetees = [], 0
    for i, l in enumerate(lues):
        ok = False
        for j in (i - 1, i + 1):
            if 0 <= j < len(lues):
                d = (lues[max(i, j)]["n"] - lues[min(i, j)]["n"]) % PERIODE
                ok |= d <= PROCHE
        if ok:
            gardees.append(l)
        else:
            rejetees += 1

    latences, dechirees, nouvelles, sans_sonde = [], 0, [], 0
    avant = None
    for l in gardees:
        if l["n"] == avant:
            continue  # la même image de la Station, encore à l'écran
        avant = l["n"]
        nouvelles.append(l)
        if l.get("dechire"):
            dechirees += 1
        candidats = sonde.get(l["n"], [])
        # L'image de la sonde la plus récente montrée avant la lecture.
        t0 = max((t for t in candidats if t <= l["t"]), default=None)
        if t0 is None or l["t"] - t0 > 2000:
            sans_sonde += 1
            continue
        latences.append(round(l["t"] - t0, 2))

    duree = (nouvelles[-1]["t"] - nouvelles[0]["t"]) / 1000 if len(nouvelles) > 1 else 0
    rtts = [h["rtt"] for h in horloges]
    stats = {
        "n": len(latences),
        "p50": centile(latences, 50),
        "p95": centile(latences, 95),
        "p99": centile(latences, 99),
        "moyenne": statistics.fmean(latences) if latences else None,
        "min": min(latences, default=None),
        "max": max(latences, default=None),
        "fps": round(len(nouvelles) / duree, 1) if duree else None,
        "dechirees_pct": round(100 * dechirees / len(nouvelles), 2) if nouvelles else None,
        "rejetees": rejetees,
        "sans_bande": len(brut["lectures"]) - len(lues),
        "sans_sonde": sans_sonde,
        "precision_horloge_ms": round(min(rtts) / 2, 2),
    }
    return {k: (round(v, 2) if isinstance(v, float) else v) for k, v in stats.items()}, latences


def ecrire_resultat(dossier, brut, entete):
    stats, latences = analyser(brut)
    res = dict(entete, banc="ecran", stats=stats, latences=latences)
    (dossier / "resultat.json").write_text(json.dumps(res, ensure_ascii=False, indent=1))
    return res


def montrer(res):
    s = res["stats"]
    f = lambda v: "—" if v is None else f"{v:.1f}"
    print(f"\n{res['etiquette']} ({'plancher' if res.get('plancher') else 'écran à écran'}) — {s['n']} images mesurées")
    print(f"  latence p50 {f(s['p50'])} ms · p95 {f(s['p95'])} ms · p99 {f(s['p99'])} ms · "
          f"moyenne {f(s['moyenne'])} ms (min {f(s['min'])}, max {f(s['max'])})")
    print(f"  {f(s['fps'])} images de la Station par seconde à l'écran · {s['dechirees_pct']} % déchirées · "
          f"{s['rejetees']} lectures rejetées · {s['sans_bande']} captures sans bande")
    print(f"  précision de l'horloge : ± {s['precision_horloge_ms']} ms")


def cmd_ecran(a):
    exe = lecteur()
    etiquette = a.etiquette or ("plancher" if a.plancher else "mesure")
    dossier = ICI / "mesures" / f"{datetime.now():%Y%m%d-%H%M%S}-{etiquette}"
    dossier.mkdir(parents=True)
    sonde = Sonde()
    srv = serveur(sonde, a.port)
    url = f"http://{'127.0.0.1' if a.plancher else adresse_locale()}:{a.port}/sonde"
    if a.plancher and not a.sans_ouvrir:
        subprocess.run(["open", url])
    print(f"Sonde : {url}\n  {'Ouverte sur ce Mac.' if a.plancher else 'À ouvrir en plein écran sur la Station (F11), puis la session au premier plan ici.'}")
    print("En attente de la sonde…", flush=True)
    if not sonde.premiere.wait(a.attente):
        raise SystemExit("La sonde ne s'est pas manifestée.")
    time.sleep(a.delai)
    with open(dossier / "lectures.jsonl", "w") as sortie:
        r = subprocess.run([str(exe), "--duree", str(a.duree)], stdout=sortie)
    time.sleep(1.2)  # le dernier lot de la sonde
    srv.shutdown()
    if r.returncode != 0:
        raise SystemExit(f"Le lecteur a échoué (code {r.returncode}).")
    lectures = [json.loads(l) for l in open(dossier / "lectures.jsonl") if l.strip()]
    with sonde.verrou:
        brut = {"images": list(sonde.images), "horloges": list(sonde.horloges), "ecran_sonde": sonde.ecran}
    (dossier / "sonde.json").write_text(json.dumps(brut))
    brut["lectures"] = lectures
    entete = {
        "etiquette": etiquette,
        "date": datetime.now().isoformat(timespec="seconds"),
        "plancher": a.plancher,
        "appareil": platform.node(),
        "ecran_sonde": sonde.ecran,
        "note": a.note or "",
    }
    (dossier / "entete.json").write_text(json.dumps(entete, ensure_ascii=False))
    res = ecrire_resultat(dossier, brut, entete)
    montrer(res)
    print(f"\nMesure : {dossier / 'resultat.json'}")


def cmd_analyse(a):
    d = Path(a.dossier)
    brut = json.loads((d / "sonde.json").read_text())
    brut["lectures"] = [json.loads(l) for l in open(d / "lectures.jsonl") if l.strip()]
    montrer(ecrire_resultat(d, brut, json.loads((d / "entete.json").read_text())))


# --- Débit automatique de plusieurs versions -----------------------------------

def cmd_debit(a):
    racine = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], cwd=ICI, text=True).strip())
    cible = ICI / "target"
    dossier = ICI / "mesures" / f"{datetime.now():%Y%m%d-%H%M%S}-debit"
    dossier.mkdir(parents=True)
    sorties = []
    for v in a.versions:
        nom = v if v != "." else "arbre de travail"
        copie = cible / "arbres" / (v.replace("/", "_") if v != "." else "arbre")
        # Le banc de cette branche, avec le calcul de la version comparée :
        # les règles du rejeu restent les mêmes d'une version à l'autre.
        (copie / "tools").mkdir(parents=True, exist_ok=True)
        (copie / "apps/core/src").mkdir(parents=True, exist_ok=True)
        subprocess.run(["rsync", "-a", "--delete", "--exclude", "target", "--exclude", "mesures",
                        f"{ICI}/", str(copie / "tools/banc")], check=True)
        if v == ".":
            texte = (racine / "apps/core/src/adaptive.rs").read_text()
        else:
            texte = subprocess.check_output(["git", "show", f"{v}:apps/core/src/adaptive.rs"], cwd=racine, text=True)
        f = copie / "apps/core/src/adaptive.rs"
        if not f.exists() or f.read_text() != texte:
            f.write_text(texte)
        # Un dossier de construction par version : partagé, cargo garde le
        # binaire d'une autre version.
        construit = cible / "versions" / copie.name
        env = dict(os.environ, CARGO_TARGET_DIR=str(construit))
        print(f"Rejeu du débit automatique de {nom}…", flush=True)
        r = subprocess.run(["cargo", "build", "--release", "--quiet", "--manifest-path", str(copie / "tools/banc/Cargo.toml")],
                           env=env)
        if r.returncode:
            raise SystemExit(f"{nom} : le calcul ne se construit pas avec le banc (voir docs/banc-de-mesure.md, "
                             "« Quand adaptive.rs change »).")
        sortie = dossier / f"{v.replace('/', '_') if v != '.' else 'arbre'}.json"
        cmd = [str(construit / "release/banc"), "rejeu-debit", "--besoin", str(a.besoin), "--rtt", str(a.rtt),
               "--json", str(sortie), "--etiquette", nom] + a.traces
        subprocess.run(cmd, check=True)
        sorties.append(str(sortie))
    cmd_rapport(argparse.Namespace(mesures=sorties, o=a.o or str(dossier / "rapport.html")))


# --- Rapport ---------------------------------------------------------------------

STYLE = """
:root { --fond:#fff; --texte:#1d1d1f; --doux:#6e6e73; --trait:#e5e5ea; --mieux:#1a7f37; --pire:#c62828;
  --s0:#2b6cb0; --s1:#d97706; --s2:#7c3aed; --s3:#0f766e; color-scheme: light dark; }
@media (prefers-color-scheme: dark) { :root { --fond:#1c1c1e; --texte:#f2f2f7; --doux:#98989d; --trait:#38383a;
  --mieux:#4ade80; --pire:#f87171; --s0:#60a5fa; --s1:#fbbf24; --s2:#a78bfa; --s3:#2dd4bf; } }
body { background:var(--fond); color:var(--texte); font:15px/1.5 -apple-system, system-ui, sans-serif; margin:0 auto; max-width:1100px; padding:24px 16px 64px; }
h1 { font-size:24px; margin:0 0 4px } h2 { font-size:19px; margin:36px 0 8px } h3 { font-size:15px; margin:20px 0 6px }
p.doux, .doux { color:var(--doux) }
table { border-collapse:collapse; width:100%; margin:8px 0; font-variant-numeric:tabular-nums }
th, td { padding:6px 10px; border-bottom:1px solid var(--trait); text-align:right; white-space:nowrap }
th:first-child, td:first-child { text-align:left }
.mieux { color:var(--mieux) } .pire { color:var(--pire) }
.tableau { overflow-x:auto }
svg { width:100%; height:auto; display:block } svg text { fill:var(--doux); font-size:11px }
.legende span { display:inline-block; margin-right:16px } .legende i { display:inline-block; width:12px; height:3px; vertical-align:middle; margin-right:6px }
"""

COULEURS = ["var(--s0)", "var(--s1)", "var(--s2)", "var(--s3)"]


def fmt(v, d=1):
    return "—" if v is None else f"{v:.{d}f}"


def ecart(a, b, plus_bas_mieux=True, d=1):
    if a is None or b is None:
        return "<td>—</td>"
    diff = b - a
    if abs(diff) < 10 ** -d / 2:
        return "<td class='doux'>=</td>"
    mieux = (diff < 0) == plus_bas_mieux
    pct = f" ({diff / a * 100:+.0f} %)" if a else ""
    return f"<td class='{'mieux' if mieux else 'pire'}'>{diff:+.{d}f}{pct}</td>"


def courbe_cdf(series, largeur=720, hauteur=260):
    """Fonction de répartition des latences : une ligne par mesure."""
    toutes = [v for _, l in series for v in l]
    if not toutes:
        return ""
    xmax = centile(toutes, 99.5) * 1.1
    g, d, h, b = 44, 12, 12, 30
    W, H = largeur - g - d, hauteur - h - b
    out = [f"<svg viewBox='0 0 {largeur} {hauteur}' role='img' aria-label='Répartition des latences'>"]
    pas = 10 ** math.floor(math.log10(max(xmax, 1) / 2))
    pas = pas * (2 if xmax / pas > 10 else 1) * (2.5 if xmax / pas > 25 else 1)
    x = 0.0
    while x <= xmax:
        px = g + W * x / xmax
        out.append(f"<line x1='{px:.1f}' x2='{px:.1f}' y1='{h}' y2='{h + H}' stroke='var(--trait)'/>"
                   f"<text x='{px:.1f}' y='{hauteur - 10}' text-anchor='middle'>{x:g} ms</text>")
        x += pas
    for q in (0, 50, 95, 100):
        py = h + H * (1 - q / 100)
        out.append(f"<line x1='{g}' x2='{g + W}' y1='{py:.1f}' y2='{py:.1f}' stroke='var(--trait)'/>"
                   f"<text x='{g - 6}' y='{py + 4:.1f}' text-anchor='end'>{q} %</text>")
    for i, (_, l) in enumerate(series):
        l = sorted(l)
        if not l:
            continue
        pts = " ".join(f"{g + W * min(v, xmax) / xmax:.1f},{h + H * (1 - (k + 1) / len(l)):.1f}"
                       for k, v in enumerate(l))
        out.append(f"<polyline points='{pts}' fill='none' stroke='{COULEURS[i % 4]}' stroke-width='2'/>")
    out.append("</svg>")
    return "".join(out)


def courbe_debit(series, largeur=720, hauteur=200):
    """Débit demandé de chaque version et capacité du lien, seconde par seconde."""
    lignes0 = series[0][1]["lignes"]
    tmax = len(lignes0)
    ymax = max([l["demande_mbps"] for _, b in series for l in b["lignes"]] +
               [l["capacite_mbps"] for l in lignes0]) * 1.1 or 1
    g, d, h, b = 44, 12, 12, 26
    W, H = largeur - g - d, hauteur - h - b
    X = lambda t: g + W * t / max(tmax - 1, 1)
    Y = lambda v: h + H * (1 - v / ymax)
    out = [f"<svg viewBox='0 0 {largeur} {hauteur}' role='img' aria-label='Débit demandé et capacité'>"]
    for k in range(5):
        v = ymax * k / 4
        out.append(f"<line x1='{g}' x2='{g + W}' y1='{Y(v):.1f}' y2='{Y(v):.1f}' stroke='var(--trait)'/>"
                   f"<text x='{g - 6}' y='{Y(v) + 4:.1f}' text-anchor='end'>{v:.0f}</text>")
    for t in range(0, tmax, 30):
        out.append(f"<text x='{X(t):.1f}' y='{hauteur - 8}' text-anchor='middle'>{t} s</text>")
    cap = [(X(l["t_s"]), Y(l["capacite_mbps"])) for l in lignes0 if l["capacite_mbps"] > 0]
    if cap:
        out.append("<polyline points='" + " ".join(f"{x:.1f},{y:.1f}" for x, y in cap) +
                   "' fill='none' stroke='var(--doux)' stroke-dasharray='4 3' stroke-width='1.5'/>")
    for i, (_, bl) in enumerate(series):
        pts = " ".join(f"{X(l['t_s']):.1f},{Y(l['demande_mbps']):.1f}" for l in bl["lignes"])
        out.append(f"<polyline points='{pts}' fill='none' stroke='{COULEURS[i % 4]}' stroke-width='2'/>")
    out.append("</svg>")
    return "".join(out)


def legende(noms, capacite=False):
    s = "".join(f"<span><i style='background:{COULEURS[i % 4]}'></i>{html.escape(n)}</span>" for i, n in enumerate(noms))
    if capacite:
        s += "<span><i style='background:var(--doux)'></i>capacité du lien</span>"
    return f"<p class='legende doux'>{s}</p>"


def cmd_rapport(a):
    docs = []
    for f in a.mesures:
        p = Path(f)
        if p.is_dir():
            p = p / "resultat.json"
        d = json.loads(p.read_text())
        d.setdefault("etiquette", p.parent.name)
        docs.append(d)
    ecrans = [d for d in docs if d.get("banc") == "ecran"]
    rejeux = [d for d in docs if d.get("banc") == "rejeu-debit"]
    metriques = [d for d in docs if d.get("banc") == "metriques"]
    corps = [f"<h1>Banc de mesure KyberNow</h1><p class='doux'>Rapport du {datetime.now():%d/%m/%Y %H:%M} · "
             f"référence : {html.escape(docs[0]['etiquette'])}</p>"]

    if ecrans:
        ref = ecrans[0]
        corps.append("<h2>Latence de l'écran à l'écran</h2><p class='doux'>De l'image montrée par la Station à la "
                     "même image montrée par l'appareil, en millisecondes. Plus bas est mieux, sauf les images par "
                     "seconde. L'écart est mesuré par rapport à la première colonne.</p>")
        lignes = [
            ("Médiane (p50)", "p50", True), ("p95", "p95", True), ("p99", "p99", True),
            ("Moyenne", "moyenne", True), ("Max", "max", True),
            ("Images par seconde à l'écran", "fps", False), ("Images déchirées (%)", "dechirees_pct", True),
            ("Images mesurées", "n", False), ("Précision de l'horloge (± ms)", "precision_horloge_ms", True),
        ]
        t = ["<div class='tableau'><table><tr><th></th>"]
        for i, e in enumerate(ecrans):
            nom = html.escape(e["etiquette"]) + (" <span class='doux'>(plancher)</span>" if e.get("plancher") else "")
            t.append(f"<th>{nom}</th>" + ("" if i == 0 else "<th>écart</th>"))
        t.append("</tr>")
        for libelle, cle, bas in lignes:
            t.append(f"<tr><td>{libelle}</td>")
            for i, e in enumerate(ecrans):
                v = e["stats"].get(cle)
                t.append(f"<td>{fmt(v, 0 if cle == 'n' else 1)}</td>")
                if i:
                    t.append(ecart(ref["stats"].get(cle), v, bas) if cle != "n" else "<td></td>")
            t.append("</tr>")
        t.append("</table></div>")
        corps += t
        corps.append("<h3>Répartition</h3>" + legende([e["etiquette"] for e in ecrans]) +
                     courbe_cdf([(e["etiquette"], e["latences"]) for e in ecrans]))
        notes = [f"<li><b>{html.escape(e['etiquette'])}</b> : {html.escape(e.get('note', ''))} "
                 f"<span class='doux'>({e.get('date', '')}, {html.escape(str(e.get('appareil', '')))})</span></li>"
                 for e in ecrans]
        corps.append("<ul class='doux'>" + "".join(notes) + "</ul>")

    if metriques:
        corps.append("<h2>Étapes de la chaîne</h2><p class='doux'>Mesures de la session (kyclient --metrics, "
                     "tools/latency/analyze_metrics.py --json), en millisecondes : médiane et p95 de chaque étape, "
                     "de la capture sur la Station à l'affichage. Elles ne comptent ni la composition de l'écran de "
                     "la Station ni celle de l'appareil : c'est l'écran à l'écran qui les donne.</p>")
        t = ["<div class='tableau'><table><tr><th></th>"]
        for i, m in enumerate(metriques):
            t.append(f"<th>{html.escape(m['etiquette'])} p50</th><th>p95</th>" + ("" if i == 0 else "<th>écart p50</th>"))
        t.append("</tr>")
        ref = {e["etape"]: e for e in metriques[0]["etapes"]}
        for nom in [e["etape"] for e in metriques[0]["etapes"]]:
            t.append(f"<tr><td>{html.escape(nom)}</td>")
            for i, m in enumerate(metriques):
                e = next((x for x in m["etapes"] if x["etape"] == nom), None)
                t.append(f"<td>{fmt(e and e['p50'], 2)}</td><td>{fmt(e and e['p95'], 2)}</td>")
                if i:
                    t.append(ecart(ref[nom]["p50"], e and e["p50"], True, 2))
            t.append("</tr>")
        t.append("</table></div>")
        corps += t

    if rejeux:
        ref = rejeux[0]
        corps.append("<h2>Débit automatique rejoué</h2><p class='doux'>Le calcul du débit automatique de chaque version "
                     f"rejoué sur les mêmes traces, sans réseau (débit du profil : {ref.get('besoin_mbps', 60):g} Mbit/s). "
                     "Utilisation : part du débit utile disponible réellement reçue. File &gt; 15 ms : secondes où "
                     "l'image attend dans le réseau. Trous : secondes avec des pertes que la correction d'erreurs ne "
                     "rattrape pas.</p>")
        cles = [("Reçu (Mbit/s)", "recu_moyen_mbps", False), ("Utilisation (%)", "utilisation_pct", False),
                ("File > 15 ms (s)", "secondes_file", True), ("Trous (s)", "secondes_trous", True),
                ("Paquets perdus", "paquets_perdus", True), ("Changements", "changements", True),
                ("Aller-retour p95 (ms)", "rtt_p95_ms", True)]
        par_trace = {}
        for r in rejeux:
            for b in r["bilans"]:
                par_trace.setdefault(b["trace"], []).append((r["etiquette"], b))
        for trace, series in par_trace.items():
            corps.append(f"<h3>{html.escape(trace)}</h3><div class='tableau'><table><tr><th></th>")
            for i, (nom, _) in enumerate(series):
                corps.append(f"<th>{html.escape(nom)}</th>" + ("" if i == 0 else "<th>écart</th>"))
            corps.append("</tr>")
            for libelle, cle, bas in cles:
                corps.append(f"<tr><td>{libelle}</td>")
                for i, (_, b) in enumerate(series):
                    d = 0 if isinstance(b[cle], int) else 1
                    corps.append(f"<td>{fmt(float(b[cle]), d)}</td>")
                    if i:
                        corps.append(ecart(float(series[0][1][cle]), float(b[cle]), bas, d))
                corps.append("</tr>")
            corps.append("</table></div>")
            corps.append(legende([n for n, _ in series], capacite=True) + courbe_debit(series))

    page = (f"<!doctype html><html lang='fr'><head><meta charset='utf-8'>"
            f"<meta name='viewport' content='width=device-width, initial-scale=1'><title>Banc KyberNow</title>"
            f"<style>{STYLE}</style></head><body>{''.join(corps)}</body></html>")
    sortie = Path(a.o)
    sortie.write_text(page)
    print(f"Rapport : {sortie.resolve()}")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    s = p.add_subparsers(dest="cmd", required=True)
    e = s.add_parser("ecran", help="mesure de l'écran à l'écran")
    e.add_argument("--duree", type=float, default=20)
    e.add_argument("--etiquette")
    e.add_argument("--note", help="réglages de la session, version, réseau…")
    e.add_argument("--plancher", action="store_true", help="la sonde sur ce Mac même, sans KyberNow")
    e.add_argument("--sans-ouvrir", action="store_true", help="avec --plancher : ne pas ouvrir la sonde")
    e.add_argument("--port", type=int, default=8765)
    e.add_argument("--attente", type=float, default=300, help="attente de la sonde, en secondes")
    e.add_argument("--delai", type=float, default=3, help="secondes avant la capture (mettre la session devant)")
    e.set_defaults(f=cmd_ecran)
    n = s.add_parser("analyse", help="recalcule une mesure")
    n.add_argument("dossier")
    n.set_defaults(f=cmd_analyse)
    b = s.add_parser("debit", help="rejoue le débit automatique de plusieurs versions")
    b.add_argument("versions", nargs="*", default=["main", "."])
    b.add_argument("--trace", dest="traces", action="append", default=[], help="profil ou fichier (plusieurs fois)")
    b.add_argument("--besoin", type=float, default=60, help="débit du profil, Mbit/s")
    b.add_argument("--rtt", type=float, default=10, help="aller-retour de base ajouté, ms")
    b.add_argument("-o")
    b.set_defaults(f=cmd_debit)
    r = s.add_parser("rapport", help="compare des mesures dans une page HTML")
    r.add_argument("mesures", nargs="+")
    r.add_argument("-o", default="rapport.html")
    r.set_defaults(f=cmd_rapport)
    a = p.parse_args()
    a.f(a)


if __name__ == "__main__":
    main()
