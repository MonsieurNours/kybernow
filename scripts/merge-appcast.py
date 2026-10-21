#!/usr/bin/env python3
"""Réunit des flux Sparkle en un flux cumulatif (site/updates/appcast.xml).

    scripts/merge-appcast.py -o site/updates/appcast.xml \\
        [--base FLUX] [--stable FLUX]... [--beta FLUX]...

Chaque <item> est recopié tel que generate_appcast l'a écrit : lien de
téléchargement vers la release de sa version, longueur, signature EdDSA et
notes restent ceux du flux d'origine (rien n'est re-signé). Une même version
(sparkle:shortVersionString) présente deux fois : la dernière lue l'emporte,
dans l'ordre --base, puis les autres options dans l'ordre de la commande.
--beta ajoute <sparkle:channel>beta</sparkle:channel> s'il manque ; --stable
retire un canal éventuel. Les entrées sont rangées de la plus récente à la
plus ancienne (sparkle:version, le numéro de build).
"""

import argparse
import re
import sys
import xml.etree.ElementTree as ET

SPARKLE = "http://www.andymatuschak.org/xml-namespaces/sparkle"
ITEM = re.compile(r"[ \t]*<item>.*?</item>", re.S)
NS_DECL = re.compile(r'xmlns:(\w+)="([^"]+)"')
CHANNEL = re.compile(r"[ \t]*<sparkle:channel>[^<]*</sparkle:channel>\n?")


def read_items(path, channel):
    """(clé, build, texte de l'item, espaces de noms) pour chaque item du flux."""
    with open(path, encoding="utf-8") as f:
        text = f.read()
    ET.fromstring(text.encode("utf-8"))  # flux bien formé, sinon erreur claire
    root_tag = re.search(r"<rss\b[^>]*>", text)
    namespaces = dict(NS_DECL.findall(root_tag.group(0))) if root_tag else {}
    namespaces.setdefault("sparkle", SPARKLE)
    decl = " ".join(f'xmlns:{k}="{v}"' for k, v in namespaces.items())
    items = []
    for m in ITEM.finditer(text):
        raw = m.group(0)
        if channel == "beta" and "<sparkle:channel>" not in raw:
            indent = re.match(r"[ \t]*", raw).group(0)
            raw = raw.replace("<item>", f"<item>\n{indent}    <sparkle:channel>beta</sparkle:channel>", 1)
        elif channel == "stable":
            raw = CHANNEL.sub("", raw)
        node = ET.fromstring(f"<rss {decl}>{raw}</rss>".encode("utf-8")).find("item")
        short = node.findtext(f"{{{SPARKLE}}}shortVersionString") or node.findtext("title")
        build = node.findtext(f"{{{SPARKLE}}}version") or "0"
        if not short:
            sys.exit(f"{path} : entrée sans numéro de version")
        if node.find("enclosure") is None:
            sys.exit(f"{path} : entrée {short} sans fichier (enclosure)")
        items.append((short.strip(), build.strip(), raw, namespaces))
    if not items:
        sys.exit(f"{path} : aucune entrée")
    return items


def sort_key(entry):
    short, build, _, _ = entry
    semver = tuple(int(x) if x.isdigit() else 0 for x in short.split("."))
    return (int(build) if build.isdigit() else 0, semver)


def main():
    p = argparse.ArgumentParser(description="Réunit des flux Sparkle en un flux cumulatif.")
    p.add_argument("-o", "--output", required=True)
    p.add_argument("--base", help="flux cumulatif existant (canaux gardés)")
    p.add_argument("--title", default="KyberNow")
    p.add_argument("--stable", action="append", default=[], metavar="FLUX")
    p.add_argument("--beta", action="append", default=[], metavar="FLUX")
    # Ordre de la commande gardé entre --stable et --beta.
    order = []
    argv = sys.argv[1:]
    for i, a in enumerate(argv):
        if a in ("--stable", "--beta") and i + 1 < len(argv):
            order.append((a[2:], argv[i + 1]))
    args = p.parse_args()

    entries = {}
    namespaces = {"sparkle": SPARKLE}
    sources = ([(None, args.base)] if args.base else []) + order
    for channel, path in sources:
        for short, build, raw, ns in read_items(path, channel):
            entries[short] = (short, build, raw, ns)
            namespaces.update(ns)

    decl = " ".join(f'xmlns:{k}="{v}"' for k, v in sorted(namespaces.items()))
    body = "\n".join(e[2] for e in sorted(entries.values(), key=sort_key, reverse=True))
    out = (
        '<?xml version="1.0" standalone="yes"?>\n'
        f'<rss {decl} version="2.0">\n'
        "    <channel>\n"
        f"        <title>{args.title}</title>\n"
        f"{body}\n"
        "    </channel>\n"
        "</rss>\n"
    )
    ET.fromstring(out.encode("utf-8"))
    with open(args.output, "w", encoding="utf-8") as f:
        f.write(out)
    for short, build, raw, _ in sorted(entries.values(), key=sort_key, reverse=True):
        beta = " (bêta)" if "<sparkle:channel>beta</sparkle:channel>" in raw else ""
        print(f"    {short}{beta}")


if __name__ == "__main__":
    main()
