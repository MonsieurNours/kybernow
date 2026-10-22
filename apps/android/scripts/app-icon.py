#!/usr/bin/env python3
"""Icône et bannière de l'app Android, dessinées comme l'icône de l'Apple TV :
l'encre, une lueur Spectre et la couture en filigrane, puis le symbole
(l'ordinateur en givre, vous en Spectre) et son néon. Le dessin est celui
d'apps/tvos/scripts/app-icon.py (géométrie de brand/v2), repris tel quel :
seule sa partie « Dessin » est chargée, pas son catalogue Xcode.

  - drawable-xhdpi/tv_banner.png : bannière du lanceur d'Android TV (320 × 180 dp) ;
  - mipmap-xxxhdpi/ic_launcher_background.png et _foreground.png : icône
    adaptative (108 dp ; le symbole tient dans la zone sûre de 66 dp).

    python3 apps/android/scripts/app-icon.py     (Pillow requis : pip install pillow)
"""
import pathlib

here = pathlib.Path(__file__).resolve().parent
tvos = here.parents[1] / "tvos/scripts/app-icon.py"
source = tvos.read_text(encoding="utf-8").split("# --- Catalogue")[0]
draw = {"__file__": str(tvos)}
exec(compile(source, str(tvos), "exec"), draw)
back, mark = draw["back"], draw["mark"]
Image = draw["Image"]

res = here.parent / "app/src/main/res"

def save(img, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path)
    print(path.relative_to(res))

# Bannière : 320 × 180 dp, en xhdpi.
w, h = 640, 360
save(Image.alpha_composite(back(w, h), mark(w, h, h * 0.56, w / 2, h / 2)).convert("RGB"), res / "drawable-xhdpi/tv_banner.png")

# Icône adaptative : 108 dp en xxxhdpi (432 px). Le symbole et son néon
# tiennent dans le cercle de 66 dp au centre, quelle que soit la découpe.
s = 432
save(back(s, s).convert("RGB"), res / "mipmap-xxxhdpi/ic_launcher_background.png")
save(mark(s, s, s * 0.40, s / 2, s / 2), res / "mipmap-xxxhdpi/ic_launcher_foreground.png")
