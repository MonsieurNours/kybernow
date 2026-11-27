// Vérifie les catalogues de langue (npm run check) :
// - fr.json et en.json ont exactement les mêmes clés et les mêmes paramètres {nom} ;
// - chaque clé littérale passée à t('…') dans src/ existe en français.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const root = join(import.meta.dirname, '..')
const dir = join(root, 'src/lib/i18n')
const load = (lang) => JSON.parse(readFileSync(join(dir, `${lang}.json`), 'utf8'))

function flatten(node, prefix = '', out = new Map()) {
  for (const [k, v] of Object.entries(node)) {
    const key = prefix ? `${prefix}.${k}` : k
    if (v !== null && typeof v === 'object') flatten(v, key, out)
    else out.set(key, String(v))
  }
  return out
}

const params = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',')
const fr = flatten(load('fr'))
const en = flatten(load('en'))
const problems = []
for (const [k, v] of fr) {
  if (!en.has(k)) problems.push(`absente de en.json : ${k}`)
  else if (params(v) !== params(en.get(k))) problems.push(`paramètres différents pour ${k} : fr {${params(v)}} / en {${params(en.get(k))}}`)
}
for (const k of en.keys()) if (!fr.has(k)) problems.push(`absente de fr.json : ${k}`)

function walk(d) {
  for (const name of readdirSync(d)) {
    const p = join(d, name)
    if (statSync(p).isDirectory()) walk(p)
    else if (/\.(svelte|ts)$/.test(name)) {
      for (const m of readFileSync(p, 'utf8').matchAll(/\bt\(\s*['"]([\w.]+)['"]/g)) {
        if (!fr.has(m[1])) problems.push(`clé inconnue ${m[1]} dans ${relative(root, p)}`)
      }
    }
  }
}
walk(join(root, 'src'))

// Français : une espace fine insécable (U+202F) avant « : ; ! ? » (docs/i18n.md, TRANS-32), dans l'interface
// et dans le catalogue Rust de l'installateur (`setup/src/i18n`).
for (const [k, v] of [...fr, ...flatten(JSON.parse(readFileSync(join(root, '../src/i18n/fr.json'), 'utf8')))]) {
  if (typeof v === 'string' && /[ \u00a0][:;!?]/.test(v)) problems.push(`espace avant la ponctuation double, à remplacer par l'espace fine (U+202F) : ${k}`)
}

if (problems.length) {
  console.error(`i18n : ${problems.length} problème(s)\n  ${problems.join('\n  ')}`)
  process.exit(1)
}
console.log(`i18n : ${fr.size} clés, fr.json et en.json concordent, clés employées présentes`)
