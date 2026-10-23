#!/usr/bin/env python3
"""Polices de l'app webOS : instances fixes des polices variables de la charte
(brand/v2/fonts), réduites aux caractères latins, dans apps/webos/assets/fonts.
L'app les dessine avec fontdue, qui ne lit pas les axes variables.

    pip install fonttools
    python3 apps/webos/scripts/fonts.py
"""
from pathlib import Path

from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[3]
SRC = ROOT / "brand/v2/fonts"
OUT = ROOT / "apps/webos/assets/fonts"

# Latin de base et étendu A, ponctuation, flèches, quelques signes.
TEXT = "".join(chr(c) for c in range(0x20, 0x7F)) + "".join(chr(c) for c in range(0xA0, 0x180)) + \
    "‘’“”‹›«»–—…•·×÷−→←↑↓≈≤≥°€™ŒœŸ✓"

INSTANCES = {
    "archivo-display.ttf": ("Archivo", {"wdth": 125, "wght": 720}),
    "archivo-title.ttf": ("Archivo", {"wdth": 125, "wght": 650}),
    "archivo-regular.ttf": ("Archivo", {"wdth": 100, "wght": 420}),
    "archivo-semibold.ttf": ("Archivo", {"wdth": 100, "wght": 620}),
    "martian-regular.ttf": ("MartianMono", {"wdth": 100, "wght": 400}),
    "martian-medium.ttf": ("MartianMono", {"wdth": 100, "wght": 540}),
}


def source(family: str) -> Path:
    for p in sorted(SRC.iterdir()):
        if p.name.lower().startswith(family.lower()) and p.suffix == ".ttf" and "italic" not in p.name.lower():
            return p
    raise SystemExit(f"{family} absente de {SRC}")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, (family, axes) in INSTANCES.items():
        font = TTFont(source(family))
        axes = {k: v for k, v in axes.items() if k in {a.axisTag for a in font["fvar"].axes}}
        inst = instantiateVariableFont(font, axes)
        opts = Options()
        opts.layout_features = ["kern", "liga", "tnum"]
        opts.name_IDs = ["*"]
        sub = Subsetter(opts)
        sub.populate(text=TEXT)
        sub.subset(inst)
        inst.save(OUT / name)
        print(f"{name} : {(OUT / name).stat().st_size // 1024} Ko")


if __name__ == "__main__":
    main()
