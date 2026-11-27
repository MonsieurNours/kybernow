// KyberNow, page Télécharger : reconnaissance de l'appareil, onglets de la Station, filtres des apps,
// scènes lancées à l'arrivée, version publiée (si GitHub répond).
// Sans JavaScript, la page montre les trois Stations l'une sous l'autre et toutes les apps.
(function () {
  var KN = window.KN || { fr: document.documentElement.lang !== 'en', onView: function (el, cb) { cb(); } };
  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };

  // ---------- L'appareil du visiteur ----------
  // me : la carte Link à mettre en avant ; os : l'onglet de la Station ouvert d'abord ; form : le nœud du schéma.
  var ua = navigator.userAgent || '', all = ua + ' ' + (navigator.platform || '');
  var me = '', os = 'win', form = '';
  var ipados = /Macintosh/.test(ua) && (navigator.maxTouchPoints || 0) > 1; // l'iPad se présente comme un Mac
  if (/iPhone|iPad|iPod/.test(ua) || ipados) { me = 'ios'; os = 'mac'; form = /iPhone|iPod/.test(ua) ? 'phone' : 'tab'; }
  else if (/Web0S|webOS/.test(ua)) { me = 'lg'; }
  else if (/Android/.test(ua)) { me = 'android'; form = /Mobile/.test(ua) ? 'phone' : 'tab'; }
  else {
    // Le user-agent d'abord, la plateforme ensuite (certains navigateurs la figent).
    [ua, navigator.platform || ''].some(function (s) {
      if (/Win/.test(s)) { me = 'win'; os = 'win'; }
      else if (/Mac/.test(s)) { me = 'mac'; os = 'mac'; }
      else if (/Linux|X11/.test(s)) { me = 'linux'; os = 'linux'; }
      return !!me;
    });
  }
  var arm = me === 'linux' && /aarch64|arm64|armv8/i.test(all);

  // Linux sur ARM : les paquets arm64 et le Flatpak aarch64.
  if (arm) $$('a[data-arm]').forEach(function (a) { a.href = a.getAttribute('data-arm'); });

  // Schéma du héros : le téléphone ou la tablette du visiteur s'allume.
  if (form) $$('.node[data-form="' + form + '"]').forEach(function (n) { n.classList.add('here'); });

  // ---------- Étape 1 : onglets Windows, Mac, Linux ----------
  var tablist = $('.os'), tabs = $$('.os [role="tab"]'), panels = $$('.stcard[data-os]');
  function pick(id, focus, anim) {
    tabs.forEach(function (t) {
      var on = t.getAttribute('data-os') === id;
      t.setAttribute('aria-selected', on ? 'true' : 'false');
      t.tabIndex = on ? 0 : -1;
      if (on && focus) t.focus();
    });
    panels.forEach(function (p) {
      var on = p.getAttribute('data-os') === id;
      p.hidden = !on;
      if (on && anim) { p.classList.remove('swap'); void p.offsetWidth; p.classList.add('swap'); }
    });
  }
  if (tablist && tabs.length) {
    tablist.hidden = false;
    panels.forEach(function (p) { p.setAttribute('role', 'tabpanel'); p.setAttribute('aria-labelledby', 'tab-' + p.getAttribute('data-os')); p.tabIndex = -1; });
    tabs.forEach(function (t, i) {
      if (t.getAttribute('data-os') === me) { var b = t.querySelector('.me'); if (b) b.hidden = false; }
      t.addEventListener('click', function () { if (t.getAttribute('aria-selected') !== 'true') pick(t.getAttribute('data-os'), false, true); });
      t.addEventListener('keydown', function (e) {
        var j = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: tabs.length - 1 }[e.key];
        if (j === undefined) return;
        e.preventDefault();
        pick(tabs[(j + tabs.length) % tabs.length].getAttribute('data-os'), true, true);
      });
    });
    pick(os, false, false);
  }

  // ---------- Étape 2 : les apps Link ----------
  var devs = $$('.dev[data-id]'), chips = $('.chips');
  var mine = me ? $('.dev[data-id="' + me + '"]') : null;
  var tagText = ($('.herome .tg') || {}).textContent || (KN.fr ? 'Cet appareil' : 'This device');
  if (mine) {
    mine.classList.add('mine');
    var tag = document.createElement('span');
    tag.className = 'tag';
    tag.textContent = tagText;
    mine.insertBefore(tag, mine.firstChild);
    // En haut de page sur téléphone : la carte de cet appareil, avec son bouton.
    var hm = $('.herome'), cta = mine.querySelector('.btn');
    if (hm && cta) {
      $('.dic', hm).innerHTML = mine.querySelector('.dic').innerHTML;
      $('b', hm).textContent = mine.querySelector('h3').textContent;
      $('.how', hm).textContent = mine.querySelector('.how').textContent;
      var a = $('a', hm);
      a.href = cta.getAttribute('href');
      a.textContent = cta.textContent;
      hm.hidden = false;
    }
  }
  if (chips) {
    chips.hidden = false;
    var buttons = $$('button[data-k]', chips);
    buttons.forEach(function (b) {
      b.addEventListener('click', function () {
        var k = b.getAttribute('data-k');
        buttons.forEach(function (x) { x.setAttribute('aria-pressed', x === b ? 'true' : 'false'); });
        devs.forEach(function (d) { d.classList.toggle('dim', k !== 'all' && d.getAttribute('data-k') !== k); });
      });
    });
  }

  // ---------- Scènes (installation, association) : elles démarrent quand on les voit ----------
  if ('IntersectionObserver' in window && !KN.reduce) {
    $$('[data-play]').forEach(function (el) {
      el.classList.add('wait');
      var io = new IntersectionObserver(function (es) {
        es.forEach(function (x) { if (x.isIntersecting) { el.classList.remove('wait'); io.disconnect(); } });
      }, { threshold: 0.4 });
      io.observe(el);
    });
  }

  // ---------- Dernière version publiée : taille des fichiers et numéro ----------
  // Taille telle que le système l'affichera : Windows compte en 1 048 576 octets (et tronque), macOS en millions d'octets.
  var metas = $$('[data-release]');
  if (metas.length && window.fetch) {
    fetch('https://api.github.com/repos/MonsieurNours/kybernow/releases/latest', { headers: { Accept: 'application/vnd.github+json' } })
      .then(function (r) { return r.ok ? r.json() : Promise.reject(); })
      .then(function (rel) {
        var v = String(rel.tag_name || '').replace(/^v/, '');
        if (!v) return;
        metas.forEach(function (m) {
          var parts = [], name = m.getAttribute('data-release');
          var asset = name && (rel.assets || []).filter(function (x) { return x.name === name; })[0];
          if (asset) {
            var win = /\.exe$/i.test(name);
            var n = win ? Math.floor(asset.size / 1048576 * 10) / 10 : Math.round(asset.size / 1e6 * 10) / 10;
            parts.push(n.toLocaleString(KN.fr ? 'fr' : 'en-GB', { maximumFractionDigits: 1 }) + (KN.fr ? ' Mo' : ' MB'));
          }
          parts.push('version ' + v);
          m.textContent = ' · ' + parts.join(' · ');
        });
      })
      .catch(function () {});
  }
})();
