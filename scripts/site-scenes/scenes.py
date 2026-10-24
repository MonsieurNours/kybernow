#!/usr/bin/env python3
"""Refait les photos de scènes du site (accueil) : génération des pièces avec un écran vert
(Z-Image Turbo sur ComfyUI), pose des vraies captures de KyberNow Link sur l'écran, export WebP.

    scripts/site-scenes/scenes.py CAPTURES [--work DOSSIER] [--only NOM] [--regen]

CAPTURES : dossier des captures de l'app Linux, rendues en français et en anglais par
    cargo run -p kybernow-linux --no-default-features -- --preview CAPTURES/fr fr
    cargo run -p kybernow-linux --no-default-features -- --preview CAPTURES/en en
(depuis apps/). Serveur ComfyUI : $COMFY (http://192.168.1.100:8188 par défaut), avec
z_image_turbo_int8_convrot, qwen_3_4b_fp8_mixed et ae (réglages : zimage.py).
Il faut aussi Pillow, numpy et opencv-python.

Les pièces générées restent dans --work (par défaut ~/kybernow-scenes) : relancer ne les
refait pas, sauf --regen. Les graines sont fixes : mêmes images avec le même modèle.
"""
import argparse
import os
import subprocess
import sys
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
SITE = HERE.parent.parent / "site" / "assets" / "img"
STYLE = ("photorealistic interior photograph, warm evening light from table lamps and a floor lamp, cozy lived-in home, "
         "natural skin tones, 35mm photograph, shallow depth of field, soft shadows, realistic, candid moment")
GREEN_TV = ("the television screen displays a uniform flat bright chroma key green color (pure #00FF00) filling the entire "
            "screen edge to edge")
GREEN = "displays a uniform flat bright chroma key green color (pure #00FF00) filling the entire screen"

# nom : (prompt, graine, taille)
SCENES = {
    "salonduo": ("A man in his mid forties with short greying hair and a short beard and a woman around forty with shoulder-length "
                 "brown hair, sitting side by side on a fabric sofa, seen from behind and slightly from the side, both holding black "
                 "wireless game controllers and playing a video game together, the woman laughing, bowl of popcorn and two glasses on a "
                 f"low coffee table, a large flat television mounted on the wall in front of them, {GREEN_TV}, wooden media console "
                 f"under the TV with a small white box, plants, bookshelf, {STYLE}", 23, (1536, 864)),
    "salonamis": ("Four adult friends having a game night in a living room, a man about fifty with grey hair and glasses, a woman in "
                  "her mid forties with a blond bob haircut, a bald man in his late thirties with a beard, and a woman around forty "
                  "with long dark hair, sitting on a large sofa, seen from behind and three-quarter view, two of them holding game "
                  "controllers and playing a video game together, the others laughing and cheering, snacks and drinks on the coffee "
                  f"table, a large flat television on a media console in front of them, {GREEN_TV}, {STYLE}", 74, (1536, 864)),
    "canape": ("A woman around forty with dark curly hair wearing a cozy knit sweater, curled up on a sofa under a blanket, seen from "
               "behind over her shoulder in three-quarter view, holding a Steam Deck handheld game console with both hands, the console "
               f"screen {GREEN}, a man in his forties reading a book next to her at the other end of the sofa, cup of tea on a side "
               f"table, {STYLE}", 41, (1536, 864)),
    "bureau": ("A man around fifty with glasses and salt-and-pepper hair sitting at a wooden desk in a home office in the evening, seen "
               f"from behind over his shoulder in three-quarter view, working on an open silver laptop, the laptop screen {GREEN}, "
               f"desk lamp, notebook and coffee mug, bookshelf, window with dusk outside, {STYLE}", 51, (1536, 864)),
    "hotel": ("A woman in her mid thirties sitting at a small desk in a hotel room at night, seen from behind over her shoulder in "
              f"three-quarter view, an open laptop in front of her, the laptop screen {GREEN}, a wireless game controller next to the "
              f"laptop, suitcase in the corner, city lights through the window, warm bedside lamp, {STYLE}", 61, (1536, 864)),
    "pisalon": ("Photorealistic interior photograph of a cozy living room in the evening, warm lamp light, a woman in her mid forties "
                "with short grey-streaked hair, seen from behind and slightly from the side, sitting on a sofa with a TV remote in her "
                f"hand, a large flat television mounted on the wall in front of her, {GREEN_TV}, on the wooden media console under the "
                "television a tiny Raspberry Pi computer in a small red and white plastic case with a cable going to the TV, plants, "
                "books, soft shadows, 35mm photograph, shallow depth of field, realistic lived-in home", 102, (1536, 864)),
    # Les jeux montrés sur les écrans « en partie » : génériques, inventés.
    "jeucoop": ("Screenshot of a colorful cooperative video game for two players, two cartoon adventurers exploring a lush fantasy "
                "valley with waterfalls and floating islands at golden hour, simple heads-up display with two player health bars at "
                "the top, vibrant stylized 3D graphics, wide 16:9 game screenshot, no text", 81, (1920, 1088)),
    "jeuaventure": ("Screenshot of a modern open world adventure video game, a lone explorer on horseback on a cliff overlooking a "
                    "misty valley at sunrise, cinematic lighting, realistic graphics, minimal heads-up display with a small compass at "
                    "the top, wide game screenshot, no text", 92, (1920, 1216)),
}

