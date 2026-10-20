// www.kybernow.app — scène 3D : le symbole « Relais » en volume, sa couture en néon, le flux de l'image.
// Géométrie reprise de brand/v2/tools/geometry.json (grille de 48).

import * as THREE from 'three';
import { SVGLoader } from 'three/addons/loaders/SVGLoader.js';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/addons/postprocessing/OutputPass.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';

const GEO = {
  source: 'M7 0 L42.275 0 A1.2 1.2 0 0 1 43.123 2.049 L2.049 43.123 A1.2 1.2 0 0 1 0 42.275 L0 7 A7 7 0 0 1 7 0 Z',
  dest: 'M48 5.725 L48 41 A7 7 0 0 1 41 48 L5.725 48 A1.2 1.2 0 0 1 4.877 45.951 L45.951 4.877 A1.2 1.2 0 0 1 48 5.725 Z',
};
const COLORS = { ink: 0x05070A, frost: 0xE3EDF0, signal: 0x9A8CFF, strong: 0x6D5BFF, core: 0xF4F1FF };
const UNIT = 0.075; // une unité de la grille, en unités de la scène
const SQ = Math.SQRT1_2;

// États de la scène, posés sur les sections (data-scene). x, y : fraction de la demi-largeur / demi-hauteur visibles.
const STATES = {
  hero:     { x: .66, y: 0,  s: .6,    rx: -.22, ry: -.5,  rz: 0,   split: 0,   glow: 1, flow: .6,  spread: 10, len: 7,   depth: 3,  speed: .05, exp: 1, edge: 0, hotW: .05 },
  split:    { x: .7,  y: 0,   s: .55, rx: -.1,  ry: -.25, rz: .12, split: .55, glow: 1, flow: .85, spread: 7,  len: 6,   depth: 2,  speed: .08, exp: .7, edge: 0, hotW: .05 },
  flow:     { x: 0,   y: .05, s: .62,   rx: 0,    ry: 0,    rz: Math.PI / 4, split: 0, glow: 1, flow: 1, spread: 3, len: 5, depth: .6, speed: .16, exp: .5, edge: 1, hotW: .5 },
  side:     { x: -.78, y: .1, s: .55,   rx: -.3,  ry: .7,   rz: .3,  split: .2,  glow: 0, flow: .3,  spread: 9,  len: 7,   depth: 3,  speed: .03, exp: .42, edge: 0, hotW: .05 },
  'rest-r': { x: .8,  y: -.1, s: .5,   rx: -.35, ry: -.8,  rz: -.2, split: .1,  glow: 0, flow: .25, spread: 9,  len: 7,   depth: 3,  speed: .03, exp: .36, edge: 0, hotW: .05 },
  'rest-l': { x: -.8, y: 0,   s: .5,   rx: -.25, ry: .9,   rz: .25, split: .1,  glow: 0, flow: .25, spread: 9,  len: 7,   depth: 3,  speed: .03, exp: .36, edge: 0, hotW: .05 },
  assemble: { x: 0,   y: .02, s: .72,  rx: -.18, ry: .35,  rz: 0,   split: 0,   glow: 1, flow: .65, spread: 10, len: 7,   depth: 3,  speed: .05, exp: 1, edge: 0, hotW: .05 },
  guide:    { x: .66, y: .1,  s: .5,   rx: -.2,  ry: -.55, rz: 0,   split: 0,   glow: 1, flow: .5,  spread: 10, len: 7,   depth: 3,  speed: .05, exp: .85, edge: 0, hotW: .05 },
  page:     { x: .7,  y: .2,  s: .42,  rx: -.25, ry: -.7,  rz: -.1, split: .15, glow: 0, flow: .3,  spread: 9,  len: 7,   depth: 3,  speed: .03, exp: .5, edge: 0, hotW: .05 },
};
// Sur téléphone : le symbole se range en haut de l'écran, plus petit, pour ne jamais passer sous le texte.
const MOBILE = {
  hero:     { x: 0,   y: .7,  s: .27 },
  split:    { x: .5,  y: .74, s: .22, flow: .5 },
  flow:     { x: 0,   y: .25, s: .38 },
  side:     { x: .78, y: .8,  s: .16, exp: .3 },
  'rest-r': { x: .78, y: .8,  s: .16, exp: .3 },
  'rest-l': { x: -.78, y: .8, s: .16, exp: .3 },
  assemble: { x: 0,   y: .12, s: .4 },
  guide:    { x: .55, y: .72, s: .24 },
  page:     { x: .6,  y: .76, s: .2 },
};
const KEYS = Object.keys(STATES.hero);

