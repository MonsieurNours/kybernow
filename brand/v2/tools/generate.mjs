// Génère tous les SVG de la marque KyberNow à partir de la géométrie du symbole
// « Relais » et du nom vectorisé (wordmark.json, produit par outline.py).
//
//   node brand/v2/tools/generate.mjs [givre|tally|spectre]
//
// Ne pas retoucher les SVG à la main : modifier ce script et relancer.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..");
const piste = process.argv[2] ?? "spectre";

// ── Couleurs ────────────────────────────────────────────────────────────────
const PISTES = {
  givre:   { signal: "#5EEBC4", strong: "#2BCF9F", ink: "#0E8F6C", glow: "#5EEBC4" },
  tally:   { signal: "#FFA24A", strong: "#FF8A1F", ink: "#B35A00", glow: "#FF8A1F" },
  spectre: { signal: "#9A8CFF", strong: "#6D5BFF", ink: "#5340E6", glow: "#7E6CFF" },
};
const P = PISTES[piste];
if (!P) throw new Error(`piste inconnue : ${piste}`);
const ICE = "#E3EDF0", INK = "#070A0E", INK_TEXT = "#0B1117";
const NEON_CORE = "#F4F1FF";

// ── Géométrie du symbole (grille de 48) ─────────────────────────────────────
// Un carré coupé en deux par une couture à 45° : l'ordinateur (en haut à gauche),
// l'écran où vous êtes (en bas à droite), et entre les deux le lien.
const G = 48, GAP = 4, c = (GAP / 2) * Math.SQRT2;
const R_RIGHT = 7, R_ACUTE = 1.2;

/** Polygone aux coins arrondis, rayon propre à chaque sommet. */
function rounded(pts, radii) {
  const n = pts.length, seg = [];
  for (let i = 0; i < n; i++) {
    const P0 = pts[(i - 1 + n) % n], P1 = pts[i], P2 = pts[(i + 1) % n];
    const u = norm([P0[0] - P1[0], P0[1] - P1[1]]), v = norm([P2[0] - P1[0], P2[1] - P1[1]]);
    const theta = Math.acos(u[0] * v[0] + u[1] * v[1]);
    const r = radii[i], t = r / Math.tan(theta / 2);
    const a = [P1[0] + u[0] * t, P1[1] + u[1] * t], b = [P1[0] + v[0] * t, P1[1] + v[1] * t];
    const sweep = u[0] * v[1] - u[1] * v[0] < 0 ? 1 : 0;
    seg.push({ a, b, r, sweep });
  }
  let d = `M${f(seg[0].b)}`;
  for (let i = 1; i <= n; i++) {
    const s = seg[i % n];
    d += ` L${f(s.a)} A${s.r} ${s.r} 0 0 ${s.sweep} ${f(s.b)}`;
  }
  return d + " Z";
}
const norm = ([x, y]) => { const l = Math.hypot(x, y); return [x / l, y / l]; };
const f = ([x, y]) => `${+x.toFixed(3)} ${+y.toFixed(3)}`;

export const SOURCE = rounded([[0, 0], [G - c, 0], [0, G - c]], [R_RIGHT, R_ACUTE, R_ACUTE]);
export const DEST = rounded([[G, c], [G, G], [c, G]], [R_ACUTE, R_RIGHT, R_ACUTE]);
// Le lien : un trait dans la couture, raccourci aux deux bouts.
const BW = 1.8, n = BW / 2 / Math.SQRT2, B0 = 5, B1 = G - 5;
export const BEAM = rounded([[B0 + n, B1 + n], [B1 + n, B0 + n], [B1 - n, B0 - n], [B0 - n, B1 - n]], [0.85, 0.85, 0.85, 0.85]);

// Axe de la couture, pour le néon (cœur, gaine, halo).
const SEAM = "M5.6 42.4 L42.4 5.6";
/** Néon de la couture : halo large, gaine Spectre, cœur presque blanc. Filtres #kn-nw et #kn-ns requis. */
const neonLayers = () =>
  `<path d="${SEAM}" fill="none" stroke="${P.signal}" stroke-width="9" stroke-linecap="round" opacity="0.9" filter="url(#kn-nw)"/>` +
  `<path d="${SEAM}" fill="none" stroke="${P.signal}" stroke-width="2.6" stroke-linecap="round" filter="url(#kn-ns)"/>` +
  `<path d="${BEAM}" fill="${NEON_CORE}"/>`;
