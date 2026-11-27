#!/usr/bin/env python3
"""Génère le guide statique du site à partir des articles Markdown.

Sources : docs/refonte-site-2026-11/guide/*.md (français) et docs/refonte-site-2026-11/guide/en/*.md (anglais, même nom
de fichier, `slug_en` dans l'en-tête). Sorties : site/guide/ (accueil, un dossier par article, faq/) et site/en/guide/.

    scripts/site-guide.py            # tout régénérer
    scripts/site-guide.py --check    # vérifier sans écrire (liens internes, en-têtes)

Après une modification de la FAQ, rien d'autre à relancer : les données structurées FAQPage sont produites ici.
"""
import html, json, os, re, sys, unicodedata

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO, 'docs', 'refonte-site-2026-11', 'guide')
SITE = os.path.join(REPO, 'site')
BASE_URL = 'https://www.kybernow.app'
DISCORD = 'https://discord.gg/sVy2mu5WdT'

RUBS = [('Commencer', 'M12 3v18M3 12h18'), ('Installer la Station', 'M4 5h16v11H4zM2 19h20'),
        ('Connecter un appareil', 'M5 3h14v18H5zM11 18h2'), ('Pendant la session', 'M4 6h16M4 12h16M4 18h10'),
        ('Consoles', 'M6 9h12a4 4 0 0 1 4 4v1a3 3 0 0 1-5 2l-1-1H8l-1 1a3 3 0 0 1-5-2v-1a4 4 0 0 1 4-4z'),
        ('Partager', 'M8 12a3 3 0 1 0 0-.01M17 6a3 3 0 1 0 0-.01M17 18a3 3 0 1 0 0-.01M10.5 10.5l4-3M10.5 13.5l4 3'),
        ('Comptes et équipes', 'M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM2 21c0-4 3-6 7-6s7 2 7 6M17 11a3 3 0 1 0 0-6M22 21c0-3-2-5-5-5.5'),
        ('Hors de chez vous', 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9s1.3-6.4 3.8-9z'),
        ('Entretien', 'M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18v3h3l6.3-6.3a4 4 0 0 0 5.4-5.4l-2.6 2.6-2.4-.6-.6-2.4z')]
RUB_EN = {'Commencer': 'Get started', 'Installer la Station': 'Install the Station', 'Connecter un appareil': 'Connect a device',
          'Pendant la session': 'During a session', 'Consoles': 'Consoles', 'Partager': 'Share', 'Comptes et équipes': 'Accounts and teams',
          'Hors de chez vous': 'Away from home', 'Entretien': 'Maintenance'}

T = {
 'fr': dict(home='/', guide='/guide/', lang_other='en', other_label='EN', other_full='English', skip='Aller au contenu', logo_label='KyberNow, version bêta, accueil',
   beta='Bêta', nav=[('j', '/jeu/', 'Jouer'), ('p', '/pro/', 'Travailler'), ('', '/share/', 'Share'), ('', '/technologie/', 'Technologie'), ('', '/guide/', 'Guide')],
   theme='Passer en thème clair', signin='Se connecter', signin_url='https://share.kybernow.app/compte/connexion?lang=fr', download='Télécharger', dl_url='/telecharger/',
   dl_full='Télécharger KyberNow', menu_open='Ouvrir le menu', menu='Menu', main_menu='Menu principal', footer='Pied de page',
   foot_who='Un projet libre de Les Lignes Claires.', foot=[('/jeu/', 'Jouer'), ('/pro/', 'Travailler'), ('/share/', 'Share'), ('/technologie/', 'Technologie'), ('/guide/', 'Guide'), ('/guide/faq/', 'FAQ'), ('/telecharger/', 'Télécharger'), (DISCORD, 'Discord'), ('https://github.com/MonsieurNours/kybernow', 'GitHub'), ('/mentions-legales/', 'Mentions légales')],
   og_locale='fr_FR', og_alt='en_GB', og_img='kybernow-fr.jpg',
   k_guide='Le guide', search_ph='Apple TV, code, relais, PS5…', search_label='Chercher dans le guide', hints=['Apple TV', 'code', 'PS5', 'relais', 'HDR'],
   k_all='Tous les tutos', k_faq='Questions fréquentes', faq_more='Toutes les questions', join='Rejoindre le Discord', see_faq='Voir la FAQ',
   empty='Aucun tuto ne correspond. Essayez un autre mot, ou posez la question sur Discord.', start='Commencer',
   paths=[('j', 'Jouer sur la télé', 'Du PC du bureau à la télé du salon', ['Installer la Station sur le PC', "Installer Link sur l'Apple TV ou la télé LG", 'Accepter le code dans Admin'], 'apple-tv'),
          ('p', 'Travailler à distance', 'Votre station, depuis la maison', ['Installer la Station au bureau', "Allumer l'accès hors de chez vous", 'Ouvrir Link sur votre portable'], 'hors-de-chez-vous'),
          ('c', 'Une équipe', 'Tout un parc en quelques minutes', ["Créer l'équipe et ses groupes", "Déployer avec un jeton d'enrôlement", 'Donner les droits par groupe'], 'familles-et-equipes')],
   crumbs='Fil d’Ariane', toc='Sommaire de l’article', in_article='DANS CET ARTICLE', helpful='Cet article vous a aidé ?', yes='Oui', no='Pas vraiment',
   all_groups='Toutes', themes='Thèmes', guide_title='Le guide de KyberNow'),
 'en': dict(home='/en/', guide='/en/guide/', lang_other='fr', other_label='FR', other_full='Français', skip='Skip to content', logo_label='KyberNow, beta, home',
   beta='Beta', nav=[('j', '/en/play/', 'Play'), ('p', '/en/pro/', 'Work'), ('', '/en/share/', 'Share'), ('', '/en/technology/', 'Technology'), ('', '/en/guide/', 'Guide')],
   theme='Switch to light theme', signin='Sign in', signin_url='https://share.kybernow.app/compte/connexion?lang=en', download='Download', dl_url='/en/download/',
   dl_full='Download KyberNow', menu_open='Open menu', menu='Menu', main_menu='Main menu', footer='Footer',
   foot_who='A free and open project by Les Lignes Claires.', foot=[('/en/play/', 'Play'), ('/en/pro/', 'Work'), ('/en/share/', 'Share'), ('/en/technology/', 'Technology'), ('/en/guide/', 'Guide'), ('/en/guide/faq/', 'FAQ'), ('/en/download/', 'Download'), (DISCORD, 'Discord'), ('https://github.com/MonsieurNours/kybernow', 'GitHub'), ('/en/legal/', 'Legal notice')],
   og_locale='en_GB', og_alt='fr_FR', og_img='kybernow-en.jpg',
   k_guide='The guide', search_ph='Apple TV, code, relay, PS5…', search_label='Search the guide', hints=['Apple TV', 'code', 'PS5', 'relay', 'HDR'],
   k_all='Every tutorial', k_faq='Frequently asked questions', faq_more='All questions', join='Join the Discord', see_faq='See the FAQ',
   empty='No tutorial matches. Try another word, or ask on Discord.', start='Start',
   paths=[('j', 'Play on the TV', 'From the office PC to the living-room TV', ['Install the Station on the PC', 'Install Link on the Apple TV or LG TV', 'Accept the code in Admin'], 'apple-tv'),
          ('p', 'Work remotely', 'Your workstation, from home', ['Install the Station at the office', 'Turn on away-from-home access', 'Open Link on your laptop'], 'hors-de-chez-vous'),
          ('c', 'A team', 'A whole fleet in a few minutes', ['Create the team and its groups', 'Deploy with an enrolment token', 'Grant rights by group'], 'familles-et-equipes')],
   crumbs='Breadcrumb', toc='Article contents', in_article='IN THIS ARTICLE', helpful='Did this article help?', yes='Yes', no='Not really',
   all_groups='All', themes='Topics', guide_title='The KyberNow guide'),
}

# ---------------------------------------------------------------- Markdown

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

class Ctx:
    def __init__(self, lang, urls):
        self.lang, self.urls = lang, urls
    def link(self, url):
        m = re.match(r'^/guide/([a-z0-9-]+)/?(#.*)?$', url)
        if m:
            u = self.urls.get(m.group(1))
            if u: return u + (m.group(2) or '')
        if self.lang == 'en':
            for a, b in [('/telecharger/', '/en/download/'), ('/jeu/', '/en/play/'), ('/technologie/', '/en/technology/'), ('/share/', '/en/share/'), ('/pro/', '/en/pro/'), ('/mentions-legales/', '/en/legal/')]:
                if url.startswith(a): return b + url[len(a):]
        return url

def inline(t, ctx):
    t = html.escape(t, quote=False)
    t = re.sub(r'`([^`]+)`', r'<code>\1</code>', t)
    t = re.sub(r'\*\*([^*]+)\*\*', r'<strong>\1</strong>', t)
    t = re.sub(r'(?<![*\w])\*([^*\n]+)\*(?!\w)', r'<em>\1</em>', t)
    t = re.sub(r'\[([^\]]+)\]\(([^)]+)\)', lambda m: f'<a href="{html.escape(ctx.link(m.group(2)))}">{m.group(1)}</a>', t)
    return t

def md(body, ctx):
    body = re.sub(r'<!--.*?-->', '', body, flags=re.S)
    out, toc, chapo, para = [], [], None, []
    lines = body.splitlines(); i = 0
    def flush():
        nonlocal para, chapo
        if para:
            txt = inline(' '.join(para), ctx)
            if chapo is None and not toc and not out: chapo = txt
            else: out.append(f'<p>{txt}</p>')
            para = []
    while i < len(lines):
        l = lines[i].rstrip()
        if not l.strip(): flush(); i += 1; continue
        if l.startswith('```'):
            flush(); code = []; i += 1
            while i < len(lines) and not lines[i].startswith('```'):
                code.append(lines[i]); i += 1
            i += 1
            out.append('<pre class="code"><code>' + html.escape('\n'.join(code)) + '</code></pre>'); continue
        if l.startswith('## '):
            flush(); h = l[3:].strip(); hid = slugify(h)[:48]
            base, n = hid, 2
            while any(x[0] == hid for x in toc): hid = f'{base}-{n}'; n += 1
            toc.append((hid, h)); out.append(f'<h2 id="{hid}">{inline(h, ctx)}</h2>'); i += 1; continue
        if l.startswith('### '): flush(); out.append(f'<h3>{inline(l[4:].strip(), ctx)}</h3>'); i += 1; continue
        if l.startswith('# '): flush(); i += 1; continue
        if l.startswith('>'):
            flush(); q = []
            while i < len(lines) and lines[i].startswith('>'):
                q.append(lines[i].lstrip('> ').strip()); i += 1
            out.append('<blockquote><p>' + inline(' '.join(x for x in q if x), ctx) + '</p></blockquote>'); continue
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
            out.append(f'<{tag}>' + ''.join(f'<li>{inline(x, ctx)}</li>' for x in items) + f'</{tag}>'); continue
        para.append(l.strip()); i += 1
    flush()
    return chapo or '', '\n'.join(out), toc

def plain(t):
    t = re.sub(r'\[([^\]]+)\]\([^)]+\)', r'\1', t)
    return re.sub(r'\s+', ' ', t.replace('**', '').replace('`', '')).strip()

# ---------------------------------------------------------------- Gabarit

def e(x): return html.escape(x or '', quote=True)

def head(lang, title, desc, path, alt_path, css=('guide',), jsonld=None):
    t = T[lang]
    url = BASE_URL + path
    fr_url = BASE_URL + (path if lang == 'fr' else alt_path)
    en_url = BASE_URL + (path if lang == 'en' else alt_path)
    ld = f'\n<script type="application/ld+json">{json.dumps(jsonld, ensure_ascii=False)}</script>' if jsonld else ''
    links = ''.join(f'\n<link rel="stylesheet" href="/assets/kn/{c}.css">' for c in css)
    return f'''<!doctype html>
<html lang="{lang}" data-u="commun" data-theme="dark">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{e(title)}</title>
<meta name="description" content="{e(desc)}">
<link rel="canonical" href="{url}">
<meta name="theme-color" content="#05070A">
<meta property="og:type" content="article">
<meta property="og:site_name" content="KyberNow">
<meta property="og:locale" content="{t['og_locale']}">
<meta property="og:locale:alternate" content="{t['og_alt']}">
<meta property="og:title" content="{e(title)}">
<meta property="og:description" content="{e(desc)}">
<meta property="og:url" content="{url}">
<meta property="og:image" content="{BASE_URL}/assets/og/{t['og_img']}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta name="twitter:card" content="summary_large_image">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="alternate" hreflang="fr" href="{fr_url}">
<link rel="alternate" hreflang="en" href="{en_url}">
<link rel="alternate" hreflang="x-default" href="{en_url}">
<script src="/assets/lang.js"></script>
<script src="/assets/kn/theme.js"></script>
<link rel="preload" href="/fonts/Archivo-wdth-wght.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/assets/kn/base.css">{links}{ld}
</head>
<body>
'''

THM = ('<button class="thm" type="button" aria-label="{label}"><svg class="moon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/></svg>'
       '<svg class="sun" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg></button>')

def header(lang, alt_path):
    t = T[lang]
    nav = ''.join(f'\n      <a{" class=" + chr(34) + c + chr(34) if c else ""} href="{h}"{" aria-current=" + chr(34) + "page" + chr(34) if h == t["guide"] else ""}>{l}</a>' for c, h, l in t['nav'])
    mi = ''.join(f'\n    <a class="mi{" " + c if c else ""}" href="{h}" style="--i: {i}">{l}</a>' for i, (c, h, l) in enumerate(t['nav']))
    return f'''<a class="skip" href="#contenu">{t['skip']}</a>
<header class="top">
  <div class="wrap">
    <a class="logo" href="{t['home']}" aria-label="{t['logo_label']}">
      <img class="dk" src="/assets/img/logo/logo-spectre.svg" alt="KyberNow" width="150" height="21">
      <img class="lt" src="/assets/img/logo/logo-spectre-clair.svg" alt="" width="150" height="21" aria-hidden="true">
      <span class="beta" aria-hidden="true">{t['beta']}</span>
    </a>
    <nav class="nav" aria-label="Sections">{nav}
      <a class="disc" href="{DISCORD}" rel="noopener"><svg class="ic" aria-hidden="true"><use href="/assets/icons.svg#discord-logo"/></svg>Discord</a>
    </nav>
    <div class="tools">
      {THM.format(label=t['theme'])}
      <a class="signin" href="{t['signin_url']}" rel="noopener"><span>{t['signin']}</span></a>
      <a class="lang" href="{alt_path}" hreflang="{t['lang_other']}" lang="{t['lang_other']}" data-set-lang="{t['lang_other']}">{t['other_label']}</a>
      <a class="btn sig" href="{t['dl_url']}">{t['download']}</a>
      <button class="burger" type="button" aria-expanded="false" aria-controls="menu" aria-label="{t['menu_open']}"><span></span><span></span><span></span></button>
    </div>
  </div>
</header>
<div class="sheet" id="menu" role="dialog" aria-modal="true" aria-label="{t['menu']}">
  <nav aria-label="{t['main_menu']}">{mi}
    <a class="mi" href="{DISCORD}" rel="noopener" style="--i: 5">Discord</a>
  </nav>
  <div class="sheet-foot">
    <a href="{t['signin_url']}" rel="noopener">{t['signin']}</a>
    <a href="{alt_path}" hreflang="{t['lang_other']}" lang="{t['lang_other']}" data-set-lang="{t['lang_other']}">{t['other_full']}</a>
    <a class="btn sig" href="{t['dl_url']}">{t['dl_full']}</a>
  </div>
</div>
'''

def footer(lang):
    t = T[lang]
    links = ''.join(f'<a href="{h}"{" rel=" + chr(34) + "noopener" + chr(34) if h.startswith("http") else ""}>{l}</a>' for h, l in t['foot'])
    return f'''<footer class="ft">
  <div class="wrap">
    <div class="who">
      <img class="dk" src="/assets/img/logo/logo-spectre.svg" alt="KyberNow" width="132" height="18">
      <img class="lt" src="/assets/img/logo/logo-spectre-clair.svg" alt="" width="132" height="18" aria-hidden="true">
      <span>{t['foot_who']}</span>
    </div>
    <nav aria-label="{t['footer']}">{links}</nav>
  </div>
</footer>
<script src="/assets/kn/base.js" defer></script>
<script src="/assets/kn/guide.js" defer></script>
</body>
</html>
'''

ARROW = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6"/></svg>'
PLUS = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>'
CHEV = '<svg class="chev" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>'

# ---------------------------------------------------------------- Chargement

def load():
    langs = {'fr': {}, 'en': {}}
    for f in sorted(os.listdir(SRC)):
        if f.endswith('.md') and not f[:-3].isupper():
            meta, body = front(os.path.join(SRC, f)); meta.setdefault('slug', f[:-3]); langs['fr'][f[:-3]] = (meta, body)
    en_dir = os.path.join(SRC, 'en')
    if os.path.isdir(en_dir):
        for f in sorted(os.listdir(en_dir)):
            if f.endswith('.md'):
                meta, body = front(os.path.join(en_dir, f)); langs['en'][f[:-3]] = (meta, body)
    return langs

def urls_for(langs):
    u = {'fr': {}, 'en': {}}
    for key, (meta, _) in langs['fr'].items():
        if key.startswith('_'): continue
        u['fr'][key] = '/guide/' + key + '/'
        en = langs['en'].get(key)
        u['en'][key] = '/en/guide/' + ((en[0].get('slug_en') if en else None) or key) + '/'
    return u

def write(path, text, check):
    if check: return
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, 'w', encoding='utf-8').write(text)

