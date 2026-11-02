#!/usr/bin/env python3
"""Mires de l'écran « Réglage du HDR » (docs/hdr.md, « Réglage du HDR ») pour
l'app webOS : une image fixe HEVC Main 10 PQ (BT.2020, plage limitée,
métadonnées HDR10 dans le flux) par palier de luminosité de chaque étape,
réunies dans apps/webos/assets/hdr-mires.bin, que l'app embarque.

Le plan graphique du téléviseur est en SDR : les mires passent par sa chaîne
vidéo (NDL), qui les affiche en HDR comme l'image d'une session. Toutes sont
en gris neutre : la luma est la valeur PQ de la luminance, la chroma 512.

    brew install ffmpeg          (ffmpeg avec libx265)
    pip install numpy pillow
    python3 apps/webos/scripts/hdr-mires.py

Format du fichier (petit-boutiste) : « KNHM », u32 version (1), u32 nombre,
puis pour chaque image u32 étape (0 luminosité maximale, 1 plein écran, 2 noir,
3 blanc du bureau), u32 valeur en millièmes de nit, u32 début, u32 longueur
(depuis le début du fichier) ; puis les images (Annex B).
"""
import struct
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "apps/webos/assets/hdr-mires.bin"
FONT = ROOT / "apps/webos/assets/fonts/archivo-regular.ttf"
FONT_BOLD = ROOT / "apps/webos/assets/fonts/archivo-semibold.ttf"
W, H = 1920, 1080

# Les paliers de chaque étape (nits), dans les bornes du cœur : maximum 100 à
# 10 000, noir 0 à 1, blanc 80 à 480. L'app reprend ces listes du fichier.
PEAK = [100, 150, 200, 250, 300, 350, 400, 450, 500, 600, 700, 800, 900, 1000, 1100, 1200, 1300, 1400, 1500, 1600,
        1800, 2000, 2500, 3000, 4000, 5000, 10000]
BLACK = [0, 0.005, 0.01, 0.02, 0.03, 0.05, 0.075, 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 0.75, 1.0]
WHITE = list(range(80, 481, 20))
CLIP = 10000.0


def pq(nits):
    """Signal PQ (SMPTE ST 2084) de 0 à 1."""
    m1, m2 = 2610 / 16384, 2523 / 4096 * 128
    c1, c2, c3 = 3424 / 4096, 2413 / 4096 * 32, 2392 / 4096 * 32
    y = np.clip(np.asarray(nits, dtype=np.float64) / 10000.0, 0.0, 1.0)
    p = np.power(y, m1)
    return np.power((c1 + c2 * p) / (1 + c3 * p), m2)


def luma(nits):
    """Luma 10 bits en plage limitée (64 à 940) d'une luminance en nits."""
    return np.round(64 + 876 * pq(nits)).astype(np.uint16)


