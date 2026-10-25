// Appels du client web à la console (même origine, session de la console).

import { ApiError } from '../lib/api'
import { langHeaders, t } from '../lib/i18n/index.svelte'
import type { Mode } from './device'

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  const { res, data } = await request(method, path, body)
  if (!res.ok) throw new ApiError(res.status, data?.error ?? t('common.error', { status: res.status }))
  return data as T
}

async function request(method: string, path: string, body?: unknown): Promise<{ res: Response; data: any }> {
  // La console répond dans la langue de ce client (messages d'erreur).
  const headers: Record<string, string> = { ...langHeaders() }
  if (method !== 'GET') headers['x-kyber-admin'] = '1'
  if (body !== undefined) headers['content-type'] = 'application/json'
  const res = await fetch(path, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    credentials: 'same-origin',
  })
  const text = await res.text()
  let data: any = null
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    data = { error: text }
  }
  return { res, data }
}

/** Refus d'une route de la Station (`code` : `session`, `too_soon`…, voir server/README.md). */
export class StationRefusal extends Error {
  constructor(
    public status: number,
    message: string,
    public code: string,
    /** L'appareil en session (`code` : `session`). */
    public device: string | null,
    /** Secondes à attendre (`too_soon`, `locked`). */
    public retryAfter: number | null,
  ) {
    super(message)
  }
}

async function station<T>(path: string, body: { device_id: string; force: boolean }): Promise<T> {
  const { res, data } = await request('POST', path, body)
  if (!res.ok) {
    const message = data?.error ?? t('common.error', { status: res.status })
    throw new StationRefusal(res.status, message, String(data?.code ?? 'failed'), data?.device ?? null, typeof data?.retry_after === 'number' ? data.retry_after : null)
  }
  return data as T
}

export interface Session {
  configured: boolean
  authenticated: boolean
  hostname?: string | null
}

export interface Ticket {
  token: string
  user: string
  expires_in: number
  kyber_port: number | null
  hostname: string | null
}

export interface Live {
  name: string
  device_id: string
  kind: string
  started_at: number
}

/** La Station, pour la rubrique Station (absente avant la 0.5.0). */
export interface StationInfo {
  version: string
  update: { available: boolean; version: string | null; installable: boolean; requires_setup: boolean; installing: boolean }
  restarting: boolean
}

/** `GET /api/stream/info` : ce que l'accueil et les réglages montrent de l'ordinateur. */
export interface Info {
  hostname: string | null
  os?: string | null
  gpu: string | null
  virtual_display: { available: boolean; active: boolean }
  station?: StationInfo
}

export interface Preview {
  url: string
  capturedAt: number
}

export const backend = {
  session: () => call<Session & { version?: string }>('GET', '/api/session'),
  info: () => call<Info>('GET', '/api/stream/info'),
  /** Empreinte du certificat d'Admin (session d'Admin requise). */
  fingerprint: async (): Promise<string | null> => (await call<{ fingerprint: string | null }>('GET', '/api/pairing')).fingerprint ?? null,
  stationRestart: (body: { device_id: string; force: boolean }) => station<{ ok: boolean }>('/api/stream/station/restart', body),
  stationUpdate: (body: { device_id: string; force: boolean }) => station<{ ok: boolean; version: string }>('/api/stream/station/update', body),
  login: (password: string) => call('POST', '/api/login', { password }),
  ticket: () => call<Ticket>('GET', '/api/web/ticket'),
  live: async (): Promise<Live | null> => {
    const s = await call<{ activity?: { live: Live | null } }>('GET', '/api/status')
    return s.activity?.live ?? null
  },
  preview: async (width: number): Promise<Preview | null> => {
    const res = await fetch(`/api/stream/preview?width=${width}`, { credentials: 'same-origin' })
    if (!res.ok) return null
    const at = Date.parse(res.headers.get('x-captured-at') ?? '')
    return { url: URL.createObjectURL(await res.blob()), capturedAt: Number.isNaN(at) ? Date.now() : at }
  },
  prepare: (body: {
    mode: Mode
    audio: boolean
    virtualDisplay: boolean
    profile: string
    device: { id: string; name: string; kind: string; os: string; app: string }
  }) =>
    call<{ virtual_display: boolean; kyber_port: number | null; heartbeat_seconds: number }>('POST', '/api/stream/prepare', {
      ...body.mode,
      audio_channels: body.audio ? 2 : 0,
      virtual_display: body.virtualDisplay,
      device: body.device,
      profile: body.profile,
    }),
  heartbeat: (latency: number | null, profile: string, extra: Extra = {}) =>
    call<{ active: boolean; end: boolean }>('POST', '/api/stream/heartbeat', heartbeatBody(latency, profile, extra)),
  foreground: () => call<{ known: boolean; fullscreen: boolean; game: boolean }>('GET', '/api/stream/foreground'),
  end: (latency: number | null, extra: Extra = {}) => call('POST', '/api/stream/end', endBody(latency, extra)),
}

/** Champs en plus dans les corps du signal de vie et de la fin (par exemple
 * `device_id`) : la console ignore ceux qu'elle ne connaît pas. */
export type Extra = Record<string, unknown>

/** Corps du signal de vie (aussi envoyé par le Worker, `heartbeat.ts`). */
export function heartbeatBody(latency: number | null, profile: string, extra: Extra = {}) {
  return { ...extra, latency_ms: latency, profile }
}

/** Corps de la fin de session (aussi envoyé en `keepalive` quand la page se ferme). */
export function endBody(latency: number | null, extra: Extra = {}) {
  return { ...extra, latency_ms: latency }
}
