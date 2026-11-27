// L'enveloppe `fetch` commune de la console et du client web (WEB-20) : en-têtes (langue, `x-kyber-admin` pour
// les écritures, JSON), réponse lue en JSON ou en texte. Une Station injoignable (redémarrage, réseau coupé) donne une
// `NetworkError` traduite (WEB-07) au lieu du message brut du navigateur (« Failed to fetch », « Load failed »).

import { langHeaders, t } from './i18n/index.svelte'

export interface SendOptions {
  headers?: Record<string, string>
  signal?: AbortSignal
  /** La requête survit à la fermeture de la page (fin de session). */
  keepalive?: boolean
}

/** La Station ne répond pas du tout : pas de statut HTTP. */
export class NetworkError extends Error {
  readonly status = 0
  constructor() {
    super(t('common.unreachable'))
  }
}

/** `json` : la réponse est du JSON (celle d'Admin) ; faux pour une page d'erreur du relais ou d'un mandataire. */
export async function send(method: string, path: string, body?: unknown, opts: SendOptions = {}): Promise<{ res: Response; data: any; json: boolean }> {
  // La console répond dans la langue de l'interface (messages d'erreur, notes de version, fil).
  const headers: Record<string, string> = { ...langHeaders(), ...opts.headers }
  if (method !== 'GET') headers['x-kyber-admin'] = '1'
  if (body !== undefined) headers['content-type'] = 'application/json'
  let res: Response
  try {
    res = await fetch(path, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      credentials: 'same-origin',
      signal: opts.signal,
      keepalive: opts.keepalive,
    })
  } catch (e) {
    // Une attente annulée reste une annulation (`AbortError`, `TimeoutError`).
    if (e instanceof DOMException && (e.name === 'AbortError' || e.name === 'TimeoutError')) throw e
    throw new NetworkError()
  }
  let text = ''
  try {
    text = await res.text()
  } catch {
    throw new NetworkError()
  }
  let data: any = null
  let json = true
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    data = { error: text }
    json = false
  }
  return { res, data, json }
}

/** Erreur réseau ou annulation : rien à dire dans un relevé périodique (le suivant réessaie). */
export function isQuiet(e: unknown): boolean {
  return e instanceof NetworkError || (e instanceof DOMException && (e.name === 'AbortError' || e.name === 'TimeoutError'))
}
