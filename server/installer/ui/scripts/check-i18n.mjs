// Même clés et mêmes {variables} dans fr.json et en.json (lancé par npm run check).
import { readFileSync } from 'node:fs'

const load = (lang) => JSON.parse(readFileSync(new URL(`../src/lib/i18n/${lang}.json`, import.meta.url), 'utf8'))
const flatten = (node, prefix = '', out = {}) => {
  for (const [key, value] of Object.entries(node)) {
    const path = prefix ? `${prefix}.${key}` : key
    if (value && typeof value === 'object') flatten(value, path, out)
    else out[path] = String(value)
  }
  return out
}
const vars = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',')

const fr = flatten(load('fr'))
const en = flatten(load('en'))
const problems = [
  ...Object.keys(fr).filter((k) => !(k in en)).map((k) => `absente en anglais : ${k}`),
  ...Object.keys(en).filter((k) => !(k in fr)).map((k) => `absente en français : ${k}`),
  ...Object.keys(fr).filter((k) => k in en && vars(fr[k]) !== vars(en[k])).map((k) => `variables différentes : ${k}`),
]
if (problems.length) {
  console.error(problems.join('\n'))
  process.exit(1)
}
console.log(`i18n : ${Object.keys(fr).length} clés, français et anglais alignés`)