const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
const canvas = document.getElementById('scene');

function fail() { document.documentElement.classList.add('no-webgl'); canvas.remove(); }

function init() {
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, powerPreference: 'high-performance' });
  } catch { fail(); return; }
  if (!renderer.getContext()) { fail(); return; }
  canvas.addEventListener('webglcontextlost', e => { e.preventDefault(); fail(); });

  renderer.setClearColor(COLORS.ink, 1);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1;

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), .04).texture;
  scene.environmentIntensity = .4;

  const camera = new THREE.PerspectiveCamera(32, 1, .1, 100);
  camera.position.set(0, 0, 11);

  // ---- Le symbole ----
  const loader = new SVGLoader();
  function halfGeometry(d) {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg"><path transform="matrix(1 0 0 -1 -24 24)" d="${d}"/></svg>`;
    const shapes = SVGLoader.createShapes(loader.parse(svg).paths[0]);
    // Biseau fin : il élargit chaque bord vers l'extérieur ; plus large que l'arrondi
    // de la pointe (1,2), il y formait un bourrelet, visible sur la moitié violette.
    const g = new THREE.ExtrudeGeometry(shapes, { depth: 6, curveSegments: 32, bevelEnabled: true, bevelThickness: 1.4, bevelSize: .3, bevelSegments: 8 });
    g.translate(0, 0, -3);
    g.scale(UNIT, UNIT, UNIT);
    g.computeVertexNormals();
    return g;
  }
  const mark = new THREE.Group();
  scene.add(mark);

  const srcMat = new THREE.MeshPhysicalMaterial({ color: COLORS.frost, roughness: .34, metalness: 0, clearcoat: .7, clearcoatRoughness: .22 });
  const dstMat = new THREE.MeshPhysicalMaterial({ color: COLORS.signal, roughness: .26, metalness: .05, clearcoat: 1, clearcoatRoughness: .16, emissive: COLORS.strong, emissiveIntensity: 0 });
  const src = new THREE.Mesh(halfGeometry(GEO.source), srcMat);
  const dst = new THREE.Mesh(halfGeometry(GEO.dest), dstMat);
  mark.add(src, dst);

  // La couture : un tube de néon (cœur, gaine) et deux lumières qui éclairent les bords.
  const beam = new THREE.Group();
  beam.rotation.z = -Math.PI / 4;
  const seamLen = 52 * UNIT;
  const coreMat = new THREE.MeshBasicMaterial({ color: COLORS.core, toneMapped: false, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false });
  const sheathMat = new THREE.MeshBasicMaterial({ color: COLORS.signal, toneMapped: false, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false });
  const core = new THREE.Mesh(new THREE.CapsuleGeometry(.85 * UNIT, seamLen, 8, 24), coreMat);
  const sheath = new THREE.Mesh(new THREE.CapsuleGeometry(2.1 * UNIT, seamLen, 8, 24), sheathMat);
  beam.add(sheath, core);
  mark.add(beam);
  const seamLights = [-.9, .9].map(t => {
    const l = new THREE.PointLight(COLORS.signal, 0, 4.5, 1.6);
    l.position.set(t * SQ * 1.6, t * SQ * 1.6, 0);
    mark.add(l);
    return l;
  });

  const key = new THREE.DirectionalLight(0xffffff, .7);
  key.position.set(-4, 5, 6);
  const rim = new THREE.DirectionalLight(COLORS.strong, 1.4);
  rim.position.set(5, -3, -4);
  scene.add(key, rim);

  // ---- Le flux : des particules qui passent de l'ordinateur (en haut à gauche) à vous ----
  const COUNT = innerWidth < 760 ? 1400 : 2800;
  const seeds = new Float32Array(COUNT * 4);
  for (let i = 0; i < COUNT * 4; i++) seeds[i] = Math.random();
  const pGeo = new THREE.BufferGeometry();
  pGeo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(COUNT * 3), 3));
  pGeo.setAttribute('seed', new THREE.BufferAttribute(seeds, 4));
  const pUniforms = {
    uPhase: { value: 0 }, uFlow: { value: 0 }, uSpread: { value: 10 }, uLen: { value: 7 }, uDepth: { value: 3 },
    uGlow: { value: 0 }, uPix: { value: 60 }, uHotPos: { value: 0 }, uHotW: { value: .05 },
    uColA: { value: new THREE.Color(COLORS.signal) }, uColB: { value: new THREE.Color(COLORS.core) },
  };
  const pMat = new THREE.ShaderMaterial({
    uniforms: pUniforms, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    vertexShader: /* glsl */`
      attribute vec4 seed;
      uniform float uPhase, uFlow, uSpread, uLen, uDepth, uGlow, uPix, uHotPos, uHotW;
      varying float vA; varying float vHot;
      void main() {
        float u = fract(seed.x + uPhase * (0.6 + seed.w * 0.8));
        float s = (u - 0.5) * 2.0 * uLen;            // à travers la couture, de l'ordinateur vers vous
        float t = (seed.y - 0.5) * uSpread;           // le long de la couture
        float z = (seed.z - 0.5) * uDepth;
        vec2 across = vec2(0.70710678, -0.70710678);
        vec2 along = vec2(0.70710678, 0.70710678);
        vec3 p = vec3(across * s + along * t, z);
        float hot = exp(-(s - uHotPos) * (s - uHotPos) / uHotW);
        vHot = hot * uGlow;
        float edge = smoothstep(0.0, 0.15, u) * smoothstep(1.0, 0.85, u);
        vA = edge * uFlow * (0.35 + 0.9 * vHot);
        vec4 mv = modelViewMatrix * vec4(p, 1.0);
        gl_PointSize = uPix * (0.35 + seed.w * 0.9) * (1.0 + hot * 1.2) / -mv.z;
        gl_Position = projectionMatrix * mv;
      }`,
    fragmentShader: /* glsl */`
      uniform vec3 uColA, uColB;
      varying float vA; varying float vHot;
      void main() {
        float r = length(gl_PointCoord - 0.5);
        float m = smoothstep(0.5, 0.0, r); m *= m;
        gl_FragColor = vec4(mix(uColA, uColB * 2.0, vHot), m * vA);
      }`,
  });
  const particles = new THREE.Points(pGeo, pMat);
  particles.frustumCulled = false;
  mark.add(particles);

  // ---- Rendu : bloom pour le néon ----
  const composer = new EffectComposer(renderer);
  composer.addPass(new RenderPass(scene, camera));
  const bloom = new UnrealBloomPass(new THREE.Vector2(256, 256), .8, .5, 1);
  composer.addPass(bloom);
  composer.addPass(new OutputPass());

  let halfH = 1, halfW = 1, mobile = false;
  function resize() {
    const w = innerWidth, h = innerHeight;
    const dpr = Math.min(devicePixelRatio || 1, 1.75);
    renderer.setPixelRatio(dpr); composer.setPixelRatio(dpr);
    renderer.setSize(w, h, false); composer.setSize(w, h);
    camera.aspect = w / h; camera.updateProjectionMatrix();
    halfH = Math.tan(THREE.MathUtils.degToRad(camera.fov / 2)) * camera.position.z;
    halfW = halfH * camera.aspect;
    mobile = camera.aspect < .9;
    pUniforms.uPix.value = 46 * dpr * (h / 900);
    measure();
    requestRender();
  }

  // ---- Ancrages : chaque section porte un état ----
  let anchors = [];
  function measure() {
    anchors = [];
    for (const el of document.querySelectorAll('[data-scene]')) {
      const r = el.getBoundingClientRect();
      const top = r.top + scrollY;
      const key = el.dataset.scene;
      const state = mobile ? { ...STATES[key], ...MOBILE[key] } : STATES[key];
      if (el.id === 'trajet') { // tenu pendant toute la partie épinglée
        anchors.push({ pos: top + innerHeight * .5, state }, { pos: top + r.height - innerHeight * .5, state });
      } else if (el.classList.contains('scene-anchor')) {
        anchors.push({ pos: top + r.height * .5, state });
      } else {
        anchors.push({ pos: Math.max(innerHeight * .5, top + Math.min(r.height, innerHeight) * .5), state, });
      }
    }
    anchors.sort((a, b) => a.pos - b.pos);
  }

  const smooth = t => t * t * (3 - 2 * t);
  function target() {
    const c = scrollY + innerHeight / 2;
    const out = {};
    if (!anchors.length) { Object.assign(out, STATES.hero); out.extraY = 0; return out; }
    let extraY = 0;
    let a = anchors[0], b = anchors[0], t = 0;
    if (c >= anchors[anchors.length - 1].pos) {
      a = b = anchors[anchors.length - 1];
      extraY = (c - a.pos) / innerHeight * 2 * halfH; // suit le contenu après le dernier ancrage
    } else if (c > anchors[0].pos) {
      for (let i = 0; i < anchors.length - 1; i++) {
        if (c >= anchors[i].pos && c < anchors[i + 1].pos) { a = anchors[i]; b = anchors[i + 1]; t = smooth((c - a.pos) / (b.pos - a.pos)); break; }
      }
    }
    for (const k of KEYS) out[k] = a.state[k] + (b.state[k] - a.state[k]) * t;
    if (reduce) { // « Réduire les animations » : le symbole ne voyage pas, la lumière suit seulement
      const keep = { glow: out.glow, exp: out.exp, flow: out.flow };
      Object.assign(out, mobile ? { ...STATES.hero, ...MOBILE.hero } : STATES.hero, keep);
      extraY = 0;
    }
    out.extraY = extraY;
    return out;
  }

  // ---- Néon : allumage d'une seconde, puis respiration sur 3,2 s ----
  const IGNITE = [[0, 0], [.06, 1.6], [.1, 0], [.16, 1], [.22, 0], [.3, 1.6], [.34, .45], [.44, 1], [1, 1]];
  let ignitedAt = -1;
  function neon(now) {
    if (ignitedAt < 0) return 0;
    if (reduce) return 1;
    const d = (now - ignitedAt) / 1000;
    if (d < 1) {
      for (let i = 0; i < IGNITE.length - 1; i++) {
        const [t0, v0] = IGNITE[i], [t1, v1] = IGNITE[i + 1];
        if (d >= t0 && d < t1) return v0 + (v1 - v0) * (d - t0) / (t1 - t0);
      }
    }
    return .78 + .22 * Math.cos(2 * Math.PI * (d - 1) / 3.2);
  }
  function ignite() { ignitedAt = performance.now(); requestRender(); }

  // ---- Pointeur, défilement ----
  const pointer = { x: 0, y: 0, tx: 0, ty: 0 };
  addEventListener('pointermove', e => { pointer.tx = e.clientX / innerWidth * 2 - 1; pointer.ty = e.clientY / innerHeight * 2 - 1; }, { passive: true });
  let lastScroll = scrollY, boost = 0;
  addEventListener('scroll', () => { boost += Math.abs(scrollY - lastScroll) / innerHeight; lastScroll = scrollY; requestRender(); }, { passive: true });
  addEventListener('resize', resize);
  new ResizeObserver(() => { measure(); requestRender(); }).observe(document.body);
  document.fonts?.ready.then(measure);

  // ---- Boucle ----
  const cur = { ...STATES.hero, glow: 0, extraY: 0 };
  let lastT = performance.now(), wasLit = true, needsRender = true, running = false;
  function requestRender() { needsRender = true; if (reduce && !running) { running = true; requestAnimationFrame(tick); } }

  function tick(now) {
    const dt = Math.min(.05, (now - lastT) / 1000); lastT = now;
    const tgt = target();
    const k = reduce ? 1 : 1 - Math.exp(-dt * 5);
    for (const key of [...KEYS, 'extraY']) cur[key] += (tgt[key] - cur[key]) * (key === 'extraY' ? 1 : k);

    // Ré-allumage quand on revient vers un moment actif (téléchargement)
    const lit = tgt.glow > .7;
    if (lit && !wasLit) ignite();
    wasLit = lit || tgt.glow > .3 && wasLit;

    pointer.x += (pointer.tx - pointer.x) * (reduce ? 1 : 1 - Math.exp(-dt * 3));
    pointer.y += (pointer.ty - pointer.y) * (reduce ? 1 : 1 - Math.exp(-dt * 3));
    const tSec = now / 1000;
    const float = reduce ? 0 : Math.sin(tSec * .6) * .05;

    const ms = cur.s;
    mark.position.set(cur.x * halfW, cur.y * halfH + float + cur.extraY, 0);
    mark.scale.setScalar(ms);
    mark.rotation.set(cur.rx + pointer.y * .12, cur.ry + pointer.x * .2 + (reduce ? 0 : Math.sin(tSec * .3) * .05), cur.rz);

    // Les deux moitiés s'écartent le long de la normale à la couture.
    // edge : les moitiés partent aux bords de l'écran (trajet d'une image)
    const edgeSplit = halfW * .86 / ms - .55;
    const split = cur.split + (edgeSplit - cur.split) * cur.edge;
    src.position.set(-SQ * split, SQ * split, 0);
    dst.position.set(SQ * split, -SQ * split, 0);

    const n = neon(now) * cur.glow;
    coreMat.color.setHex(COLORS.core).multiplyScalar(n * 2.4);
    sheathMat.color.setHex(COLORS.signal).multiplyScalar(n * .55);
    core.visible = sheath.visible = n > .002;
    // La couture s'efface quand les moitiés s'écartent : le flux prend le relais.
    const seamFade = 1 - Math.min(1, split / 1.2);
    coreMat.opacity = sheathMat.opacity = seamFade;
    seamLights.forEach(l => { l.intensity = n * 2.2 * (.35 + .65 * seamFade); });
    dstMat.emissiveIntensity = n * .12;

    boost *= Math.exp(-dt * 3);
    if (!reduce) pUniforms.uPhase.value += dt * (cur.speed + Math.min(boost, 1.5) * .35);
    pUniforms.uFlow.value = cur.flow;
    pUniforms.uSpread.value = cur.spread;
    pUniforms.uLen.value = Math.max(cur.len, split + 1.6);
    const tp = Math.min(1, Math.max(0, ((window.knTrajet || 0) - .06) / .8));
    pUniforms.uHotPos.value = cur.edge * (tp * 2 - 1) * split;
    pUniforms.uHotW.value = cur.hotW;
    pUniforms.uDepth.value = cur.depth;
    pUniforms.uGlow.value = Math.max(n, .25);

    renderer.toneMappingExposure = cur.exp;
    bloom.strength = .35 + .45 * cur.glow;

    if (!document.hidden && (needsRender || !reduce)) { composer.render(); needsRender = false; }
    if (reduce) { running = false; return; }
    requestAnimationFrame(tick);
  }

  resize();
  if (reduce) { ignitedAt = 0; requestRender(); }
  else {
    running = true;
    requestAnimationFrame(tick);
    setTimeout(ignite, 450); // l'écran apparaît, la couture s'allume
  }
}

init();