# sortie : (scène, capture ou jeu, langue ou None, options de composite.py, dossier)
OUTPUTS = [
    ("scenes/salon-duo-{lang}", "salonduo", "salon-01-accueil", True, [], "scenes"),
    ("scenes/salon-amis", "salonamis", "@jeucoop", False, [], "scenes"),
    ("scenes/canape-{lang}", "canape", "deck-01-accueil", True, ["--glare", "0.04"], "scenes"),
    ("scenes/bureau-{lang}", "bureau", "bureau-01-accueil", True, ["--glare", "0.04"], "scenes"),
    ("scenes/hotel", "hotel", "@jeuaventure", False, ["--glare", "0.04"], "scenes"),
    ("pi/link-os-salon-{lang}", "pisalon", "os-10b-accueil-a-jour", True, [], "pi"),
]


def crop_to(path, out, w, h):
    im = Image.open(path)
    y = (im.height - h) // 2
    im.crop((0, y, w, y + h)).save(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("captures")
    ap.add_argument("--work", default=os.path.expanduser("~/kybernow-scenes"))
    ap.add_argument("--only")
    ap.add_argument("--regen", action="store_true")
    a = ap.parse_args()
    work = Path(a.work); work.mkdir(parents=True, exist_ok=True)
    caps = Path(a.captures)

    for name, (prompt, seed, (w, h)) in SCENES.items():
        f = work / f"{name}-{seed}.png"
        if a.regen or not f.exists():
            subprocess.run([sys.executable, str(HERE / "zimage.py"), name, prompt, "--w", str(w), "--h", str(h), "--seed", str(seed)],
                           cwd=work, check=True)
    crop_to(work / "jeucoop-81.png", work / "jeucoop-16x9.png", 1920, 1080)
    crop_to(work / "jeuaventure-92.png", work / "jeuaventure-16x10.png", 1920, 1200)

    for out, scene, capture, per_lang, opts, _ in OUTPUTS:
        if a.only and a.only not in out:
            continue
        seed = SCENES[scene][1]
        for lang in (["fr", "en"] if per_lang else [None]):
            src = work / (f"{capture[1:]}-16x9.png" if capture == "@jeucoop" else f"{capture[1:]}-16x10.png") if capture.startswith("@") \
                else caps / lang / f"{capture}.png"
            png = work / (out.replace("/", "-").format(lang=lang) + ".png")
            subprocess.run([sys.executable, str(HERE / "composite.py"), str(work / f"{scene}-{seed}.png"), str(src), str(png), *opts], check=True)
            im = Image.open(png).convert("RGB")
            dst = SITE / out.format(lang=lang)
            dst.parent.mkdir(parents=True, exist_ok=True)
            im.save(f"{dst}.webp", "WEBP", quality=80, method=6)
            im.resize((960, round(im.height * 960 / im.width)), Image.LANCZOS).save(f"{dst}-960.webp", "WEBP", quality=80, method=6)
            print(f"{dst}.webp et -960.webp")


if __name__ == "__main__":
    main()
