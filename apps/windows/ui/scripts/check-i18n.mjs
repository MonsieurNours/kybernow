// Vérifie que fr.json et en.json ont exactement les mêmes clés (npm run check) :
// ceux de l'interface, et ceux du côté Rust de l'app et du cœur (leurs tests
// Rust le vérifient aussi pour le cœur ; l'app ne se teste que sous Windows).
import { readFileSync } from 'node:fs'

const catalogues = ['../src/lib/i18n/', '../../src/i18n/', '../../../core/src/i18n/']

function keys(node, prefix = '', out = new Set()) {
  for (const [k, v] of Object.entries(node)) {
    const key = prefix ? `${prefix}.${k}` : k
    if (v && typeof v === 'object') keys(v, key, out)
    else out.add(key)
  }
  return out
}

let failed = false
for (const path of catalogues) {
  const dir = new URL(path, import.meta.url)
  const load = (lang) => JSON.parse(readFileSync(new URL(`${lang}.json`, dir), 'utf8'))
  const fr = keys(load('fr'))
  const en = keys(load('en'))
  const missing = [...fr].filter((k) => !en.has(k))
  const extra = [...en].filter((k) => !fr.has(k))
  const name = dir.pathname.replace(/.*\/apps\//, 'apps/')
  if (missing.length) console.error(`i18n ${name} : absentes de en.json : ${missing.join(', ')}`)
  if (extra.length) console.error(`i18n ${name} : absentes de fr.json : ${extra.join(', ')}`)
  if (missing.length || extra.length) failed = true
  else console.log(`i18n ${name} : ${fr.size} clés, fr.json et en.json concordent`)
}
if (failed) process.exit(1)
