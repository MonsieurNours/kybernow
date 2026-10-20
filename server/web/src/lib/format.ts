// Dates, durées et mesures, en français.

const LOCALE = 'fr-FR'

/** « 21 h 41 » */
export function clock(seconds: number): string {
  return new Date(seconds * 1000).toLocaleTimeString(LOCALE, { hour: '2-digit', minute: '2-digit' }).replace(':', ' h ')
}

/** « 21:41 » pour les fils et journaux. */
export function hhmm(seconds: number): string {
  return new Date(seconds * 1000).toLocaleTimeString(LOCALE, { hour: '2-digit', minute: '2-digit' })
}

export function isToday(seconds: number): boolean {
  return new Date(seconds * 1000).toDateString() === new Date().toDateString()
}

/** « à l'instant », « il y a 2 h », « hier », « il y a 3 jours », « la semaine dernière ». */
export function ago(seconds: number): string {
  const d = Math.max(0, Date.now() / 1000 - seconds)
  if (d < 120) return "à l'instant"
  if (d < 3600) return `il y a ${Math.floor(d / 60)} min`
  const then = new Date(seconds * 1000)
  const today = new Date()
  const days = Math.round((new Date(today.toDateString()).getTime() - new Date(then.toDateString()).getTime()) / 86_400_000)
  if (days === 0) return `il y a ${Math.floor(d / 3600)} h`
  if (days === 1) return 'hier'
  if (days < 7) return `il y a ${days} jours`
  if (days < 14) return 'la semaine dernière'
  return `le ${then.toLocaleDateString(LOCALE, { day: 'numeric', month: 'long' })}`
}

/** « aujourd'hui à 14 h 01 », « hier à 23 h 12 », « le 12 octobre » */
export function when(seconds: number): string {
  const then = new Date(seconds * 1000)
  const days = Math.round((new Date(new Date().toDateString()).getTime() - new Date(then.toDateString()).getTime()) / 86_400_000)
  if (days === 0) return `aujourd'hui à ${clock(seconds)}`
  if (days === 1) return `hier à ${clock(seconds)}`
  return `le ${then.toLocaleDateString(LOCALE, { day: 'numeric', month: 'long' })}`
}

export function capital(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}

/** « 42 minutes », « 2 h 05 » */
export function duration(seconds: number): string {
  const m = Math.floor(seconds / 60)
  if (m < 1) return "moins d'une minute"
  if (m < 60) return m === 1 ? '1 minute' : `${m} minutes`
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')}`
}

/** « 8,3 ms » */
export function ms(value: number): string {
  return value >= 100 ? `${Math.round(value)} ms` : `${value.toFixed(1).replace('.', ',')} ms`
}

/** « 3456 × 2234 à 120 Hz » */
export function mode(m: { width: number; height: number; refresh: number } | null | undefined): string {
  return m ? `${m.width} × ${m.height} à ${m.refresh} Hz` : ''
}

export function deviceIcon(kind: string): string {
  return ['mac', 'tv', 'phone', 'tablet', 'browser'].includes(kind) ? kind : 'other'
}
