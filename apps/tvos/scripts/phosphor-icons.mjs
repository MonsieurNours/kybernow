// Copie les icônes Phosphor utilisées par l'app dans le catalogue d'images,
// en vectoriel et en modèle (elles prennent la couleur du texte).
//
//   node scripts/phosphor-icons.mjs (apps/tvos)
//   node scripts/phosphor-icons.mjs <…/node_modules/@phosphor-icons/core/assets>
//
// Dans le code : Icon("game-controller") ou Icon("x", bold: true).

import { readFileSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ICONS = [
  "game-controller", "user", "sliders-horizontal", "monitor", "arrow-right", "arrow-left", "speaker-high", "speaker-slash",
  "warning", "wifi-high", "plus", "cloud", "check", "keyboard", "power", "play", "cursor", "headphones", "television-simple",
  "battery-high", "battery-medium", "battery-low", "vibrate", "number-square-one", "hand-pointing", "arrows-clockwise",
  "lock-simple", "link-simple", "device-mobile", "trash", "x", "lightning", "article", "globe-hemisphere-west",
];

const assets = process.argv[2];
if (!assets) throw new Error("chemin des icônes Phosphor attendu");
const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "KyberNowTV", "Assets.xcassets", "Icons");
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
writeFileSync(join(out, "Contents.json"), JSON.stringify({ info: { author: "xcode", version: 1 }, properties: { "provides-namespace": true } }, null, 2));

for (const name of ICONS) {
  for (const [weight, suffix] of [["regular", ""], ["bold", "-bold"]]) {
    const file = join(assets, weight, `${name}${suffix}.svg`);
    const svg = readFileSync(file, "utf8").replace(/currentColor/g, "#000000");
    const set = join(out, `${name}${suffix}.imageset`);
    mkdirSync(set, { recursive: true });
    writeFileSync(join(set, `${name}${suffix}.svg`), svg);
    writeFileSync(join(set, "Contents.json"), JSON.stringify({
      images: [{ filename: `${name}${suffix}.svg`, idiom: "universal" }],
      info: { author: "xcode", version: 1 },
      properties: { "preserves-vector-representation": true, "template-rendering-intent": "template" },
    }, null, 2));
  }
}
console.log(`${ICONS.length} icônes × 2 graisses → ${out}`);
