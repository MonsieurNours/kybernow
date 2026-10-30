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

/** La couleur d'un ami (son avatar, ses créneaux dans la semaine) : la même à chaque fois. */
const AVATARS = ['#D4CEFF', '#9FE6C0', '#F8E2B0', '#FFC9C4', '#BDE3FF', '#F0C4FF', '#C9F2DA']
export function avatarColor(key: string): string {
  let h = 0
  for (const c of key) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return AVATARS[h % AVATARS.length]
}

/** L'initiale d'un prénom. */
export function initial(name: string): string {
  return (name.trim().charAt(0) || '?').toUpperCase()
}

/** Le jour de la semaine d'une date, 1 = lundi … 7 = dimanche. */
export function weekday(d: Date): number {
  return d.getDay() === 0 ? 7 : d.getDay()
}

/** « 14:00 » → minutes depuis minuit. */
export function slotMinutes(text: string): number {
  const [h, m] = text.split(':').map(Number)
  return (h || 0) * 60 + (m || 0)
}

/**
 * Le créneau d'ami le plus proche (en cours, ou le prochain dans les 7 jours), à l'heure de ce
 * navigateur : son prénom, son début et sa fin (secondes Unix), et s'il est en cours.
 */
export function nextSlot(
  friends: { name: string; enabled: boolean; slots: { days: number[]; start: string; end: string }[] }[],
  now = new Date(),
): { name: string; start: number; end: number; now: boolean } | null {
  let best: { name: string; start: number; end: number; now: boolean } | null = null
  for (const f of friends) {
    if (!f.enabled) continue
    for (const s of f.slots) {
      for (let ahead = 0; ahead <= 7; ahead++) {
        const day = new Date(now.getFullYear(), now.getMonth(), now.getDate() + ahead)
        if (!s.days.includes(weekday(day))) continue
        const start = day.getTime() / 1000 + slotMinutes(s.start) * 60
        const end = day.getTime() / 1000 + slotMinutes(s.end) * 60
        if (end <= now.getTime() / 1000) continue
        const cand = { name: f.name, start, end, now: start <= now.getTime() / 1000 }
        if (!best || cand.start < best.start) best = cand
        break
      }
    }
  }
  return best
}

/** « mar. 28 oct. » : le jour d'un prêt (secondes Unix). */
export function dayDate(seconds: number): string {
  return new Date(seconds * 1000).toLocaleDateString(locale(), { weekday: 'short', day: 'numeric', month: 'short' })
}

/** Écrit un texte dans le presse-papiers ; vrai s'il y est. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}
