// Vérifie les catalogues de langue (lancé par `npm run check`) :
// - fr.json et en.json ont exactement les mêmes clés ;
// - chaque texte a les mêmes paramètres {nom} dans les deux langues ;
// - chaque clé littérale passée à t('…') / tn('…') dans src/ existe en français.

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const root = join(import.meta.dirname, '..')
const dir = join(root, 'src/lib/i18n')
const fr = JSON.parse(readFileSync(join(dir, 'fr.json'), 'utf8'))
const en = JSON.parse(readFileSync(join(dir, 'en.json'), 'utf8'))

function flatten(node, prefix = '', out = new Map()) {
  for (const [k, v] of Object.entries(node)) {
    const key = prefix ? `${prefix}.${k}` : k
    if (v !== null && typeof v === 'object') flatten(v, key, out)
    else out.set(key, String(v))
  }
  return out
}

const params = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',')
const a = flatten(fr)
const b = flatten(en)
const problems = []
for (const [k, v] of a) {
  if (!b.has(k)) problems.push(`absent de en.json : ${k}`)
  else if (params(v) !== params(b.get(k))) problems.push(`paramètres différents pour ${k} : fr {${params(v)}} / en {${params(b.get(k))}}`)
}
for (const k of b.keys()) if (!a.has(k)) problems.push(`absent de fr.json : ${k}`)

// Clés employées dans le code (littérales seulement).
function walk(d) {
  for (const name of readdirSync(d)) {
    const p = join(d, name)
    if (statSync(p).isDirectory()) walk(p)
    else if (/\.(svelte|ts)$/.test(name)) {
      const src = readFileSync(p, 'utf8')
      for (const m of src.matchAll(/\bt(n?)\(\s*'([\w.]+)'/g)) {
        const key = m[2]
        const ok = m[1] ? a.has(`${key}.one`) && a.has(`${key}.other`) : a.has(key)
        if (!ok) problems.push(`clé inconnue ${key} dans ${relative(root, p)}`)
      }
    }
  }
}
walk(join(root, 'src'))

if (problems.length) {
  console.error(`Catalogues de langue : ${problems.length} problème(s)\n  ${problems.join('\n  ')}`)
  process.exit(1)
}
console.log(`Catalogues de langue : ${a.size} textes, fr et en alignés.`)
