// Banc local : rend un .dc.html sans le runtime de Claude Design (mesures et essais de défilement seulement).
(function(){
  const mv = document.createElement('meta'); mv.name = 'viewport'; mv.content = 'width=device-width, initial-scale=1'; document.head.appendChild(mv);
  const q = new URLSearchParams(location.search);
  if (q.has('static')) Object.defineProperty(window, 'innerHeight', { get: () => 9000 });
  window.DCLogic = class { constructor(p){ this.props = p || {}; this.state = null; }
    setState(p){ this.state = Object.assign({}, this.state || {}, typeof p === 'function' ? p(this.state) : p); if (!this._q){ this._q = 1; queueMicrotask(() => { this._q = 0; window.__render(); }); } }
    forceUpdate(){ window.__render(); } };
  document.addEventListener('DOMContentLoaded', () => {
    const x = document.querySelector('x-dc'); const h = x.querySelector('helmet');
    if (h) { [...h.children].forEach(c => document.head.appendChild(c)); h.remove(); }
    const t = document.createElement('template'); t.innerHTML = x.innerHTML;
    const host = document.createElement('div'); x.replaceWith(host);
    const src = document.querySelector('script[data-dc-script]').textContent;
    const C = new Function('DCLogic', src + '\n;return Component;')(window.DCLogic);
    const inst = new C({});
    const W = /^\{\{([^}]+)\}\}$/;
    const get = (p, sc) => { p = p.trim(); if (p === 'true') return true; if (p === 'false') return false; if (/^-?\d/.test(p)) return +p;
      return p.split('.').reduce((o, k) => o == null ? undefined : o[k], sc); };
    const str = (v, sc) => v.replace(/\{\{([^}]+)\}\}/g, (_, p) => { const r = get(p, sc); return r == null ? '' : r; });
    function proc(n, sc){
      if (n.nodeType === 3) return [document.createTextNode(str(n.nodeValue, sc))];
      if (n.nodeType !== 1) return [];
      const tag = n.localName;
      const kids = (node, s) => { const o = []; node.childNodes.forEach(c => o.push(...proc(c, s))); return o; };
      if (tag === 'sc-for') { const l = get(n.getAttribute('list').match(W)[1], sc) || []; const as = n.getAttribute('as'); const o = [];
        l.forEach((it, i) => { const s = Object.assign(Object.create(sc), { [as]: it, $index: i }); o.push(...kids(n, s)); }); return o; }
      if (tag === 'sc-if') return get(n.getAttribute('value').match(W)[1], sc) ? kids(n, sc) : [];
      const el = document.createElementNS(n.namespaceURI, n.localName);
      for (const a of n.attributes) {
        if (a.name.startsWith('hint-')) continue;
        const m = a.value.match(W);
        if (m) { const v = get(m[1], sc);
          if (/^on/.test(a.name) && typeof v === 'function') { const ev = a.name.slice(2).toLowerCase(); el.__h = el.__h || {}; el.__h[ev] = v; continue; }
          if (a.name === 'ref') { if (typeof v === 'function') el.__ref = v; continue; }
          el.setAttribute(a.name, v == null ? '' : v);
        } else el.setAttribute(a.name, str(a.value, sc));
      }
      kids(tag === 'template' ? n.content : n, sc).forEach(c => el.appendChild(c));
      return [el];
    }
    window.__render = () => { const vals = inst.renderVals(); const out = []; t.content.childNodes.forEach(c => out.push(...proc(c, vals))); patchKids(host, out); if (inst.update) inst.update(); };
    // Mise à jour en place, comme un vrai moteur : les éléments (et leurs animations, leur défilement) survivent.
    const EVS = ['click', 'input', 'change', 'keydown', 'scroll', 'pointerdown'];
    function bind(el) {
      if (el.nodeType !== 1 || el.__bound) return; el.__bound = 1; el.__st = el.getAttribute('style');
      EVS.forEach((ev) => el.addEventListener(ev, (e) => { const h = el.__h && el.__h[ev]; if (h) h(e); }));
    }
    function patch(old, nu) {
      if (old.nodeType !== nu.nodeType || (old.nodeType === 1 && (old.namespaceURI !== nu.namespaceURI || old.localName !== nu.localName))) {
        bindTree(nu); old.replaceWith(nu); runRefs(nu); return;
      }
      if (old.nodeType === 3) { if (old.nodeValue !== nu.nodeValue) old.nodeValue = nu.nodeValue; return; }
      if (old.nodeType !== 1) return;
      for (const at of [...old.attributes]) {
        if (!nu.hasAttribute(at.name) && at.name !== 'style') old.removeAttribute(at.name);
      }
      for (const at of nu.attributes) {
        if (at.name === 'style') { if (old.__st !== at.value) { old.setAttribute('style', at.value); old.__st = at.value; } continue; }
        if (old.getAttribute(at.name) !== at.value) old.setAttribute(at.name, at.value);
      }
      if (old.localName === 'input' && old !== document.activeElement && nu.hasAttribute('value')) old.value = nu.getAttribute('value');
      old.__h = nu.__h; if (nu.__ref) { old.__ref = nu.__ref; }
      patchKids(old, [...nu.childNodes]);
      if (old.__ref) old.__ref(old);
    }
    function bindTree(n) { if (n.nodeType === 1) { bind(n); n.querySelectorAll('*').forEach(bind); } }
    function runRefs(n) { if (n.nodeType === 1) { [n, ...n.querySelectorAll('*')].forEach((e) => e.__ref && e.__ref(e)); } }
    function patchKids(parent, list) {
      const cur = [...parent.childNodes];
      list.forEach((nu, i) => {
        if (i < cur.length) patch(cur[i], nu);
        else { bindTree(nu); parent.appendChild(nu); runRefs(nu); }
      });
      for (let i = list.length; i < cur.length; i++) cur[i].remove();
    }
    window.__render();
    if (inst.componentDidMount) inst.componentDidMount();
  });
})();
