// Prêt de l'ordinateur (docs/pret.md) : heures, durées et lignes d'état, dans les mots de
// docs/link-ui.md (« Prêt de l'ordinateur »). Partagé par Admin et le client web.

import { i18n, locale, t } from './i18n/index.svelte'
import { sinceText } from './format'
import type { LendDuration, LendSteam, LendView } from './api'

/** Les durées d'un code pour une séance, dans l'ordre de link-ui (2 heures proposé). */
export const LEND_DURATIONS: LendDuration[] = [3600, 7200, 14400, null]

/** « 1 heure », « 2 heures », « 4 heures », « Jusqu'à ce que je reprenne ». */
export function durationName(d: LendDuration): string {
  return d === null ? t('lend.duration.untilBack') : t(`lend.duration.h${d / 3600}`)
}

/** « Son compte Steam », « Votre compte Steam » (vus par l'hôte). */
export function steamName(s: LendSteam): string {
  return t(`lend.steam.${s}`)
}

/** L'app d'un ami (« iPhone ou iPad », « Navigateur »), comme pour le partage. */
export function lendAppName(app: string): string {
  const name = t(`share.apps.${app}`)
  return name.startsWith('share.') ? app : name
}

function hm(hours: number, minutes: number): string {
  if (i18n.lang === 'fr') return `${hours} h${minutes ? ` ${String(minutes).padStart(2, '0')}` : ''}`
  const d = new Date(2024, 0, 1, hours, minutes)
  return d.toLocaleTimeString(locale(), { hour: 'numeric', ...(minutes ? { minute: '2-digit' } : {}) })
}

/** « 18 h », « 18 h 30 » (anglais : « 6 PM », « 6:30 PM ») : une heure en secondes Unix. */
export function hourText(seconds: number): string {
  const d = new Date(seconds * 1000)
  return hm(d.getHours(), d.getMinutes())
}

/** Une heure de créneau (« 14:00 ») dans les mêmes mots. */
export function slotHour(text: string): string {
  const [h, m] = text.split(':').map(Number)
  return Number.isFinite(h) ? hm(h, Number.isFinite(m) ? m : 0) : text
}

/** Le jour `n` (1 = lundi … 7 = dimanche) : « mercredi », « mer. ». */
export function dayName(n: number, style: 'long' | 'short' = 'long'): string {
  // Le 1er janvier 2024 est un lundi.
  return new Date(2024, 0, n).toLocaleDateString(locale(), { weekday: style })
}

/** « 1:42 » : le temps qui reste avant une échéance. */
export function countdown(seconds: number): string {
  const s = Math.max(0, Math.ceil(seconds))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

/** « Encore 1 h 20 », « Encore 12 min » : le temps qui reste au prêt. */
export function leftText(seconds: number): string {
  return t('lend.bar.left', { time: sinceText(seconds) })
}

/** L'ordinateur est prêté (ou en train de l'être, ou de revenir). */
export function isLent(view: LendView | null | undefined): boolean {
  return !!view?.borrower && view.state !== 'free'
}

/** « Prêté à Léa », « Prêté à Léa jusqu'à 18 h ». */
export function lentLine(view: LendView | null | undefined): string {
  const b = view?.borrower
  if (!b) return ''
  return b.until ? t('lend.host.lentUntil', { name: b.name, time: hourText(b.until) }) : t('lend.host.lentTo', { name: b.name })
}

/** Le préavis de reprise ou de fin en cours (pas le créneau d'un ami qui commence). */
export function takingBack(view: LendView | null | undefined): number | null {
  const n = view?.notice
  return n && n.reason !== 'slot_start' ? n.at : null
}

/** Le créneau d'un ami qui commence dans 2 min (une demande `slot`) : son prénom, ou `null`. */
export function slotStarting(view: LendView | null | undefined): string | null {
  const slot = view?.requests.find((r) => r.slot)
  if (slot) return slot.name
  const n = view?.notice
  return n?.reason === 'slot_start' ? (n.name ?? view?.borrower?.name ?? '') : null
}

/** Les demandes qui attendent une réponse de l'hôte (pas le créneau d'un ami, qui passe avant lui). */
export function askingRequests(view: LendView | null | undefined) {
  return view?.requests.filter((r) => !r.slot) ?? []
}
