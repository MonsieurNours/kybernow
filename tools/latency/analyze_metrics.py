#!/usr/bin/env python3
"""Analyse le metrics.json produit par `kyclient --metrics true`.

Le fichier est du JSON Lines. Deux formes sont gérées :
  - lignes "frame_metric" (déjà consolidées, horodatages serveur compensés) ;
  - lignes "raw_metric" (événements bruts) : on regroupe par (source_id, pts)
    et on compense les horodatages serveur avec le décalage d'horloge mesuré
    par les "network_ping" (offset = serveur - client, en microsecondes).

Usage : analyze_metrics.py metrics.json [--skip-seconds 5]
"""

import argparse
import json
import statistics
import sys
from collections import defaultdict

SERVER_EVENTS = ("acquired", "encoding", "encoded", "sent")
CLIENT_EVENTS = ("received", "decoding", "decoded", "prepared", "displayed")

# (libellé, événement de départ, événement d'arrivée)
STAGES = (
    ("encodage (acquired → encoded)", "acquired", "encoded"),
    ("envoi (encoded → sent)", "encoded", "sent"),
    ("réseau (sent → received)", "sent", "received"),
    ("décodage (received → decoded)", "received", "decoded"),
    ("affichage (decoded → displayed)", "decoded", "displayed"),
    ("TOTAL (acquired → displayed)", "acquired", "displayed"),
)


def load(path):
    frames = {}
    raw = defaultdict(dict)
    offsets = []
    rtts = []
    lost = 0
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                m = json.loads(line)
            except json.JSONDecodeError:
                continue
            if m.get("log_type") == "frame_metric":
                frames[(m["source_id"], m["pts"])] = {
                    k: m[k] for k in SERVER_EVENTS + CLIENT_EVENTS if k in m
                }
                continue
            kind = m.get("type")
            if kind == "video":
                raw[(m["source_id"], m["pts"])][m["event"]] = m["ts"]
            elif kind == "network_ping":
                offsets.append(m["offset_micros"])
            elif kind == "network_local":
                if m.get("rtt_micros", -1) >= 0:
                    rtts.append(m["rtt_micros"])
                lost = max(lost, m.get("packets_lost", -1))

    if not frames and raw:
        if not offsets:
            sys.exit("Aucun network_ping : impossible de recaler l'horloge du serveur.")
        offset = statistics.median(offsets)
        for key, events in raw.items():
            frames[key] = {
                ev: (ts - offset if ev in SERVER_EVENTS else ts)
                for ev, ts in events.items()
            }
    return frames, offsets, rtts, lost


def pct(values, p):
    values = sorted(values)
    k = (len(values) - 1) * p / 100
    lo, hi = int(k), min(int(k) + 1, len(values) - 1)
    return values[lo] + (values[hi] - values[lo]) * (k - lo)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("metrics")
    ap.add_argument("--skip-seconds", type=float, default=5.0,
                    help="ignore le début de session (montée en charge)")
    args = ap.parse_args()

    frames, offsets, rtts, lost = load(args.metrics)
    if not frames:
        sys.exit("Aucune métrique vidéo trouvée.")

    first = min(f.get("received", f.get("displayed", 0)) for f in frames.values() if f)
    cutoff = first + args.skip_seconds * 1_000_000
    frames = {k: f for k, f in frames.items() if f.get("received", cutoff) >= cutoff}

    displayed = [f for f in frames.values() if "displayed" in f]
    skipped = sum(1 for f in frames.values() if "skipped" in f)
    span = (max(f["displayed"] for f in displayed) - min(f["displayed"] for f in displayed)) / 1e6 if len(displayed) > 1 else 0

    print(f"Images analysées : {len(frames)}  affichées : {len(displayed)}  sautées : {skipped}")
    if span:
        print(f"Durée : {span:.1f} s  →  {len(displayed) / span:.1f} fps affichés")
    if offsets:
        print(f"Décalage d'horloge serveur-client : médiane {statistics.median(offsets) / 1000:.2f} ms "
              f"(dispersion {(max(offsets) - min(offsets)) / 1000:.2f} ms)")
    if rtts:
        print(f"RTT QUIC : médiane {statistics.median(rtts) / 1000:.2f} ms")
    if lost > 0:
        print(f"Paquets perdus (cumul) : {lost}")
    print()
    print(f"{'étape':<34}{'n':>7}{'p50':>9}{'p95':>9}{'p99':>9}{'max':>9}   (ms)")
    for label, a, b in STAGES:
        d = [(f[b] - f[a]) / 1000 for f in frames.values() if a in f and b in f]
        if not d:
            print(f"{label:<34}{'—':>7}")
            continue
        print(f"{label:<34}{len(d):>7}{pct(d, 50):>9.2f}{pct(d, 95):>9.2f}{pct(d, 99):>9.2f}{max(d):>9.2f}")
    print()
    # Encodage selon le temps écoulé depuis l'image précédente : les pics après une
    # période sans changement d'écran trahissent une remontée en fréquence du GPU.
    seq = sorted((f for f in frames.values() if "acquired" in f and "encoded" in f),
                 key=lambda f: f["acquired"])
    pairs = [((b["acquired"] - a["acquired"]) / 1000, (b["encoded"] - b["acquired"]) / 1000)
             for a, b in zip(seq, seq[1:])]
    if pairs:
        print(f"Intervalle de capture : médiane {pct([g for g, _ in pairs], 50):.2f} ms")
        for label, lo, hi in (("< 50 ms", 0, 50), ("≥ 200 ms (écran resté fixe)", 200, float("inf"))):
            e = [enc for g, enc in pairs if lo <= g < hi]
            if e:
                print(f"  encodage après un écart {label:<28} n={len(e):<6} p50 {pct(e, 50):.2f}  p95 {pct(e, 95):.2f} ms")
        print()
    print("Note : « réseau » et « TOTAL » dépendent de la précision du recalage d'horloge "
          "(± la dispersion ci-dessus). Le délai d'affichage écran n'est pas inclus : "
          "voir le test caméra dans docs/archives/prototype-A.md (§3.2).")


if __name__ == "__main__":
    main()
