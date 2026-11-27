#!/usr/bin/env python3
"""Verse les articles Markdown du guide (docs/refonte-site-2026-11/guide/) dans les maquettes :
Guide.dc.html (sommaire), GuideArticle.dc.html (un article, slug en argument), GuideFaq.dc.html (FAQ).
    python3 shim/guide_build.py [slug]"""
import html, json, os, re, sys, unicodedata

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GUIDE = os.path.join(os.path.dirname(ROOT), 'guide')
PROJ = os.path.join(ROOT, 'project')
RUBS = [('Commencer', 'M12 3v18M3 12h18'), ('Installer la Station', 'M4 5h16v11H4zM2 19h20'),
        ('Connecter un appareil', 'M5 3h14v18H5zM11 18h2'), ('Pendant la session', 'M4 6h16M4 12h16M4 18h10'),
        ('Consoles', 'M6 9h12a4 4 0 0 1 4 4v1a3 3 0 0 1-5 2l-1-1H8l-1 1a3 3 0 0 1-5-2v-1a4 4 0 0 1 4-4z'),
        ('Partager', 'M8 12a3 3 0 1 0 0-.01M17 6a3 3 0 1 0 0-.01M17 18a3 3 0 1 0 0-.01M10.5 10.5l4-3M10.5 13.5l4 3'),
        ('Comptes et équipes', 'M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM2 21c0-4 3-6 7-6s7 2 7 6M17 11a3 3 0 1 0 0-6M22 21c0-3-2-5-5-5.5'),
        ('Hors de chez vous', 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9s1.3-6.4 3.8-9z'),
        ('Entretien', 'M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18v3h3l6.3-6.3a4 4 0 0 0 5.4-5.4l-2.6 2.6-2.4-.6-.6-2.4z')]

def front(path):
    t = open(path, encoding='utf-8').read()
    m = re.match(r'---\n(.*?)\n---\n(.*)', t, re.S)
    meta, body = {}, t
    if m:
        body = m.group(2)
        for line in m.group(1).splitlines():
            mm = re.match(r'(\w+):\s*(.*?)\s*(#.*)?$', line)
            if not mm: continue
            k, v = mm.group(1), mm.group(2).strip()
            if v.startswith('[') and v.endswith(']'):
                v = [x.strip().strip('"\'') for x in v[1:-1].split(',') if x.strip()]
            else:
                v = v.strip('"\'')
            meta[k] = v
    return meta, body

def slugify(x):
    x = unicodedata.normalize('NFD', x).encode('ascii', 'ignore').decode().lower()
    return re.sub(r'[^a-z0-9]+', '-', x).strip('-')

def inline(t):
    t = html.escape(t, quote=False)
    t = re.sub(r'`([^`]+)`', r'<code>\1</code>', t)
    t = re.sub(r'\*\*([^*]+)\*\*', r'<strong>\1</strong>', t)
    t = re.sub(r'(?<![*\w])\*([^*\n]+)\*(?!\w)', r'<em>\1</em>', t)
    def link(m):
        txt, url = m.group(1), m.group(2)
        if url.startswith('/guide/'): url = 'GuideArticle.dc.html'
        return f'<a href="{html.escape(url)}">{txt}</a>'
    t = re.sub(r'\[([^\]]+)\]\(([^)]+)\)', link, t)
    return t.replace('{{', '{ {').replace('}}', '} }')

def md(body):
    body = re.sub(r'<!--.*?-->', '', body, flags=re.S)
    out, toc, chapo, para = [], [], None, []
    lines = body.splitlines(); i = 0
    def flush():
        nonlocal para, chapo
        if para:
            txt = inline(' '.join(para))
            if chapo is None and not toc and not out: chapo = txt
            else: out.append(f'<p>{txt}</p>')
            para = []
    while i < len(lines):
        l = lines[i].rstrip()
        if not l.strip(): flush(); i += 1; continue
        if l.startswith('## '):
            flush(); h = l[3:].strip(); hid = slugify(h)[:48]; toc.append((hid, h))
            out.append(f'<h2 id="{hid}">{inline(h)}</h2>'); i += 1; continue
        if l.startswith('### '): flush(); out.append(f'<h3>{inline(l[4:].strip())}</h3>'); i += 1; continue
        if l.startswith('# '): flush(); i += 1; continue
        if l.startswith('>'):
            flush(); q = []
            while i < len(lines) and lines[i].startswith('>'):
                q.append(lines[i].lstrip('> ').strip()); i += 1
            out.append('<blockquote><p>' + inline(' '.join(x for x in q if x)) + '</p></blockquote>'); continue
        m = re.match(r'(\d+)\.\s+(.*)', l)
        if m or re.match(r'[-*]\s+', l):
            flush(); ordered = bool(m); items = []
            while i < len(lines):
                ll = lines[i].rstrip()
                mo = re.match(r'(\d+)\.\s+(.*)', ll) if ordered else re.match(r'[-*]\s+(.*)', ll)
                if mo: items.append(mo.group(2) if ordered else mo.group(1)); i += 1
                elif ll.startswith('   ') and items and ll.strip(): items[-1] += ' ' + ll.strip(); i += 1
                else: break
            tag = 'ol' if ordered else 'ul'
            out.append(f'<{tag}>' + ''.join(f'<li>{inline(x)}</li>' for x in items) + f'</{tag}>'); continue
        para.append(l.strip()); i += 1
    flush()
    return chapo or '', '\n'.join(out), toc

def load():
    arts = {}
    for f in sorted(os.listdir(GUIDE)):
        if not f.endswith('.md') or f == 'BRIEF.md': continue
        meta, body = front(os.path.join(GUIDE, f))
        meta.setdefault('slug', f[:-3]); arts[meta['slug']] = (meta, body)
    return arts

def jsstr(x): return json.dumps(x, ensure_ascii=False)
def unlink(t): return re.sub(r'\[([^\]]+)\]\([^)]+\)', r'\1', t)

def build_hub(arts):
    p = os.path.join(PROJ, 'Guide.dc.html'); s = open(p, encoding='utf-8').read()
    acc = arts.get('guide') or arts.get('_accueil')
    T = {'heroTitle': 'Le guide de KyberNow', 'heroLead': '', 'rubTitle': 'Tous les tutos', 'faqTitle': 'Les questions qu’on nous pose', 'helpTitle': 'Vous ne trouvez pas ?', 'helpText': ''}
    intros = {}
    if acc:
        meta, body = acc
        T['heroTitle'] = meta.get('titre', T['heroTitle'])
        paras = [x.strip() for x in re.split(r'\n\s*\n', re.sub(r'<!--.*?-->', '', body, flags=re.S)) if x.strip()]
        if paras and not paras[0].startswith('#'): T['heroLead'] = unlink(re.sub(r'\s+', ' ', paras[0]))
        for m in re.finditer(r'^##\s+(.+?)\n+((?:(?!^#).+\n?)+)', body, re.M):
            head = m.group(1).strip(); text = unlink(re.sub(r'\s+', ' ', m.group(2)).strip())
            if 'trouvez' in head.lower(): T['helpTitle'] = head; T['helpText'] = text
            else: intros[head] = text
    R = []
    for name, icon in RUBS:
        items = [{'titre': m.get('titre', sl), 'duree': m.get('duree', ''), 'mots': sl.replace('-', ' ') + ' ' + m.get('description', '')}
                 for sl, (m, b) in arts.items() if m.get('rubrique', '').strip().lower() == name.lower()]
        if not items: continue
        intro = next((v for k, v in intros.items() if name.lower() in k.lower() or k.lower() in name.lower()), '')
        R.append({'name': name, 'icon': icon, 'intro': intro, 'items': items})
    F = []
    if 'faq' in arts:
        for m in re.finditer(r'^##\s+(.+?)\n+((?:(?!^#).*\n?)+)', arts['faq'][1], re.M):
            a = re.sub(r'\s+', ' ', unlink(m.group(2))).replace('**', '').strip()
            if a: F.append({'q': m.group(1).strip(), 'a': a})
    block = f"    const T = {jsstr(T)};\n    const R = {jsstr(R)};\n    const F = {jsstr(F)};\n"
    s = re.sub(r'(    /\* CONTENU \*/\n).*?(    /\* FIN DU CONTENU \*/)', lambda m: m.group(1) + block + m.group(2), s, flags=re.S)
    open(p, 'w', encoding='utf-8').write(s)
    print('Guide :', len(R), 'rubriques,', sum(len(r['items']) for r in R), 'articles,', len(F), 'questions')

def build_article(arts, slug):
    p = os.path.join(PROJ, 'GuideArticle.dc.html'); s = open(p, encoding='utf-8').read()
    meta, body = arts[slug]
    titles = {k: v[0].get('titre', k) for k, v in arts.items()}
    chapo, prose, toc = md(body)
    rel = ''.join(f'<a href="GuideArticle.dc.html"><span>{html.escape(arts[c][0].get("rubrique", ""))}</span>{html.escape(titles[c])}</a>'
                  for c in (meta.get('connexes') or []) if c in arts)
    chips = ''.join(f'<span>{html.escape(x)}</span>' for x in [meta.get('rubrique', ''), meta.get('duree', '')] if x)
    # Sommaire : liste dans le script (/* CONTENU */), rendue par sc-for ; repliable et collé sous l'en-tête sur téléphone
    tocdata = [{'id': hid, 'label': h} for hid, h in toc]
    tochtml = ('<button class="tocbtn" type="button" aria-expanded="{{ tocExp }}" aria-controls="toc-list" onClick="{{ toggleToc }}">'
               '<span class="tocmeta"><b>DANS CET ARTICLE</b><span class="tocpos">{{ tocPos }}</span></span>'
               '<span class="tocnow">{{ tocNow }}</span>'
               '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg></button>'
               '<div class="toclist" id="toc-list"><b>DANS CET ARTICLE</b>'
               '<sc-for list="{{ toc }}" as="t" hint-placeholder-count="8"><a class="{{ t.cls }}" href="{{ t.href }}" aria-current="{{ t.cur }}" onClick="{{ t.go }}"><i>{{ t.n }}</i>{{ t.label }}</a></sc-for></div>'
               '<span class="tocscrim" onClick="{{ closeToc }}" aria-hidden="true"></span>')
    art = f'''<!-- ARTICLE -->
<article>
  <header class="ahead"><div class="wrap">
    <nav class="crumbs" aria-label="Fil d'Ariane"><a href="Guide.dc.html">Guide</a><span aria-hidden="true">›</span><a href="Guide.dc.html#rubriques">{html.escape(meta.get('rubrique', ''))}</a></nav>
    <h1 class="disp">{html.escape(meta.get('titre', slug))}</h1>
    <p class="chapo">{chapo}</p>
    <div class="meta">{chips}</div>
  </div></header>
  <div class="wrap body">
    <nav class="{{{{ tocCls }}}}" aria-label="Sommaire de l'article">{tochtml}</nav>
    <div class="prose" ref="{{{{ proseRef }}}}">
{prose}
      <div class="helpful"><span class="hq">Cet article vous a aidé&#8239;?</span><button class="{{{{ yesCls }}}}" onClick="{{{{ voteYes }}}}">Oui</button><button class="{{{{ noCls }}}}" onClick="{{{{ voteNo }}}}">Pas vraiment</button><span class="hthx" aria-live="polite">{{{{ thanks }}}}</span></div>
      <div class="rel">{rel}</div>
    </div>
  </div>
</article>
<!-- FIN ARTICLE -->'''
    if '<!-- FIN ARTICLE -->' in s: s = re.sub(r'<!-- ARTICLE -->.*?<!-- FIN ARTICLE -->', lambda m: art, s, flags=re.S)
    else: s = s.replace('<!-- ARTICLE -->', art, 1)
    s = re.sub(r'<title>[^<]*</title>', f'<title>{html.escape(meta.get("titre", slug))}</title>', s, count=1)
    block = f"    const TOC = {jsstr(tocdata)};\n"
    s = re.sub(r'(    /\* CONTENU \*/\n).*?(    /\* FIN DU CONTENU \*/)', lambda m: m.group(1) + block + m.group(2), s, flags=re.S)
    open(p, 'w', encoding='utf-8').write(s)
    print('Article :', slug, len(toc), 'intertitres')

def build_faq(arts):
    p = os.path.join(PROJ, 'GuideFaq.dc.html'); s = open(p, encoding='utf-8').read()
    meta, body = arts['faq']
    body = re.sub(r'<!--.*?-->', '', body, flags=re.S)
    Q, group, cur = [], 'Général', None
    paras = [x.strip() for x in re.split(r'\n\s*\n', body) if x.strip()]
    chapo = unlink(re.sub(r'\s+', ' ', paras[0])) if paras and not paras[0].startswith('#') else ''
    for line in body.splitlines():
        if line.startswith('# '): group = line[2:].strip(); cur = None; continue
        if line.startswith('## '): cur = {'group': group, 'q': line[3:].strip(), 'a': ''}; Q.append(cur); continue
        if cur is not None and line.strip():
            cur['a'] += (' ' if cur['a'] else '') + unlink(line.strip()).replace('**', '')
    block = f"    const META = {jsstr({'title': meta.get('titre', 'Questions fréquentes'), 'chapo': chapo})};\n    const Q = {jsstr(Q)};\n"
    s = re.sub(r'(    /\* CONTENU \*/\n).*?(    /\* FIN DU CONTENU \*/)', lambda m: m.group(1) + block + m.group(2), s, flags=re.S)
    open(p, 'w', encoding='utf-8').write(s)
    print('FAQ :', len(Q), 'questions,', len(set(q['group'] for q in Q)), 'thèmes')

if __name__ == '__main__':
    arts = load()
    build_hub(arts)
    slug = sys.argv[1] if len(sys.argv) > 1 else 'apple-tv'
    if slug in arts: build_article(arts, slug)
    else: print('Article absent :', slug)
    if 'faq' in arts: build_faq(arts)
