// Langue de la console et du client web (/ecran) : celle du navigateur
// (`navigator.languages` : français si la première commence par « fr »,
// anglais sinon), ou le choix fait dans la console ou le client web
// (« Auto », « Français », « English »), gardé dans ce navigateur.
//
// Les textes vivent dans `fr.json` (la source) et `en.json`, mêmes clés,
// objets imbriqués par écran. Une clé absente en anglais retombe sur le
// français ; `npm run check` compare les deux fichiers (scripts/i18n-check.mjs).

import fr from './fr.json'
import en from './en.json'

export type Lang = 'fr' | 'en'
export type LangChoice = 'auto' | Lang

export const LANGS: Lang[] = ['fr', 'en']
const KEY = 'kybernow.lang'
const catalogs: Record<Lang, Record<string, unknown>> = { fr, en }

/** Langue du navigateur : français si sa première langue l'est, anglais sinon. */
export function systemLang(): Lang {
  const first = (typeof navigator !== 'undefined' && (navigator.languages?.[0] ?? navigator.language)) || ''
  return first.toLowerCase().startsWith('fr') ? 'fr' : 'en'
}

function loadChoice(): LangChoice {
  try {
    const v = localStorage.getItem(KEY)
    if (v === 'fr' || v === 'en') return v
  } catch {
    // Stockage indisponible (navigation privée) : langue du navigateur.
  }
  return 'auto'
}

const initial = loadChoice()

/** État réactif : `choice` (réglage) et `lang` (langue en vigueur). */
export const i18n = $state({ choice: initial, lang: initial === 'auto' ? systemLang() : initial })

export function setLangChoice(choice: LangChoice) {
  i18n.choice = choice
  i18n.lang = choice === 'auto' ? systemLang() : choice
  try {
    if (choice === 'auto') localStorage.removeItem(KEY)
    else localStorage.setItem(KEY, choice)
  } catch {
    // Sans stockage, le choix vaut pour cette visite.
  }
  applyDocumentLang()
}

/** `<html lang>` suit la langue (lecteurs d'écran, césure). */
export function applyDocumentLang() {
  if (typeof document !== 'undefined') document.documentElement.lang = i18n.lang
}

// Un autre onglet (console ou client web) a changé la langue.
if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (e.key === KEY || e.key === null) {
      const choice = loadChoice()
      i18n.choice = choice
      i18n.lang = choice === 'auto' ? systemLang() : choice
      applyDocumentLang()
    }
  })
  window.addEventListener('languagechange', () => {
    if (i18n.choice === 'auto') {
      i18n.lang = systemLang()
      applyDocumentLang()
    }
  })
}

export type Params = Record<string, string | number | null | undefined>

function lookup(catalog: Record<string, unknown>, key: string): unknown {
  let node: unknown = catalog
  for (const part of key.split('.')) {
    if (node === null || typeof node !== 'object') return undefined
    node = (node as Record<string, unknown>)[part]
  }
  return node
}

function fill(text: string, params?: Params): string {
  if (!params) return text
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name] ?? '') : whole))
}

/**
 * Texte de `key` (« overview.title ») dans la langue en vigueur, `{nom}`
 * remplacé par `params.nom`. Repli : le français, puis la clé elle-même.
 * Lu dans un gabarit ou un `$derived`, il suit les changements de langue.
 */
export function t(key: string, params?: Params): string {
  const text = lookup(catalogs[i18n.lang], key) ?? lookup(catalogs.fr, key)
  return typeof text === 'string' ? fill(text, params) : key
}

/**
 * Texte au singulier ou au pluriel : `key` est un objet `{ "one": …, "other": … }`
 * (`{n}` y est le nombre). Français : 0 et 1 au singulier ; anglais : 1 seulement.
 */
export function tn(key: string, n: number, params?: Params): string {
  const one = i18n.lang === 'fr' ? Math.abs(n) < 2 : Math.abs(n) === 1
  return t(`${key}.${one ? 'one' : 'other'}`, { n, ...params })
}

/** Locale des dates et nombres : « fr-FR », ou la variante anglaise du navigateur. */
export function locale(): string {
  if (i18n.lang === 'fr') return 'fr-FR'
  const nav = typeof navigator !== 'undefined' ? (navigator.languages ?? [navigator.language]) : []
  return nav.find((l) => l?.toLowerCase().startsWith('en')) ?? 'en-US'
}

/** « A », « A et B », « A, B et C » (« A, B and C »). */
export function list(items: string[]): string {
  if (items.length <= 1) return items[0] ?? ''
  return t('common.list', { rest: items.slice(0, -1).join(', '), last: items[items.length - 1] })
}

/** En-tête à joindre aux appels de l'API : la console répond dans cette langue. */
export function langHeaders(): Record<string, string> {
  return { 'accept-language': i18n.lang }
}

applyDocumentLang()
