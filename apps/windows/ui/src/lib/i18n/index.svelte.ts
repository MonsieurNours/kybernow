// Langue de l'interface (docs/i18n.md). L'app la décide (langue de Windows ou
// choix de ses réglages) et la donne aux deux fenêtres : `window.__KN_LANG`
// avant le premier affichage, puis l'événement `lang` à chaque changement.
// Dans un navigateur (imitation), `?lang=` s'il est là, sinon celle du
// navigateur. Le français est la source ; une clé absente en anglais retombe
// sur le français.

import en from './en.json'
import fr from './fr.json'

export type Lang = 'fr' | 'en'

type Catalogue = { [key: string]: string | Catalogue }
const catalogues: Record<Lang, Catalogue> = { fr, en }

/** `fr`, `fr-FR`, `fr-CA`… : français ; toute autre langue : anglais. */
export function langFromTag(tag: string | null | undefined): Lang {
  const t = (tag ?? '').trim().toLowerCase()
  return t === 'fr' || t.startsWith('fr-') || t.startsWith('fr_') ? 'fr' : 'en'
}

function initialLang(): Lang {
  if (typeof window === 'undefined') return 'fr'
  const given = (window as unknown as { __KN_LANG?: string }).__KN_LANG
  if (given) return langFromTag(given)
  const asked = new URLSearchParams(location.search).get('lang')
  if (asked) return langFromTag(asked)
  return langFromTag(navigator.languages?.[0] ?? navigator.language)
}

export const i18n = $state({ lang: initialLang() })

function lookup(catalogue: Catalogue, key: string): string | undefined {
  let node: string | Catalogue | undefined = catalogue
  for (const part of key.split('.')) {
    if (typeof node !== 'object') return undefined
    node = node[part]
  }
  return typeof node === 'string' ? node : undefined
}

/** Un nom qui commence par une voyelle (accentuées comprises) ou un h élide « de » : « d'Atelier », « d'Hugo-PC ». */
const elides = (name: string) => /^[aeiouyhàâäéèêëîïôöùûüÿæœ]/i.test(name.trim())

/**
 * Texte de `key` dans la langue courante, `{nom}` remplacé par sa valeur. En
 * français, « de {nom} » devient « d'{nom} » devant une voyelle ou un h
 * (« Réglages d'Atelier », mais « Réglages de Bureau-Salon »).
 */
export function t(key: string, params: Record<string, string | number> = {}): string {
  let text = lookup(catalogues[i18n.lang], key) ?? lookup(catalogues.fr, key) ?? key
  if (i18n.lang === 'fr') {
    text = text.replace(/\bde \{(\w+)\}/g, (all, name: string) => (name in params && elides(String(params[name])) ? `d'{${name}}` : all))
  }
  return text.replace(/\{(\w+)\}/g, (all, name: string) => (name in params ? String(params[name]) : all))
}

/** Un libellé qui finit par « de », suivi d'un nom affiché à part (« Réglages de » au-dessus du nom) : élidé en français. */
export function beforeName(text: string, name: string): string {
  return i18n.lang === 'fr' && / de$/.test(text) && elides(name) ? `${text.slice(0, -2)}d'` : text
}

/** Applique la langue (textes, attribut `lang` du document). */
export function setLang(lang: string) {
  i18n.lang = langFromTag(lang)
  if (typeof document !== 'undefined') document.documentElement.lang = i18n.lang
}

if (typeof document !== 'undefined') document.documentElement.lang = i18n.lang
