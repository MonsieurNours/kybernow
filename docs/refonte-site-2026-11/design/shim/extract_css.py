#!/usr/bin/env python3
"""Extrait d'une ou plusieurs maquettes .dc.html le CSS propre à la page (sans ce que site/assets/kn/base.css fournit),
et le convertit pour le vrai site : .pg.light → :root[data-theme="light"], .pg.live/.live → :root.live, .pg → :root.
    python3 extract_css.py Guide GuideArticle GuideFaq > site/assets/kn/guide.css"""
import os, re, sys
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

PATS = [r'body', r'a', r'a:hover', r'p', r'\*', r'h1,\s*h2.*', r'\.pg', r'\.pg \*', r'\.pg\.light', r'\.wrap', r'\.mono', r'\.k', r'\.disp', r'\.disp em', r'\.lead',
        r'\.btn(\.sig)?(:hover)?', r'\.btn svg', r'\.top.*', r'\.logo.*', r'\.nav.*', r'\.tools.*', r'\.thm.*', r'\.lt', r'\.dk', r'\.pg:not\(\.light\) \.lt',
        r'\.pg\.light \.(dk|lt)', r'\.pg\.light \.thm.*', r'\.burger.*', r'\.sheet.*', r'\.mi.*', r'\.mi-small', r'\.ft.*', r'\.rv', r'\.sechead h2',
        r'\.pg\.light :is\(\.nothing\)', r'\.pg\.light \.hphoto img']
BASE = re.compile('^(' + '|'.join(PATS) + ')$')

def rules(css):
    """Découpe en règles de premier niveau : (prélude, corps)."""
    out, i, n = [], 0, len(css)
    while i < n:
        j = css.find('{', i)
        if j == -1: break
        pre = css[i:j].strip()
        depth, k = 1, j + 1
        while k < n and depth:
            if css[k] == '{': depth += 1
            elif css[k] == '}': depth -= 1
            k += 1
        out.append((pre, css[j + 1:k - 1]))
        i = k
    return out

def clean_pre(pre):
    return re.sub(r'/\*.*?\*/', '', pre, flags=re.S).strip()

def keep(sel):
    parts = [p.strip() for p in sel.split(',')]
    return not all(BASE.match(p) for p in parts)

def conv(sel):
    sel = re.sub(r'\.pg\.light\.live', ':root[data-theme="light"].live', sel)
    sel = re.sub(r'\.pg\.light', ':root[data-theme="light"]', sel)
    sel = re.sub(r'\.pg\.live', ':root.live', sel)
    sel = re.sub(r'(^|[\s,(])\.live\b', r'\1:root.live', sel)
    sel = re.sub(r'\.pg:not\(\.light\)', ':root:not([data-theme="light"])', sel)
    sel = re.sub(r'(^|[\s,(])\.pg\b', r'\1:root', sel)
    return sel

def process(css, inmedia=False):
    out = []
    for pre, body in rules(css):
        pre = clean_pre(pre)
        if pre.startswith('@keyframes'):
            if 'xfade' in pre or 'miIn' in pre: continue
            out.append(f'{pre}{{{body}}}'); continue
        if pre.startswith('@media') or pre.startswith('@supports'):
            if pre.startswith('@supports (animation-timeline'): continue
            inner = process(body, True)
            if inner.strip(): out.append(f'{pre}{{\n{inner}\n}}')
            continue
        if not keep(pre): continue
        out.append(f'{conv(pre)}{{{body.strip()}}}')
    return '\n'.join(out)

css_all = []
for page in sys.argv[1:]:
    s = open(os.path.join(ROOT, 'project', page + '.dc.html'), encoding='utf-8').read()
    css = re.search(r'<style>(.*?)</style>', s, re.S).group(1)
    css_all.append(f'/* ---- {page} ---- */\n' + process(css))
print('\n'.join(css_all))
