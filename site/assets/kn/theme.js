// Thème, posé dans <head> avant le rendu : le choix du visiteur (localStorage « kn-theme »), sinon celui de son appareil.
(function () {
  var t = null;
  try { t = localStorage.getItem('kn-theme'); } catch (e) {}
  if (t !== 'light' && t !== 'dark') t = window.matchMedia && matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  document.documentElement.setAttribute('data-theme', t);
  var m = document.querySelector('meta[name="theme-color"]');
  if (m) m.content = t === 'light' ? '#F2F5F6' : '#05070A';
})();
