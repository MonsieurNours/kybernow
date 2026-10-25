// Le nom de l'ordinateur dans une phrase : en français, « de » s'élide
// devant une voyelle (accentuée ou non) ou un h : « Réglages d'Atelier »,
// « Réglages d'Hugo-PC », mais « Réglages de Bureau-Salon ».

import { i18n, t, type Params } from '../lib/i18n/index.svelte'

const ELIDED = /^[aeiouyhàâäéèêëîïôöùûüÿæœ]/i

/** Remplace « de {nom} » par « d'{nom} » quand le français le demande. */
export function elide(text: string, name: string): string {
  if (i18n.lang !== 'fr' || !name || !ELIDED.test(name)) return text
  return text.replaceAll(`de ${name}`, `d'${name}`)
}

/** `t()` dont le paramètre `param` est un nom d'ordinateur (élision comprise). */
export function tName(key: string, param: string, name: string, params: Params = {}): string {
  return elide(t(key, { ...params, [param]: name }), name)
}
