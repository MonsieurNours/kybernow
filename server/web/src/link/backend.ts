// Appels du client web à la console (même origine, session de la console).

import { ApiError, type GuestImage, type ShareQuality, type ShareRole, type ShareView } from '../lib/api'
import { langHeaders, t } from '../lib/i18n/index.svelte'
import type { Mode } from './device'
import { viaRelay } from './plan'
import { deviceId } from './prefs'

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  const { res, data } = await request(method, path, body)
  if (!res.ok) throw new ApiError(res.status, data?.error ?? t('common.error', { status: res.status }), typeof data?.code === 'string' ? data.code : undefined)
  return data as T
}

/**
 * `json` : la réponse est du JSON (celle d'Admin) ; faux pour une page d'erreur du relais (`502`,
 * `503`, `504`) ou d'un mandataire. `headers` et `signal` : en plus (le secret d'un invité, l'attente
 * qu'on annule).
 */
async function request(
  method: string,
  path: string,
  body?: unknown,
  extra: { headers?: Record<string, string>; signal?: AbortSignal } = {},
): Promise<{ res: Response; data: any; json: boolean }> {
  // La console répond dans la langue de ce client (messages d'erreur).
  const headers: Record<string, string> = { ...langHeaders(), ...extra.headers }
  if (method !== 'GET') headers['x-kyber-admin'] = '1'
  if (body !== undefined) headers['content-type'] = 'application/json'
  const res = await fetch(path, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    credentials: 'same-origin',
    signal: extra.signal,
  })
  const text = await res.text()
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

/** Le relais (ou un mandataire) répond à la place d'Admin : une page d'erreur, pas du JSON. */
function relaySilent(res: Response, json: boolean): boolean {
  return !json && (res.status === 502 || res.status === 503 || res.status === 504)
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
  /**
   * La page est venue par le relais (docs/relais.md, « Partage ») : seul « Rejoindre un partage » est
   * ouvert, sans nom d'ordinateur (il n'est connu qu'après l'accord de l'hôte), image en WebSocket.
   */
  relay?: boolean
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
  /** KyberNow Share : absent pour une Station d'avant la 0.8.0 ; `enabled` est faux si `multi_client` est coupé. */
  share?: { available: boolean; enabled: boolean }
}

/** Écran que reçoit un invité (celui du partage). */
export interface GuestDisplay {
  id: number
  width: number
  height: number
  refresh: number
}

/** Réponse de `POST /api/share/join` (docs/kybernow-share.md, « Échange avec Admin »). */
export interface GuestJoin {
  guest: string
  secret: string
  role: ShareRole
  /** Prénom de l'hôte, ou nom de l'ordinateur pour un partage lancé depuis le PC. */
  host: string
  /** Nom de l'ordinateur. */
  computer: string
  kyber: { port: number; fingerprint: string; token: string; expires: number }
  display: GuestDisplay
  /** Format de l'image en cours (`h264`, `hevc`, `av1`), `null` si elle n'a pas encore démarré. */
  codec: string | null
  yuv444: boolean
  /** Secondes entre deux signes de vie. */
  beat: number
  /** Venu par le relais : l'image passe en WebSocket (raison `relay`), jamais en WebTransport. */
  relay?: boolean
}

/**
 * `POST /api/share/join` par le relais (`202`) : la demande attend l'accord de l'hôte. `guest` et
 * `secret` servent à l'attente (`POST /api/share/wait`) et à l'annuler (`leave`) ; `expires` : secondes
 * avant « L'hôte n'a pas répondu. ».
 */
export interface GuestPending {
  pending: true
  guest: string
  secret: string
  expires: number
}

/** Réponse de `POST /api/share/beat`. `end` : pourquoi l'invité est sorti. */
export interface GuestBeat {
  role: ShareRole
  token: string | null
  expires: number
  end: null | 'stopped' | 'removed'
  display?: GuestDisplay
  /** Son image : absente d'une Station qui ne connaît pas la qualité (avant le correctif Kyber 0020). */
  image?: GuestImage
}

/**
 * Pour « Auto » (`join`, `quality`) : ce que ce navigateur sait afficher et décoder. `width` et `height` :
 * le côté long et le côté court, en pixels réels ; `fps` : 60 au plus (un navigateur) ; `codecs` et
 * `yuv444` : ce que son décodeur accepte à cette taille.
 */
export interface AutoCaps {
  width: number
  height: number
  fps: number
  codecs: string[]
  yuv444: boolean
}

/**
 * Refus de `POST /api/share/join` ou de `POST /api/share/wait` : `kind` dit lequel (`message` n'est que
 * le texte d'Admin, pour `other`). Par le relais : `pending_full` (trois demandes en attente déjà),
 * `refused` (l'hôte a refusé), `expired` (pas de réponse en 120 s), `relay` (le relais ne répond pas).
 */
export class JoinRefusal extends Error {
  constructor(
    public kind:
      | 'no_share'
      | 'full'
      | 'too_many'
      | 'bad_request'
      | 'unavailable'
      | 'network'
      | 'pending_full'
      | 'refused'
      | 'expired'
      | 'relay'
      | 'other',
    message = '',
    /** Secondes à attendre (`too_many`). */
    public retryAfter: number | null = null,
  ) {
    super(message)
  }
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

