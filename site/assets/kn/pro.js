// KyberNow, page Travailler : diaporama du héros, bascule 4:2:0 / 4:4:4, usages épinglés et carrousel,
// console d'équipe qui se construit au défilement, droits cliquables. Textes lus dans le HTML (FR et EN).
// L'état de départ est complet dans le HTML : sans ce script, tout est visible et lisible.
(function () {
  var KN = window.KN || { reduce: false, pin: function () {}, onView: function () {} };
  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };

  // Héros : quatre scènes (photo, mot du titre, carte de l'appareil), fondu au noir au défilement en live.
  var hero = $('.hero');
  if (hero) {
    var imgs = $$('.hphoto img', hero), words = $$('.rot em', hero), bars = $$('.hcard .bars span', hero);
    var dev = $('.hcard b', hero), note = $('.hcard small', hero);
    var sc = 0, timer = 0, seen = true;
    var show = function (i) {
      sc = i % imgs.length;
      imgs.forEach(function (im, k) { im.classList.toggle('on', k === sc); });
      words.forEach(function (w, k) { w.classList.toggle('on', k === sc); if (k === sc) w.removeAttribute('aria-hidden'); else w.setAttribute('aria-hidden', 'true'); });
      bars.forEach(function (b, k) { b.classList.toggle('on', k === sc); });
      if (dev) dev.textContent = imgs[sc].getAttribute('data-dev') || '';
      if (note) note.textContent = imgs[sc].getAttribute('data-note') || '';
    };
    var run = function () {
      clearInterval(timer); timer = 0;
      if (!KN.reduce && seen && !document.hidden) timer = setInterval(function () { show(sc + 1); }, 4800);
    };
    if (imgs.length > 1) {
      if ('IntersectionObserver' in window) new IntersectionObserver(function (es) { seen = es[es.length - 1].isIntersecting; run(); }).observe(hero);
      document.addEventListener('visibilitychange', run);
      run();
    }
    KN.pin(hero, function () { return $('.hstage', hero).offsetHeight; });
  }

  // 4:2:0 / 4:4:4 : deux boutons à bascule (aria-pressed), étiquette lue dans data-label.
  var sw = $('.switch');
  if (sw) {
    var btns = $$('button', sw), dup = $('.sample .dup'), ui = $('.sample .zoom > div'), label = $('.sample .label');
    btns.forEach(function (b) {
      b.addEventListener('click', function () {
        var m420 = b.getAttribute('data-mode') === '420';
        btns.forEach(function (x) { x.setAttribute('aria-pressed', x === b ? 'true' : 'false'); });
        if (dup) dup.hidden = !m420;
        if (ui) ui.classList.toggle('lum', m420);
        if (label) label.textContent = b.getAttribute('data-label') || '';
      });
    });
  }

  // Quatre usages : épinglés en live (un usage par quart de défilement), carrousel avec points sinon.
  var uses = $('.uses');
  if (uses) {
    var items = $$('.uitem', uses), photos = $$('.uphotos img', uses), ulist = $('.ulist', uses);
    var dotsBox = $('.udots', uses), dots = dotsBox ? $$('button', dotsBox) : [];
    var curUse = -1, curLive = null;
    var setUse = function (u, live) {
      if (u === curUse && live === curLive) return;
      curUse = u; curLive = live;
      items.forEach(function (it, k) { it.classList.toggle('on', !live || k === u); });
      photos.forEach(function (im, k) { im.classList.toggle('on', k === u); });
    };
    KN.pin(uses, function () { return $('.ustage', uses).offsetHeight; }, function (p, live) {
      setUse(live ? Math.min(3, Math.floor(p * 4)) : 0, !!live);
    });
    if (dotsBox && ulist) {
      dotsBox.hidden = false;
      var mark = function (i) { dots.forEach(function (d, k) { d.setAttribute('aria-current', k === i ? 'true' : 'false'); }); };
      var raf = 0;
      ulist.addEventListener('scroll', function () {
        if (raf) return;
        raf = requestAnimationFrame(function () {
          raf = 0;
          var step = items[0].getBoundingClientRect().width + 14;
          var i = Math.round(ulist.scrollLeft / step);
          if (ulist.scrollLeft + ulist.clientWidth >= ulist.scrollWidth - 4) i = items.length - 1;
          mark(Math.max(0, Math.min(items.length - 1, i)));
        });
      }, { passive: true });
      dots.forEach(function (d, i) {
        d.addEventListener('click', function () {
          ulist.scrollTo({ left: items[i].offsetLeft - items[0].offsetLeft, behavior: KN.reduce ? 'auto' : 'smooth' });
          mark(i);
        });
      });
    }
  }

  // Console d'équipe : se construit en cinq temps au défilement en live ; complète sinon.
  var team = $('.team');
  if (team) {
    var steps = $$('.tsteps li', team), rows = $$('.prow', team), grp = $('.cside .new', team), log = $('.log', team);
    var curStep = -1;
    KN.pin(team, function () { return $('.tstage', team).offsetHeight; }, function (p, live) {
      var s = live ? Math.min(4, Math.floor(p * 5)) : 4;
      if (s === curStep) return;
      curStep = s;
      steps.forEach(function (li, k) { li.classList.toggle('on', k < s); });
      if (grp) grp.classList.toggle('off', s < 1);
      rows.forEach(function (r) { r.classList.toggle('off', s < (+r.getAttribute('data-show') || 0)); });
      if (log) log.classList.toggle('off', s < 4);
    });
    // Droits : chaque bouton s'allume ou s'éteint (aria-pressed).
    team.addEventListener('click', function (e) {
      var b = e.target.closest && e.target.closest('.rights button');
      if (!b) return;
      b.setAttribute('aria-pressed', b.getAttribute('aria-pressed') === 'true' ? 'false' : 'true');
    });
  }
})();
