#!/usr/bin/env python3
"""Dessins vectoriels de l'app Android depuis la charte (brand/v2), sans les
retoucher : logo signé KyberNow Link, symbole, premier plan de l'icône
adaptative et bannière du lanceur d'Android TV.

Les SVG de la charte n'emploient que des groupes `translate(x y) scale(s)`,
des tracés `d` et des remplissages : ils passent tels quels en
`VectorDrawable` (groupes avec translateX/Y et scaleX/Y, tracés avec arcs).

    python3 apps/android/scripts/brand-drawables.py
"""

import re
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BRAND = ROOT / "brand/v2"
OUT = ROOT / "apps/android/app/src/main/res/drawable"
SVG = "{http://www.w3.org/2000/svg}"

INK = "#05070A"
SPECTRE = "#9A8CFF"


def transform(t):
    """translate(x y) scale(s) → attributs d'un groupe."""
    attrs = {}
    m = re.search(r"translate\(([-\d.]+)[ ,]+([-\d.]+)\)", t or "")
    if m:
        attrs["android:translateX"] = m.group(1)
        attrs["android:translateY"] = m.group(2)
    m = re.search(r"scale\(([-\d.]+)\)", t or "")
    if m:
        attrs["android:scaleX"] = m.group(1)
        attrs["android:scaleY"] = m.group(1)
    return attrs


def convert(node, fill, indent):
    """Un élément SVG (g ou path) en XML de VectorDrawable."""
    pad = "    " * indent
    tag = node.tag.replace(SVG, "")
    fill = node.get("fill", fill)
    if tag == "path":
        if not fill or fill == "none" or fill.startswith("url("):
            return ""
        return f'{pad}<path android:fillColor="{fill}" android:pathData="{node.get("d")}" />\n'
    if tag == "g":
        attrs = transform(node.get("transform"))
        inner = "".join(convert(c, fill, indent + 1) for c in node)
        a = "".join(f' {k}="{v}"' for k, v in attrs.items())
        return f"{pad}<group{a}>\n{inner}{pad}</group>\n"
    return ""


def body(svg_path, fill=None, indent=1):
    root = ET.parse(svg_path).getroot()
    return "".join(convert(c, fill, indent) for c in root), [float(v) for v in root.get("viewBox").split()]


def vector(width_dp, height_dp, vw, vh, inner, comment):
    return (
        f"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- {comment} Généré par apps/android/scripts/brand-drawables.py : ne pas retoucher. -->\n"
        f'<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
        f'    android:width="{width_dp}dp" android:height="{height_dp}dp"\n'
        f'    android:viewportWidth="{vw}" android:viewportHeight="{vh}">\n{inner}</vector>\n'
    )


def write(name, text):
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / name).write_text(text, encoding="utf-8")
    print(f"{name} ({len(text)} octets)")


def main():
    # Logo signé, fond sombre.
    inner, (_, _, w, h) = body(BRAND / "logo/kybernow-link-dark.svg")
    write("kn_logo_link.xml", vector(round(w / 4, 1), round(h / 4, 1), w, h, inner, "Logo KyberNow Link (charte v2)."))
    # Logo sans signature (petites tailles).
    inner, (_, _, w, h) = body(BRAND / "logo/kybernow-logo-dark.svg")
    write("kn_logo.xml", vector(round(w / 4, 1), round(h / 4, 1), w, h, inner, "Logo KyberNow (charte v2)."))
    # Symbole « Relais », état now.
    inner, (_, _, w, h) = body(BRAND / "mark/kybernow-mark-now-dark.svg")
    write("kn_mark.xml", vector(48, 48, w, h, inner, "Symbole Relais, état now (charte v2)."))

    # Icône adaptative : le symbole dans la zone sûre (66 sur 108), la couture
    # en néon par-dessus, comme l'icône de l'app Mac (sans le flou).
    mark, _ = body(BRAND / "mark/kybernow-mark-now-dark.svg", indent=2)
    s = 52 / 48
    fg = (
        f'    <group android:translateX="{(108 - 52) / 2}" android:translateY="{(108 - 52) / 2}" android:scaleX="{s:.4f}" android:scaleY="{s:.4f}">\n'
        f"{mark}"
        f'        <path android:strokeColor="{SPECTRE}" android:strokeWidth="2.6" android:strokeLineCap="round" android:pathData="M5.6,42.4 L42.4,5.6" />\n'
        f'        <path android:strokeColor="#F4F1FF" android:strokeWidth="0.9" android:strokeLineCap="round" android:pathData="M5.6,42.4 L42.4,5.6" />\n'
        f"    </group>\n"
    )
    write("ic_launcher_foreground.xml", vector(108, 108, 108, 108, fg, "Premier plan de l'icône adaptative."))

    # Bannière d'Android TV (320 × 180) : encre, logo signé au centre.
    logo, (_, _, lw, lh) = body(BRAND / "logo/kybernow-link-dark.svg", indent=2)
    scale = 240 / lw
    banner = (
        f'    <path android:fillColor="{INK}" android:pathData="M0,0 L320,0 L320,180 L0,180 Z" />\n'
        f'    <group android:translateX="{(320 - 240) / 2}" android:translateY="{(180 - lh * scale) / 2:.2f}" android:scaleX="{scale:.5f}" android:scaleY="{scale:.5f}">\n'
        f"{logo}    </group>\n"
    )
    write("tv_banner.xml", vector(320, 180, 320, 180, banner, "Bannière du lanceur d'Android TV."))


if __name__ == "__main__":
    main()