# ---------------------------------------------------------------- Pages

def faq_items(body):
    body = re.sub(r'<!--.*?-->', '', body, flags=re.S)
    Q, group, cur = [], '', None
    for line in body.splitlines():
        if line.startswith('# '): group = line[2:].strip(); cur = None; continue
        if line.startswith('## '): cur = {'group': group, 'q': line[3:].strip(), 'a': []}; Q.append(cur); continue
        if cur is not None and line.strip(): cur['a'].append(line.strip())
    return Q

def page_article(lang, key, langs, urls, check):
    t = T[lang]; meta, body = langs[lang][key]
    ctx = Ctx(lang, urls[lang])
    path = urls[lang][key]; alt = urls['en' if lang == 'fr' else 'fr'][key]
    chapo, prose, toc = md(body, ctx)
    rub = meta.get('rubrique', '')
    rub_label = (meta.get('rubrique_en') or RUB_EN.get(rub, rub)) if lang == 'en' else rub
    rel = ''
    for c in (meta.get('connexes') or []):
        if c in langs[lang]:
            cm = langs[lang][c][0]; cr = cm.get('rubrique', '')
            crl = (cm.get('rubrique_en') or RUB_EN.get(cr, cr)) if lang == 'en' else cr
            rel += f'<a href="{urls[lang][c]}"><span>{e(crl)}</span>{e(cm.get("titre", c))}</a>'
    chips = ''.join(f'<span>{e(x)}</span>' for x in [rub_label, meta.get('duree', '')] if x)
    n = len(toc)
    tocl = ''.join(f'<a{" class=" + chr(34) + "on" + chr(34) if i == 0 else ""} href="#{hid}"><i>{i + 1:02d}</i>{e(h)}</a>' for i, (hid, h) in enumerate(toc))
    tochtml = (f'<button class="tocbtn" type="button" aria-expanded="false" aria-controls="toc-list"><span class="tocmeta"><b>{t["in_article"]}</b><span class="tocpos">01 / {n:02d}</span></span>'
               f'<span class="tocnow">{e(toc[0][1]) if toc else ""}</span>{CHEV}</button>'
               f'<div class="toclist" id="toc-list"><b>{t["in_article"]}</b>{tocl}</div><span class="tocscrim" aria-hidden="true"></span>') if toc else ''
    title = meta.get('title') or meta.get('titre', key)
    ld = {'@context': 'https://schema.org', '@type': 'TechArticle', 'headline': meta.get('titre', key), 'description': meta.get('description', ''),
          'inLanguage': lang, 'url': BASE_URL + path, 'publisher': {'@type': 'Organization', 'name': 'Les Lignes Claires'},
          'isPartOf': {'@type': 'WebSite', 'name': 'KyberNow', 'url': BASE_URL + '/'}}
    crumbs_ld = {'@context': 'https://schema.org', '@type': 'BreadcrumbList', 'itemListElement': [
        {'@type': 'ListItem', 'position': 1, 'name': 'Guide', 'item': BASE_URL + t['guide']},
        {'@type': 'ListItem', 'position': 2, 'name': meta.get('titre', key), 'item': BASE_URL + path}]}
    out = head(lang, title, meta.get('description', ''), path, alt, jsonld=[ld, crumbs_ld]) + header(lang, alt) + f'''<div class="progress" aria-hidden="true"><i></i></div>
<main id="contenu">
<article>
  <header class="ahead"><div class="wrap">
    <nav class="crumbs" aria-label="{t['crumbs']}"><a href="{t['guide']}">Guide</a><span aria-hidden="true">›</span><a href="{t['guide']}#rubriques">{e(rub_label)}</a></nav>
    <h1 class="disp">{e(meta.get('titre', key))}</h1>
    <p class="chapo">{chapo}</p>
    <div class="meta">{chips}</div>
  </div></header>
  <div class="wrap body">
    <nav class="toc{'' if toc else ' none'}" aria-label="{t['toc']}">{tochtml}</nav>
    <div class="prose">
{prose}
      <div class="helpful"><span class="hq">{t['helpful']}</span><button type="button" data-vote="oui">{t['yes']}</button><button type="button" data-vote="non">{t['no']}</button><span class="hthx" aria-live="polite"></span></div>
      <div class="rel">{rel}</div>
    </div>
  </div>
</article>
</main>
''' + footer(lang)
    write(os.path.join(SITE, path.strip('/'), 'index.html'), out, check)
    return toc

