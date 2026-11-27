// KyberNow, comportements communs : thème, menu mobile, langue, mode « live », sections épinglées, anciennes ancres.
// Chargé en fin de <body> (ou avec defer). Expose window.KN pour les scripts de page.
(function () {
  var doc = document.documentElement;
  var reduce = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;
  var KN = window.KN = { reduce: reduce, fr: doc.lang !== 'en' };

  // Thème : bouton soleil/lune ; suit l'appareil tant que le visiteur n'a rien choisi.
  function setTheme(t, save) {
    doc.setAttribute('data-theme', t);
    var m = document.querySelector('meta[name="theme-color"]');
    if (m) m.content = t === 'light' ? '#F2F5F6' : '#05070A';
    document.querySelectorAll('.thm').forEach(function (b) {
      b.setAttribute('aria-label', t === 'light' ? (KN.fr ? 'Passer en thème sombre' : 'Switch to dark theme') : (KN.fr ? 'Passer en thème clair' : 'Switch to light theme'));
    });
    if (save) { try { localStorage.setItem('kn-theme', t); } catch (e) {} }
  }
  setTheme(doc.getAttribute('data-theme') || 'dark', false);
  document.addEventListener('click', function (e) {
    var b = e.target.closest && e.target.closest('.thm');
    if (b) setTheme(doc.getAttribute('data-theme') === 'light' ? 'dark' : 'light', true);
  });
  if (window.matchMedia) {
    var mq = matchMedia('(prefers-color-scheme: light)');
    var follow = function () { var s = null; try { s = localStorage.getItem('kn-theme'); } catch (e) {} if (!s) setTheme(mq.matches ? 'light' : 'dark', false); };
    if (mq.addEventListener) mq.addEventListener('change', follow);
  }

  // Langue : le sélecteur retient le choix (lu par /assets/lang.js).
  document.addEventListener('click', function (e) {
    var a = e.target.closest && e.target.closest('[data-set-lang]');
    if (a) { try { localStorage.setItem('kn-lang', a.getAttribute('data-set-lang')); } catch (err) {} }
  });

  // Menu mobile
  var burger = document.querySelector('.burger'), sheet = document.getElementById('menu');
  function menu(open) {
    if (!burger || !sheet) return;
    burger.setAttribute('aria-expanded', open ? 'true' : 'false');
    burger.setAttribute('aria-label', open ? (KN.fr ? 'Fermer le menu' : 'Close menu') : (KN.fr ? 'Ouvrir le menu' : 'Open menu'));
    sheet.classList.toggle('open', open);
    document.body.classList.toggle('menu-open', open);
  }
  if (burger && sheet) {
    burger.addEventListener('click', function () { menu(burger.getAttribute('aria-expanded') !== 'true'); });
    sheet.addEventListener('click', function (e) { if (e.target.closest('a')) menu(false); });
    document.addEventListener('keydown', function (e) { if (e.key === 'Escape') menu(false); });
    window.addEventListener('resize', function () { if (innerWidth > 900) menu(false); });
  }

  // Mode « live » : sections épinglées et effets liés au défilement, sur grand écran seulement.
  function isLive() { return !reduce && innerWidth >= 900; }
  function applyLive() { doc.classList.toggle('live', isLive()); }
  applyLive();
  window.addEventListener('resize', applyLive);

  // KN.pin(section, hauteurDeScène, rappel(p)) : progression 0..1 d'une section épinglée, posée en --p.
  var pins = [];
  KN.pin = function (el, stage, cb) { if (el) pins.push({ el: el, stage: stage, cb: cb }); tick(); };
  var raf = 0;
  function tick() {
    if (raf) return;
    raf = requestAnimationFrame(function () {
      raf = 0;
      var live = doc.classList.contains('live');
      var top = parseFloat(getComputedStyle(doc).getPropertyValue('--top')) || 68;
      pins.forEach(function (o) {
        var p;
        if (!live) { o.el.style.removeProperty('--p'); p = null; }
        else {
          var r = o.el.getBoundingClientRect();
          var stage = typeof o.stage === 'function' ? o.stage() : o.stage;
          p = Math.max(0, Math.min(1, (top - r.top) / Math.max(1, r.height - stage)));
          o.el.style.setProperty('--p', p.toFixed(3));
        }
        if (o.cb) o.cb(p, live);
      });
    });
  }
  window.addEventListener('scroll', tick, { passive: true });
  window.addEventListener('resize', tick);

  // KN.onView(el, rappel, seuil) : rappel quand l'élément arrive à l'écran (une fois par entrée).
  KN.onView = function (el, cb, ratio) {
    if (!el || !('IntersectionObserver' in window)) { if (el) cb(); return; }
    var io = new IntersectionObserver(function (es) { es.forEach(function (x) { if (x.isIntersecting) cb(x); }); }, { threshold: ratio || 0.35 });
    io.observe(el);
    return io;
  };

  // Anciennes ancres de l'accueil et du guide d'une seule page : on emmène le visiteur au bon endroit.
  var moves = {
    fr: { '/': { appareils: '/jeu/#appareil', playstation: '/jeu/#playstation', partage: '/share/', fonctions: '/jeu/#regle', telecharger: '/telecharger/', commencer: '/guide/ce-qu-il-vous-faut/', apps: '/#famille', questions: '/guide/faq/', technique: '/technologie/', reel: '/#telecharger' },
          '/guide/': { prerequis: 'ce-qu-il-vous-faut/', serveur: 'installer-station-windows/', appareil: 'connecter-un-appareil/', mac: 'link-mac/', windows: 'link-windows/', linux: 'link-linux-steam-deck/', 'steam-deck': 'link-linux-steam-deck/', 'apple-tv': 'apple-tv/', iphone: 'iphone-ipad/', android: 'android/', 'android-tv': 'android/', lg: 'televiseur-lg/', 'raspberry-pi': 'raspberry-pi/', navigateur: 'navigateur/', session: 'profils-et-qualite/', playstation: 'playstation/', partage: 'partager/', pret: 'preter/', compte: 'compte-kybernow/', distance: 'hors-de-chez-vous/', relais: 'installer-votre-relais/', 'mises-a-jour': 'mettre-a-jour/', desinstaller: 'desinstaller/', faq: 'faq/' } },
    en: { '/en/': { appareils: '/en/play/#appareil', playstation: '/en/play/#playstation', partage: '/en/share/', fonctions: '/en/play/#regle', telecharger: '/en/download/', commencer: '/en/guide/what-you-need/', apps: '/en/#famille', questions: '/en/guide/faq/', technique: '/en/technology/', reel: '/en/#telecharger' },
          '/en/guide/': { prerequis: 'what-you-need/', serveur: 'install-station-windows/', appareil: 'connect-a-device/', mac: 'link-mac/', windows: 'link-windows/', linux: 'link-linux-steam-deck/', 'steam-deck': 'link-linux-steam-deck/', 'apple-tv': 'apple-tv/', iphone: 'iphone-ipad/', android: 'android/', 'android-tv': 'android/', lg: 'lg-tv/', 'raspberry-pi': 'raspberry-pi/', navigateur: 'browser/', session: 'profiles-and-quality/', playstation: 'playstation/', partage: 'share/', pret: 'lend/', compte: 'kybernow-account/', distance: 'away-from-home/', relais: 'install-your-relay/', 'mises-a-jour': 'update/', desinstaller: 'uninstall/', faq: 'faq/' } }
  };
  var h = location.hash.slice(1);
  if (h) {
    var table = (moves[KN.fr ? 'fr' : 'en'] || {})[location.pathname];
    var to = table && table[h];
    if (to && !document.getElementById(h)) location.replace(to.charAt(0) === '/' ? to : location.pathname + to);
  }
})();
