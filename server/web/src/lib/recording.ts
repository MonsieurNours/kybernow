// Enregistrement de la session (docs/enregistrement.md, docs/link-ui.md, « Enregistrement de la
// session ») : l'état que donnent les signes de vie (hôte et invité) et `GET /api/recording`, le temps
// (« 02:14 »), les avis brefs (mêmes règles que le cœur, apps/core/src/recording.rs), et les formats
// de la liste des fichiers d'Admin. Partagé par Admin et le client web.

import { locale, t } from './i18n/index.svelte'

/** Pourquoi le dernier enregistrement s'est fini (`last_end.reason` d'Admin). */
export type RecordingEndReason = 'stopped' | 'ended' | 'write' | 'disk_full' | 'codec' | 'lent'

/** L'état d'Admin : `elapsed` en secondes (compté par la Station), `started_at` en ms Unix. */
export interface RecordingState {
  allowed: boolean
  active: boolean
  elapsed: number | null
  started_at: number | null
  last_end: { reason: RecordingEndReason | string; at: number } | null
}

/** `recording` d'un signe de vie, ou `null` : une Station qui n'enregistre pas n'en envoie pas (rien à montrer). */
export function readRecording(v: unknown): RecordingState | null {
  if (!v || typeof v !== 'object') return null
  const r = v as Record<string, unknown>
  const num = (x: unknown) => (typeof x === 'number' && Number.isFinite(x) ? x : null)
  const end = r.last_end && typeof r.last_end === 'object' ? (r.last_end as Record<string, unknown>) : null
  const active = r.active === true
  return {
    allowed: r.allowed === true,
    active,
    elapsed: active ? num(r.elapsed) : null,
    started_at: active ? num(r.started_at) : null,
    last_end: end ? { reason: String(end.reason ?? 'stopped'), at: num(end.at) ?? 0 } : null,
  }
}

/** « 02:14 », puis « 1:02:14 » au-delà d'une heure. */
export function recordingClock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds))
  const h = Math.floor(s / 3600)
  const mm = String(Math.floor((s % 3600) / 60)).padStart(2, '0')
  const ss = String(s % 60).padStart(2, '0')
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
}

/**
 * Secondes écoulées maintenant : `elapsed` du dernier état, plus le temps passé depuis qu'il est arrivé
 * (`receivedAt`, `now` : ms de cette page). Le temps vient de la Station : tous les appareils montrent le même.
 */
export function elapsedNow(state: RecordingState | null, receivedAt: number, now: number): number {
  if (!state?.active) return 0
  return (state.elapsed ?? 0) + Math.max(0, Math.floor((now - receivedAt) / 1000))
}

/**
 * Un texte du cœur avec `{shortcut}` : le raccourci entre parenthèses s'il y en a un, sinon sans la
 * parenthèse (le client web n'a pas de raccourci pour enregistrer).
 */
export function withShortcut(key: string, shortcut = ''): string {
  if (shortcut) return t(key, { shortcut })
  return t(key, { shortcut: '\u0000' }).replace(/\s*\(\u0000\)/, '').replace('\u0000', '')
}

/** Infobulle du bouton de l'hôte : coupé dans Admin, arrêter ou enregistrer. */
export function recordingTooltip(state: RecordingState, shortcut = ''): string {
  if (!state.allowed && !state.active) return t('recording.disabled')
  return withShortcut(state.active ? 'recording.stop_tooltip' : 'recording.record_tooltip', shortcut)
}

/** Qui regarde : l'hôte, ou un invité du partage (avec le nom de l'ordinateur). */
export type RecordingViewer = { guest: string } | 'host'

/**
 * L'avis bref quand l'état change (`before` : l'état d'avant, `undefined` au premier signe de vie) :
 * l'hôte au lancement et à la fin (rien quand la session se finit ou qu'un prêt commence) ; un invité
 * à l'arrivée dans une session enregistrée, au lancement et à l'arrêt. Mêmes règles que le cœur.
 */
export function recordingNotice(before: RecordingState | null | undefined, after: RecordingState | null, viewer: RecordingViewer): string | null {
  if (!after) return null
  const was = !!before?.active
  const same = !!before?.active && before.started_at === after.started_at
  if (viewer !== 'host') {
    if (after.active && !same) return t('recording.guest_notice', { name: viewer.guest })
    if (was && !after.active) return t('recording.guest_stopped')
    return null
  }
  if (!was && after.active) return before !== undefined ? t('recording.started') : null
  if (was && !after.active) {
    switch (after.last_end?.reason) {
      case 'disk_full':
      case 'write':
        return t('recording.stopped_disk')
      case 'codec':
        return t('recording.stopped_codec')
      case 'ended':
      case 'lent':
        return null
      default:
        return t('recording.saved')
    }
  }
  return null
}

// --- Liste des fichiers d'Admin -------------------------------------------------

function decimal(value: number, digits = 1): string {
  return new Intl.NumberFormat(locale(), { maximumFractionDigits: digits }).format(value)
}

/** « 1,4 Go », « 820 Mo » (anglais : « 1.4 GB », « 820 MB »). */
export function fileSize(bytes: number): string {
  const gb = bytes / 1e9
  if (gb >= 1) return t('recordings.size.gb', { n: decimal(gb) })
  return t('recordings.size.mb', { n: decimal(bytes > 0 ? Math.max(1, bytes / 1e6) : 0, 0) })
}

/** « 37 Go libres » : la place libre, en Go entiers. */
export function freeSpace(bytes: number): string {
  return t('recordings.free', { n: decimal(Math.floor(bytes / 1e9), 0) })
}

/** « 12 min », « 1 h 05 », « 40 s ». */
export function fileDuration(ms: number): string {
  const s = Math.round(ms / 1000)
  if (s < 60) return t('recordings.duration.seconds', { n: s })
  const m = Math.round(s / 60)
  if (m < 60) return t('recordings.duration.minutes', { n: m })
  return t('recordings.duration.hours', { h: Math.floor(m / 60), m: String(m % 60).padStart(2, '0') })
}

/** « 26 nov. 2026 · 14:32 » (anglais : « Nov 26, 2026 · 2:32 PM »), à l'heure de ce navigateur. */
export function fileDate(seconds: number): string {
  const d = new Date(seconds * 1000)
  const date = d.toLocaleDateString(locale(), { day: 'numeric', month: 'short', year: 'numeric' })
  const time = d.toLocaleTimeString(locale(), { hour: '2-digit', minute: '2-digit' })
  return `${date} · ${time}`
}
