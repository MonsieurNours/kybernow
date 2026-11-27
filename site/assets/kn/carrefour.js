// KyberNow, page carrefour (/ et /en/) : héros épinglé, trajet d'une image, repères des carrousels.
// Sans ce script, la page reste dans son état « au repos » : symbole ouvert, compteur à 9,4 ms, six étapes allumées.
(function () {
  var KN = window.KN;
  if (!KN) return;
  var doc = document.documentElement;
  var comma = KN.fr ? ',' : '.';

  // Héros : KN.pin pose --p (0 en haut, 1 symbole séparé) ; les raccourcis s'effacent, on les sort aussi du clavier.
  var split = document.querySelector('.split');
  var stage = split && split.querySelector('.stage');
  var cue = split && split.querySelector('.cue');
  if (split && stage) {
    KN.pin(split, function () { return stage.offsetHeight; }, function (p, live) {
      if (cue) cue.style.visibility = live && p > 0.3 ? 'hidden' : '';
    });
  }

  // Trajet d'une image
  var trip = document.querySelector('.trip');
  if (!trip) return;
  var tst = trip.querySelector('.tstage');
  var box = trip.querySelector('.counter');
  var num = box && box.querySelector('b');
  var items = [].slice.call(trip.querySelectorAll('.st'));
  var ENDS = [0, 5.19, 5.31, 5.97, 8.47, 9.09]; // fin de chaque étape, cumul des médianes (ms)
  var last = '';

  function show(tenths, on, cur) {
    var key = tenths + '|' + on + '|' + cur;
    if (key === last) return;
    last = key;
    if (num) num.textContent = Math.floor(tenths / 10) + comma + (tenths % 10);
    items.forEach(function (li, i) {
      li.classList.toggle('on', i <= on);
      li.classList.toggle('cur', i === cur);
    });
  }

  // Téléphone : la frise se rejoue quand le compteur arrive à l'écran, sans épinglage.
  var m = { u: null, armed: true, raf: 0, playing: false };
  function setU(u) {
    m.u = u;
    if (tst) tst.style.setProperty('--p', u.toFixed(3));
    var x = u * 9.4, on = -1;
    ENDS.forEach(function (e, i) { if (x >= e) on = i; });
    show(Math.round(u * 94), on, m.playing && on < 5 ? on + 1 : -1);
  }
  function stop() {
    if (m.raf) cancelAnimationFrame(m.raf);
    m.raf = 0;
    m.playing = false;
  }
  function play() {
    stop();
    var t0 = performance.now(), D = 4200;
    m.playing = true;
    var step = function (now) {
      var u = Math.min(1, (now - t0) / D);
      m.playing = u < 1;
      m.raf = u < 1 ? requestAnimationFrame(step) : 0;
      setU(u);
    };
    m.raf = requestAnimationFrame(step);
  }
  function watch() {
    if (KN.reduce) { stop(); setU(1); return; }
    var vh = innerHeight, r = trip.getBoundingClientRect(), rc = box.getBoundingClientRect();
    if (m.u === null) { m.armed = true; setU(0); }
    if (r.bottom < 0 || r.top > vh) { stop(); if (m.u !== 0) setU(0); m.armed = true; return; }
    if (m.armed && rc.top < vh * 0.72 && rc.bottom > 0) { m.armed = false; play(); }
  }

  KN.pin(trip, function () { return tst ? tst.offsetHeight : 760; }, function (p, live) {
    if (live) {
      // Bureau : le défilement fait avancer l'image, étape par étape.
      stop(); m.u = null; m.armed = true;
      if (tst) tst.style.removeProperty('--p');
      show(Math.round(p * 94), Math.min(5, Math.floor(p * 6.2)), -1);
    } else if (innerWidth < 900) {
      watch();
    } else {
      // Grand écran sans animations : l'état complet.
      stop(); m.u = null;
      if (tst) tst.style.removeProperty('--p');
      show(94, 5, -1);
    }
  });

  // Carrousels du téléphone (portes, installation) : le repère suit le doigt.
  function dots(el) {
    var d = el.nextElementSibling;
    if (!d || !d.classList.contains('dots')) return;
    var f = function () {
      var n = el.children.length;
      var max = el.scrollWidth - el.clientWidth;
      var pitch = n > 1 ? el.children[1].offsetLeft - el.children[0].offsetLeft : 0;
      var i = pitch > 0 ? Math.min(n - 1, el.scrollLeft / pitch) : 0;
      if (max > 1 && el.scrollLeft >= max - 2) i = n - 1;
      d.style.setProperty('--di', i.toFixed(3));
    };
    el.addEventListener('scroll', f, { passive: true });
    window.addEventListener('resize', f);
    f();
  }
  document.querySelectorAll('.doors, .install').forEach(dots);
})();
