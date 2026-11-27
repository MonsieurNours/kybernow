# Menu mobile commun : bouton dans l'en-tête, feuille plein écran sous 900 px.
#   python3 shim/menu.py Main Jeu Pro Telecharger Guide GuideArticle GuideFaq
import re, sys, os
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BTN = ('<button class="burger" onClick="{{ toggleMenu }}" aria-expanded="{{ menuExpanded }}" aria-controls="menu-mobile" aria-label="{{ menuLabel }}">'
       '<span></span><span></span><span></span></button>')

def sheet(links):
    items = ''.join(f'\n      <a class="mi{(" " + cls) if cls else ""}" href="{href}" onClick="{{{{ closeMenu }}}}" style="--i: {i}">{label}</a>' for i, (label, href, cls) in enumerate(links))
    return f'''
<div class="{{{{ sheetCls }}}}" id="menu-mobile" role="dialog" aria-modal="true" aria-label="Menu">
  <nav class="sheet-in" aria-label="Menu principal">{items}
  </nav>
  <div class="sheet-foot">
    <a class="mi-small" href="#" onClick="{{{{ closeMenu }}}}">Se connecter</a>
    <span class="mi-small">FR · <span style="opacity: .6">EN</span></span>
    <a class="btn sig" href="Telecharger.dc.html" onClick="{{{{ closeMenu }}}}" style="width: 100%; justify-content: center">Télécharger KyberNow</a>
  </div>
</div>'''

CSS = '''
/* Menu mobile */
.burger{display:none;width:44px;height:44px;border-radius:10px;border:1px solid var(--line2);background:transparent;cursor:pointer;padding:0;flex-direction:column;align-items:center;justify-content:center;gap:5px}
.burger span{display:block;width:18px;height:1.6px;border-radius:2px;background:var(--frost);transition:transform .35s cubic-bezier(.2,.9,.1,1),opacity .2s}
.burger[aria-expanded="true"] span:nth-child(1){transform:translateY(6.6px) rotate(45deg)}
.burger[aria-expanded="true"] span:nth-child(2){opacity:0}
.burger[aria-expanded="true"] span:nth-child(3){transform:translateY(-6.6px) rotate(-45deg)}
.sheet{position:fixed;left:0;right:0;top:64px;bottom:0;z-index:19;display:none;flex-direction:column;justify-content:space-between;gap:28px;padding:28px 24px calc(28px + env(safe-area-inset-bottom, 0px));background:rgba(var(--veil),.97);backdrop-filter:blur(18px);-webkit-backdrop-filter:blur(18px);overflow-y:auto}
.sheet-in{display:grid;gap:4px}
.mi{display:block;padding:12px 0;font-size:34px;font-stretch:118%;font-weight:720;letter-spacing:-.01em;line-height:1.1;text-decoration:none;color:var(--frost);border-bottom:1px solid var(--line);opacity:0;transform:translateY(14px)}
.mi.j{color:var(--spectre)}.mi.p{color:var(--glacier)}
.sheet.open .mi{animation:miIn .5s cubic-bezier(.2,.9,.1,1) both;animation-delay:calc(var(--i) * 45ms + 60ms)}
@keyframes miIn{to{opacity:1;transform:none}}
.sheet-foot{display:grid;grid-template-columns:1fr auto;gap:14px;align-items:center}
.sheet-foot .btn{grid-column:1 / -1}
.mi-small{font-size:16px;color:var(--frost2);text-decoration:none;min-height:44px;display:inline-flex;align-items:center}
@media (max-width:900px){
  .burger{display:inline-flex}
  .sheet.open{display:flex}
  .top .tools > span{display:none}
  .top .tools{gap:10px;margin-left:auto}
  .top .wrap{flex-wrap:nowrap;gap:12px;min-height:64px}
  .top .logo,.top .wrap > a:first-child{flex:none}
  .top .tools .btn.sig{min-height:40px;padding:0 14px;font-size:14px}
}
@media (prefers-reduced-motion:reduce){.sheet.open .mi{animation:none;opacity:1;transform:none}}
'''

def links(page):
    c = 'Clair' if page.endswith('Clair') else ''
    return [('Jouer', f'Jeu{c}.dc.html', 'j'), ('Travailler', f'Pro{c}.dc.html', 'p'), ('Share', f'Main{c}.dc.html#famille', ''),
            ('Technologie', f'Main{c}.dc.html#trajet', ''), ('Guide', f'Guide{c}.dc.html', ''), ('Discord', 'https://discord.gg/sVy2mu5WdT', '')]

def run(page):
    p = os.path.join(ROOT, 'project', page + '.dc.html'); s = open(p).read()
    if 'class="burger"' in s:
        print(page, 'déjà'); return
    m = re.search(r'(<div class="tools">.*?)(</div>\n  </div>\n</header>)', s, re.S)
    assert m, page
    s = s[:m.start(2)] + BTN + s[m.start(2):]
    s = s.replace('</header>', '</header>' + sheet(links(page)), 1)
    anchor = '@supports (animation-timeline: view())' if '@supports (animation-timeline: view())' in s else '@media (max-width:900px){'
    s = s.replace(anchor, CSS + anchor, 1)
    if '--veil:' not in s:  # pages sans jetons de voile (guide) : valeur sombre par défaut
        s = s.replace('.pg{--ink0:#05070A;', '.pg{--veil:5,7,10;--veil2:9,13,18;--ink0:#05070A;', 1)
    s = s.replace("  renderVals() {\n    const s = this.state || {};\n", "  renderVals() {\n    const s = this.state || {};\n    const menu = !!s.menu;\n", 1)
    i = s.index('    return {\n', s.index('renderVals() {'))
    s = s[:i] + ("    return {\n      toggleMenu: () => this.setState({ menu: !menu }),\n      closeMenu: () => this.setState({ menu: false }),\n"
                 "      menuExpanded: menu ? 'true' : 'false',\n      menuLabel: menu ? 'Fermer le menu' : 'Ouvrir le menu',\n      sheetCls: menu ? 'sheet open' : 'sheet',\n") + s[i + len('    return {\n'):]
    c = 'Clair' if page.endswith('Clair') else ''
    s = re.sub(r'(<nav class="nav"[^>]*>.*?)<a href="[^"]*">Guide</a>', lambda mm: mm.group(1) + f'<a href="Guide{c}.dc.html">Guide</a>', s, count=1, flags=re.S)
    open(p, 'w').write(s); print(page, 'ok')

for pg in sys.argv[1:]:
    run(pg)
