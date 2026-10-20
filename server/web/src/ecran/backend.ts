// Appels du client web à la console (même origine, session de la console).

import { ApiError } from '../lib/api'
import type { Mode } from './device'

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {}
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
  if (!res.ok) throw new ApiError(res.status, data?.error ?? `Erreur ${res.status}`)
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

export interface Preview {
  url: string
  capturedAt: number
}

export const backend = {
  session: () => call<Session>('GET', '/api/session'),
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
  heartbeat: (latency: number | null, profile: string) =>
    call<{ active: boolean; end: boolean }>('POST', '/api/stream/heartbeat', { latency_ms: latency, profile }),
  foreground: () => call<{ known: boolean; fullscreen: boolean; game: boolean }>('GET', '/api/stream/foreground'),
  end: (latency: number | null) => call('POST', '/api/stream/end', { latency_ms: latency }),
}
