#!/usr/bin/env python3
"""Icône et images du Top Shelf de l'app Apple TV, depuis la géométrie du
symbole et du nom (brand/v2/tools/geometry.json).

tvOS veut une icône en couches (effet de parallaxe au focus) :
  - Back  : l'encre, une lueur Spectre et la couture en filigrane ;
  - Front : le symbole (l'ordinateur en givre, vous en Spectre) et son néon.
Plus les images du Top Shelf (rangée du haut de l'accueil), utilisées tant
que l'app n'a pas encore d'ordinateur à proposer : symbole et nom, signé
« LINK » (logo signé de KyberNow Link, geometry.json `signed.link`).

    python3 scripts/app-icon.py     (Pillow requis : pip install pillow)
"""
import json, math, pathlib, re
from PIL import Image, ImageChops, ImageDraw, ImageFilter

here = pathlib.Path(__file__).resolve().parent
geo = json.loads((here.parents[2] / "brand/v2/tools/geometry.json").read_text())
out = here.parent / "KyberNowTV/Assets.xcassets/App Icon & Top Shelf Image.brandassets"

INK = (5, 7, 10)
ICE = (232, 241, 243)
SPECTRE = (154, 140, 255)
TAG = (0x1E, 0x28, 0x32)       # étiquette du logo signé, thème sombre
TAG_TEXT = (0xC9, 0xD5, 0xDA)
K = 4  # suréchantillonnage

# --- Tracés -----------------------------------------------------------------

def rounded(points, radii, steps=24):
    """Polygone aux coins arrondis (comme addArc(tangent1End:) de SwiftUI)."""
    out = []
    n = len(points)
    for i in range(n):
        p0, p1, p2 = points[i - 1], points[i], points[(i + 1) % n]
        v1 = (p0[0] - p1[0], p0[1] - p1[1])
        v2 = (p2[0] - p1[0], p2[1] - p1[1])
        l1, l2 = math.hypot(*v1), math.hypot(*v2)
        u1, u2 = (v1[0] / l1, v1[1] / l1), (v2[0] / l2, v2[1] / l2)
        angle = math.acos(max(-1, min(1, u1[0] * u2[0] + u1[1] * u2[1])))
        r = radii[i]
        t = r / math.tan(angle / 2)
        a = (p1[0] + u1[0] * t, p1[1] + u1[1] * t)
        b = (p1[0] + u2[0] * t, p1[1] + u2[1] * t)
        bis = (u1[0] + u2[0], u1[1] + u2[1])
        bl = math.hypot(*bis)
        d = r / math.sin(angle / 2)
        c = (p1[0] + bis[0] / bl * d, p1[1] + bis[1] / bl * d)
        sa = math.atan2(a[1] - c[1], a[0] - c[0])
        sb = math.atan2(b[1] - c[1], b[0] - c[0])
        delta = (sb - sa + math.pi) % (2 * math.pi) - math.pi
        for s in range(steps + 1):
            th = sa + delta * s / steps
            out.append((c[0] + r * math.cos(th), c[1] + r * math.sin(th)))
    return out

C = 2 * math.sqrt(2)
SOURCE = rounded([(0, 0), (48 - C, 0), (0, 48 - C)], [7, 1.2, 1.2])
DEST = rounded([(48, C), (48, 48), (C, 48)], [1.2, 7, 1.2])

def subpaths(d):
    """Tracé SVG du nom (M, L, H, V, Q, Z) → liste de polygones."""
    tokens = re.findall(r"[MLHVQZ]|-?\d*\.?\d+(?:e-?\d+)?", d)
    polys, cur, pos, i, cmd = [], [], (0.0, 0.0), 0, None
    def num():
        nonlocal i
        i += 1
        return float(tokens[i - 1])
    while i < len(tokens):
        if re.match(r"[MLHVQZ]", tokens[i]):
            cmd = tokens[i]
            i += 1
            if cmd == "Z":
                if cur: polys.append(cur)
                cur = []
                continue
        if cmd == "M":
            if cur: polys.append(cur)
            pos = (num(), num()); cur = [pos]; cmd = "L"
        elif cmd == "L":
            pos = (num(), num()); cur.append(pos)
        elif cmd == "H":
            pos = (num(), pos[1]); cur.append(pos)
        elif cmd == "V":
            pos = (pos[0], num()); cur.append(pos)
        elif cmd == "Q":
            c, e = (num(), num()), (num(), num())
            for s in range(1, 9):
                t = s / 8
                cur.append(((1 - t) ** 2 * pos[0] + 2 * (1 - t) * t * c[0] + t * t * e[0],
                            (1 - t) ** 2 * pos[1] + 2 * (1 - t) * t * c[1] + t * t * e[1]))
            pos = e
    if cur: polys.append(cur)
    return polys

