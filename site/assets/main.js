// www.kybernow.app — défilement, apparitions, menu, langue, trajet d'une image, simulation QUIC, profils, téléchargements.
// Partagé par toutes les pages : chaque bloc ne s'active que si ses éléments sont présents.

const root = document.documentElement;
root.classList.remove('no-js');
const EN = root.lang === 'en';
const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;
const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const num = (v, d = 2) => EN ? v.toFixed(d) : v.toFixed(d).replace('.', ',');
const $ = id => document.getElementById(id);

const T = EN ? {
  captions: [
    'The computer captures the virtual display, created for your device.',
    'The computer compresses the image in 5.19\u00a0ms.',
    'Kyber cuts it into encrypted QUIC packets.',
    'It crosses your network in under a millisecond.',
    'Your device decodes it in hardware.',
    'It shows up as soon as the screen is ready.',
  ],
  start: 'Scroll: from capture to display.',
  final: 'End to end, median, measured at 3840 × 2160 and 160 Hz.',
  of: ' of ', lanes: ['COMPUTER', 'NETWORK', 'DEVICE'], version: 'Version ', mb: ' MB',
  profiles: {
    jeu: { title: 'Lowest latency', text: 'The highest refresh rate your screen can show, relative mouse, gamepad. 4:2:0 colour to save every millisecond.', chips: ['Maximum refresh rate', 'Relative mouse', 'Gamepad'] },
    bureau: { title: 'Sharp text', text: '4:4:4 colour for text as sharp as on the computer, shared clipboard, a cursor that never hides.', chips: ['4:4:4', 'Clipboard', 'Cursor always visible'] },
    max: { title: 'No compromise', text: 'Low latency, 4:4:4 and a high bitrate, all at once. For a wired network and a recent device.', chips: ['4:4:4', 'High bitrate', 'Wired network'] },
  },
} : {
  captions: [
    "L'ordinateur capture l'écran virtuel, créé pour votre appareil.",
    'L’ordinateur compresse l’image en 5,19\u00a0ms.',
    'Kyber la découpe en paquets QUIC, chiffrés.',
    'Elle traverse votre réseau en moins d’une milliseconde.',
    'Votre appareil la décompresse, en matériel.',
    'Elle s’affiche dès que l’écran est prêt.',
  ],
  start: 'Faites défiler : de la capture à l’affichage.',
  final: 'De bout en bout, en médiane, mesuré en 3840 × 2160 à 160 Hz.',
  of: ' sur ', lanes: ['ORDINATEUR', 'RÉSEAU', 'APPAREIL'], version: 'Version ', mb: ' Mo',
  profiles: {
    jeu: { title: 'Latence minimale', text: 'La fréquence la plus haute que votre écran affiche, la souris relative, la manette. Les couleurs en 4:2:0 pour gagner chaque milliseconde.', chips: ['Fréquence maximale', 'Souris relative', 'Manette'] },
    bureau: { title: 'Texte net', text: 'Les couleurs en 4:4:4 pour un texte aussi net que sur l’ordinateur, le presse-papiers partagé, un curseur toujours visible.', chips: ['4:4:4', 'Presse-papiers', 'Curseur toujours visible'] },
    max: { title: 'Aucun compromis', text: 'Faible latence, 4:4:4 et débit élevé à la fois. Pour un réseau câblé et un appareil récent.', chips: ['4:4:4', 'Débit élevé', 'Réseau câblé'] },
  },
};

/* ---------- Langue : le choix fait au sélecteur est retenu (voir lang.js) ---------- */
document.querySelectorAll('[data-set-lang]').forEach(a => a.addEventListener('click', () => {
  try { localStorage.setItem('kn-lang', a.dataset.setLang); } catch (e) {}
}));

/* ---------- Titres découpés en mots ---------- */
for (const el of document.querySelectorAll('[data-split]')) {
  let i = 0;
  const walk = node => {
    for (const child of [...node.childNodes]) {
      if (child.nodeType !== Node.TEXT_NODE) { if (child.nodeName !== 'BR') walk(child); continue; }
      const frag = document.createDocumentFragment();
      child.textContent.split(/( +)/).forEach(part => {
        if (!part) return;
        if (/^ +$/.test(part)) { frag.append(' '); return; }
        const w = document.createElement('span'); w.className = 'w';
        const inner = document.createElement('span'); inner.style.setProperty('--i', i++); inner.textContent = part;
        w.append(inner); frag.append(w);
      });
      child.replaceWith(frag);
    }
  };
  walk(el);
}

/* ---------- Apparitions ---------- */
const io = new IntersectionObserver(entries => {
  for (const e of entries) if (e.isIntersecting) { e.target.classList.add('is-in'); io.unobserve(e.target); }
}, { rootMargin: '0px 0px -8% 0px', threshold: .12 });
document.querySelectorAll('[data-reveal], [data-split]').forEach(el => io.observe(el));

