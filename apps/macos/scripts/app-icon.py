#!/usr/bin/env python3
"""Icône de l'app au format Icon Composer (KyberNow/AppIcon.icon), depuis la
géométrie du symbole (brand/v2/tools/geometry.json).

Couches, de haut en bas :
  - le néon de la couture (cœur, gaine, halo), sans verre : il reste lumineux ;
  - les deux moitiés du symbole, en SVG, en verre « Liquid Glass » ;
  - le fond : dégradé automatique de macOS sur l'encre de la charte.
Le symbole occupe 540/824 de la tuile, comme dans brand/v2/app-icon.

    python3 scripts/app-icon.py     (Pillow requis : pip install pillow)
Puis ouvrir KyberNow/AppIcon.icon dans Icon Composer pour l'ajuster au besoin.
"""
import json, pathlib
from PIL import Image, ImageDraw, ImageFilter

here = pathlib.Path(__file__).resolve().parent
geo = json.loads((here.parents[2] / "brand/v2/tools/geometry.json").read_text())
icon = here.parent / "KyberNow/AppIcon.icon"
assets = icon / "Assets"
assets.mkdir(parents=True, exist_ok=True)

CANVAS = 1024
SIZE = 540 / 824 * CANVAS          # côté du symbole
S = SIZE / 48                      # grille de 48 → points
O = (CANVAS - SIZE) / 2            # origine du symbole

def svg(path, color):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {CANVAS} {CANVAS}" width="{CANVAS}" height="{CANVAS}">'
            f'<g transform="translate({O:.3f} {O:.3f}) scale({S:.5f})"><path fill="{color}" d="{path}"/></g></svg>\n')

(assets / "ordinateur.svg").write_text(svg(geo["source"], "#E8F1F3"))
(assets / "vous.svg").write_text(svg(geo["dest"], "#9A8CFF"))

# Néon : tracé suréchantillonné ×4, flous gaussiens, puis réduit.
k = 4
def point(x, y):
    return ((O + x * S) * k, (O + y * S) * k)
a, b = point(5.6, 42.4), point(42.4, 5.6)
neon = Image.new("RGBA", (CANVAS * k, CANVAS * k), (0, 0, 0, 0))
for width, color, blur in [(9, (0x6D, 0x5B, 0xFF, 210), 3.2), (2.6, (0x9A, 0x8C, 0xFF, 255), 0.7), (1.8, (0xF4, 0xF1, 0xFF, 255), 0.08)]:
    layer = Image.new("RGBA", neon.size, (0, 0, 0, 0))
    ImageDraw.Draw(layer).line([a, b], fill=color, width=round(width * S * k))
    r = round(width * S * k / 2)
    for (x, y) in (a, b):
        ImageDraw.Draw(layer).ellipse([x - r, y - r, x + r, y + r], fill=color)
    neon = Image.alpha_composite(neon, layer.filter(ImageFilter.GaussianBlur(blur * S * k)))
neon.resize((CANVAS, CANVAS), Image.LANCZOS).save(assets / "neon.png")

def layer(name, image, glass):
    return {"blend-mode": "normal", "fill": "automatic", "glass": glass, "hidden": False,
            "image-name": image, "name": name, "opacity": 1,
            "position": {"scale": 1, "translation-in-points": [0, 0]}}

(icon / "icon.json").write_text(json.dumps({
    "fill": {"automatic-gradient": "srgb:0.04314,0.06275,0.08627,1.00000"},
    "groups": [
        {"layers": [layer("neon", "neon.png", False)],
         "shadow": {"kind": "none", "opacity": 0}, "translucency": {"enabled": False, "value": 0}},
        {"layers": [layer("vous", "vous.svg", True), layer("ordinateur", "ordinateur.svg", True)],
         "shadow": {"kind": "neutral", "opacity": 0.5}, "translucency": {"enabled": False, "value": 0}},
    ],
    "supported-platforms": {"circles": ["watchOS"], "squares": "shared"},
}, indent=2) + "\n")
print("icône écrite dans", icon)
