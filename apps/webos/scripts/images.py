#!/usr/bin/env python3
"""Images de l'app webOS, tirées de la charte (brand/v2) :

  - webos/icon.png (80 × 80) et webos/icon-large.png (130 × 130) : icônes
    du lanceur de webOS, dessinées comme celles de l'Apple TV et d'Android
    (apps/tvos/scripts/app-icon.py : l'encre, la lueur Spectre, le symbole et
    son néon) ;
  - webos/splash.png (1920 × 1080) : écran de lancement de webOS ;
  - assets/logo.png : le logo signé LINK (brand/v2/logo/kybernow-link-dark.svg)
    à la hauteur de l'accueil (48 px), embarqué dans l'app.

    pip install pillow cairosvg
    python3 apps/webos/scripts/images.py
"""
import io
import pathlib

import cairosvg

here = pathlib.Path(__file__).resolve().parent
root = here.parents[2]
tvos = root / "apps/tvos/scripts/app-icon.py"
source = tvos.read_text(encoding="utf-8").split("# --- Catalogue")[0]
draw = {"__file__": str(tvos)}
exec(compile(source, str(tvos), "exec"), draw)
back, mark, Image = draw["back"], draw["mark"], draw["Image"]

out = here.parent / "webos"
out.mkdir(exist_ok=True)


def icon(size, path):
    s = size * 4
    img = Image.alpha_composite(back(s, s), mark(s, s, s * 0.50, s / 2, s / 2))
    img.resize((size, size), Image.LANCZOS).convert("RGB").save(path)
    print(path.relative_to(root))


icon(80, out / "icon.png")
icon(130, out / "icon-large.png")

w, h = 1920, 1080
splash = Image.alpha_composite(back(w, h), mark(w, h, h * 0.30, w / 2, h / 2))
splash.convert("RGB").save(out / "splash.png")
print((out / "splash.png").relative_to(root))

logo = root / "brand/v2/logo/kybernow-link-dark.svg"
png = cairosvg.svg2png(url=str(logo), output_height=48 * 2)
img = Image.open(io.BytesIO(png)).convert("RGBA")
img = img.resize((round(img.width * 48 / img.height), 48), Image.LANCZOS)
assets = here.parent / "assets"
img.save(assets / "logo.png")
print((assets / "logo.png").relative_to(root), img.size)
