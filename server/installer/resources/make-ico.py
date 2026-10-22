#!/usr/bin/env python3
"""Génère kybernow.ico (identité v2, symbole « server ») avec Pillow.

- 16 à 32 px : le symbole seul (zone de notification, petites icônes) ;
- 48 à 256 px : la tuile de l'app (fond sombre, halo, couture en néon),
  comme brand/v2/app-icon/kybernow-station-1024.svg.

Usage : python3 make-ico.py  (écrit kybernow.ico à côté de ce script)
"""

import math
import re
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

# brand/v2/tools/geometry.json, dans un carré de 48.
SRC = "M7 0 L42.275 0 A1.2 1.2 0 0 1 43.123 2.049 L2.049 43.123 A1.2 1.2 0 0 1 0 42.275 L0 7 A7 7 0 0 1 7 0 Z"
DST = "M48 5.725 L48 41 A7 7 0 0 1 41 48 L5.725 48 A1.2 1.2 0 0 1 4.877 45.951 L45.951 4.877 A1.2 1.2 0 0 1 48 5.725 Z"
SIGNAL = (154, 140, 255)
FROST = (227, 237, 240)
NEON = (244, 241, 255)
SS = 8  # suréchantillonnage


def arc_points(x1, y1, rx, ry, phi, large, sweep, x2, y2, steps=24):
    """Arc SVG (paramètres de fin) → points (rx == ry ici, phi = 0)."""
    dx, dy = (x1 - x2) / 2, (y1 - y2) / 2
    r = rx
    d2 = dx * dx + dy * dy
    r = max(r, math.sqrt(d2))
    k = math.sqrt(max(0.0, (r * r - d2) / d2)) if d2 else 0
    if large == sweep:
        k = -k
    cx, cy = k * dy + (x1 + x2) / 2, -k * dx + (y1 + y2) / 2
    a1 = math.atan2(y1 - cy, x1 - cx)
    a2 = math.atan2(y2 - cy, x2 - cx)
    delta = a2 - a1
    if sweep and delta < 0:
        delta += 2 * math.pi
    if not sweep and delta > 0:
        delta -= 2 * math.pi
    return [(cx + r * math.cos(a1 + delta * i / steps), cy + r * math.sin(a1 + delta * i / steps)) for i in range(1, steps + 1)]


def polygon(path, scale, ox=0.0, oy=0.0):
    tokens = re.findall(r"[MLAZ]|-?[0-9.]+", path)
    pts, i, cur = [], 0, (0.0, 0.0)
    while i < len(tokens):
        t = tokens[i]
        if t in ("M", "L"):
            cur = (float(tokens[i + 1]), float(tokens[i + 2]))
            pts.append(cur)
            i += 3
        elif t == "A":
            rx, ry, phi, large, sweep, x, y = (float(v) for v in tokens[i + 1 : i + 8])
            pts += arc_points(cur[0], cur[1], rx, ry, phi, int(large), int(sweep), x, y)
            cur = (x, y)
            i += 8
        else:  # Z
            i += 1
    return [(ox + x * scale, oy + y * scale) for x, y in pts]


def mark(size):
    """Le symbole seul, en couleurs pleines : moitié haute en signal, basse en givre."""
    big = size * SS
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    s = big / 48
    d.polygon(polygon(SRC, s), fill=SIGNAL + (255,))
    d.polygon(polygon(DST, s), fill=FROST + (255,))
    return img.resize((size, size), Image.LANCZOS)


def tile(size):
    """La tuile de l'app : fond, halo, symbole et couture allumée."""
    big = size * SS
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    inset, radius = big * 100 / 1024, big * 185 / 1024
    box = (inset, inset, big - inset, big - inset)
    # Fond en dégradé diagonal, halo violet.
    bg = Image.new("RGBA", (big, big))
    px = bg.load()
    for y in range(0, big, SS):
        for x in range(0, big, SS):
            t = (x + y) / (2 * big)
            c = tuple(int(a + (b - a) * t) for a, b in zip((11, 16, 22), (5, 7, 10)))
            for yy in range(SS):
                for xx in range(SS):
                    if x + xx < big and y + yy < big:
                        px[x + xx, y + yy] = c + (255,)
    halo = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    hd = ImageDraw.Draw(halo)
    r = big * 0.42
    hd.ellipse((big * 0.62 - r, big * 0.62 - r, big * 0.62 + r, big * 0.62 + r), fill=(126, 108, 255, 90))
    halo = halo.filter(ImageFilter.GaussianBlur(big * 0.12))
    bg = Image.alpha_composite(bg, halo)
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle(box, radius, fill=255)
    img.paste(bg, (0, 0), mask)
    # Symbole : 11,25 unités par point, décalé de 242 (sur 1024).
    s, o = big * 11.25 / 1024, big * 242 / 1024
    layer = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    d.polygon(polygon(SRC, s, o, o), fill=SIGNAL + (255,))
    d.polygon(polygon(DST, s, o, o), fill=FROST + (255,))
    # Couture en néon : halo, gaine, cœur.
    a, b = (o + 5.6 * s, o + 42.4 * s), (o + 42.4 * s, o + 5.6 * s)
    glow = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(glow).line([a, b], fill=SIGNAL + (230,), width=int(9 * s))
    glow = glow.filter(ImageFilter.GaussianBlur(3.4 * s))
    sheath = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(sheath).line([a, b], fill=SIGNAL + (255,), width=int(2.6 * s))
    sheath = sheath.filter(ImageFilter.GaussianBlur(0.8 * s))
    core = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(core).line([a, b], fill=NEON + (255,), width=max(1, int(1.2 * s)))
    for part in (layer, glow, sheath, core):
        img = Image.alpha_composite(img, part)
    # Rien ne déborde de la tuile.
    img.putalpha(ImageChops.multiply(img.getchannel("A"), mask))
    return img.resize((size, size), Image.LANCZOS)


def main():
    sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    images = [mark(n) if n <= 32 else tile(n) for n in sizes]
    out = Path(__file__).with_name("kybernow.ico")
    images[-1].save(out, format="ICO", sizes=[(n, n) for n in sizes], append_images=images[:-1])
    print(f"{out} : {', '.join(map(str, sizes))} px")


if __name__ == "__main__":
    main()