const neonDefs = (wide = 3.2, soft = 0.7) =>
  `<filter id="kn-nw" filterUnits="userSpaceOnUse" x="-12" y="-12" width="72" height="72"><feGaussianBlur stdDeviation="${wide}"/></filter>` +
  `<filter id="kn-ns" filterUnits="userSpaceOnUse" x="-12" y="-12" width="72" height="72"><feGaussianBlur stdDeviation="${soft}"/></filter>`;

/** Les trois états du symbole : chaque app allume une partie différente. */
const STATES = {
  // Écosystème et Client : le signal arrive chez vous.
  now:     (ice, sig) => [[SOURCE, ice], [DEST, sig]],
  // Server : le signal part de l'ordinateur.
  server:  (ice, sig) => [[SOURCE, sig], [DEST, ice]],
  // Console : le lien lui-même, sous surveillance.
  console: (ice, sig, dim) => [[SOURCE, dim], [DEST, dim], [BEAM, sig]],
};

function markGroup(state, { ice, sig, dim, x = 0, y = 0, size = G }) {
  const s = size / G;
  const paths = STATES[state](ice, sig, dim ?? ice).map(([d, fill]) => `<path d="${d}" fill="${fill}"/>`).join("");
  return `<g transform="translate(${+x.toFixed(3)} ${+y.toFixed(3)}) scale(${+s.toFixed(5)})">${paths}</g>`;
}

const svg = (w, h, body, label = "KyberNow") =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${+w.toFixed(2)} ${+h.toFixed(2)}" role="img" aria-label="${label}">\n<title>${label}</title>\n${body}\n</svg>\n`;

// ── Nom ─────────────────────────────────────────────────────────────────────
// « Kyber » en graisse légère, « Now » en graisse forte : la technologie,
// puis ce qui est à nous. Archivo, largeur 125.
const WM = JSON.parse(readFileSync(join(here, "wordmark.json"), "utf8"));
const CAP = 68.6, DESC = 21;
const WM_W = WM.kyber.advance - 1.2 + WM.now.advance;
function wordmark(color, x, y, s = 1) {
  return `<g transform="translate(${+x.toFixed(3)} ${+y.toFixed(3)}) scale(${s})" fill="${color}">` +
    `<path d="${WM.kyber.d}"/>` +
    `<g transform="translate(${+(WM.kyber.advance - 1.2).toFixed(2)} 0)"><path d="${WM.now.d}"/></g></g>`;
}

// ── Fichiers ────────────────────────────────────────────────────────────────
const files = {};
const themes = {
  dark:  { ice: ICE, sig: P.signal, dim: "#3A4652", word: ICE },
  light: { ice: INK_TEXT, sig: P.ink, dim: "#B9C4CB", word: INK_TEXT },
};

for (const [t, col] of Object.entries(themes)) {
  for (const state of Object.keys(STATES)) {
    files[`mark/kybernow-mark-${state}-${t}.svg`] = svg(G, G, markGroup(state, col));
  }
  // Allumé (niveau 2 de la charte) : fond sombre uniquement, marge pour le halo.
  if (t === "dark") for (const state of Object.keys(STATES)) {
    const cons = state === "console";
    files[`mark/kybernow-mark-${state}-lit-dark.svg`] = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-8 -8 64 64" role="img" aria-label="KyberNow">\n<title>KyberNow</title>\n<defs>${neonDefs()}</defs>\n` +
      STATES[state](col.ice, cons ? col.sig : P.signal, col.dim).filter(([d]) => d !== BEAM).map(([d, fill]) => `<path d="${d}" fill="${fill}"/>`).join("") +
      neonLayers() + "\n</svg>\n";
  }
  // Horizontal : symbole haut de 1,46 capitale, centré sur la hauteur de capitale.
  const mh = CAP * 1.46, gap = mh * 0.36, pad = 0;
  const top = -CAP / 2 - mh / 2;
  const W = mh + gap + WM_W, H = mh;
  files[`logo/kybernow-logo-${t}.svg`] = svg(W, H,
    markGroup("now", { ...col, size: mh, y: 0 }) + wordmark(col.word, mh + gap, -top));
  // Empilé : symbole au-dessus, nom centré.
  const sw = WM_W * 0.62, smh = sw * 0.46, sgap = smh * 0.34;
  files[`logo/kybernow-logo-stacked-${t}.svg`] = svg(Math.max(sw, smh), smh + sgap + CAP * 0.62 + 2,
    markGroup("now", { ...col, size: smh, x: (sw - smh) / 2 }) + wordmark(col.word, 0, smh + sgap + CAP * 0.62, 0.62));
  // Nom seul.
  files[`logo/kybernow-wordmark-${t}.svg`] = svg(WM_W, CAP + DESC, wordmark(col.word, 0, CAP));
}