def page_faq(lang, langs, urls, check):
    t = T[lang]; meta, body = langs[lang]['faq']
    ctx = Ctx(lang, urls[lang])
    path = urls[lang]['faq']; alt = urls['en' if lang == 'fr' else 'fr']['faq']
    Q = faq_items(body)
    paras = [x.strip() for x in re.split(r'\n\s*\n', re.sub(r'<!--.*?-->', '', body, flags=re.S)) if x.strip()]
    chapo = inline(re.sub(r'\s+', ' ', paras[0]), ctx) if paras and not paras[0].startswith('#') else ''
    groups = []
    for q in Q:
        if q['group'] not in groups: groups.append(q['group'])
    def gbtn(name, n, on): return f'<button type="button" class="{"on" if on else ""}" aria-pressed="{"true" if on else "false"}" data-g="{e(name)}">{e(name)}<span>{n}</span></button>'
    gb = gbtn(t['all_groups'], len(Q), True) + ''.join(gbtn(g, sum(1 for q in Q if q['group'] == g), False) for g in groups)
    items, last = [], None
    for i, q in enumerate(Q):
        if q['group'] != last:
            items.append(f'<p class="fgt" data-g="{e(q["group"])}">{e(q["group"])}</p>'); last = q['group']
        ans = inline(' '.join(q['a']), ctx)
        items.append(f'<div class="qa{" open" if i == 0 else ""}" data-g="{e(q["group"])}" id="q-{slugify(q["q"])[:40]}"><button type="button" aria-expanded="{"true" if i == 0 else "false"}">{e(q["q"])}{PLUS}</button><p class="ans">{ans}</p></div>')
    ld = {'@context': 'https://schema.org', '@type': 'FAQPage', 'inLanguage': lang, 'mainEntity': [
        {'@type': 'Question', 'name': q['q'], 'acceptedAnswer': {'@type': 'Answer', 'text': plain(' '.join(q['a']))}} for q in Q]}
    title = meta.get('title') or meta.get('titre', 'FAQ')
    out = head(lang, title, meta.get('description', ''), path, alt, jsonld=ld) + header(lang, alt) + f'''<main id="contenu">
<header class="ahead"><div class="wrap">
  <nav class="crumbs" aria-label="{t['crumbs']}"><a href="{t['guide']}">Guide</a><span aria-hidden="true">›</span><span>{e(meta.get('titre', 'FAQ'))}</span></nav>
  <h1 class="disp">{e(meta.get('titre', 'FAQ'))}</h1>
  <p class="chapo">{chapo}</p>
  <div class="fgroups" role="group" aria-label="{t['themes']}">{gb}</div>
</div></header>
<div class="fbar" role="group" aria-label="{t['themes']}"><div class="fbar-in">{gb}</div></div>
<div class="wrap">
  <div class="flist">
{chr(10).join(items)}
  </div>
</div>
</main>
''' + footer(lang)
    write(os.path.join(SITE, path.strip('/'), 'index.html'), out, check)
    return Q

