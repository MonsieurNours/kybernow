#!/usr/bin/env python3
"""Écrit site/sitemap.xml : chaque page dans ses deux langues, avec ses alternates hreflang.
Les pages du guide viennent de docs/refonte-site-2026-11/guide/.urls.json (écrit par scripts/site-guide.py).
Une page dont la version anglaise n'existe pas encore sur le disque n'a que sa version française.
    scripts/site-sitemap.py [AAAA-MM-JJ]"""
import datetime, json, os, sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SITE = os.path.join(REPO, 'site')
BASE = 'https://www.kybernow.app'
PAGES = [('/', '/en/'), ('/jeu/', '/en/play/'), ('/pro/', '/en/pro/'), ('/telecharger/', '/en/download/'), ('/share/', '/en/share/'),
         ('/technologie/', '/en/technology/'), ('/guide/', '/en/guide/')]

def exists(path):
    return os.path.isfile(os.path.join(SITE, path.strip('/'), 'index.html'))

def main():
    day = sys.argv[1] if len(sys.argv) > 1 else datetime.date.today().isoformat()
    pairs = list(PAGES)
    try:
        urls = json.load(open(os.path.join(REPO, 'docs', 'refonte-site-2026-11', 'guide', '.urls.json')))
        for k, fr in urls['fr'].items():
            pairs.append((fr, urls['en'].get(k, '')))
    except FileNotFoundError:
        pass
    out = ['<?xml version="1.0" encoding="UTF-8"?>', '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">']
    n = 0
    for fr, en in pairs:
        if not exists(fr): continue
        en = en if en and exists(en) else ''
        alts = [f'    <xhtml:link rel="alternate" hreflang="fr" href="{BASE}{fr}"/>']
        if en:
            alts += [f'    <xhtml:link rel="alternate" hreflang="en" href="{BASE}{en}"/>', f'    <xhtml:link rel="alternate" hreflang="x-default" href="{BASE}{en}"/>']
        for loc in [fr] + ([en] if en else []):
            out += ['  <url>', f'    <loc>{BASE}{loc}</loc>', f'    <lastmod>{day}</lastmod>'] + alts + ['  </url>']; n += 1
    out.append('</urlset>')
    open(os.path.join(SITE, 'sitemap.xml'), 'w', encoding='utf-8').write('\n'.join(out) + '\n')
    print('sitemap.xml :', n, 'adresses')

if __name__ == '__main__':
    main()
