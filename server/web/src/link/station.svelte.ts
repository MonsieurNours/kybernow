// Redémarrer et mettre à jour la Station depuis le client web (rubrique
// Station des réglages, et dernier recours de la connexion), puis attendre
// son retour.
//
// La Station redémarre Admin avec elle : la session d'Admin de ce navigateur
// ne survit pas (son cookie n'est plus reconnu). La page suit donc le retour
// par `GET /api/session`, qui répond sans compte et donne la version, puis se
// recharge (pages neuves après une mise à jour) en laissant une phrase à
// montrer au retour (`takeNotice`) ; le mot de passe est alors redemandé.

import { backend, StationRefusal } from './backend'
import { deviceId } from './prefs'
import { t } from '../lib/i18n/index.svelte'

export type Action = 'restart' | 'update'

export const station = $state({
  /** Ce qui est en cours : la demande, puis l'attente du retour. */
  running: null as Action | null,
  /** Un appareil est en session : confirmer avant de couper sa session. */
  confirm: null as { action: Action; device: string } | null,
  /** Dernier refus ou échec, en une phrase. */
  message: '',
})

const NOTICE_KEY = 'kybernow.ecran.notice'
/** Reprendre la connexion après le mot de passe (dernier recours). */
const RESUME_KEY = 'kybernow.ecran.resume'
/** Heure du dernier recours : une fois, pas à chaque échec. */
const LAST_RESORT_KEY = 'kybernow.ecran.lastResort'
const LAST_RESORT_SPACING_MS = 10 * 60_000
/** Interrogation pendant l'attente (`info` toutes les 8 s le reste du temps). */
const POLL_MS = 2000
/** Redémarrage : 1 min 30 au plus ; mise à jour : téléchargée avant la réponse, puis posée. */
const LIMIT_MS = { restart: 90_000, update: 600_000 }
/** Revenue avec l'ancienne version : le temps qu'Admin repasse à la nouvelle. */
const OLD_VERSION_GRACE_MS = 30_000

type Outcome = { result: 'back'; version: string | null } | { result: 'failed' } | { result: 'slow' }

/** Attend qu'Admin réponde de nouveau : après un arrêt (ou une session
 * d'Admin oubliée) pour un redémarrage, à la nouvelle version pour une mise
 * à jour. `failed` : la mise à jour est revenue à l'ancienne version (ou ne
 * s'est pas faite) ; `slow` : Admin ne répond toujours pas. */
async function waitForStation(action: Action, target: string | null): Promise<Outcome> {
  const start = Date.now()
  let down = false
  let oldSince = 0
  let answered = false
  while (Date.now() - start < LIMIT_MS[action]) {
    await new Promise((r) => setTimeout(r, POLL_MS))
    const s = await backend.session().catch(() => null)
    answered = !!s
    if (!s) {
      down = true
      continue
    }
    if (action === 'restart' && (down || !s.authenticated)) return { result: 'back', version: s.version ?? null }
    if (action === 'update') {
      if (target ? s.version === target : down) return { result: 'back', version: s.version ?? target }
      if (down) {
        oldSince ||= Date.now()
        if (Date.now() - oldSince > OLD_VERSION_GRACE_MS) return { result: 'failed' }
      }
    }
  }
  return action === 'update' && answered ? { result: 'failed' } : { result: 'slow' }
}

/** Recharge la page avec une phrase à montrer au retour, et la reprise de la connexion si demandée. */
function reloadWith(text: string, resume = false) {
  try {
    sessionStorage.setItem(NOTICE_KEY, text)
    if (resume) sessionStorage.setItem(RESUME_KEY, String(Date.now()))
  } catch {
    // Sans stockage, la page revient sans la phrase.
  }
  location.reload()
}

export async function run(action: Action, force = false) {
  if (station.running) return
  station.confirm = null
  station.message = ''
  station.running = action
  try {
    const body = { device_id: deviceId(), force }
    const target = action === 'update' ? (await backend.stationUpdate(body)).version : null
    if (action === 'restart') await backend.stationRestart(body)
    const out = await waitForStation(action, target)
    if (out.result === 'back') {
      reloadWith(action === 'update' ? t('web.station.updated', { version: out.version ?? '' }) : t('web.station.restarted'))
      return
    }
    station.running = null
    station.message = out.result === 'failed' ? t('web.station.updateFailed') : t('web.station.slow')
  } catch (e) {
    station.running = null
    if (!(e instanceof StationRefusal)) {
      station.message = e instanceof Error ? e.message : String(e)
      return
    }
    if (e.code === 'session' && !force) station.confirm = { action, device: e.device ?? t('web.station.someDevice') }
    else if (e.code === 'too_soon') station.message = t('web.station.tooSoon')
    else station.message = e.message
  }
}

export function cancelConfirm() {
  station.confirm = null
}

/** Le dernier recours n'a pas servi dans les dix dernières minutes. */
export function lastResortAllowed(): boolean {
  try {
    return Date.now() - Number(sessionStorage.getItem(LAST_RESORT_KEY) ?? 0) > LAST_RESORT_SPACING_MS
  } catch {
    return false
  }
}

/**
 * Dernier recours de la connexion (docs/link-ui.md) : Admin répond, mais
 * Kyber ne se connecte pas ou ne démarre pas l'image. Redémarre la Station,
 * sans `force` (la session d'un autre appareil n'est jamais coupée : Admin
 * refuse), une fois, attend son retour, puis recharge la page : le mot de
 * passe est redemandé et la connexion reprend d'elle-même (`takeResume`),
 * sauf si `resume()` dit non (Annuler pendant l'attente).
 * `refused` : rien n'a été redémarré ; `slow` : la Station tarde à revenir.
 */
export async function restartForConnection(resume: () => boolean): Promise<'refused' | 'slow'> {
  try {
    sessionStorage.setItem(LAST_RESORT_KEY, String(Date.now()))
  } catch {
    // Sans stockage, pas de dernier recours : on ne saurait pas qu'il a déjà servi.
    return 'refused'
  }
  if (station.running) return 'refused'
  try {
    await backend.stationRestart({ device_id: deviceId(), force: false })
  } catch {
    return 'refused'
  }
  station.running = 'restart'
  const out = await waitForStation('restart', null)
  if (out.result === 'back') {
    reloadWith(t('web.station.restarted'), resume())
    return new Promise(() => {})
  }
  station.running = null
  return 'slow'
}

/** Reprendre la connexion après le mot de passe : demandé par le dernier recours, il y a moins de dix minutes. */
export function takeResume(): boolean {
  try {
    const at = Number(sessionStorage.getItem(RESUME_KEY) ?? 0)
    sessionStorage.removeItem(RESUME_KEY)
    return Date.now() - at < LAST_RESORT_SPACING_MS
  } catch {
    return false
  }
}

/** Phrase laissée par une attente avant le rechargement, lue une fois. */
export function takeNotice(): string {
  try {
    const text = sessionStorage.getItem(NOTICE_KEY) ?? ''
    sessionStorage.removeItem(NOTICE_KEY)
    return text
  } catch {
    return ''
  }
}
