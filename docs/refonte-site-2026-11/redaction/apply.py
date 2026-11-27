#!/usr/bin/env python3
"""Applique copy/<Page>.<etape>.json à project/<Page>.dc.html : chaque `old` doit apparaître une seule fois.
Usage : apply.py ETAPE [--dry]"""
import json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
PROJ = os.path.join(os.path.dirname(HERE), 'project')
step = sys.argv[1]
dry = '--dry' in sys.argv
total = 0
for f in sorted(os.listdir(HERE)):
    m = re.match(r'(\w+)\.' + re.escape(step) + r'\.json$', f)
    if not m:
        continue
    page = os.path.join(PROJ, m.group(1) + '.dc.html')
    src = open(page).read()
    items = json.load(open(os.path.join(HERE, f)))
    ok = skip = 0
    for it in items:
        old, new = it['old'], it['new']
        n = src.count(old)
        if n != 1:
            print(f'  ! {f}: {n} occurrence(s) : {old[:70]!r}')
            skip += 1
            continue
        i = src.index(old)
        # Dans le script : une apostrophe droite dans une chaîne entre apostrophes casserait le JS.
        script_start = src.find('data-dc-script')
        if script_start != -1 and i > script_start:
            line = src[src.rfind('\n', 0, i) + 1: src.find('\n', i)]
            q = line[:line.find(old) if old in line else 0]
            if q.count("'") % 2 == 1 and "'" in new.replace("\\'", ''):
                print(f'  ! {f}: apostrophe droite dans une chaîne JS : {new[:70]!r}')
                skip += 1
                continue
        src = src.replace(old, new, 1)
        ok += 1
    print(f'{m.group(1)}: {ok} appliqué(s), {skip} écarté(s)')
    total += ok
    if not dry:
        open(page, 'w').write(src)
print('total', total)
