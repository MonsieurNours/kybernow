// Langue : sans choix enregistré, un navigateur qui n'est pas en français va vers la version anglaise
// (et inversement). Le choix fait avec le sélecteur de langue est retenu. Chargé dans <head>, avant le rendu.
(function () {
  if (/bot|crawl|spider|slurp|lighthouse/i.test(navigator.userAgent)) return;
  var here = document.documentElement.lang === 'en' ? 'en' : 'fr';
  var want = null;
  try { want = localStorage.getItem('kn-lang'); } catch (e) {}
  if (want !== 'fr' && want !== 'en') {
    var l = (navigator.languages && navigator.languages[0]) || navigator.language || 'fr';
    want = /^fr\b/i.test(l) ? 'fr' : 'en';
  }
  if (want === here) return;
  var alt = document.querySelector('link[rel="alternate"][hreflang="' + want + '"]');
  if (!alt) return;
  location.replace(new URL(alt.href).pathname + location.hash);
})();
