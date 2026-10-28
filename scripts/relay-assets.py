#!/usr/bin/env python3
"""Fichiers du site que le relais embarque (server/relay/assets).

L'image Docker du relais ne voit que server/relay : ce script y écrit
  - kybernow.css  : les jetons de la charte, copie de brand/v2/tokens/kybernow.css ;
  - archivo.woff2 et martian-mono.woff2 : les polices de la charte, réduites au
    latin (français et anglais) et aux graisses que la page emploie.
  - icons-symbols.html : les icônes Phosphor (MIT) de la page, extraites de
    site/assets/icons.svg, que la page d'accueil écrit en ligne ;
  - les licences des polices (OFL) et des icônes (MIT), à côté des fichiers.
Le logo et le favicon sont écrits par brand/v2/tools/generate.mjs.

À relancer quand la charte ou les polices changent :
  python3 -m venv /tmp/fonts && /tmp/fonts/bin/pip install fonttools brotli
  /tmp/fonts/bin/python scripts/relay-assets.py

La mise en page de la page d'accueil (share.css) s'écrit à la main dans le
même dossier.
"""

import re
import shutil
import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "server" / "relay" / "assets"

# Latin de base et Latin-1 (accents, « », ·, ×), œ, espaces fines, tirets,
# apostrophes et guillemets typographiques, points de suspension, €, flèches, puce.
UNICODES = (
    list(range(0x20, 0x7F))
    + list(range(0xA0, 0x100))
    + [0x152, 0x153, 0x2009, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D]
    + [0x2022, 0x2026, 0x202F, 0x20AC, 0x2190, 0x2192]
)


# Icônes de la page d'accueil (identifiants de site/assets/icons.svg).
ICONS = [
    "game-controller", "cursor-click", "frame-corners", "globe", "shield-check", "browser",
    "desktop-tower", "plugs", "arrow-right", "download-simple", "book-open-text", "qr-code",
]


def write_icons() -> None:
    sprite = (ROOT / "site" / "assets" / "icons.svg").read_text(encoding="utf-8")
    symbols = dict(re.findall(r'<symbol id="([^"]+)"(.*?)</symbol>', sprite, flags=re.S))
    missing = [name for name in ICONS if name not in symbols]
    if missing:
        sys.exit(f"icônes absentes de site/assets/icons.svg : {missing}")
    lines = [f'<symbol id="{name}"{symbols[name]}</symbol>' for name in ICONS]
    dest = OUT / "icons-symbols.html"
    dest.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{dest.relative_to(ROOT)}: {dest.stat().st_size} octets")


def reduce_font(src: Path, dest: Path, limits: dict) -> None:
    font = TTFont(src)
    options = subset.Options()
    options.layout_features = ["*"]
    options.name_IDs = [1, 2]
    options.hinting = False
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=UNICODES)
    subsetter.subset(font)
    # Les glyphes d'abord, puis les axes (l'inverse casse gvar sur Martian Mono).
    font = instancer.instantiateVariableFont(font, limits)
    font.flavor = "woff2"
    font.save(dest)
    print(f"{dest.relative_to(ROOT)}: {dest.stat().st_size} octets")


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "brand" / "v2" / "tokens" / "kybernow.css", OUT / "kybernow.css")
    write_icons()
    for licence in ("OFL-Archivo.txt", "OFL-MartianMono.txt"):
        shutil.copyfile(ROOT / "brand" / "v2" / "fonts" / licence, OUT / licence)
    shutil.copyfile(ROOT / "site" / "assets" / "PHOSPHOR-LICENSE", OUT / "PHOSPHOR-LICENSE")
    # Archivo : titres en largeur 125 (graisses 380 à 760), texte en largeur 100.
    reduce_font(
        ROOT / "brand" / "v2" / "fonts" / "Archivo-wdth-wght.ttf",
        OUT / "archivo.woff2",
        {"wght": (380, 760), "wdth": (100, 125)},
    )
    # Martian Mono : étiquettes et code, en largeur 100.
    reduce_font(
        ROOT / "brand" / "v2" / "fonts" / "MartianMono-wdth-wght.ttf",
        OUT / "martian-mono.woff2",
        {"wght": (400, 700), "wdth": 100},
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
