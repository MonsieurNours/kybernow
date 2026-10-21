// Langue de l'interface : celle de Windows, donnée par l'installateur
// (window.__knLang) avant le démarrage de la page. Dans un navigateur
// (simulation) : ?lang=en|fr, sinon la langue du navigateur.
// Le français est la source : une clé absente en anglais retombe dessus.

import en from './en.json'
import fr from './fr.json'

export type Lang = 'fr' | 'en'

const catalogs: Record<Lang, unknown> = { fr, en }

/** fr, fr-FR, fr-CA… : français ; toute autre langue : anglais. */
export function fromTag(tag: string | null | undefined): Lang {
  return (tag ?? '').trim().toLowerCase().startsWith('fr') ? 'fr' : 'en'
}

function detect(): Lang {
  if (typeof window === 'undefined') return 'fr'
  if (window.__knLang) return fromTag(window.__knLang)
  const forced = new URLSearchParams(location.search).get('lang')
  if (forced) return fromTag(forced)
  return fromTag(navigator.languages?.[0] ?? navigator.language)
}

export const i18n = $state({ lang: detect() })

function lookup(lang: Lang, key: string): string | undefined {
  let node: unknown = catalogs[lang]
  for (const part of key.split('.')) {
    if (node === null || typeof node !== 'object') return undefined
    node = (node as Record<string, unknown>)[part]
  }
  return typeof node === 'string' ? node : undefined
}

/** Texte de `key` (« ecran.cle ») dans la langue courante, `{nom}` remplacé par sa valeur. */
export function t(key: string, vars: Record<string, string | number> = {}): string {
  const text = lookup(i18n.lang, key) ?? lookup('fr', key) ?? key
  return text.replace(/\{(\w+)\}/g, (all, name: string) => (name in vars ? String(vars[name]) : all))
}
