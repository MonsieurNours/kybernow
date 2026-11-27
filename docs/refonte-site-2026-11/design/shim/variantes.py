#!/usr/bin/env python3
"""Régénère, à partir des pages sombres (qui ont déjà le thème clair intégré) :
- <Page>Clair.dc.html : même page, thème clair par défaut, liens vers les autres pages claires ;
- <Page>Mobile.dc.html : même page, pour une planche de 390 px de large sur le canevas.
    python3 shim/variantes.py Main Jeu Pro Telecharger Guide GuideArticle GuideFaq"""
import os, re, sys
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PAGES = ['Main', 'Jeu', 'Pro', 'Telecharger', 'Guide', 'GuideArticle', 'GuideFaq']

def relink(s, suffix):
    for p in sorted(PAGES, key=len, reverse=True):
        s = re.sub(r'href="' + p + r'\.dc\.html', f'href="{p}{suffix}.dc.html', s)
    return s

def run(page):
    src = open(os.path.join(ROOT, 'project', page + '.dc.html'), encoding='utf-8').read()
    if "this.props.theme || 'dark'" in src:
        c = src.replace("this.props.theme || 'dark'", "this.props.theme || 'light'").replace('"default":"dark"}', '"default":"light"}')
        c = relink(c, 'Clair')
        c = re.sub(r'<title>([^<]*)</title>', r'<title>\1, clair</title>', c, count=1)
        open(os.path.join(ROOT, 'project', page + 'Clair.dc.html'), 'w', encoding='utf-8').write(c)
    m = relink(src, 'Mobile')
    m = re.sub(r'<title>([^<]*)</title>', r'<title>\1, mobile</title>', m, count=1)
    m = re.sub(r'"\$preview":\{"width":\d+,"height":\d+\}', '"$preview":{"width":390,"height":844}', m)
    open(os.path.join(ROOT, 'project', page + 'Mobile.dc.html'), 'w', encoding='utf-8').write(m)
    print(page, 'clair' if "this.props.theme" in src else '(pas de thème clair)', '+ mobile')

for p in sys.argv[1:] or PAGES:
    run(p)
