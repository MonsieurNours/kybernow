// Dates, durées et mesures, dans la langue de la console.

import { i18n, locale, t, tn } from './i18n/index.svelte'

/** « 21 h 41 » (anglais : « 9:41 PM » selon le navigateur) */
export function clock(seconds: number): string {
  const time = new Date(seconds * 1000).toLocaleTimeString(locale(), { hour: '2-digit', minute: '2-digit' })
  return i18n.lang === 'fr' ? time.replace(':', ' h ') : time
}

/** « 21:41 » pour les fils et journaux. */
export function hhmm(seconds: number): string {
  return new Date(seconds * 1000).toLocaleTimeString(locale(), { hour: '2-digit', minute: '2-digit' })
}

export function isToday(seconds: number): boolean {
  return new Date(seconds * 1000).toDateString() === new Date().toDateString()
}

function day(then: Date): string {
  return then.toLocaleDateString(locale(), { day: 'numeric', month: 'long' })
}

/** « à l'instant », « il y a 2 h », « hier », « il y a 3 jours », « la semaine dernière ». */
export function ago(seconds: number): string {
  const d = Math.max(0, Date.now() / 1000 - seconds)
  if (d < 120) return t('format.now')
  if (d < 3600) return t('format.minutesAgo', { n: Math.floor(d / 60) })
  const then = new Date(seconds * 1000)
  const today = new Date()
  const days = Math.round((new Date(today.toDateString()).getTime() - new Date(then.toDateString()).getTime()) / 86_400_000)
  if (days === 0) return t('format.hoursAgo', { n: Math.floor(d / 3600) })
  if (days === 1) return t('format.yesterday')
  if (days < 7) return t('format.daysAgo', { n: days })
  if (days < 14) return t('format.lastWeek')
  return t('format.onDate', { date: day(then) })
}

/** « aujourd'hui à 14 h 01 », « hier à 23 h 12 », « le 12 octobre » */
export function when(seconds: number): string {
  const then = new Date(seconds * 1000)
  const days = Math.round((new Date(new Date().toDateString()).getTime() - new Date(then.toDateString()).getTime()) / 86_400_000)
  if (days === 0) return t('format.todayAt', { time: clock(seconds) })
  if (days === 1) return t('format.yesterdayAt', { time: clock(seconds) })
  return t('format.onDate', { date: day(then) })
}

export function capital(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}

/** « 42 minutes », « 2 h 05 » */
export function duration(seconds: number): string {
  const m = Math.floor(seconds / 60)
  if (m < 1) return t('format.lessThanMinute')
  if (m < 60) return tn('format.minutes', m)
  return t('format.hours', { h: Math.floor(m / 60), m: String(m % 60).padStart(2, '0') })
}

/** « 8,3 ms » (anglais : « 8.3 ms ») */
export function ms(value: number): string {
  if (value >= 100) return `${Math.round(value)} ms`
  const text = value.toFixed(1)
  return `${i18n.lang === 'fr' ? text.replace('.', ',') : text} ms`
}

/** « 3456 × 2234 à 120 Hz » */
export function mode(m: { width: number; height: number; refresh: number } | null | undefined): string {
  return m ? t('format.mode', m) : ''
}

/** « Écran 1 — 3840 × 2160 » : un écran du PC à partager (le libellé vient d'Admin, déjà traduit). */
export function screenLine(d: { label: string; width: number; height: number }): string {
  return t('share.display', { label: d.label, width: d.width, height: d.height })
}

export function deviceIcon(kind: string): string {
  return ['mac', 'tv', 'phone', 'tablet', 'browser'].includes(kind) ? kind : 'other'
}
