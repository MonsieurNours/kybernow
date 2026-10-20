// Assemble la charte (brand/v2/charte/index.html) depuis charte.src.html :
//   <x-mark state="now|server|console" class="…"></x-mark>  → symbole en SVG (couleurs en CSS)
//   <x-word class="…"></x-word>                             → nom vectorisé
//   <x-i n="qr-code" w="regular|bold|fill"></x-i>           → icône Phosphor en ligne
//   /*GEOMETRY*/                                            → géométrie pour le script 3D
//
//   node brand/v2/tools/build-charte.mjs <dossier des icônes Phosphor (assets)> [corps pour Artifact]

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const phosphor = process.argv[2];
if (!phosphor) throw new Error("chemin des icônes Phosphor attendu (…/@phosphor-icons/core/assets)");
const geo = JSON.parse(readFileSync(join(here, "geometry.json"), "utf8"));
let html = readFileSync(join(here, "charte.src.html"), "utf8");

const attrs = (s) => Object.fromEntries([...s.matchAll(/(\w[\w-]*)="([^"]*)"/g)].map((m) => [m[1], m[2]]));

const SEAM = "M5.6 42.4 L42.4 5.6"; // axe de la couture, raccourci aux pointes
html = html.replace(/<x-mark([^>]*)><\/x-mark>/g, (_, a) => {
  const { state = "now", class: cls = "", label, id } = attrs(a);
  const aria = (label ? `role="img" aria-label="${label}"` : `aria-hidden="true"`) + (id ? ` id="${id}"` : "");
  // m-halo et m-sheath : le néon de la couture (éteint au repos, voir .mark.lit).
  return `<svg class="mark ${cls}" data-state="${state}" viewBox="0 0 48 48" ${aria}><path class="m-src" d="${geo.source}"/><path class="m-dst" d="${geo.dest}"/><path class="m-halo" d="${SEAM}" filter="url(#kn-glow-wide)"/><path class="m-sheath" d="${SEAM}" filter="url(#kn-glow-soft)"/><path class="m-beam" d="${geo.beam}"/></svg>`;
});

const W = geo.wordmark;
html = html.replace(/<x-word([^>]*)><\/x-word>/g, (_, a) => {
  const { class: cls = "" } = attrs(a);
  return `<svg class="word ${cls}" viewBox="0 -74 ${W.width.toFixed(1)} 96" role="img" aria-label="KyberNow"><path class="w-kyber" d="${W.kyber}"/><path class="w-now" transform="translate(${W.nowOffset.toFixed(2)} 0)" d="${W.now}"/></svg>`;
});

// Planche d'icônes : chaque icône dans les trois graisses, la page choisit laquelle afficher.
const GRID = [
  ["desktop-tower", "Ordinateur"], ["monitor", "Écran virtuel"], ["game-controller", "Profil Jeu"], ["text-aa", "Profil Bureau"],
  ["lightning", "Profil Max"], ["wifi-high", "Réseau local"], ["shield-check", "VPN"], ["qr-code", "QR code"],
  ["link-simple", "Lien d'appairage"], ["lock-simple", "Empreinte vérifiée"], ["keyboard", "Clavier"], ["cursor", "Souris"],
  ["clipboard-text", "Presse-papiers"], ["speaker-high", "Son"], ["broadcast", "Flux en cours"], ["timer", "Latence"],
  ["sliders-horizontal", "Réglages"], ["plugs-connected", "Connecté"], ["wifi-slash", "Hors ligne"], ["arrows-out", "Plein écran"],
  ["power", "Arrêter"], ["television-simple", "Apple TV"], ["browser", "Console"], ["gear-six", "Préférences"],
];
html = html.replace("ICONS_GRID", GRID.map(([n, label]) =>
  `<figure>${["regular", "bold", "fill"].map((w) => `<x-i n="${n}" w="${w}" class="w w-${w}"></x-i>`).join("")}<figcaption>${label}</figcaption></figure>`).join("\n"));
html = html.replace("GEO_SRC", geo.source).replace("GEO_DST", geo.dest);

const used = new Set();
html = html.replace(/<x-i([^>]*)><\/x-i>/g, (_, a) => {
  const { n, w = "regular", class: cls = "" } = attrs(a);
  const file = join(phosphor, w, w === "regular" ? `${n}.svg` : `${n}-${w}.svg`);
  const svg = readFileSync(file, "utf8");
  used.add(`${n}/${w}`);
  const inner = svg.replace(/^[\s\S]*?<svg[^>]*>/, "").replace(/<\/svg>\s*$/, "");
  return `<svg class="ic ${cls}" viewBox="0 0 256 256" fill="currentColor" aria-hidden="true">${inner}</svg>`;
});

html = html.replace("/*GEOMETRY*/", `const GEO = ${JSON.stringify({ source: geo.source, dest: geo.dest })};`);

// Page complète pour une lecture locale ; le corps seul (sans doctype) pour une page Artifact.
const out = join(here, "..", "charte", "index.html");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `<!doctype html>\n<html lang="fr">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n<style>[hidden]{display:none!important}body{margin:0}</style>\n</head>\n<body>\n${html}\n</body>\n</html>\n`);
if (process.argv[3]) writeFileSync(process.argv[3], html);
console.log(`${out} · ${(html.length / 1024).toFixed(0)} Ko · ${used.size} icônes`);