/* ---------- Navigation : fond au défilement, section active, menu mobile ---------- */
const nav = $('nav');
const sectionLinks = [...document.querySelectorAll('.nav-links a, .toc a')].filter(a => a.hash);
const navIo = new IntersectionObserver(entries => {
  for (const e of entries) if (e.isIntersecting) {
    sectionLinks.forEach(a => a.classList.toggle('is-active', a.hash === '#' + e.target.id && a.pathname === location.pathname));
  }
}, { rootMargin: '-45% 0px -50% 0px' });
new Set(sectionLinks.map(a => a.hash.slice(1))).forEach(id => { const el = $(id); if (el) navIo.observe(el); });

const burger = document.querySelector('.nav-burger');
const menu = $('menu');
if (burger && menu) {
  const setMenu = open => {
    burger.setAttribute('aria-expanded', String(open));
    menu.hidden = !open;
    root.classList.toggle('menu-open', open);
  };
  burger.addEventListener('click', () => setMenu(burger.getAttribute('aria-expanded') !== 'true'));
  menu.addEventListener('click', e => { if (e.target.closest('a')) setMenu(false); });
  addEventListener('keydown', e => { if (e.key === 'Escape') setMenu(false); });
  matchMedia('(min-width: 1101px)').addEventListener('change', e => { if (e.matches) setMenu(false); });
}

/* ---------- Le trajet d'une image (section épinglée) ---------- */
const trajet = $('trajet');
let updateTrajet = () => {};
if (trajet) {
  const stages = [...document.querySelectorAll('#stages li')];
  const counter = $('counter'), caption = $('counter-caption'), fill = $('track-fill'), pulse = $('track-pulse');
  const stageMs = stages.map(li => parseFloat(li.dataset.ms));
  const cumul = stageMs.map((_, i) => stageMs.slice(0, i + 1).reduce((a, b) => a + b, 0));
  const TOTAL = 9.36; // médiane du trajet complet (pas la somme des médianes)
  let lastStage = -2;
  updateTrajet = () => {
    const r = trajet.getBoundingClientRect();
    const span = trajet.offsetHeight - innerHeight;
    const p = clamp(-r.top / span);
    window.knTrajet = p; // lu par la scène 3D
    const a = .06, b = .86; // le trajet occupe cette part du défilement
    const t = clamp((p - a) / (b - a));
    const n = stages.length;
    const seg = Math.min(n - 1, Math.floor(t * n));
    const local = t * n - seg;
    const done = p >= b + .02;
    const ms = p < a ? 0 : done ? TOTAL : (seg ? cumul[seg - 1] : 0) + stageMs[seg] * clamp(local * 1.25);
    counter.textContent = num(ms);
    const stageIndex = p < a ? -1 : done ? n : seg;
    if (stageIndex !== lastStage) {
      lastStage = stageIndex;
      stages.forEach((li, i) => {
        li.classList.toggle('is-on', i <= stageIndex);
        li.classList.toggle('is-current', i === stageIndex);
      });
      caption.textContent = stageIndex < 0 ? T.start : done ? T.final : T.captions[stageIndex];
    }
    const w = (done ? 1 : t) * 100;
    fill.style.width = w + '%';
    pulse.style.left = w + '%';
    trajet.classList.toggle('is-live', p > a && p < .995 && r.top < innerHeight && r.bottom > 0);
  };
}

let ticking = false;
function onScroll() {
  if (ticking) return;
  ticking = true;
  requestAnimationFrame(() => {
    ticking = false;
    nav?.classList.toggle('is-scrolled', scrollY > 24);
    updateTrajet();
  });
}
addEventListener('scroll', onScroll, { passive: true });
addEventListener('resize', onScroll);
onScroll();

