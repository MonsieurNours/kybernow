// KyberNow, pages reprises de l'ancien site : trajet d'une image (épinglé en mode live), simulation QUIC,
// profils, petite démonstration de l'écran virtuel, sommaire des mentions légales.
// Chargé après base.js (window.KN). Les textes affichés viennent du HTML (attributs data-).
(function () {
  var KN = window.KN || {};
  var doc = document.documentElement;
  var $ = function (id) { return document.getElementById(id); };
  var clamp = function (v) { return Math.max(0, Math.min(1, v)); };
  var fmt = function (v) { var s = v.toFixed(2); return KN.fr ? s.replace('.', ',') : s; };

  /* ---------- Le trajet d'une image ---------- */
  var trajet = $('trajet');
  if (trajet && KN.pin) {
    var stages = [].slice.call(trajet.querySelectorAll('#stages li'));
    var counter = $('counter'), caption = $('counter-caption'), fill = $('track-fill'), pulse = $('track-pulse');
    var stageMs = stages.map(function (li) { return parseFloat(li.getAttribute('data-ms')); });
    var cumul = stageMs.map(function (_, i) { return stageMs.slice(0, i + 1).reduce(function (a, b) { return a + b; }, 0); });
    var TOTAL = parseFloat(counter.getAttribute('data-total')); // médiane du trajet complet (pas la somme des médianes)
    var capStart = caption.getAttribute('data-start'), capFinal = caption.getAttribute('data-final');
    var last = null;
    var stage = function () { return innerHeight - (parseFloat(getComputedStyle(doc).getPropertyValue('--top')) || 68); };
    KN.pin(trajet, stage, function (p, live) {
      if (!live) {
        // Repos (téléphone, animations réduites) : le trajet complet, tel qu'écrit dans le HTML.
        if (last === 'rest') return;
        last = 'rest';
        stages.forEach(function (li) { li.classList.remove('is-on', 'is-current'); });
        counter.textContent = fmt(TOTAL); caption.textContent = capFinal;
        fill.style.removeProperty('width'); pulse.style.removeProperty('left');
        trajet.classList.remove('is-live');
        return;
      }
      var a = .06, b = .86; // le trajet occupe cette part du défilement
      var t = clamp((p - a) / (b - a));
      var n = stages.length;
      var seg = Math.min(n - 1, Math.floor(t * n));
      var local = t * n - seg;
      var done = p >= b + .02;
      var ms = p < a ? 0 : done ? TOTAL : (seg ? cumul[seg - 1] : 0) + stageMs[seg] * clamp(local * 1.25);
      counter.textContent = fmt(ms);
      var idx = p < a ? -1 : done ? n : seg;
      if (idx !== last) {
        last = idx;
        stages.forEach(function (li, i) { li.classList.toggle('is-on', i <= idx); li.classList.toggle('is-current', i === idx); });
        caption.textContent = idx < 0 ? capStart : done ? capFinal : stages[idx].getAttribute('data-cap');
      }
      var w = (done ? 1 : t) * 100;
      fill.style.width = w + '%'; pulse.style.left = w + '%';
      var r = trajet.getBoundingClientRect();
      trajet.classList.toggle('is-live', p > a && p < .995 && r.top < innerHeight && r.bottom > 0);
    });
  }

  /* ---------- Simulation : QUIC contre une connexion unique ---------- */
  var canvas = $('sim');
  if (canvas && canvas.getContext) {
    var ctx = canvas.getContext('2d');
    var blockedEl = $('sim-blocked'), fixedEl = $('sim-fixed');
    var of = blockedEl.getAttribute('data-of') || ' / ';
    var lanesTxt = (canvas.getAttribute('data-lanes') || '||').split('|');
    var C = {};
    var colors = function () {
      var cs = getComputedStyle(canvas);
      var g = function (n) { return cs.getPropertyValue(n).trim(); };
      C = { pkt: g('--frost2'), line: g('--line'), band: g('--line'), faint: g('--mist'), bad: g('--bad'), warn: g('--warn'), sig: g('--sig') };
    };
    colors();
    new MutationObserver(colors).observe(doc, { attributes: true, attributeFilter: ['data-theme'] });
    var LANES = [{ rate: 11, w: 14 }, { rate: 7, w: 8 }, { rate: 4, w: 6 }, { rate: .8, w: 22 }];
    var SPEED = KN.reduce ? .12 : .24; // largeurs par seconde
    var LOSS = .03, NET = .5, GATE = .9;
    var mode = 'quic', packets = [], ghosts = [], fixedCount = 0, stall = null, W = 0, H = 0, running = false, lastT = 0;
    var acc = LANES.map(function () { return Math.random(); });
    var size = function () {
      var dpr = Math.min(devicePixelRatio || 1, 2);
      W = canvas.clientWidth; H = canvas.clientHeight;
      canvas.width = W * dpr; canvas.height = H * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    if ('ResizeObserver' in window) new ResizeObserver(size).observe(canvas); else size();
    var reset = function () { packets = []; ghosts = []; stall = null; fixedCount = 0; fixedEl.textContent = '0'; };
    var modeBtns = [].slice.call(document.querySelectorAll('.sim [data-mode]'));
    modeBtns.forEach(function (btn) {
      btn.addEventListener('click', function () {
        mode = btn.getAttribute('data-mode');
        modeBtns.forEach(function (b) { b.setAttribute('aria-pressed', String(b === btn)); });
        reset();
      });
    });
    var step = function (dt) {
      LANES.forEach(function (lane, i) {
        acc[i] += lane.rate * dt * (0.6 + Math.random() * .8);
        while (acc[i] >= 1) { acc[i] -= 1; packets.push({ lane: i, x: -.02, w: lane.w }); }
      });
      packets.forEach(function (p) {
        var prev = p.x;
        // Une connexion unique : tout ce qui arrive attend le paquet perdu (blocage en tête de file).
        if (stall && p.held) return;
        p.x += SPEED * dt;
        if (stall && !p.retx && prev < GATE && p.x >= GATE) { p.x = GATE; p.held = true; }
        if (!p.lost && !p.fixed && !p.retx && prev < NET && p.x >= NET && Math.random() < LOSS) {
          p.lost = true;
          ghosts.push({ lane: p.lane, x: NET, w: p.w, life: 1 });
          if (mode === 'quic') { p.fixed = true; fixedCount++; fixedEl.textContent = String(fixedCount); }
          else if (!stall) { stall = { lane: p.lane }; packets.push({ lane: p.lane, x: -.02, w: p.w, retx: true }); p.dead = true; }
          else p.dead = true;
        }
        if (p.retx && p.x >= GATE && stall) { stall = null; packets.forEach(function (q) { q.held = false; }); }
      });
      packets = packets.filter(function (p) { return !p.dead && p.x < 1.04; });
      ghosts.forEach(function (g) { g.life -= dt * 1.1; });
      ghosts = ghosts.filter(function (g) { return g.life > 0; });
      var blocked = stall ? 4 : 0;
      blockedEl.textContent = blocked + of + '4';
      blockedEl.classList.toggle('is-bad', blocked > 0);
    };
    var rr = function (x, y, w, h, r) { ctx.beginPath(); if (ctx.roundRect) ctx.roundRect(x, y, w, h, r); else ctx.rect(x, y, w, h); };
    var yOf = function (lane) { return 16 + lane * (H - 16) / 4 + (H - 16) / 8; };
    var draw = function () {
      ctx.clearRect(0, 0, W, H);
      ctx.font = '500 10px "Martian Mono", monospace'; ctx.fillStyle = C.faint; ctx.textBaseline = 'top';
      ctx.textAlign = 'left'; ctx.fillText(lanesTxt[0], 0, 0);
      ctx.textAlign = 'center'; ctx.fillText(lanesTxt[1], W * NET, 0);
      ctx.textAlign = 'right'; ctx.fillText(lanesTxt[2], W, 0);
      ctx.globalAlpha = .35; ctx.fillStyle = C.band; ctx.fillRect(W * (NET - .06), 16, W * .12, H - 16); ctx.globalAlpha = 1;
      ctx.strokeStyle = stall ? C.warn : C.line; ctx.lineWidth = stall ? 2 : 1;
      ctx.beginPath(); ctx.moveTo(W * GATE + 10, 16); ctx.lineTo(W * GATE + 10, H); ctx.stroke();
      for (var i = 0; i < 4; i++) {
        ctx.strokeStyle = C.line; ctx.lineWidth = 1; ctx.setLineDash([2, 6]);
        ctx.beginPath(); ctx.moveTo(0, yOf(i)); ctx.lineTo(W, yOf(i)); ctx.stroke(); ctx.setLineDash([]);
      }
      ghosts.forEach(function (g) {
        ctx.globalAlpha = Math.max(0, g.life); ctx.strokeStyle = C.bad; ctx.lineWidth = 1.5;
        rr(W * g.x - g.w / 2, yOf(g.lane) - 4, g.w, 8, 2); ctx.stroke();
        ctx.beginPath(); ctx.moveTo(W * g.x - 4, yOf(g.lane) - 12); ctx.lineTo(W * g.x + 4, yOf(g.lane) - 4);
        ctx.moveTo(W * g.x + 4, yOf(g.lane) - 12); ctx.lineTo(W * g.x - 4, yOf(g.lane) - 4); ctx.stroke();
      });
      ctx.globalAlpha = 1;
      var held = [0, 0, 0, 0];
      packets.forEach(function (p) {
        if (p.lost && !p.fixed) return;
        var x = W * p.x;
        if (p.held) { x = W * GATE - held[p.lane] * (p.w + 3); held[p.lane]++; }
        ctx.fillStyle = p.fixed ? C.sig : p.retx ? C.warn : C.pkt;
        ctx.globalAlpha = p.held ? .55 : 1;
        rr(x - p.w / 2, yOf(p.lane) - 3, p.w, 6, 2); ctx.fill();
      });
      ctx.globalAlpha = 1;
    };
    var frame = function (t) {
      if (!running) return;
      var dt = Math.min(.05, (t - (lastT || t)) / 1000); lastT = t;
      step(dt); draw();
      requestAnimationFrame(frame);
    };
    if ('IntersectionObserver' in window) {
      new IntersectionObserver(function (es) {
        var e = es[0];
        if (e.isIntersecting && !running) { running = true; lastT = 0; requestAnimationFrame(frame); }
        else if (!e.isIntersecting) running = false;
      }).observe(canvas);
    } else { running = true; requestAnimationFrame(frame); }
  }

  /* ---------- Onglets (profils) et boutons groupés : flèches gauche et droite ---------- */
  var tabs = [].slice.call(document.querySelectorAll('.profiles [role="tab"]'));
  if (tabs.length) {
    var select = function (tab, focus) {
      tabs.forEach(function (t) {
        var on = t === tab;
        t.setAttribute('aria-selected', String(on));
        t.tabIndex = on ? 0 : -1;
        var panel = $(t.getAttribute('aria-controls'));
        if (panel) {
          panel.hidden = !on;
          if (on) { panel.classList.remove('is-swapping'); void panel.offsetWidth; panel.classList.add('is-swapping'); }
        }
      });
      if (focus) tab.focus();
    };
    tabs.forEach(function (t) {
      var panel = $(t.getAttribute('aria-controls'));
      if (panel && t.getAttribute('aria-selected') !== 'true') panel.hidden = true;
      t.addEventListener('click', function () { select(t, false); });
    });
  }
  document.querySelectorAll('.segmented').forEach(function (group) {
    group.addEventListener('keydown', function (e) {
      if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
      var btns = [].slice.call(group.querySelectorAll('button'));
      var i = btns.indexOf(document.activeElement);
      if (i < 0) return;
      e.preventDefault();
      var next = btns[(i + (e.key === 'ArrowRight' ? 1 : btns.length - 1)) % btns.length];
      next.focus(); next.click();
    });
  });

  /* ---------- Écran virtuel : les définitions s'emboîtent à l'arrivée ---------- */
  var feature = document.querySelector('.card-feature');
  if (feature && KN.onView) KN.onView(feature, function () { feature.classList.add('is-in'); }, .4);

  /* ---------- Sommaire des mentions légales : la rubrique lue ---------- */
  var toc = [].slice.call(document.querySelectorAll('.toc a[href^="#"]'));
  if (toc.length && 'IntersectionObserver' in window) {
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (e) {
        if (!e.isIntersecting) return;
        toc.forEach(function (a) {
          var on = a.getAttribute('href') === '#' + e.target.id;
          a.classList.toggle('is-active', on);
          if (on) a.setAttribute('aria-current', 'true'); else a.removeAttribute('aria-current');
        });
      });
    }, { rootMargin: '-40% 0px -55% 0px' });
    toc.forEach(function (a) { var el = $(a.getAttribute('href').slice(1)); if (el) io.observe(el); });
  }
})();
