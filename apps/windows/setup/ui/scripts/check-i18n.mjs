// Vérifie que fr.json et en.json ont exactement les mêmes clés (npm run check).
import { readFileSync } from 'node:fs'

const dir = new URL('../src/lib/i18n/', import.meta.url)
const load = (lang) => JSON.parse(readFileSync(new URL(`${lang}.json`, dir), 'utf8'))

function keys(node, prefix = '', out = new Set()) {
  for (const [k, v] of Object.entries(node)) {
    const key = prefix ? `${prefix}.${k}` : k
    if (v && typeof v === 'object') keys(v, key, out)
    else out.add(key)
  }
  return out
}

const fr = keys(load('fr'))
const en = keys(load('en'))
const missing = [...fr].filter((k) => !en.has(k))
const extra = [...en].filter((k) => !fr.has(k))
if (missing.length || extra.length) {
  if (missing.length) console.error(`i18n : absentes de en.json : ${missing.join(', ')}`)
  if (extra.length) console.error(`i18n : absentes de fr.json : ${extra.join(', ')}`)
  process.exit(1)
}
console.log(`i18n : ${fr.size} clés, fr.json et en.json concordent`)
