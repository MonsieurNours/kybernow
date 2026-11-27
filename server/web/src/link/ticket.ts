// Le jeton de streaming de la page (`GET /api/web/ticket`), à part de `backend.ts` pour ses essais.

import { ApiError } from '../lib/api'
import { send } from '../lib/http'
import { t } from '../lib/i18n/index.svelte'

export interface Ticket {
  token: string
  user: string
  expires_in: number
  kyber_port: number | null
  hostname: string | null
}

/**
 * Le jeton de streaming de cette page (WEB-14) : Admin en signe un de 12 h à chaque demande. Il est gardé et
 * réutilisé (session, rubrique Station qui n'en lit que le compte) tant qu'il lui reste plus d'une heure, au lieu
 * d'un nouveau jeton à chaque ouverture du panneau.
 */
let cachedTicket: { at: number; ticket: Promise<Ticket> } | null = null
const TICKET_MARGIN_S = 3600

export function ticket(): Promise<Ticket> {
  const now = Date.now()
  if (cachedTicket && now < cachedTicket.at) return cachedTicket.ticket
  const p = send('GET', '/api/web/ticket').then(({ res, data }) => {
    if (!res.ok) throw new ApiError(res.status, data?.error ?? t('common.error', { status: res.status }))
    return data as Ticket
  })
  const entry = { at: Number.POSITIVE_INFINITY, ticket: p }
  cachedTicket = entry
  p.then(
    (t) => {
      entry.at = now + Math.max(0, (t.expires_in ?? 0) - TICKET_MARGIN_S) * 1000
    },
    () => {
      if (cachedTicket === entry) cachedTicket = null
    },
  )
  return p
}

/** Le compte de streaming, d'après le jeton de la page (sans en signer un nouveau s'il y en a un). */
export async function streamUser(): Promise<string> {
  return (await ticket()).user
}

/** Le compte de ce navigateur a changé (déconnexion) : le jeton suivant est redemandé. */
export function forgetTicket() {
  cachedTicket = null
}
