// Copie les icônes Phosphor utilisées par une app dans son catalogue d'images,
// en vectoriel et en modèle (elles prennent la couleur du texte). Un seul
// script pour les trois catalogues (Mac, iPhone et iPad, Apple TV).
//
//   node apps/macos/scripts/phosphor-icons.mjs <mac|ios|tvos> <…/node_modules/@phosphor-icons/core/assets>
//
// Dans le code : Icon("game-controller") ou Icon("x", bold: true). Chaque liste
// est exactement celle des icônes que le code compilé par l'app appelle : ses
// propres fichiers, ceux du Mac que son project.yml reprend et KyberKit ; les
// noms passés à une vue qui appelle Icon, les ternaires et tableaux de noms
// comptent. Le catalogue est vidé puis refait : une icône absente de la liste
// disparaît, une nouvelle icône s'ajoute ici avant de relancer le script.

import { readFileSync, writeFileSync, mkdirSync, rmSync, existsSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

const CATALOGS = {
  // App Mac (apps/macos/KyberNow, apps/macos/KyberKit).
  mac: {
    out: join(here, "..", "KyberNow", "Assets.xcassets", "Icons"),
    icons: [
      "arrow-counter-clockwise", "arrow-elbow-down-left", "arrow-left", "arrow-right", "arrow-up",
      "arrows-clockwise", "article", "caret-down", "chart-line", "check", "check-circle", "clipboard-text",
      "cloud", "copy", "corners-in", "corners-out", "cpu", "crosshair", "cursor", "desktop-tower", "dots-three",
      "eye", "game-controller", "gear-six", "globe-hemisphere-west", "image", "info", "key", "keyboard",
      "lightning", "link-simple", "microphone", "microphone-slash", "monitor", "plus", "power", "share-network",
      "shield-check", "sliders-horizontal", "sparkle", "speaker-high", "speaker-slash", "television-simple",
      "trash", "user", "video-camera", "video-camera-slash", "warning", "wifi-high", "x",
    ],
  },
  // iPhone et iPad (apps/ios/KyberNowIOS, KyberKitIOS, fichiers du Mac de project.yml).
  ios: {
    out: join(here, "..", "..", "ios", "KyberNowIOS", "Assets.xcassets", "Icons"),
    icons: [
      "arrow-left", "arrow-right", "arrow-up", "arrows-clockwise", "article", "check", "check-circle", "cloud",
      "copy", "cursor", "eye", "game-controller", "gear-six", "globe-hemisphere-west", "info", "key", "keyboard",
      "lightning", "link-simple", "lock-simple", "microphone", "microphone-slash", "monitor", "number-square-one",
      "plus", "power", "share-network", "sliders-horizontal", "sparkle", "speaker-high", "speaker-slash",
      "television-simple", "trash", "user", "video-camera", "video-camera-slash", "warning", "wifi-high", "x",
    ],
  },
  // Apple TV (apps/tvos/KyberNowTV, KyberKitTV, fichiers du Mac de project.yml).
  tvos: {
    out: join(here, "..", "..", "tvos", "KyberNowTV", "Assets.xcassets", "Icons"),
    icons: [
      "arrow-left", "arrow-right", "arrow-up", "arrows-clockwise", "article", "check", "check-circle", "cloud",
      "cursor", "device-mobile", "eye", "game-controller", "globe-hemisphere-west", "info", "key", "keyboard",
      "lightning", "link-simple", "monitor", "number-square-one", "play", "plus", "power", "share-network",
      "sliders-horizontal", "sparkle", "speaker-high", "speaker-slash", "television-simple", "trash", "user",
      "vibrate", "video-camera", "video-camera-slash", "warning", "wifi-high", "x",
    ],
  },
};

const [which, assets] = process.argv.slice(2);
const catalog = CATALOGS[which];
if (!catalog || !assets) throw new Error("usage : phosphor-icons.mjs <mac|ios|tvos> <…/@phosphor-icons/core/assets>");
const { out, icons } = catalog;

// Toutes les sources d'abord : un mauvais chemin ne vide pas le catalogue.
const weights = [["regular", ""], ["bold", "-bold"]];
const missing = icons.flatMap((name) => weights.map(([w, s]) => join(assets, w, `${name}${s}.svg`))).filter((f) => !existsSync(f));
if (missing.length) throw new Error(`icônes Phosphor introuvables :\n${missing.join("\n")}`);

// Ce que la nouvelle liste retire, pour le voir passer.
const wanted = new Set(icons.flatMap((name) => weights.map(([, s]) => `${name}${s}.imageset`)));
const gone = existsSync(out) ? readdirSync(out).filter((f) => f.endsWith(".imageset") && !wanted.has(f)) : [];

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
writeFileSync(join(out, "Contents.json"), JSON.stringify({ info: { author: "xcode", version: 1 }, properties: { "provides-namespace": true } }, null, 2));

for (const name of icons) {
  for (const [weight, suffix] of weights) {
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
if (gone.length) console.log(`retirées : ${gone.join(", ")}`);
console.log(`${icons.length} icônes × 2 graisses → ${out}`);
