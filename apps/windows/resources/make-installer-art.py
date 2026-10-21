#!/usr/bin/env python3
"""Images de l'installateur et des notifications, depuis la géométrie du
symbole (même dessin que make-ico.py, état « now ») :

- kybernow.png (256 px) : icône des notifications de Windows ;
- welcome.bmp (164 × 314) : bandeau des pages d'accueil et de fin ;
- header.bmp (150 × 57) : en-tête des autres pages.

Usage : python3 make-installer-art.py  (Pillow requis)
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

import importlib.util

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("ico", HERE / "make-ico.py")
ico = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ico)

INK = (5, 7, 10)


def mark(size):
    """Symbole « now » avec sa couture allumée, sur fond transparent."""
    ss = 8
    big = size * ss
    s = big / 48
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.polygon(ico.polygon(ico.SRC, s), fill=ico.FROST + (255,))
    d.polygon(ico.polygon(ico.DST, s), fill=ico.SIGNAL + (255,))
    a, b = (5.6 * s, 42.4 * s), (42.4 * s, 5.6 * s)
    glow = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(glow).line([a, b], fill=ico.SIGNAL + (220,), width=int(7 * s))
    glow = glow.filter(ImageFilter.GaussianBlur(2.6 * s))
    core = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(core).line([a, b], fill=ico.NEON + (255,), width=max(1, int(1.2 * s)))
    for part in (glow, core):
        img = Image.alpha_composite(img, part)
    return img.resize((size, size), Image.LANCZOS)


def panel(w, h, mark_size, at):
    bg = Image.new("RGBA", (w, h), INK + (255,))
    halo = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    r = max(w, h) * 0.5
    cx, cy = at[0] + mark_size / 2, at[1] + mark_size / 2
    ImageDraw.Draw(halo).ellipse((cx - r, cy - r, cx + r, cy + r), fill=(109, 91, 255, 70))
    bg = Image.alpha_composite(bg, halo.filter(ImageFilter.GaussianBlur(r * 0.5)))
    bg.alpha_composite(mark(mark_size), at)
    return bg.convert("RGB")


def main():
    ico.tile(256).save(HERE / "kybernow.png")
    panel(164, 314, 92, (36, 70)).save(HERE / "welcome.bmp")
    panel(150, 57, 36, (100, 10)).save(HERE / "header.bmp")
    print("kybernow.png, welcome.bmp, header.bmp")


if __name__ == "__main__":
    main()
