// Vidéo de présentation : démarre en muet quand elle est à l'écran, s'arrête quand elle le quitte ;
// le son vient d'un geste du visiteur. Qualité selon le débit (HLS 720p, 1080p, 4K) : hls.js chargé à l'approche,
// HLS natif de Safari sinon, MP4 720p en dernier recours. Avec « Réduire les animations », rien ne démarre seul.
(function () {
  var fig = document.querySelector('.kv');
  if (!fig) return;
  var video = fig.querySelector('video');
  var fr = document.documentElement.lang !== 'en';
  var T = fr ? { play: 'Lire la présentation', pause: 'Mettre en pause', unmute: 'Activer le son', mute: 'Couper le son' }
             : { play: 'Play the presentation', pause: 'Pause', unmute: 'Turn sound on', mute: 'Mute' };
  var reduce = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;
  var toggle = fig.querySelector('[data-video="toggle"]'), sound = fig.querySelector('[data-video="sound"]');
  var label = sound && sound.querySelector('.kv-label');
  var wanted = !reduce, started = false, ready = null;

  function sync() {
    var playing = !video.paused && !video.ended;
    fig.classList.toggle('is-playing', playing);
    if (started) fig.classList.add('has-started');
    fig.classList.toggle('has-sound', !video.muted);
    if (toggle) toggle.setAttribute('aria-label', playing ? T.pause : T.play);
    if (sound) { sound.setAttribute('aria-label', video.muted ? T.unmute : T.mute); sound.setAttribute('aria-pressed', String(!video.muted)); }
    if (label) label.textContent = video.muted ? T.unmute : T.mute;
  }
  function attach() {
    return ready || (ready = new Promise(function (done) {
      var url = video.getAttribute('data-hls');
      if (!url) return done();
      var native = function () { if (video.canPlayType('application/vnd.apple.mpegurl')) video.src = url; done(); };
      if (!window.MediaSource && !window.ManagedMediaSource) return native();
      var s = document.createElement('script');
      s.src = '/vendor/hls/hls.light.min.js';
      s.onload = function () {
        if (!window.Hls || !window.Hls.isSupported()) return native();
        var hls = new window.Hls({ capLevelToPlayerSize: true, startLevel: -1, maxBufferLength: 20 });
        hls.on(window.Hls.Events.MANIFEST_PARSED, function () { done(); });
        hls.on(window.Hls.Events.ERROR, function (_, d) { if (d.fatal) { hls.destroy(); video.src = '/assets/video/presentation-720p.mp4'; done(); } });
        hls.loadSource(url); hls.attachMedia(video);
      };
      s.onerror = native;
      document.head.appendChild(s);
    }));
  }
  function play() { attach().then(function () { return video.play(); }).then(function () { started = true; sync(); }).catch(function () {}); }
  function unmute() { video.muted = false; wanted = true; if (video.paused) play(); else sync(); }

  if ('IntersectionObserver' in window) {
    var near = new IntersectionObserver(function (es) { if (es[0].isIntersecting) { attach(); near.disconnect(); } }, { rootMargin: '600px' });
    near.observe(fig);
    if (!reduce) new IntersectionObserver(function (es) {
      var e = es[0];
      if (e.isIntersecting && wanted && video.paused) play();
      else if (!e.isIntersecting && !video.paused) video.pause();
    }, { threshold: 0.45 }).observe(fig);
  }
  ['play', 'pause', 'ended', 'volumechange'].forEach(function (ev) { video.addEventListener(ev, sync); });
  video.addEventListener('click', function () { if (video.muted) unmute(); else { wanted = video.paused; if (wanted) play(); else video.pause(); } });
  video.addEventListener('error', function () { if (!video.currentSrc || /\.mp4$/.test(video.currentSrc)) fig.classList.add('no-source'); }, true);
  if (sound) sound.addEventListener('click', function () { if (video.muted) unmute(); else video.muted = true; });
  if (toggle) toggle.addEventListener('click', function () { wanted = video.paused; if (wanted) play(); else video.pause(); });
  var big = fig.querySelector('[data-video="play"]');
  if (big) big.addEventListener('click', function () { video.muted = false; wanted = true; play(); });
  var again = fig.querySelector('[data-video="restart"]');
  if (again) again.addEventListener('click', function () { video.currentTime = 0; wanted = true; play(); });
  var full = fig.querySelector('[data-video="fullscreen"]');
  if (full) full.addEventListener('click', function () {
    if (video.requestFullscreen) video.requestFullscreen(); else if (video.webkitEnterFullscreen) video.webkitEnterFullscreen();
  });
  sync();
})();
