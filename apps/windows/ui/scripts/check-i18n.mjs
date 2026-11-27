// Vérifie les catalogues de langue (npm run check) :
// - fr.json et en.json ont exactement les mêmes clés, pour l'interface, le
//   côté Rust de l'app, le cœur (leurs tests Rust le vérifient aussi, sur le
//   Mac : `catalogues` de src/menu.rs pour l'app) et l'imitation (mock) ;
// - chaque texte a les mêmes paramètres {nom} dans les deux langues ;
// - chaque clé littérale passée à t('…') dans ui/src existe dans le
//   catalogue de l'interface (ou de l'imitation pour mock.ts), et chaque
//   t("…") du côté Rust (apps/windows/src) dans celui de l'app ;
// - aucune clé de l'interface n'est inutilisée (une clé compte comme
//   employée si elle paraît en toutes lettres dans le code, ou si elle
//   commence par un préfixe construit : t(`home.${x}`), `'errors.' + code`).
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const here = import.meta.dirname
const ui = join(here, '..')
const app = join(ui, '..')

function flatten(node, prefix = '', out = new Map()) {
  for (const [k, v] of Object.entries(node)) {
    const key = prefix ? `${prefix}.${k}` : k
    if (v !== null && typeof v === 'object') flatten(v, key, out)
    else out.set(key, String(v))
  }
  return out
}

const params = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',')
const problems = []

/** Les deux catalogues d'un dossier, comparés ; rend le français. */
function pair(dir, fr = 'fr.json', en = 'en.json', wrap = null) {
  const load = (f) => {
    const v = JSON.parse(readFileSync(join(dir, f), 'utf8'))
    return wrap ? { [wrap]: v } : v
  }
  const a = flatten(load(fr))
  const b = flatten(load(en))
  const name = relative(join(app, '..', '..'), join(dir, fr))
  for (const [k, v] of a) {
    if (!b.has(k)) problems.push(`${name} : absente de ${en} : ${k}`)
    else if (params(v) !== params(b.get(k))) problems.push(`${name} : paramètres différents pour ${k} : fr {${params(v)}} / en {${params(b.get(k))}}`)
  }
  for (const k of b.keys()) if (!a.has(k)) problems.push(`${name} : absente de ${fr} : ${k}`)
  console.log(`i18n ${name} : ${a.size} clés`)
  return a
}

function files(dir, pattern, out = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) files(p, pattern, out)
    else if (pattern.test(name)) out.push(p)
  }
  return out
}

const uiDir = join(ui, 'src/lib/i18n')
const uiKeys = pair(uiDir)
const mockKeys = pair(uiDir, 'mock.fr.json', 'mock.en.json', 'mock')
const rustKeys = pair(join(app, 'src/i18n'))
pair(join(app, '../core/src/i18n'))

// Interface : clés littérales employées, et clés jamais employées.
const sources = files(join(ui, 'src'), /\.(svelte|ts)$/)
const text = new Map(sources.map((p) => [p, readFileSync(p, 'utf8')]))
for (const [p, src] of text) {
  const known = p.endsWith('mock.ts') ? (k) => uiKeys.has(k) || mockKeys.has(k) : (k) => uiKeys.has(k)
  for (const m of src.matchAll(/\bt\(\s*['"]([\w.]+)['"]/g)) {
    if (!known(m[1])) problems.push(`clé inconnue ${m[1]} dans ${relative(ui, p)}`)
  }
}
const all = [...text.entries()].filter(([p]) => !p.endsWith('mock.ts')).map(([, s]) => s).join('\n')
// Préfixes construits : `share.quality_${…}` et 'errors.' + …
const prefixes = new Set()
for (const m of all.matchAll(/`([a-z][\w]*\.[\w.]*)\$\{/g)) prefixes.add(m[1])
for (const m of all.matchAll(/['"]([a-z][\w]*\.[\w.]*)['"]\s*\+/g)) prefixes.add(m[1])
for (const key of uiKeys.keys()) {
  const literal = all.includes(`'${key}'`) || all.includes(`"${key}"`) || all.includes(`\`${key}\``)
  if (!literal && ![...prefixes].some((p) => key.startsWith(p))) problems.push(`clé jamais employée par l'interface : ${key}`)
}

// Côté Rust de l'app : t("…") et tr(lang, "…") (pas ceux du cœur,
// `kybernow_core::i18n::tr`, qui lisent son catalogue).
for (const p of files(join(app, 'src'), /\.rs$/)) {
  const src = readFileSync(p, 'utf8')
  for (const m of src.matchAll(/(?<!(?:core_i18n|kybernow_core::i18n)::)\b(?:t\(|tr\([^,()]*,)\s*"([a-z_]+\.[\w.]+)"/g)) {
    if (!rustKeys.has(m[1])) problems.push(`clé inconnue ${m[1]} dans ${relative(app, p)}`)
  }
}

// Français : une espace fine insécable (U+202F) avant « : ; ! ? » (docs/i18n.md, TRANS-32), dans les catalogues
// de l'app (interface, imitation, côté Rust) ; celui du cœur se vérifie de son côté.
for (const f of [join(uiDir, 'fr.json'), join(uiDir, 'mock.fr.json'), join(app, 'src/i18n/fr.json')]) {
  for (const [k, v] of flatten(JSON.parse(readFileSync(f, 'utf8')))) {
    if (/[ \u00a0][:;!?]/.test(v)) problems.push(`${relative(join(app, '..', '..'), f)} : espace avant la ponctuation double, à remplacer par l'espace fine (U+202F) : ${k}`)
  }
}

if (problems.length) {
  console.error(`Catalogues de langue : ${problems.length} problème(s)\n  ${problems.join('\n  ')}`)
  process.exit(1)
}
console.log('Catalogues de langue : fr et en alignés, clés employées présentes, aucune inutilisée, espaces fines en français.')