def cross(img, cx, cy, arm, width, value):
    """Une croix centrée (le symbole des mires)."""
    img[cy - width // 2:cy + width // 2, cx - arm:cx + arm] = value
    img[cy - arm:cy + arm, cx - width // 2:cx + width // 2] = value


def peak(symbol):
    """Étape 1 : une zone de 10 % de l'écran au maximum (10 000 nits, écrêtée par
    l'écran) sur du noir, une croix à `symbol` nits dedans."""
    img = np.zeros((H, W), dtype=np.float64)
    side = 456
    x0, y0 = (W - side) // 2, (H - side) // 2
    img[y0:y0 + side, x0:x0 + side] = CLIP
    cross(img, W // 2, H // 2, 120, 36, symbol)
    return img


def full(symbol):
    """Étape 2 : tout l'écran au maximum, la même croix, plus grande."""
    img = np.full((H, W), CLIP, dtype=np.float64)
    cross(img, W // 2, H // 2, 200, 60, symbol)
    return img


def black(symbol):
    """Étape 3 : du noir, une croix à `symbol` nits."""
    img = np.zeros((H, W), dtype=np.float64)
    cross(img, W // 2, H // 2, 200, 60, symbol)
    return img


def desktop_sdr():
    """Étape 4 : un bureau en SDR (0 à 1) : fond, une fenêtre avec sa barre de
    titre et du texte, sans langue (l'app dit l'étape dans la sienne)."""
    im = Image.new("L", (W, H), 64)
    d = ImageDraw.Draw(im)
    x0, y0, x1, y1 = 360, 190, 1560, 890
    d.rectangle((x0 + 8, y0 + 10, x1 + 8, y1 + 10), fill=40)
    d.rectangle((x0, y0, x1, y1), fill=255)
    d.rectangle((x0, y0, x1, y0 + 56), fill=232)
    title = ImageFont.truetype(str(FONT_BOLD), 26)
    body = ImageFont.truetype(str(FONT), 30)
    d.text((x0 + 28, y0 + 13), "KyberNow", font=title, fill=30)
    lines = ["Aa Bb Cc Dd Ee Ff Gg Hh Ii Jj Kk Ll Mm", "Nn Oo Pp Qq Rr Ss Tt Uu Vv Ww Xx Yy Zz",
             "0123456789  ·  @ # % & ( ) [ ] { } ? !", "", "Aa Bb Cc Dd Ee Ff Gg Hh Ii Jj Kk Ll Mm"]
    for i, line in enumerate(lines):
        d.text((x0 + 60, y0 + 110 + i * 58), line, font=body, fill=20)
    for i in range(4):
        d.rectangle((x0 + 60, y0 + 440 + i * 50, x0 + 60 + [900, 760, 980, 520][i], y0 + 462 + i * 50), fill=170)
    d.rectangle((x1 - 260, y1 - 90, x1 - 60, y1 - 36), fill=96)
    return np.asarray(im, dtype=np.float64) / 255.0


def white(level, sdr):
    """Le bureau, son blanc à `level` nits (gamma 2.2)."""
    return level * np.power(sdr, 2.2)


def encode(nits, workdir):
    """Une image fixe HEVC Main 10 PQ, BT.2020, plage limitée, avec ses SEI HDR10."""
    y = luma(nits)
    uv = np.full((H // 2, W // 2), 512, dtype=np.uint16)
    raw = workdir / "in.yuv"
    out = workdir / "out.hevc"
    raw.write_bytes(y.astype("<u2").tobytes() + uv.astype("<u2").tobytes() * 2)
    # ffmpeg et libx265 : une image seule, image clé, en vrai Main 10. Avec keyint=1 (ou le x265 en ligne de
    # commande), x265 signale « Main 10 Intra », un profil RExt que les décodeurs des téléviseurs peuvent refuser.
    params = ":".join([
        "log-level=error", "crf=10", "repeat-headers=1", "aud=1", "info=0",
        "colorprim=bt2020", "transfer=smpte2084", "colormatrix=bt2020nc", "range=limited",
        "master-display=G(8500,39850)B(6550,2300)R(35400,14600)WP(15635,16450)L(100000000,0)",
        "max-cll=10000,10000", "hdr10=1",
    ])
    subprocess.run([
        "ffmpeg", "-v", "error", "-f", "rawvideo", "-pix_fmt", "yuv420p10le", "-s", f"{W}x{H}", "-r", "60",
        "-i", str(raw), "-frames:v", "1", "-c:v", "libx265", "-preset", "slow", "-profile:v", "main10",
        "-x265-params", params, "-f", "hevc", "-y", str(out),
    ], check=True)
    return out.read_bytes()


def main():
    sdr = desktop_sdr()
    steps = [(0, PEAK, peak), (1, PEAK, full), (2, BLACK, black), (3, WHITE, lambda v: white(v, sdr))]
    entries, blobs = [], []
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        for step, values, make in steps:
            for v in values:
                blobs.append(encode(make(v), work))
                entries.append((step, int(round(v * 1000))))
    header = 12 + 16 * len(entries)
    out = bytearray(b"KNHM" + struct.pack("<II", 1, len(entries)))
    offset = header
    for (step, milli), blob in zip(entries, blobs):
        out += struct.pack("<IIII", step, milli, offset, len(blob))
        offset += len(blob)
    for blob in blobs:
        out += blob
    OUT.write_bytes(bytes(out))
    print(f"{OUT} : {len(entries)} mires, {len(out) // 1024} Ko")


if __name__ == "__main__":
    main()