// Icônes d'application (1024, gabarit macOS : 824 utiles, rayon 185).
function appIcon(state, { tile = "#0B1016", tile2 = "#05070A" } = {}) {
  const m = 540, x = (1024 - m) / 2, y = (1024 - m) / 2;
  const lit = state === "now" ? DEST : state === "server" ? SOURCE : BEAM;
  const s = m / G;
  return svg(1024, 1024, `<defs>
  <linearGradient id="tile" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${tile}"/><stop offset="1" stop-color="${tile2}"/></linearGradient>
  <linearGradient id="ice" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#F4FAFB"/><stop offset="1" stop-color="#B9CAD1"/></linearGradient>
  <radialGradient id="halo" cx="0.62" cy="0.62" r="0.55"><stop offset="0" stop-color="${P.glow}" stop-opacity="0.38"/><stop offset="1" stop-color="${P.glow}" stop-opacity="0"/></radialGradient>
  <filter id="bloom" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="22"/></filter>
  ${neonDefs(3.4, 0.8)}
  <clipPath id="sq"><rect x="100" y="100" width="824" height="824" rx="185"/></clipPath>
</defs>
<g clip-path="url(#sq)">
  <rect x="100" y="100" width="824" height="824" fill="url(#tile)"/>
  <rect x="100" y="100" width="824" height="824" fill="url(#halo)"/>
  <path d="M100 924 L924 100" stroke="#FFFFFF" stroke-opacity="0.04" stroke-width="2"/>
  <g transform="translate(${x} ${y}) scale(${s})">
    <path d="${lit}" fill="${P.signal}" filter="url(#bloom)" opacity="0.6"/>
    ${STATES[state]("url(#ice)", P.signal, "#2A343E").filter(([d]) => d !== BEAM).map(([d, fill]) => `<path d="${d}" fill="${fill}"/>`).join("\n    ")}
    ${neonLayers()}
  </g>
  <rect x="101" y="101" width="822" height="822" rx="184" fill="none" stroke="#FFFFFF" stroke-opacity="0.08" stroke-width="2"/>
</g>`, `KyberNow ${state === "now" ? "Client" : state === "server" ? "Server" : "Console"}`);
}
files["app-icon/kybernow-client-1024.svg"] = appIcon("now");
files["app-icon/kybernow-server-1024.svg"] = appIcon("server");
files["app-icon/kybernow-console-1024.svg"] = appIcon("console");

// Favicon : symbole seul, sans dégradé, lisible à 16 px.
files["favicon/kybernow-favicon.svg"] = svg(G, G,
  `<style>.i{fill:${INK_TEXT}}.s{fill:${P.ink}}@media(prefers-color-scheme:dark){.i{fill:${ICE}}.s{fill:${P.signal}}}</style>` +
  `<path class="i" d="${SOURCE}"/><path class="s" d="${DEST}"/>`);

for (const [p, content] of Object.entries(files)) {
  mkdirSync(dirname(join(out, p)), { recursive: true });
  writeFileSync(join(out, p), content);
}
writeFileSync(join(here, "geometry.json"), JSON.stringify({ grid: G, source: SOURCE, dest: DEST, beam: BEAM, wordmark: { kyber: WM.kyber.d, now: WM.now.d, nowOffset: WM.kyber.advance - 1.2, width: WM_W, cap: CAP } }, null, 1));
console.log(`piste ${piste} : ${Object.keys(files).length} fichiers`);
