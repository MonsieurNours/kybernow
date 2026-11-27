# Bloc commun : héros vivant (diaporama + défilement), pour Jeu (texte à gauche) et Pro (texte à droite)
def css(side):
    left = side == 'left'
    shade = ('linear-gradient(90deg,rgba(5,7,10,.94) 0%,rgba(5,7,10,.7) 38%,rgba(5,7,10,.05) 72%)' if left
             else 'linear-gradient(270deg,rgba(5,7,10,.95) 0%,rgba(5,7,10,.72) 38%,rgba(5,7,10,.05) 72%)')
    card = 'right:max(24px,5vw)' if left else 'left:max(24px,5vw)'
    inmargin = '' if left else 'margin-left:auto;'
    return f'''/* Héros vivant */
.hero{{position:relative;--p:.3}}
.live .hero{{height:1500px}}
.hstage{{position:relative;height:820px;overflow:hidden}}
.live .hstage{{position:sticky;top:68px}}
.hphoto{{position:absolute;inset:0;transform-origin:50% 45%;transform:scale(calc(1 + var(--p) * .28))}}
.hphoto img{{position:absolute;inset:0;width:100%;height:100%;object-fit:cover;opacity:0;transition:opacity 1.4s ease}}
.hphoto img.on{{opacity:1;animation:kb 9s ease-out both}}
.hphoto img.flip{{scale:-1 1}}
@keyframes kb{{from{{transform:scale(1.08)}}to{{transform:scale(1)}}}}
.hshade{{position:absolute;inset:0;background:{shade},linear-gradient(0deg,rgba(5,7,10,.9),transparent 30%)}}
.hdark{{position:absolute;inset:0;background:#05070A;opacity:calc(max(0, var(--p) - .58) * 2.6);pointer-events:none}}
.htext{{position:absolute;left:0;right:0;top:110px;opacity:calc(1 - max(0, var(--p) - .35) * 2.6);transform:translateY(calc(max(0, var(--p) - .35) * -80px))}}
.htext .in{{display:grid;gap:22px;max-width:640px;{inmargin}}}
.htext h1{{font-size:clamp(44px,5vw,72px)}}
.rot{{display:grid}}
.rot em{{grid-area:1/1;font-style:normal;color:var(--sig);opacity:0;transform:translateY(.35em);transition:opacity .6s ease,transform .9s cubic-bezier(.2,.9,.1,1)}}
.rot em.on{{opacity:1;transform:none}}
.hcard{{position:absolute;{card};bottom:64px;display:block;padding:16px 18px 26px;border-radius:14px;background:rgba(9,13,18,.82);backdrop-filter:blur(12px);-webkit-backdrop-filter:blur(12px);border:1px solid var(--sline);width:320px;max-width:calc(100% - 48px);opacity:calc(1 - max(0, var(--p) - .45) * 3.2)}}
.hcard .row{{display:flex;align-items:center;gap:12px;min-height:44px}}
.hcard .row > div{{display:flex;flex-direction:column;gap:2px;min-width:0}}
.hcard .dot{{flex:none;width:9px;height:9px;border-radius:50%;background:#4CD68A;box-shadow:0 0 10px #4CD68A}}
.hcard b{{display:block;font-size:16px}}
.hcard small{{display:block;color:var(--frost2);font-size:13px}}
.hcard .bars{{position:absolute;left:18px;right:18px;bottom:12px;display:flex;gap:6px}}
.hcard .bars span{{flex:1;height:3px;border-radius:3px;background:var(--line2);transition:background .6s}}
.hcard .bars span.on{{background:var(--sig);box-shadow:0 0 8px var(--sig)}}
.scrollhint{{position:absolute;left:50%;bottom:24px;transform:translateX(-50%);font-family:"Martian Mono",monospace;font-size:11px;letter-spacing:.16em;color:var(--mist);opacity:calc(1 - var(--p) * 2.5)}}
'''

def html(kicker, title_start, lead, ctas, note):
    return f'''<section class="hero" id="haut" ref="{{{{ heroRef }}}}">
  <div class="hstage">
    <div class="hphoto">
      <sc-for list="{{{{ scenes }}}}" as="sc" hint-placeholder-count="4"><img class="{{{{ sc.cls }}}}" src="{{{{ sc.img }}}}" alt="{{{{ sc.alt }}}}"></sc-for>
    </div>
    <div class="hshade" aria-hidden="true"></div>
    <div class="htext"><div class="wrap"><div class="in">
      <p class="k">{kicker}</p>
      <h1 class="disp">{title_start} <span class="rot"><sc-for list="{{{{ scenes }}}}" as="sc" hint-placeholder-count="4"><em class="{{{{ sc.wcls }}}}">{{{{ sc.word }}}}</em></sc-for></span></h1>
      <p class="lead">{lead}</p>
      <div style="display: flex; gap: 12px; flex-wrap: wrap">{ctas}</div>
      <p style="font-size: 14px; color: #8A99A3">{note}</p>
    </div></div></div>
    <div class="hcard" aria-live="off">
      <div class="row"><span class="dot"></span><div><b>{{{{ scene.dev }}}}</b><small>{{{{ scene.note }}}}</small></div></div>
      <div class="bars"><sc-for list="{{{{ scenes }}}}" as="sc" hint-placeholder-count="4"><span class="{{{{ sc.bcls }}}}"></span></sc-for></div>
    </div>
    <div class="hdark" aria-hidden="true"></div>
    <p class="scrollhint">FAITES DÉFILER</p>
  </div>
</section>'''

JS_TIMER = '''    if (!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches)) {
      this._t = setInterval(() => { const s = this.state || {}; this.setState({ sc: (s.sc || 0) + 1 }); }, 4800);
    }
'''
JS_SCENES = '''    const sc = (s.sc || 0) % SC.length;
    const scenes = SC.map((x, i) => ({ ...x, cls: (i === sc ? 'on' : '') + (x.flip ? ' flip' : ''), wcls: i === sc ? 'on' : '', bcls: i === sc ? 'on' : '' }));
'''