# --- Dessin -----------------------------------------------------------------

def fill(size, polys, color, transform, evenodd=False):
    mask = Image.new("L", size, 0)
    for poly in polys:
        layer = Image.new("L", size, 0)
        ImageDraw.Draw(layer).polygon([transform(p) for p in poly], fill=255)
        mask = ImageChops.logical_xor(mask.convert("1"), layer.convert("1")).convert("L") if evenodd else ImageChops.lighter(mask, layer)
    img = Image.new("RGBA", size, color + (0,))
    img.putalpha(mask)
    return img

def mark(w, h, side, cx, cy, neon=True):
    """Symbole (et son néon) de côté `side`, centré en (cx, cy), fond transparent."""
    W, H, S = w * K, h * K, side * K / 48
    ox, oy = cx * K - side * K / 2, cy * K - side * K / 2
    tr = lambda p: (ox + p[0] * S, oy + p[1] * S)
    img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    img = Image.alpha_composite(img, fill((W, H), [SOURCE], ICE, tr))
    img = Image.alpha_composite(img, fill((W, H), [DEST], SPECTRE, tr))
    if neon:
        a, b = tr((5.6, 42.4)), tr((42.4, 5.6))
        for width, color, blur in [(9, (0x6D, 0x5B, 0xFF, 210), 3.2), (2.6, (0x9A, 0x8C, 0xFF, 255), 0.7), (1.8, (0xF4, 0xF1, 0xFF, 255), 0.08)]:
            layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
            d = ImageDraw.Draw(layer)
            d.line([a, b], fill=color, width=round(width * S))
            r = round(width * S / 2)
            for (x, y) in (a, b):
                d.ellipse([x - r, y - r, x + r, y + r], fill=color)
            img = Image.alpha_composite(img, layer.filter(ImageFilter.GaussianBlur(blur * S)))
    return img.resize((w, h), Image.LANCZOS)

def wordmark(w, h, cap, x, baseline):
    """Le nom, hauteur de capitale `cap`, posé à (x, ligne de base)."""
    wm = geo["wordmark"]
    W, H, s = w * K, h * K, cap * K / wm["cap"]
    img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    for key, dx in (("kyber", 0), ("now", wm["nowOffset"])):
        tr = lambda p, dx=dx: (x * K + (p[0] + dx) * s, baseline * K + p[1] * s)
        img = Image.alpha_composite(img, fill((W, H), subpaths(wm[key]), ICE, tr, evenodd=True))
    return img.resize((w, h), Image.LANCZOS)

def signature(w, h, cap, x, baseline, product="link"):
    """L'étiquette du logo signé (« LINK »), sous le nom posé à (x, ligne de
    base) avec la hauteur de capitale `cap` : même mise en page que
    brand/v2/logo/kybernow-link-dark.svg, rapportée à l'origine du nom."""
    sg, cap0 = geo["signed"][product], geo["wordmark"]["cap"]
    W, H, s = w * K, h * K, cap * K / cap0
    ox, oy, k = sg["wordX"], sg["baseline"], sg["sigScale"]
    tag = lambda p: (x * K + (p[0] - ox) * s, baseline * K + (p[1] - oy) * s)
    sig = lambda p: (x * K + (sg["sigX"] - ox + p[0] * k) * s, baseline * K + (sg["sigY"] - oy + p[1] * k) * s)
    img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    img = Image.alpha_composite(img, fill((W, H), subpaths(sg["tag"]), TAG, tag))
    img = Image.alpha_composite(img, fill((W, H), subpaths(sg["sig"]), TAG_TEXT, sig, evenodd=True))
    return img.resize((w, h), Image.LANCZOS)