/* ---------- Simulation : QUIC contre un seul flux ---------- */
const canvas = $('sim');
if (canvas) {
  const ctx = canvas.getContext('2d');
  const blockedEl = $('sim-blocked');
  const fixedEl = $('sim-fixed');
  const css = getComputedStyle(root);
  const C = {
    pkt: css.getPropertyValue('--kn-frost-2').trim(), line: css.getPropertyValue('--kn-line').trim(),
    faint: css.getPropertyValue('--kn-faint').trim(), bad: css.getPropertyValue('--kn-bad').trim(),
    warn: css.getPropertyValue('--kn-warn').trim(), sig: css.getPropertyValue('--kn-signal').trim(),
  };
  const LANES = [{ rate: 11, w: 14 }, { rate: 7, w: 8 }, { rate: 4, w: 6 }, { rate: .8, w: 22 }];
  const SPEED = reduceMotion ? .12 : .24; // largeurs par seconde
  const LOSS = .03, NET = .5, GATE = .9;
  let mode = 'quic', packets = [], ghosts = [], fixedCount = 0, stall = null, W = 0, H = 0, running = false, last = 0;
  const acc = LANES.map(() => Math.random());

  const size = () => {
    const dpr = Math.min(devicePixelRatio || 1, 2);
    W = canvas.clientWidth; H = canvas.clientHeight;
    canvas.width = W * dpr; canvas.height = H * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  };
  new ResizeObserver(size).observe(canvas);

  const reset = () => { packets = []; ghosts = []; stall = null; fixedCount = 0; fixedEl.textContent = '0'; };
  document.querySelectorAll('.sim [data-mode]').forEach(btn => btn.addEventListener('click', () => {
    mode = btn.dataset.mode;
    document.querySelectorAll('.sim [data-mode]').forEach(b => b.setAttribute('aria-checked', String(b === btn)));
    reset();
  }));

  const step = dt => {
    LANES.forEach((lane, i) => {
      acc[i] += lane.rate * dt * (0.6 + Math.random() * .8);
      while (acc[i] >= 1) { acc[i] -= 1; packets.push({ lane: i, x: -.02, w: lane.w, lost: false, fixed: false, retx: false, held: false }); }
    });
    for (const p of packets) {
      const prev = p.x;
      // Un seul flux : tout ce qui arrive attend le paquet perdu (blocage en tête de file).
      if (stall && p.held) continue;
      p.x += SPEED * dt;
      if (stall && !p.retx && prev < GATE && p.x >= GATE) { p.x = GATE; p.held = true; }
      if (!p.lost && !p.fixed && !p.retx && prev < NET && p.x >= NET && Math.random() < LOSS) {
        p.lost = true;
        ghosts.push({ lane: p.lane, x: NET, w: p.w, life: 1 });
        if (mode === 'quic') { p.fixed = true; fixedCount++; fixedEl.textContent = String(fixedCount); }
        else if (!stall) { stall = { lane: p.lane }; packets.push({ lane: p.lane, x: -.02, w: p.w, retx: true }); p.dead = true; }
        else p.dead = true;
      }
      if (p.retx && p.x >= GATE && stall) { stall = null; packets.forEach(q => q.held = false); }
    }
    packets = packets.filter(p => !p.dead && p.x < 1.04);
    ghosts.forEach(g => g.life -= dt * 1.1); ghosts = ghosts.filter(g => g.life > 0);
    const blocked = stall ? 4 : 0;
    blockedEl.textContent = blocked + T.of + '4';
    blockedEl.classList.toggle('is-bad', blocked > 0);
  };

  const rr = (x, y, w, h, r) => { ctx.beginPath(); ctx.roundRect(x, y, w, h, r); };
  const draw = () => {
    ctx.clearRect(0, 0, W, H);
    ctx.font = '500 10px "Martian Mono", monospace'; ctx.fillStyle = C.faint; ctx.textBaseline = 'top';
    ctx.textAlign = 'left'; ctx.fillText(T.lanes[0], 0, 0);
    ctx.textAlign = 'center'; ctx.fillText(T.lanes[1], W * NET, 0);
    ctx.textAlign = 'right'; ctx.fillText(T.lanes[2], W, 0);
    // bande du réseau, porte d'arrivée
    ctx.fillStyle = 'rgba(227,237,240,.025)'; ctx.fillRect(W * (NET - .06), 16, W * .12, H - 16);
    ctx.strokeStyle = stall ? C.warn : C.line; ctx.lineWidth = stall ? 2 : 1;
    ctx.beginPath(); ctx.moveTo(W * GATE + 10, 16); ctx.lineTo(W * GATE + 10, H); ctx.stroke();
    const yOf = lane => 16 + lane * (H - 16) / 4 + (H - 16) / 8;
    for (let i = 0; i < 4; i++) {
      ctx.strokeStyle = C.line; ctx.lineWidth = 1; ctx.setLineDash([2, 6]);
      ctx.beginPath(); ctx.moveTo(0, yOf(i)); ctx.lineTo(W, yOf(i)); ctx.stroke(); ctx.setLineDash([]);
    }
    for (const g of ghosts) {
      ctx.globalAlpha = g.life; ctx.strokeStyle = C.bad; ctx.lineWidth = 1.5;
      rr(W * g.x - g.w / 2, yOf(g.lane) - 4, g.w, 8, 2); ctx.stroke();
      ctx.beginPath(); ctx.moveTo(W * g.x - 4, yOf(g.lane) - 12); ctx.lineTo(W * g.x + 4, yOf(g.lane) - 4);
      ctx.moveTo(W * g.x + 4, yOf(g.lane) - 12); ctx.lineTo(W * g.x - 4, yOf(g.lane) - 4); ctx.stroke();
    }
    ctx.globalAlpha = 1;
    // paquets retenus : empilés contre la porte
    const heldPerLane = [0, 0, 0, 0];
    for (const p of packets) {
      if (p.lost && !p.fixed) continue;
      let x = W * p.x;
      if (p.held) { x = W * GATE - heldPerLane[p.lane] * (p.w + 3); heldPerLane[p.lane]++; }
      ctx.fillStyle = p.fixed ? C.sig : p.retx ? C.warn : C.pkt;
      ctx.globalAlpha = p.held ? .55 : 1;
      rr(x - p.w / 2, yOf(p.lane) - 3, p.w, 6, 2); ctx.fill();
    }
    ctx.globalAlpha = 1;
  };

  const frame = t => {
    if (!running) return;
    const dt = Math.min(.05, (t - (last || t)) / 1000); last = t;
    step(dt); draw();
    requestAnimationFrame(frame);
  };
  new IntersectionObserver(([e]) => {
    if (e.isIntersecting && !running) { running = true; last = 0; requestAnimationFrame(frame); }
    else if (!e.isIntersecting) running = false;
  }).observe(canvas);
}

