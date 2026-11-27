// KyberNow, page Jouer : diaporama du héros (et son épinglage en mode live), choix de l'appareil.
// Tous les textes sont dans le HTML ; ce script ne fait que les montrer à tour de rôle.
(function () {
  var KN = window.KN || {};
  var doc = document.documentElement;
  var reduce = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ===== Héros : cinq scènes (photo, mot du titre, carte avec barres) =====
  var hero = document.querySelector('.hero');
  if (hero) {
    var stage = hero.querySelector('.hstage');
    var imgs = [].slice.call(hero.querySelectorAll('.hphoto img'));
    var words = [].slice.call(hero.querySelectorAll('.rot em'));
    var bars = [].slice.call(hero.querySelectorAll('.hcard .bars span'));
    var devEl = hero.querySelector('.hcard b'), noteEl = hero.querySelector('.hcard small');
    var n = imgs.length, cur = 0, timer = 0, inView = true;

    var show = function (i) {
      var prev = cur;
      cur = (i + n) % n;
      imgs.forEach(function (im, k) {
        im.classList.remove('was');
        if (k === prev && k !== cur) im.classList.add('was');
        im.classList.toggle('on', k === cur);
      });
      words.forEach(function (w, k) { w.classList.toggle('on', k === cur); });
      bars.forEach(function (b, k) {
        // On repose la classe pour relancer le remplissage de la barre (téléphone).
        b.classList.remove('on');
        if (k === cur) { void b.offsetWidth; b.classList.add('on'); }
      });
      var im = imgs[cur];
      if (devEl && im.dataset.dev) devEl.textContent = im.dataset.dev;
      if (noteEl && im.dataset.note) noteEl.textContent = im.dataset.note;
    };
    var schedule = function () {
      clearTimeout(timer);
      if (reduce || !inView || document.hidden) return;
      timer = setTimeout(function () { show(cur + 1); schedule(); }, 4800);
    };
    // Les photos suivantes ne se chargent qu'une fois la première affichée.
    var warm = function () { imgs.forEach(function (im) { if (im.loading === 'lazy') im.loading = 'eager'; }); };
    if (document.readyState === 'complete') warm(); else window.addEventListener('load', warm);

    if (!reduce) {
      if ('IntersectionObserver' in window) {
        new IntersectionObserver(function (es) {
          var v = es[0].isIntersecting;
          if (v && !inView) { inView = true; show(cur); schedule(); }
          else if (!v) { inView = false; clearTimeout(timer); }
        }, { threshold: 0.15 }).observe(stage);
      }
      document.addEventListener('visibilitychange', function () {
        if (document.hidden) clearTimeout(timer); else { show(cur); schedule(); }
      });
      schedule();
    }

    // Mode live : la scène reste à l'écran, la photo s'approche et le texte s'efface vers le noir.
    if (KN.pin) KN.pin(hero, function () { return stage.offsetHeight; });
  }

  // ===== Choix de l'appareil : onglets accessibles =====
  var picker = document.querySelector('.picker');
  if (picker) {
    var tablist = picker.querySelector('[role="tablist"]');
    var tabs = [].slice.call(picker.querySelectorAll('[role="tab"]'));
    var views = picker.querySelector('.views');
    var panels = tabs.map(function (t) { return document.getElementById(t.getAttribute('aria-controls')); });

    var center = function (t) {
      if (innerWidth > 900 || !tablist) return;
      var er = tablist.getBoundingClientRect(), br = t.getBoundingClientRect();
      tablist.scrollTo({ left: tablist.scrollLeft + (br.left - er.left) - (er.width - br.width) / 2, behavior: reduce ? 'auto' : 'smooth' });
    };
    var select = function (i, focus, animate) {
      tabs.forEach(function (t, k) {
        var on = k === i;
        t.setAttribute('aria-selected', on ? 'true' : 'false');
        t.tabIndex = on ? 0 : -1;
        if (panels[k]) {
          panels[k].hidden = !on;
          panels[k].classList.remove('swap');
          if (on && animate && !reduce) { void panels[k].offsetWidth; panels[k].classList.add('swap'); }
        }
      });
      if (focus) tabs[i].focus();
      center(tabs[i]);
    };
    tabs.forEach(function (t, i) {
      t.addEventListener('click', function () { if (t.getAttribute('aria-selected') !== 'true') select(i, false, true); });
      t.addEventListener('keydown', function (e) {
        var k = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key], to = null;
        if (k) to = (i + k + tabs.length) % tabs.length;
        else if (e.key === 'Home') to = 0;
        else if (e.key === 'End') to = tabs.length - 1;
        if (to === null) return;
        e.preventDefault();
        select(to, true, true);
      });
    });
    // Un lien vers une fiche (#app-loin…) ouvre le bon onglet.
    var fromHash = function () {
      var h = location.hash.slice(1), k = panels.findIndex(function (p) { return p && p.id === h; });
      if (k >= 0) { select(k, false, false); picker.scrollIntoView({ block: 'start' }); }
    };
    select(0, false, false);
    views.classList.add('ready');
    window.addEventListener('hashchange', fromHash);
    if (location.hash) fromHash();
  }
})();