def back(w, h):
    """Encre, lueur Spectre en bas à droite, couture en filigrane."""
    img = Image.new("RGBA", (w, h), INK + (255,))
    glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    r = max(w, h) * 0.55
    ImageDraw.Draw(glow).ellipse([w * 0.62 - r, h * 0.66 - r, w * 0.62 + r, h * 0.66 + r], fill=(0x6D, 0x5B, 0xFF, 38))
    img = Image.alpha_composite(img, glow.filter(ImageFilter.GaussianBlur(max(w, h) * 0.18)))
    seam = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ImageDraw.Draw(seam).line([(w * 0.5 + h * 0.75, -h * 0.25), (w * 0.5 - h * 0.75, h * 1.25)], fill=(255, 255, 255, 14), width=max(1, round(h / 240)))
    return Image.alpha_composite(img, seam)

# --- Catalogue --------------------------------------------------------------

def write_json(path, data):
    path.mkdir(parents=True, exist_ok=True)
    (path / "Contents.json").write_text(json.dumps(data, indent=2) + "\n")

INFO = {"author": "xcode", "version": 1}

def stack(name, size, scales):
    root = out / f"{name}.imagestack"
    write_json(root, {"info": INFO, "layers": [{"filename": "Front.imagestacklayer"}, {"filename": "Back.imagestacklayer"}]})
    for layer in ("Front", "Back"):
        layer_dir = root / f"{layer}.imagestacklayer"
        write_json(layer_dir, {"info": INFO})
        images = []
        for scale in scales:
            w, h = size[0] * scale, size[1] * scale
            img = back(w, h) if layer == "Back" else mark(w, h, h * 0.56, w / 2, h / 2)
            file = f"{layer.lower()}{'' if scale == 1 else f'@{scale}x'}.png"
            set_dir = layer_dir / "Content.imageset"
            set_dir.mkdir(parents=True, exist_ok=True)
            img.save(set_dir / file)
            images.append({"filename": file, "idiom": "tv", "scale": f"{scale}x"})
        write_json(layer_dir / "Content.imageset", {"images": images, "info": INFO})

def shelf(name, size):
    root = out / f"{name}.imageset"
    images = []
    for scale in (1, 2):
        w, h = size[0] * scale, size[1] * scale
        img = back(w, h)
        side = h * 0.34
        cap = h * 0.13
        total = side + h * 0.06 + geo["wordmark"]["width"] * cap / geo["wordmark"]["cap"]
        x0 = (w - total) / 2
        img = Image.alpha_composite(img, mark(w, h, side, x0 + side / 2, h / 2))
        img = Image.alpha_composite(img, wordmark(w, h, cap, x0 + side + h * 0.06, h / 2 + cap / 2))
        # Signature sous le nom : elle reste dans la hauteur du symbole.
        img = Image.alpha_composite(img, signature(w, h, cap, x0 + side + h * 0.06, h / 2 + cap / 2))
        file = f"{name.replace(' ', '-').lower()}{'' if scale == 1 else '@2x'}.png"
        root.mkdir(parents=True, exist_ok=True)
        img.convert("RGB").save(root / file)
        images.append({"filename": file, "idiom": "tv", "scale": f"{scale}x"})
    write_json(root, {"images": images, "info": INFO})

stack("App Icon", (400, 240), (1, 2))
stack("App Icon - App Store", (1280, 768), (1,))
shelf("Top Shelf Image", (1920, 720))
shelf("Top Shelf Image Wide", (2320, 720))
write_json(out, {"info": INFO, "assets": [
    {"filename": "App Icon - App Store.imagestack", "idiom": "tv", "role": "primary-app-icon", "size": "1280x768"},
    {"filename": "App Icon.imagestack", "idiom": "tv", "role": "primary-app-icon", "size": "400x240"},
    {"filename": "Top Shelf Image Wide.imageset", "idiom": "tv", "role": "top-shelf-image-wide", "size": "2320x720"},
    {"filename": "Top Shelf Image.imageset", "idiom": "tv", "role": "top-shelf-image", "size": "1920x720"},
]})
print("icône et Top Shelf écrits dans", out)