/* ---------- Profils ---------- */
const panel = $('profile-panel');
const profileBtns = [...document.querySelectorAll('[data-profile]')];
if (panel) profileBtns.forEach(btn => btn.addEventListener('click', () => {
  profileBtns.forEach(b => b.setAttribute('aria-checked', String(b === btn)));
  const d = T.profiles[btn.dataset.profile];
  panel.querySelector('[data-p="title"]').textContent = d.title;
  panel.querySelector('[data-p="text"]').textContent = d.text;
  panel.querySelector('[data-p="chips"]').innerHTML = d.chips.map(c => `<li>${c}</li>`).join('');
  panel.classList.remove('is-swapping'); void panel.offsetWidth; panel.classList.add('is-swapping');
}));
document.querySelectorAll('.segmented').forEach(group => group.addEventListener('keydown', e => {
  if (!['ArrowLeft', 'ArrowRight'].includes(e.key)) return;
  const btns = [...group.querySelectorAll('button')];
  const i = btns.indexOf(document.activeElement);
  const next = btns[(i + (e.key === 'ArrowRight' ? 1 : btns.length - 1)) % btns.length];
  next.focus(); next.click();
}));

/* ---------- Reflet des cartes ---------- */
document.querySelectorAll('.card').forEach(card => card.addEventListener('pointermove', e => {
  const r = card.getBoundingClientRect();
  card.style.setProperty('--mx', e.clientX - r.left + 'px');
  card.style.setProperty('--my', e.clientY - r.top + 'px');
}));

/* ---------- Téléchargements : le bouton principal suit l'appareil du visiteur ---------- */
if (document.querySelector('.dl-card')) {
  const ua = navigator.userAgent;
  const touch = navigator.maxTouchPoints > 1;
  const isMac = /Macintosh|Mac OS X/.test(ua) && !touch;
  const isWindows = /Windows/.test(ua);
  // Mac : l'app ; Windows : l'installateur ; tablette, téléphone et le reste : le navigateur
  const target = isMac ? 'mac' : isWindows ? 'windows' : 'web';
  const card = document.querySelector(`.dl-card[data-os="${target}"]`);
  if (card) {
    card.classList.add('is-primary');
    const btn = card.querySelector('.btn');
    btn.classList.remove('btn-ghost'); btn.classList.add('kn-primary');
  }
}

/* ---------- Dernière version publiée (si GitHub répond) ---------- */
// Taille telle que le système de l'appareil l'affichera une fois le fichier
// téléchargé : Windows compte en 1 048 576 octets (et tronque), macOS en
// millions d'octets. Une décimale, à la française en français.
function size(asset) {
  const win = /\.exe$/i.test(asset.name);
  const v = win ? Math.floor(asset.size / 1048576 * 10) / 10 : Math.round(asset.size / 1e6 * 10) / 10;
  return v.toLocaleString(EN ? 'en' : 'fr', { maximumFractionDigits: 1 }) + T.mb;
}
if ($('release-version')) {
  fetch('https://api.github.com/repos/MonsieurNours/kybernow/releases/latest', { headers: { Accept: 'application/vnd.github+json' } })
    .then(r => r.ok ? r.json() : Promise.reject())
    .then(rel => {
      $('release-version').textContent = T.version + rel.tag_name.replace(/^v/, '');
      for (const li of document.querySelectorAll('[data-size]')) {
        const asset = rel.assets.find(a => a.name === li.dataset.size);
        if (asset) li.textContent = size(asset);
      }
    })
    .catch(() => {});
}
