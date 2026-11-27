// Vérifie les catalogues de langue (lancé par `npm run check`) :
// - fr.json et en.json ont exactement les mêmes clés ;
// - chaque texte a les mêmes paramètres {nom} dans les deux langues ;
// - chaque clé littérale passée à t('…') / tn('…') dans src/ existe en français ;
// - chaque texte sert : sa clé est écrite quelque part dans src/ (entre guillemets, ou
//   en préfixe d'un gabarit `share.role.${…}`), pluriels et versions Mac ou Linux compris.

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

// Clés employées dans le code (littérales seulement), et tout ce qui peut en être une.
const literals = new Set()
const prefixes = new Set()
function walk(d) {
  for (const name of readdirSync(d)) {
    const p = join(d, name)
    if (statSync(p).isDirectory()) walk(p)
    else if (/\.(svelte|ts)$/.test(name)) {
      const src = readFileSync(p, 'utf8')
      for (const m of src.matchAll(/['"`]([A-Za-z]\w*(?:\.\w+)+)['"`]/g)) literals.add(m[1])
      for (const m of src.matchAll(/`([A-Za-z]\w*(?:\.\w+)*\.?)\$\{/g)) prefixes.add(m[1])
      for (const m of src.matchAll(/\bt(n?)\(\s*'([\w.]+)'/g)) {
        const key = m[2]
        const ok = m[1] ? a.has(`${key}.one`) && a.has(`${key}.other`) : a.has(key)
        if (!ok) problems.push(`clé inconnue ${key} dans ${relative(root, p)}`)
      }
    }
  }
}
walk(join(root, 'src'))

// Textes que rien n'emploie (WEB-23) : un pluriel compte par sa clé (`tn`), une version Mac ou Linux
// par le texte d'origine.
const PLURAL = /\.(zero|one|two|few|many|other)$/
function used(key) {
  if (literals.has(key)) return true
  if (PLURAL.test(key) && literals.has(key.replace(PLURAL, ''))) return true
  for (const p of prefixes) if (p && key.startsWith(p)) return true
  return false
}
for (const key of a.keys()) {
  const base = key.startsWith('macos.') || key.startsWith('linux.') ? key.slice(6) : key
  if (!used(base)) problems.push(`texte inutilisé : ${key}`)
}

// Version Mac ou Linux d'un texte propre à Windows (`macos.<clé>`, `linux.<clé>`,
// docs/i18n.md) : la clé doit exister.
for (const key of a.keys()) {
  if (key.startsWith('macos.') && !a.has(key.slice(6))) problems.push(`version Mac sans texte d'origine : ${key}`)
  if (key.startsWith('linux.') && !a.has(key.slice(6))) problems.push(`version Linux sans texte d'origine : ${key}`)
}

// Typographie (docs/i18n.md, TRANS-30, TRANS-32) : apostrophe droite partout ; en français, espace fine insécable
// (U+202F) avant « : ; ! ? », jamais une espace simple ou insécable ordinaire.
for (const [lang, m] of [['fr', a], ['en', b]]) {
  for (const [k, v] of m) if (v.includes('\u2019')) problems.push(`apostrophe typographique ’ (${lang}) : ${k}`)
}
for (const [k, v] of a) if (/[ \u00a0][:;!?]/.test(v)) problems.push(`espace avant « : ; ! ? » à remplacer par l'espace fine insécable : ${k}`)

if (problems.length) {
  console.error(`Catalogues de langue : ${problems.length} problème(s)\n  ${problems.join('\n  ')}`)
  process.exit(1)
}
console.log(`Catalogues de langue : ${a.size} textes, fr et en alignés.`)
