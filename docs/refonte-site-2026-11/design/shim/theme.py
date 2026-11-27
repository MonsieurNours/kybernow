import re, sys
import os
P = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'project') + '/'
LOGOS = {'17a5ec7a51421394b7eb52319ad30a07':'611b6d20f3b5596ebeeb51e67178b61e','1eaee3615692f218cd7e2a9e89c4ed16':'1a7bfae102793249377414c9af3ab1ca',
 '2f8f231b8fadc0f1e96c2fbcdaaa2dcd':'5b7db1a3743ca82286f2529ce5f49ecb','16f9109604df4d29602a355ad8e1e1ea':'ecfa83729141a4c5ca1469536e1a89c6',
 'aaed82986e6a1f50dc5e94a9c304bd0c':'e664fbf5b4077185a3bc1ed52506f4a2','e52857a4d35e7a9324f349659e60bb9d':'2856a0df1554610a00fd215affcc8932',
 '8c14e4cdfaa7adbade09b28eb000bd84':'a8ba0166ba6ef9e435c6802d9ace3ee3','f2cc670deaeaad229fb84f950c75cea8':'3ad7c07bfffddae589d7b9b31099af68',
 'a8cf2fbbcd65f53b2dea638f541b5037':'029a5d191934a40e852a7159a4308bf2','8d2635ccea1b62c53f2a34f144f874ee':'1e955d1b57be7c06627aea414db8a2ec'}
CFG = {
 'Main': dict(sigd='#9A8CFF', island='.step .scr', soft='154,140,255', light_sig=None, btn='frost'),
 'Jeu': dict(sigd='#9A8CFF', island='.browser,.sessbar .panel', soft='154,140,255', light_sig=('#5340E6','83,64,230'), btn='sig'),
 'Pro': dict(sigd='#5CC8E8', island='.console,.sample,.term', soft='92,200,232', light_sig=('#08779E','8,119,158'), btn='sig'),
 'Telecharger': dict(sigd='#9A8CFF', island='.stcard .inst,.pair .scr', soft='154,140,255', light_sig=None, btn='frost'),
 'Guide': dict(sigd='#9A8CFF', island='.nothing', soft='154,140,255', light_sig=None, btn='frost'),
 'GuideArticle': dict(sigd='#9A8CFF', island='.nothing', soft='154,140,255', light_sig=None, btn='frost'),
 'GuideFaq': dict(sigd='#9A8CFF', island='.nothing', soft='154,140,255', light_sig=None, btn='frost'),
}
TOGGLE = ('<button class="thm" onClick="{{ toggleTheme }}" aria-label="{{ themeLabel }}">'
 '<svg class="moon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/></svg>'
 '<svg class="sun" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg></button>')

def sub(s, a, b, n=1):
    assert a in s, ('MISSING', a[:90]); return s.replace(a, b, n)