def page_hub(lang, langs, urls, Q, check):
    t = T[lang]
    acc = langs[lang].get('_accueil')
    path = t['guide']; alt = T['en' if lang == 'fr' else 'fr']['guide']
    hero_title, hero_lead, help_title, help_text, intros, desc, title = t['guide_title'], '', '', '', {}, '', t['guide_title']
    ctx = Ctx(lang, urls[lang])
    if acc:
        meta, body = acc
        hero_title = meta.get('titre', hero_title); desc = meta.get('description', ''); title = meta.get('title') or hero_title
        b = re.sub(r'<!--.*?-->', '', body, flags=re.S)
        paras = [x.strip() for x in re.split(r'\n\s*\n', b) if x.strip()]
        if paras and not paras[0].startswith('#'): hero_lead = inline(re.sub(r'\s+', ' ', paras[0]), ctx)
        for m in re.finditer(r'^##\s+(.+?)\n+((?:(?!^#).+\n?)+)', b, re.M):
            hd = m.group(1).strip(); tx = inline(re.sub(r'\s+', ' ', m.group(2)).strip(), ctx)
            if 'trouvez' in hd.lower() or 'find' in hd.lower() or 'can' in hd.lower() and '?' in hd: help_title, help_text = hd, tx
            else: intros[hd] = tx
    paths = ''
    for cls, tag, h3, steps, target in t['paths']:
        lis = ''.join(f'<li>{e(s)}</li>' for s in steps)
        paths += f'''
    <a class="path {cls} rv" href="{urls[lang].get(target, t['guide'])}">
      <span class="tag">{e(tag)}</span>
      <h3>{e(h3)}</h3>
      <ol>{lis}</ol>
      <span class="go">{t['start']} {ARROW}</span>
    </a>'''
    rubs = ''
    for ri, (name, icon) in enumerate(RUBS):
        keys = [k for k, (m, b) in langs['fr'].items() if not k.startswith('_') and k != 'faq' and m.get('rubrique', '').strip().lower() == name.lower()]
        if not keys: continue
        label = RUB_EN.get(name, name) if lang == 'en' else name
        intro = next((v for k, v in intros.items() if label.lower() in k.lower() or k.lower() in label.lower() or name.lower() in k.lower()), '')
        lis = ''
        for k in keys:
            m = (langs[lang].get(k) or langs['fr'][k])[0]
            words = (k.replace('-', ' ') + ' ' + m.get('description', '') + ' ' + label)
            lis += f'<li><a href="{urls[lang][k]}" data-words="{e(words)}">{e(m.get("titre", k))}<span>{e(m.get("duree", ""))}</span></a></li>'
        rubs += f'''
      <div class="rub rv" data-rub="{ri}">
        <div class="hd"><span class="ic"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="{icon}"/></svg></span><h3><button class="rt" type="button" aria-expanded="false" aria-controls="rub-{ri}">{e(label)}</button></h3><span class="cnt" aria-hidden="true">{len(keys):02d}</span>{CHEV}</div>
        <p>{intro}</p>
        <ul id="rub-{ri}">{lis}</ul>
      </div>'''
    faqs = ''.join(f'<div class="qa{" open" if i == 0 else ""}"><button type="button" aria-expanded="{"true" if i == 0 else "false"}">{e(q["q"])}{PLUS}</button><p class="ans">{inline(" ".join(q["a"]), ctx)}</p></div>' for i, q in enumerate(Q[:5]))
    hints = ''.join(f'<button type="button" data-q="{e(h)}">{e(h)}</button>' for h in t['hints'])
    faq_title = {'fr': 'Les questions qu’on nous pose', 'en': 'The questions people ask us'}[lang]
    rub_title = {'fr': 'Tous les tutos', 'en': 'Every tutorial'}[lang]
    out = head(lang, title, desc, path, alt) + header(lang, alt) + f'''<main id="contenu">
<section class="ghero" id="haut">
  <div class="wrap"><div class="in">
    <p class="k">{t['k_guide']}</p>
    <h1 class="disp">{e(hero_title)}</h1>
    <p class="lead">{hero_lead}</p>
    <label class="search" for="q">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/></svg>
      <input id="q" type="search" placeholder="{e(t['search_ph'])}" aria-label="{e(t['search_label'])}" autocomplete="off">
      <kbd>/</kbd>
    </label>
    <div class="chipsq">{hints}</div>
  </div></div>
</section>
<section class="wrap pathsec">
  <div class="paths">{paths}
  </div>
  <div class="pdots" aria-hidden="true"><i></i></div>
</section>
<section class="sec" id="rubriques">
  <div class="wrap">
    <div class="sechead rv"><p class="k">{t['k_all']}</p><h2 class="disp">{rub_title}</h2></div>
    <div class="rubs">{rubs}
    </div>
    <p class="empty" hidden>{t['empty']}</p>
  </div>
</section>
<section class="sec" id="faq">
  <div class="wrap">
    <div class="sechead rv"><p class="k">{t['k_faq']}</p><h2 class="disp">{faq_title}</h2></div>
    <div class="faq">{faqs}</div>
    <p class="faqmore"><a class="btn" href="{urls[lang]['faq']}">{t['faq_more']}</a></p>
  </div>
</section>
<section class="sec">
  <div class="wrap">
    <div class="help rv">
      <div><h2 class="disp">{e(help_title)}</h2><p>{help_text}</p></div>
      <div class="act"><a class="btn" href="{DISCORD}" rel="noopener">{t['join']}</a><a class="btn sig" href="{urls[lang]['faq']}">{t['see_faq']}</a></div>
    </div>
  </div>
</section>
</main>
''' + footer(lang)
    write(os.path.join(SITE, path.strip('/'), 'index.html'), out, check)