  // KyberNow Share, côté invité (sans compte : le secret de l'invité, puis son cookie).
  /**
   * Rejoint le partage de cet ordinateur avec son code ; Admin pose le cookie de l'invité. `auto` : pour
   * « Auto ». Par le relais, la demande attend l'accord de l'hôte (`202`, `GuestPending`) : la suite
   * vient de `shareWait`.
   */
  joinShare: async (code: string, name: string, quality: ShareQuality = 'auto', auto?: AutoCaps): Promise<GuestJoin | GuestPending> => {
    let out: { res: Response; data: any; json: boolean }
    try {
      out = await request('POST', '/api/share/join', { code, name, device: deviceId(), app: 'web', quality, ...(auto ? { auto } : {}) })
    } catch {
      throw new JoinRefusal(viaRelay() ? 'relay' : 'network')
    }
    const { res, data, json } = out
    if (res.status === 202 && data?.pending) return data as GuestPending
    if (res.ok) return data as GuestJoin
    if (viaRelay() && relaySilent(res, json)) throw new JoinRefusal('relay')
    const wait = Number(data?.retry_after ?? res.headers.get('retry-after'))
    switch (res.status) {
      case 404:
        throw new JoinRefusal('no_share')
      case 403:
        throw new JoinRefusal(data?.error === 'refused' ? 'refused' : 'other', data?.error ?? t('common.error', { status: res.status }))
      case 409:
        throw new JoinRefusal(data?.error === 'full' ? 'full' : data?.error === 'pending_full' ? 'pending_full' : 'other', data?.error ?? '')
      case 429:
        throw new JoinRefusal('too_many', '', Number.isFinite(wait) && wait > 0 ? wait : null)
      case 400:
        throw new JoinRefusal('bad_request')
      case 503:
        throw new JoinRefusal('unavailable')
      default:
        throw new JoinRefusal('other', data?.error ?? t('common.error', { status: res.status }))
    }
  },

  /**
   * Attend l'accord de l'hôte pour une demande venue par le relais (réponse tenue 25 s au plus) : `null`
   * tant qu'elle attend (rappeler), la réponse de `join` une fois acceptée (Admin pose alors le cookie de
   * l'invité). Refus : `refused`, `expired`, `no_share` (partage arrêté) ; `relay` quand le relais ne
   * répond pas. Une attente annulée (`signal`) rejette avec l'erreur du navigateur.
   */
  shareWait: async (pending: GuestPending, signal?: AbortSignal): Promise<GuestJoin | null> => {
    let out: { res: Response; data: any; json: boolean }
    try {
      out = await request('POST', '/api/share/wait', { guest: pending.guest }, { headers: { 'x-kybernow-guest': pending.secret }, signal })
    } catch (e) {
      if (signal?.aborted) throw e
      throw new JoinRefusal('relay')
    }
    const { res, data, json } = out
    if (res.status === 202) return null
    if (res.ok) return data as GuestJoin
    if (relaySilent(res, json)) throw new JoinRefusal('relay')
    switch (res.status) {
      case 403:
        throw new JoinRefusal('refused')
      case 410:
        throw new JoinRefusal('expired')
      case 404:
        throw new JoinRefusal('no_share')
      default:
        throw new JoinRefusal('other', data?.error ?? t('common.error', { status: res.status }))
    }
  },

  /** Change la « Qualité de l'image » d'un invité en cours de partage : Kyber relance son image seulement. */
  shareQuality: async (join: GuestJoin, quality: ShareQuality, auto?: AutoCaps): Promise<GuestImage> => {
    const res = await fetch('/api/share/quality', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { ...langHeaders(), 'x-kyber-admin': '1', 'content-type': 'application/json', 'x-kybernow-guest': join.secret },
      body: JSON.stringify({ guest: join.guest, quality, ...(auto ? { auto } : {}) }),
    })
    let data: any = null
    try {
      data = await res.json()
    } catch {
      // Réponse vide ou illisible.
    }
    if (res.status === 429) {
      // Un changement par seconde et par invité : la Station dit quand refaire la demande.
      const wait = Number(data?.retry_after ?? res.headers.get('retry-after'))
      throw new ApiError(429, data?.error ?? 'too_many', 'too_many', Number.isFinite(wait) && wait > 0 ? wait : 1)
    }
    if (!res.ok) throw new ApiError(res.status, data?.error ?? t('common.error', { status: res.status }))
    return data as GuestImage
  },

  // KyberNow Share, côté hôte (la session d'Admin, avec l'appareil en session).
  hostShare: {
    open: () => call<ShareView>('POST', '/api/share/open', { device_id: deviceId() }),
    get: () => call<ShareView | { open: false }>('GET', `/api/share?device_id=${encodeURIComponent(deviceId())}`),
    code: () => call<ShareView>('POST', '/api/share/code', { device_id: deviceId() }),
    role: (id: string, role: ShareRole) => call<ShareView>('POST', `/api/share/guest/${encodeURIComponent(id)}`, { role, device_id: deviceId() }),
    remove: (id: string) => call<ShareView>('DELETE', `/api/share/guest/${encodeURIComponent(id)}?device_id=${encodeURIComponent(deviceId())}`),
    stop: () => call<{ open: false }>('DELETE', `/api/share?device_id=${encodeURIComponent(deviceId())}`),
    /** Une demande venue par le relais : l'hôte l'accepte ou la refuse. */
    accept: (id: string) => call<ShareView>('POST', `/api/share/guest/${encodeURIComponent(id)}/accept`, { device_id: deviceId() }),
    refuse: (id: string) => call<ShareView>('POST', `/api/share/guest/${encodeURIComponent(id)}/refuse`, { device_id: deviceId() }),
  },
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