def run(name):
    c = CFG[name]; p = P + name + '.dc.html'; s = open(p).read()
    s = s.replace('.pg{--veil:5,7,10;--veil2:9,13,18;--ink0:#05070A;', '.pg{--ink0:#05070A;', 1)
    s = sub(s, '.pg{--ink0:#05070A;', f'.pg{{--veil:5,7,10;--veil2:9,13,18;--src:#E3EDF0;--onsig:#05070A;--sigd:{c["sigd"]};--spectre:#9A8CFF;--glacier:#5CC8E8;--ink0:#05070A;')
    s = s.replace('rgba(5,7,10,', 'rgba(var(--veil),').replace('rgba(9,13,18,', 'rgba(var(--veil2),')
    s = s.replace('.hdark{position:absolute;inset:0;background:#05070A;', '.hdark{position:absolute;inset:0;background:var(--ink0);')
    if c['btn'] == 'frost':
        s = sub(s, '.btn.sig{background:var(--frost);color:#05070A;', '.btn.sig{background:var(--frost);color:var(--ink0);')
    else:
        s = sub(s, '.btn.sig{background:var(--sig);color:#05070A;', '.btn.sig{background:var(--sig);color:var(--onsig);')
    s = s.replace('.chips button.on{background:var(--frost);color:#05070A;', '.chips button.on{background:var(--frost);color:var(--ink0);')
    for a, b in [('color: #8A99A3', 'color: var(--mist)'), ('color: #E3EDF0', 'color: var(--frost)'), ('color: #5CC8E8', 'color: var(--glacier)'), ('color: #9A8CFF', 'color: var(--spectre)')]:
        s = s.replace(a, b)
    s = s.replace('fill="#E3EDF0"/>', 'fill="#E3EDF0" style="fill: var(--src)"/>')
    # fleet (Pro) et étapes (Main) : couleurs des animations en jetons
    s = s.replace('@keyframes enroll{0%,45%{border-color:#2B3742;background:#090D12;color:#65737E;box-shadow:none}55%,100%{border-color:rgba(92,200,232,.6);background:rgba(92,200,232,.12);color:#E3EDF0;box-shadow:0 0 18px -4px rgba(92,200,232,.6)}}',
                  '@keyframes enroll{0%,45%{border-color:var(--line2);background:var(--ink1);color:var(--mist);box-shadow:none}55%,100%{border-color:var(--sline);background:var(--soft);color:var(--frost);box-shadow:0 0 18px -4px var(--sline)}}')
    s = s.replace('@keyframes steplit{0%,33%{border-color:rgba(227,237,240,.5);', '@keyframes steplit{0%,33%{border-color:var(--frost2);').replace('34%,100%{border-color:#1E2832;box-shadow:none}', '34%,100%{border-color:var(--line);box-shadow:none}')
    # logos clairs à côté des sombres
    def dup(m):
        tag = m.group(0); i = m.group(1)
        if i not in LOGOS: return tag
        if 'class="' in tag:
            dk = re.sub(r'class="([^"]*)"', r'class="\1 dk"', tag, 1); lt = re.sub(r'class="([^"]*)"', r'class="\1 lt"', tag, 1)
        else:
            dk = tag.replace('<img ', '<img class="dk" ', 1); lt = tag.replace('<img ', '<img class="lt" ', 1)
        lt = lt.replace(i, LOGOS[i]); lt = re.sub(r'alt="[^"]*"', 'alt="" aria-hidden="true"', lt, 1)
        return dk + lt
    s = re.sub(r'<img [^>]*src="/_blob/([0-9a-f]{32})"[^>]*>', dup, s)
    # bouton de thème
    s = sub(s, '<div class="tools"><span>Se connecter</span>', '<div class="tools">' + TOGGLE + '<span>Se connecter</span>')
    # CSS du thème clair
    ls = c['light_sig']
    sigl = f'--sig:{ls[0]};--sigs:{ls[0]};--soft:rgba({ls[1]},.10);--sline:rgba({ls[1]},.35);' if ls else ''
    css = f'''
/* Thème clair */
.thm{{width:38px;height:38px;border-radius:8px;border:1px solid var(--line2);background:transparent;color:var(--frost2);display:grid;place-items:center;cursor:pointer;padding:0}}
.thm:hover{{color:var(--frost)}}
.thm .sun{{display:none}}
.pg.light .thm .sun{{display:block}}.pg.light .thm .moon{{display:none}}
.pg:not(.light) .lt{{display:none!important}}
.pg.light .dk{{display:none!important}}
.pg.light{{--ink0:#F2F5F6;--ink1:#E8EDEF;--ink2:#FFFFFF;--ink3:#F6F8F9;--line:#D8E0E4;--line2:#C3CED4;--frost:#0B1117;--frost2:#2F3B45;--mist:#55636E;--spectre:#5340E6;--glacier:#08779E;--src:#0B1117;--veil:242,245,246;--veil2:232,237,239;--onsig:#FFFFFF;{sigl}color-scheme:light}}
.pg.light :is({c["island"]}){{--ink0:#05070A;--ink1:#090D12;--ink2:#0E141A;--ink3:#141B23;--line:#1E2832;--line2:#2B3742;--frost:#E3EDF0;--frost2:#B4C2C9;--mist:#8A99A3;--veil:5,7,10;--veil2:9,13,18;--sig:var(--sigd);--soft:rgba({c["soft"]},.14);--sline:rgba({c["soft"]},.45);--src:#E3EDF0;color:var(--frost)}}
.pg.light .hphoto img{{filter:brightness(1.08) saturate(.95)}}
'''
    anchor = '@supports (animation-timeline: view())' if '@supports (animation-timeline: view())' in s else '@media (max-width:900px){'
    s = sub(s, anchor, css + anchor)
    # JS
    if "rootCls: s.live ? 'pg live' : 'pg'," in s:
        s = sub(s, "rootCls: s.live ? 'pg live' : 'pg',", "rootCls: (s.live ? 'pg live' : 'pg') + (theme === 'light' ? ' light' : ''),\n      toggleTheme: () => this.setState({ theme: theme === 'light' ? 'dark' : 'light' }),\n      themeLabel: theme === 'light' ? 'Passer en thème sombre' : 'Passer en thème clair',")
    else:
        s = sub(s, '<div class="pg">', '<div class="{{ rootCls }}">')
        i = s.index('    return {\n', s.index('renderVals() {'))
        s = s[:i] + "    return {\n      rootCls: 'pg' + (theme === 'light' ? ' light' : ''),\n      toggleTheme: () => this.setState({ theme: theme === 'light' ? 'dark' : 'light' }),\n      themeLabel: theme === 'light' ? 'Passer en thème sombre' : 'Passer en thème clair',\n" + s[i + len('    return {\n'):]
    s = sub(s, '  renderVals() {\n    const s = this.state || {};\n', "  renderVals() {\n    const s = this.state || {};\n    const theme = s.theme || this.props.theme || 'dark';\n")
    s = sub(s, "data-props='{\"$preview\"", "data-props='{\"theme\":{\"editor\":\"enum\",\"options\":[\"dark\",\"light\"],\"default\":\"dark\"},\"$preview\"")
    open(p, 'w').write(s)
    # copie claire
    c2 = s.replace("const theme = s.theme || this.props.theme || 'dark';", "const theme = s.theme || this.props.theme || 'light';").replace('"default":"dark"}', '"default":"light"}')
    for a, b in [('href="Main.dc.html', 'href="MainClair.dc.html'), ('href="Jeu.dc.html', 'href="JeuClair.dc.html'), ('href="Pro.dc.html', 'href="ProClair.dc.html'), ('href="Telecharger.dc.html', 'href="TelechargerClair.dc.html')]:
        c2 = c2.replace(a, b)
    c2 = re.sub(r'<title>([^<]*)</title>', r'<title>\1, clair</title>', c2, 1)
    open(P + name + 'Clair.dc.html', 'w').write(c2)
    print(name, 'ok')

for n in (sys.argv[1:] or list(CFG)): run(n)
