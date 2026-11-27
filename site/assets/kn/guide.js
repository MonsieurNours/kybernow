// Guide : recherche et rubriques (accueil), FAQ (filtres et questions dépliables), article (sommaire, barre de lecture, avis).
(function () {
  var KN = window.KN || { fr: document.documentElement.lang !== 'en' };
  var mob = window.matchMedia ? matchMedia('(max-width: 900px)') : { matches: false };
  var reduce = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;

  // Questions dépliables (accueil du guide et FAQ)
  document.addEventListener('click', function (e) {
    var b = e.target.closest && e.target.closest('.qa > button');
    if (!b) return;
    var qa = b.parentElement, open = !qa.classList.contains('open');
    qa.classList.toggle('open', open);
    b.setAttribute('aria-expanded', open ? 'true' : 'false');
  });

  // ---- Accueil du guide
  var q = document.getElementById('q');
  if (q) {
    var norm = function (x) { return (x || '').toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, ''); };
    var rubs = [].slice.call(document.querySelectorAll('.rub'));
    var empty = document.querySelector('#rubriques .empty');
    var pathsec = document.querySelector('.pathsec');
    var rubsec = document.getElementById('rubriques');
    var filter = function () {
      var v = norm(q.value).trim(), any = false;
      rubs.forEach(function (r) {
        var n = 0, title = norm(r.querySelector('h3').textContent);
        r.querySelectorAll('li').forEach(function (li) {
          var a = li.querySelector('a');
          var hit = !v || norm(a.textContent + ' ' + (a.getAttribute('data-words') || '') + ' ' + title).indexOf(v) !== -1;
          li.hidden = !hit; if (hit) n++;
        });
        r.classList.toggle('hide', !n);
        if (v) setRub(r, true);
        var c = r.querySelector('.cnt'); if (c) c.textContent = String(n).padStart(2, '0');
        if (n) any = true;
      });
      if (empty) empty.hidden = !v || any;
      if (pathsec) pathsec.classList.toggle('searching', !!v);
      if (rubsec) rubsec.classList.toggle('found', !!v);
    };
    q.addEventListener('input', filter);
    document.querySelectorAll('.chipsq [data-q]').forEach(function (b) {
      b.addEventListener('click', function () { q.value = b.getAttribute('data-q'); filter(); q.focus(); });
    });
    document.addEventListener('keydown', function (e) {
      if (e.key === '/' && document.activeElement !== q && !/input|textarea/i.test((document.activeElement || {}).tagName || '')) { e.preventDefault(); q.focus(); }
    });
    // Rubriques dépliables sur téléphone
    var setRub = function (r, open) {
      r.classList.toggle('open', open);
      var bt = r.querySelector('.rt'); if (bt) bt.setAttribute('aria-expanded', open ? 'true' : 'false');
    };
    rubs.forEach(function (r) {
      var bt = r.querySelector('.rt');
      if (bt) bt.addEventListener('click', function () { if (mob.matches) setRub(r, !r.classList.contains('open')); });
    });
    // Repère du carrousel des parcours (téléphone)
    var paths = document.querySelector('.paths'), dot = document.querySelector('.pdots i');
    if (paths && dot) paths.addEventListener('scroll', function () {
      var max = paths.scrollWidth - paths.clientWidth;
      dot.style.setProperty('--x', max > 0 ? (paths.scrollLeft / max).toFixed(3) : '0');
    }, { passive: true });
  }

  // ---- FAQ : filtres par thème
  var groups = document.querySelectorAll('.fgroups button, .fbar button');
  if (groups.length) {
    var all = groups[0].getAttribute('data-g');
    var pick = function (g) {
      groups.forEach(function (b) { var on = b.getAttribute('data-g') === g; b.classList.toggle('on', on); b.setAttribute('aria-pressed', on ? 'true' : 'false'); });
      document.querySelectorAll('.flist [data-g]').forEach(function (x) { x.hidden = g !== all && x.getAttribute('data-g') !== g; });
      var list = document.querySelector('.flist');
      if (list && mob.matches) {
        var top = list.getBoundingClientRect().top;
        if (top < 130) window.scrollTo({ top: Math.max(0, top + scrollY - 130), behavior: reduce ? 'auto' : 'smooth' });
      }
    };
    groups.forEach(function (b) { b.addEventListener('click', function () { pick(b.getAttribute('data-g')); }); });
    if (location.hash) { var t = document.getElementById(location.hash.slice(1)); if (t && t.classList.contains('qa')) { t.classList.add('open'); var tb = t.querySelector('button'); if (tb) tb.setAttribute('aria-expanded', 'true'); } }
  }

  // ---- Article : barre de lecture, sommaire qui suit la lecture, avis
  var prose = document.querySelector('.prose');
  if (prose) {
    var bar = document.querySelector('.progress');
    var toc = document.querySelector('.toc');
    var heads = [].slice.call(prose.querySelectorAll('h2[id]'));
    var links = toc ? [].slice.call(toc.querySelectorAll('.toclist a')) : [];
    var now = toc && toc.querySelector('.tocnow'), pos = toc && toc.querySelector('.tocpos');
    var btn = toc && toc.querySelector('.tocbtn');
    var pad = function (n) { return String(n).padStart(2, '0'); };
    var cur = -1, raf = 0;
    var tick = function () {
      raf = 0;
      var se = document.scrollingElement || document.documentElement;
      var max = se.scrollHeight - innerHeight;
      if (bar) bar.style.setProperty('--read', max > 40 ? Math.min(1, se.scrollTop / max).toFixed(3) : '0');
      var line = mob.matches ? 150 : 130, k = 0;
      heads.forEach(function (h, i) { if (h.getBoundingClientRect().top - line <= 0) k = i; });
      if (k !== cur && links.length) {
        cur = k;
        links.forEach(function (a, i) { a.classList.toggle('on', i === k); if (i === k) a.setAttribute('aria-current', 'location'); else a.removeAttribute('aria-current'); });
        if (now) now.textContent = heads[k] ? heads[k].textContent : '';
        if (pos) pos.textContent = pad(k + 1) + ' / ' + pad(heads.length);
      }
    };
    window.addEventListener('scroll', function () { if (!raf) raf = requestAnimationFrame(tick); }, { passive: true });
    tick();
    var setToc = function (open) { if (!toc) return; toc.classList.toggle('open', open); if (btn) btn.setAttribute('aria-expanded', open ? 'true' : 'false'); };
    if (btn) btn.addEventListener('click', function () { setToc(!toc.classList.contains('open')); });
    if (toc) {
      toc.addEventListener('click', function (e) { if (e.target.closest('.toclist a') || e.target.closest('.tocscrim')) setToc(false); });
      document.addEventListener('keydown', function (e) { if (e.key === 'Escape') setToc(false); });
    }
    var thx = document.querySelector('.helpful .hthx');
    document.querySelectorAll('.helpful [data-vote]').forEach(function (b) {
      b.addEventListener('click', function () {
        document.querySelectorAll('.helpful [data-vote]').forEach(function (x) { x.classList.toggle('on', x === b); x.setAttribute('aria-pressed', x === b ? 'true' : 'false'); });
        if (thx) thx.textContent = KN.fr ? 'Merci, c’est noté.' : 'Thanks, noted.';
      });
    });
  }
})();