# ---------------------------------------------------------------- Programme

def main():
    check = '--check' in sys.argv
    langs = load(); urls = urls_for(langs)
    problems = []
    for lang in ('fr', 'en'):
        keys = [k for k in langs['fr'] if not k.startswith('_')]
        if lang == 'en':
            missing = [k for k in keys + ['_accueil'] if k not in langs['en']]
            if missing:
                print('Anglais absent pour :', ', '.join(missing), '— le guide anglais n’est pas généré.'); continue
        for k in keys:
            m = langs[lang][k][0]
            if len(m.get('title', '')) > 60: problems.append(f'{lang}/{k} : title > 60')
            if len(m.get('description', '')) > 160: problems.append(f'{lang}/{k} : description > 160')
            for c in (m.get('connexes') or []):
                if c not in langs['fr']: problems.append(f'{lang}/{k} : connexe inconnu {c}')
            for mm in re.finditer(r'\]\((/guide/[^)#]*)', langs[lang][k][1]):
                s = mm.group(1).strip('/').split('/')
                if len(s) > 1 and s[1] not in langs['fr']: problems.append(f'{lang}/{k} : lien inconnu {mm.group(1)}')
        Q = []
        for k in keys:
            if k == 'faq': Q = faq_items(langs[lang]['faq'][1]); page_faq(lang, langs, urls, check)
            else: page_article(lang, k, langs, urls, check)
        page_hub(lang, langs, urls, Q, check)
        print(f'{lang} : {len(keys)} pages + accueil' + (' (vérification seule)' if check else ''))
    for p in problems: print('  !', p)
    # adresses pour le plan du site
    if not check:
        json.dump(urls, open(os.path.join(SRC, '.urls.json'), 'w'), ensure_ascii=False, indent=1)

if __name__ == '__main__':
    main()
